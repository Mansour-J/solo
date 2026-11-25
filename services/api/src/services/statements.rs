//! Emailing a customer their statement of account (SL-513): the statement PDF for a period, sent
//! through the owner's email, logged with the customer like invoice emails.

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::money::Currency;
use crate::domain::reports::{Cell, Report};
use crate::domain::validation;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::customers as customer_repo;
use crate::repositories::invoices::{self as invoice_repo, EmailLogRow, NewEmailLog};
use crate::services::email::{EmailAddress, EmailAttachment, templates};
use crate::services::invoices::load_business;
use crate::services::reports::{self, ReportKind, ReportParams};
use crate::services::{businesses, email_settings, report_export};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SendStatementInput {
    /// The statement's period (defaults to the current financial year to date).
    #[serde(default)]
    pub from: Option<NaiveDate>,
    #[serde(default)]
    pub to: Option<NaiveDate>,
    /// Defaults to the customer's email.
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    /// Defaults to a short greeting.
    #[serde(default)]
    pub message: Option<String>,
}

/// The amount owing at the end of the statement (its summary's total).
fn amount_owing(report: &Report) -> Option<i64> {
    let summary = report.sections.iter().find(|s| s.title == "Summary")?;
    match summary.totals.as_ref()?.get(1)? {
        Cell::Integer(v) => Some(*v),
        _ => None,
    }
}

/// Sends the statement and returns the customer's statement email history.
pub async fn send(
    state: &AppState,
    ctx: &BusinessContext,
    customer_id: Uuid,
    input: SendStatementInput,
) -> AppResult<Vec<EmailLogRow>> {
    let Some(mailer) = email_settings::mailer(state, ctx.user_id()).await? else {
        return Err(AppError::Conflict(
            "Email isn’t set up yet: add your email account in Settings › Email, or download the PDF and send it yourself.".into(),
        ));
    };
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = businesses::business_today(&business.timezone);
    let customer = customer_repo::find(&mut tx, today, customer_id)
        .await?
        .ok_or(AppError::NotFound("customer"))?;
    tx.commit().await?;

    let mut errors = FieldErrors::new();
    let to = validation::normalize_email(
        input
            .email
            .as_deref()
            .or(customer.email.as_deref())
            .unwrap_or_default(),
    );
    errors.check(
        validation::is_valid_email(&to),
        "email",
        "Enter the customer’s email address.",
    );
    let business_name = if business.trading_name.is_empty() {
        business.name.clone()
    } else {
        business.trading_name.clone()
    };
    let subject = input
        .subject
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Statement from {business_name}"));
    errors.check(
        subject.chars().count() <= 200,
        "subject",
        "Use 200 characters or fewer.",
    );
    let message = input
        .message
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| {
            format!(
                "Hi {},\n\nPlease find your statement attached.",
                customer.name
            )
        });
    errors.check(
        message.chars().count() <= 4000,
        "message",
        "Use 4000 characters or fewer.",
    );
    errors.into_result()?;

    let report = reports::build(
        state,
        ctx,
        ReportKind::StatementOfAccount,
        &ReportParams {
            from: input.from,
            to: input.to,
            customer_id: Some(customer_id),
            ..ReportParams::default()
        },
    )
    .await?;
    let brand = &state.config.brand;
    let pdf = report_export::to_pdf(&report, &brand.primary_color, &brand.name).await?;
    let currency = Currency::from_code(&business.currency)
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("unknown currency")))?;
    let mut summary = Vec::new();
    if let Some(period) = &report.period {
        summary.push(format!(
            "Period: {} to {}",
            period.from.format("%-d %b %Y"),
            period.to.format("%-d %b %Y")
        ));
    }
    if let Some(owing) = amount_owing(&report) {
        summary.push(format!("Amount owing: {}", currency.format(owing)));
    }
    let mut email = templates::document(
        brand,
        EmailAddress::new(customer.name.clone(), to.clone()),
        &business_name,
        subject.clone(),
        &message,
        &summary,
    );
    email.reply_to = business
        .email
        .as_ref()
        .map(|e| EmailAddress::new(business_name.clone(), e.clone()));
    email.attachments.push(EmailAttachment {
        filename: report_export::file_name(&report, report_export::ExportFormat::Pdf),
        content_type: "application/pdf".into(),
        bytes: pdf,
    });
    let result = mailer.send_as(&business_name, email).await;

    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let (status, reference, error) = match &result {
        Ok(receipt) => ("sent", Some(receipt.provider_reference.clone()), None),
        Err(e) => ("failed", None, Some(e.to_string())),
    };
    invoice_repo::log_email(
        &mut tx,
        NewEmailLog {
            business_id: ctx.business_id,
            document_type: "statement",
            document_id: customer_id,
            to_email: &to,
            subject: &subject,
            status,
            provider_reference: reference.as_deref(),
            error: error.as_deref(),
            sent_by: ctx.user_id(),
            reminder_day: None,
        },
    )
    .await?;
    if result.is_ok() {
        audit::record(
            &mut tx,
            &ctx.actor(),
            audit::Entry {
                business_id: Some(ctx.business_id),
                action: "customer.statement_sent",
                entity_type: "customer",
                entity_id: Some(customer_id),
                summary: format!("Emailed a statement to {} ({to})", customer.name),
                changes: json!({ "to": to, "period": report.period }),
            },
        )
        .await?;
    }
    let history = invoice_repo::email_history(&mut tx, "statement", customer_id).await?;
    tx.commit().await?;
    if let Err(e) = result {
        return Err(AppError::Conflict(format!(
            "The statement couldn’t be sent: {e}. Check Settings › Email."
        )));
    }
    Ok(history)
}

/// The customer's statement emails, newest first.
pub async fn history(
    state: &AppState,
    ctx: &BusinessContext,
    customer_id: Uuid,
) -> AppResult<Vec<EmailLogRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = invoice_repo::email_history(&mut tx, "statement", customer_id).await?;
    tx.commit().await?;
    Ok(rows)
}
