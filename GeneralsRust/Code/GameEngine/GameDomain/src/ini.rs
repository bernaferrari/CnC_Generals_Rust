//! Lexical rules shared by the original INI parsers.

mod blocks;
pub use blocks::{OrderedIniProperties, parse_ini_sections_ordered};

/// Parse leading ASCII whitespace, an optional sign, and one or more decimal digits.
///
/// The complete integer prefix must fit in `i32`; missing digits and overflow return `None`.
pub fn parse_decimal_integer_prefix(value: &str) -> Option<i32> {
    let bytes = value.as_bytes();
    let mut start = 0;
    while bytes
        .get(start)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        start += 1;
    }

    let mut end = start;
    if matches!(bytes.get(end), Some(b'+' | b'-')) {
        end += 1;
    }
    let digits_start = end;
    while bytes.get(end).is_some_and(|byte| byte.is_ascii_digit()) {
        end += 1;
    }
    if end == digits_start {
        return None;
    }

    value[start..end].parse().ok()
}

/// Parse replacement flags or +/- changes to an existing flag set.
pub fn parse_bit_string_32_with_initial(value: &str, names: &[&str], mut bits: u32) -> Option<u32> {
    let mut found_normal = false;
    let mut found_add_or_sub = false;
    for token in value.split_whitespace() {
        if token.eq_ignore_ascii_case("NONE") {
            if found_normal || found_add_or_sub {
                return None;
            }
            return Some(0);
        }
        let (name, set) = if let Some(rest) = token.strip_prefix('+') {
            if found_normal {
                return None;
            }
            found_add_or_sub = true;
            (rest, true)
        } else if let Some(rest) = token.strip_prefix('-') {
            if found_normal {
                return None;
            }
            found_add_or_sub = true;
            (rest, false)
        } else {
            if found_add_or_sub {
                return None;
            }
            if !found_normal {
                bits = 0;
            }
            found_normal = true;
            (token, true)
        };
        let index = names
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(name))?;
        let mask = 1u32 << index;
        if set {
            bits |= mask;
        } else {
            bits &= !mask;
        }
    }
    Some(bits)
}

#[cfg(test)]
mod tests {
    use super::parse_decimal_integer_prefix;

    #[test]
    fn parses_defined_signed_decimal_prefixes_and_rejects_missing_or_overflow() {
        for (input, expected) in [
            ("  +12.5", Some(12)),
            ("25suffix", Some(25)),
            ("-2147483648tail", Some(i32::MIN)),
            ("2147483647.0", Some(i32::MAX)),
            ("0x10", Some(0)),
            ("suffix25", None),
            ("+", None),
            ("2147483648", None),
            ("-2147483649", None),
        ] {
            assert_eq!(parse_decimal_integer_prefix(input), expected, "{input:?}");
        }
    }
}
