//! Recurring invoice dates: weekly and fortnightly add days; monthly, quarterly and yearly keep to the
//! schedule's day of the month (the 31st becomes each month's last day).

use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Weekly,
    Fortnightly,
    Monthly,
    Quarterly,
    Yearly,
}

impl Frequency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Fortnightly => "fortnightly",
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Yearly => "yearly",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "weekly" => Self::Weekly,
            "fortnightly" => Self::Fortnightly,
            "monthly" => Self::Monthly,
            "quarterly" => Self::Quarterly,
            "yearly" => Self::Yearly,
            _ => return None,
        })
    }
}

/// The date after `date` in a schedule keeping to `anchor_day`.
pub fn next(date: NaiveDate, frequency: Frequency, anchor_day: u32) -> NaiveDate {
    let months = match frequency {
        Frequency::Weekly => return date + Duration::days(7),
        Frequency::Fortnightly => return date + Duration::days(14),
        Frequency::Monthly => 1,
        Frequency::Quarterly => 3,
        Frequency::Yearly => 12,
    };
    let first = date.with_day(1).expect("first of month") + Months::new(months);
    let last = (first + Months::new(1) - Duration::days(1)).day();
    first.with_day(anchor_day.min(last)).expect("valid day")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn monthly_keeps_to_the_day_and_the_31st_becomes_the_last_day() {
        assert_eq!(next(d(2026, 1, 31), Frequency::Monthly, 31), d(2026, 2, 28));
        assert_eq!(next(d(2026, 2, 28), Frequency::Monthly, 31), d(2026, 3, 31));
        assert_eq!(next(d(2026, 4, 1), Frequency::Monthly, 1), d(2026, 5, 1));
        assert_eq!(
            next(d(2026, 11, 30), Frequency::Quarterly, 30),
            d(2027, 2, 28)
        );
        assert_eq!(next(d(2028, 2, 29), Frequency::Yearly, 29), d(2029, 2, 28));
    }

    #[test]
    fn weekly_and_fortnightly_add_days() {
        assert_eq!(next(d(2026, 12, 28), Frequency::Weekly, 28), d(2027, 1, 4));
        assert_eq!(
            next(d(2026, 12, 28), Frequency::Fortnightly, 28),
            d(2027, 1, 11)
        );
        assert_eq!(
            Frequency::parse("fortnightly"),
            Some(Frequency::Fortnightly)
        );
        assert_eq!(Frequency::parse("daily"), None);
    }
}
