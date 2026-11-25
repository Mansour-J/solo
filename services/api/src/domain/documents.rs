//! Invoice and quote calculations: line amounts, discounts, GST and totals.
//!
//! Rules (docs/architecture/financial-model.md):
//! * a line's amount is `round(quantity × unit price)`, less `round(amount × discount %)`;
//! * GST is calculated per line on the rounded line amount, then lines are summed, so every printed
//!   line adds up to the printed totals;
//! * prices are GST-exclusive by default; with `prices_include_gst` the line amount contains GST.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::money::round_to_minor;
use super::tax::{GstContext, TaxTreatment, gst_in_inclusive, gst_on_exclusive};

pub const MAX_LINES: usize = 200;
/// Largest unit price accepted: $10 billion in cents.
pub const MAX_UNIT_PRICE: i64 = 1_000_000_000_000;
pub const MAX_QUANTITY: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LineInput {
    pub description: String,
    #[schema(value_type = String, example = "1.5")]
    pub quantity: Decimal,
    /// Minor units, GST-exclusive unless the document's prices include GST.
    pub unit_price: i64,
    #[serde(default)]
    #[schema(value_type = String, example = "0")]
    pub discount_percent: Decimal,
    pub tax_treatment: TaxTreatment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CalculatedLine {
    pub description: String,
    #[schema(value_type = String)]
    pub quantity: Decimal,
    pub unit_price: i64,
    #[schema(value_type = String)]
    pub discount_percent: Decimal,
    /// The treatment applied (no GST when the business isn't registered on the document date).
    pub tax_treatment: TaxTreatment,
    #[schema(value_type = String)]
    pub tax_rate: Decimal,
    pub discount_amount: i64,
    pub net_amount: i64,
    pub tax_amount: i64,
    pub total_amount: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, ToSchema)]
pub struct Totals {
    /// Sum of line amounts excluding GST, after discounts.
    pub subtotal: i64,
    pub discount_total: i64,
    pub gst_total: i64,
    pub total: i64,
}

/// A field-level problem with a line, e.g. `("lines.2.quantity", "…")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    pub field: String,
    pub message: &'static str,
}

fn line_error(index: usize, field: &str, message: &'static str) -> LineError {
    LineError {
        field: format!("lines.{index}.{field}"),
        message,
    }
}

