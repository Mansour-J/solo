//! Upcoming key dates for the dashboard: GST returns, provisional and terminal tax, the income tax
//! return (IR3) and the end of the financial year, each with the days left. The dates come from the
//! GST and income tax services, so they match those pages exactly.

use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::business::financial_year;
use crate::domain::nz_calendar::next_working_day;
use crate::error::AppResult;
use crate::services::businesses::business_today;
use crate::services::gst_returns::{self, GstReturnStatus};
use crate::services::income_tax::{self, TaxPaymentKind, TaxPaymentStatus};
use crate::services::invoices::load_business;
use crate::state::AppState;

/// Dates within this many days are "due soon".
const DUE_SOON_DAYS: i64 = 14;
/// How many dates the dashboard shows.
const MAX_DATES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyDateKind {
    GstReturn,
    ProvisionalTax,
    TerminalTax,
    IncomeTaxReturn,
    YearEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyDateUrgency {
    /// Past the date with something still to do or pay.
    Overdue,
    /// Within two weeks.
    DueSoon,
    Upcoming,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct KeyDate {
    pub kind: KeyDateKind,
    /// "GST return", "Provisional tax".
    pub title: String,
    /// What it's for, e.g. "1 Oct – 30 Nov 2026" or "Instalment 2 of 3, FY2027".
    pub detail: String,
    pub date: NaiveDate,
    /// Days from today (negative when overdue).
    pub days_left: i64,
    /// Amount to pay so far (minor units), when there is one.
    pub amount: Option<i64>,
    pub urgency: KeyDateUrgency,
    /// The client route that deals with it.
    pub link: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct KeyDates {
    pub today: NaiveDate,
    /// Overdue first, then by date; at most six.
    pub items: Vec<KeyDate>,
}

fn short(date: NaiveDate) -> String {
    date.format("%-d %b %Y").to_string()
}

fn item(
    today: NaiveDate,
    kind: KeyDateKind,
    title: &str,
    detail: String,
    (date, overdue): (NaiveDate, bool),
    amount: Option<i64>,
    link: &str,
) -> KeyDate {
    let days_left = (date - today).num_days();
    KeyDate {
        kind,
        title: title.into(),
        detail,
        date,
        days_left,
        amount,
        urgency: if overdue {
            KeyDateUrgency::Overdue
        } else if days_left <= DUE_SOON_DAYS {
            KeyDateUrgency::DueSoon
        } else {
            KeyDateUrgency::Upcoming
        },
        link: link.into(),
    }
}

pub async fn upcoming(state: &AppState, ctx: &BusinessContext) -> AppResult<KeyDates> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    tx.commit().await?;
    let today = business_today(&business.timezone);
    let mut items = Vec::new();

    // GST: returns still to pay or file, including the one in progress.
    let gst = gst_returns::returns(state, ctx).await?;
    if gst.gst_registered {
        for r in &gst.returns {
            let open = match r.status {
                GstReturnStatus::Overdue => true,
                GstReturnStatus::ToFile | GstReturnStatus::InProgress => r.due_date >= today,
                _ => false,
            };
            if open {
                items.push(item(
                    today,
                    KeyDateKind::GstReturn,
                    "GST return",
                    format!("{} – {}", r.start.format("%-d %b"), short(r.end)),
                    (r.due_date, r.status == GstReturnStatus::Overdue),
                    (r.outstanding > 0).then_some(r.outstanding),
                    "/gst",
                ));
            }
        }
    }

    // Income tax: provisional instalments and terminal tax still to pay.
    let tax = income_tax::summary(state, ctx).await?;
    for year in &tax.years {
        for p in &year.payments {
            let overdue = p.status == TaxPaymentStatus::Overdue;
            let open = overdue
                || (matches!(
                    p.status,
                    TaxPaymentStatus::Upcoming | TaxPaymentStatus::InProgress
                ) && p.due_date >= today);
            if !open {
                continue;
            }
            let (kind, title) = match p.kind {
                TaxPaymentKind::Provisional => (KeyDateKind::ProvisionalTax, "Provisional tax"),
                TaxPaymentKind::Terminal => (KeyDateKind::TerminalTax, "Terminal tax"),
            };
            items.push(item(
                today,
                kind,
                title,
                format!("{}, {}", p.label, year.label),
                (p.due_date, overdue),
                (p.outstanding > 0).then_some(p.outstanding),
                "/income-tax",
            ));
        }
    }

    // The income tax return (IR3) for the last year that ended: due 7 July.
    let (month, day) = (
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let (year_start, year_end) = financial_year(today, month, day);
    let last_end = year_start - Duration::days(1);
    if let Some(due) =
        NaiveDate::from_ymd_opt(last_end.year() + i32::from(last_end.month() > 6), 7, 7)
            .map(next_working_day)
            .filter(|due| *due >= today)
    {
        let label = tax
            .years
            .iter()
            .find(|y| y.end == last_end)
            .map(|y| y.label.clone())
            .unwrap_or_else(|| format!("year to {}", short(last_end)));
        items.push(item(
            today,
            KeyDateKind::IncomeTaxReturn,
            "Income tax return (IR3)",
            format!("For {label}, without a tax agent"),
            (due, false),
            None,
            "/income-tax",
        ));
    }

    // The end of this financial year: depreciation, home office and asset reviews follow it.
    items.push(item(
        today,
        KeyDateKind::YearEnd,
        "Financial year ends",
        "Then make your yearly report".into(),
        (year_end, false),
        None,
        "/year-end",
    ));

    items.sort_by_key(|i| (i.urgency != KeyDateUrgency::Overdue, i.date));
    items.truncate(MAX_DATES);
    Ok(KeyDates { today, items })
}
