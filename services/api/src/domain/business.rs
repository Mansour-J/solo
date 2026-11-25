//! Business profile types and calendar rules (financial years, periods).

use chrono::{Datelike, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum BusinessType {
    SoleTrader,
    Contractor,
    Freelancer,
    Partnership,
    Company,
    Other,
}

/// How GST is accounted for with IRD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum GstBasis {
    Invoice,
    Payments,
    Hybrid,
}

/// Cash basis recognises income and expenses when money moves; accrual when invoiced or incurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ReportingBasis {
    Cash,
    Accrual,
}

/// A postal address. Every field is optional so users can complete it later.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct Address {
    pub line1: String,
    pub line2: String,
    pub suburb: String,
    pub city: String,
    pub region: String,
    pub postcode: String,
    /// ISO 3166-1 alpha-2, e.g. `NZ`.
    pub country: String,
}

impl Address {
    pub const MAX_FIELD: usize = 120;

    pub fn trimmed(self) -> Self {
        Self {
            line1: self.line1.trim().to_owned(),
            line2: self.line2.trim().to_owned(),
            suburb: self.suburb.trim().to_owned(),
            city: self.city.trim().to_owned(),
            region: self.region.trim().to_owned(),
            postcode: self.postcode.trim().to_owned(),
            country: self.country.trim().to_uppercase(),
        }
    }

    pub fn fields(&self) -> [(&'static str, &str); 7] {
        [
            ("line1", &self.line1),
            ("line2", &self.line2),
            ("suburb", &self.suburb),
            ("city", &self.city),
            ("region", &self.region),
            ("postcode", &self.postcode),
            ("country", &self.country),
        ]
    }

    /// Non-empty lines for printing on documents.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for part in [&self.line1, &self.line2, &self.suburb] {
            if !part.is_empty() {
                lines.push(part.clone());
            }
        }
        let city_line = [self.city.as_str(), self.postcode.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if !city_line.is_empty() {
            lines.push(city_line);
        }
        if !self.region.is_empty() {
            lines.push(self.region.clone());
        }
        lines
    }
}

/// The financial year containing `date`, as an inclusive `(start, end)` pair.
/// A year starting on 1 April: 15 May 2026 → (1 April 2026, 31 March 2027).
pub fn financial_year(date: NaiveDate, start_month: u32, start_day: u32) -> (NaiveDate, NaiveDate) {
    let start_this_year =
        NaiveDate::from_ymd_opt(date.year(), start_month, start_day).unwrap_or(date);
    let start = if date >= start_this_year {
        start_this_year
    } else {
        NaiveDate::from_ymd_opt(date.year() - 1, start_month, start_day).unwrap_or(date)
    };
    let next_start = start.checked_add_months(Months::new(12)).unwrap_or(start);
    (start, next_start.pred_opt().unwrap_or(next_start))
}

/// Calendar month containing `date`, inclusive.
pub fn month_bounds(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = date.with_day(1).unwrap_or(date);
    let next = start.checked_add_months(Months::new(1)).unwrap_or(start);
    (start, next.pred_opt().unwrap_or(next))
}

/// The label users see for a financial year, named by the year it ends in (NZ convention: "FY2027").
pub fn financial_year_label(end: NaiveDate) -> String {
    format!("FY{}", end.year())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("valid date")
    }

    #[test]
    fn nz_financial_year_runs_april_to_march() {
        assert_eq!(
            financial_year(d(2026, 5, 15), 4, 1),
            (d(2026, 4, 1), d(2027, 3, 31))
        );
        assert_eq!(
            financial_year(d(2026, 3, 31), 4, 1),
            (d(2025, 4, 1), d(2026, 3, 31))
        );
        assert_eq!(
            financial_year(d(2026, 4, 1), 4, 1),
            (d(2026, 4, 1), d(2027, 3, 31))
        );
        assert_eq!(financial_year_label(d(2027, 3, 31)), "FY2027");
    }

    #[test]
    fn calendar_and_custom_years() {
        assert_eq!(
            financial_year(d(2026, 12, 31), 1, 1),
            (d(2026, 1, 1), d(2026, 12, 31))
        );
        assert_eq!(
            financial_year(d(2026, 7, 10), 7, 15),
            (d(2025, 7, 15), d(2026, 7, 14))
        );
    }

    #[test]
    fn month_bounds_handle_leap_years() {
        assert_eq!(
            month_bounds(d(2028, 2, 10)),
            (d(2028, 2, 1), d(2028, 2, 29))
        );
        assert_eq!(
            month_bounds(d(2026, 12, 5)),
            (d(2026, 12, 1), d(2026, 12, 31))
        );
    }

    #[test]
    fn address_lines_skip_blanks() {
        let address = Address {
            line1: "1 Queen St".into(),
            city: "Auckland".into(),
            postcode: "1010".into(),
            ..Default::default()
        };
        assert_eq!(
            address.lines(),
            vec!["1 Queen St".to_owned(), "Auckland 1010".to_owned()]
        );
    }
}
