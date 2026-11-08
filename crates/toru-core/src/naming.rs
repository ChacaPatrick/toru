//! Capture names: the name pattern, `_2`/`_3` numbering and Windows file name rules.

use std::collections::HashSet;

use crate::time::LocalDate;

pub const DEFAULT_PATTERN: &str = "screenshot_{mm}_{dd}_{yyyy}";

const TOKENS: [&str; 3] = ["mm", "dd", "yyyy"];

// Windows allows 255 UTF-16 units in a file name. The rest is room for `_N` and the extension.
const MAX_NAME_UNITS: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    Empty,
    ReservedCharacter(char),
    TrailingDotOrSpace,
    ReservedDeviceName,
    TooLong,
    UnknownToken(String),
}

/// Returns the first name from the pattern that isn't in `taken`: the expanded pattern itself, then
/// `_2`, `_3` and so on. Names are compared the way Windows compares file names.
pub fn next_free_name(pattern: &str, date: LocalDate, taken: &[&str]) -> String {
    let taken: HashSet<String> = taken.iter().map(|name| fold_case(name)).collect();
    let base = expand_pattern(pattern, date);
    let mut candidate = base.clone();
    let mut number = 2;
    while taken.contains(&fold_case(&candidate)) {
        candidate = format!("{base}_{number}");
        number += 1;
    }
    candidate
}

/// Checks that `name` can be a file name on Windows.
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    let reserved = |c| {
        matches!(
            c,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\x1f'
        )
    };
    if let Some(c) = name.chars().find(|&c| reserved(c)) {
        return Err(NameError::ReservedCharacter(c));
    }
    if name.ends_with(['.', ' ']) {
        return Err(NameError::TrailingDotOrSpace);
    }
    if is_reserved_device_name(name) {
        return Err(NameError::ReservedDeviceName);
    }
    if name.encode_utf16().count() > MAX_NAME_UNITS {
        return Err(NameError::TooLong);
    }
    Ok(())
}

/// Checks that every token is known and that the pattern expands to a valid file name.
pub fn validate_pattern(pattern: &str) -> Result<(), NameError> {
    // An unknown token is rejected rather than kept as text. Braces are legal in file names, so a
    // typo like `{yy}` would otherwise go unnoticed until files are saved with it in their names.
    // A brace with no partner isn't a token, so it stays as ordinary text.
    let mut rest = pattern;
    while let Some((_, after_open)) = rest.split_once('{') {
        let Some((token, after_close)) = after_open.split_once('}') else {
            break;
        };
        if !TOKENS.contains(&token) {
            return Err(NameError::UnknownToken(token.to_string()));
        }
        rest = after_close;
    }

    // Tokens expand to digits of a fixed width, so every date gives the same result here.
    let sample = LocalDate {
        year: 2000,
        month: 1,
        day: 1,
    };
    validate_name(&expand_pattern(pattern, sample))
}

fn expand_pattern(pattern: &str, date: LocalDate) -> String {
    pattern
        .replace("{mm}", &format!("{:02}", date.month))
        .replace("{dd}", &format!("{:02}", date.day))
        .replace("{yyyy}", &format!("{:04}", date.year))
}

// Uppercase, because `str::to_lowercase` gives a final Greek sigma its own form and two names that
// are the same file on Windows could then compare as different.
pub(crate) fn fold_case(name: &str) -> String {
    name.to_uppercase()
}

