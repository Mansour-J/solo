//! Money. Amounts are integers in a currency's minor unit (cents for NZD) and are never floats.
//! Rates, quantities and percentages are `Decimal`; converting a computed decimal back to money always
//! goes through [`round_to_minor`], which rounds half away from zero (the usual commercial rule).

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

/// A currency the product can calculate with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Currency {
    pub code: &'static str,
    /// Digits after the decimal point (ISO 4217 minor unit).
    pub minor_units: u32,
    pub symbol: &'static str,
}

const CURRENCIES: &[Currency] = &[
    Currency {
        code: "NZD",
        minor_units: 2,
        symbol: "$",
    },
    Currency {
        code: "AUD",
        minor_units: 2,
        symbol: "$",
    },
    Currency {
        code: "USD",
        minor_units: 2,
        symbol: "$",
    },
    Currency {
        code: "GBP",
        minor_units: 2,
        symbol: "£",
    },
    Currency {
        code: "EUR",
        minor_units: 2,
        symbol: "€",
    },
    Currency {
        code: "CAD",
        minor_units: 2,
        symbol: "$",
    },
    Currency {
        code: "SGD",
        minor_units: 2,
        symbol: "$",
    },
    Currency {
        code: "JPY",
        minor_units: 0,
        symbol: "¥",
    },
];

impl Currency {
    pub fn from_code(code: &str) -> Option<Self> {
        CURRENCIES
            .iter()
            .copied()
            .find(|c| c.code.eq_ignore_ascii_case(code))
    }

    pub fn all() -> &'static [Currency] {
        CURRENCIES
    }

    /// Converts minor units to a decimal amount (1234 → 12.34 for NZD).
    pub fn to_decimal(self, minor: i64) -> Decimal {
        Decimal::new(minor, self.minor_units)
    }

    /// Converts a decimal amount to minor units, rounding half away from zero.
    pub fn to_minor(self, amount: Decimal) -> Option<i64> {
        round_to_minor(amount * Decimal::from(10_i64.pow(self.minor_units)))
    }

    /// Formats for documents and emails, e.g. `$1,234.50` or `-$5.00`.
    pub fn format(self, minor: i64) -> String {
        let negative = minor < 0;
        let abs = minor.unsigned_abs();
        let divisor = 10_u64.pow(self.minor_units);
        let whole = abs / divisor;
        let fraction = abs % divisor;
        let grouped = group_thousands(whole);
        let body = if self.minor_units == 0 {
            grouped
        } else {
            format!(
                "{grouped}.{fraction:0width$}",
                width = self.minor_units as usize
            )
        };
        format!("{}{}{}", if negative { "-" } else { "" }, self.symbol, body)
    }
}

fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Rounds a decimal number of minor units to an integer, half away from zero (2.5 → 3, −2.5 → −3).
pub fn round_to_minor(minor: Decimal) -> Option<i64> {
    minor
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
}

/// `amount × percentage / 100`, rounded to minor units. Used for business-use and deductible portions.
pub fn percentage_of(amount: i64, percentage: Decimal) -> Option<i64> {
    round_to_minor(Decimal::from(amount) * percentage / Decimal::ONE_HUNDRED)
}

/// Splits `total` into parts proportional to `weights`, so the parts always add up to `total`
/// exactly (largest-remainder method). Used to apportion a payment across invoice lines.
pub fn allocate_proportionally(total: i64, weights: &[i64]) -> Vec<i64> {
    let weight_sum: i128 = weights.iter().map(|w| i128::from(*w)).sum();
    if weights.is_empty() {
        return Vec::new();
    }
    if weight_sum == 0 {
        let mut parts = vec![0; weights.len()];
        parts[0] = total;
        return parts;
    }
    let total = i128::from(total);
    let mut parts: Vec<i128> = weights
        .iter()
        .map(|w| total * i128::from(*w) / weight_sum)
        .collect();
    let mut remainders: Vec<(usize, i128)> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| (i, (total * i128::from(*w) % weight_sum).abs()))
        .collect();
    remainders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut leftover = total - parts.iter().sum::<i128>();
    let step = leftover.signum();
    for (index, _) in remainders {
        if leftover == 0 {
            break;
        }
        parts[index] += step;
        leftover -= step;
    }
    parts.into_iter().map(|p| p as i64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn currency_lookup_and_precision() {
        let nzd = Currency::from_code("nzd").expect("NZD is supported");
        assert_eq!(nzd.minor_units, 2);
        assert_eq!(Currency::from_code("JPY").map(|c| c.minor_units), Some(0));
        assert!(Currency::from_code("XXX").is_none());
    }

    #[test]
    fn decimal_conversion_rounds_half_away_from_zero() {
        let nzd = Currency::from_code("NZD").expect("supported");
        assert_eq!(nzd.to_minor(dec!(12.345)), Some(1235));
        assert_eq!(nzd.to_minor(dec!(-12.345)), Some(-1235));
        assert_eq!(nzd.to_minor(dec!(0.004)), Some(0));
        assert_eq!(nzd.to_decimal(1234), dec!(12.34));
    }

    #[test]
    fn formatting() {
        let nzd = Currency::from_code("NZD").expect("supported");
        assert_eq!(nzd.format(123_456_789), "$1,234,567.89");
        assert_eq!(nzd.format(-500), "-$5.00");
        assert_eq!(nzd.format(7), "$0.07");
        assert_eq!(
            Currency::from_code("JPY").expect("supported").format(1500),
            "¥1,500"
        );
    }

    #[test]
    fn percentages() {
        assert_eq!(percentage_of(10_000, dec!(50)), Some(5_000));
        assert_eq!(percentage_of(333, dec!(33.33)), Some(111));
        assert_eq!(percentage_of(1_005, dec!(50)), Some(503), "502.5 rounds up");
    }

    #[test]
    fn proportional_allocation_always_sums_to_total() {
        assert_eq!(allocate_proportionally(100, &[1, 1, 1]), vec![34, 33, 33]);
        assert_eq!(
            allocate_proportionally(-100, &[1, 1, 1]),
            vec![-34, -33, -33]
        );
        assert_eq!(allocate_proportionally(1_000, &[0, 0]), vec![1_000, 0]);
        let parts = allocate_proportionally(12_345, &[8_000, 1_200, 3_145]);
        assert_eq!(parts.iter().sum::<i64>(), 12_345);
        assert!(allocate_proportionally(5, &[]).is_empty());
    }
}
