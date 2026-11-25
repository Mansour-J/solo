//! New Zealand's national public holidays, for IRD due dates: "If the due date … falls on a weekend
//! or public holiday, it will be due the next working day" (IR375). Regional anniversary days aren't
//! national holidays and aren't included.
//!
//! - New Year's Day and the day after, Christmas and Boxing Day: when one falls on a weekend, it is
//!   observed on the next weekday that isn't already a holiday (Mondayisation).
//! - Waitangi Day and Anzac Day: observed on the Monday when they fall on a weekend.
//! - Good Friday and Easter Monday; the King's Birthday (first Monday in June); Labour Day (fourth
//!   Monday in October).
//! - Matariki: the dates in the Te Kāhui o Matariki Public Holiday Act 2022 that are confirmed here
//!   (2022–2035 and 2049–2052).

use chrono::{Datelike, Duration, NaiveDate, Weekday};

const MATARIKI: &[(i32, u32, u32)] = &[
    (2022, 6, 24),
    (2023, 7, 14),
    (2024, 6, 28),
    (2025, 6, 20),
    (2026, 7, 10),
    (2027, 6, 25),
    (2028, 7, 14),
    (2029, 7, 6),
    (2030, 6, 21),
    (2031, 7, 11),
    (2032, 7, 2),
    (2033, 6, 24),
    (2034, 7, 7),
    (2035, 6, 29),
    (2049, 6, 25),
    (2050, 7, 15),
    (2051, 6, 30),
    (2052, 6, 21),
];

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
}

fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Easter Sunday (the Anonymous Gregorian algorithm).
fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    date(year, month as u32, day as u32)
}

/// The `n`th (1-based) `weekday` of a month.
fn nth_weekday(year: i32, month: u32, weekday: Weekday, n: u32) -> NaiveDate {
    let first = date(year, month, 1);
    let offset = (7 + weekday.num_days_from_monday() - first.weekday().num_days_from_monday()) % 7;
    first + Duration::days(i64::from(offset + 7 * (n - 1)))
}

/// Two consecutive holidays (25–26 Dec, 1–2 Jan): each is observed on the next weekday not
/// already taken.
fn pair(first: NaiveDate) -> [NaiveDate; 2] {
    let mut days = Vec::with_capacity(2);
    let mut d = first;
    while days.len() < 2 {
        if !is_weekend(d) {
            days.push(d);
        }
        d += Duration::days(1);
    }
    [days[0], days[1]]
}

/// A holiday observed on the Monday when it falls on a weekend.
fn mondayised(d: NaiveDate) -> NaiveDate {
    match d.weekday() {
        Weekday::Sat => d + Duration::days(2),
        Weekday::Sun => d + Duration::days(1),
        _ => d,
    }
}

/// The national public holidays observed in `year` (on the days they are observed).
pub fn public_holidays(year: i32) -> Vec<NaiveDate> {
    let easter = easter_sunday(year);
    let mut days = Vec::new();
    days.extend(pair(date(year, 1, 1)));
    days.push(mondayised(date(year, 2, 6)));
    days.push(easter - Duration::days(2));
    days.push(easter + Duration::days(1));
    days.push(mondayised(date(year, 4, 25)));
    days.push(nth_weekday(year, 6, Weekday::Mon, 1));
    days.extend(
        MATARIKI
            .iter()
            .filter(|(y, _, _)| *y == year)
            .map(|&(y, m, d)| date(y, m, d)),
    );
    days.push(nth_weekday(year, 10, Weekday::Mon, 4));
    days.extend(pair(date(year, 12, 25)));
    days.sort();
    days.dedup();
    days
}

pub fn is_working_day(d: NaiveDate) -> bool {
    !is_weekend(d) && !public_holidays(d.year()).contains(&d)
}

/// `d` if it is a working day, else the next one.
pub fn next_working_day(d: NaiveDate) -> NaiveDate {
    let mut day = d;
    while !is_working_day(day) {
        day += Duration::days(1);
    }
    day
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easter_and_the_moving_holidays() {
        assert_eq!(easter_sunday(2024), date(2024, 3, 31));
        assert_eq!(easter_sunday(2026), date(2026, 4, 5));
        let h2026 = public_holidays(2026);
        for d in [
            date(2026, 4, 3),   // Good Friday
            date(2026, 4, 6),   // Easter Monday
            date(2026, 6, 1),   // King's Birthday
            date(2026, 7, 10),  // Matariki
            date(2026, 10, 26), // Labour Day
        ] {
            assert!(h2026.contains(&d), "{d}");
        }
        assert!(
            public_holidays(2024).contains(&date(2024, 10, 28)),
            "Labour Day 2024"
        );
    }

    #[test]
    fn weekend_holidays_are_mondayised() {
        // 2026: Anzac Day is a Saturday, observed Monday 27 April.
        assert!(public_holidays(2026).contains(&date(2026, 4, 27)));
        // 2021: Christmas on Saturday, Boxing Day on Sunday: observed 27 and 28 December.
        let h = public_holidays(2021);
        assert!(h.contains(&date(2021, 12, 27)) && h.contains(&date(2021, 12, 28)));
        // 2022: New Year's Day on Saturday, 2 January on Sunday: observed 3 and 4 January.
        let h = public_holidays(2022);
        assert!(h.contains(&date(2022, 1, 3)) && h.contains(&date(2022, 1, 4)));
    }

    #[test]
    fn the_next_working_day_skips_weekends_and_holidays() {
        // Matariki on Friday 28 June 2024, then the weekend.
        assert_eq!(next_working_day(date(2024, 6, 28)), date(2024, 7, 1));
        // Labour Day, Monday 28 October 2024.
        assert_eq!(next_working_day(date(2024, 10, 28)), date(2024, 10, 29));
        assert_eq!(next_working_day(date(2026, 8, 28)), date(2026, 8, 28));
    }
}
