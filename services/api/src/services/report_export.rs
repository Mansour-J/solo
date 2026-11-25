//! One export layer for every report: a [`Report`] becomes CSV, Excel (`.xlsx`) or a branded PDF
//! (Typst, `assets/templates/report.typ`). The formats show the same rows and totals as the JSON.

use std::sync::LazyLock;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{ExcelDateTime, Format, FormatAlign, Workbook};
use serde::{Deserialize, Serialize};
use typst::foundations::{Dict, Value};
use typst_as_lib::{TypstEngine, TypstTemplateMainFile};
use typst_layout::PagedDocument;
use utoipa::ToSchema;

use crate::domain::money::Currency;
use crate::domain::reports::{Cell, ColumnKind, Report};
use crate::error::{AppError, AppResult};
use crate::services::documents::FONTS;
use crate::services::export::{CsvWriter, number_cell, text_cell};

static TEMPLATE: &str = include_str!("../../assets/templates/report.typ");
static ENGINE: LazyLock<TypstEngine<TypstTemplateMainFile>> = LazyLock::new(|| {
    TypstEngine::builder()
        .main_file(TEMPLATE)
        .fonts(FONTS)
        .build()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Xlsx,
    Pdf,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Pdf => "pdf",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv; charset=utf-8",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pdf => "application/pdf",
        }
    }
}

fn currency(report: &Report) -> Currency {
    Currency::from_code(&report.currency)
        .or_else(|| Currency::from_code("NZD"))
        .expect("NZD is a known currency")
}

fn display_date(value: &str) -> String {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|d| d.format("%-d %b %Y").to_string())
        .unwrap_or_else(|_| value.to_owned())
}

/// The heading lines every format shows: period, basis and context.
fn heading(report: &Report) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(p) = report.period {
        lines.push(format!(
            "{} to {}",
            p.from.format("%-d %b %Y"),
            p.to.format("%-d %b %Y")
        ));
    }
    if let Some(basis) = &report.basis {
        lines.push(format!(
            "{} basis",
            if basis == "cash" { "Cash" } else { "Accrual" }
        ));
    }
    lines
}

/// The file name for a download, e.g. `Profit and loss 2026-04-01 to 2027-03-31.pdf`.
pub fn file_name(report: &Report, format: ExportFormat) -> String {
    let period = report
        .period
        .map(|p| format!(" {} to {}", p.from, p.to))
        .unwrap_or_else(|| format!(" {}", report.generated_on));
    let base: String = format!("{}{period}", report.title)
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    format!("{base}.{}", format.extension())
}

// ─── CSV ────────────────────────────────────────────────────────────────────

fn csv_cell(cell: &Cell, kind: ColumnKind, currency: Currency) -> String {
    match (cell, kind) {
        (Cell::Integer(v), ColumnKind::Money) => number_cell(currency.to_decimal(*v)),
        (Cell::Integer(v), _) => number_cell(v),
        (Cell::Text(t), ColumnKind::Number | ColumnKind::Date) => number_cell(t),
        (Cell::Text(t), _) => text_cell(t),
        (Cell::Empty(()), _) => String::new(),
    }
}

/// CSV in UTF-8 with a byte-order mark: the heading, then each section with its title, columns,
/// rows and totals, separated by blank lines. Money is in major units (e.g. 1234.50).
pub fn to_csv(report: &Report) -> Vec<u8> {
    to_csv_many(std::slice::from_ref(report))
}

/// Several reports in one CSV, one after another, separated by two blank lines.
pub fn to_csv_many(reports: &[Report]) -> Vec<u8> {
    let mut csv = CsvWriter::new();
    for (i, report) in reports.iter().enumerate() {
        if i > 0 {
            csv.row(Vec::new());
            csv.row(Vec::new());
        }
        write_csv(&mut csv, report);
    }
    csv.into_bytes()
}

