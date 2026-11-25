//! Input normalisation and validation rules shared by services. Pure functions, unit-tested here.

/// Trims and lower-cases an email address for storage and comparison.
pub fn normalize_email(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// A pragmatic email check: one `@`, a non-empty local part and a dotted domain, no spaces.
pub fn is_valid_email(email: &str) -> bool {
    if email.len() > 254 || email.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains("..")
}

/// Trims a free-text field and enforces a maximum length in characters.
pub fn clean_text(value: &str, max_chars: usize) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max_chars {
        return Err(format!("Use {max_chars} characters or fewer."));
    }
    Ok(trimmed.to_owned())
}

/// Removes spaces and dashes, the separators people commonly type in identifiers.
pub fn digits_only(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect()
}

/// Validates a New Zealand IRD (GST) number with IRD's mod-11 check digit algorithm.
/// Accepts 8 or 9 digits, in the range 10,000,000 – 150,000,000.
pub fn is_valid_ird_number(value: &str) -> bool {
    let digits = digits_only(value);
    if !(8..=9).contains(&digits.len()) || !digits.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Ok(number) = digits.parse::<u64>() else {
        return false;
    };
    if !(10_000_000..=150_000_000).contains(&number) {
        return false;
    }
    let padded = format!("{number:09}");
    let values: Vec<u32> = padded.chars().filter_map(|c| c.to_digit(10)).collect();
    let (base, check) = values.split_at(8);
    let check = check[0];
    let compute = |weights: [u32; 8]| -> Option<u32> {
        let sum: u32 = base.iter().zip(weights).map(|(d, w)| d * w).sum();
        match sum % 11 {
            0 => Some(0),
            remainder => {
                let digit = 11 - remainder;
                (digit != 10).then_some(digit)
            }
        }
    };
    match compute([3, 2, 7, 6, 5, 4, 3, 2]).or_else(|| compute([7, 4, 3, 2, 5, 2, 7, 6])) {
        Some(digit) => digit == check,
        None => false,
    }
}

/// Validates a New Zealand Business Number: 13 digits with a GS1 (GTIN-13) check digit.
pub fn is_valid_nzbn(value: &str) -> bool {
    let digits = digits_only(value);
    if digits.len() != 13 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let values: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    let sum: u32 = values[..12]
        .iter()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { *d } else { d * 3 })
        .sum();
    (10 - sum % 10) % 10 == values[12]
}

/// A `#rrggbb` colour.
pub fn is_hex_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// ISO 4217 currency codes the product formats and calculates with (precision in `money.rs`).
pub fn is_supported_currency(code: &str) -> bool {
    crate::domain::money::Currency::from_code(code).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert_eq!(normalize_email("  Jane@Example.COM "), "jane@example.com");
        assert!(is_valid_email("jane@example.co.nz"));
        assert!(!is_valid_email("jane@localhost"));
        assert!(!is_valid_email("jane example@x.nz"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("jane@@example.com"));
        assert!(!is_valid_email("jane@example..com"));
    }

    #[test]
    fn ird_numbers_from_the_ird_specification() {
        for valid in [
            "49091850",
            "35901981",
            "49098576",
            "136410132",
            "49-091-850",
            "136 410 132",
        ] {
            assert!(is_valid_ird_number(valid), "{valid} should be valid");
        }
        for invalid in [
            "136410133",
            "9125568",
            "12345678",
            "1234567890",
            "abcdefgh",
            "",
        ] {
            assert!(!is_valid_ird_number(invalid), "{invalid} should be invalid");
        }
    }

    #[test]
    fn nzbn_check_digit() {
        // GS1 worked example: 942904123456 → weighted sum 1·9+3·4+1·2+3·9+1·0+3·4+1·1+3·2+1·3+3·4+1·5+3·6 = 107,
        // so the check digit is (10 − 7) mod 10 = 3.
        assert!(is_valid_nzbn("9429041234563"));
        assert!(is_valid_nzbn("9429-0412-3456-3"));
        assert!(!is_valid_nzbn("9429041234564"));
        assert!(!is_valid_nzbn("942904123456"));
        assert!(!is_valid_nzbn("94290412345a3"));
    }

    #[test]
    fn text_cleaning() {
        assert_eq!(clean_text("  hi  ", 5).as_deref(), Ok("hi"));
        assert!(clean_text("toolong", 3).is_err());
        assert_eq!(digits_only("12-345 678"), "12345678");
    }
}
