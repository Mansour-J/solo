//! Selling or scrapping a fixed asset (IR260): no depreciation in the year of disposal; the sale is
//! compared with the adjusted tax value at the start of that year. See `domain::depreciation::disposal`.

use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::financial_year;
use crate::domain::change_of_use;
use crate::domain::depreciation::disposal;
use crate::domain::ledger;
use crate::domain::money::Currency;
use crate::domain::tax::gst_in_inclusive;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::annual::{self as repo, DisposalFields, DisposalRow};
use crate::repositories::audit;
use crate::services::change_of_use::use_claimed;
use crate::services::invoices::{clean, gst_context, load_business};
use crate::services::ledger::{self as ledger_service, Context, Desired};
use crate::state::AppState;

pub const SOURCE_ASSET_DISPOSAL: &str = "asset_disposal";

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DisposalInput {
    pub disposed_on: NaiveDate,
    /// What was received, including any GST (minor units); 0 when scrapped.
    pub sale_price: i64,
    #[serde(default)]
    pub notes: String,
}

/// Records an asset as sold or scrapped and posts the result (or updates the record).
pub async fn dispose(
    state: &AppState,
    ctx: &BusinessContext,
    asset_id: Uuid,
    input: DisposalInput,
) -> AppResult<DisposalRow> {
    let mut errors = FieldErrors::new();
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    errors.check(
        input.sale_price >= 0 && input.sale_price <= 1_000_000_000_000,
        "sale_price",
        "Enter what you received (0 if it was scrapped).",
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let asset = repo::find_asset(&mut tx, asset_id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    if input.disposed_on < asset.acquired_on {
        return Err(AppError::validation(
            "disposed_on",
            "This is before the asset was bought.",
        ));
    }
    let opening = repo::find_opening(&mut tx).await?;
    if let Some(o) = &opening
        && input.disposed_on <= o.as_at
    {
        return Err(AppError::validation(
            "disposed_on",
            "This is before your opening balances.",
        ));
    }
    let (start, end) = financial_year(
        input.disposed_on,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let previous_end = start - Duration::days(1);
    let entries = repo::depreciation_entries(&mut tx, Some(asset_id)).await?;
    if let Some(later) = entries.iter().find(|e| e.year_end >= end) {
        return Err(AppError::Conflict(format!(
            "There’s no depreciation in the year an asset is sold: remove the depreciation for the year ending {} first.",
            later.year_end.format("%d/%m/%Y")
        )));
    }
    // The adjusted tax value at the start of the year of disposal.
    let book_value = match entries.iter().find(|e| e.year_end == previous_end) {
        Some(e) => e.closing_value,
        None if asset.acquired_on >= start => asset.cost,
        None => match (asset.opening_value, &opening) {
            (Some(value), Some(o)) if o.as_at == previous_end => value,
            _ => {
                return Err(AppError::Conflict(format!(
                    "Post depreciation for the year ending {} first: the sale is measured against the value at the start of this year.",
                    previous_end.format("%d/%m/%Y")
                )));
            }
        },
    };
    let gst = gst_context(&mut tx, &business, input.disposed_on).await?;
    let sale_gst = if gst.registered {
        gst_in_inclusive(input.sale_price, gst.standard_rate).unwrap_or(0)
    } else {
        0
    };
    let outcome = disposal(
        book_value,
        asset.cost,
        input.sale_price - sale_gst,
        asset.private_use,
    );
    let final_gst_credit = match use_claimed(&mut tx, &business, &asset).await? {
        Some(claimed) if sale_gst > 0 => change_of_use::final_adjustment(sale_gst, claimed),
        _ => 0,
    };
    let before = repo::find_disposal(&mut tx, asset_id).await?;
    let id = repo::save_disposal(
        &mut tx,
        ctx.business_id,
        asset_id,
        &DisposalFields {
            disposed_on: input.disposed_on,
            sale_price: input.sale_price,
            sale_gst,
            book_value,
            recovered: outcome.recovered,
            loss: outcome.loss,
            capital_gain: outcome.capital_gain,
            notes,
        },
    )
    .await?;
    let spec = ledger::asset_disposal(
        input.disposed_on,
        &asset.name,
        input.sale_price,
        sale_gst,
        book_value,
        &outcome,
        final_gst_credit,
    );
    let desired = if spec.is_valid() {
        vec![Desired {
            source_type: SOURCE_ASSET_DISPOSAL,
            source_id: id,
            spec,
        }]
    } else {
        Vec::new()
    };
    let context = Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    };
    ledger_service::reconcile(&mut tx, &context, &[(SOURCE_ASSET_DISPOSAL, id)], desired).await?;
    let money = |amount: i64| {
        Currency::from_code(&business.currency)
            .map_or_else(|| amount.to_string(), |c| c.format(amount))
    };
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: if before.is_some() {
                "asset.disposal_updated"
            } else {
                "asset.disposed"
            },
            entity_type: "fixed_asset",
            entity_id: Some(asset_id),
            summary: format!(
                "{} sold or scrapped for {}",
                asset.name,
                money(input.sale_price)
            ),
            changes: serde_json::json!({
                "disposed_on": input.disposed_on,
                "sale_price": input.sale_price,
                "book_value": book_value,
                "recovered": outcome.recovered,
                "loss": outcome.loss,
                "final_gst_credit": final_gst_credit,
            }),
        },
    )
    .await?;
    let row = repo::find_disposal(&mut tx, asset_id)
        .await?
        .ok_or(AppError::NotFound("Disposal"))?;
    tx.commit().await?;
    Ok(row)
}

/// Undoes a disposal: the asset is held again and the posting is reversed.
pub async fn undo(state: &AppState, ctx: &BusinessContext, asset_id: Uuid) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let asset = repo::find_asset(&mut tx, asset_id)
        .await?
        .ok_or(AppError::NotFound("Asset"))?;
    let Some(id) = repo::delete_disposal(&mut tx, asset_id).await? else {
        return Err(AppError::NotFound("Disposal"));
    };
    let context = Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    };
    ledger_service::reconcile(
        &mut tx,
        &context,
        &[(SOURCE_ASSET_DISPOSAL, id)],
        Vec::new(),
    )
    .await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "asset.disposal_undone",
            entity_type: "fixed_asset",
            entity_id: Some(asset_id),
            summary: format!("Sale of {} undone", asset.name),
            changes: serde_json::json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn get(
    state: &AppState,
    ctx: &BusinessContext,
    asset_id: Uuid,
) -> AppResult<DisposalRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = repo::find_disposal(&mut tx, asset_id)
        .await?
        .ok_or(AppError::NotFound("Disposal"))?;
    tx.commit().await?;
    Ok(row)
}
