//! Home office by IRD's square metre rate option (ird.govt.nz/home-office-expenses):
//! claim = (a × b) + (c × d), where a is the year's mortgage interest, rates and rent, b the business
//! area divided by the home's total floor area, c the business area in square metres and d IRD's rate
//! per square metre for the income year. Pure and unit-tested.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// The claim in minor units. `premises_costs` and `rate_per_m2` are minor units; areas in m².
pub fn claim(
    premises_costs: i64,
    business_area: Decimal,
    total_area: Decimal,
    rate_per_m2: i64,
) -> i64 {
    if total_area <= Decimal::ZERO || business_area <= Decimal::ZERO {
        return 0;
    }
    let share = business_area.min(total_area) / total_area;
    let premises = (Decimal::from(premises_costs.max(0)) * share).round();
    let utilities = (business_area * Decimal::from(rate_per_m2)).round();
    (premises + utilities).to_i64().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn irds_square_metre_formula() {
        // 20 m² of a 200 m² home, $10,000 of mortgage interest, rates and rent, 2025-26's $57.30:
        // $10,000 × 10% + 20 × $57.30 = $1,000 + $1,146.
        assert_eq!(claim(1_000_000, dec!(20), dec!(200), 5_730), 214_600);
        // Renting with no premises costs entered: the utilities part only.
        assert_eq!(claim(0, dec!(12.5), dec!(110), 5_560), 69_500);
        assert_eq!(claim(1_000_000, dec!(0), dec!(200), 5_730), 0);
    }
}