/// Validates and calculates every line and the document totals.
pub fn calculate(
    lines: &[LineInput],
    prices_include_gst: bool,
    gst: GstContext,
) -> Result<(Vec<CalculatedLine>, Totals), Vec<LineError>> {
    let mut errors = Vec::new();
    if lines.is_empty() {
        errors.push(LineError {
            field: "lines".into(),
            message: "Add at least one line.",
        });
    }
    if lines.len() > MAX_LINES {
        errors.push(LineError {
            field: "lines".into(),
            message: "Use 200 lines or fewer.",
        });
    }
    for (i, line) in lines.iter().enumerate() {
        let description = line.description.trim();
        if description.is_empty() {
            errors.push(line_error(i, "description", "Describe the item."));
        } else if description.chars().count() > 1000 {
            errors.push(line_error(
                i,
                "description",
                "Use 1000 characters or fewer.",
            ));
        }
        if line.quantity <= Decimal::ZERO
            || line.quantity > MAX_QUANTITY
            || line.quantity.scale() > 4
        {
            errors.push(line_error(
                i,
                "quantity",
                "Enter a quantity above 0 with up to 4 decimal places.",
            ));
        }
        if !(0..=MAX_UNIT_PRICE).contains(&line.unit_price) {
            errors.push(line_error(i, "unit_price", "Enter a price of 0 or more."));
        }
        if line.discount_percent < Decimal::ZERO
            || line.discount_percent > Decimal::ONE_HUNDRED
            || line.discount_percent.scale() > 2
        {
            errors.push(line_error(
                i,
                "discount_percent",
                "Use a discount between 0 and 100%.",
            ));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut calculated = Vec::with_capacity(lines.len());
    let mut totals = Totals::default();
    for (i, line) in lines.iter().enumerate() {
        let overflow = || vec![line_error(i, "unit_price", "This amount is too large.")];
        let base =
            round_to_minor(line.quantity * Decimal::from(line.unit_price)).ok_or_else(overflow)?;
        let discount =
            round_to_minor(Decimal::from(base) * line.discount_percent / Decimal::ONE_HUNDRED)
                .ok_or_else(overflow)?;
        let amount = base - discount;
        let treatment = gst.effective(line.tax_treatment);
        let rate = gst.rate_for(line.tax_treatment);
        let (net, tax) = if prices_include_gst {
            let tax = gst_in_inclusive(amount, rate).ok_or_else(overflow)?;
            (amount - tax, tax)
        } else {
            (amount, gst_on_exclusive(amount, rate).ok_or_else(overflow)?)
        };
        totals.subtotal += net;
        totals.gst_total += tax;
        totals.discount_total += discount;
        calculated.push(CalculatedLine {
            description: line.description.trim().to_owned(),
            quantity: line.quantity.normalize(),
            unit_price: line.unit_price,
            discount_percent: line.discount_percent.normalize(),
            tax_treatment: treatment,
            tax_rate: rate,
            discount_amount: discount,
            net_amount: net,
            tax_amount: tax,
            total_amount: net + tax,
        });
    }
    totals.total = totals.subtotal + totals.gst_total;
    Ok((calculated, totals))
}

/// What users see: the stored lifecycle status combined with payments and the due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvoiceDisplayStatus {
    Draft,
    Issued,
    Sent,
    PartiallyPaid,
    Paid,
    /// Settled by writing off what was left as a bad debt.
    WrittenOff,
    Overdue,
    Cancelled,
}

pub fn invoice_display_status(
    status: &str,
    total: i64,
    paid: i64,
    written_off: i64,
    due_date: NaiveDate,
    today: NaiveDate,
    sent: bool,
) -> InvoiceDisplayStatus {
    match status {
        "draft" => InvoiceDisplayStatus::Draft,
        "cancelled" => InvoiceDisplayStatus::Cancelled,
        _ if paid >= total && written_off > 0 => InvoiceDisplayStatus::WrittenOff,
        _ if paid >= total => InvoiceDisplayStatus::Paid,
        _ if due_date < today => InvoiceDisplayStatus::Overdue,
        _ if paid > 0 => InvoiceDisplayStatus::PartiallyPaid,
        _ if sent => InvoiceDisplayStatus::Sent,
        _ => InvoiceDisplayStatus::Issued,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteDisplayStatus {
    Draft,
    Sent,
    Accepted,
    Rejected,
    Expired,
    Converted,
}

pub fn quote_display_status(
    status: &str,
    expiry_date: NaiveDate,
    today: NaiveDate,
    converted: bool,
) -> QuoteDisplayStatus {
    match status {
        _ if converted => QuoteDisplayStatus::Converted,
        "accepted" => QuoteDisplayStatus::Accepted,
        "rejected" => QuoteDisplayStatus::Rejected,
        _ if expiry_date < today => QuoteDisplayStatus::Expired,
        "sent" => QuoteDisplayStatus::Sent,
        _ => QuoteDisplayStatus::Draft,
    }
}

/// `INV-0042` from a prefix, a sequence number and the zero-padding width.
pub fn document_number(prefix: &str, number: i32, padding: i16) -> String {
    format!(
        "{prefix}{number:0width$}",
        width = padding.clamp(1, 10) as usize
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    const GST: GstContext = GstContext {
        registered: true,
        standard_rate: dec!(0.15),
    };

    fn line(
        quantity: Decimal,
        unit_price: i64,
        discount: Decimal,
        treatment: TaxTreatment,
    ) -> LineInput {
        LineInput {
            description: "Work".into(),
            quantity,
            unit_price,
            discount_percent: discount,
            tax_treatment: treatment,
        }
    }

    #[test]
    fn too_many_lines_and_long_descriptions_are_refused() {
        let many = vec![line(dec!(1), 100, dec!(0), TaxTreatment::Standard); 201];
        let errors = calculate(&many, false, GST).expect_err("too many");
        assert!(
            errors
                .iter()
                .any(|e| e.field == "lines" && e.message.contains("200"))
        );
        let mut long = line(dec!(1), 100, dec!(0), TaxTreatment::Standard);
        long.description = "x".repeat(1001);
        let errors = calculate(&[long], false, GST).expect_err("too long");
        assert!(
            errors[0].field.contains("description"),
            "{}",
            errors[0].field
        );
    }

    #[test]
    fn quote_statuses_follow_the_decision_before_the_expiry() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 1).expect("date");
        let past = NaiveDate::from_ymd_opt(2026, 4, 1).expect("date");
        assert_eq!(
            quote_display_status("accepted", past, today, false),
            QuoteDisplayStatus::Accepted
        );
        assert_eq!(
            quote_display_status("rejected", past, today, false),
            QuoteDisplayStatus::Rejected
        );
        assert_eq!(
            quote_display_status("sent", past, today, false),
            QuoteDisplayStatus::Expired
        );
        assert_eq!(
            quote_display_status("sent", today, today, false),
            QuoteDisplayStatus::Sent
        );
        assert_eq!(
            quote_display_status("accepted", past, today, true),
            QuoteDisplayStatus::Converted
        );
    }

    #[test]
    fn exclusive_prices_add_gst_per_line() {
        let (lines, totals) = calculate(
            &[
                line(dec!(7.5), 12_000, dec!(0), TaxTreatment::Standard),
                line(dec!(1), 4_999, dec!(0), TaxTreatment::Standard),
            ],
            false,
            GST,
        )
        .expect("valid");
        assert_eq!(lines[0].net_amount, 90_000);
        assert_eq!(lines[0].tax_amount, 13_500);
        // 49.99 × 0.15 = 7.4985 → 7.50
        assert_eq!(lines[1].tax_amount, 750);
        assert_eq!(
            totals,
            Totals {
                subtotal: 94_999,
                discount_total: 0,
                gst_total: 14_250,
                total: 109_249
            }
        );
    }

    #[test]
    fn inclusive_prices_extract_gst() {
        let (lines, totals) = calculate(
            &[line(dec!(1), 11_500, dec!(0), TaxTreatment::Standard)],
            true,
            GST,
        )
        .expect("valid");
        assert_eq!(
            (
                lines[0].net_amount,
                lines[0].tax_amount,
                lines[0].total_amount
            ),
            (10_000, 1_500, 11_500)
        );
        assert_eq!(totals.total, 11_500);
    }

    #[test]
    fn discounts_apply_before_gst() {
        let (lines, totals) = calculate(
            &[line(dec!(3), 3_333, dec!(10), TaxTreatment::Standard)],
            false,
            GST,
        )
        .expect("valid");
        // 3 × 33.33 = 99.99; 10% = 9.999 → 10.00; 89.99 + 13.4985 → 13.50
        assert_eq!(lines[0].discount_amount, 1_000);
        assert_eq!(lines[0].net_amount, 8_999);
        assert_eq!(lines[0].tax_amount, 1_350);
        assert_eq!(totals.discount_total, 1_000);
    }

    #[test]
    fn zero_rated_exempt_and_unregistered_carry_no_gst() {
        let (lines, _) = calculate(
            &[
                line(dec!(1), 1_000, dec!(0), TaxTreatment::ZeroRated),
                line(dec!(1), 1_000, dec!(0), TaxTreatment::Exempt),
            ],
            false,
            GST,
        )
        .expect("valid");
        assert!(lines.iter().all(|l| l.tax_amount == 0));
        let (lines, totals) = calculate(
            &[line(dec!(1), 1_000, dec!(0), TaxTreatment::Standard)],
            false,
            GstContext::NOT_REGISTERED,
        )
        .expect("valid");
        assert_eq!(lines[0].tax_treatment, TaxTreatment::NoGst);
        assert_eq!(totals.gst_total, 0);
    }

    #[test]
    fn totals_always_equal_the_sum_of_lines() {
        let lines: Vec<LineInput> = (1..=40)
            .map(|i| {
                line(
                    Decimal::new(i * 37, 2),
                    1_000 + i * 113,
                    Decimal::new(i % 7, 0),
                    TaxTreatment::Standard,
                )
            })
            .collect();
        for inclusive in [false, true] {
            let (calculated, totals) = calculate(&lines, inclusive, GST).expect("valid");
            assert_eq!(
                totals.subtotal,
                calculated.iter().map(|l| l.net_amount).sum::<i64>()
            );
            assert_eq!(
                totals.gst_total,
                calculated.iter().map(|l| l.tax_amount).sum::<i64>()
            );
            assert_eq!(
                totals.total,
                calculated.iter().map(|l| l.total_amount).sum::<i64>()
            );
        }
    }

    #[test]
    fn invalid_lines_report_their_fields() {
        let errors = calculate(
            &[line(dec!(0), -5, dec!(120), TaxTreatment::Standard)],
            false,
            GST,
        )
        .expect_err("invalid");
        let fields: Vec<_> = errors.iter().map(|e| e.field.as_str()).collect();
        assert_eq!(
            fields,
            [
                "lines.0.quantity",
                "lines.0.unit_price",
                "lines.0.discount_percent"
            ]
        );
        assert!(calculate(&[], false, GST).is_err());
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).expect("date")
    }

    #[test]
    fn invoice_statuses() {
        let today = d(2026, 6, 1);
        let s = |status, paid, due, sent| {
            invoice_display_status(status, 10_000, paid, 0, due, today, sent)
        };
        assert_eq!(s("draft", 0, today, false), InvoiceDisplayStatus::Draft);
        assert_eq!(
            s("cancelled", 0, today, false),
            InvoiceDisplayStatus::Cancelled
        );
        assert_eq!(
            s("issued", 10_000, d(2026, 1, 1), false),
            InvoiceDisplayStatus::Paid
        );
        assert_eq!(
            s("issued", 2_000, d(2026, 5, 31), true),
            InvoiceDisplayStatus::Overdue
        );
        assert_eq!(
            s("issued", 2_000, today, true),
            InvoiceDisplayStatus::PartiallyPaid
        );
        assert_eq!(s("issued", 0, today, true), InvoiceDisplayStatus::Sent);
        assert_eq!(s("issued", 0, today, false), InvoiceDisplayStatus::Issued);
    }

    #[test]
    fn quote_statuses_and_numbers() {
        let today = d(2026, 6, 1);
        assert_eq!(
            quote_display_status("sent", d(2026, 5, 1), today, false),
            QuoteDisplayStatus::Expired
        );
        assert_eq!(
            quote_display_status("accepted", d(2026, 5, 1), today, false),
            QuoteDisplayStatus::Accepted
        );
        assert_eq!(
            quote_display_status("accepted", today, today, true),
            QuoteDisplayStatus::Converted
        );
        assert_eq!(document_number("INV-", 42, 4), "INV-0042");
        assert_eq!(document_number("", 12_345, 2), "12345");
    }
}
