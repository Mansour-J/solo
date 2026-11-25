//! Income received without an invoice (cash sales, interest, grants…).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::ledger;
use crate::domain::tax::{TaxTreatment, gst_in_inclusive};
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::allocations::SourceRef;
use crate::repositories::audit;
use crate::repositories::businesses::BusinessRow;
use crate::repositories::customers;
use crate::repositories::expenses::{self as repo, IncomeFields, IncomeRow};
use crate::services::allocations::ReceiptInfo;
use crate::services::businesses::business_today;
use crate::services::hooks;
use crate::services::invoices::{PaymentMethod, clean, gst_context, load_business};
use crate::services::ledger::{self as ledger_service, Context, Desired, SOURCE_INCOME};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IncomeType {
    /// Sales of goods or services (counted as revenue).
    Sales,
    /// Interest, grants and other income.
    Other,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct IncomeInput {
    pub income_date: NaiveDate,
    pub payer: String,
    #[serde(default)]
    pub description: String,
    pub income_type: IncomeType,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    /// Amount received including GST (minor units).
    pub gross_amount: i64,
    pub gst_treatment: TaxTreatment,
    #[serde(default)]
    pub gst_amount: Option<i64>,
    pub payment_method: PaymentMethod,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IncomePage {
    pub items: Vec<IncomeRow>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_gross: i64,
}

async fn sync_postings(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<()> {
    let row = repo::find_income(conn, id)
        .await?
        .ok_or(AppError::NotFound("Income"))?;
    let mut desired = Vec::new();
    if row.voided_at.is_none() {
        desired.push(Desired {
            source_type: SOURCE_INCOME,
            source_id: id,
            spec: ledger::income(
                row.income_date,
                &row.payer,
                row.income_type == "other",
                row.customer_id,
                row.net_amount,
                row.gst_amount,
            ),
        });
    }
    let context = Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    };
    ledger_service::reconcile(conn, &context, &[(SOURCE_INCOME, id)], desired).await
}

async fn validate(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: IncomeInput,
) -> AppResult<IncomeFields> {
    let mut errors = FieldErrors::new();
    let payer = clean(&input.payer, 200, "payer", &mut errors);
    errors.check(!payer.is_empty(), "payer", "Enter who paid you.");
    let description = clean(&input.description, 1000, "description", &mut errors);
    let reference = clean(&input.reference, 100, "reference", &mut errors);
    let notes = clean(&input.notes, 2000, "notes", &mut errors);
    errors.check(
        input.gross_amount > 0 && input.gross_amount <= 1_000_000_000_000,
        "gross_amount",
        "Enter the amount received.",
    );
    if let Some(id) = input.customer_id
        && customers::find(conn, business_today(&business.timezone), id)
            .await?
            .is_none()
    {
        errors.add("customer_id", "Choose a customer.");
    }
    let gst = gst_context(conn, business, input.income_date).await?;
    let treatment = gst.effective(input.gst_treatment);
    let gst_amount = match (treatment, input.gst_amount) {
        (TaxTreatment::Standard, Some(amount)) => amount,
        (TaxTreatment::Standard, None) => {
            gst_in_inclusive(input.gross_amount, gst.standard_rate).unwrap_or(0)
        }
        _ => 0,
    };
    errors.check(
        gst_amount >= 0 && gst_amount <= input.gross_amount,
        "gst_amount",
        "GST must be between 0 and the amount received.",
    );
    errors.into_result()?;
    Ok(IncomeFields {
        income_date: input.income_date,
        payer,
        description,
        income_type: match input.income_type {
            IncomeType::Sales => "sales".into(),
            IncomeType::Other => "other".into(),
        },
        customer_id: input.customer_id,
        currency: business.currency.clone(),
        gross_amount: input.gross_amount,
        gst_treatment: treatment,
        gst_amount,
        payment_method: input.payment_method.as_str().into(),
        reference,
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
            entity_type: "income",
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

pub async fn list(
    state: &AppState,
    ctx: &BusinessContext,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    search: Option<String>,
    page: u32,
    per_page: i64,
) -> AppResult<IncomePage> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let offset = i64::from(page.max(1) - 1) * per_page;
    let (items, total, total_gross) =
        repo::list_income(&mut tx, from, to, search, per_page, offset).await?;
    tx.commit().await?;
    Ok(IncomePage {
        items,
        page: page.max(1),
        per_page: per_page as u32,
        total,
        total_gross,
    })
}

fn receipt(id: Uuid, fields: &IncomeFields) -> ReceiptInfo {
    ReceiptInfo {
        source: SourceRef::Income(id),
        amount: fields.gross_amount,
        date: fields.income_date,
    }
}

pub async fn get(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<IncomeRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::find_income(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Income"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn create(
    state: &AppState,
    ctx: &BusinessContext,
    input: IncomeInput,
) -> AppResult<IncomeRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let fields = validate(&mut tx, &business, input).await?;
    let id = repo::insert_income(&mut tx, ctx.business_id, ctx.user_id(), &fields).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    hooks::on_receipt_recorded(&mut tx, ctx, receipt(id, &fields)).await?;
    record(
        &mut tx,
        ctx,
        "income.created",
        id,
        format!("Income from {} recorded", fields.payer),
        serde_json::json!({ "gross_amount": fields.gross_amount }),
    )
    .await?;
    let row = repo::find_income(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Income"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: IncomeInput,
) -> AppResult<IncomeRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_income(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Income"))?;
    if before.voided_at.is_some() {
        return Err(AppError::Conflict(
            "Deleted income can’t be changed.".into(),
        ));
    }
    let fields = validate(&mut tx, &business, input).await?;
    repo::update_income(&mut tx, id, &fields).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    hooks::on_receipt_changed(&mut tx, ctx, receipt(id, &fields)).await?;
    record(
        &mut tx,
        ctx,
        "income.updated",
        id,
        format!("Income from {} updated", fields.payer),
        serde_json::json!({ "gross_amount": { "from": before.gross_amount, "to": fields.gross_amount }, "income_date": { "from": before.income_date, "to": fields.income_date } }),
    )
    .await?;
    let row = repo::find_income(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Income"))?;
    tx.commit().await?;
    Ok(row)
}

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
    let row = repo::find_income(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Income"))?;
    if row.voided_at.is_some() {
        return Err(AppError::Conflict("This income is already deleted.".into()));
    }
    repo::void_income(&mut tx, id, &reason).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    hooks::on_receipt_removed(&mut tx, ctx, SourceRef::Income(id), "Income deleted").await?;
    record(
        &mut tx,
        ctx,
        "income.deleted",
        id,
        format!("Income from {} deleted", row.payer),
        serde_json::json!({ "reason": reason, "gross_amount": row.gross_amount }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
