//! Depreciation of fixed assets for a financial year (ADR 0013). Rates are the user's own (from
//! their accountant or IRD's rate finder), never constants here. Diminishing value applies the rate
//! to the value at the start of the year; straight line applies it to cost. A year in which the asset
//! was bought counts the months it was owned, including the month of purchase. A low-value asset
//! (IRD: $1,000 or less, excluding GST) can instead be deducted in full in the year it was bought.

use chrono::{Datelike, NaiveDate};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::money::round_to_minor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Diminishing value: the rate times the value at the start of the year.
    Dv,
    /// Straight line: the rate times the cost.
    Sl,
    /// A low-value asset deducted in full in the year it was bought.
    Full,
}

/// IRD's low-value asset threshold: a cost of $1,000 or less, excluding GST (minor units).
pub const LOW_VALUE_LIMIT: i64 = 100_000;

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dv => "dv",
            Self::Sl => "sl",
            Self::Full => "full",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "sl" => Self::Sl,
            "full" => Self::Full,
            _ => Self::Dv,
        }
    }
}

/// One asset's year, in minor units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetYear {
    /// Value at the start of the year (zero when bought during it).
    pub opening: i64,
    /// Cost when bought during the year.
    pub purchases: i64,
    pub depreciation: i64,
    /// The private-use share of the depreciation (not a business expense).
    pub private_amount: i64,
    pub closing: i64,
}

/// Months of the year ending `year_end` (starting the day after `year_start - 1`) the asset was owned,
/// counting the month it was bought; 12 for an asset owned all year, 0 for one bought after it.
pub fn months_owned(acquired_on: NaiveDate, year_start: NaiveDate, year_end: NaiveDate) -> u32 {
    if acquired_on < year_start {
        return 12;
    }
    if acquired_on > year_end {
        return 0;
    }
    let months = (year_end.year() - acquired_on.year()) * 12 + year_end.month() as i32
        - acquired_on.month() as i32
        + 1;
    months.clamp(0, 12) as u32
}

/// The depreciation for one year.
///
/// - `opening`: value at the start of the year, or `None` when the asset was bought during it.
/// - `cost`: cost excluding GST (the base for straight line, and the purchase when bought in the year).
/// - `months`: months owned in the year (see [`months_owned`]).
pub fn year(
    method: Method,
    rate_percent: Decimal,
    cost: i64,
    opening: Option<i64>,
    months: u32,
    private_use_percent: Decimal,
) -> AssetYear {
    let (opening_value, purchases) = match opening {
        Some(value) => (value.max(0), 0),
        None => (0, cost.max(0)),
    };
    let base_value = opening_value + purchases;
    let basis = match method {
        Method::Dv => base_value,
        Method::Sl => cost.max(0),
        Method::Full => base_value,
    };
    let share = Decimal::from(months.min(12)) / Decimal::from(12);
    let raw = if method == Method::Full {
        // The whole cost in the year it was bought, whatever the month.
        base_value
    } else {
        round_to_minor(Decimal::from(basis) * rate_percent / Decimal::ONE_HUNDRED * share)
            .unwrap_or(0)
    };
    let depreciation = raw.clamp(0, base_value);
    let private_amount =
        round_to_minor(Decimal::from(depreciation) * private_use_percent / Decimal::ONE_HUNDRED)
            .unwrap_or(0)
            .clamp(0, depreciation);
    AssetYear {
        opening: opening_value,
        purchases,
        depreciation,
        private_amount,
        closing: base_value - depreciation,
    }
}

/// The outcome of selling or scrapping an asset, in minor units (IR260): the sale (GST-exclusive)
/// against the adjusted tax value at the start of the year of disposal. Each figure is split into the
/// business share and the private-use share, which stays with the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Disposal {
    /// Depreciation recovered: the sale above the book value, up to cost (business share, taxable).
    pub recovered: i64,
    pub recovered_private: i64,
    /// Loss on disposal: the book value not recovered (business share, deductible).
    pub loss: i64,
    pub loss_private: i64,
    /// The sale above cost: a capital gain, not taxable.
    pub capital_gain: i64,
}

/// `amount` split by private use: (business share, private share), the business share rounded.
fn split_private(amount: i64, private_use: Decimal) -> (i64, i64) {
    let business = (Decimal::from(amount) * (Decimal::ONE_HUNDRED - private_use)
        / Decimal::ONE_HUNDRED)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(0);
    (business, amount - business)
}