fn write_csv(csv: &mut CsvWriter, report: &Report) {
    let currency = currency(report);
    csv.text_row(&[&report.title]);
    csv.text_row(&[&report.business_name]);
    for line in heading(report).iter().chain(&report.subtitle) {
        csv.text_row(&[line]);
    }
    for section in &report.sections {
        csv.row(Vec::new());
        csv.text_row(&[&section.title]);
        csv.text_row(
            &section
                .columns
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
        );
        for row in section.rows.iter().chain(section.totals.iter()) {
            csv.row(
                row.iter()
                    .zip(&section.columns)
                    .map(|(cell, column)| csv_cell(cell, column.kind, currency))
                    .collect(),
            );
        }
    }
    if !report.notes.is_empty() {
        csv.row(Vec::new());
        for note in &report.notes {
            csv.text_row(&[note]);
        }
    }
}

// ─── Excel ──────────────────────────────────────────────────────────────────

fn sheet_name(title: &str) -> String {
    let name: String = title
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if name.trim().is_empty() {
        "Report".into()
    } else {
        name
    }
}

/// One worksheet: the heading, then each section. Money cells are numbers with two decimal places
/// (so they sum in Excel), dates are dates.
pub fn to_xlsx(report: &Report) -> AppResult<Vec<u8>> {
    to_xlsx_many(std::slice::from_ref(report))
}

/// Several reports in one workbook, a worksheet each.
pub fn to_xlsx_many(reports: &[Report]) -> AppResult<Vec<u8>> {
    let build = || -> Result<Vec<u8>, rust_xlsxwriter::XlsxError> {
        let mut workbook = Workbook::new();
        for report in reports {
            write_sheet(&mut workbook, report)?;
        }
        workbook.save_to_buffer()
    };
    build().map_err(|e| AppError::internal(anyhow::anyhow!("Excel export failed: {e}")))
}

fn write_sheet(workbook: &mut Workbook, report: &Report) -> Result<(), rust_xlsxwriter::XlsxError> {
    let currency = currency(report);
    let money_format = if currency.minor_units == 0 {
        "#,##0"
    } else {
        "#,##0.00"
    };
    let title = Format::new().set_bold().set_font_size(14);
    let bold = Format::new().set_bold();
    let header = Format::new()
        .set_bold()
        .set_border_bottom(rust_xlsxwriter::FormatBorder::Thin);
    let header_right = header.clone().set_align(FormatAlign::Right);
    let money = Format::new().set_num_format(money_format);
    let money_bold = money.clone().set_bold();
    let date = Format::new().set_num_format("d mmm yyyy");
    let sheet = workbook.add_worksheet();
    sheet.set_name(sheet_name(&report.title))?;
    sheet.set_column_width(0, 34)?;
    for col in 1..12 {
        sheet.set_column_width(col, 16)?;
    }
    sheet.write_string_with_format(0, 0, &report.title, &title)?;
    sheet.write_string(1, 0, &report.business_name)?;
    let mut row: u32 = 2;
    for line in heading(report).iter().chain(&report.subtitle) {
        sheet.write_string(row, 0, line)?;
        row += 1;
    }
    for section in &report.sections {
        row += 1;
        sheet.write_string_with_format(row, 0, &section.title, &bold)?;
        row += 1;
        for (col, column) in section.columns.iter().enumerate() {
            let format = if matches!(column.kind, ColumnKind::Text | ColumnKind::Date) {
                &header
            } else {
                &header_right
            };
            sheet.write_string_with_format(row, col as u16, &column.label, format)?;
        }
        row += 1;
        let totals = section.totals.iter().map(|t| (t, true));
        for (cells, is_total) in section.rows.iter().map(|r| (r, false)).chain(totals) {
            for (col, (cell, column)) in cells.iter().zip(&section.columns).enumerate() {
                let col = col as u16;
                match (cell, column.kind) {
                    (Cell::Integer(v), ColumnKind::Money) => {
                        let value = currency.to_decimal(*v).to_f64().unwrap_or_default();
                        let format = if is_total { &money_bold } else { &money };
                        sheet.write_number_with_format(row, col, value, format)?;
                    }
                    (Cell::Integer(v), _) => {
                        sheet.write_number(row, col, *v as f64)?;
                    }
                    (Cell::Text(t), ColumnKind::Number) => match t.parse::<Decimal>() {
                        Ok(d) => {
                            sheet.write_number(row, col, d.to_f64().unwrap_or_default())?;
                        }
                        Err(_) => {
                            sheet.write_string(row, col, t)?;
                        }
                    },
                    (Cell::Text(t), ColumnKind::Date) => {
                        match chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d") {
                            Ok(d) => {
                                use chrono::Datelike;
                                let value = ExcelDateTime::from_ymd(
                                    d.year() as u16,
                                    d.month() as u8,
                                    d.day() as u8,
                                )?;
                                sheet.write_datetime_with_format(row, col, &value, &date)?;
                            }
                            Err(_) => {
                                sheet.write_string(row, col, t)?;
                            }
                        }
                    }
                    (Cell::Text(t), _) => {
                        if is_total {
                            sheet.write_string_with_format(row, col, t, &bold)?;
                        } else {
                            sheet.write_string(row, col, t)?;
                        }
                    }
                    (Cell::Empty(()), _) => {}
                }
            }
            row += 1;
        }
    }
    if !report.notes.is_empty() {
        row += 1;
        for note in &report.notes {
            sheet.write_string(row, 0, note)?;
            row += 1;
        }
    }
    Ok(())
}

