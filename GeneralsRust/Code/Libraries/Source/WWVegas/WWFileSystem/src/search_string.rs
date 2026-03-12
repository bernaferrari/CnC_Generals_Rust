/// Checks if a string matches a search pattern with wildcards
/// Matches the C++ SearchStringMatches function exactly
/// * is used to denote any number of characters
/// ? is used to denote a single wildcard character
pub fn search_string_matches(s: &str, search_string: &str) -> bool {
    if s.is_empty() {
        return search_string.is_empty();
    }
    if search_string.is_empty() {
        return false;
    }

    let s_chars: Vec<char> = s.chars().collect();
    let search_chars: Vec<char> = search_string.chars().collect();

    search_string_matches_impl(&s_chars, &search_chars, 0, 0)
}

fn search_string_matches_impl(
    s_chars: &[char],
    search_chars: &[char],
    s_idx: usize,
    search_idx: usize,
) -> bool {
    let mut s_i = s_idx;
    let mut search_i = search_idx;

    loop {
        // Check if we've reached the end of either string
        if s_i >= s_chars.len() {
            return search_i >= search_chars.len();
        }
        if search_i >= search_chars.len() {
            return false;
        }

        let s_char = s_chars[s_i];
        let search_char = search_chars[search_i];

        if s_char == search_char || search_char == '?' {
            s_i += 1;
            search_i += 1;
        } else if search_char == '*' {
            search_i += 1;
            if search_i >= search_chars.len() {
                return true;
            }
            // Try matching the rest from various positions
            while s_i < s_chars.len() {
                if search_string_matches_impl(s_chars, search_chars, s_i, search_i) {
                    return true;
                }
                s_i += 1;
            }
            return false;
        } else {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(search_string_matches("test.txt", "test.txt"));
        assert!(!search_string_matches("test.txt", "test.ini"));
    }

    #[test]
    fn test_wildcard_star() {
        assert!(search_string_matches("test.txt", "*.txt"));
        assert!(search_string_matches("test.txt", "test.*"));
        assert!(search_string_matches("test.txt", "*"));
    }

    #[test]
    fn test_wildcard_question() {
        assert!(search_string_matches("test.txt", "test.?xt"));
        assert!(search_string_matches("test.txt", "?est.txt"));
    }

    #[test]
    fn test_empty_strings() {
        assert!(search_string_matches("", ""));
        assert!(!search_string_matches("test", ""));
        assert!(!search_string_matches("", "test"));
    }
}
