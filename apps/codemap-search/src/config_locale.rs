#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfigCommentLanguage {
    English,
    Korean,
}

impl ConfigCommentLanguage {
    pub(crate) fn select<'a>(self, english: &'a str, korean: &'a str) -> &'a str {
        match self {
            Self::English => english,
            Self::Korean => korean,
        }
    }

    fn from_locale_tag(tag: &str) -> Self {
        let tag = tag.trim();
        if tag.is_empty() || tag.eq_ignore_ascii_case("C") || tag.eq_ignore_ascii_case("POSIX") {
            return Self::English;
        }

        let tag_without_encoding = tag
            .split_once(['.', '@'])
            .map(|(language_tag, _)| language_tag)
            .unwrap_or(tag);
        let language = tag_without_encoding
            .split_once(['-', '_'])
            .map(|(language, _)| language)
            .unwrap_or(tag_without_encoding);

        if language.eq_ignore_ascii_case("ko") {
            Self::Korean
        } else {
            Self::English
        }
    }
}

pub(crate) fn config_comment_language() -> ConfigCommentLanguage {
    match std::panic::catch_unwind(provider::preferred_locale_tags) {
        Ok(mut locale_tags) => locale_tags
            .drain(..)
            .next()
            .map(|tag| ConfigCommentLanguage::from_locale_tag(&tag))
            .unwrap_or(ConfigCommentLanguage::English),
        Err(_) => ConfigCommentLanguage::English,
    }
}

#[cfg(target_os = "macos")]
mod provider {
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::locale::CFLocaleCopyPreferredLanguages;

    pub(super) fn preferred_locale_tags() -> Vec<String> {
        unsafe {
            let raw_languages = CFLocaleCopyPreferredLanguages();
            if raw_languages.is_null() {
                return Vec::new();
            }

            let languages: CFArray<CFString> = TCFType::wrap_under_create_rule(raw_languages);
            languages
                .iter()
                .map(|language| language.to_string())
                .filter(|language| !language.trim().is_empty())
                .collect()
        }
    }
}

#[cfg(windows)]
mod provider {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};

    pub(super) fn preferred_locale_tags() -> Vec<String> {
        unsafe {
            let mut language_count = 0u32;
            let mut buffer_length = 0u32;
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut language_count,
                std::ptr::null_mut(),
                &mut buffer_length,
            ) == 0
                || language_count == 0
                || buffer_length == 0
            {
                return Vec::new();
            }

            let mut buffer = vec![0u16; buffer_length as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut language_count,
                buffer.as_mut_ptr(),
                &mut buffer_length,
            ) == 0
            {
                return Vec::new();
            }

            parse_language_buffer(&buffer[..buffer_length.min(buffer.len() as u32) as usize])
        }
    }

    fn parse_language_buffer(buffer: &[u16]) -> Vec<String> {
        let mut locale_tags = Vec::new();
        let mut start = 0usize;
        for (index, code_unit) in buffer.iter().copied().enumerate() {
            if code_unit != 0 {
                continue;
            }
            if index == start {
                break;
            }
            if let Ok(locale_tag) = String::from_utf16(&buffer[start..index]) {
                let locale_tag = locale_tag.trim().to_string();
                if !locale_tag.is_empty() {
                    locale_tags.push(locale_tag);
                }
            }
            start = index + 1;
        }
        locale_tags
    }
}

#[cfg(target_os = "linux")]
mod provider {
    pub(super) fn preferred_locale_tags() -> Vec<String> {
        sys_locale::get_locales().collect()
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
mod provider {
    pub(super) fn preferred_locale_tags() -> Vec<String> {
        Vec::new()
    }
}