// ─── PDF ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct PdfColumn {
    label: String,
    right: bool,
}

#[derive(Debug, Serialize)]
struct PdfSection {
    title: String,
    description: String,
    columns: Vec<PdfColumn>,
    rows: Vec<Vec<String>>,
    totals: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct PdfReport {
    title: String,
    business_name: String,
    accent: String,
    landscape: bool,
    subtitle: Vec<String>,
    meta: Vec<String>,
    sections: Vec<PdfSection>,
    notes: Vec<String>,
}

/// The template's input: one report, or several, each starting on a new page.
#[derive(Debug, Serialize)]
struct PdfDocument {
    title: String,
    business_name: String,
    footer: String,
    reports: Vec<PdfReport>,
}

fn pdf_cell(cell: &Cell, kind: ColumnKind, currency: Currency) -> String {
    match (cell, kind) {
        (Cell::Integer(v), ColumnKind::Money) => currency.format(*v),
        (Cell::Integer(v), _) => v.to_string(),
        (Cell::Text(t), ColumnKind::Date) => display_date(t),
        (Cell::Text(t), _) => t.clone(),
        (Cell::Empty(()), _) => String::new(),
    }
}

/// A branded A4 PDF (landscape when a section has more than six columns). `accent` is a hex colour.
pub async fn to_pdf(report: &Report, accent: &str, product_name: &str) -> AppResult<Vec<u8>> {
    to_pdf_many(
        std::slice::from_ref(report),
        &report.title,
        accent,
        product_name,
    )
    .await
}

/// Several reports in one PDF called `title`, each starting on a new page.
pub async fn to_pdf_many(
    reports: &[Report],
    title: &str,
    accent: &str,
    product_name: &str,
) -> AppResult<Vec<u8>> {
    let first = reports
        .first()
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("no reports to export")))?;
    let data = PdfDocument {
        title: title.to_owned(),
        business_name: first.business_name.clone(),
        footer: format!(
            "Prepared with {product_name} on {}",
            first.generated_on.format("%-d %b %Y")
        ),
        reports: reports.iter().map(|r| pdf_report(r, accent)).collect(),
    };
    render_pdf(&data).await
}

