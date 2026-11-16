//! Barcode formats and card-number validation.
//!
//! The server is authoritative; the frontend mirrors these rules only to give
//! instant feedback. Numbers are normalised before validation (spaces removed
//! from numeric symbologies, Code 39 / Codabar upper-cased).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::ApiError;

/// 2D symbologies carry arbitrary text (often URLs or JSON); keep it bounded.
const MAX_TEXT_CHARS: usize = 512;
/// Linear symbologies wider than this do not fit a phone screen anyway.
const MAX_LINEAR_CHARS: usize = 80;
/// Code 39 is not dense; beyond ~43 characters it no longer fits a phone screen.
const MAX_CODE39_CHARS: usize = 43;
const CODE39_EXTRA: &str = " -.$/+%";
const CODABAR_EXTRA: &str = "-$:/.+";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum BarcodeFormat {
    Ean13,
    Ean8,
    Upca,
    Code128,
    Code39,
    Itf,
    Codabar,
    Qr,
    Pdf417,
    Aztec,
    Datamatrix,
    /// No barcode: the number is shown as text and typed in by the cashier.
    None,
}

impl BarcodeFormat {
    /// Returns the normalised number, or a message explaining what is wrong.
    pub fn normalize(self, raw: &str) -> Result<String, ApiError> {
        let number = match self {
            Self::Ean13 | Self::Ean8 | Self::Upca | Self::Itf => strip_whitespace(raw),
            Self::Code39 | Self::Codabar => raw.trim().to_ascii_uppercase(),
            _ => raw.trim().to_owned(),
        };
        if number.is_empty() {
            return Err(invalid("card number is required"));
        }
        self.check(&number)?;
        Ok(number)
    }

    fn check(self, number: &str) -> Result<(), ApiError> {
        match self {
            Self::Ean13 => check_gtin(number, 13, "EAN-13"),
            Self::Ean8 => check_gtin(number, 8, "EAN-8"),
            Self::Upca => check_gtin(number, 12, "UPC-A"),
            Self::Itf => check_itf(number),
            Self::Code128 => check_code128(number),
            Self::Code39 => check_code39(number),
            Self::Codabar => check_codabar(number),
            Self::Qr | Self::Pdf417 | Self::Aztec | Self::Datamatrix | Self::None => {
                check_text(number)
            }
        }
    }
}

fn invalid(message: impl Into<String>) -> ApiError {
    ApiError::Validation(message.into())
}

fn strip_whitespace(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}

fn all_digits(number: &str) -> bool {
    number.bytes().all(|b| b.is_ascii_digit())
}

/// EAN/UPC (GTIN) numbers: fixed length, digits only, mod-10 check digit last.
fn check_gtin(number: &str, length: usize, name: &str) -> Result<(), ApiError> {
    if number.len() != length || !all_digits(number) {
        return Err(invalid(format!(
            "{name} numbers have exactly {length} digits"
        )));
    }
    let (body, check) = number.split_at(length - 1);
    if gtin_check_digit(body) != check.as_bytes()[0] - b'0' {
        return Err(invalid(format!(
            "this is not a valid {name} number (the last digit does not match)"
        )));
    }
    Ok(())
}

/// GS1 mod-10: weights 3,1,3,1… from the rightmost digit of the body.
pub fn gtin_check_digit(body: &str) -> u8 {
    let sum: u32 = body
        .bytes()
        .rev()
        .enumerate()
        .map(|(index, byte)| {
            let digit = u32::from(byte - b'0');
            if index % 2 == 0 { digit * 3 } else { digit }
        })
        .sum();
    // `sum % 10` is below 10, so the result always fits in a u8.
    ((10 - sum % 10) % 10) as u8
}

fn check_itf(number: &str) -> Result<(), ApiError> {
    if !all_digits(number) || !number.len().is_multiple_of(2) || number.len() > MAX_LINEAR_CHARS {
        return Err(invalid(
            "ITF numbers have an even number of digits (up to 80)",
        ));
    }
    Ok(())
}

fn check_code128(number: &str) -> Result<(), ApiError> {
    if number.len() > MAX_LINEAR_CHARS || !number.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return Err(invalid("Code 128 supports up to 80 plain ASCII characters"));
    }
    Ok(())
}

fn check_code39(number: &str) -> Result<(), ApiError> {
    let valid = number
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || CODE39_EXTRA.contains(c));
    if !valid || number.chars().count() > MAX_CODE39_CHARS {
        return Err(invalid(
            "Code 39 supports up to 43 letters, digits and - . $ / + % or space",
        ));
    }
    Ok(())
}

