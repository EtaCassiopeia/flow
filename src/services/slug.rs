const MAX_SLUG_LEN: usize = 50;

pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = true;
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_alphanumeric() {
            ascii_fold(ch).map(|c| c.to_ascii_lowercase())
        } else {
            None
        };
        match mapped {
            Some(c) => {
                out.push(c);
                prev_dash = false;
            }
            None => {
                if !prev_dash && !out.is_empty() {
                    out.push('-');
                    prev_dash = true;
                }
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > MAX_SLUG_LEN {
        let mut cut = MAX_SLUG_LEN;
        while cut > 0 && !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

fn ascii_fold(ch: char) -> Option<char> {
    Some(match ch {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ß' => 's',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn slug_lowercases_and_kebabs() {
        assert_eq!(slugify("Fix the Login Bug"), "fix-the-login-bug");
    }

    #[test]
    fn slug_collapses_separators() {
        assert_eq!(slugify("a   b___c"), "a-b-c");
    }

    #[test]
    fn slug_trims_trailing_dashes() {
        assert_eq!(slugify("hello!!!"), "hello");
    }

    #[test]
    fn slug_handles_empty() {
        assert_eq!(slugify("   "), "");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slug_truncates_to_max_len() {
        let s = slugify(&"a".repeat(200));
        assert_eq!(s.len(), MAX_SLUG_LEN);
    }

    #[test]
    fn slug_folds_diacritics() {
        assert_eq!(slugify("Crème brûlée"), "creme-brulee");
    }

    proptest! {
        #[test]
        fn slug_only_emits_lowercase_and_dashes(input in ".*") {
            let s = slugify(&input);
            prop_assert!(s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
            prop_assert!(!s.starts_with('-'));
            prop_assert!(!s.ends_with('-'));
            prop_assert!(s.len() <= MAX_SLUG_LEN);
        }
    }
}