fn pdf_report(report: &Report, accent: &str) -> PdfReport {
    let currency = currency(report);
    let sections: Vec<PdfSection> = report
        .sections
        .iter()
        .map(|s| {
            let cells = |row: &Vec<Cell>| -> Vec<String> {
                row.iter()
                    .zip(&s.columns)
                    .map(|(cell, column)| pdf_cell(cell, column.kind, currency))
                    .collect()
            };
            PdfSection {
                title: s.title.clone(),
                description: s.description.clone().unwrap_or_default(),
                columns: s
                    .columns
                    .iter()
                    .map(|c| PdfColumn {
                        label: c.label.clone(),
                        right: matches!(
                            c.kind,
                            ColumnKind::Money | ColumnKind::Number | ColumnKind::Count
                        ),
                    })
                    .collect(),
                rows: s.rows.iter().map(cells).collect(),
                totals: s.totals.as_ref().map(cells),
            }
        })
        .collect();
    PdfReport {
        title: report.title.clone(),
        business_name: report.business_name.clone(),
        accent: accent.to_owned(),
        landscape: report.sections.iter().any(|s| s.columns.len() > 6),
        subtitle: report.subtitle.clone(),
        meta: heading(report),
        sections,
        notes: report.notes.clone(),
    }
}

async fn render_pdf(data: &PdfDocument) -> AppResult<Vec<u8>> {
    let json = serde_json::to_string(data).map_err(AppError::internal)?;
    tokio::task::spawn_blocking(move || {
        let mut inputs = Dict::new();
        inputs.insert("data".into(), Value::Str(json.as_str().into()));
        let document: PagedDocument = ENGINE
            .compile_with_input(inputs)
            .output
            .map_err(|e| anyhow::anyhow!("report PDF layout failed: {e:?}"))?;
        typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
            .map_err(|e| anyhow::anyhow!("report PDF export failed: {e:?}"))
    })
    .await
    .map_err(AppError::internal)?
    .map_err(AppError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::reports::{Column, ReportPeriod, Section};
    use chrono::NaiveDate;

    fn report() -> Report {
        let mut section = Section::new(
            "Expenses",
            vec![
                Column::new("Category", ColumnKind::Text),
                Column::new("Date", ColumnKind::Date),
                Column::new("Amount", ColumnKind::Money),
            ],
        );
        section.row(vec![
            Cell::text("=Software"),
            Cell::text("2026-06-01"),
            Cell::from(123_450),
        ]);
        section.totals = Some(vec![
            Cell::text("Total"),
            Cell::empty(),
            Cell::from(123_450),
        ]);
        Report {
            kind: "expense_summary".into(),
            title: "Expense summary".into(),
            business_name: "Kererū Studio".into(),
            currency: "NZD".into(),
            period: Some(ReportPeriod {
                from: NaiveDate::from_ymd_opt(2026, 4, 1).expect("date"),
                to: NaiveDate::from_ymd_opt(2027, 3, 31).expect("date"),
            }),
            basis: Some("cash".into()),
            subtitle: vec![],
            sections: vec![section],
            notes: vec!["Informational.".into()],
            generated_on: NaiveDate::from_ymd_opt(2026, 11, 20).expect("date"),
        }
    }

    #[test]
    fn csv_has_the_heading_sections_and_safe_cells() {
        let text = String::from_utf8(to_csv(&report())).expect("utf8");
        assert!(text.starts_with('\u{feff}'));
        assert!(text.contains("1 Apr 2026 to 31 Mar 2027\r\nCash basis\r\n"));
        assert!(text.contains("Category,Date,Amount\r\n"));
        assert!(text.contains("'=Software,2026-06-01,1234.50\r\n"), "{text}");
        assert!(text.contains("Total,,1234.50\r\n"));
    }

    #[test]
    fn xlsx_is_a_workbook() {
        let bytes = to_xlsx(&report()).expect("xlsx");
        assert_eq!(&bytes[..2], b"PK", "an xlsx file is a zip archive");
    }

    #[test]
    fn file_names_carry_the_period_and_no_unsafe_characters() {
        let mut r = report();
        assert_eq!(
            file_name(&r, ExportFormat::Pdf),
            "Expense summary 2026-04-01 to 2027-03-31.pdf"
        );
        r.title = "Statement: A/B".into();
        r.period = None;
        assert_eq!(
            file_name(&r, ExportFormat::Csv),
            "Statement AB 2026-11-20.csv"
        );
    }

    #[tokio::test]
    async fn pdf_renders() {
        let bytes = to_pdf(&report(), "#0f766e", "Solo").await.expect("pdf");
        assert_eq!(&bytes[..4], b"%PDF");
    }
}
