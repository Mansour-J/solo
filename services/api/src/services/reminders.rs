//! Invoice reminders (SL-702): opt-in emails before and after an invoice's due date, sent through the
//! owner's email when the app runs, logged with the invoice's emails, with a per-customer opt-out.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::money::Currency;
use crate::domain::reminders;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::customers as customer_repo;
use crate::repositories::invoices::{self as invoice_repo, NewEmailLog};
use crate::services::documents::{self, DocumentType};
use crate::services::email::{EmailAddress, EmailAttachment, templates};
use crate::services::invoices::load_business;
use crate::services::{businesses, email_settings};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct ReminderSettings {
    /// Reminders are sent (off until turned on).
    pub reminders_enabled: bool,
    /// Days before the due date for the "coming up" reminder; `null` for none.
    pub reminder_days_before: Option<i32>,
    /// Days after the due date for overdue reminders (up to six).
    pub reminder_days_after: Vec<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CustomerRemindersInput {
    pub send_reminders: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SentReminder {
    pub invoice_id: Uuid,
    pub number: String,
    pub to_email: String,
    /// The reminder's day relative to the due date (-3 = three days before).
    pub day: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReminderRun {
    pub sent: Vec<SentReminder>,
    /// Why reminders stopped (email not set up, or the mail server refused), if they did.
    pub stopped: Option<String>,
}

async fn settings_of(
    conn: &mut PgConnection,
    business_id: Uuid,
) -> Result<ReminderSettings, sqlx::Error> {
    sqlx::query_as(
        "SELECT reminders_enabled, reminder_days_before, reminder_days_after FROM business_settings WHERE business_id = $1",
    )
    .bind(business_id)
    .fetch_one(conn)
    .await
}

pub async fn get_settings(state: &AppState, ctx: &BusinessContext) -> AppResult<ReminderSettings> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let settings = settings_of(&mut tx, ctx.business_id).await?;
    tx.commit().await?;
    Ok(settings)
}

pub async fn save_settings(
    state: &AppState,
    ctx: &BusinessContext,
    mut input: ReminderSettings,
) -> AppResult<ReminderSettings> {
    let mut errors = FieldErrors::new();
    if let Some(d) = input.reminder_days_before {
        errors.check(
            (1..=60).contains(&d),
            "reminder_days_before",
            "Use between 1 and 60 days, or leave it empty.",
        );
    }
    input.reminder_days_after.sort_unstable();
    input.reminder_days_after.dedup();
    errors.check(
        input
            .reminder_days_after
            .iter()
            .all(|d| (1..=365).contains(d)),
        "reminder_days_after",
        "Use days between 1 and 365 after the due date.",
    );
    errors.check(
        input.reminder_days_after.len() <= 6,
        "reminder_days_after",
        "Use up to six reminders after the due date.",
    );
    errors.check(
        !input.reminders_enabled
            || input.reminder_days_before.is_some()
            || !input.reminder_days_after.is_empty(),
        "reminder_days_after",
        "Choose at least one reminder.",
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = settings_of(&mut tx, ctx.business_id).await?;
    sqlx::query(
        "UPDATE business_settings SET reminders_enabled = $2, reminder_days_before = $3, reminder_days_after = $4
         WHERE business_id = $1",
    )
    .bind(ctx.business_id)
    .bind(input.reminders_enabled)
    .bind(input.reminder_days_before)
    .bind(&input.reminder_days_after)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "settings.reminders_updated",
            entity_type: "business",
            entity_id: Some(ctx.business_id),
            summary: if input.reminders_enabled {
                "Turned on invoice reminders".into()
            } else {
                "Updated invoice reminders (off)".into()
            },
            changes: json!({ "before": before, "after": input }),
        },
    )
    .await?;
    let saved = settings_of(&mut tx, ctx.business_id).await?;
    tx.commit().await?;
    Ok(saved)
}

/// Turns reminders on or off for one customer.
pub async fn set_customer(
    state: &AppState,
    ctx: &BusinessContext,
    customer_id: Uuid,
    input: CustomerRemindersInput,
) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    if !customer_repo::set_send_reminders(&mut tx, customer_id, input.send_reminders).await? {
        return Err(AppError::NotFound("customer"));
    }
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "customer.reminders_updated",
            entity_type: "customer",
            entity_id: Some(customer_id),
            summary: if input.send_reminders {
                "Turned on invoice reminders for a customer".into()
            } else {
                "Turned off invoice reminders for a customer".into()
            },
            changes: json!({ "send_reminders": input.send_reminders }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Candidate {
    invoice_id: Uuid,
    number: Option<String>,
    due_date: NaiveDate,
    owing: i64,
    customer_name: String,
    email: String,
}

async fn candidates(
    conn: &mut PgConnection,
    today: NaiveDate,
) -> Result<Vec<Candidate>, sqlx::Error> {
    sqlx::query_as(
        "SELECT b.invoice_id, b.number, b.due_date, (b.total - b.paid)::bigint AS owing, c.name AS customer_name,
                c.email::text AS email
         FROM invoice_balances b JOIN customers c ON c.id = b.customer_id
         WHERE b.status = 'issued' AND b.total > b.paid AND c.send_reminders AND c.email IS NOT NULL
           AND b.due_date BETWEEN $1::date - 400 AND $1::date + 60
         ORDER BY b.due_date, b.number",
    )
    .bind(today)
    .fetch_all(conn)
    .await
}

async fn already_sent(
    conn: &mut PgConnection,
    invoice_id: Uuid,
    day: i32,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM email_logs WHERE document_type = 'invoice' AND document_id = $1
                          AND reminder_day = $2 AND status = 'sent')",
    )
    .bind(invoice_id)
    .bind(day)
    .fetch_one(conn)
    .await
}

