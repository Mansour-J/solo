//! Home office by IRD's square metre rate option: one claim per financial year, posted as a home office
//! expense the owner paid personally.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::business::financial_year;
use crate::domain::{home_office, ledger};
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses::BusinessRow;
use crate::repositories::home_office::{self as repo, ClaimFields, HomeOfficeClaim};
use crate::services::invoices::{clean, load_business};
use crate::services::ledger::{self as ledger_service, Context, Desired, SOURCE_HOME_OFFICE};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct HomeOfficeYear {
    pub year_end: NaiveDate,
    /// IRD's rate for the income year (cents per m²); `None` until IRD publishes it.
    pub rate: Option<i64>,
    pub claim: Option<HomeOfficeClaim>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct HomeOfficeInput {
    pub year_end: NaiveDate,
    /// Square metres set aside and used mainly for the business.
    #[schema(value_type = String)]
    pub business_area: Decimal,
    /// The home's total floor area in square metres.
    #[schema(value_type = String)]
    pub total_area: Decimal,
    /// Mortgage interest (not principal), rates and rent paid in the year, in minor units.
    #[serde(default)]
    pub premises_costs: i64,
    #[serde(default)]
    pub notes: String,
}

/// IRD's rates are for income years ending 31 March: the one this financial year ends in.
fn income_year_end(year_end: NaiveDate) -> NaiveDate {
    let year = if year_end.month() <= 3 {
        year_end.year()
    } else {
        year_end.year() + 1
    };
    NaiveDate::from_ymd_opt(year, 3, 31).expect("valid date")
}

fn check_year_end(business: &BusinessRow, year_end: NaiveDate) -> AppResult<()> {
    let (_, end) = financial_year(
        year_end,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    if end != year_end {
        return Err(AppError::validation(
            "year_end",
            "Choose the last day of a financial year.",
        ));
    }
    Ok(())
}

fn ledger_context(ctx: &BusinessContext, business: &BusinessRow) -> Context {
    Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    }
}

async fn audit_entry(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    action: &str,
    id: uuid::Uuid,
    summary: String,
    changes: serde_json::Value,
) -> AppResult<()> {
    audit::record(
        conn,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action,
            entity_type: "home_office_claim",
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await?;
    Ok(())
}

pub async fn year(
    state: &AppState,
    ctx: &BusinessContext,
    year_end: NaiveDate,
) -> AppResult<HomeOfficeYear> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    check_year_end(&business, year_end)?;
    let rate = repo::rate(&mut tx, income_year_end(year_end)).await?;
    let claim = repo::find(&mut tx, year_end).await?;
    tx.commit().await?;
    Ok(HomeOfficeYear {
        year_end,
        rate,
        claim,
    })
}

pub async fn save(
    state: &AppState,
    ctx: &BusinessContext,
    input: HomeOfficeInput,
) -> AppResult<HomeOfficeYear> {
    let mut errors = FieldErrors::new();
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    errors.check(
        input.business_area > Decimal::ZERO && input.business_area < Decimal::from(100_000),
        "business_area",
        "Enter the business area in square metres.",
    );
    errors.check(
        input.total_area > Decimal::ZERO && input.total_area < Decimal::from(100_000),
        "total_area",
        "Enter your home’s floor area in square metres.",
    );
    errors.check(
        input.business_area <= input.total_area,
        "business_area",
        "The business area can’t be more than the whole home.",
    );
    errors.check(
        (0..=100_000_000_000).contains(&input.premises_costs),
        "premises_costs",
        "Enter the year’s mortgage interest, rates and rent.",
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    check_year_end(&business, input.year_end)?;
    let Some(rate) = repo::rate(&mut tx, income_year_end(input.year_end)).await? else {
        return Err(AppError::validation(
            "year_end",
            "IRD hasn’t published the square metre rate for this year yet.",
        ));
    };
    let business_area = input.business_area.round_dp(2);
    let total_area = input.total_area.round_dp(2);
    let amount = home_office::claim(input.premises_costs, business_area, total_area, rate);
    let before = repo::find(&mut tx, input.year_end).await?;
    let claim = repo::save(
        &mut tx,
        ctx.business_id,
        &ClaimFields {
            year_end: input.year_end,
            business_area,
            total_area,
            premises_costs: input.premises_costs,
            rate,
            claim: amount,
            notes,
        },
    )
    .await?;
    ledger_service::reconcile(
        &mut tx,
        &ledger_context(ctx, &business),
        &[(SOURCE_HOME_OFFICE, claim.id)],
        vec![Desired {
            source_type: SOURCE_HOME_OFFICE,
            source_id: claim.id,
            spec: ledger::home_office_claim(input.year_end, amount),
        }],
    )
    .await?;
    audit_entry(
        &mut tx,
        ctx,
        "home_office.saved",
        claim.id,
        format!(
            "Home office claim for the year to {} saved",
            input.year_end.format("%d/%m/%Y")
        ),
        serde_json::json!({ "claim": { "from": before.map(|b| b.claim), "to": amount } }),
    )
    .await?;
    tx.commit().await?;
    Ok(HomeOfficeYear {
        year_end: input.year_end,
        rate: Some(rate),
        claim: Some(claim),
    })
}

pub async fn delete(state: &AppState, ctx: &BusinessContext, year_end: NaiveDate) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let Some(id) = repo::delete(&mut tx, year_end).await? else {
        return Err(AppError::NotFound("Home office claim"));
    };
    ledger_service::reconcile(
        &mut tx,
        &ledger_context(ctx, &business),
        &[(SOURCE_HOME_OFFICE, id)],
        Vec::new(),
    )
    .await?;
    audit_entry(
        &mut tx,
        ctx,
        "home_office.deleted",
        id,
        format!(
            "Home office claim for the year to {} removed",
            year_end.format("%d/%m/%Y")
        ),
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
