//! Invoices: drafts, issuing, editing, cancellation and payments, with ledger postings kept in sync.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::Address;
use crate::domain::documents::{self, CalculatedLine, InvoiceDisplayStatus, LineInput, Totals};
use crate::domain::ledger;
use crate::domain::tax::GstContext;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::allocations::SourceRef;
use crate::repositories::businesses::{self, BusinessRow};
use crate::repositories::customers;
use crate::repositories::documents::{self as docs, DocKind, ItemRow};
use crate::repositories::invoices::{
    self, EmailLogRow, InvoiceFields, InvoiceRow, ListFilter, PaymentFields, PaymentRow,
};
use crate::repositories::ledger::{self as ledger_repo, JournalEntryView};
use crate::repositories::{audit, tax_rates};
use crate::services::allocations::ReceiptInfo;
use crate::services::businesses::business_today;
use crate::services::hooks;
use crate::services::ledger::{
    self as ledger_service, Context, Desired, SOURCE_INVOICE, SOURCE_PAYMENT,
};
use crate::state::AppState;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct InvoiceInput {
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    /// Defaults to the issue date plus the customer's (or business's) payment terms.
    #[serde(default)]
    pub due_date: Option<NaiveDate>,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub prices_include_gst: bool,
    /// Message shown on the invoice. Defaults to the business's default notes for new invoices.
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub footer: Option<String>,
    pub lines: Vec<LineInput>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CalculateInput {
    pub issue_date: NaiveDate,
    #[serde(default)]
    pub prices_include_gst: bool,
    pub lines: Vec<LineInput>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Calculation {
    pub lines: Vec<CalculatedLine>,
    pub totals: Totals,
    /// Whether GST applies on the issue date.
    pub gst_registered: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvoiceSummary {
    pub id: Uuid,
    pub number: Option<String>,
    pub status: String,
    pub display_status: InvoiceDisplayStatus,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub total: i64,
    pub paid: i64,
    pub balance: i64,
    pub sent_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InvoiceCustomer {
    pub id: Uuid,
    pub name: String,
    pub business_name: String,
    pub contact_person: String,
    pub email: Option<String>,
    pub billing_address: Address,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Invoice {
    pub id: Uuid,
    pub number: Option<String>,
    pub status: String,
    pub display_status: InvoiceDisplayStatus,
    pub customer: InvoiceCustomer,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
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
    pub paid: i64,
    pub balance: i64,
    pub payments: Vec<PaymentRow>,
    pub emails: Vec<EmailLogRow>,
    /// Every journal entry posted for this invoice and its payments, including reversals.
    pub journal: Vec<JournalEntryView>,
    pub quote_id: Option<Uuid>,
    pub issued_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    BankTransfer,
    Cash,
    Card,
    Cheque,
    Online,
    Other,
    /// What's left written off as a bad debt: no money received.
    WriteOff,
}

impl PaymentMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BankTransfer => "bank_transfer",
            Self::Cash => "cash",
            Self::Card => "card",
            Self::Cheque => "cheque",
            Self::Online => "online",
            Self::Other => "other",
            Self::WriteOff => "write_off",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PaymentInput {
    pub payment_date: NaiveDate,
    /// Minor units settled on the invoice (including any tax withheld).
    pub amount: i64,
    /// Part of `amount` the customer withheld and paid to IRD for you (schedular payments).
    #[serde(default)]
    pub withholding_tax: i64,
    pub method: PaymentMethod,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReasonInput {
    #[serde(default)]
    pub reason: String,
}

// ─── Helpers shared with quotes, expenses and income ────────────────────────

/// GST position of the business on `date`, using the versioned rate table.
pub async fn gst_context(
    conn: &mut PgConnection,
    business: &BusinessRow,
    date: NaiveDate,
) -> AppResult<GstContext> {
    let rate = if business.gst_registered {
        tax_rates::rate_on(conn, "NZ", "gst", date).await?
    } else {
        None
    };
    Ok(GstContext::for_date(
        business.gst_registered,
        business.gst_registered_from,
        date,
        rate,
    ))
}

pub async fn load_business(conn: &mut PgConnection, business_id: Uuid) -> AppResult<BusinessRow> {
    businesses::find(conn, business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))
}

/// Field errors from line validation, shaped like the request.
pub fn line_errors(errors: Vec<documents::LineError>) -> AppError {
    let mut fields = FieldErrors::new();
    for e in errors {
        fields.add(e.field, e.message);
    }
    AppError::Validation(fields)
}

pub fn clean(value: &str, max: usize, field: &str, errors: &mut FieldErrors) -> String {
    let value = value.trim();
    if value.chars().count() > max {
        errors.add(field, format!("Use {max} characters or fewer."));
    }
    value.to_owned()
}

fn ctx_for(ctx: &BusinessContext, business: &BusinessRow) -> Context {
    Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    }
}

fn summary(row: invoices::InvoiceListRow) -> AppResult<InvoiceSummary> {
    let display_status = serde_json::from_value(serde_json::Value::String(row.display_status))
        .map_err(AppError::internal)?;
    Ok(InvoiceSummary {
        balance: row.total - row.paid,
        id: row.id,
        number: row.number,
        status: row.status,
        display_status,
        customer_id: row.customer_id,
        customer_name: row.customer_name,
        issue_date: row.issue_date,
        due_date: row.due_date,
        reference: row.reference,
        currency: row.currency,
        total: row.total,
        paid: row.paid,
        sent_at: row.sent_at,
    })
}

/// Makes the journal reflect the invoice and its payments as they are now.
async fn sync_postings(
    conn: &mut PgConnection,
    context: &Context,
    invoice_id: Uuid,
) -> AppResult<()> {
    let invoice = invoices::find(conn, invoice_id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    let payments = invoices::payments(conn, invoice_id).await?;
    let mut desired = Vec::new();
    if invoice.status == "issued"
        && let Some(number) = &invoice.number
    {
        desired.push(Desired {
            source_type: SOURCE_INVOICE,
            source_id: invoice.id,
            spec: ledger::invoice_issued(
                invoice.issue_date,
                number,
                invoice.customer_id,
                invoice.subtotal,
                invoice.gst_total,
            ),
        });
        let mut paid_before = 0;
        for payment in payments.iter().filter(|p| p.voided_at.is_none()) {
            if payment.method == "write_off" {
                desired.push(Desired {
                    source_type: SOURCE_PAYMENT,
                    source_id: payment.id,
                    spec: ledger::invoice_write_off(
                        payment.payment_date,
                        number,
                        invoice.customer_id,
                        payment.amount,
                        invoice.subtotal,
                        invoice.gst_total,
                        paid_before,
                    ),
                });
                paid_before += payment.amount;
                continue;
            }
            for spec in ledger::invoice_payment(
                payment.payment_date,
                number,
                invoice.customer_id,
                payment.amount,
                payment.withholding_tax,
                invoice.subtotal,
                invoice.gst_total,
                paid_before,
            ) {
                desired.push(Desired {
                    source_type: SOURCE_PAYMENT,
                    source_id: payment.id,
                    spec,
                });
            }
            paid_before += payment.amount;
        }
    }
    let mut sources: Vec<(&'static str, Uuid)> = vec![(SOURCE_INVOICE, invoice.id)];
    sources.extend(payments.iter().map(|p| (SOURCE_PAYMENT, p.id)));
    ledger_service::reconcile(conn, context, &sources, desired).await
}

// ─── Queries ────────────────────────────────────────────────────────────────

pub async fn calculate(
    state: &AppState,
    ctx: &BusinessContext,
    input: CalculateInput,
) -> AppResult<Calculation> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let gst = gst_context(&mut tx, &business, input.issue_date).await?;
    tx.commit().await?;
    let (lines, totals) =
        documents::calculate(&input.lines, input.prices_include_gst, gst).map_err(line_errors)?;
    Ok(Calculation {
        lines,
        totals,
        gst_registered: gst.registered,
    })
}

pub async fn list(
    state: &AppState,
    ctx: &BusinessContext,
    filter: ListFilter,
) -> AppResult<(Vec<InvoiceSummary>, i64)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (rows, total) =
        invoices::list(&mut tx, business_today(&business.timezone), &filter).await?;
    tx.commit().await?;
    Ok((
        rows.into_iter()
            .map(summary)
            .collect::<AppResult<Vec<_>>>()?,
        total,
    ))
}

async fn load(conn: &mut PgConnection, business: &BusinessRow, id: Uuid) -> AppResult<Invoice> {
    let row: InvoiceRow = invoices::find(conn, id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    let today = business_today(&business.timezone);
    let customer = customers::find(conn, today, row.customer_id)
        .await?
        .ok_or(AppError::NotFound("Customer"))?;
    let lines = docs::items(conn, DocKind::Invoice, id).await?;
    let payments = invoices::payments(conn, id).await?;
    let emails = invoices::email_history(conn, "invoice", id).await?;
    let mut sources: Vec<(&str, Uuid)> = vec![(SOURCE_INVOICE, id)];
    sources.extend(payments.iter().map(|p| (SOURCE_PAYMENT, p.id)));
    let journal = ledger_repo::history(conn, &sources).await?;
    Ok(Invoice {
        display_status: documents::invoice_display_status(
            &row.status,
            row.total,
            row.paid,
            row.written_off,
            row.due_date,
            today,
            row.sent_at.is_some(),
        ),
        customer: InvoiceCustomer {
            id: customer.id,
            name: customer.name,
            business_name: customer.business_name,
            contact_person: customer.contact_person,
            email: customer.email,
            billing_address: serde_json::from_value(customer.billing_address).unwrap_or_default(),
        },
        balance: row.total - row.paid,
        id: row.id,
        number: row.number,
        status: row.status,
        issue_date: row.issue_date,
        due_date: row.due_date,
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
        paid: row.paid,
        payments,
        emails,
        journal,
        quote_id: row.quote_id,
        issued_at: row.issued_at,
        sent_at: row.sent_at,
        cancelled_at: row.cancelled_at,
        cancellation_reason: row.cancellation_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

pub async fn get(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<Invoice> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let invoice = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(invoice)
}

// ─── Commands ───────────────────────────────────────────────────────────────

/// Validates input and returns storable fields plus calculated lines.
async fn prepare(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: InvoiceInput,
    defaults_for_new: bool,
) -> AppResult<(InvoiceFields, Vec<CalculatedLine>, Totals)> {
    let mut errors = FieldErrors::new();
    let today = business_today(&business.timezone);
    let customer = customers::find(conn, today, input.customer_id).await?;
    match &customer {
        None => errors.add("customer_id", "Choose a customer."),
        Some(c) if !c.is_active && defaults_for_new => errors.add(
            "customer_id",
            "This customer is inactive. Reactivate it first.",
        ),
        _ => {}
    }
    let settings = businesses::settings(conn, business.id).await?;
    let terms = customer
        .as_ref()
        .and_then(|c| c.payment_terms_days)
        .unwrap_or(settings.default_payment_terms_days);
    let due_date = input
        .due_date
        .unwrap_or(input.issue_date + Duration::days(i64::from(terms)));
    errors.check(
        due_date >= input.issue_date,
        "due_date",
        "The due date can’t be before the issue date.",
    );
    let reference = clean(&input.reference, 100, "reference", &mut errors);
    let notes_default = if defaults_for_new {
        settings.default_invoice_notes.clone()
    } else {
        String::new()
    };
    let footer_default = if defaults_for_new {
        settings.default_invoice_footer.clone()
    } else {
        String::new()
    };
    let notes = clean(
        &input.notes.unwrap_or(notes_default),
        4000,
        "notes",
        &mut errors,
    );
    let footer = clean(
        &input.footer.unwrap_or(footer_default),
        2000,
        "footer",
        &mut errors,
    );
    let gst = gst_context(conn, business, input.issue_date).await?;
    let calculated = documents::calculate(&input.lines, input.prices_include_gst, gst);
    if let Err(line_problems) = &calculated {
        for e in line_problems {
            errors.add(e.field.clone(), e.message);
        }
    }
    errors.into_result()?;
    let (lines, totals) = calculated.map_err(line_errors)?;
    Ok((
        InvoiceFields {
            customer_id: input.customer_id,
            issue_date: input.issue_date,
            due_date,
            reference,
            currency: business.currency.clone(),
            prices_include_gst: input.prices_include_gst,
            notes,
            footer,
            quote_id: None,
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
    changes: serde_json::Value,
) -> AppResult<()> {
    audit::record(
        conn,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action,
            entity_type: "invoice",
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

/// Creates a draft inside an existing transaction (also used when converting a quote).
pub async fn create_draft_in(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    input: InvoiceInput,
    quote_id: Option<Uuid>,
) -> AppResult<Uuid> {
    let (mut fields, lines, totals) = prepare(conn, business, input, true).await?;
    fields.quote_id = quote_id;
    let id = invoices::insert(conn, ctx.business_id, ctx.user_id(), &fields, totals).await?;
    docs::replace_items(conn, DocKind::Invoice, ctx.business_id, id, &lines).await?;
    record(
        conn,
        ctx,
        "invoice.created",
        id,
        "Draft invoice created".into(),
        serde_json::json!({ "total": totals.total }),
    )
    .await?;
    Ok(id)
}

pub async fn create(
    state: &AppState,
    ctx: &BusinessContext,
    input: InvoiceInput,
) -> AppResult<Invoice> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let id = create_draft_in(&mut tx, ctx, &business, input, None).await?;
    let invoice = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(invoice)
}

/// Updates a draft or an issued invoice. Issued invoices keep their number; their ledger entries are
/// reversed and re-posted, so the change is visible in the journal and audit log.
pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: InvoiceInput,
) -> AppResult<Invoice> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    if !invoices::lock(&mut tx, id).await? {
        return Err(AppError::NotFound("Invoice"));
    }
    let before = invoices::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    if before.status == "cancelled" {
        return Err(AppError::Conflict(
            "Cancelled invoices can’t be changed.".into(),
        ));
    }
    let (fields, lines, totals) = prepare(&mut tx, &business, input, false).await?;
    if totals.total < before.paid {
        return Err(AppError::validation(
            "lines",
            "The new total is less than the payments already recorded. Remove a payment first.",
        ));
    }
    invoices::update(&mut tx, id, &fields, totals).await?;
    docs::replace_items(&mut tx, DocKind::Invoice, ctx.business_id, id, &lines).await?;
    if before.status == "issued" {
        sync_postings(&mut tx, &ctx_for(ctx, &business), id).await?;
    }
    record(
        &mut tx,
        ctx,
        "invoice.updated",
        id,
        format!(
            "Invoice {} updated",
            before.number.as_deref().unwrap_or("draft")
        ),
        serde_json::json!({
            "total": { "from": before.total, "to": totals.total },
            "issue_date": { "from": before.issue_date, "to": fields.issue_date },
            "due_date": { "from": before.due_date, "to": fields.due_date },
            "customer_id": { "from": before.customer_id, "to": fields.customer_id },
        }),
    )
    .await?;
    let invoice = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(invoice)
}

/// Gives a draft its number and posts it to the ledger.
pub async fn issue(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<Invoice> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    if !invoices::lock(&mut tx, id).await? {
        return Err(AppError::NotFound("Invoice"));
    }
    let invoice = invoices::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    if invoice.status != "draft" {
        return Err(AppError::Conflict(
            "This invoice has already been issued.".into(),
        ));
    }
    let number = docs::next_number(&mut tx, DocKind::Invoice, ctx.business_id).await?;
    invoices::issue(&mut tx, id, &number).await?;
    sync_postings(&mut tx, &ctx_for(ctx, &business), id).await?;
    record(
        &mut tx,
        ctx,
        "invoice.issued",
        id,
        format!("Invoice {number} issued"),
        serde_json::json!({ "total": invoice.total }),
    )
    .await?;
    let invoice = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(invoice)
}

pub async fn mark_sent(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<Invoice> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let invoice = invoices::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    if invoice.status != "issued" {
        return Err(AppError::Conflict(
            "Only issued invoices can be marked as sent.".into(),
        ));
    }
    invoices::mark_sent(&mut tx, id).await?;
    record(
        &mut tx,
        ctx,
        "invoice.marked_sent",
        id,
        format!(
            "Invoice {} marked as sent",
            invoice.number.unwrap_or_default()
        ),
        serde_json::json!({}),
    )
    .await?;
    let invoice = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(invoice)
}

/// Cancels an issued invoice (its ledger entries are reversed). Payments must be removed first.
pub async fn cancel(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: ReasonInput,
) -> AppResult<Invoice> {
    let mut errors = FieldErrors::new();
    let reason = clean(&input.reason, 500, "reason", &mut errors);
    errors.check(
        !reason.is_empty(),
        "reason",
        "Say why the invoice is cancelled (it’s kept in the history).",
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    if !invoices::lock(&mut tx, id).await? {
        return Err(AppError::NotFound("Invoice"));
    }
    let invoice = invoices::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    match invoice.status.as_str() {
        "draft" => {
            return Err(AppError::Conflict(
                "Drafts aren’t cancelled; delete them instead.".into(),
            ));
        }
        "cancelled" => {
            return Err(AppError::Conflict(
                "This invoice is already cancelled.".into(),
            ));
        }
        _ => {}
    }
    if invoice.paid > 0 {
        return Err(AppError::Conflict(
            "This invoice has payments. Remove them before cancelling.".into(),
        ));
    }
    invoices::cancel(&mut tx, id, &reason).await?;
    sync_postings(&mut tx, &ctx_for(ctx, &business), id).await?;
    record(
        &mut tx,
        ctx,
        "invoice.cancelled",
        id,
        format!("Invoice {} cancelled", invoice.number.unwrap_or_default()),
        serde_json::json!({ "reason": reason }),
    )
    .await?;
    let invoice = load(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(invoice)
}

pub async fn delete_draft(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let invoice = invoices::find(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    if invoice.status != "draft" {
        return Err(AppError::Conflict(
            "Issued invoices can’t be deleted. Cancel them instead.".into(),
        ));
    }
    invoices::delete(&mut tx, id).await?;
    record(
        &mut tx,
        ctx,
        "invoice.deleted",
        id,
        "Draft invoice deleted".into(),
        serde_json::json!({ "total": invoice.total }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ─── Payments ───────────────────────────────────────────────────────────────

fn payment_fields(input: PaymentInput) -> AppResult<PaymentFields> {
    let mut errors = FieldErrors::new();
    errors.check(input.amount > 0, "amount", "Enter an amount above zero.");
    errors.check(
        input.withholding_tax >= 0 && input.withholding_tax < input.amount.max(1),
        "withholding_tax",
        "Tax withheld must be less than the amount paid.",
    );
    errors.check(
        input.method != PaymentMethod::WriteOff || input.withholding_tax == 0,
        "withholding_tax",
        "Nothing is withheld from an amount written off.",
    );
    let reference = clean(&input.reference, 100, "reference", &mut errors);
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    errors.into_result()?;
    Ok(PaymentFields {
        payment_date: input.payment_date,
        amount: input.amount,
        withholding_tax: input.withholding_tax,
        method: input.method.as_str().into(),
        reference,
        notes,
    })
}

async fn payment_audit(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    action: &str,
    invoice: &InvoiceRow,
    payment_id: Uuid,
    summary: String,
    changes: serde_json::Value,
) -> AppResult<()> {
    audit::record(
        conn,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action,
            entity_type: "invoice",
            entity_id: Some(invoice.id),
            summary,
            changes: serde_json::json!({ "payment_id": payment_id, "changes": changes }),
        },
    )
    .await
    .map_err(Into::into)
}

fn payment_receipt(payment_id: Uuid, fields: &PaymentFields) -> ReceiptInfo {
    ReceiptInfo {
        source: SourceRef::InvoicePayment(payment_id),
        amount: fields.amount,
        date: fields.payment_date,
    }
}

pub async fn add_payment(
    state: &AppState,
    ctx: &BusinessContext,
    invoice_id: Uuid,
    input: PaymentInput,
) -> AppResult<Invoice> {
    let fields = payment_fields(input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    if !invoices::lock(&mut tx, invoice_id).await? {
        return Err(AppError::NotFound("Invoice"));
    }
    let invoice = invoices::find(&mut tx, invoice_id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    if invoice.status != "issued" {
        return Err(AppError::Conflict(
            "Payments can only be recorded against issued invoices.".into(),
        ));
    }
    let balance = invoice.total - invoice.paid;
    if fields.amount > balance {
        return Err(AppError::validation(
            "amount",
            "This is more than the amount still owing.",
        ));
    }
    let payment_id =
        invoices::insert_payment(&mut tx, ctx.business_id, invoice_id, ctx.user_id(), &fields)
            .await?;
    sync_postings(&mut tx, &ctx_for(ctx, &business), invoice_id).await?;
    // A write-off isn't money received: nothing to set aside.
    if fields.method != "write_off" {
        hooks::on_receipt_recorded(&mut tx, ctx, payment_receipt(payment_id, &fields)).await?;
    }
    let number = invoice.number.clone().unwrap_or_default();
    payment_audit(
        &mut tx,
        ctx,
        "invoice.payment_recorded",
        &invoice,
        payment_id,
        format!("Payment of {} recorded for invoice {number}", crate::domain::money::Currency::from_code(&invoice.currency).map(|c| c.format(fields.amount)).unwrap_or_default()),
        serde_json::json!({ "amount": fields.amount, "payment_date": fields.payment_date, "method": fields.method }),
    )
    .await?;
    let invoice = load(&mut tx, &business, invoice_id).await?;
    tx.commit().await?;
    Ok(invoice)
}

async fn find_payment(
    conn: &mut PgConnection,
    invoice_id: Uuid,
    payment_id: Uuid,
) -> AppResult<PaymentRow> {
    invoices::payments(conn, invoice_id)
        .await?
        .into_iter()
        .find(|p| p.id == payment_id && p.voided_at.is_none())
        .ok_or(AppError::NotFound("Payment"))
}

pub async fn update_payment(
    state: &AppState,
    ctx: &BusinessContext,
    invoice_id: Uuid,
    payment_id: Uuid,
    input: PaymentInput,
) -> AppResult<Invoice> {
    let fields = payment_fields(input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    if !invoices::lock(&mut tx, invoice_id).await? {
        return Err(AppError::NotFound("Invoice"));
    }
    let invoice = invoices::find(&mut tx, invoice_id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    let before = find_payment(&mut tx, invoice_id, payment_id).await?;
    if fields.amount > invoice.total - invoice.paid + before.amount {
        return Err(AppError::validation(
            "amount",
            "This is more than the amount still owing.",
        ));
    }
    invoices::update_payment(&mut tx, payment_id, &fields).await?;
    sync_postings(&mut tx, &ctx_for(ctx, &business), invoice_id).await?;
    if fields.method == "write_off" {
        // No longer money received: its plan for setting money aside goes.
        hooks::on_receipt_removed(
            &mut tx,
            ctx,
            SourceRef::InvoicePayment(payment_id),
            "Written off as a bad debt",
        )
        .await?;
    } else {
        hooks::on_receipt_changed(&mut tx, ctx, payment_receipt(payment_id, &fields)).await?;
    }
    payment_audit(
        &mut tx,
        ctx,
        "invoice.payment_updated",
        &invoice,
        payment_id,
        format!(
            "Payment updated on invoice {}",
            invoice.number.clone().unwrap_or_default()
        ),
        serde_json::json!({
            "amount": { "from": before.amount, "to": fields.amount },
            "withholding_tax": { "from": before.withholding_tax, "to": fields.withholding_tax },
            "payment_date": { "from": before.payment_date, "to": fields.payment_date },
            "method": { "from": before.method, "to": fields.method },
            "reference": { "from": before.reference, "to": fields.reference },
        }),
    )
    .await?;
    let invoice = load(&mut tx, &business, invoice_id).await?;
    tx.commit().await?;
    Ok(invoice)
}

/// Removes a payment from the books. The row is kept (voided) and its ledger entries reversed.
pub async fn void_payment(
    state: &AppState,
    ctx: &BusinessContext,
    invoice_id: Uuid,
    payment_id: Uuid,
    input: ReasonInput,
) -> AppResult<Invoice> {
    let mut errors = FieldErrors::new();
    let reason = clean(&input.reason, 500, "reason", &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    if !invoices::lock(&mut tx, invoice_id).await? {
        return Err(AppError::NotFound("Invoice"));
    }
    let invoice = invoices::find(&mut tx, invoice_id)
        .await?
        .ok_or(AppError::NotFound("Invoice"))?;
    let payment = find_payment(&mut tx, invoice_id, payment_id).await?;
    invoices::void_payment(&mut tx, payment_id, &reason).await?;
    sync_postings(&mut tx, &ctx_for(ctx, &business), invoice_id).await?;
    hooks::on_receipt_removed(
        &mut tx,
        ctx,
        SourceRef::InvoicePayment(payment_id),
        "Payment removed",
    )
    .await?;
    payment_audit(
        &mut tx,
        ctx,
        "invoice.payment_removed",
        &invoice,
        payment_id,
        format!("Payment removed from invoice {}", invoice.number.clone().unwrap_or_default()),
        serde_json::json!({ "amount": payment.amount, "payment_date": payment.payment_date, "reason": reason }),
    )
    .await?;
    let invoice = load(&mut tx, &business, invoice_id).await?;
    tx.commit().await?;
    Ok(invoice)
}

/// Loads an invoice for PDF rendering or emailing.
pub async fn load_for_document(
    conn: &mut PgConnection,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<Invoice> {
    load(conn, business, id).await
}

pub async fn mark_sent_in(conn: &mut PgConnection, id: Uuid) -> AppResult<()> {
    invoices::mark_sent(conn, id).await.map_err(Into::into)
}