// Windows ignores anything after the first dot and spaces before it, so `con.png` and `NUL .txt`
// are device names too.
fn is_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    let stem = stem.to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(
            stem.strip_prefix("COM").or(stem.strip_prefix("LPT")),
            Some("0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOV_7: LocalDate = LocalDate {
        year: 2026,
        month: 11,
        day: 7,
    };
    const BASE: &str = "screenshot_11_07_2026";

    fn next_default(taken: &[&str]) -> String {
        next_free_name(DEFAULT_PATTERN, NOV_7, taken)
    }

    #[test]
    fn default_pattern_expands_the_date() {
        let date = LocalDate {
            year: 2026,
            month: 12,
            day: 25,
        };
        assert_eq!(
            next_free_name(DEFAULT_PATTERN, date, &[]),
            "screenshot_12_25_2026"
        );
    }

    #[test]
    fn single_digit_months_and_days_are_zero_padded() {
        let date = LocalDate {
            year: 2026,
            month: 3,
            day: 7,
        };
        assert_eq!(
            next_free_name(DEFAULT_PATTERN, date, &[]),
            "screenshot_03_07_2026"
        );
    }

    #[test]
    fn tokens_can_be_reordered_and_repeated() {
        assert_eq!(
            next_free_name("{yyyy}-{mm}-{dd}_{dd}", NOV_7, &[]),
            "2026-11-07_07"
        );
    }

    #[test]
    fn a_pattern_without_tokens_is_used_as_written() {
        assert_eq!(next_free_name("notes", NOV_7, &[]), "notes");
    }

    #[test]
    fn first_duplicate_gets_2_and_the_next_gets_3() {
        assert_eq!(next_default(&[BASE]), format!("{BASE}_2"));
        assert_eq!(
            next_default(&[BASE, &format!("{BASE}_2")]),
            format!("{BASE}_3")
        );
    }

    #[test]
    fn unrelated_names_are_ignored() {
        assert_eq!(next_default(&["login-bug-repro"]), BASE);
    }

    #[test]
    fn numbering_fills_the_first_gap() {
        let two = format!("{BASE}_2");
        let four = format!("{BASE}_4");
        assert_eq!(next_default(&[BASE, &two, &four]), format!("{BASE}_3"));
    }

    #[test]
    fn numbering_does_not_assume_taken_names_are_in_order() {
        let two = format!("{BASE}_2");
        let three = format!("{BASE}_3");
        assert_eq!(next_default(&[&three, BASE, &two]), format!("{BASE}_4"));
    }

    #[test]
    fn the_plain_name_is_reused_when_only_numbered_names_are_taken() {
        assert_eq!(next_default(&[&format!("{BASE}_2")]), BASE);
    }

    #[test]
    fn a_custom_name_that_looks_numbered_is_skipped() {
        assert_eq!(next_free_name("foo", NOV_7, &["foo", "foo_2"]), "foo_3");
    }

    #[test]
    fn a_pattern_ending_in_a_number_gets_its_own_suffix() {
        assert_eq!(next_free_name("foo_2", NOV_7, &["foo_2"]), "foo_2_2");
    }

    #[test]
    fn taken_names_are_compared_without_case() {
        assert_eq!(
            next_default(&["SCREENSHOT_11_07_2026"]),
            format!("{BASE}_2")
        );
        assert_eq!(
            next_default(&[BASE, "Screenshot_11_07_2026_2"]),
            format!("{BASE}_3")
        );
    }

    #[test]
    fn case_comparison_covers_non_ascii_names() {
        assert_eq!(next_free_name("écran", NOV_7, &["ÉCRAN"]), "écran_2");
        assert_eq!(next_free_name("σασ", NOV_7, &["ΣΑΣ"]), "σασ_2");
    }

    #[test]
    fn ordinary_names_are_valid() {
        for name in [
            "screenshot_11_07_2026",
            "login-bug-repro",
            "foo_2",
            "has spaces inside",
            " leading space",
            ".hidden",
            "two.dots.here",
            "braces {are} fine in a name",
            "スクリーン",
        ] {
            assert_eq!(validate_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn empty_name_is_rejected() {
        assert_eq!(validate_name(""), Err(NameError::Empty));
    }

    #[test]
    fn reserved_characters_are_rejected() {
        for c in [
            '<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0', '\n', '\x1f',
        ] {
            assert_eq!(
                validate_name(&format!("a{c}b")),
                Err(NameError::ReservedCharacter(c))
            );
        }
    }

    #[test]
    fn trailing_dots_and_spaces_are_rejected() {
        for name in ["foo.", "foo ", "foo. ", ".", "..", "   "] {
            assert_eq!(
                validate_name(name),
                Err(NameError::TrailingDotOrSpace),
                "{name}"
            );
        }
    }

    #[test]
    fn reserved_device_names_are_rejected_in_any_case() {
        for name in [
            "CON", "PRN", "AUX", "NUL", "con", "Nul", "COM0", "COM1", "com9", "LPT0", "LPT1",
            "lpt9", "COM¹", "COM²", "LPT³",
        ] {
            assert_eq!(
                validate_name(name),
                Err(NameError::ReservedDeviceName),
                "{name}"
            );
        }
    }

    #[test]
    fn reserved_device_names_are_rejected_with_an_extension() {
        for name in ["con.png", "NUL.tar.gz", "NUL .txt", "com1.webp"] {
            assert_eq!(
                validate_name(name),
                Err(NameError::ReservedDeviceName),
                "{name}"
            );
        }
    }

    #[test]
    fn names_that_only_resemble_device_names_are_valid() {
        for name in [
            "CONSOLE", "COM10", "COM", "LPT", "COMA", "my.con", "con_2", " CON",
        ] {
            assert_eq!(validate_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn name_length_is_counted_in_utf16_units() {
        assert_eq!(validate_name(&"a".repeat(MAX_NAME_UNITS)), Ok(()));
        assert_eq!(
            validate_name(&"a".repeat(MAX_NAME_UNITS + 1)),
            Err(NameError::TooLong)
        );
        assert_eq!(validate_name(&"😀".repeat(MAX_NAME_UNITS / 2)), Ok(()));
        assert_eq!(
            validate_name(&"😀".repeat(MAX_NAME_UNITS / 2 + 1)),
            Err(NameError::TooLong)
        );
    }

    #[test]
    fn known_patterns_are_valid() {
        for pattern in [
            DEFAULT_PATTERN,
            "notes",
            "{yyyy}-{mm}-{dd}",
            "{dd}",
            "open { only",
            "close } only",
            "} backwards {",
        ] {
            assert_eq!(validate_pattern(pattern), Ok(()), "{pattern}");
        }
    }

    #[test]
    fn unknown_tokens_are_rejected() {
        for (pattern, token) in [
            ("shot_{yy}", "yy"),
            ("shot_{MM}", "MM"),
            ("{mm}_{hour}_{dd}", "hour"),
            ("shot_{}", ""),
            ("{{mm}}", "{mm"),
        ] {
            assert_eq!(
                validate_pattern(pattern),
                Err(NameError::UnknownToken(token.to_string())),
                "{pattern}"
            );
        }
    }

    #[test]
    fn patterns_follow_the_file_name_rules() {
        assert_eq!(validate_pattern(""), Err(NameError::Empty));
        assert_eq!(
            validate_pattern("a:{mm}"),
            Err(NameError::ReservedCharacter(':'))
        );
        assert_eq!(
            validate_pattern("{mm}."),
            Err(NameError::TrailingDotOrSpace)
        );
        assert_eq!(
            validate_pattern("{mm} "),
            Err(NameError::TrailingDotOrSpace)
        );
        assert_eq!(validate_pattern("con"), Err(NameError::ReservedDeviceName));
        assert_eq!(
            validate_pattern(&"a".repeat(MAX_NAME_UNITS + 1)),
            Err(NameError::TooLong)
        );
    }

    #[test]
    fn pattern_length_is_measured_after_expansion() {
        let pattern = format!("{}{{yyyy}}", "a".repeat(MAX_NAME_UNITS - 4));
        assert_eq!(validate_pattern(&pattern), Ok(()));
    }
}
