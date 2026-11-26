//! Income tax for each income year: taxable income from the journal, tax by IRD's bands, tax already
//! withheld (schedular payments), residual income tax (RIT), provisional tax under the standard option
//! and terminal tax, with the income tax payments recorded settling them oldest first.
//!
//! An estimate from the business's records only: other income (wages, interest, rent), tax credits
//! and losses carried forward aren't known here. ACC levies are estimated on the same profit
//! (`domain::acc`), with the owner's work levy rate and whether they work full-time.

use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::acc::{self, AccRates, Adjustment};
use crate::domain::business::{self, ReportingBasis};
use crate::domain::gst_returns::{self, GstPeriod};
use crate::domain::income_tax::{self, Band};
use crate::error::{AppResult, FieldErrors};
use crate::repositories::{annual, audit, reports as repo, tax_rates};
use crate::services::businesses::business_today;
use crate::services::invoices::load_business;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaxPaymentKind {
    Provisional,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaxPaymentStatus {
    /// The year hasn't ended: terminal tax so far.
    InProgress,
    /// Not due yet.
    Upcoming,
    /// Past its due date with tax still to pay.
    Overdue,
    Paid,
    /// IRD owes a refund (more was withheld or paid than the tax).
    RefundDue,
    Refunded,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaxPayment {
    pub kind: TaxPaymentKind,
    /// "Instalment 1 of 3", "Terminal tax".
    pub label: String,
    pub due_date: NaiveDate,
    /// Positive: to pay; negative: a refund due.
    pub amount: i64,
    pub settled: i64,
    pub outstanding: i64,
    pub status: TaxPaymentStatus,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaxBand {
    pub from: i64,
    pub to: Option<i64>,
    /// e.g. "0.175".
    pub rate: String,
    pub income: i64,
    pub tax: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccAdjustment {
    /// Levied on the earnings as recorded.
    None,
    /// Full-time and under the minimum: levied on the minimum.
    Minimum,
    /// Over the maximum: levied on the maximum.
    Maximum,
}

/// ACC levies on the year's profit (an estimate: ACC invoices them after the IR3 return is filed).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AccEstimate {
    pub liable_earnings: i64,
    pub adjustment: AccAdjustment,
    pub min_liable: i64,
    pub max_liable: i64,
    /// Dollars per $100, excl. GST, e.g. "1.52".
    pub earners_levy_rate: String,
    pub earners_levy: i64,
    /// The owner's rate; `null` until they enter it.
    pub work_levy_rate: Option<String>,
    pub work_levy: Option<i64>,
    pub working_safer_levy_rate: String,
    pub working_safer_levy: i64,
    /// GST ACC adds to the levies.
    pub gst: i64,
    /// Levies plus GST.
    pub total: i64,
}

/// What the ACC estimate needs from the owner.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccSettings {
    /// The work levy for the business's ACC classification unit, dollars per $100 excl. GST (on the
    /// ACC invoice, or in ACC's levy guidebook); `null` when not known.
    #[schema(value_type = Option<String>)]
    pub work_levy_rate: Option<Decimal>,
    /// Works over 30 hours a week on average, so ACC levies at least the minimum.
    pub full_time: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaxYear {
    /// e.g. "FY2026".
    pub label: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// The year has ended (its figures are final as far as the records go).
    pub ended: bool,
    pub income: i64,
    /// Expenses that reduce taxable income (non-deductible ones excluded).
    pub deductible_expenses: i64,
    pub taxable_income: i64,
    pub bands: Vec<TaxBand>,
    pub income_tax: i64,
    /// Tax customers withheld from payments (schedular payments) in the year.
    pub withholding_tax: i64,
    /// `income_tax − withholding_tax`: negative is a refund.
    pub residual_income_tax: i64,
    /// Provisional tax for this year (last year's RIT + 5%), when last year's RIT was over $5,000.
    pub provisional_tax: Option<i64>,
    pub payments: Vec<TaxPayment>,
    /// ACC levies on this year's profit; `null` for years before Solo has ACC's rates (2024-25).
    pub acc: Option<AccEstimate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IncomeTax {
    pub basis: ReportingBasis,
    /// Newest first.
    pub years: Vec<TaxYear>,
    /// Tax due now or earlier and not yet paid.
    pub overdue: i64,
    /// The next payment with something to pay.
    pub next_due: Option<TaxPayment>,
    /// Refunds IRD owes for years that have ended.
    pub refunds_due: i64,
    /// Payments beyond everything due so far.
    pub credit: i64,
    pub acc_settings: AccSettings,
}

pub async fn summary(state: &AppState, ctx: &BusinessContext) -> AppResult<IncomeTax> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let basis = business.reporting_basis;
    let basis_name = match basis {
        ReportingBasis::Cash => "cash",
        ReportingBasis::Accrual => "accrual",
    };
    let (month, day) = (
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let six_monthly = business.gst_registered && business.gst_period == GstPeriod::SixMonthly;

    let (work_levy_rate, full_time) = repo::acc_settings(&mut tx).await?;
    let opening = annual::find_opening(&mut tx).await?;
    let first = match &opening {
        Some(o) => Some(o.as_at + Duration::days(1)),
        None => repo::first_entry_date(&mut tx).await?,
    };
    let mut years = Vec::new();
    if let Some(first) = first {
        let (mut start, mut end) = business::financial_year(first, month, day);
        let (_, current_end) = business::financial_year(today, month, day);
        while end <= current_end {
            let bands: Vec<Band> = repo::income_tax_bands(&mut tx, start)
                .await?
                .into_iter()
                .map(|(from, rate)| Band { from, rate })
                .collect();
            let income: i64 = repo::totals_by_account(&mut tx, start, end, basis_name, "income")
                .await?
                .iter()
                .map(|a| a.amount)
                .sum();
            let non_deductible_share: i64 =
                repo::non_deductible_shares(&mut tx, start, end, basis_name)
                    .await?
                    .iter()
                    .filter(|s| !s.gst)
                    .map(|s| s.amount)
                    .sum();
            let deductible: i64 =
                repo::totals_by_account(&mut tx, start, end, basis_name, "expense")
                    .await?
                    .iter()
                    .filter(|a| !a.non_deductible)
                    .map(|a| a.amount)
                    .sum::<i64>()
                    - non_deductible_share;
            let withholding = repo::withholding_credits(&mut tx, start, end, basis_name).await?;
            let taxable = income - deductible;
            let (tax, rows) = income_tax::tax_on(taxable, &bands);
            let gst_rate = tax_rates::rate_on(&mut tx, "NZ", "gst", start)
                .await?
                .unwrap_or_default();
            let acc = repo::acc_rates(&mut tx, start).await?.map(
                |(earners_levy, working_safer_levy, min_liable, max_liable)| {
                    let rates = AccRates {
                        earners_levy,
                        working_safer_levy,
                        min_liable,
                        max_liable,
                    };
                    acc_estimate(taxable, &rates, work_levy_rate, full_time, gst_rate)
                },
            );
            years.push(TaxYear {
                label: business::financial_year_label(end),
                start,
                end,
                ended: end < today,
                income,
                deductible_expenses: deductible,
                taxable_income: taxable,
                bands: rows
                    .into_iter()
                    .map(|b| TaxBand {
                        from: b.from,
                        to: b.to,
                        rate: b.rate.normalize().to_string(),
                        income: b.income,
                        tax: b.tax,
                    })
                    .collect(),
                income_tax: tax,
                withholding_tax: withholding,
                residual_income_tax: tax - withholding,
                provisional_tax: None,
                payments: Vec::new(),
                acc,
            });
            start = end + Duration::days(1);
            end = business::financial_year(start, month, day).1;
        }
    }
    let (paid, refunded) = repo::income_tax_settlements(&mut tx).await?;
    tx.commit().await?;

    // Provisional tax from the year before, then terminal tax for what's left.
    for i in 0..years.len() {
        let provisional = i
            .checked_sub(1)
            .and_then(|p| income_tax::standard_provisional(years[p].residual_income_tax));
        let year = &mut years[i];
        year.provisional_tax = provisional;
        if let Some(total) = provisional {
            let dates = income_tax::instalment_dates(year.start, six_monthly);
            let count = dates.len();
            for (n, (due, amount)) in dates
                .into_iter()
                .zip(income_tax::split(total, count))
                .enumerate()
            {
                year.payments.push(new_payment(
                    TaxPaymentKind::Provisional,
                    format!("Instalment {} of {count}", n + 1),
                    due,
                    amount,
                ));
            }
        }
        let terminal = year.residual_income_tax - provisional.unwrap_or(0);
        year.payments.push(new_payment(
            TaxPaymentKind::Terminal,
            if terminal < 0 {
                "Refund".into()
            } else {
                "Terminal tax".into()
            },
            income_tax::terminal_tax_due(year.end),
            terminal,
        ));
    }

    // Payments settle what's due oldest first; refunds received settle refunds due.
    let mut order: Vec<(usize, usize)> = years
        .iter()
        .enumerate()
        .flat_map(|(y, year)| (0..year.payments.len()).map(move |p| (y, p)))
        .collect();
    order.sort_by_key(|&(y, p)| years[y].payments[p].due_date);
    let amounts: Vec<i64> = order
        .iter()
        .map(|&(y, p)| years[y].payments[p].amount)
        .collect();
    let settlement = gst_returns::settle(&amounts, paid, refunded);
    let mut result = IncomeTax {
        basis,
        years: Vec::new(),
        overdue: 0,
        next_due: None,
        refunds_due: 0,
        credit: settlement.unapplied_payments,
        acc_settings: AccSettings {
            work_levy_rate,
            full_time,
        },
    };
    for (&(y, p), settled) in order.iter().zip(settlement.settled) {
        let ended = years[y].ended;
        let payment = &mut years[y].payments[p];
        payment.settled = settled;
        payment.outstanding = payment.amount.abs() - settled;
        payment.status = status(payment, ended, today);
        match payment.status {
            TaxPaymentStatus::Overdue => result.overdue += payment.outstanding,
            TaxPaymentStatus::RefundDue => result.refunds_due += payment.outstanding,
            _ => {}
        }
        if result.next_due.is_none()
            && matches!(
                payment.status,
                TaxPaymentStatus::Overdue | TaxPaymentStatus::Upcoming
            )
            && payment.outstanding > 0
        {
            result.next_due = Some(payment.clone());
        }
    }
    years.reverse();
    result.years = years;
    Ok(result)
}

fn acc_estimate(
    earnings: i64,
    rates: &AccRates,
    work_levy_rate: Option<Decimal>,
    full_time: bool,
    gst_rate: Decimal,
) -> AccEstimate {
    let l = acc::levies(earnings, rates, work_levy_rate, full_time, gst_rate);
    let rate = |r: Decimal| r.normalize().to_string();
    AccEstimate {
        liable_earnings: l.liable_earnings,
        adjustment: match l.adjustment {
            Adjustment::None => AccAdjustment::None,
            Adjustment::Minimum => AccAdjustment::Minimum,
            Adjustment::Maximum => AccAdjustment::Maximum,
        },
        min_liable: rates.min_liable,
        max_liable: rates.max_liable,
        earners_levy_rate: rate(rates.earners_levy),
        earners_levy: l.earners_levy,
        work_levy_rate: work_levy_rate.map(rate),
        work_levy: l.work_levy,
        working_safer_levy_rate: rate(rates.working_safer_levy),
        working_safer_levy: l.working_safer_levy,
        gst: l.gst,
        total: l.total,
    }
}

/// Saves the owner's ACC work levy rate and whether they work full-time.
pub async fn save_acc_settings(
    state: &AppState,
    ctx: &BusinessContext,
    input: AccSettings,
) -> AppResult<AccSettings> {
    let mut errors = FieldErrors::new();
    if let Some(rate) = input.work_levy_rate {
        errors.check(
            rate >= Decimal::ZERO && rate < Decimal::ONE_HUNDRED && rate.normalize().scale() <= 4,
            "work_levy_rate",
            "Enter the rate in dollars per $100, e.g. 0.69, or leave it empty.",
        );
    }
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let (before_rate, before_full_time) = repo::acc_settings(&mut tx).await?;
    sqlx::query(
        "UPDATE business_settings SET acc_work_levy_rate = $2, acc_full_time = $3 WHERE business_id = $1",
    )
    .bind(ctx.business_id)
    .bind(input.work_levy_rate)
    .bind(input.full_time)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "settings.acc_updated",
            entity_type: "business",
            entity_id: Some(ctx.business_id),
            summary: "Updated the ACC levy estimate settings".into(),
            changes: json!({
                "before": { "work_levy_rate": before_rate, "full_time": before_full_time },
                "after": input,
            }),
        },
    )
    .await?;
    let (work_levy_rate, full_time) = repo::acc_settings(&mut tx).await?;
    tx.commit().await?;
    Ok(AccSettings {
        work_levy_rate,
        full_time,
    })
}

fn new_payment(
    kind: TaxPaymentKind,
    label: String,
    due_date: NaiveDate,
    amount: i64,
) -> TaxPayment {
    TaxPayment {
        kind,
        label,
        due_date,
        amount,
        settled: 0,
        outstanding: amount.abs(),
        status: TaxPaymentStatus::Upcoming,
    }
}

fn status(p: &TaxPayment, year_ended: bool, today: NaiveDate) -> TaxPaymentStatus {
    if p.kind == TaxPaymentKind::Terminal && !year_ended {
        return TaxPaymentStatus::InProgress;
    }
    if p.amount < 0 {
        return if p.outstanding == 0 {
            TaxPaymentStatus::Refunded
        } else {
            TaxPaymentStatus::RefundDue
        };
    }
    if p.outstanding == 0 {
        TaxPaymentStatus::Paid
    } else if p.due_date < today {
        TaxPaymentStatus::Overdue
    } else {
        TaxPaymentStatus::Upcoming
    }
}
