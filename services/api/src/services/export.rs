//! Spreadsheet exports. CSV in UTF-8 with a byte-order mark, so Excel opens it with the right
//! encoding; text that a spreadsheet would run as a formula is neutralised.

/// Builds a CSV document row by row.
pub struct CsvWriter {
    buf: String,
}

impl Default for CsvWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl CsvWriter {
    pub fn new() -> Self {
        Self {
            buf: String::from('\u{feff}'),
        }
    }

    /// Appends a row of text cells (escaped; formula-like text is prefixed with `'`).
    pub fn text_row<S: AsRef<str>>(&mut self, cells: &[S]) {
        self.push_row(cells.iter().map(|c| text_cell(c.as_ref())));
    }

    /// Appends a row whose cells are already escaped (see [`text_cell`] and [`number_cell`]).
    pub fn row(&mut self, cells: Vec<String>) {
        self.push_row(cells.into_iter());
    }

    fn push_row(&mut self, cells: impl Iterator<Item = String>) {
        let line: Vec<String> = cells.collect();
        self.buf.push_str(&line.join(","));
        self.buf.push_str("\r\n");
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf.into_bytes()
    }
}

/// A text cell: quoted when needed, and prefixed with `'` when a spreadsheet would treat it as a
/// formula (`=`, `+`, `-`, `@`, tab or carriage return first).
pub fn text_cell(value: &str) -> String {
    let neutralised = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    quote(&neutralised)
}

/// A number or date cell, written as is (never treated as a formula).
pub fn number_cell(value: impl ToString) -> String {
    quote(&value.to_string())
}

fn quote(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_starts_with_a_bom_and_escapes_cells() {
        let mut csv = CsvWriter::new();
        csv.text_row(&["Date", "Purpose"]);
        csv.row(vec![
            number_cell("2026-11-20"),
            text_cell("Client visit, \"Kererū Café\""),
        ]);
        let text = String::from_utf8(csv.into_bytes()).expect("utf-8");
        assert_eq!(
            text,
            "\u{feff}Date,Purpose\r\n2026-11-20,\"Client visit, \"\"Kererū Café\"\"\"\r\n"
        );
    }

    #[test]
    fn formula_like_text_is_neutralised() {
        assert_eq!(text_cell("=SUM(A1:A9)"), "'=SUM(A1:A9)");
        assert_eq!(text_cell("+64 21 000 000"), "'+64 21 000 000");
        assert_eq!(text_cell("@home"), "'@home");
        assert_eq!(text_cell("Wellington"), "Wellington");
        assert_eq!(number_cell("-12.50"), "-12.50");
    }
}
