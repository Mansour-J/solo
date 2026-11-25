//! GST change-of-use adjustments (IR375, "Annual review and adjustments for change in use").
//!
//! An asset costing over $10,000 excluding GST is reviewed at each balance date for a number of
//! adjustment periods set by its cost. Actual use is measured from acquisition: each period's
//! business use weighted by its months. It's compared with the intended use (or the use at the last
//! adjustment); the adjustment is the full GST × the change, unless the change is under 10 percentage
//! points and the adjustment is $1,000 or less.

use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

/// Cost (excluding GST, minor units) up to which no adjustment is needed.
pub const NO_ADJUSTMENT_UP_TO: i64 = 1_000_000;
/// An adjustment of this much or less (minor units) is skipped when the change is under 10 points.
pub const SMALL_ADJUSTMENT: i64 = 100_000;

/// How many adjustment periods an asset is reviewed for: none for $10,000 or less, 2 up to $20,000,
/// 5 up to $500,000 and 10 above it (IR375).
pub fn adjustment_periods(cost: i64) -> u32 {
    match cost {
        c if c <= NO_ADJUSTMENT_UP_TO => 0,
        c if c <= 2_000_000 => 2,
        c if c <= 50_000_000 => 5,
        _ => 10,
    }
}

/// Months from `from` to `to` counting both partial months, e.g. 1 October to 31 March is 6.
pub fn months(from: NaiveDate, to: NaiveDate) -> u32 {
    let index = |d: NaiveDate| d.year() * 12 + d.month() as i32;
    (index(to) - index(from) + 1).max(0) as u32
}

/// One adjustment period and the business use recorded for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub end: NaiveDate,
    /// Months in this period.
    pub months: u32,
    /// Business use during the period (0–100), or None until it's reviewed.
    pub business_use: Option<Decimal>,
}

/// The outcome of reviewing one period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Review {
    pub end: NaiveDate,
    /// Business use from acquisition to the end of this period, to 0.1%.
    pub actual_use: Decimal,
    /// The use it's compared with: intended, or the use at the last adjustment.
    pub previous_use: Decimal,
    /// `actual_use − previous_use`, in percentage points.
    pub change: Decimal,
    /// GST to claim (positive) or pay back (negative), in minor units; 0 when no adjustment is made.
    pub adjustment: i64,
}

fn tenth(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero)
}

/// The business use the next adjustment (or a disposal's final adjustment) is measured against:
/// the intended use, or the actual use at the last adjustment made.
pub fn current_use(reviews: &[Review], intended_use: Decimal) -> Decimal {
    reviews
        .iter()
        .rev()
        .find(|r| r.adjustment != 0)
        .map_or(intended_use, |r| r.actual_use)
}

/// IR375's final adjustment on disposal: a credit of the GST on the sale × the share not yet
/// claimed (1 − the use claimed), in minor units.
pub fn final_adjustment(sale_gst: i64, use_claimed: Decimal) -> i64 {
    (Decimal::from(sale_gst) * (Decimal::ONE_HUNDRED - use_claimed) / Decimal::ONE_HUNDRED)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0)
        .max(0)
}

