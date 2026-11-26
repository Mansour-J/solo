//! Business health: gathers the facts `domain::business_health` checks, calculated on request.

use chrono::{DateTime, Duration, Months, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::business::{ReportingBasis, financial_year};
use crate::domain::business_health::{self, Facts, HealthCheck};
use crate::domain::insights::must_register_for_gst;
use crate::error::AppResult;
use crate::repositories::business_health as repo;
use crate::repositories::reports;
use crate::services::businesses::business_today;
use crate::services::invoices::load_business;
use crate::services::{backups, email_settings};
use crate::state::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct BusinessHealth {
    pub checked_at: DateTime<Utc>,
    /// When the books were last backed up (the desktop app; `null` when never or elsewhere).
    pub last_backup_at: Option<DateTime<Utc>>,
    /// Every check, in the order the screen shows them within each status.
    pub checks: Vec<HealthCheck>,
}

fn filled(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|v| !v.trim().is_empty())
}

pub async fn build(state: &AppState, ctx: &BusinessContext) -> AppResult<BusinessHealth> {
    let email_ready = email_settings::enabled(state).await?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let (fy_start, fy_end) = financial_year(
        today,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let basis = match business.reporting_basis {
        ReportingBasis::Cash => "cash",
        ReportingBasis::Accrual => "accrual",
    };
    // The same 12 months and sales figure as the GST threshold notice in Insights.
    let year_ago = today.checked_sub_months(Months::new(12)).unwrap_or(today) + Duration::days(1);
    let sales = reports::profit(&mut tx, year_ago, today, basis)
        .await?
        .income;
    let (has_payment_details, reminders_enabled) = repo::invoicing(&mut tx).await?;
    let (customers_owing, customers_owing_without_email) = repo::customers_owing(&mut tx).await?;
    let (expenses_without_receipt, gst_claims_without_receipt) =
        repo::expenses_without_receipt(&mut tx, fy_start, fy_end).await?;
    let last_backup = backups::last_backup(state, &mut tx).await?;
    tx.commit().await?;
    let now = Utc::now();

    let address = |field: &str| {
        business.address[field]
            .as_str()
            .is_some_and(|v| !v.trim().is_empty())
    };
    let facts = Facts {
        has_address: address("line1") || address("city"),
        has_email: filled(&business.email),
        has_phone: filled(&business.phone),
        gst_registered: business.gst_registered,
        has_gst_number: filled(&business.gst_number),
        has_gst_registered_from: business.gst_registered_from.is_some(),
        over_gst_threshold: must_register_for_gst(business.gst_registered, sales),
        has_payment_details,
        email_ready,
        reminders_enabled,
        customers_owing,
        customers_owing_without_email,
        expenses_without_receipt,
        gst_claims_without_receipt,
        backups_available: last_backup.is_some(),
        days_since_backup: last_backup.flatten().map(|at| (now - at).num_days()),
        business_age_days: (now - business.created_at).num_days(),
    };
    Ok(BusinessHealth {
        checked_at: now,
        last_backup_at: last_backup.flatten(),
        checks: business_health::checks(&facts),
    })
}
