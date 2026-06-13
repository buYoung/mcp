pub fn split_identifier(ident: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = ident.chars().collect();
    let len = chars.len();

    for i in 0..len {
        let c = chars[i];

        if c == '_' || c == '-' {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            continue;
        }

        let prev_is_lowercase = i > 0 && chars[i - 1].is_lowercase();
        let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let prev_is_uppercase = i > 0 && chars[i - 1].is_uppercase();

        let current_is_uppercase = c.is_uppercase();

        let next_is_lowercase = i + 1 < len && chars[i + 1].is_lowercase();

        let is_camel_boundary = (prev_is_lowercase || prev_is_digit) && current_is_uppercase;
        let is_acronym_boundary = prev_is_uppercase && current_is_uppercase && next_is_lowercase;
        let is_digit_boundary = prev_is_digit && c.is_alphabetic();

        let prev_is_uncased = i > 0
            && chars[i - 1].is_alphabetic()
            && !chars[i - 1].is_lowercase()
            && !chars[i - 1].is_uppercase();
        let current_is_uncased = c.is_alphabetic() && !c.is_lowercase() && !c.is_uppercase();
        let is_uncased_boundary = i > 0
            && ((prev_is_uncased
                && !current_is_uncased
                && (c.is_alphabetic() || c.is_ascii_digit()))
                || (!prev_is_uncased
                    && (chars[i - 1].is_alphabetic() || chars[i - 1].is_ascii_digit())
                    && current_is_uncased));

        if (is_camel_boundary || is_acronym_boundary || is_digit_boundary || is_uncased_boundary)
            && !current.is_empty()
        {
            tokens.push(current.to_lowercase());
            current.clear();
        }

        current.push(c);
    }

    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Sub-tokenization Tests ---
    #[test]
    fn test_split_identifier_cases() {
        assert_eq!(
            split_identifier("handleLoginError"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(
            split_identifier("handle_login_error"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(
            split_identifier("handle-login-error"),
            vec!["handle", "login", "error"]
        );
        assert_eq!(split_identifier("HTTPClient"), vec!["http", "client"]);
        assert_eq!(split_identifier("v2Engine"), vec!["v2", "engine"]);
        assert_eq!(
            split_identifier("API2026version"),
            vec!["api2026", "version"]
        );
        assert_eq!(
            split_identifier("XMLHttpRequest"),
            vec!["xml", "http", "request"]
        );
        assert_eq!(split_identifier("HTMLElement"), vec!["html", "element"]);
        assert_eq!(split_identifier(""), Vec::<String>::new());
        assert_eq!(split_identifier("a"), vec!["a"]);
    }

    #[test]
    fn test_split_identifier_unicode_boundaries() {
        assert_eq!(split_identifier("한글MyName"), vec!["한글", "my", "name"]);
        assert_eq!(split_identifier("My한글Name"), vec!["my", "한글", "name"]);
        assert_eq!(split_identifier("한글_my_name"), vec!["한글", "my", "name"]);
    }
}
