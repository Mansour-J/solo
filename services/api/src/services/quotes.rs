//! Quotes: create, edit, send, accept or reject, and convert into an invoice.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::documents::{self, LineInput, QuoteDisplayStatus};
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses::{self, BusinessRow};
use crate::repositories::customers;
use crate::repositories::documents::{self as docs, DocKind, ItemRow};
use crate::repositories::invoices::{self as invoice_repo, EmailLogRow};
use crate::repositories::quotes::{self, ListFilter, QuoteFields, QuoteRow};
use crate::services::businesses::business_today;
use crate::services::invoices::{
    self, InvoiceCustomer, InvoiceInput, clean, gst_context, line_errors, load_business,
};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct QuoteInput {
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    /// Defaults to the issue date plus the business's quote validity.
    #[serde(default)]
    pub expiry_date: Option<NaiveDate>,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub prices_include_gst: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub footer: String,
    pub lines: Vec<LineInput>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct QuoteSummary {
    pub id: Uuid,
    pub number: String,
    pub status: String,
    pub display_status: QuoteDisplayStatus,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub total: i64,
    pub converted_invoice_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Quote {
    pub id: Uuid,
    pub number: String,
    pub status: String,
    pub display_status: QuoteDisplayStatus,
    pub customer: InvoiceCustomer,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub prices_include_gst: bool,
    pub notes: String,
    pub footer: String,
    pub lines: Vec<ItemRow>,
    pub subtotal: i64,
    pub discount_total: i64,
    pub gst_total: i64,
    pub total: i64,
    pub emails: Vec<EmailLogRow>,
    pub converted_invoice_id: Option<Uuid>,
    pub sent_at: Option<DateTime<Utc>>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteDecision {
    Sent,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct QuoteStatusInput {
    pub status: QuoteDecision,
}

fn parse_status(value: String) -> AppResult<QuoteDisplayStatus> {
    serde_json::from_value(serde_json::Value::String(value)).map_err(AppError::internal)
}

pub async fn list(
    state: &AppState,
    ctx: &BusinessContext,
    filter: ListFilter,
) -> AppResult<(Vec<QuoteSummary>, i64)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (rows, total) = quotes::list(&mut tx, business_today(&business.timezone), &filter).await?;
    tx.commit().await?;
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(QuoteSummary {
                display_status: parse_status(r.display_status)?,
                id: r.id,
                number: r.number,
                status: r.status,
                customer_id: r.customer_id,
                customer_name: r.customer_name,
                issue_date: r.issue_date,
                expiry_date: r.expiry_date,
                reference: r.reference,
                currency: r.currency,
                total: r.total,
                converted_invoice_id: r.converted_invoice_id,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok((items, total))
}

async fn load(conn: &mut PgConnection, business: &BusinessRow, id: Uuid) -> AppResult<Quote> {
    let row: QuoteRow = quotes::find(conn, id)
        .await?
        .ok_or(AppError::NotFound("Quote"))?;
    let today = business_today(&business.timezone);
    let customer = customers::find(conn, today, row.customer_id)
        .await?
        .ok_or(AppError::NotFound("Customer"))?;
    let lines = docs::items(conn, DocKind::Quote, id).await?;
    let emails = invoice_repo::email_history(conn, "quote", id).await?;
    Ok(Quote {
        display_status: documents::quote_display_status(
            &row.status,
            row.expiry_date,
            today,
            row.converted_invoice_id.is_some(),
        ),
        customer: InvoiceCustomer {
            id: customer.id,
            name: customer.name,
            business_name: customer.business_name,
            contact_person: customer.contact_person,
            email: customer.email,
            billing_address: serde_json::from_value(customer.billing_address).unwrap_or_default(),
        },
        id: row.id,
        number: row.number,
        status: row.status,
        issue_date: row.issue_date,
        expiry_date: row.expiry_date,
        reference: row.reference,
        currency: row.currency,
        prices_include_gst: row.prices_include_gst,
        notes: row.notes,
        footer: row.footer,
        lines,
        subtotal: row.subtotal,
        discount_total: row.discount_total,
        gst_total: row.gst_total,
        total: row.total,
        emails,
        converted_invoice_id: row.converted_invoice_id,
        sent_at: row.sent_at,
        decided_at: row.decided_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub async fn get(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<Quote> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let quote = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(quote)
}

pub async fn load_for_document(
    conn: &mut PgConnection,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<Quote> {
    load(conn, business, id).await
}

async fn prepare(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: QuoteInput,
) -> AppResult<(
    QuoteFields,
    Vec<documents::CalculatedLine>,
    documents::Totals,
)> {
    let mut errors = FieldErrors::new();
    let today = business_today(&business.timezone);
    match customers::find(conn, today, input.customer_id).await? {
        None => errors.add("customer_id", "Choose a customer."),
        Some(c) if !c.is_active => errors.add(
            "customer_id",
            "This customer is inactive. Reactivate it first.",
        ),
        _ => {}
    }
    let settings = businesses::settings(conn, business.id).await?;
    let expiry_date = input.expiry_date.unwrap_or(
        input.issue_date + Duration::days(i64::from(settings.default_quote_validity_days)),
    );
    errors.check(
        expiry_date >= input.issue_date,
        "expiry_date",
        "The expiry date can’t be before the quote date.",
    );
    let reference = clean(&input.reference, 100, "reference", &mut errors);
    let notes = clean(&input.notes, 4000, "notes", &mut errors);
    let footer = clean(&input.footer, 2000, "footer", &mut errors);
    let gst = gst_context(conn, business, input.issue_date).await?;
    let calculated = documents::calculate(&input.lines, input.prices_include_gst, gst);
    if let Err(problems) = &calculated {
        for e in problems {
            errors.add(e.field.clone(), e.message);
        }
    }
    errors.into_result()?;
    let (lines, totals) = calculated.map_err(line_errors)?;
    Ok((
        QuoteFields {
            customer_id: input.customer_id,
            issue_date: input.issue_date,
            expiry_date,
            reference,
            currency: business.currency.clone(),
            prices_include_gst: input.prices_include_gst,
            notes,
            footer,
        },
        lines,
        totals,
    ))
}

async fn record(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    action: &str,
    id: Uuid,
    summary: String,
) -> AppResult<()> {
    audit::record(
        conn,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action,
            entity_type: "quote",
            entity_id: Some(id),
            summary,
            changes: serde_json::json!({}),
        },
    )
    .await
    .map_err(Into::into)
}

pub async fn create(
    state: &AppState,
    ctx: &BusinessContext,
    input: QuoteInput,
) -> AppResult<Quote> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (fields, lines, totals) = prepare(&mut tx, &business, input).await?;
    let number = docs::next_number(&mut tx, DocKind::Quote, ctx.business_id).await?;
    let id = quotes::insert(
        &mut tx,
        ctx.business_id,
        ctx.user_id(),
        &number,
        &fields,
        totals,
    )
    .await?;
    docs::replace_items(&mut tx, DocKind::Quote, ctx.business_id, id, &lines).await?;
    record(
        &mut tx,
        ctx,
        "quote.created",
        id,
        format!("Quote {number} created"),
    )
    .await?;
    let quote = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(quote)
}

pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: QuoteInput,
) -> AppResult<Quote> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = quotes::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Quote"))?;
    if before.converted_invoice_id.is_some()
        || matches!(before.status.as_str(), "accepted" | "rejected")
    {
        return Err(AppError::Conflict(
            "This quote has been decided and can’t be changed.".into(),
        ));
    }
    let (fields, lines, totals) = prepare(&mut tx, &business, input).await?;
    quotes::update(&mut tx, id, &fields, totals).await?;
    docs::replace_items(&mut tx, DocKind::Quote, ctx.business_id, id, &lines).await?;
    record(
        &mut tx,
        ctx,
        "quote.updated",
        id,
        format!("Quote {} updated", before.number),
    )
    .await?;
    let quote = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(quote)
}

pub async fn set_status(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    decision: QuoteDecision,
) -> AppResult<Quote> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let quote = quotes::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Quote"))?;
    if quote.converted_invoice_id.is_some() {
        return Err(AppError::Conflict(
            "This quote was converted to an invoice.".into(),
        ));
    }
    let (status, verb) = match decision {
        QuoteDecision::Sent => ("sent", "marked as sent"),
        QuoteDecision::Accepted => ("accepted", "accepted"),
        QuoteDecision::Rejected => ("rejected", "rejected"),
    };
    if decision == QuoteDecision::Accepted
        && quote.expiry_date < business_today(&business.timezone)
        && quote.status != "accepted"
    {
        return Err(AppError::Conflict(
            "This quote has expired. Update its expiry date before accepting it.".into(),
        ));
    }
    quotes::set_status(&mut tx, id, status).await?;
    record(
        &mut tx,
        ctx,
        &format!("quote.{status}"),
        id,
        format!("Quote {} {verb}", quote.number),
    )
    .await?;
    let quote = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(quote)
}

/// Creates a draft invoice from an accepted (or sent) quote, keeping customer and lines.
pub async fn convert(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<Uuid> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let quote = quotes::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Quote"))?;
    if let Some(invoice_id) = quote.converted_invoice_id {
        return Err(AppError::Conflict(format!(
            "This quote was already converted (invoice {invoice_id})."
        )));
    }
    if quote.status == "rejected" {
        return Err(AppError::Conflict(
            "Rejected quotes can’t be converted.".into(),
        ));
    }
    let items = docs::items(&mut tx, DocKind::Quote, id).await?;
    let input = InvoiceInput {
        customer_id: quote.customer_id,
        issue_date: business_today(&business.timezone),
        due_date: None,
        reference: if quote.reference.is_empty() {
            quote.number.clone()
        } else {
            quote.reference.clone()
        },
        prices_include_gst: quote.prices_include_gst,
        notes: Some(quote.notes.clone()),
        footer: Some(quote.footer.clone()),
        lines: items
            .iter()
            .map(|i| LineInput {
                description: i.description.clone(),
                quantity: i.quantity,
                unit_price: i.unit_price,
                discount_percent: i.discount_percent,
                tax_treatment: i.tax_treatment,
            })
            .collect(),
    };
    let invoice_id = invoices::create_draft_in(&mut tx, ctx, &business, input, Some(id)).await?;
    quotes::set_converted(&mut tx, id, invoice_id).await?;
    record(
        &mut tx,
        ctx,
        "quote.converted",
        id,
        format!("Quote {} converted to an invoice", quote.number),
    )
    .await?;
    tx.commit().await?;
    Ok(invoice_id)
}

pub async fn delete(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let quote = quotes::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Quote"))?;
    if quote.converted_invoice_id.is_some() {
        return Err(AppError::Conflict(
            "This quote was converted to an invoice and is kept for the record.".into(),
        ));
    }
    quotes::delete(&mut tx, id).await?;
    record(
        &mut tx,
        ctx,
        "quote.deleted",
        id,
        format!("Quote {} deleted", quote.number),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn mark_sent_in(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    let quote = quotes::find(conn, id)
        .await?
        .ok_or(AppError::NotFound("Quote"))?;
    if quote.status == "draft" {
        quotes::set_status(conn, id, "sent").await?;
    }
    Ok(())
}