pub fn disposal(book_value: i64, cost: i64, proceeds: i64, private_use: Decimal) -> Disposal {
    if proceeds >= book_value {
        let recovered = proceeds.min(cost.max(book_value)) - book_value;
        let (recovered, recovered_private) = split_private(recovered, private_use);
        Disposal {
            recovered,
            recovered_private,
            capital_gain: (proceeds - cost.max(book_value)).max(0),
            ..Disposal::default()
        }
    } else {
        let (loss, loss_private) = split_private(book_value - proceeds, private_use);
        Disposal {
            loss,
            loss_private,
            ..Disposal::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn diminishing_value_applies_the_rate_to_the_opening_value() {
        // A tablet at 67% on $989.34 → $662.86 (662.8578), leaving $326.48.
        let tablet = year(Method::Dv, dec!(67), 149_900, Some(98_934), 12, dec!(0));
        assert_eq!(tablet.depreciation, 66_286);
        assert_eq!(tablet.closing, 32_648);
        assert_eq!(tablet.opening, 98_934);
        assert_eq!(tablet.purchases, 0);
        // A camera at 40% on $1,317.00 → $526.80.
        assert_eq!(
            year(Method::Dv, dec!(40), 219_500, Some(131_700), 12, dec!(0)).depreciation,
            52_680
        );
    }

    #[test]
    fn straight_line_applies_the_rate_to_cost_and_never_goes_below_zero() {
        let asset = year(Method::Sl, dec!(25), 400_000, Some(300_000), 12, dec!(0));
        assert_eq!(asset.depreciation, 100_000);
        assert_eq!(asset.closing, 200_000);
        // The last year only takes what's left.
        let last = year(Method::Sl, dec!(25), 400_000, Some(50_000), 12, dec!(0));
        assert_eq!(last.depreciation, 50_000);
        assert_eq!(last.closing, 0);
    }

    #[test]
    fn a_low_value_asset_is_deducted_in_full_in_the_year_bought() {
        // A $900 monitor bought in March, the last month of the year: the whole $900, not 1/12.
        let monitor = year(Method::Full, dec!(100), 90_000, None, 1, dec!(0));
        assert_eq!((monitor.depreciation, monitor.closing), (90_000, 0));
        // 20% private use: $180 of it isn't a business expense.
        let shared = year(Method::Full, dec!(100), 90_000, None, 6, dec!(20));
        assert_eq!(
            (shared.depreciation, shared.private_amount),
            (90_000, 18_000)
        );
        // Nothing left in later years.
        assert_eq!(
            year(Method::Full, dec!(100), 90_000, Some(0), 12, dec!(0)).depreciation,
            0
        );
    }

    #[test]
    fn a_purchase_in_the_year_counts_the_months_owned() {
        // Bought 10 October 2024 in the year to 31 March 2025: October to March is six months.
        let months = months_owned(d(2024, 10, 10), d(2024, 4, 1), d(2025, 3, 31));
        assert_eq!(months, 6);
        let laptop = year(Method::Dv, dec!(50), 240_000, None, months, dec!(0));
        assert_eq!(laptop.purchases, 240_000);
        assert_eq!(laptop.opening, 0);
        assert_eq!(laptop.depreciation, 60_000);
        assert_eq!(laptop.closing, 180_000);
        assert_eq!(
            months_owned(d(2024, 3, 31), d(2024, 4, 1), d(2025, 3, 31)),
            12
        );
        assert_eq!(
            months_owned(d(2025, 3, 1), d(2024, 4, 1), d(2025, 3, 31)),
            1
        );
        assert_eq!(
            months_owned(d(2025, 4, 1), d(2024, 4, 1), d(2025, 3, 31)),
            0
        );
    }

    #[test]
    fn private_use_is_split_out_of_the_depreciation() {
        let phone = year(Method::Dv, dec!(40), 150_000, Some(100_000), 12, dec!(30));
        assert_eq!(phone.depreciation, 40_000);
        assert_eq!(phone.private_amount, 12_000);
        assert_eq!(phone.closing, 60_000);
    }

    #[test]
    fn methods_round_trip_through_their_names() {
        assert_eq!(Method::parse(Method::Sl.as_str()), Method::Sl);
        assert_eq!(Method::parse(Method::Dv.as_str()), Method::Dv);
        assert_eq!(Method::parse("anything else"), Method::Dv);
    }

    #[test]
    fn a_sale_above_book_value_recovers_depreciation_up_to_cost() {
        // Bought for $3,000, book value $1,200, sold for $1,500: $300 recovered.
        assert_eq!(
            disposal(120_000, 300_000, 150_000, Decimal::ZERO),
            Disposal {
                recovered: 30_000,
                ..Disposal::default()
            }
        );
        // Sold for $3,500: $1,800 recovered (up to cost) and $500 capital gain.
        assert_eq!(
            disposal(120_000, 300_000, 350_000, Decimal::ZERO),
            Disposal {
                recovered: 180_000,
                capital_gain: 50_000,
                ..Disposal::default()
            }
        );
    }

    #[test]
    fn a_sale_below_book_value_is_a_loss_with_the_private_share_kept_out() {
        // Book value $1,200, scrapped for nothing, 25% private use: $900 loss, $300 private.
        assert_eq!(
            disposal(120_000, 300_000, 0, Decimal::from(25)),
            Disposal {
                loss: 90_000,
                loss_private: 30_000,
                ..Disposal::default()
            }
        );
        // Exactly the book value: nothing to record.
        assert_eq!(
            disposal(120_000, 300_000, 120_000, Decimal::ZERO),
            Disposal::default()
        );
    }
}