/// Reviews the periods in order, stopping at the first one without a recorded use. `full_gst` is the
/// GST on the whole cost; `intended_use` the business use estimated when it was bought.
pub fn review(full_gst: i64, intended_use: Decimal, periods: &[Period]) -> Vec<Review> {
    let mut reviews = Vec::new();
    let mut previous = intended_use;
    let mut history: Vec<(u32, Decimal)> = Vec::new();
    for period in periods {
        let Some(used) = period.business_use else {
            break;
        };
        history.push((period.months, used));
        let total: u32 = history.iter().map(|(m, _)| m).sum();
        // As IR375's worked example does: each period's share to 0.1%, then summed.
        let actual_use = if total == 0 {
            used
        } else {
            history
                .iter()
                .map(|(m, u)| tenth(*u * Decimal::from(*m) / Decimal::from(total)))
                .sum()
        };
        let change = actual_use - previous;
        let amount = (Decimal::from(full_gst) * change / Decimal::ONE_HUNDRED)
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_i64()
            .unwrap_or(0);
        let made = change.abs() >= Decimal::TEN || amount.abs() > SMALL_ADJUSTMENT;
        reviews.push(Review {
            end: period.end,
            actual_use,
            previous_use: previous,
            change,
            adjustment: if made { amount } else { 0 },
        });
        if made {
            previous = actual_use;
        }
    }
    reviews
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    fn period(end: NaiveDate, months: u32, used: Decimal) -> Period {
        Period {
            end,
            months,
            business_use: Some(used),
        }
    }

    #[test]
    fn the_number_of_adjustment_periods_follows_the_cost() {
        assert_eq!(adjustment_periods(1_000_000), 0);
        assert_eq!(adjustment_periods(1_000_100), 2);
        assert_eq!(adjustment_periods(2_000_000), 2);
        assert_eq!(adjustment_periods(2_500_000), 5);
        assert_eq!(adjustment_periods(50_000_000), 5);
        assert_eq!(adjustment_periods(50_000_100), 10);
    }

    #[test]
    fn months_count_both_partial_months() {
        assert_eq!(months(d(2025, 10, 1), d(2026, 3, 31)), 6);
        assert_eq!(months(d(2025, 5, 14), d(2026, 3, 31)), 11);
        assert_eq!(months(d(2026, 4, 1), d(2027, 3, 31)), 12);
    }

    #[test]
    fn irds_boat_example() {
        // IR375: a boat bought 1 October for $575,000 including $75,000 GST, intended 100% business;
        // actual use 100%, 80%, 83%, 50% and 90% over five periods.
        let periods = [
            period(d(2026, 3, 31), 6, dec!(100)),
            period(d(2027, 3, 31), 12, dec!(80)),
            period(d(2028, 3, 31), 12, dec!(83)),
            period(d(2029, 3, 31), 12, dec!(50)),
            period(d(2030, 3, 31), 12, dec!(90)),
        ];
        let reviews = review(7_500_000, dec!(100), &periods);
        let figures: Vec<(Decimal, i64)> = reviews
            .iter()
            .map(|r| (r.actual_use, r.adjustment))
            .collect();
        assert_eq!(
            figures,
            vec![
                (dec!(100.0), 0),
                (dec!(86.6), -1_005_000),
                (dec!(85.2), -105_000),
                (dec!(75.2), -750_000),
                (dec!(78.4), 240_000),
            ]
        );
        let total: i64 = reviews.iter().map(|r| r.adjustment).sum();
        assert_eq!(total, -1_620_000, "IRD: $16,200 in all");
        // Period 3 compares with period 2's adjusted use.
        assert_eq!(reviews[2].previous_use, dec!(86.6));
    }

    #[test]
    fn irds_car_examples() {
        // Ben: $3,750 GST, intended 80%, actually 100% in the first period → claim the other $750.
        let ben = review(375_000, dec!(80), &[period(d(2026, 3, 31), 11, dec!(100))]);
        assert_eq!(ben[0].adjustment, 75_000);
        // Miriama: $4,500 GST, intended 50%, actually 59%: under 10 points and $405 → no adjustment.
        let miriama = review(450_000, dec!(50), &[period(d(2026, 3, 31), 12, dec!(59))]);
        assert_eq!(miriama[0].change, dec!(9.0));
        assert_eq!(miriama[0].adjustment, 0);
    }

    #[test]
    fn reviews_stop_at_the_first_period_without_a_use() {
        let periods = [
            period(d(2026, 3, 31), 12, dec!(60)),
            Period {
                end: d(2027, 3, 31),
                months: 12,
                business_use: None,
            },
            period(d(2028, 3, 31), 12, dec!(60)),
        ];
        assert_eq!(review(500_000, dec!(100), &periods).len(), 1);
    }

    #[test]
    fn irds_final_adjustment_on_disposal() {
        // IR375 (Melissa): claimed 70%, sold for $34,500 including $4,500 GST → claim $1,350 more.
        assert_eq!(final_adjustment(450_000, Decimal::from(70)), 135_000);
        assert_eq!(final_adjustment(450_000, Decimal::ONE_HUNDRED), 0);
        let reviews = review(
            7_500_000,
            Decimal::ONE_HUNDRED,
            &[
                period(d(2026, 3, 31), 6, Decimal::ONE_HUNDRED),
                period(d(2027, 3, 31), 12, Decimal::from(80)),
            ],
        );
        assert_eq!(
            current_use(&reviews, Decimal::ONE_HUNDRED).to_string(),
            "86.6"
        );
    }
}
