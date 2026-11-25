//! Records for annual accounts (ADR 0013): money moved in and out by the owner or to IRD, fixed
//! assets with yearly depreciation, and opening balances. Each posts to the journal through
//! `services::ledger::reconcile`, so reports (balance sheet, equity, depreciation schedule) read it.

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::financial_year;
use crate::domain::depreciation::{self as dep, Method};
use crate::domain::ledger::{self, MovementKind, Opening};
use crate::domain::money::Currency;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::annual::{
    self as repo, AssetFields, AssetRow, DepreciationFields, DepreciationRow, MovementFields,
    MovementRow, OpeningFields, OpeningRow,
};
use crate::repositories::audit;
use crate::repositories::businesses::BusinessRow;
use crate::services::invoices::{clean, load_business};
use crate::services::ledger::{
    self as ledger_service, Context, Desired, SOURCE_CASH_MOVEMENT, SOURCE_DEPRECIATION,
    SOURCE_FIXED_ASSET, SOURCE_OPENING,
};
pub use crate::services::mileage::ArchiveInput;
use crate::state::AppState;

/// Largest amount accepted (a billion in major units).
const MAX_AMOUNT: i64 = 100_000_000_000;

async fn record(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    action: &str,
    entity_type: &str,
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
            entity_type,
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

fn context(ctx: &BusinessContext, business: &BusinessRow) -> Context {
    Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    }
}

// ─── Money in and out ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MovementInput {
    pub movement_date: NaiveDate,
    pub kind: MovementKind,
    /// Minor units.
    pub amount: i64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub reference: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MovementPage {
    pub items: Vec<MovementRow>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
}

fn movement_fields(input: &MovementInput, business: &BusinessRow) -> AppResult<MovementFields> {
    let mut errors = FieldErrors::new();
    errors.check(
        input.amount > 0 && input.amount <= MAX_AMOUNT,
        "amount",
        "Enter an amount above zero.",
    );
    let description = clean(&input.description, 300, "description", &mut errors);
    let reference = clean(&input.reference, 100, "reference", &mut errors);
    errors.into_result()?;
    Ok(MovementFields {
        movement_date: input.movement_date,
        kind: input.kind.as_str().into(),
        amount: input.amount,
        currency: business.currency.clone(),
        description,
        reference,
    })
}

async fn sync_movement(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<()> {
    let row = repo::find_movement(conn, id)
        .await?
        .ok_or(AppError::NotFound("Movement"))?;
    let mut desired = Vec::new();
    if row.voided_at.is_none()
        && let Some(kind) = MovementKind::parse(&row.kind)
    {
        desired.push(Desired {
            source_type: SOURCE_CASH_MOVEMENT,
            source_id: id,
            spec: ledger::cash_movement(row.movement_date, kind, row.amount, &row.description),
        });
    }
    ledger_service::reconcile(
        conn,
        &context(ctx, business),
        &[(SOURCE_CASH_MOVEMENT, id)],
        desired,
    )
    .await
}

pub async fn list_movements(
    state: &AppState,
    ctx: &BusinessContext,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    kind: Option<MovementKind>,
    page: u32,
    per_page: i64,
) -> AppResult<MovementPage> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let offset = i64::from(page.max(1) - 1) * per_page;
    let (items, total) = repo::list_movements(
        &mut tx,
        from,
        to,
        kind.map(MovementKind::as_str),
        per_page,
        offset,
    )
    .await?;
    tx.commit().await?;
    Ok(MovementPage {
        items,
        page: page.max(1),
        per_page: per_page as u32,
        total,
    })
}

