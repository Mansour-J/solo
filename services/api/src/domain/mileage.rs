//! Mileage claims: distance × business share × rate per kilometre (docs/architecture/financial-model.md).
//! Rates are user data with effective dates; nothing here knows a government rate.

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};

use super::money::Currency;

/// Largest distance accepted for one trip, in kilometres.
pub const MAX_TRIP_KM: Decimal = Decimal::from_parts(10_000, 0, 0, false, 0);
/// Largest odometer reading accepted, in kilometres.
pub const MAX_ODOMETER_KM: Decimal = Decimal::from_parts(9_999_999, 0, 0, false, 0);
/// Largest rate per kilometre accepted.
pub const MAX_RATE_PER_KM: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

/// Rounds kilometres to one decimal place, half away from zero.
pub fn round_km(km: Decimal) -> Decimal {
    km.round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero)
}

/// Kilometres with exactly one decimal place, for consistent output (PostgreSQL returns a numeric
/// zero without its scale).
pub fn km_display(km: Decimal) -> Decimal {
    let mut km = round_km(km);
    km.rescale(1);
    km
}

/// How the distance of a trip was entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceInput {
    Odometer { start: Decimal, end: Decimal },
    Distance(Decimal),
}

/// Why a trip's distance can't be worked out; each maps to the field to correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceError {
    /// Only one of the two odometer readings was entered.
    MissingOdometer { start_missing: bool },
    /// The end reading isn't after the start reading.
    OdometerOrder,
    /// A reading is negative or implausibly large.
    OdometerRange { start: bool },
    /// Neither odometer readings nor a distance.
    MissingDistance,
    /// The distance is zero, negative or more than [`MAX_TRIP_KM`].
    DistanceRange,
    /// A distance was entered as well as odometer readings, and they disagree.
    DistanceMismatch,
}

/// The distance of a trip, from odometer readings when both are given, or else the distance entered.
/// Readings and distance are rounded to 0.1 km.
pub fn trip_distance(
    start_odometer: Option<Decimal>,
    end_odometer: Option<Decimal>,
    distance: Option<Decimal>,
) -> Result<(DistanceInput, Decimal), DistanceError> {
    let in_range = |km: Decimal| km >= Decimal::ZERO && km <= MAX_ODOMETER_KM;
    match (start_odometer.map(round_km), end_odometer.map(round_km)) {
        (Some(start), Some(end)) => {
            if !in_range(start) {
                return Err(DistanceError::OdometerRange { start: true });
            }
            if !in_range(end) {
                return Err(DistanceError::OdometerRange { start: false });
            }
            if end <= start {
                return Err(DistanceError::OdometerOrder);
            }
            let km = end - start;
            if km > MAX_TRIP_KM {
                return Err(DistanceError::DistanceRange);
            }
            if distance.is_some_and(|d| round_km(d) != km) {
                return Err(DistanceError::DistanceMismatch);
            }
            Ok((DistanceInput::Odometer { start, end }, km))
        }
        (Some(_), None) => Err(DistanceError::MissingOdometer {
            start_missing: false,
        }),
        (None, Some(_)) => Err(DistanceError::MissingOdometer {
            start_missing: true,
        }),
        (None, None) => {
            let km = round_km(distance.ok_or(DistanceError::MissingDistance)?);
            if km <= Decimal::ZERO || km > MAX_TRIP_KM {
                return Err(DistanceError::DistanceRange);
            }
            Ok((DistanceInput::Distance(km), km))
        }
    }
}

/// The business part of a distance: `distance × percentage / 100`, rounded to 0.1 km.
pub fn business_distance(distance: Decimal, business_percentage: Decimal) -> Decimal {
    round_km(distance * business_percentage / Decimal::ONE_HUNDRED).min(distance)
}

