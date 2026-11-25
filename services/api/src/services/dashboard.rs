//! Dashboard figures for a period, with the previous period of the same length for comparison.

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::business::ReportingBasis;
use crate::error::{AppError, AppResult};
use crate::repositories::invoices::ListFilter;
use crate::repositories::mileage::TripTotals;
use crate::repositories::reports::{
    self, AccountTotal, GstTotals, MonthTotals, ProfitTotals, Receivables,
};
use crate::services::businesses::business_today;
use crate::services::invoices::{self, InvoiceSummary, load_business};
use crate::services::mileage;
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct Dashboard {
    pub from: NaiveDate,
    pub to: NaiveDate,
    /// The basis used for income, expenses and GST.
    pub basis: ReportingBasis,
    pub currency: String,
    pub totals: ProfitTotals,
    /// The same figures for the preceding period of equal length.
    pub previous_totals: ProfitTotals,
    pub receivables: Receivables,
    pub monthly: Vec<MonthTotals>,
    pub expenses_by_category: Vec<AccountTotal>,
    pub gst: GstTotals,
    pub gst_registered: bool,
    /// Trips in the period (their claims are already part of the expenses).
    pub mileage: TripTotals,
    pub recent_invoices: Vec<InvoiceSummary>,
    /// True when the business has no financial records at all yet.
    pub is_empty: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DashboardQuery {
    /// Start of the period (inclusive). Defaults to the start of the current financial year.
    pub from: Option<NaiveDate>,
    /// End of the period (inclusive). Defaults to today.
    pub to: Option<NaiveDate>,
    /// Defaults to the business's reporting basis.
    pub basis: Option<ReportingBasis>,
}

pub async fn dashboard(
    state: &AppState,
    ctx: &BusinessContext,
    query: DashboardQuery,
) -> AppResult<Dashboard> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let (fy_start, _) = crate::domain::business::financial_year(
        today,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let from = query.from.unwrap_or(fy_start);
    let to = query.to.unwrap_or(today);
    if to < from {
        return Err(AppError::validation(
            "to",
            "The end date can’t be before the start date.",
        ));
    }
    if (to - from).num_days() > 366 * 5 {
        return Err(AppError::validation(
            "from",
            "Choose a period of up to five years.",
        ));
    }
    let basis = query.basis.unwrap_or(business.reporting_basis);
    let basis_str = match basis {
        ReportingBasis::Cash => "cash",
        ReportingBasis::Accrual => "accrual",
    };
    let length = to - from;
    let previous_to = from - Duration::days(1);
    let previous_from = previous_to - length;
    let totals = reports::profit(&mut tx, from, to, basis_str).await?;
    let previous_totals = reports::profit(&mut tx, previous_from, previous_to, basis_str).await?;
    let receivables = reports::receivables(&mut tx, today, from, to).await?;
    let monthly = reports::monthly(&mut tx, from, to, basis_str).await?;
    let expenses_by_category = reports::expenses_by_account(&mut tx, from, to, basis_str).await?;
    let gst = reports::gst(&mut tx, from, to, basis_str).await?;
    let mileage = mileage::totals_for_period(&mut tx, from, to).await?;
    let is_empty: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM journal_entries) AND NOT EXISTS (SELECT 1 FROM invoices)
           AND NOT EXISTS (SELECT 1 FROM mileage_trips)",
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let (recent_invoices, _) = invoices::list(
        state,
        ctx,
        ListFilter {
            statuses: None,
            customer_id: None,
            from: None,
            to: None,
            search: None,
            sort: String::new(),
            limit: 5,
            offset: 0,
        },
    )
    .await?;
    Ok(Dashboard {
        from,
        to,
        basis,
        currency: business.currency.clone(),
        totals,
        previous_totals,
        receivables,
        monthly,
        expenses_by_category,
        gst,
        gst_registered: business.gst_registered,
        mileage,
        recent_invoices,
        is_empty,
    })
}
