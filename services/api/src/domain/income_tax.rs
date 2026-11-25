//! Income tax for a sole trader: tax on the year's taxable income by IRD's bands, residual income tax
//! (RIT) after tax already withheld, and provisional tax under IRD's standard option. Pure and
//! unit-tested; the bands are data (`income_tax_rates`), never constants.
//!
//! IRD (ird.govt.nz, checked 24 November 2026):
//! - provisional tax applies when last year's RIT was more than $5,000;
//! - the standard option is last year's RIT plus 5% (IRD's example: $10,130 → $10,636), paid in
//!   three instalments on the 28th of the 5th, 9th and 13th months of the income year (28 August,
//!   15 January instead of 28 December, and 7 May instead of 28 April for a 31 March balance date),
//!   or two (28 October, 7 May) for six-monthly GST filers;
//! - terminal tax is due on the 7th of the 11th month after the balance date (7 February), or 7 April
//!   with a tax agent's extension;
//! - due dates on a weekend or public holiday move to the next working day.

use chrono::{Datelike, Months, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::nz_calendar::next_working_day;

/// RIT above which provisional tax is due the next year, in cents ($5,000).
pub const PROVISIONAL_THRESHOLD: i64 = 500_000;

/// One band: income from `from` (cents) up to the next band's `from` is taxed at `rate`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub from: i64,
    pub rate: Decimal,
}

/// The tax on one band.
#[derive(Debug, Clone, PartialEq)]
pub struct BandTax {
    pub from: i64,
    /// Exclusive upper bound; `None` for the top band.
    pub to: Option<i64>,
    pub rate: Decimal,
    /// Taxable income in this band.
    pub income: i64,
    pub tax: i64,
}

/// Tax on `income` (cents; IRD taxes whole dollars, so cents are dropped). Bands must be sorted by
/// `from`, starting at 0.
pub fn tax_on(income: i64, bands: &[Band]) -> (i64, Vec<BandTax>) {
    let income = (income.max(0) / 100) * 100;
    let mut total = Decimal::ZERO;
    let mut rows = Vec::new();
    for (i, band) in bands.iter().enumerate() {
        let to = bands.get(i + 1).map(|b| b.from);
        let top = to.map_or(income, |t| income.min(t));
        let in_band = (top - band.from).max(0);
        let tax = Decimal::from(in_band) * band.rate;
        total += tax;
        rows.push(BandTax {
            from: band.from,
            to,
            rate: band.rate,
            income: in_band,
            tax: tax.round().to_i64().unwrap_or(0),
        });
    }
    (total.round().to_i64().unwrap_or(0), rows)
}

/// Provisional tax for the next year under the standard option: last year's RIT plus 5%, in whole
/// dollars (rounded down, as IRD's example does), when that RIT was over $5,000.
pub fn standard_provisional(last_year_rit: i64) -> Option<i64> {
    if last_year_rit <= PROVISIONAL_THRESHOLD {
        return None;
    }
    let uplifted = Decimal::from(last_year_rit) * Decimal::new(105, 2);
    Some((uplifted.floor().to_i64().unwrap_or(0) / 100) * 100)
}

fn twenty_eighth(year_start: NaiveDate, months_after: u32) -> NaiveDate {
    let month = year_start
        .with_day(1)
        .expect("first of month")
        .checked_add_months(Months::new(months_after))
        .expect("valid month");
    let due = match month.month() {
        12 => NaiveDate::from_ymd_opt(month.year() + 1, 1, 15),
        4 => NaiveDate::from_ymd_opt(month.year(), 5, 7),
        _ => NaiveDate::from_ymd_opt(month.year(), month.month(), 28),
    }
    .expect("valid due date");
    next_working_day(due)
}

/// Provisional tax instalment due dates for the income year starting on `year_start`.
pub fn instalment_dates(year_start: NaiveDate, six_monthly_gst: bool) -> Vec<NaiveDate> {
    let months: &[u32] = if six_monthly_gst {
        &[6, 12]
    } else {
        &[4, 8, 12]
    };
    months
        .iter()
        .map(|&m| twenty_eighth(year_start, m))
        .collect()
}

