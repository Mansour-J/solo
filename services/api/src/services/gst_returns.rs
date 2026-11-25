//! GST returns: each filing period with its IRD due date, the GST to pay (or the refund due), what has
//! been paid against it, and what is still owing. Figures come from the journal's GST account on the
//! business's GST basis; payments and refunds are the GST cash movements, settling the oldest first.

use chrono::{Duration, NaiveDate};
use serde::Serialize;
use utoipa::ToSchema;

use crate::context::BusinessContext;
use crate::domain::business::{GstBasis, financial_year};
use crate::domain::gst_returns::{self, GstPeriod, ReturnPeriod};
use crate::error::AppResult;
use crate::repositories::{annual, reports as repo};
use crate::services::businesses::business_today;
use crate::services::invoices::load_business;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GstReturnStatus {
    /// The period hasn't ended: the figures so far.
    InProgress,
    /// Ended; file and pay by the due date.
    ToFile,
    /// Past the due date with GST still to pay.
    Overdue,
    /// Paid in full.
    Paid,
    /// IRD owes a refund that hasn't been recorded as received.
    RefundDue,
    /// The refund was received.
    Refunded,
    /// Nothing to pay or claim (a nil return still has to be filed).
    Nil,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GstReturn {
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// When the return and payment are due (IRD dates; a weekend moves to the Monday).
    pub due_date: NaiveDate,
    /// GST on sales and income.
    pub collected: i64,
    /// GST claimable on expenses.
    pub claimable: i64,
    /// IRD's yearly adjustment for costs only partly deductible (entertainment: 15% of the
    /// non-deductible GST-exclusive amount), in the period the financial year ends in.
    pub adjustments: i64,
    /// `collected − claimable + adjustments`: positive to pay, negative a refund due.
    pub net: i64,
    /// Paid against it (or, for a refund, received).
    pub settled: i64,
    /// Still to pay (or, for a refund, still to receive).
    pub outstanding: i64,
    pub status: GstReturnStatus,
}

/// GST owed (or refundable) before the first period, from the opening balances.
#[derive(Debug, Serialize, ToSchema)]
pub struct BroughtForward {
    pub as_at: NaiveDate,
    /// Positive: owed to IRD; negative: a refund due.
    pub amount: i64,
    pub settled: i64,
    pub outstanding: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GstReturns {
    pub gst_registered: bool,
    pub gst_period: GstPeriod,
    pub gst_period_end_month: i16,
    pub gst_basis: GstBasis,
    pub today: NaiveDate,
    pub brought_forward: Option<BroughtForward>,
    /// Oldest first, ending with the period that contains today.
    pub returns: Vec<GstReturn>,
    /// GST to pay for periods that have ended (including anything overdue).
    pub owing_now: i64,
    /// Of `owing_now`, what is past its due date.
    pub overdue: i64,
    /// Refunds IRD owes for ended periods.
    pub refunds_due: i64,
    /// Payments beyond everything owed so far.
    pub credit: i64,
    /// The next return to file: the oldest ended period still owing, else the current period.
    pub next_due: Option<NextReturn>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NextReturn {
    pub end: NaiveDate,
    pub due_date: NaiveDate,
    /// What to pay for it (so far, for the current period).
    pub amount: i64,
    pub status: GstReturnStatus,
}

pub async fn returns(state: &AppState, ctx: &BusinessContext) -> AppResult<GstReturns> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let end_month = business.gst_period_end_month as u32;
    let mut result = GstReturns {
        gst_registered: business.gst_registered,
        gst_period: business.gst_period,
        gst_period_end_month: business.gst_period_end_month,
        gst_basis: business.gst_basis,
        today,
        brought_forward: None,
        returns: Vec::new(),
        owing_now: 0,
        overdue: 0,
        refunds_due: 0,
        credit: 0,
        next_due: None,
    };
    if !business.gst_registered {
        tx.commit().await?;
        return Ok(result);
    }

    let opening = annual::find_opening(&mut tx).await?;
    let first_entry = repo::first_gst_entry_date(&mut tx).await?;
    // With opening balances, returns start the day after them: GST before then is brought forward
    // (and the opening balances' own journal entry is dated on that day).
    let start = match &opening {
        Some(o) => Some(o.as_at + Duration::days(1)),
        None => first_entry,
    };
    let Some(mut start) = start else {
        tx.commit().await?;
        return Ok(result);
    };
    if let Some(registered) = business.gst_registered_from {
        start = start.max(registered);
    }
    let periods = gst_returns::periods_between(start, today, business.gst_period, end_month);
    let (Some(first), Some(last)) = (periods.first(), periods.last()) else {
        tx.commit().await?;
        return Ok(result);
    };
    // Sales on the invoice basis use the accrual entries; on the payments basis, the cash ones.
    // Hybrid: sales by invoice, purchases by payment.
    let (sales_basis, purchases_basis) = match business.gst_basis {
        GstBasis::Invoice => ("accrual", "accrual"),
        GstBasis::Payments => ("cash", "cash"),
        GstBasis::Hybrid => ("accrual", "cash"),
    };
    let sales = repo::gst_entries(&mut tx, first.start, last.end, sales_basis).await?;
    let purchases = if purchases_basis == sales_basis {
        None
    } else {
        Some(repo::gst_entries(&mut tx, first.start, last.end, purchases_basis).await?)
    };
    let (payments, refunds) = repo::gst_settlements(&mut tx).await?;
    // The yearly adjustment for each financial year that ends within these periods.
    let (fy_month, fy_day) = (
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let mut year_adjustments = Vec::new();
    let (mut fy_start, mut fy_end) = financial_year(first.start, fy_month, fy_day);
    while fy_start <= last.end {
        let gst: i64 = repo::non_deductible_shares(&mut tx, fy_start, fy_end, purchases_basis)
            .await?
            .iter()
            .filter(|s| s.gst)
            .map(|s| s.amount)
            .sum();
        if gst != 0 {
            year_adjustments.push((fy_end, gst));
        }
        fy_start = fy_end + Duration::days(1);
        fy_end = financial_year(fy_start, fy_month, fy_day).1;
    }
    tx.commit().await?;

    let in_period = |p: &ReturnPeriod, date: NaiveDate| date >= p.start && date <= p.end;
    let figures: Vec<(i64, i64, i64)> = periods
        .iter()
        .map(|p| {
            let collected = sales
                .iter()
                .filter(|e| in_period(p, e.entry_date))
                .map(|e| e.collected)
                .sum();
            let claimable = purchases
                .as_ref()
                .unwrap_or(&sales)
                .iter()
                .filter(|e| in_period(p, e.entry_date))
                .map(|e| e.claimable)
                .sum();
            let adjustments = year_adjustments
                .iter()
                .filter(|(end, _)| in_period(p, *end))
                .map(|(_, a)| a)
                .sum();
            (collected, claimable, adjustments)
        })
        .collect();

    let brought = opening
        .as_ref()
        .filter(|o| o.gst_payable != 0 && o.as_at < first.start)
        .map(|o| (o.as_at, o.gst_payable));
    let amounts: Vec<i64> = brought
        .iter()
        .map(|(_, a)| *a)
        .chain(figures.iter().map(|(c, cl, adj)| c - cl + adj))
        .collect();
    let settlement = gst_returns::settle(&amounts, payments, refunds);
    let mut settled = settlement.settled.iter().copied();
    if let Some((as_at, amount)) = brought {
        let s = settled.next().unwrap_or_default();
        result.brought_forward = Some(BroughtForward {
            as_at,
            amount,
            settled: s,
            outstanding: amount.abs() - s,
        });
        if amount > 0 {
            result.owing_now += amount - s;
            result.overdue += amount - s;
        } else {
            result.refunds_due += amount.abs() - s;
        }
    }
    for (p, ((collected, claimable, adjustments), s)) in
        periods.iter().zip(figures.into_iter().zip(settled))
    {
        let net = collected - claimable + adjustments;
        let outstanding = net.abs() - s;
        let due = gst_returns::due_date(p.end);
        let ended = p.end < today;
        let status = if !ended {
            GstReturnStatus::InProgress
        } else if net == 0 {
            GstReturnStatus::Nil
        } else if net < 0 {
            if outstanding == 0 {
                GstReturnStatus::Refunded
            } else {
                GstReturnStatus::RefundDue
            }
        } else if outstanding == 0 {
            GstReturnStatus::Paid
        } else if due < today {
            GstReturnStatus::Overdue
        } else {
            GstReturnStatus::ToFile
        };
        match status {
            GstReturnStatus::ToFile => result.owing_now += outstanding,
            GstReturnStatus::Overdue => {
                result.owing_now += outstanding;
                result.overdue += outstanding;
            }
            GstReturnStatus::RefundDue => result.refunds_due += outstanding,
            _ => {}
        }
        result.returns.push(GstReturn {
            start: p.start,
            end: p.end,
            due_date: due,
            collected,
            claimable,
            adjustments,
            net,
            settled: s,
            outstanding,
            status,
        });
    }
    result.credit = settlement.unapplied_payments;
    result.next_due = result
        .returns
        .iter()
        .find(|r| matches!(r.status, GstReturnStatus::Overdue | GstReturnStatus::ToFile))
        .or_else(|| result.returns.last())
        .map(|r| NextReturn {
            end: r.end,
            due_date: r.due_date,
            amount: if r.net > 0 { r.outstanding } else { r.net },
            status: r.status,
        });
    Ok(result)
}

// ─── The return's boxes (SL-802) ────────────────────────────────────────────

/// The figures for IRD's GST return (GST101A) for one period, in minor units, on the business's
/// GST basis. Box 15 is the period's GST to pay (positive) or refund (negative) on the GST page.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct GstReturnBoxes {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub due_date: NaiveDate,
    pub gst_basis: GstBasis,
    /// Box 5: total sales and income, including GST and zero-rated supplies.
    pub box5_sales: i64,
    /// Box 6: zero-rated supplies.
    pub box6_zero_rated: i64,
    /// Box 7: box 5 less box 6.
    pub box7: i64,
    /// Box 8: GST on sales and income.
    pub box8_gst: i64,
    /// Box 9: debit adjustments (entertainment, GST paid back for private use of assets).
    pub box9_debit_adjustments: i64,
    /// Box 10: total GST collected (8 + 9).
    pub box10: i64,
    /// Box 11: purchases and expenses including GST, with GST claimed.
    pub box11_purchases: i64,
    /// Box 12: GST on purchases and expenses.
    pub box12_gst: i64,
    /// Box 13: credit adjustments (bad debts written off, change-of-use and disposal credits).
    pub box13_credit_adjustments: i64,
    /// Box 14: total GST credit (12 + 13).
    pub box14: i64,
    /// Box 15: GST to pay (positive) or refund (negative): box 10 less box 14.
    pub box15: i64,
}

/// The boxes for the return of the period ending on `end`.
pub async fn boxes(
    state: &AppState,
    ctx: &BusinessContext,
    end: NaiveDate,
) -> AppResult<GstReturnBoxes> {
    use crate::error::AppError;
    let all = returns(state, ctx).await?;
    let period = all
        .returns
        .iter()
        .find(|r| r.end == end)
        .ok_or(AppError::NotFound("GST return"))?
        .clone();
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (sales_basis, purchases_basis) = match business.gst_basis {
        GstBasis::Invoice => ("accrual", "accrual"),
        GstBasis::Payments => ("cash", "cash"),
        GstBasis::Hybrid => ("accrual", "cash"),
    };
    let sales = repo::gst_box_entries(&mut tx, period.start, period.end, sales_basis).await?;
    let purchases = if purchases_basis == sales_basis {
        sales.clone()
    } else {
        repo::gst_box_entries(&mut tx, period.start, period.end, purchases_basis).await?
    };
    let (zero_rated, exempt) =
        repo::untaxed_sales(&mut tx, period.start, period.end, sales_basis == "cash").await?;
    tx.commit().await?;

    let (mut box5, mut box8, mut debit, mut credit) = (0, 0, 0, 0);
    for e in &sales {
        match e.source_type.as_str() {
            "invoice" | "invoice_payment" if e.write_off => credit += -e.gst,
            "invoice" | "invoice_payment" => {
                box5 += e.income + e.gst;
                box8 += e.gst;
            }
            "income" if !e.exempt_income => {
                box5 += e.income + e.gst;
                box8 += e.gst;
            }
            "asset_disposal" => {
                let sale_gst = e.sale_gst.unwrap_or(e.gst.max(0));
                box5 += e.sale_price.unwrap_or(0);
                box8 += sale_gst;
                // IR375's final change-of-use adjustment on the sale is a credit.
                credit += sale_gst - e.gst;
            }
            _ => {}
        }
    }
    // Exempt invoice lines are in the invoices' income but not a GST supply.
    box5 -= exempt
        - sales
            .iter()
            .filter(|e| e.source_type == "income" && e.exempt_income)
            .map(|e| e.income)
            .sum::<i64>()
            .min(exempt);
    let (mut box11, mut box12) = (0, 0);
    for e in &purchases {
        match e.source_type.as_str() {
            "expense" | "fixed_asset" if e.gst != 0 => {
                box11 += e.purchases - e.gst;
                box12 += -e.gst;
            }
            "asset_use_review" if e.gst < 0 => credit += -e.gst,
            "asset_use_review" => debit += e.gst,
            _ => {}
        }
    }
    // The yearly entertainment adjustment is a debit adjustment (period.adjustments).
    debit += period.adjustments;
    let box10 = box8 + debit;
    let box14 = box12 + credit;
    Ok(GstReturnBoxes {
        start: period.start,
        end: period.end,
        due_date: period.due_date,
        gst_basis: business.gst_basis,
        box5_sales: box5,
        box6_zero_rated: zero_rated,
        box7: box5 - zero_rated,
        box8_gst: box8,
        box9_debit_adjustments: debit,
        box10,
        box11_purchases: box11,
        box12_gst: box12,
        box13_credit_adjustments: credit,
        box14,
        box15: box10 - box14,
    })
}
