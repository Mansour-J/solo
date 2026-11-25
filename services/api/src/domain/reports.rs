//! The shape every report shares, and the date rules reports use. Services fill a [`Report`] from
//! journal queries; one export layer turns it into JSON, CSV, Excel or PDF, so every format shows
//! the same figures.

use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;
use utoipa::ToSchema;

/// How a column's cells are written and read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    /// A string.
    Text,
    /// Minor units of the report's currency (an integer).
    Money,
    /// A decimal as a string, e.g. "42.5".
    Number,
    /// A whole number (an integer).
    Count,
    /// `YYYY-MM-DD`.
    Date,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Column {
    pub label: String,
    pub kind: ColumnKind,
}

impl Column {
    pub fn new(label: &str, kind: ColumnKind) -> Self {
        Self {
            label: label.into(),
            kind,
        }
    }
}

/// A cell: text for `text`, `number` and `date` columns, an integer for `money` and `count`, or
/// empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(untagged)]
pub enum Cell {
    Text(String),
    Integer(i64),
    Empty(()),
}

impl Cell {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn date(value: NaiveDate) -> Self {
        Self::Text(value.format("%Y-%m-%d").to_string())
    }

    pub fn empty() -> Self {
        Self::Empty(())
    }
}

impl From<i64> for Cell {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Section {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    /// A totals row, in the same columns.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totals: Option<Vec<Cell>>,
}

impl Section {
    pub fn new(title: &str, columns: Vec<Column>) -> Self {
        Self {
            title: title.into(),
            description: None,
            columns,
            rows: Vec::new(),
            totals: None,
        }
    }

    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn row(&mut self, cells: Vec<Cell>) {
        self.rows.push(cells);
    }
}

/// Inclusive dates the report covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub struct ReportPeriod {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Report {
    /// The report's identifier, e.g. `profit_and_loss`.
    pub kind: String,
    pub title: String,
    pub business_name: String,
    pub currency: String,
    pub period: Option<ReportPeriod>,
    /// `cash` or `accrual`, for reports read from the journal on one basis.
    pub basis: Option<String>,
    /// Further context lines, e.g. the customer a statement is for.
    pub subtitle: Vec<String>,
    pub sections: Vec<Section>,
    /// Notes printed under the report (what it includes, what it isn't).
    pub notes: Vec<String>,
    pub generated_on: NaiveDate,
}

/// The period of the same length that ends the day before `from`.
pub fn previous_period(from: NaiveDate, to: NaiveDate) -> (NaiveDate, NaiveDate) {
    // Whole months (first to last day) compare with the same number of whole months before.
    let first_of_month = from.day() == 1;
    let last_of_month = (to + Duration::days(1)).day() == 1;
    if first_of_month && last_of_month && to >= from {
        let months = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32 + 1;
        let start = shift_months(from, -months);
        return (start, from - Duration::days(1));
    }
    let days = (to - from).num_days() + 1;
    (from - Duration::days(days), from - Duration::days(1))
}

fn shift_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total = date.year() * 12 + date.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1)
        .unwrap_or(date)
}

/// Ageing buckets for outstanding invoices, by days past the due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    /// Not yet due.
    Current,
    Days1To30,
    Days31To60,
    Days61To90,
    Over90,
}

impl Bucket {
    pub const ALL: [Bucket; 5] = [
        Bucket::Current,
        Bucket::Days1To30,
        Bucket::Days31To60,
        Bucket::Days61To90,
        Bucket::Over90,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "Current",
            Self::Days1To30 => "1–30 days",
            Self::Days31To60 => "31–60 days",
            Self::Days61To90 => "61–90 days",
            Self::Over90 => "Over 90 days",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|b| *b == self).unwrap_or(0)
    }
}

/// The bucket of an invoice due on `due` as at `as_of`.
pub fn ageing_bucket(due: NaiveDate, as_of: NaiveDate) -> Bucket {
    match (as_of - due).num_days() {
        days if days <= 0 => Bucket::Current,
        1..=30 => Bucket::Days1To30,
        31..=60 => Bucket::Days31To60,
        61..=90 => Bucket::Days61To90,
        _ => Bucket::Over90,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_can_be_described_and_buckets_are_labelled_in_order() {
        let section = Section::new("Adjustments", Vec::new()).describe("Non-deductible expenses.");
        assert_eq!(
            section.description.as_deref(),
            Some("Non-deductible expenses.")
        );
        let labels: Vec<&str> = Bucket::ALL.iter().map(|b| b.label()).collect();
        assert_eq!(
            labels,
            [
                "Current",
                "1–30 days",
                "31–60 days",
                "61–90 days",
                "Over 90 days"
            ]
        );
        for (i, bucket) in Bucket::ALL.iter().enumerate() {
            assert_eq!(bucket.index(), i);
        }
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn whole_months_compare_with_the_months_before() {
        // A financial year compares with the previous one.
        assert_eq!(
            previous_period(d(2026, 4, 1), d(2027, 3, 31)),
            (d(2025, 4, 1), d(2026, 3, 31))
        );
        // March compares with February, whatever their lengths.
        assert_eq!(
            previous_period(d(2026, 3, 1), d(2026, 3, 31)),
            (d(2026, 2, 1), d(2026, 2, 28))
        );
        // A quarter across a year end.
        assert_eq!(
            previous_period(d(2026, 1, 1), d(2026, 3, 31)),
            (d(2025, 10, 1), d(2025, 12, 31))
        );
    }

    #[test]
    fn other_ranges_compare_with_the_same_number_of_days_before() {
        assert_eq!(
            previous_period(d(2026, 6, 10), d(2026, 6, 19)),
            (d(2026, 5, 31), d(2026, 6, 9))
        );
        assert_eq!(
            previous_period(d(2026, 6, 10), d(2026, 6, 10)),
            (d(2026, 6, 9), d(2026, 6, 9))
        );
    }

    #[test]
    fn invoices_age_by_days_past_due() {
        let as_of = d(2026, 6, 30);
        assert_eq!(ageing_bucket(d(2026, 7, 15), as_of), Bucket::Current);
        assert_eq!(ageing_bucket(d(2026, 6, 30), as_of), Bucket::Current);
        assert_eq!(ageing_bucket(d(2026, 6, 29), as_of), Bucket::Days1To30);
        assert_eq!(ageing_bucket(d(2026, 5, 31), as_of), Bucket::Days1To30);
        assert_eq!(ageing_bucket(d(2026, 5, 30), as_of), Bucket::Days31To60);
        assert_eq!(ageing_bucket(d(2026, 4, 1), as_of), Bucket::Days61To90);
        assert_eq!(ageing_bucket(d(2026, 3, 31), as_of), Bucket::Over90);
    }

    #[test]
    fn cells_serialise_as_plain_json_values() {
        let row = vec![
            Cell::text("Sales"),
            Cell::from(12_345),
            Cell::date(d(2026, 6, 1)),
            Cell::empty(),
        ];
        assert_eq!(
            serde_json::to_string(&row).expect("json"),
            r#"["Sales",12345,"2026-06-01",null]"#
        );
    }
}
