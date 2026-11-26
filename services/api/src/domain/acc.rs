//! ACC levies for a self-employed person: the earners' levy, the work levy and the Working Safer
//! levy on the year's liable earnings, plus GST, which ACC adds to self-employed invoices. Pure and
//! unit-tested; the rates are data (`acc_levy_rates`, and the owner's work levy rate), never
//! constants.
//!
//! ACC (acc.co.nz "Calculating your levies", checked 26 November 2026): levies are worked out from
//! the earnings declared in the IR3 return, at dollars per $100 of liable earnings; someone working
//! full-time (over 30 hours a week on average) who earns less than the minimum is levied on the
//! minimum; earnings above the maximum aren't levied.

use rust_decimal::Decimal;

use super::money::percentage_of;

/// One ACC levy year's rates (dollars per $100, excl. GST) and liable earnings limits (cents).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccRates {
    pub earners_levy: Decimal,
    pub working_safer_levy: Decimal,
    pub min_liable: i64,
    pub max_liable: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Adjustment {
    /// Earnings as recorded.
    None,
    /// Full-time and under the minimum: levied on the minimum.
    Minimum,
    /// Over the maximum: levied on the maximum.
    Maximum,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccLevies {
    /// The earnings the levies are on (cents, whole dollars), after the minimum and maximum.
    pub liable_earnings: i64,
    pub adjustment: Adjustment,
    pub earners_levy: i64,
    /// `None` until the owner enters their work levy rate.
    pub work_levy: Option<i64>,
    pub working_safer_levy: i64,
    /// GST on the levies at `gst_rate`.
    pub gst: i64,
    /// Levies plus GST.
    pub total: i64,
}

/// Levies on `earnings` (cents; the year's net self-employed income). `work_levy_rate` is dollars per
/// $100 for the owner's classification unit; `gst_rate` a fraction (0.15).
pub fn levies(
    earnings: i64,
    rates: &AccRates,
    work_levy_rate: Option<Decimal>,
    full_time: bool,
    gst_rate: Decimal,
) -> AccLevies {
    // Whole dollars, as IRD and ACC work in.
    let earnings = (earnings.max(0) / 100) * 100;
    let (liable_earnings, adjustment) = if earnings > rates.max_liable {
        (rates.max_liable, Adjustment::Maximum)
    } else if full_time && earnings < rates.min_liable {
        (rates.min_liable, Adjustment::Minimum)
    } else {
        (earnings, Adjustment::None)
    };
    let per_100 = |rate: Decimal| percentage_of(liable_earnings, rate).unwrap_or(0);
    let earners_levy = per_100(rates.earners_levy);
    let work_levy = work_levy_rate.map(per_100);
    let working_safer_levy = per_100(rates.working_safer_levy);
    let levies = earners_levy + work_levy.unwrap_or(0) + working_safer_levy;
    let gst = percentage_of(levies, gst_rate * Decimal::ONE_HUNDRED).unwrap_or(0);
    AccLevies {
        liable_earnings,
        adjustment,
        earners_levy,
        work_levy,
        working_safer_levy,
        gst,
        total: levies + gst,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// 2026/27: $1.52 earners' levy, $0.08 Working Safer, $50,501 – $156,641.
    fn rates_2027() -> AccRates {
        AccRates {
            earners_levy: dec!(1.52),
            working_safer_levy: dec!(0.08),
            min_liable: 5_050_100,
            max_liable: 15_664_100,
        }
    }

    #[test]
    fn levies_per_100_dollars_plus_gst() {
        // $90,000 with a $1.76 work levy (a carpenter's rate): 1,368 + 1,584 + 72 = 3,024; GST 453.60.
        let l = levies(9_000_050, &rates_2027(), Some(dec!(1.76)), true, dec!(0.15));
        assert_eq!(l.liable_earnings, 9_000_000, "cents are dropped");
        assert_eq!(l.adjustment, Adjustment::None);
        assert_eq!(l.earners_levy, 136_800);
        assert_eq!(l.work_levy, Some(158_400));
        assert_eq!(l.working_safer_levy, 7_200);
        assert_eq!(l.gst, 45_360);
        assert_eq!(l.total, 347_760);
    }

    #[test]
    fn the_minimum_for_full_time_and_the_maximum() {
        // Full-time on $30,000: levied on the $50,501 minimum.
        let l = levies(3_000_000, &rates_2027(), None, true, dec!(0.15));
        assert_eq!(l.liable_earnings, 5_050_100);
        assert_eq!(l.adjustment, Adjustment::Minimum);
        assert_eq!(l.earners_levy, 76_762, "$50,501 × 1.52%");
        assert_eq!(l.work_levy, None);
        // Part-time: the actual earnings.
        let l = levies(3_000_000, &rates_2027(), None, false, dec!(0.15));
        assert_eq!(l.liable_earnings, 3_000_000);
        assert_eq!(l.adjustment, Adjustment::None);
        // A loss: nothing, unless full-time.
        assert_eq!(
            levies(-500_000, &rates_2027(), None, false, dec!(0.15)).total,
            0
        );
        // Over the maximum: levied on the $156,641 maximum, so the earners' levy is $2,380.94.
        let l = levies(
            20_000_000,
            &rates_2027(),
            Some(dec!(0.10)),
            false,
            dec!(0.15),
        );
        assert_eq!(l.liable_earnings, 15_664_100);
        assert_eq!(l.adjustment, Adjustment::Maximum);
        assert_eq!(l.earners_levy, 238_094);
        assert_eq!(l.work_levy, Some(15_664));
    }
}
