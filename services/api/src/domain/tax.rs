//! GST treatment and arithmetic. Rates come from the versioned `tax_rates` table, never constants.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::money::round_to_minor;

/// How GST applies to a line, an expense or an income record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum TaxTreatment {
    /// The standard rate (15% in New Zealand).
    Standard,
    /// Taxable at 0% (exports, some services to non-residents).
    ZeroRated,
    /// Not a taxable supply (e.g. financial services, residential rent).
    Exempt,
    /// The business isn't GST-registered, or GST doesn't apply.
    NoGst,
}

/// GST position of a business on a given date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GstContext {
    pub registered: bool,
    /// The standard rate in force on the document date (e.g. 0.15).
    pub standard_rate: Decimal,
}

impl GstContext {
    pub const NOT_REGISTERED: Self = Self {
        registered: false,
        standard_rate: Decimal::ZERO,
    };

    /// Registered businesses charge and claim GST from their registration date.
    pub fn for_date(
        registered: bool,
        registered_from: Option<NaiveDate>,
        date: NaiveDate,
        standard_rate: Option<Decimal>,
    ) -> Self {
        let active = registered && registered_from.is_none_or(|from| date >= from);
        match (active, standard_rate) {
            (true, Some(rate)) => Self {
                registered: true,
                standard_rate: rate,
            },
            _ => Self::NOT_REGISTERED,
        }
    }

    /// The treatment that actually applies: without registration nothing carries GST.
    pub fn effective(self, treatment: TaxTreatment) -> TaxTreatment {
        if self.registered {
            treatment
        } else {
            TaxTreatment::NoGst
        }
    }

    pub fn rate_for(self, treatment: TaxTreatment) -> Decimal {
        match self.effective(treatment) {
            TaxTreatment::Standard => self.standard_rate,
            _ => Decimal::ZERO,
        }
    }
}

/// GST on a GST-exclusive amount: `round(net × rate)`.
pub fn gst_on_exclusive(net: i64, rate: Decimal) -> Option<i64> {
    round_to_minor(Decimal::from(net) * rate)
}

/// GST contained in a GST-inclusive amount: `round(gross × rate / (1 + rate))` (3/23 at 15%).
pub fn gst_in_inclusive(gross: i64, rate: Decimal) -> Option<i64> {
    if rate.is_zero() {
        return Some(0);
    }
    round_to_minor(Decimal::from(gross) * rate / (Decimal::ONE + rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn nz_gst_arithmetic() {
        assert_eq!(gst_on_exclusive(10_000, dec!(0.15)), Some(1_500));
        assert_eq!(gst_in_inclusive(11_500, dec!(0.15)), Some(1_500));
        // $10.00 inclusive: 10 × 3/23 = 1.3043… → $1.30.
        assert_eq!(gst_in_inclusive(1_000, dec!(0.15)), Some(130));
        assert_eq!(gst_in_inclusive(1_000, Decimal::ZERO), Some(0));
    }

    #[test]
    fn registration_date_controls_gst() {
        let ctx = |date| GstContext::for_date(true, Some(d(2026, 4, 1)), date, Some(dec!(0.15)));
        assert!(!ctx(d(2026, 3, 31)).registered);
        assert!(ctx(d(2026, 4, 1)).registered);
        assert_eq!(
            ctx(d(2026, 3, 31)).rate_for(TaxTreatment::Standard),
            Decimal::ZERO
        );
        assert_eq!(
            ctx(d(2026, 5, 1)).rate_for(TaxTreatment::Standard),
            dec!(0.15)
        );
        assert_eq!(
            ctx(d(2026, 5, 1)).rate_for(TaxTreatment::ZeroRated),
            Decimal::ZERO
        );
        assert_eq!(
            GstContext::NOT_REGISTERED.effective(TaxTreatment::Standard),
            TaxTreatment::NoGst
        );
    }
}
