//! GST change-of-use adjustments for assets over $10,000 (IR375): the review periods of an asset, the
//! business use recorded for each, and the adjustments they post to the GST account.

use chrono::{Duration, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::financial_year;
use crate::domain::change_of_use::{self as rules, Period};
use crate::domain::ledger;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::annual::{self as repo, AssetRow};
use crate::repositories::audit;
use crate::repositories::businesses::BusinessRow;
use crate::services::businesses::business_today;
use crate::services::invoices::{clean, gst_context, load_business};
use crate::services::ledger::{self as ledger_service, Context, Desired, SOURCE_ASSET_USE_REVIEW};
use crate::state::AppState;

/// One adjustment period of an asset.
#[derive(Debug, Serialize, ToSchema)]
pub struct UsePeriod {
    /// The balance date that ends the period.
    pub period_end: NaiveDate,
    pub months: u32,
    /// Business use recorded for the period (None until reviewed).
    #[schema(value_type = Option<String>)]
    pub business_use: Option<Decimal>,
    pub notes: String,
    /// Business use from acquisition to the end of this period (once reviewed).
    #[schema(value_type = Option<String>)]
    pub actual_use: Option<Decimal>,
    /// What it's compared with: the intended use, or the use at the last adjustment.
    #[schema(value_type = Option<String>)]
    pub previous_use: Option<Decimal>,
    /// GST to claim (positive) or pay back (negative), minor units; 0 when the change is too small.
    pub adjustment: Option<i64>,
    /// The period has ended, so it can be reviewed.
    pub ended: bool,
    /// The next period to review: earlier ones are all reviewed.
    pub reviewable: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssetUse {
    pub asset_id: Uuid,
    /// Whether the asset needs change-of-use reviews.
    pub applies: bool,
    /// Why not, when it doesn't.
    pub reason: Option<String>,
    /// GST on the whole cost (minor units).
    pub full_gst: i64,
    /// Business use intended when bought (100 − private use).
    #[schema(value_type = String)]
    pub intended_use: Decimal,
    pub periods: Vec<UsePeriod>,
    /// Sum of the adjustments so far (minor units).
    pub total_adjustment: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UseReviewInput {
    pub period_end: NaiveDate,
    /// Business use in the period (0–100); null removes this review and the later ones.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub business_use: Option<Decimal>,
    #[serde(default)]
    pub notes: String,
}

/// The asset's adjustment periods: the first ends at the first balance date on or after it was
/// bought, then one per financial year.
fn period_ends(business: &BusinessRow, acquired_on: NaiveDate, count: u32) -> Vec<NaiveDate> {
    let (month, day) = (
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let mut ends = Vec::new();
    let mut end = financial_year(acquired_on, month, day).1;
    for _ in 0..count {
        ends.push(end);
        end = financial_year(end + Duration::days(1), month, day).1;
    }
    ends
}

async fn build(
    conn: &mut PgConnection,
    business: &BusinessRow,
    asset: &AssetRow,
) -> AppResult<(AssetUse, Vec<(Uuid, NaiveDate, i64)>)> {
    let intended_use = Decimal::ONE_HUNDRED - asset.private_use;
    let gst = gst_context(conn, business, asset.acquired_on).await?;
    let full_gst = (Decimal::from(asset.cost) * gst.standard_rate)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0);
    let count = rules::adjustment_periods(asset.cost);
    let reason = if !gst.registered || full_gst == 0 {
        Some("No GST was claimed on this asset, so there’s nothing to adjust.".to_owned())
    } else if count == 0 {
        Some(
            "Assets costing $10,000 or less (excluding GST) don’t need change-of-use adjustments."
                .to_owned(),
        )
    } else {
        None
    };
    let mut result = AssetUse {
        asset_id: asset.id,
        applies: reason.is_none(),
        reason,
        full_gst,
        intended_use,
        periods: Vec::new(),
        total_adjustment: 0,
    };
    if !result.applies {
        return Ok((result, Vec::new()));
    }
    let today = business_today(&business.timezone);
    let saved = repo::use_reviews(conn, asset.id).await?;
    let ends = period_ends(business, asset.acquired_on, count);
    let mut start = asset.acquired_on;
    let mut periods = Vec::new();
    for end in &ends {
        let review = saved.iter().find(|r| r.period_end == *end);
        periods.push(Period {
            end: *end,
            months: rules::months(start, *end),
            business_use: review.map(|r| r.business_use),
        });
        start = *end + Duration::days(1);
    }
    let reviews = rules::review(full_gst, intended_use, &periods);
    let mut postings = Vec::new();
    for (i, period) in periods.iter().enumerate() {
        let review = reviews.get(i);
        let row = saved.iter().find(|r| r.period_end == period.end);
        if let (Some(review), Some(row)) = (review, row) {
            postings.push((row.id, period.end, review.adjustment));
        }
        result.periods.push(UsePeriod {
            period_end: period.end,
            months: period.months,
            business_use: period.business_use,
            notes: row.map(|r| r.notes.clone()).unwrap_or_default(),
            actual_use: review.map(|r| r.actual_use),
            previous_use: review.map(|r| r.previous_use),
            adjustment: review.map(|r| r.adjustment),
            ended: period.end <= today,
            reviewable: period.end <= today && i <= reviews.len(),
        });
    }
    result.total_adjustment = reviews.iter().map(|r| r.adjustment).sum();
    Ok((result, postings))
}

/// The business use claimed so far for an asset that needs change-of-use reviews, for the final
/// adjustment when it's sold (None when reviews don't apply to it).
pub async fn use_claimed(
    conn: &mut PgConnection,
    business: &BusinessRow,
    asset: &AssetRow,
) -> AppResult<Option<Decimal>> {
    let (result, _) = build(conn, business, asset).await?;
    if !result.applies {
        return Ok(None);
    }
    let reviews: Vec<rules::Review> = result
        .periods
        .iter()
        .filter_map(|p| {
            Some(rules::Review {
                end: p.period_end,
                actual_use: p.actual_use?,
                previous_use: p.previous_use?,
                change: p.actual_use? - p.previous_use?,
                adjustment: p.adjustment?,
            })
        })
        .collect();
    Ok(Some(rules::current_use(&reviews, result.intended_use)))
}

async fn load_asset(conn: &mut PgConnection, id: Uuid) -> AppResult<AssetRow> {
    repo::find_asset(conn, id)
        .await?
        .ok_or(AppError::NotFound("Asset"))
}

pub async fn asset_use(
    state: &AppState,
    ctx: &BusinessContext,
    asset_id: Uuid,
) -> AppResult<AssetUse> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let asset = load_asset(&mut tx, asset_id).await?;
    let (result, _) = build(&mut tx, &business, &asset).await?;
    tx.commit().await?;
    Ok(result)
}

/// Posts each review's adjustment (and reverses those of reviews removed or no longer adjusting).
async fn sync_postings(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    asset: &AssetRow,
    removed: &[Uuid],
) -> AppResult<()> {
    let (_, postings) = build(conn, business, asset).await?;
    let mut sources: Vec<(&str, Uuid)> = postings
        .iter()
        .map(|(id, _, _)| (SOURCE_ASSET_USE_REVIEW, *id))
        .collect();
    sources.extend(removed.iter().map(|id| (SOURCE_ASSET_USE_REVIEW, *id)));
    let desired = postings
        .iter()
        .filter(|(_, _, amount)| *amount != 0)
        .map(|(id, end, amount)| Desired {
            source_type: SOURCE_ASSET_USE_REVIEW,
            source_id: *id,
            spec: ledger::asset_use_adjustment(*end, &asset.name, *amount),
        })
        .collect();
    let context = Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    };
    ledger_service::reconcile(conn, &context, &sources, desired).await
}

pub async fn save_review(
    state: &AppState,
    ctx: &BusinessContext,
    asset_id: Uuid,
    input: UseReviewInput,
) -> AppResult<AssetUse> {
    let mut errors = FieldErrors::new();
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    let business_use = input.business_use.map(|u| u.round_dp(2));
    if let Some(used) = business_use {
        errors.check(
            used >= Decimal::ZERO && used <= Decimal::ONE_HUNDRED,
            "business_use",
            "Enter a percentage between 0 and 100.",
        );
    }
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let asset = load_asset(&mut tx, asset_id).await?;
    let (before, _) = build(&mut tx, &business, &asset).await?;
    if !before.applies {
        return Err(AppError::Conflict(before.reason.unwrap_or_default()));
    }
    let Some(index) = before
        .periods
        .iter()
        .position(|p| p.period_end == input.period_end)
    else {
        return Err(AppError::validation(
            "period_end",
            "This isn’t one of the asset’s adjustment periods.",
        ));
    };
    let period = &before.periods[index];
    let mut removed = Vec::new();
    match business_use {
        Some(used) => {
            if !period.reviewable {
                return Err(AppError::validation(
                    "business_use",
                    if period.ended {
                        "Review the earlier periods first."
                    } else {
                        "This period hasn’t ended yet."
                    },
                ));
            }
            repo::save_use_review(
                &mut tx,
                ctx.business_id,
                asset_id,
                input.period_end,
                used,
                &notes,
            )
            .await?;
        }
        None => {
            removed = repo::use_reviews(&mut tx, asset_id)
                .await?
                .into_iter()
                .filter(|r| r.period_end >= input.period_end)
                .map(|r| r.id)
                .collect();
            repo::delete_use_review(&mut tx, asset_id, input.period_end).await?;
        }
    }
    sync_postings(&mut tx, ctx, &business, &asset, &removed).await?;
    let (after, _) = build(&mut tx, &business, &asset).await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "asset.use_reviewed",
            entity_type: "fixed_asset",
            entity_id: Some(asset_id),
            summary: format!("GST change-of-use review of {}", asset.name),
            changes: serde_json::json!({
                "period_end": input.period_end,
                "business_use": { "from": period.business_use, "to": business_use },
                "total_adjustment": { "from": before.total_adjustment, "to": after.total_adjustment },
            }),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(after)
}
