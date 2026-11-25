//! GST return periods, their IRD due dates, and how GST payments settle them. Pure and unit-tested.
//!
//! IRD rules used here (ird.govt.nz, "GST due dates"):
//! - a return and its payment are due on the 28th of the month after the period ends,
//! - except a period ending in March (due 7 May) or November (due 15 January);
//! - a due date on a weekend or a national public holiday moves to the next working day
//!   (`nz_calendar`).

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// How often the business files GST returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum GstPeriod {
    Monthly,
    TwoMonthly,
    SixMonthly,
}

impl GstPeriod {
    pub fn months(self) -> u32 {
        match self {
            Self::Monthly => 1,
            Self::TwoMonthly => 2,
            Self::SixMonthly => 6,
        }
    }
}

/// One GST return period: first and last day, inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnPeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

fn first_of(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("valid month")
}

fn last_of(year: i32, month: u32) -> NaiveDate {
    let (y, m) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    first_of(y, m) - Duration::days(1)
}

/// The period containing `date`, for returns every `period` ending in months that line up with
/// `end_month` (1–12).
pub fn period_containing(date: NaiveDate, period: GstPeriod, end_month: u32) -> ReturnPeriod {
    let months = period.months();
    let end_month = end_month.clamp(1, 12);
    // Months from the date's month to the period end: the next month that lines up with `end_month`.
    let offset = (end_month as i32 - date.month() as i32).rem_euclid(months as i32) as u32;
    let mut year = date.year();
    let mut month = date.month() + offset;
    if month > 12 {
        month -= 12;
        year += 1;
    }
    let end = last_of(year, month);
    let start_index = year * 12 + month as i32 - 1 - (months as i32 - 1);
    let start = first_of(
        start_index.div_euclid(12),
        start_index.rem_euclid(12) as u32 + 1,
    );
    ReturnPeriod { start, end }
}

/// Every period from the one containing `from` to the one containing `to`, oldest first.
pub fn periods_between(
    from: NaiveDate,
    to: NaiveDate,
    period: GstPeriod,
    end_month: u32,
) -> Vec<ReturnPeriod> {
    let mut periods = Vec::new();
    let mut current = period_containing(from, period, end_month);
    while current.start <= to {
        periods.push(current);
        current = period_containing(current.end + Duration::days(1), period, end_month);
    }
    periods
}

/// When the return and payment for a period ending on `end` are due.
pub fn due_date(end: NaiveDate) -> NaiveDate {
    let due = match end.month() {
        3 => NaiveDate::from_ymd_opt(end.year(), 5, 7),
        11 => NaiveDate::from_ymd_opt(end.year() + 1, 1, 15),
        12 => NaiveDate::from_ymd_opt(end.year() + 1, 1, 28),
        m => NaiveDate::from_ymd_opt(end.year(), m + 1, 28),
    }
    .expect("valid due date");
    super::nz_calendar::next_working_day(due)
}

/// How much of each amount owed was paid, and of each refund due was received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settlement {
    /// Per amount, in the same order: what was paid (for an amount owed) or received (for a refund).
    pub settled: Vec<i64>,
    /// Payments beyond everything owed: a credit with IRD.
    pub unapplied_payments: i64,
    /// Refunds received beyond the refunds due.
    pub unapplied_refunds: i64,
}

