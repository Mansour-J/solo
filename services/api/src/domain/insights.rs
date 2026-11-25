//! Deterministic insight rules (ADR 0012): comparisons, shares, averages, unusual increases and
//! recurring commitments, each with an explicit threshold for "enough data". A rule returns `None`
//! when the data can't support it, and the insight isn't shown. No figure here is authoritative
//! bookkeeping: insights read report figures and say what they mean.

use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

use super::money::round_to_minor;

/// Smallest increase (minor units) worth calling out, so cents don't produce "up 200%".
pub const MIN_NOTABLE_CHANGE: i64 = 5_000;
/// A category is "growing" when the last three months are at least this much higher (percent).
pub const GROWTH_THRESHOLD_PERCENT: i64 = 20;
/// A month is "unusual" for a category at this multiple of its usual monthly spend…
pub const UNUSUAL_MULTIPLE: Decimal = Decimal::from_parts(15, 0, 0, false, 1);
/// …and at least this much above it (minor units).
pub const UNUSUAL_MIN_EXCESS: i64 = 10_000;
/// Months of history an unusual-spend check needs (with spending).
pub const UNUSUAL_MIN_HISTORY: usize = 3;
/// Recurring commitments: amounts within this many percent of their average, every month.
pub const RECURRING_TOLERANCE_PERCENT: i64 = 10;
/// Fewest records for an average (invoice value, payment time) to mean anything.
pub const MIN_SAMPLE: usize = 3;
/// Revenue concentration is worth a warning when one customer brings in at least this share.
pub const CONCENTRATION_WARNING_PERCENT: i64 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Positive,
    Negative,
    Neutral,
    Warning,
}

/// How a figure's amount is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FigureKind {
    /// Minor units.
    Money,
    /// Whole percent (may be negative).
    Percent,
    Days,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct Figure {
    pub kind: FigureKind,
    pub amount: i64,
}

impl Figure {
    pub fn money(amount: i64) -> Self {
        Self {
            kind: FigureKind::Money,
            amount,
        }
    }
    pub fn percent(amount: i64) -> Self {
        Self {
            kind: FigureKind::Percent,
            amount,
        }
    }
    pub fn days(amount: i64) -> Self {
        Self {
            kind: FigureKind::Days,
            amount,
        }
    }
    pub fn count(amount: i64) -> Self {
        Self {
            kind: FigureKind::Count,
            amount,
        }
    }
}

/// `round(part × 100 / whole)` in whole percent; `None` when `whole` isn't positive.
/// IRD's GST registration threshold: turnover over $60,000 in the last 12 months (cents).
pub const GST_REGISTRATION_THRESHOLD: i64 = 6_000_000;

/// Whether a business not registered for GST has passed IRD's registration threshold with its
/// sales over the last 12 months (registration is then due within 21 days).
pub fn must_register_for_gst(registered: bool, sales_last_12_months: i64) -> bool {
    !registered && sales_last_12_months > GST_REGISTRATION_THRESHOLD
}

pub fn percent_of(part: i64, whole: i64) -> Option<i64> {
    if whole <= 0 {
        return None;
    }
    round_to_minor(Decimal::from(part) * Decimal::ONE_HUNDRED / Decimal::from(whole))
}

/// Change from `previous` to `current` in whole percent; `None` unless both are positive (a change
/// from nothing isn't a percentage).
pub fn percent_change(current: i64, previous: i64) -> Option<i64> {
    if current <= 0 || previous <= 0 {
        return None;
    }
    percent_of(current - previous, previous)
}

/// The average of `values`, rounded to a whole unit; `None` for no values.
pub fn average(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let total: i64 = values.iter().sum();
    round_to_minor(Decimal::from(total) / Decimal::from(values.len()))
}

/// Whether a category grew notably: up at least [`GROWTH_THRESHOLD_PERCENT`] and
/// [`MIN_NOTABLE_CHANGE`] on a previous amount that wasn't zero. Returns the change in percent.
pub fn notable_growth(current: i64, previous: i64) -> Option<i64> {
    let change = percent_change(current, previous)?;
    (change >= GROWTH_THRESHOLD_PERCENT && current - previous >= MIN_NOTABLE_CHANGE)
        .then_some(change)
}

/// Whether `current` is unusually high against `history` (earlier months, oldest first): at least
/// [`UNUSUAL_MULTIPLE`] times the average of the months with spending and [`UNUSUAL_MIN_EXCESS`]
/// above it, with at least [`UNUSUAL_MIN_HISTORY`] such months. Returns the usual (average) amount.
pub fn unusual_spend(current: i64, history: &[i64]) -> Option<i64> {
    let spent: Vec<i64> = history.iter().copied().filter(|v| *v > 0).collect();
    if spent.len() < UNUSUAL_MIN_HISTORY {
        return None;
    }
    let usual = average(&spent)?;
    let unusual = Decimal::from(current) >= Decimal::from(usual) * UNUSUAL_MULTIPLE
        && current - usual >= UNUSUAL_MIN_EXCESS;
    unusual.then_some(usual)
}