pub async fn create_movement(
    state: &AppState,
    ctx: &BusinessContext,
    input: MovementInput,
) -> AppResult<MovementRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let fields = movement_fields(&input, &business)?;
    let id = repo::insert_movement(&mut tx, ctx.business_id, ctx.user_id(), &fields).await?;
    sync_movement(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "cash_movement.created",
        "cash_movement",
        id,
        format!(
            "{} of {} recorded",
            input.kind.label(),
            Currency::from_code(&business.currency)
                .map(|c| c.format(fields.amount))
                .unwrap_or_else(|| fields.amount.to_string())
        ),
        serde_json::json!({ "kind": fields.kind, "amount": fields.amount, "movement_date": fields.movement_date }),
    )
    .await?;
    let row = repo::find_movement(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Movement"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_movement(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: MovementInput,
) -> AppResult<MovementRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_movement(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Movement"))?;
    if before.voided_at.is_some() {
        return Err(AppError::Conflict(
            "A deleted record can’t be changed.".into(),
        ));
    }
    let fields = movement_fields(&input, &business)?;
    repo::update_movement(&mut tx, id, &fields).await?;
    sync_movement(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "cash_movement.updated",
        "cash_movement",
        id,
        format!("{} updated", input.kind.label()),
        serde_json::json!({
            "kind": { "from": before.kind, "to": fields.kind },
            "amount": { "from": before.amount, "to": fields.amount },
            "movement_date": { "from": before.movement_date, "to": fields.movement_date },
        }),
    )
    .await?;
    let row = repo::find_movement(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Movement"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn void_movement(
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
    let row = repo::find_movement(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Movement"))?;
    if row.voided_at.is_some() {
        return Err(AppError::Conflict("This record is already deleted.".into()));
    }
    repo::void_movement(&mut tx, id, &reason).await?;
    sync_movement(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "cash_movement.deleted",
        "cash_movement",
        id,
        "Money movement deleted".into(),
        serde_json::json!({ "reason": reason, "amount": row.amount, "kind": row.kind }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ─── Opening balances ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct OpeningInput {
    /// The last day before you started recording in the app, e.g. the end of a financial year.
    pub as_at: NaiveDate,
    /// Minor units; negative for an overdraft.
    #[serde(default)]
    pub bank: i64,
    /// Owed to IRD; negative for a refund due.
    #[serde(default)]
    pub gst_payable: i64,
    /// As on a balance sheet: zero or negative.
    #[serde(default)]
    pub drawings: i64,
    #[serde(default)]
    pub funds_introduced: i64,
    #[serde(default)]
    pub personal_accounts: i64,
    /// As on a balance sheet: zero or negative.
    #[serde(default)]
    pub withholding_tax: i64,
    /// As on a balance sheet: zero or negative.
    #[serde(default)]
    pub income_tax: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OpeningView {
    pub balances: Option<OpeningRow>,
    /// Book value of fixed assets brought forward (from the assets' opening values).
    pub fixed_assets: i64,
    /// The figure that makes the balances balance.
    pub retained_earnings: Option<i64>,
}

fn opening_of(row: &OpeningRow, fixed_assets: i64) -> Opening {
    Opening {
        bank: row.bank,
        fixed_assets,
        gst_payable: row.gst_payable,
        drawings: row.drawings,
        funds_introduced: row.funds_introduced,
        personal_accounts: row.personal_accounts,
        withholding_tax: row.withholding_tax,
        income_tax: row.income_tax,
    }
}

/// Re-posts the opening balances entry (after the balances or a brought-forward asset changed).
async fn sync_opening(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
) -> AppResult<()> {
    let row = repo::find_opening(conn).await?;
    let fixed_assets = repo::opening_asset_total(conn).await?;
    let (sources, desired) = match &row {
        Some(row) => {
            let spec = ledger::opening_balances(row.as_at, &opening_of(row, fixed_assets));
            let desired = if spec.is_valid() {
                vec![Desired {
                    source_type: SOURCE_OPENING,
                    source_id: row.id,
                    spec,
                }]
            } else {
                Vec::new()
            };
            (vec![(SOURCE_OPENING, row.id)], desired)
        }
        None => return Ok(()),
    };
    ledger_service::reconcile(conn, &context(ctx, business), &sources, desired).await
}

pub async fn opening(state: &AppState, ctx: &BusinessContext) -> AppResult<OpeningView> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let balances = repo::find_opening(&mut tx).await?;
    let fixed_assets = repo::opening_asset_total(&mut tx).await?;
    tx.commit().await?;
    Ok(OpeningView {
        retained_earnings: balances
            .as_ref()
            .map(|b| opening_of(b, fixed_assets).retained_earnings()),
        balances,
        fixed_assets,
    })
}

pub async fn save_opening(
    state: &AppState,
    ctx: &BusinessContext,
    input: OpeningInput,
) -> AppResult<OpeningView> {
    let mut errors = FieldErrors::new();
    let within = |v: i64| v.abs() <= MAX_AMOUNT;
    for (field, value) in [
        ("bank", input.bank),
        ("gst_payable", input.gst_payable),
        ("personal_accounts", input.personal_accounts),
        ("funds_introduced", input.funds_introduced),
    ] {
        errors.check(within(value), field, "Enter a smaller amount.");
    }
    errors.check(
        input.funds_introduced >= 0,
        "funds_introduced",
        "Funds introduced can’t be negative.",
    );
    for (field, value) in [
        ("drawings", input.drawings),
        ("withholding_tax", input.withholding_tax),
        ("income_tax", input.income_tax),
    ] {
        errors.check(
            value <= 0 && within(value),
            field,
            "Enter this as it appears on the balance sheet: zero or negative.",
        );
    }
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_opening(&mut tx).await?;
    let id = repo::save_opening(
        &mut tx,
        ctx.business_id,
        &OpeningFields {
            as_at: input.as_at,
            bank: input.bank,
            gst_payable: input.gst_payable,
            drawings: input.drawings,
            funds_introduced: input.funds_introduced,
            personal_accounts: input.personal_accounts,
            withholding_tax: input.withholding_tax,
            income_tax: input.income_tax,
        },
    )
    .await?;
    sync_opening(&mut tx, ctx, &business).await?;
    record(
        &mut tx,
        ctx,
        "opening_balances.saved",
        "opening_balances",
        id,
        format!("Opening balances as at {} saved", input.as_at.format("%d/%m/%Y")),
        serde_json::json!({
            "as_at": { "from": before.as_ref().map(|b| b.as_at), "to": input.as_at },
            "bank": { "from": before.as_ref().map(|b| b.bank), "to": input.bank },
            "gst_payable": { "from": before.as_ref().map(|b| b.gst_payable), "to": input.gst_payable },
        }),
    )
    .await?;
    tx.commit().await?;
    opening(state, ctx).await
}

pub async fn delete_opening(state: &AppState, ctx: &BusinessContext) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let Some(id) = repo::delete_opening(&mut tx).await? else {
        return Err(AppError::NotFound("Opening balances"));
    };
    ledger_service::reconcile(
        &mut tx,
        &context(ctx, &business),
        &[(SOURCE_OPENING, id)],
        Vec::new(),
    )
    .await?;
    record(
        &mut tx,
        ctx,
        "opening_balances.deleted",
        "opening_balances",
        id,
        "Opening balances removed".into(),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

// ─── Fixed assets ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AssetInput {
    pub name: String,
    /// e.g. "Computer equipment".
    pub asset_group: String,
    /// Minor units, excluding GST.
    pub cost: i64,
    /// GST paid on the purchase (claimed when it posts). Minor units.
    #[serde(default)]
    pub purchase_gst: i64,
    pub acquired_on: NaiveDate,
    pub method: Method,
    /// Percent a year, e.g. "50".
    #[schema(value_type = String)]
    pub rate: Decimal,
    /// Percent of private use, e.g. "20".
    #[serde(default)]
    #[schema(value_type = String)]
    pub private_use: Decimal,
    /// For an asset you had before your opening balances: its book value at that date.
    #[serde(default)]
    pub opening_value: Option<i64>,
    #[serde(default)]
    pub notes: String,
}

async fn asset_fields(conn: &mut PgConnection, input: &AssetInput) -> AppResult<AssetFields> {
    let mut errors = FieldErrors::new();
    let name = clean(&input.name, 120, "name", &mut errors);
    errors.check(!name.is_empty(), "name", "Enter a name.");
    let asset_group = clean(&input.asset_group, 80, "asset_group", &mut errors);
    errors.check(
        !asset_group.is_empty(),
        "asset_group",
        "Enter a group, e.g. Computer equipment.",
    );
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    errors.check(
        input.cost > 0 && input.cost <= MAX_AMOUNT,
        "cost",
        "Enter the cost excluding GST.",
    );
    errors.check(
        input.purchase_gst >= 0 && input.purchase_gst <= input.cost.max(0),
        "purchase_gst",
        "GST must be between zero and the cost.",
    );
    let rate = input.rate.round_dp(2);
    errors.check(
        rate > Decimal::ZERO && rate <= Decimal::ONE_HUNDRED,
        "rate",
        "Enter a rate above 0 and up to 100%.",
    );
    let private_use = input.private_use.round_dp(2);
    errors.check(
        private_use >= Decimal::ZERO && private_use < Decimal::ONE_HUNDRED,
        "private_use",
        "Enter private use from 0 to under 100%.",
    );
    if input.method == Method::Full {
        errors.check(
            input.cost <= crate::domain::depreciation::LOW_VALUE_LIMIT,
            "method",
            "Only an asset costing $1,000 or less (excluding GST) can be deducted in full.",
        );
        errors.check(
            input.opening_value.is_none(),
            "method",
            "Deduct in full only in the year it was bought, not for an asset brought forward.",
        );
    }
    if let Some(value) = input.opening_value {
        errors.check(
            value >= 0 && value <= input.cost,
            "opening_value",
            "The value brought forward can’t be more than the cost.",
        );
    }
    errors.into_result()?;
    if let Some(opening) = repo::find_opening(conn).await? {
        match input.opening_value {
            Some(_) if input.acquired_on > opening.as_at => {
                return Err(AppError::validation(
                    "opening_value",
                    "Only assets you had at the opening balance date have a value brought forward.",
                ));
            }
            None if input.acquired_on <= opening.as_at => {
                return Err(AppError::validation(
                    "opening_value",
                    "This asset was bought before your opening balances: enter its value at that date.",
                ));
            }
            _ => {}
        }
    }
    Ok(AssetFields {
        name,
        asset_group,
        cost: input.cost,
        purchase_gst: if input.opening_value.is_some() {
            0
        } else {
            input.purchase_gst
        },
        acquired_on: input.acquired_on,
        method: input.method.as_str().into(),
        rate,
        private_use,
        opening_value: input.opening_value,
        notes,
    })
}

/// Posts an asset's purchase (unless it was brought forward) and keeps the opening entry in step.
async fn sync_asset(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<()> {
    let asset = repo::find_asset(conn, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    let mut desired = Vec::new();
    if asset.opening_value.is_none() {
        desired.push(Desired {
            source_type: SOURCE_FIXED_ASSET,
            source_id: id,
            spec: ledger::asset_purchase(
                asset.acquired_on,
                &asset.name,
                asset.cost,
                asset.purchase_gst,
            ),
        });
    }
    ledger_service::reconcile(
        conn,
        &context(ctx, business),
        &[(SOURCE_FIXED_ASSET, id)],
        desired,
    )
    .await?;
    sync_opening(conn, ctx, business).await
}

pub async fn list_assets(state: &AppState, ctx: &BusinessContext) -> AppResult<Vec<AssetRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list_assets(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn create_asset(
    state: &AppState,
    ctx: &BusinessContext,
    input: AssetInput,
) -> AppResult<AssetRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let fields = asset_fields(&mut tx, &input).await?;
    let id = repo::insert_asset(&mut tx, ctx.business_id, &fields).await?;
    sync_asset(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "fixed_asset.created",
        "fixed_asset",
        id,
        format!("Asset “{}” added", fields.name),
        serde_json::json!({ "cost": fields.cost, "rate": fields.rate, "method": fields.method, "opening_value": fields.opening_value }),
    )
    .await?;
    let row = repo::find_asset(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_asset(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: AssetInput,
) -> AppResult<AssetRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_asset(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    if before.archived {
        return Err(AppError::Conflict(
            "Restore this asset before changing it.".into(),
        ));
    }
    if !repo::depreciation_entries(&mut tx, Some(id))
        .await?
        .is_empty()
    {
        return Err(AppError::Conflict(
            "Depreciation has been posted for this asset. Remove that year’s depreciation first, then change it.".into(),
        ));
    }
    let fields = asset_fields(&mut tx, &input).await?;
    repo::update_asset(&mut tx, id, &fields).await?;
    sync_asset(&mut tx, ctx, &business, id).await?;
    record(
        &mut tx,
        ctx,
        "fixed_asset.updated",
        "fixed_asset",
        id,
        format!("Asset “{}” updated", fields.name),
        serde_json::json!({
            "cost": { "from": before.cost, "to": fields.cost },
            "rate": { "from": before.rate, "to": fields.rate },
            "opening_value": { "from": before.opening_value, "to": fields.opening_value },
        }),
    )
    .await?;
    let row = repo::find_asset(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    tx.commit().await?;
    Ok(row)
}

/// Archiving an asset stops its depreciation in later years (its history stays).
pub async fn archive_asset(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<AssetRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::find_asset(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    repo::set_asset_archived(&mut tx, id, archived).await?;
    record(
        &mut tx,
        ctx,
        if archived {
            "fixed_asset.archived"
        } else {
            "fixed_asset.restored"
        },
        "fixed_asset",
        id,
        format!(
            "Asset “{}” {}",
            row.name,
            if archived { "archived" } else { "restored" }
        ),
        serde_json::json!({}),
    )
    .await?;
    let row = repo::find_asset(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    tx.commit().await?;
    Ok(row)
}

// ─── Depreciation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DepreciationInput {
    /// The last day of the financial year to depreciate.
    pub year_end: NaiveDate,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DepreciationLine {
    pub asset_id: Uuid,
    pub name: String,
    pub asset_group: String,
    pub cost: i64,
    #[serde(flatten)]
    pub entry: DepreciationRow,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DepreciationYear {
    pub year_end: NaiveDate,
    pub lines: Vec<DepreciationLine>,
    pub depreciation: i64,
    pub private_amount: i64,
}

/// The financial year ending on `year_end`, or a field error if it isn't a year end.
fn year_of(business: &BusinessRow, year_end: NaiveDate) -> AppResult<(NaiveDate, NaiveDate)> {
    let (start, end) = financial_year(
        year_end,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    if end != year_end {
        return Err(AppError::validation(
            "year_end",
            &format!(
                "Choose the last day of a financial year, e.g. {}.",
                end.format("%d/%m/%Y")
            ),
        ));
    }
    Ok((start, end))
}

async fn depreciation_year(
    conn: &mut PgConnection,
    year_end: NaiveDate,
) -> AppResult<DepreciationYear> {
    let assets = repo::list_assets(conn).await?;
    let lines: Vec<DepreciationLine> = repo::depreciation_entries(conn, None)
        .await?
        .into_iter()
        .filter(|e| e.year_end == year_end)
        .filter_map(|entry| {
            let asset = assets.iter().find(|a| a.id == entry.asset_id)?;
            Some(DepreciationLine {
                asset_id: asset.id,
                name: asset.name.clone(),
                asset_group: asset.asset_group.clone(),
                cost: asset.cost,
                entry,
            })
        })
        .collect();
    Ok(DepreciationYear {
        year_end,
        depreciation: lines.iter().map(|l| l.entry.depreciation).sum(),
        private_amount: lines.iter().map(|l| l.entry.private_amount).sum(),
        lines,
    })
}

pub async fn get_depreciation(
    state: &AppState,
    ctx: &BusinessContext,
    year_end: NaiveDate,
) -> AppResult<DepreciationYear> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    year_of(&business, year_end)?;
    let year = depreciation_year(&mut tx, year_end).await?;
    tx.commit().await?;
    Ok(year)
}

/// Works out and posts every active asset's depreciation for the year, replacing what was posted
/// for it before (and removing later years, which depended on it).
pub async fn post_depreciation(
    state: &AppState,
    ctx: &BusinessContext,
    input: DepreciationInput,
) -> AppResult<DepreciationYear> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (start, end) = year_of(&business, input.year_end)?;
    let previous_end = start - Duration::days(1);
    let opening = repo::find_opening(&mut tx).await?;
    if let Some(o) = &opening
        && end <= o.as_at
    {
        return Err(AppError::validation(
            "year_end",
            "This year ends before your opening balances.",
        ));
    }
    let entries = repo::depreciation_entries(&mut tx, None).await?;
    let context = context(ctx, &business);
    let mut posted = 0;
    for asset in repo::list_assets(&mut tx)
        .await?
        .into_iter()
        .filter(|a| !a.archived && a.acquired_on <= end)
    {
        // No depreciation in the year an asset is sold, or after it. An earlier year's depreciation
        // sets the value the sale was measured against, so it can't change under a recorded sale.
        if let Some(sold) = asset.disposed_on {
            if sold <= end {
                continue;
            }
            return Err(AppError::Conflict(format!(
                "“{}” was sold on {}: undo the sale before changing an earlier year’s depreciation.",
                asset.name,
                sold.format("%d/%m/%Y")
            )));
        }
        let prior = entries
            .iter()
            .find(|e| e.asset_id == asset.id && e.year_end == previous_end);
        let earlier = entries
            .iter()
            .any(|e| e.asset_id == asset.id && e.year_end < previous_end);
        let opening_value = match (prior, asset.opening_value) {
            (Some(p), _) => Some(p.closing_value),
            (None, Some(_)) if earlier => {
                return Err(AppError::Conflict(format!(
                    "Post depreciation for the year ending {} first.",
                    previous_end.format("%d/%m/%Y")
                )));
            }
            (None, Some(value)) => {
                if let Some(o) = &opening
                    && o.as_at != previous_end
                {
                    return Err(AppError::Conflict(format!(
                        "Your opening balances are at {}: post depreciation from the year after that first.",
                        o.as_at.format("%d/%m/%Y")
                    )));
                }
                Some(value)
            }
            (None, None) if asset.acquired_on >= start => None,
            (None, None) => {
                return Err(AppError::Conflict(format!(
                    "“{}” was bought before this year: post depreciation for the year it was bought first.",
                    asset.name
                )));
            }
        };
        let months = dep::months_owned(asset.acquired_on, start, end);
        let year = dep::year(
            Method::parse(&asset.method),
            asset.rate,
            asset.cost,
            opening_value,
            months,
            asset.private_use,
        );
        let entry_id = repo::upsert_depreciation(
            &mut tx,
            ctx.business_id,
            asset.id,
            end,
            &DepreciationFields {
                opening_value: year.opening,
                purchases: year.purchases,
                depreciation: year.depreciation,
                private_amount: year.private_amount,
                closing_value: year.closing,
            },
        )
        .await?;
        let desired = if year.depreciation > 0 {
            vec![Desired {
                source_type: SOURCE_DEPRECIATION,
                source_id: entry_id,
                spec: ledger::depreciation(
                    end,
                    &asset.name,
                    year.depreciation,
                    year.private_amount,
                ),
            }]
        } else {
            Vec::new()
        };
        ledger_service::reconcile(
            &mut tx,
            &context,
            &[(SOURCE_DEPRECIATION, entry_id)],
            desired,
        )
        .await?;
        for later in repo::delete_depreciation_after(&mut tx, asset.id, end).await? {
            ledger_service::reconcile(
                &mut tx,
                &context,
                &[(SOURCE_DEPRECIATION, later)],
                Vec::new(),
            )
            .await?;
        }
        posted += year.depreciation;
    }
    record(
        &mut tx,
        ctx,
        "depreciation.posted",
        "depreciation",
        ctx.business_id,
        format!(
            "Depreciation for the year ending {} posted",
            end.format("%d/%m/%Y")
        ),
        serde_json::json!({ "year_end": end, "depreciation": posted }),
    )
    .await?;
    let year = depreciation_year(&mut tx, end).await?;
    tx.commit().await?;
    Ok(year)
}

/// Removes a year's depreciation (and later years') and reverses its postings.
pub async fn delete_depreciation(
    state: &AppState,
    ctx: &BusinessContext,
    year_end: NaiveDate,
) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    year_of(&business, year_end)?;
    let context = context(ctx, &business);
    let doomed: Vec<DepreciationRow> = repo::depreciation_entries(&mut tx, None)
        .await?
        .into_iter()
        .filter(|e| e.year_end >= year_end)
        .collect();
    if doomed.iter().all(|e| e.year_end != year_end) {
        return Err(AppError::NotFound("Depreciation"));
    }
    // A sale was measured against the value this depreciation leaves: it can't go under a sale.
    let assets = repo::list_assets(&mut tx).await?;
    if let Some(sold) = assets.iter().find(|a| {
        a.disposed_on.is_some_and(|d| d > year_end) && doomed.iter().any(|e| e.asset_id == a.id)
    }) {
        return Err(AppError::Conflict(format!(
            "“{}” was sold after this year: undo the sale before removing this depreciation.",
            sold.name
        )));
    }
    for entry in &doomed {
        repo::delete_depreciation(&mut tx, entry.id).await?;
        ledger_service::reconcile(
            &mut tx,
            &context,
            &[(SOURCE_DEPRECIATION, entry.id)],
            Vec::new(),
        )
        .await?;
    }
    record(
        &mut tx,
        ctx,
        "depreciation.removed",
        "depreciation",
        ctx.business_id,
        format!(
            "Depreciation from the year ending {} removed",
            year_end.format("%d/%m/%Y")
        ),
        serde_json::json!({ "year_end": year_end, "entries": doomed.len() }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