/// The claim for a business distance at a rate per kilometre, in minor units.
pub fn claim_amount(business_km: Decimal, rate_per_km: Decimal, currency: Currency) -> Option<i64> {
    currency.to_minor(business_km * rate_per_km)
}

/// IRD's two tiers: the rate per kilometre applies to the first `tier1_limit_km` a vehicle travels in
/// the financial year (business and private), and `tier2_rate_per_km` to business kilometres after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tiers {
    pub tier1_limit_km: Decimal,
    pub tier2_rate_per_km: Decimal,
}

/// A dated version of a mileage rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateVersion {
    pub effective_from: NaiveDate,
    pub rate_per_km: Decimal,
    pub tiers: Option<Tiers>,
}

/// Of a trip's business kilometres, those at the tier 2 rate: the share of the trip past the tier 1
/// limit (counting `travelled_before`, the vehicle's earlier travel in the year), in the trip's
/// business proportion, rounded to 0.1 km.
pub fn tier2_business_km(
    distance: Decimal,
    business_km: Decimal,
    travelled_before: Decimal,
    tier1_limit_km: Decimal,
) -> Decimal {
    if distance <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let past = (travelled_before + distance - tier1_limit_km)
        .max(Decimal::ZERO)
        .min(distance);
    round_km(past * business_km / distance).min(business_km)
}

/// The claim with tier 1 kilometres at `rate_per_km` and `tier2_km` at the tier 2 rate.
pub fn tiered_claim(
    business_km: Decimal,
    tier2_km: Decimal,
    rate_per_km: Decimal,
    tier2_rate_per_km: Decimal,
    currency: Currency,
) -> Option<i64> {
    currency.to_minor((business_km - tier2_km) * rate_per_km + tier2_km * tier2_rate_per_km)
}