/// Digits and `-$:/.+`, optionally wrapped in start/stop letters A–D.
fn check_codabar(number: &str) -> Result<(), ApiError> {
    let body = match (number.chars().next(), number.chars().last()) {
        (Some(start), Some(stop))
            if number.len() >= 3 && is_codabar_guard(start) && is_codabar_guard(stop) =>
        {
            &number[1..number.len() - 1]
        }
        _ => number,
    };
    let valid = body
        .chars()
        .all(|c| c.is_ascii_digit() || CODABAR_EXTRA.contains(c));
    if !valid || body.is_empty() || number.len() > MAX_LINEAR_CHARS {
        return Err(invalid(
            "Codabar supports digits and - $ : / . +, optionally between start/stop letters A–D",
        ));
    }
    Ok(())
}

fn is_codabar_guard(c: char) -> bool {
    matches!(c, 'A'..='D')
}

fn check_text(number: &str) -> Result<(), ApiError> {
    if number.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(format!(
            "card number must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    if number
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(invalid("card number contains invalid characters"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::BarcodeFormat::*;
    use super::*;

    fn ok(format: BarcodeFormat, raw: &str) -> String {
        format
            .normalize(raw)
            .unwrap_or_else(|e| panic!("{format:?} {raw:?}: {e}"))
    }

    fn err(format: BarcodeFormat, raw: &str) -> String {
        format
            .normalize(raw)
            .expect_err(&format!("{format:?} {raw:?} should be rejected"))
            .to_string()
    }

    #[test]
    fn gtin_check_digit_matches_known_codes() {
        assert_eq!(gtin_check_digit("400638133393"), 1); // EAN-13 4006381333931
        assert_eq!(gtin_check_digit("9638507"), 4); // EAN-8 96385074
        assert_eq!(gtin_check_digit("03600029145"), 2); // UPC-A 036000291452
        assert_eq!(gtin_check_digit("000000000000"), 0);
    }

    #[test]
    fn ean13_accepts_valid_numbers_and_strips_spaces() {
        assert_eq!(ok(Ean13, " 4 006381 333931 "), "4006381333931");
    }

    #[test]
    fn ean13_rejects_wrong_length_letters_and_bad_check_digit() {
        assert!(err(Ean13, "400638133393").contains("exactly 13 digits"));
        assert!(err(Ean13, "40063813339AB").contains("exactly 13 digits"));
        assert!(err(Ean13, "4006381333932").contains("last digit"));
    }

    #[test]
    fn ean8_and_upca_use_their_own_lengths() {
        assert_eq!(ok(Ean8, "96385074"), "96385074");
        assert!(err(Ean8, "96385075").contains("EAN-8"));
        assert_eq!(ok(Upca, "036000291452"), "036000291452");
        assert!(err(Upca, "4006381333931").contains("12 digits"));
    }

    #[test]
    fn itf_needs_an_even_number_of_digits() {
        assert_eq!(ok(Itf, "1234 5678"), "12345678");
        assert!(err(Itf, "123").contains("even"));
        assert!(err(Itf, "12AB").contains("even"));
    }

    #[test]
    fn code128_accepts_printable_ascii_only() {
        assert_eq!(ok(Code128, " Abc-123/xyz "), "Abc-123/xyz");
        assert!(err(Code128, "caffè").contains("ASCII"));
        assert!(err(Code128, &"9".repeat(81)).contains("80"));
    }

    #[test]
    fn code39_is_uppercased_and_limited_to_its_alphabet() {
        assert_eq!(ok(Code39, "abc-12"), "ABC-12");
        assert!(err(Code39, "AB_12").contains("Code 39"));
        assert!(err(Code39, &"A".repeat(44)).contains("43"));
    }

    #[test]
    fn codabar_accepts_optional_start_stop_letters() {
        assert_eq!(ok(Codabar, "a40156b"), "A40156B");
        assert_eq!(ok(Codabar, "40156"), "40156");
        assert_eq!(ok(Codabar, "12-34$56"), "12-34$56");
        assert!(err(Codabar, "A12E").contains("Codabar"));
        assert!(err(Codabar, "AB").contains("Codabar"));
    }

    #[test]
    fn two_dimensional_formats_accept_any_text() {
        assert_eq!(
            ok(Qr, "https://example.com/c?id=1 2"),
            "https://example.com/c?id=1 2"
        );
        assert_eq!(ok(Datamatrix, "Ünïcødé"), "Ünïcødé");
        assert_eq!(ok(None, " 12 34 "), "12 34");
        assert!(err(Pdf417, &"x".repeat(513)).contains("512"));
        assert!(err(Aztec, "a\u{0}b").contains("invalid characters"));
    }

    #[test]
    fn empty_numbers_are_rejected_for_every_format() {
        for format in [Ean13, Code128, Qr, None] {
            assert!(err(format, "   ").contains("required"));
        }
    }
}