/// Settles `amounts` (positive: GST owed for a period; negative: a refund due) oldest first: GST
/// payments pay what is owed, refunds received settle the refunds due.
pub fn settle(amounts: &[i64], payments: i64, refunds: i64) -> Settlement {
    let (mut payments, mut refunds) = (payments.max(0), refunds.max(0));
    let settled = amounts
        .iter()
        .map(|&amount| {
            let pool = if amount >= 0 {
                &mut payments
            } else {
                &mut refunds
            };
            let applied = (*pool).min(amount.abs());
            *pool -= applied;
            applied
        })
        .collect();
    Settlement {
        settled,
        unapplied_payments: payments,
        unapplied_refunds: refunds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn two_monthly_periods_ending_with_march_end_in_odd_months() {
        let p = period_containing(d(2026, 4, 15), GstPeriod::TwoMonthly, 3);
        assert_eq!((p.start, p.end), (d(2026, 4, 1), d(2026, 5, 31)));
        let p = period_containing(d(2026, 1, 10), GstPeriod::TwoMonthly, 3);
        assert_eq!((p.start, p.end), (d(2025, 12, 1), d(2026, 1, 31)));
        let p = period_containing(d(2025, 12, 31), GstPeriod::TwoMonthly, 3);
        assert_eq!((p.start, p.end), (d(2025, 12, 1), d(2026, 1, 31)));
        let p = period_containing(d(2026, 3, 31), GstPeriod::TwoMonthly, 3);
        assert_eq!((p.start, p.end), (d(2026, 2, 1), d(2026, 3, 31)));
    }

    #[test]
    fn two_monthly_periods_can_end_in_even_months() {
        let p = period_containing(d(2026, 4, 15), GstPeriod::TwoMonthly, 2);
        assert_eq!((p.start, p.end), (d(2026, 3, 1), d(2026, 4, 30)));
        let p = period_containing(d(2026, 12, 5), GstPeriod::TwoMonthly, 12);
        assert_eq!((p.start, p.end), (d(2026, 11, 1), d(2026, 12, 31)));
    }

    #[test]
    fn monthly_and_six_monthly_periods() {
        let p = period_containing(d(2026, 2, 14), GstPeriod::Monthly, 3);
        assert_eq!((p.start, p.end), (d(2026, 2, 1), d(2026, 2, 28)));
        let p = period_containing(d(2026, 11, 2), GstPeriod::SixMonthly, 3);
        assert_eq!((p.start, p.end), (d(2026, 10, 1), d(2027, 3, 31)));
        let p = period_containing(d(2026, 4, 1), GstPeriod::SixMonthly, 3);
        assert_eq!((p.start, p.end), (d(2026, 4, 1), d(2026, 9, 30)));
    }

    #[test]
    fn periods_cover_a_range_without_gaps() {
        let ps = periods_between(d(2026, 4, 1), d(2026, 11, 24), GstPeriod::TwoMonthly, 3);
        let ends: Vec<_> = ps.iter().map(|p| p.end).collect();
        assert_eq!(
            ends,
            [
                d(2026, 5, 31),
                d(2026, 7, 31),
                d(2026, 9, 30),
                d(2026, 11, 30)
            ]
        );
        for pair in ps.windows(2) {
            assert_eq!(pair[0].end + Duration::days(1), pair[1].start);
        }
    }

    #[test]
    fn due_dates_follow_ird_and_skip_weekends() {
        // 28 June 2026 is a Sunday.
        assert_eq!(due_date(d(2026, 5, 31)), d(2026, 6, 29));
        assert_eq!(due_date(d(2026, 7, 31)), d(2026, 8, 28));
        assert_eq!(
            due_date(d(2026, 3, 31)),
            d(2026, 5, 7),
            "March returns are due 7 May"
        );
        assert_eq!(
            due_date(d(2026, 11, 30)),
            d(2027, 1, 15),
            "November returns are due 15 January"
        );
        assert_eq!(due_date(d(2026, 12, 31)), d(2027, 1, 28));
        // 7 May 2028 is a Sunday.
        assert_eq!(due_date(d(2028, 3, 31)), d(2028, 5, 8));
    }

    #[test]
    fn due_dates_skip_public_holidays() {
        // Matariki was Friday 28 June 2024: the May return was due Monday 1 July.
        assert_eq!(due_date(d(2024, 5, 31)), d(2024, 7, 1));
        // Labour Day was Monday 28 October 2024: the September return was due 29 October.
        assert_eq!(due_date(d(2024, 9, 30)), d(2024, 10, 29));
    }

    #[test]
    fn irds_own_examples() {
        // IR375 (March 2026): "if you file your GST returns 2-monthly and have a March balance date
        // your taxable periods are April-May, June-July, August-September, October-November,
        // December-January, February-March."
        let periods = periods_between(d(2026, 4, 1), d(2027, 3, 31), GstPeriod::TwoMonthly, 3);
        let months: Vec<_> = periods
            .iter()
            .map(|p| (p.start.month(), p.end.month()))
            .collect();
        assert_eq!(months, [(4, 5), (6, 7), (8, 9), (10, 11), (12, 1), (2, 3)]);
        // ird.govt.nz on 24 Nov 2026: the return for the month ending 31 October 2026 is due
        // 30 November (the 28th is a Saturday).
        assert_eq!(due_date(d(2026, 10, 31)), d(2026, 11, 30));
        // "the due date is usually the 28th of the month following the end of your taxable period"
        // (period ending 31 May: 28 June).
        assert_eq!(due_date(d(2027, 5, 31)), d(2027, 6, 28));
        // Six-monthly returns ending in March and September: 7 May and 28 October.
        let six = periods_between(d(2026, 4, 1), d(2027, 3, 31), GstPeriod::SixMonthly, 3);
        let dues: Vec<_> = six.iter().map(|p| due_date(p.end)).collect();
        assert_eq!(dues, [d(2026, 10, 28), d(2027, 5, 7)]);
    }

    #[test]
    fn payments_settle_the_oldest_amounts_first_and_refunds_settle_refunds() {
        let s = settle(&[1_000, -200, 500, 700], 1_200, 200);
        assert_eq!(s.settled, [1_000, 200, 200, 0]);
        assert_eq!((s.unapplied_payments, s.unapplied_refunds), (0, 0));
        let over = settle(&[100], 150, 0);
        assert_eq!(over.settled, [100]);
        assert_eq!(over.unapplied_payments, 50);
    }
}