/// The version in force on `date`: the latest one effective on or before it.
pub fn rate_on(versions: &[RateVersion], date: NaiveDate) -> Option<RateVersion> {
    versions
        .iter()
        .filter(|v| v.effective_from <= date)
        .max_by_key(|v| v.effective_from)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn nzd() -> Currency {
        Currency::from_code("NZD").expect("NZD")
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn distance_comes_from_the_odometer_when_both_readings_are_given() {
        assert_eq!(
            trip_distance(Some(dec!(10000.0)), Some(dec!(10042.5)), None),
            Ok((
                DistanceInput::Odometer {
                    start: dec!(10000.0),
                    end: dec!(10042.5)
                },
                dec!(42.5)
            ))
        );
        // A matching distance is accepted; a different one is an error.
        assert!(trip_distance(Some(dec!(100)), Some(dec!(150)), Some(dec!(50))).is_ok());
        assert_eq!(
            trip_distance(Some(dec!(100)), Some(dec!(150)), Some(dec!(40))),
            Err(DistanceError::DistanceMismatch)
        );
    }

    #[test]
    fn odometer_readings_must_be_complete_and_increasing() {
        assert_eq!(
            trip_distance(Some(dec!(100)), None, Some(dec!(5))),
            Err(DistanceError::MissingOdometer {
                start_missing: false
            })
        );
        assert_eq!(
            trip_distance(None, Some(dec!(100)), None),
            Err(DistanceError::MissingOdometer {
                start_missing: true
            })
        );
        assert_eq!(
            trip_distance(Some(dec!(100)), Some(dec!(100)), None),
            Err(DistanceError::OdometerOrder)
        );
        assert_eq!(
            trip_distance(Some(dec!(-1)), Some(dec!(100)), None),
            Err(DistanceError::OdometerRange { start: true })
        );
        assert_eq!(
            trip_distance(Some(dec!(0)), Some(dec!(20000)), None),
            Err(DistanceError::DistanceRange)
        );
    }

    #[test]
    fn a_distance_alone_is_rounded_to_a_tenth_of_a_kilometre() {
        assert_eq!(
            trip_distance(None, None, Some(dec!(12.35))),
            Ok((DistanceInput::Distance(dec!(12.4)), dec!(12.4)))
        );
        assert_eq!(
            trip_distance(None, None, None),
            Err(DistanceError::MissingDistance)
        );
        assert_eq!(
            trip_distance(None, None, Some(dec!(0.04))),
            Err(DistanceError::DistanceRange)
        );
        assert_eq!(
            trip_distance(None, None, Some(dec!(10000.1))),
            Err(DistanceError::DistanceRange)
        );
    }

    #[test]
    fn kilometres_always_show_one_decimal_place() {
        assert_eq!(km_display(dec!(0)).to_string(), "0.0");
        assert_eq!(km_display(dec!(42.5)).to_string(), "42.5");
        assert_eq!(km_display(dec!(110)).to_string(), "110.0");
    }

    #[test]
    fn claims_use_the_rounded_business_distance() {
        // 33.3 km at 33.33% business = 11.09889 → 11.1 km; 11.1 × $1.04 = $11.544 → $11.54.
        let business = business_distance(dec!(33.3), dec!(33.33));
        assert_eq!(business, dec!(11.1));
        assert_eq!(claim_amount(business, dec!(1.04), nzd()), Some(1_154));
        assert_eq!(business_distance(dec!(42.5), dec!(100)), dec!(42.5));
        assert_eq!(business_distance(dec!(42.5), dec!(0)), dec!(0.0));
        // Half a cent rounds away from zero: 0.5 km × $0.01 = $0.005 → 1 cent.
        assert_eq!(claim_amount(dec!(0.5), dec!(0.01), nzd()), Some(1));
    }

    #[test]
    fn the_rate_in_force_is_the_latest_effective_on_or_before_the_date() {
        let versions = [
            RateVersion {
                effective_from: d(2025, 4, 1),
                rate_per_km: dec!(1.04),
                tiers: None,
            },
            RateVersion {
                effective_from: d(2026, 4, 1),
                rate_per_km: dec!(1.17),
                tiers: None,
            },
        ];
        assert_eq!(rate_on(&versions, d(2025, 3, 31)), None);
        assert_eq!(
            rate_on(&versions, d(2026, 3, 31)).map(|v| v.rate_per_km),
            Some(dec!(1.04))
        );
        assert_eq!(
            rate_on(&versions, d(2026, 4, 1)).map(|v| v.rate_per_km),
            Some(dec!(1.17))
        );
    }

    #[test]
    fn kilometres_past_the_tier_1_limit_take_the_tier_2_rate() {
        // 13,900 km travelled before; a 200 km trip crosses 14,000 km by 100 km.
        let t2 = tier2_business_km(dec!(200), dec!(200), dec!(13900), dec!(14000));
        assert_eq!(t2, dec!(100.0));
        // 100 km × $1.20 + 100 km × $0.37 = $157.00.
        assert_eq!(
            tiered_claim(dec!(200), t2, dec!(1.20), dec!(0.37), nzd()),
            Some(15_700)
        );
        // Half business: 100 business km, 50 of them at tier 2 → 50 × 1.20 + 50 × 0.37 = $78.50.
        let t2 = tier2_business_km(dec!(200), dec!(100), dec!(13900), dec!(14000));
        assert_eq!(t2, dec!(50.0));
        assert_eq!(
            tiered_claim(dec!(100), t2, dec!(1.20), dec!(0.37), nzd()),
            Some(7_850)
        );
        // Before the limit, nothing is at tier 2; well past it, all of it is.
        assert_eq!(
            tier2_business_km(dec!(200), dec!(200), dec!(100), dec!(14000)),
            dec!(0)
        );
        assert_eq!(
            tier2_business_km(dec!(200), dec!(150), dec!(20000), dec!(14000)),
            dec!(150)
        );
        assert_eq!(
            tier2_business_km(dec!(200), dec!(0), dec!(20000), dec!(14000)),
            dec!(0)
        );
    }
}
