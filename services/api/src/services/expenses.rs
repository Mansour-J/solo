//! Expenses, expense categories and receipt attachments.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::ledger::{self, ExpenseSplit};
use crate::domain::tax::{TaxTreatment, gst_in_inclusive};
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses::{self, BusinessRow};
use crate::repositories::customers;
use crate::repositories::expenses::{
    self as repo, AttachmentRow, CategoryRow, ExpenseFields, ExpenseFilter, ExpenseRow,
    NewAttachment,
};
use crate::repositories::ledger::{self as ledger_repo, JournalEntryView};
use crate::repositories::tax_rates::{self, TaxRateRow};
use crate::services::businesses::business_today;
use crate::services::invoices::{clean, gst_context, load_business};
use crate::services::ledger::{self as ledger_service, Context, Desired, SOURCE_EXPENSE};
use crate::services::storage::FileKind;
use crate::state::AppState;

pub const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_ATTACHMENTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    /// Entirely for the business.
    Business,
    /// Entirely private (recorded so the books show it, but not a business cost).
    Personal,
    /// Partly business: set `business_percentage`.
    Partial,
    /// A business cost to be recharged to a customer.
    Rechargeable,
}

impl Classification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Business => "business",
            Self::Personal => "personal",
            Self::Partial => "partial",
            Self::Rechargeable => "rechargeable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExpensePaymentMethod {
    BusinessAccount,
    /// Paid from the owner's own money (recorded as funds introduced).
    PersonalFunds,
    CreditCard,
    Cash,
    Other,
}