/// Terminal tax for the income year ending on `year_end`: the 7th of the 11th month after it.
pub fn terminal_tax_due(year_end: NaiveDate) -> NaiveDate {
    let month = year_end
        .with_day(1)
        .expect("first of month")
        .checked_add_months(Months::new(11))
        .expect("valid month");
    next_working_day(NaiveDate::from_ymd_opt(month.year(), month.month(), 7).expect("valid date"))
}

/// `total` split into `n` equal instalments; the last takes any remainder.
pub fn split(total: i64, n: usize) -> Vec<i64> {
    if n == 0 {
        return Vec::new();
    }
    let each = total / n as i64;
    let mut parts = vec![each; n];
    parts[n - 1] = total - each * (n as i64 - 1);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    fn bands_2026() -> Vec<Band> {
        [
            (0, dec!(0.105)),
            (1_560_000, dec!(0.175)),
            (5_350_000, dec!(0.30)),
            (7_810_000, dec!(0.33)),
            (18_000_000, dec!(0.39)),
        ]
        .into_iter()
        .map(|(from, rate)| Band { from, rate })
        .collect()
    }

    #[test]
    fn tax_by_band_from_1_april_2025() {
        // $50,000: 15,600 × 10.5% + 34,400 × 17.5% = 1,638 + 6,020.
        assert_eq!(tax_on(5_000_000, &bands_2026()).0, 765_800);
        // $100,000: 1,638 + 6,632.50 + 7,380 + 7,227.
        let (total, rows) = tax_on(10_000_000, &bands_2026());
        assert_eq!(total, 2_287_750);
        assert_eq!(rows[3].income, 2_190_000);
        assert_eq!(rows[4].income, 0);
        // Cents are dropped: $50,000.99 is taxed as $50,000.
        assert_eq!(tax_on(5_000_099, &bands_2026()).0, 765_800);
        assert_eq!(tax_on(-100, &bands_2026()).0, 0);
    }

    #[test]
    fn the_2024_25_composite_rates() {
        let bands: Vec<Band> = [
            (0, dec!(0.105)),
            (1_400_000, dec!(0.1282)),
            (1_560_000, dec!(0.175)),
            (4_800_000, dec!(0.2164)),
            (5_350_000, dec!(0.30)),
            (7_000_000, dec!(0.3099)),
            (7_810_000, dec!(0.33)),
            (18_000_000, dec!(0.39)),
        ]
        .into_iter()
        .map(|(from, rate)| Band { from, rate })
        .collect();
        // $100,000: 1,470 + 205.12 + 5,670 + 1,190.20 + 4,950 + 2,510.19 + 7,227.
        assert_eq!(tax_on(10_000_000, &bands).0, 2_322_251);
    }

    #[test]
    fn provisional_tax_by_the_standard_option() {
        // IRD: "if your RIT was $10,130, your next year's provisional tax will be $10,636".
        assert_eq!(standard_provisional(1_013_000), Some(1_063_600));
        assert_eq!(standard_provisional(500_000), None, "$5,000 or less: none");
        assert_eq!(standard_provisional(500_100), Some(525_100));
        assert_eq!(split(1_063_600, 3), [354_533, 354_533, 354_534]);
    }

    #[test]
    fn instalment_and_terminal_tax_dates() {
        // The 2026-27 income year (1 April 2026 – 31 March 2027).
        assert_eq!(
            instalment_dates(d(2026, 4, 1), false),
            [d(2026, 8, 28), d(2027, 1, 15), d(2027, 5, 7)]
        );
        // Six-monthly GST filers: 28 October and 7 May.
        assert_eq!(
            instalment_dates(d(2026, 4, 1), true),
            [d(2026, 10, 28), d(2027, 5, 7)]
        );
        // 2025-26: 28 August 2025, 15 January 2026, 7 May 2026.
        assert_eq!(
            instalment_dates(d(2025, 4, 1), false),
            [d(2025, 8, 28), d(2026, 1, 15), d(2026, 5, 7)]
        );
        // Terminal tax for the year to 31 March 2026: 7 February 2027 is a Sunday, and Waitangi Day
        // (Saturday 6th) is observed on Monday 8th, so it is due Tuesday 9 February.
        assert_eq!(terminal_tax_due(d(2026, 3, 31)), d(2027, 2, 9));
        assert_eq!(
            terminal_tax_due(d(2025, 3, 31)),
            d(2026, 2, 9),
            "7 February 2026 is a Saturday"
        );
    }
}