fn wording(business_name: &str, c: &Candidate, owing: &str, today: NaiveDate) -> (String, String) {
    let number = c.number.clone().unwrap_or_default();
    let due = c.due_date.format("%-d %B %Y");
    if today <= c.due_date {
        let when = if today == c.due_date {
            "today".to_owned()
        } else {
            format!("on {due}")
        };
        (
            format!("Reminder: invoice {number} from {business_name} is due {when}"),
            format!(
                "Hi {},\n\nA friendly reminder that invoice {number} for {owing} is due {when}. The invoice is attached.\n\nIf you’ve already paid, thank you, and please ignore this email.",
                c.customer_name
            ),
        )
    } else {
        (
            format!("Overdue: invoice {number} from {business_name}"),
            format!(
                "Hi {},\n\nInvoice {number} was due on {due} and {owing} is still owing. The invoice is attached.\n\nIf you’ve already paid, thank you, and please ignore this email. If something’s wrong with the invoice, just reply.",
                c.customer_name
            ),
        )
    }
}

/// Sends the reminders due today for the business. Safe to call often: each reminder goes once.
pub async fn run(state: &AppState, ctx: &BusinessContext) -> AppResult<ReminderRun> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let settings = settings_of(&mut tx, ctx.business_id).await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = businesses::business_today(&business.timezone);
    // The demo's customers are made up: it never sends reminders.
    if !settings.reminders_enabled || business.is_demo {
        tx.commit().await?;
        return Ok(ReminderRun {
            sent: Vec::new(),
            stopped: None,
        });
    }
    let schedule =
        reminders::schedule(settings.reminder_days_before, &settings.reminder_days_after);
    let mut due = Vec::new();
    for c in candidates(&mut tx, today).await? {
        if let Some(day) = reminders::due_reminder(c.due_date, today, &schedule)
            && !already_sent(&mut tx, c.invoice_id, day).await?
        {
            due.push((c, day));
        }
    }
    tx.commit().await?;
    if due.is_empty() {
        return Ok(ReminderRun {
            sent: Vec::new(),
            stopped: None,
        });
    }
    let Some(mailer) = email_settings::mailer(state, ctx.user_id()).await? else {
        return Ok(ReminderRun {
            sent: Vec::new(),
            stopped: Some("Email isn’t set up, so reminders can’t be sent.".into()),
        });
    };
    let currency = Currency::from_code(&business.currency)
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("unknown currency")))?;
    let business_name = if business.trading_name.is_empty() {
        business.name.clone()
    } else {
        business.trading_name.clone()
    };
    let mut sent = Vec::new();
    let mut stopped = None;
    for (c, day) in due {
        let doc =
            documents::render_document(state, ctx, DocumentType::Invoice, c.invoice_id).await?;
        let owing = currency.format(c.owing);
        let (subject, message) = wording(&business_name, &c, &owing, today);
        let summary = vec![
            format!("Amount owing: {owing}"),
            format!("Due date: {}", c.due_date.format("%-d %b %Y")),
        ];
        let mut email = templates::document(
            &state.config.brand,
            EmailAddress::new(c.customer_name.clone(), c.email.clone()),
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
            filename: doc.filename,
            content_type: "application/pdf".into(),
            bytes: doc.bytes,
        });
        let result = mailer.send_as(&business_name, email).await;
        let mut tx = state
            .db
            .begin_business(ctx.user_id(), ctx.business_id)
            .await?;
        let (status, reference, error) = match &result {
            Ok(r) => ("sent", Some(r.provider_reference.clone()), None),
            Err(e) => ("failed", None, Some(e.to_string())),
        };
        invoice_repo::log_email(
            &mut tx,
            NewEmailLog {
                business_id: ctx.business_id,
                document_type: "invoice",
                document_id: c.invoice_id,
                to_email: &c.email,
                subject: &subject,
                status,
                provider_reference: reference.as_deref(),
                error: error.as_deref(),
                sent_by: ctx.user_id(),
                reminder_day: Some(day),
            },
        )
        .await?;
        let number = c.number.clone().unwrap_or_default();
        if result.is_ok() {
            audit::record(
                &mut tx,
                &ctx.actor(),
                audit::Entry {
                    business_id: Some(ctx.business_id),
                    action: "invoice.reminder_sent",
                    entity_type: "invoice",
                    entity_id: Some(c.invoice_id),
                    summary: format!("Sent a reminder for invoice {number} to {}", c.email),
                    changes: json!({ "to": c.email, "day": day }),
                },
            )
            .await?;
        }
        tx.commit().await?;
        match result {
            Ok(_) => sent.push(SentReminder {
                invoice_id: c.invoice_id,
                number,
                to_email: c.email,
                day,
            }),
            Err(e) => {
                // The mail server refused: the rest would fail the same way, so try again next time.
                stopped = Some(format!(
                    "A reminder couldn’t be sent: {e}. Check Settings › Email."
                ));
                break;
            }
        }
    }
    Ok(ReminderRun { sent, stopped })
}