/// A recurring commitment: something paid in every one of `months` (at least three), each within
/// [`RECURRING_TOLERANCE_PERCENT`] of their average. Returns the typical monthly amount.
pub fn recurring(months: &[i64]) -> Option<i64> {
    if months.len() < 3 || months.iter().any(|v| *v <= 0) {
        return None;
    }
    let typical = average(months)?;
    let within = |v: i64| (v - typical).abs() * 100 <= typical * RECURRING_TOLERANCE_PERCENT;
    months.iter().all(|v| within(*v)).then_some(typical)
}

/// Direction of a series over its last `window` values against the `window` before them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Rising,
    Falling,
    Steady,
}

/// Compares the sum of the last `window` values with the `window` before them; `None` without
/// `2 × window` values. Changes under [`MIN_NOTABLE_CHANGE`] are steady.
pub fn trend(series: &[i64], window: usize) -> Option<Trend> {
    if window == 0 || series.len() < window * 2 {
        return None;
    }
    let recent: i64 = series[series.len() - window..].iter().sum();
    let before: i64 = series[series.len() - 2 * window..series.len() - window]
        .iter()
        .sum();
    Some(if recent - before >= MIN_NOTABLE_CHANGE {
        Trend::Rising
    } else if before - recent >= MIN_NOTABLE_CHANGE {
        Trend::Falling
    } else {
        Trend::Steady
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn gst_registration_threshold() {
        // IRD: register when turnover was over $60,000 in the last 12 months.
        assert!(must_register_for_gst(false, 6_000_001));
        assert!(!must_register_for_gst(false, 6_000_000));
        assert!(!must_register_for_gst(true, 50_000_000));
    }

    use super::*;

    #[test]
    fn figures_carry_their_kind() {
        assert_eq!(Figure::money(5).kind, FigureKind::Money);
        assert_eq!(Figure::percent(5).kind, FigureKind::Percent);
        assert_eq!(Figure::days(5).kind, FigureKind::Days);
        let count = Figure::count(7);
        assert_eq!((count.kind, count.amount), (FigureKind::Count, 7));
    }

    #[test]
    fn percentages_need_a_positive_base() {
        assert_eq!(percent_of(1, 3), Some(33));
        assert_eq!(percent_of(2, 3), Some(67));
        assert_eq!(percent_of(5, 0), None);
        assert_eq!(percent_change(11_500, 10_000), Some(15));
        assert_eq!(percent_change(8_000, 10_000), Some(-20));
        assert_eq!(percent_change(10_000, 0), None, "no change from nothing");
        assert_eq!(
            percent_change(0, 10_000),
            None,
            "nothing this time isn't a trend"
        );
    }

    #[test]
    fn averages_round_half_away_from_zero() {
        assert_eq!(average(&[100, 101]), Some(101));
        assert_eq!(average(&[1, 2, 2]), Some(2));
        assert_eq!(average(&[]), None);
    }

    #[test]
    fn growth_must_be_both_relatively_and_absolutely_notable() {
        // $300 → $400: +33% and +$100.
        assert_eq!(notable_growth(40_000, 30_000), Some(33));
        // $10 → $30: +200% but only $20 more.
        assert_eq!(notable_growth(3_000, 1_000), None);
        // $1,000 → $1,100: +$100 but only +10%.
        assert_eq!(notable_growth(110_000, 10_0000), None);
        assert_eq!(notable_growth(50_000, 0), None);
    }

    #[test]
    fn unusual_spending_needs_history_and_a_clear_jump() {
        // Usually $100 a month; $250 is 2.5× and $150 more.
        assert_eq!(
            unusual_spend(25_000, &[10_000, 10_000, 10_000]),
            Some(10_000)
        );
        // $140 is only 1.4×.
        assert_eq!(unusual_spend(14_000, &[10_000, 10_000, 10_000]), None);
        // Usually $20: $60 is 3× but only $40 more.
        assert_eq!(unusual_spend(6_000, &[2_000, 2_000, 2_000]), None);
        // Two months of history isn't enough; months without spending don't count.
        assert_eq!(unusual_spend(50_000, &[0, 10_000, 10_000]), None);
    }

    #[test]
    fn recurring_commitments_are_regular_and_similar() {
        assert_eq!(recurring(&[4_999, 4_999, 5_200]), Some(5_066));
        assert_eq!(recurring(&[5_000, 0, 5_000]), None, "missed a month");
        assert_eq!(recurring(&[5_000, 8_000, 5_000]), None, "too different");
        assert_eq!(
            recurring(&[5_000, 5_000]),
            None,
            "two months isn't a pattern"
        );
    }

    #[test]
    fn trends_compare_recent_months_with_the_ones_before() {
        let rising = [10_000, 10_000, 10_000, 20_000, 20_000, 20_000];
        assert_eq!(trend(&rising, 3), Some(Trend::Rising));
        let falling: Vec<i64> = rising.iter().rev().copied().collect();
        assert_eq!(trend(&falling, 3), Some(Trend::Falling));
        assert_eq!(
            trend(&[10_000, 10_001, 10_000, 10_002], 2),
            Some(Trend::Steady)
        );
        assert_eq!(trend(&[10_000, 20_000], 3), None);
    }
}