impl ExpensePaymentMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::BusinessAccount => "business_account",
            Self::PersonalFunds => "personal_funds",
            Self::CreditCard => "credit_card",
            Self::Cash => "cash",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExpenseInput {
    pub expense_date: NaiveDate,
    pub supplier: String,
    #[serde(default)]
    pub description: String,
    pub category_id: Uuid,
    /// Amount paid including GST (minor units).
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    /// GST shown on the receipt. Leave empty to calculate it from the gross amount.
    #[serde(default)]
    pub gst_amount: Option<i64>,
    pub payment_method: ExpensePaymentMethod,
    #[serde(default)]
    pub payment_reference: String,
    pub classification: Classification,
    /// Required for `partial`; ignored otherwise.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub business_percentage: Option<Decimal>,
    /// Share the user treats as tax-deductible. Defaults to the business share.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub deductible_percentage: Option<Decimal>,
    #[serde(default)]
    pub rechargeable_customer_id: Option<Uuid>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Expense {
    pub id: Uuid,
    pub expense_date: NaiveDate,
    pub supplier: String,
    pub description: String,
    pub category_id: Uuid,
    pub category_name: String,
    pub currency: String,
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    pub gst_amount: i64,
    /// The GST was entered from the receipt rather than worked out from the amount.
    pub gst_entered: bool,
    pub net_amount: i64,
    pub payment_method: String,
    pub payment_reference: String,
    pub classification: String,
    #[schema(value_type = String)]
    pub business_percentage: Decimal,
    #[schema(value_type = String)]
    pub deductible_percentage: Decimal,
    pub rechargeable_customer_id: Option<Uuid>,
    pub rechargeable_customer_name: Option<String>,
    pub notes: String,
    /// How the amount is booked: business cost, claimable GST and private share.
    pub split: ExpenseSplit,
    pub attachment_count: i64,
    pub voided: bool,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExpenseDetail {
    #[serde(flatten)]
    pub expense: Expense,
    pub attachments: Vec<AttachmentRow>,
    pub journal: Vec<JournalEntryView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExpensePage {
    pub items: Vec<Expense>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    /// Totals over every matching (non-voided) expense, not just this page.
    pub total_gross: i64,
    pub total_gst: i64,
}

fn gst_claimable(business: &BusinessRow, date: NaiveDate, treatment: TaxTreatment) -> bool {
    treatment == TaxTreatment::Standard
        && business.gst_registered
        && business.gst_registered_from.is_none_or(|from| date >= from)
}

/// The standard GST rate on `date`, from the dated rates.
fn gst_rate_on(rates: &[TaxRateRow], date: NaiveDate) -> Option<Decimal> {
    rates
        .iter()
        .find(|r| r.effective_from <= date && r.effective_to.is_none_or(|to| date < to))
        .map(|r| r.rate)
}

async fn gst_rates(conn: &mut PgConnection) -> AppResult<Vec<TaxRateRow>> {
    Ok(tax_rates::list(conn, "NZ")
        .await?
        .into_iter()
        .filter(|r| r.tax_type == "gst")
        .collect())
}

fn to_expense(row: ExpenseRow, business: &BusinessRow, rates: &[TaxRateRow]) -> Expense {
    let claimable = gst_claimable(business, row.expense_date, row.gst_treatment);
    let gst_entered = row.gst_treatment == TaxTreatment::Standard
        && gst_rate_on(rates, row.expense_date)
            .and_then(|rate| gst_in_inclusive(row.gross_amount, rate))
            .is_none_or(|calculated| calculated != row.gst_amount);
    Expense {
        gst_entered,
        split: ledger::split_expense(
            row.gross_amount,
            row.gst_amount,
            row.business_percentage,
            claimable,
        ),
        id: row.id,
        expense_date: row.expense_date,
        supplier: row.supplier,
        description: row.description,
        category_id: row.category_id,
        category_name: row.category_name,
        currency: row.currency,
        gross_amount: row.gross_amount,
        gst_treatment: row.gst_treatment,
        gst_amount: row.gst_amount,
        net_amount: row.net_amount,
        payment_method: row.payment_method,
        payment_reference: row.payment_reference,
        classification: row.classification,
        business_percentage: row.business_percentage,
        deductible_percentage: row.deductible_percentage,
        rechargeable_customer_id: row.rechargeable_customer_id,
        rechargeable_customer_name: row.rechargeable_customer_name,
        notes: row.notes,
        attachment_count: row.attachment_count,
        voided: row.voided_at.is_some(),
        void_reason: row.void_reason,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

async fn sync_postings(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<()> {
    let row = repo::find_expense(conn, id)
        .await?
        .ok_or(AppError::NotFound("Expense"))?;
    let mut desired = Vec::new();
    if row.voided_at.is_none() {
        let claimable = gst_claimable(business, row.expense_date, row.gst_treatment);
        let split = ledger::split_expense(
            row.gross_amount,
            row.gst_amount,
            row.business_percentage,
            claimable,
        );
        let spec = ledger::expense(
            row.expense_date,
            &row.supplier,
            row.account_id,
            row.gross_amount,
            split,
            row.payment_method == "personal_funds",
        );
        // A wholly private expense paid personally never touches the business: nothing to post.
        if !spec.lines.is_empty() {
            desired.push(Desired {
                source_type: SOURCE_EXPENSE,
                source_id: id,
                spec,
            });
        }
    }
    let context = Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    };
    ledger_service::reconcile(conn, &context, &[(SOURCE_EXPENSE, id)], desired).await
}

/// What the expense form needs to preview a booking: the same figures saving would produce.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExpenseCalculationInput {
    pub expense_date: NaiveDate,
    /// Amount paid including GST (minor units).
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    /// GST shown on the receipt. Leave empty to calculate it from the gross amount.
    #[serde(default)]
    pub gst_amount: Option<i64>,
    pub classification: Classification,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub business_percentage: Option<Decimal>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub deductible_percentage: Option<Decimal>,
}

/// How an expense would be booked, as saving it would work it out.
#[derive(Debug, Serialize, ToSchema)]
pub struct ExpenseCalculation {
    /// Whether the business is registered for GST on the expense date.
    pub registered: bool,
    /// The treatment that applies (no GST when not registered).
    pub gst_treatment: TaxTreatment,
    /// The standard GST rate on the date, e.g. "0.15".
    #[schema(value_type = String)]
    pub gst_rate: Decimal,
    /// GST included in the amount (minor units).
    pub gst_amount: i64,
    #[schema(value_type = String)]
    pub business_percentage: Decimal,
    /// The deductible share used (the default when none is entered).
    #[schema(value_type = String)]
    pub deductible_percentage: Decimal,
    pub split: ExpenseSplit,
}

/// GST, business share and deductible share for an expense, with field errors for anything invalid.
async fn amounts(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: &ExpenseCalculationInput,
    errors: &mut FieldErrors,
) -> AppResult<ExpenseCalculation> {
    // Without GST registration, GST on receipts isn't claimable: it is part of the cost.
    let registered = gst_context(conn, business, input.expense_date).await?;
    let treatment = registered.effective(input.gst_treatment);
    let gst_amount = match (treatment, input.gst_amount) {
        (TaxTreatment::Standard, Some(amount)) => amount,
        (TaxTreatment::Standard, None) => {
            gst_in_inclusive(input.gross_amount, registered.standard_rate).unwrap_or(0)
        }
        _ => 0,
    };
    errors.check(
        gst_amount >= 0 && gst_amount <= input.gross_amount,
        "gst_amount",
        "GST must be between 0 and the amount paid.",
    );
    let business_percentage = match input.classification {
        Classification::Business | Classification::Rechargeable => Decimal::ONE_HUNDRED,
        Classification::Personal => Decimal::ZERO,
        Classification::Partial => {
            let pct = input.business_percentage.unwrap_or(Decimal::ZERO);
            errors.check(
                pct > Decimal::ZERO && pct < Decimal::ONE_HUNDRED,
                "business_percentage",
                "Enter the business share (between 0 and 100%).",
            );
            pct
        }
    };
    let settings = businesses::settings(conn, business.id).await?;
    let deductible_default = if input.classification == Classification::Personal {
        Decimal::ZERO
    } else {
        business_percentage.min(settings.default_deductible_percentage)
    };
    let deductible_percentage = input.deductible_percentage.unwrap_or(deductible_default);
    errors.check(
        deductible_percentage >= Decimal::ZERO && deductible_percentage <= business_percentage,
        "deductible_percentage",
        "The deductible share can’t be more than the business share.",
    );
    let claimable = gst_claimable(business, input.expense_date, treatment);
    Ok(ExpenseCalculation {
        registered: registered.registered,
        gst_treatment: treatment,
        gst_rate: registered.standard_rate.normalize(),
        gst_amount,
        business_percentage: two_places(business_percentage),
        deductible_percentage: two_places(deductible_percentage),
        split: ledger::split_expense(
            input.gross_amount,
            gst_amount.clamp(0, input.gross_amount.max(0)),
            business_percentage.round_dp(2),
            claimable,
        ),
    })
}

/// A percentage with exactly two decimal places, as saved ("35.00").
fn two_places(value: Decimal) -> Decimal {
    let mut value = value.round_dp(2);
    value.rescale(2);
    value
}

/// Works out how an expense would be booked without saving it (for the expense form).
pub async fn calculate(
    state: &AppState,
    ctx: &BusinessContext,
    input: ExpenseCalculationInput,
) -> AppResult<ExpenseCalculation> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let mut errors = FieldErrors::new();
    errors.check(
        input.gross_amount > 0 && input.gross_amount <= 1_000_000_000_000,
        "gross_amount",
        "Enter the amount paid.",
    );
    let result = amounts(&mut tx, &business, &input, &mut errors).await?;
    tx.commit().await?;
    errors.into_result()?;
    Ok(result)
}

async fn validate(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: ExpenseInput,
) -> AppResult<ExpenseFields> {
    let mut errors = FieldErrors::new();
    let supplier = clean(&input.supplier, 200, "supplier", &mut errors);
    errors.check(!supplier.is_empty(), "supplier", "Enter who you paid.");
    let description = clean(&input.description, 1000, "description", &mut errors);
    let notes = clean(&input.notes, 2000, "notes", &mut errors);
    let payment_reference = clean(
        &input.payment_reference,
        100,
        "payment_reference",
        &mut errors,
    );
    errors.check(
        input.gross_amount > 0,
        "gross_amount",
        "Enter the amount paid.",
    );
    errors.check(
        input.gross_amount <= 1_000_000_000_000,
        "gross_amount",
        "This amount is too large.",
    );
    match repo::find_category(conn, input.category_id).await? {
        None => errors.add("category_id", "Choose a category."),
        Some(c) if c.archived => errors.add("category_id", "This category is archived."),
        _ => {}
    }
    let amounts = amounts(
        conn,
        business,
        &ExpenseCalculationInput {
            expense_date: input.expense_date,
            gross_amount: input.gross_amount,
            gst_treatment: input.gst_treatment,
            gst_amount: input.gst_amount,
            classification: input.classification,
            business_percentage: input.business_percentage,
            deductible_percentage: input.deductible_percentage,
        },
        &mut errors,
    )
    .await?;
    let (treatment, gst_amount, business_percentage, deductible_percentage) = (
        amounts.gst_treatment,
        amounts.gst_amount,
        amounts.business_percentage,
        amounts.deductible_percentage,
    );
    let rechargeable_customer_id = if input.classification == Classification::Rechargeable {
        input.rechargeable_customer_id
    } else {
        None
    };
    if input.classification == Classification::Rechargeable {
        match rechargeable_customer_id {
            None => errors.add(
                "rechargeable_customer_id",
                "Choose the customer to recharge.",
            ),
            Some(id) => {
                if customers::find(conn, business_today(&business.timezone), id)
                    .await?
                    .is_none()
                {
                    errors.add(
                        "rechargeable_customer_id",
                        "Choose the customer to recharge.",
                    );
                }
            }
        }
    }
    errors.into_result()?;
    Ok(ExpenseFields {
        expense_date: input.expense_date,
        supplier,
        description,
        category_id: input.category_id,
        currency: business.currency.clone(),
        gross_amount: input.gross_amount,
        gst_treatment: treatment,
        gst_amount,
        payment_method: input.payment_method.as_str().into(),
        payment_reference,
        classification: input.classification.as_str().into(),
        business_percentage: business_percentage.round_dp(2),
        deductible_percentage: deductible_percentage.round_dp(2),
        rechargeable_customer_id,
        notes,
    })
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
            entity_type: "expense",
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

fn snapshot(row: &ExpenseRow) -> serde_json::Value {
    serde_json::json!({
        "expense_date": row.expense_date, "supplier": row.supplier, "category": row.category_name, "gross_amount": row.gross_amount,
        "gst_amount": row.gst_amount, "gst_treatment": row.gst_treatment, "classification": row.classification,
        "business_percentage": row.business_percentage, "deductible_percentage": row.deductible_percentage,
        "payment_method": row.payment_method, "description": row.description,
    })
}

pub async fn list(
    state: &AppState,
    ctx: &BusinessContext,
    filter: ExpenseFilter,
    page: u32,
) -> AppResult<ExpensePage> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let per_page = filter.limit as u32;
    let (rows, total, total_gross, total_gst) = repo::list_expenses(&mut tx, &filter).await?;
    let rates = gst_rates(&mut tx).await?;
    tx.commit().await?;
    Ok(ExpensePage {
        items: rows
            .into_iter()
            .map(|r| to_expense(r, &business, &rates))
            .collect(),
        page,
        per_page,
        total,
        total_gross,
        total_gst,
    })
}

async fn detail(
    conn: &mut PgConnection,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<ExpenseDetail> {
    let row = repo::find_expense(conn, id)
        .await?
        .ok_or(AppError::NotFound("Expense"))?;
    let attachments = repo::attachments(conn, id).await?;
    let journal = ledger_repo::history(conn, &[(SOURCE_EXPENSE, id)]).await?;
    let rates = gst_rates(conn).await?;
    Ok(ExpenseDetail {
        expense: to_expense(row, business, &rates),
        attachments,
        journal,
    })
}

pub async fn get(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<ExpenseDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let result = detail(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn create(
    state: &AppState,
    ctx: &BusinessContext,
    input: ExpenseInput,
) -> AppResult<ExpenseDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let fields = validate(&mut tx, &business, input).await?;
    let id = repo::insert_expense(&mut tx, ctx.business_id, ctx.user_id(), &fields).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "expense.created",
        id,
        format!("Expense from {} recorded", fields.supplier),
        serde_json::json!({ "gross_amount": fields.gross_amount }),
    )
    .await?;
    let result = detail(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: ExpenseInput,
) -> AppResult<ExpenseDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_expense(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Expense"))?;
    if before.voided_at.is_some() {
        return Err(AppError::Conflict(
            "Deleted expenses can’t be changed.".into(),
        ));
    }
    let fields = validate(&mut tx, &business, input).await?;
    repo::update_expense(&mut tx, id, &fields).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    let after = repo::find_expense(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Expense"))?;
    let changes = crate::services::businesses::diff(&snapshot(&before), &snapshot(&after));
    record(
        &mut tx,
        ctx,
        "expense.updated",
        id,
        format!("Expense from {} updated", after.supplier),
        changes,
    )
    .await?;
    let result = detail(&mut tx, &business, id).await?;
    tx.commit().await?;
    Ok(result)
}

/// Deletes an expense from the books: it is kept (voided) and its ledger entry reversed.
pub async fn void(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    reason: String,
) -> AppResult<()> {
    let mut errors = FieldErrors::new();
    let reason = clean(&reason, 500, "reason", &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let row = repo::find_expense(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Expense"))?;
    if row.voided_at.is_some() {
        return Err(AppError::Conflict(
            "This expense is already deleted.".into(),
        ));
    }
    repo::void_expense(&mut tx, id, &reason).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "expense.deleted",
        id,
        format!("Expense from {} deleted", row.supplier),
        serde_json::json!({ "reason": reason, "before": snapshot(&row) }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ─── Attachments ────────────────────────────────────────────────────────────

pub async fn add_attachment(
    state: &AppState,
    ctx: &BusinessContext,
    expense_id: Uuid,
    filename: String,
    bytes: Vec<u8>,
) -> AppResult<ExpenseDetail> {
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err(AppError::PayloadTooLarge(
            "Receipts can be up to 10 MB.".into(),
        ));
    }
    let kind = FileKind::sniff(&bytes)
        .ok_or_else(|| AppError::validation("file", "Upload a PDF, JPEG, PNG or WebP file."))?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let expense = repo::find_expense(&mut tx, expense_id)
        .await?
        .ok_or(AppError::NotFound("Expense"))?;
    if expense.voided_at.is_some() {
        return Err(AppError::Conflict(
            "Deleted expenses can’t get new receipts.".into(),
        ));
    }
    if repo::attachments(&mut tx, expense_id).await?.len() >= MAX_ATTACHMENTS {
        return Err(AppError::validation(
            "file",
            "An expense can have up to 10 attachments.",
        ));
    }
    let safe_name: String = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("receipt")
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect();
    let safe_name = if safe_name.trim().is_empty() {
        format!("receipt.{}", kind.extension())
    } else {
        safe_name
    };
    let key = format!(
        "businesses/{}/expenses/{}/{}.{}",
        ctx.business_id,
        expense_id,
        Uuid::now_v7(),
        kind.extension()
    );
    let size = i32::try_from(bytes.len()).map_err(AppError::internal)?;
    state.storage.put(&key, bytes).await?;
    let attachment_id = repo::insert_attachment(
        &mut tx,
        NewAttachment {
            business_id: ctx.business_id,
            expense_id,
            object_key: &key,
            filename: &safe_name,
            content_type: kind.content_type(),
            size_bytes: size,
            uploaded_by: ctx.user_id(),
        },
    )
    .await?;
    record(
        &mut tx,
        ctx,
        "expense.receipt_added",
        expense_id,
        format!("Receipt “{safe_name}” attached"),
        serde_json::json!({ "attachment_id": attachment_id }),
    )
    .await?;
    let result = detail(&mut tx, &business, expense_id).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn attachment(
    state: &AppState,
    ctx: &BusinessContext,
    expense_id: Uuid,
    attachment_id: Uuid,
) -> AppResult<(AttachmentRow, Vec<u8>)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::attachments(&mut tx, expense_id)
        .await?
        .into_iter()
        .find(|a| a.id == attachment_id)
        .ok_or(AppError::NotFound("Attachment"))?;
    tx.commit().await?;
    let bytes = state
        .storage
        .get(&row.object_key)
        .await?
        .ok_or(AppError::NotFound("Attachment"))?;
    Ok((row, bytes))
}

pub async fn delete_attachment(
    state: &AppState,
    ctx: &BusinessContext,
    expense_id: Uuid,
    attachment_id: Uuid,
) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::attachments(&mut tx, expense_id)
        .await?
        .into_iter()
        .find(|a| a.id == attachment_id)
        .ok_or(AppError::NotFound("Attachment"))?;
    repo::delete_attachment(&mut tx, attachment_id).await?;
    record(
        &mut tx,
        ctx,
        "expense.receipt_removed",
        expense_id,
        format!("Receipt “{}” removed", row.filename),
        serde_json::json!({ "attachment_id": attachment_id }),
    )
    .await?;
    tx.commit().await?;
    if let Err(error) = state.storage.delete(&row.object_key).await {
        tracing::warn!(%error, "could not delete a removed receipt from storage");
    }
    Ok(())
}

// ─── Categories ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CategoryInput {
    pub name: String,
    /// Not deductible for tax (e.g. penalties): reports show it as an adjustment.
    #[serde(default)]
    pub non_deductible: bool,
    /// A direct cost of what's sold (materials, stock): shown as cost of sales, before gross profit.
    #[serde(default)]
    pub cost_of_sales: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CategoryArchiveInput {
    pub archived: bool,
}

pub async fn categories(state: &AppState, ctx: &BusinessContext) -> AppResult<Vec<CategoryRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::categories(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

fn category_name(input: &CategoryInput) -> AppResult<String> {
    let name = input.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::validation(
            "name",
            "Enter a name of up to 80 characters.",
        ));
    }
    if input.cost_of_sales && input.non_deductible {
        return Err(AppError::validation(
            "cost_of_sales",
            "A cost of sales is deductible: untick Not deductible for tax first.",
        ));
    }
    Ok(name)
}

pub async fn create_category(
    state: &AppState,
    ctx: &BusinessContext,
    input: CategoryInput,
) -> AppResult<CategoryRow> {
    let name = category_name(&input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    if repo::category_name_taken(&mut tx, &name, None).await? {
        return Err(AppError::validation(
            "name",
            "A category with this name already exists.",
        ));
    }
    let id = repo::insert_category(&mut tx, ctx.business_id, &name)
        .await?
        .ok_or_else(|| {
            AppError::validation(
                "name",
                "This business has the most categories it can have. Rename an existing one instead.",
            )
        })?;
    let row = repo::find_category(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Category"))?;
    repo::set_account_non_deductible(&mut tx, row.account_id, input.non_deductible).await?;
    repo::set_account_cost_of_sales(&mut tx, row.account_id, input.cost_of_sales).await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "expense_category.created",
            entity_type: "expense_category",
            entity_id: Some(id),
            summary: format!("Expense category “{name}” added"),
            changes: serde_json::json!({ "non_deductible": input.non_deductible, "cost_of_sales": input.cost_of_sales }),
        },
    )
    .await?;
    let row = repo::find_category(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Category"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_category(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: CategoryInput,
) -> AppResult<CategoryRow> {
    let name = category_name(&input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = repo::find_category(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Category"))?;
    if repo::category_name_taken(&mut tx, &name, Some(id)).await? {
        return Err(AppError::validation(
            "name",
            "A category with this name already exists.",
        ));
    }
    repo::rename_category(&mut tx, id, before.account_id, &name).await?;
    repo::set_account_non_deductible(&mut tx, before.account_id, input.non_deductible).await?;
    repo::set_account_cost_of_sales(&mut tx, before.account_id, input.cost_of_sales).await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "expense_category.updated",
            entity_type: "expense_category",
            entity_id: Some(id),
            summary: if before.name == name {
                format!("Expense category “{name}” updated")
            } else {
                format!("Expense category “{}” renamed to “{name}”", before.name)
            },
            changes: serde_json::json!({
                "name": { "from": before.name, "to": name },
                "non_deductible": { "from": before.non_deductible, "to": input.non_deductible },
                "cost_of_sales": { "from": before.cost_of_sales, "to": input.cost_of_sales },
            }),
        },
    )
    .await?;
    let row = repo::find_category(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Category"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn archive_category(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<CategoryRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let category = repo::find_category(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Category"))?;
    if !archived && repo::category_name_taken(&mut tx, &category.name, Some(id)).await? {
        return Err(AppError::Conflict(
            "Another active category has this name. Rename one of them first.".into(),
        ));
    }
    repo::set_category_archived(&mut tx, id, archived).await?;
    let row = repo::find_category(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Category"))?;
    tx.commit().await?;
    Ok(row)
}
