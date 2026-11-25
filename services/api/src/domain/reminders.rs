//! Invoice reminders (SL-702): which reminder an unpaid invoice is due for today.
//!
//! A reminder is a day relative to the due date: -3 is three days before, 7 is a week after. Only the
//! latest reminder reached is sent, and only within a week of its day, so turning reminders on (or not
//! opening the app for a while) never sends a burst of old reminders.

use chrono::{Duration, NaiveDate};

/// How long after its day a reminder can still go out.
pub const GRACE_DAYS: i64 = 7;

/// The reminder days in order: the one before the due date (as a negative day), then those after.
pub fn schedule(days_before: Option<i32>, days_after: &[i32]) -> Vec<i32> {
    let mut days: Vec<i32> = days_before
        .filter(|d| *d > 0)
        .map(|d| -d)
        .into_iter()
        .chain(days_after.iter().copied().filter(|d| *d > 0))
        .collect();
    days.sort_unstable();
    days.dedup();
    days
}

/// The reminder due today for an invoice due on `due`, if any: the latest day in `schedule` that has
/// been reached, provided it was reached within the grace period.
pub fn due_reminder(due: NaiveDate, today: NaiveDate, schedule: &[i32]) -> Option<i32> {
    let day = schedule
        .iter()
        .copied()
        .filter(|d| due + Duration::days(i64::from(*d)) <= today)
        .max()?;
    let reached = due + Duration::days(i64::from(day));
    (today - reached < Duration::days(GRACE_DAYS)).then_some(day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().expect("date")
    }

    #[test]
    fn schedule_orders_and_drops_nonsense() {
        assert_eq!(schedule(Some(3), &[14, 1, 7, 7, 0]), vec![-3, 1, 7, 14]);
        assert_eq!(schedule(None, &[7]), vec![7]);
        assert_eq!(schedule(Some(0), &[]), Vec::<i32>::new());
    }

    #[test]
    fn the_latest_reached_reminder_is_due() {
        let s = schedule(Some(3), &[1, 7, 14]);
        let due = d("2026-07-20");
        assert_eq!(due_reminder(due, d("2026-07-16"), &s), None, "too early");
        assert_eq!(due_reminder(due, d("2026-07-17"), &s), Some(-3));
        assert_eq!(
            due_reminder(due, d("2026-07-20"), &s),
            Some(-3),
            "still the before reminder on the day"
        );
        assert_eq!(due_reminder(due, d("2026-07-21"), &s), Some(1));
        assert_eq!(due_reminder(due, d("2026-07-29"), &s), Some(7));
        assert_eq!(due_reminder(due, d("2026-08-03"), &s), Some(14));
    }

    #[test]
    fn old_reminders_are_not_sent_late() {
        let s = schedule(None, &[1]);
        let due = d("2026-07-20");
        assert_eq!(
            due_reminder(due, d("2026-07-27"), &s),
            Some(1),
            "six days late is fine"
        );
        assert_eq!(
            due_reminder(due, d("2026-07-28"), &s),
            None,
            "a week late is not"
        );
    }
}
