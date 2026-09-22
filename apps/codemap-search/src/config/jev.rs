use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JevConfig {
    #[serde(rename = "overview_enabled")]
    pub is_overview_enabled: bool,
    #[serde(rename = "search_filter_enabled")]
    pub is_search_filter_enabled: bool,
    pub model: String,
    pub api_key_env: String,
    pub timeout_ms: u64,
    pub max_in_flight_requests: usize,
    pub request_spacing_ms: u64,
    pub max_batch_bytes: usize,
    pub pool_idle_timeout_ms: u64,
    pub search_filter_min_unrelated_probability: f64,
}

impl Default for JevConfig {
    fn default() -> Self {
        let policy = crate::jev::Policy::default();
        Self {
            is_overview_enabled: false,
            is_search_filter_enabled: false,
            model: crate::jev::MODEL.into(),
            api_key_env: "TYPESAFE_API_KEY".into(),
            timeout_ms: policy.timeout_ms,
            max_in_flight_requests: policy.max_in_flight_requests,
            request_spacing_ms: policy.request_spacing_ms,
            max_batch_bytes: policy.max_batch_bytes,
            pool_idle_timeout_ms: policy.pool_idle_timeout_ms,
            search_filter_min_unrelated_probability:
                crate::tools::search::jev::DEFAULT_MIN_UNRELATED_PROBABILITY,
        }
    }
}

impl JevConfig {
    pub fn policy(&self) -> crate::jev::Policy {
        crate::jev::Policy {
            timeout_ms: self.timeout_ms,
            max_in_flight_requests: self.max_in_flight_requests,
            request_spacing_ms: self.request_spacing_ms,
            max_batch_bytes: self.max_batch_bytes,
            pool_idle_timeout_ms: self.pool_idle_timeout_ms,
        }
    }
}

#[derive(Default, PartialEq)]
pub(super) struct JevLayer(BTreeMap<String, serde_json::Value>);

pub(super) fn normalize(value: &toml::Value, path: &Path) -> JevLayer {
    let mut values = BTreeMap::new();
    let Some(table) = value.as_table() else {
        super::warn("analysis.jev must be a table — ignored");
        return JevLayer(values);
    };
    for (key, value) in table {
        let is_valid = match key.as_str() {
            "overview_enabled" | "search_filter_enabled" => value.as_bool().is_some(),
            "model" => value.as_str() == Some(crate::jev::MODEL),
            "api_key_env" => value.as_str().is_some_and(|name| {
                !name.is_empty()
                    && name.len() <= 128
                    && name.bytes().enumerate().all(|(i, c)| {
                        c == b'_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()
                    })
            }),
            "timeout_ms" => value
                .as_integer()
                .is_some_and(|n| (1..=45_000).contains(&n)),
            "max_in_flight_requests" => value.as_integer().is_some_and(|n| (1..=3).contains(&n)),
            "request_spacing_ms" => value
                .as_integer()
                .is_some_and(|n| (300..=45_000).contains(&n)),
            "max_batch_bytes" => value
                .as_integer()
                .is_some_and(|n| (1024..=80_000).contains(&n)),
            "pool_idle_timeout_ms" => value
                .as_integer()
                .is_some_and(|n| (1..=30_000).contains(&n)),
            "search_filter_min_unrelated_probability" => value
                .as_float()
                .or_else(|| value.as_integer().map(|n| n as f64))
                .is_some_and(crate::tools::search::jev::is_valid_threshold),
            _ => false,
        };
        if is_valid {
            if let Ok(value) = serde_json::to_value(value) {
                values.insert(key.clone(), value);
            }
        } else {
            super::warn(&format!(
                "invalid or unknown config key 'analysis.jev.{key}': {} — using lower layer",
                path.display()
            ));
        }
    }
    JevLayer(values)
}

pub(super) fn merge(repo: JevLayer, global: JevLayer) -> JevConfig {
    let mut values = serde_json::to_value(JevConfig::default()).expect("serializable defaults");
    let object = values.as_object_mut().unwrap();
    object.extend(global.0);
    object.extend(repo.0);
    serde_json::from_value(values).expect("normalized Jev settings")
}

/// Scope migration guards to this section: a model field elsewhere is unrelated.
pub(super) fn mentions_key(contents: &str, key: &str) -> bool {
    if toml::from_str::<toml::Value>(contents)
        .ok()
        .and_then(|value| value.get("analysis")?.get("jev")?.get(key).cloned())
        .is_some()
    {
        return true;
    }
    let ranges = super::config_value_ranges(contents);
    let mut offset = 0;
    let mut is_jev_section = false;
    for line in contents.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        if ranges.iter().any(|range| range.contains(&start)) {
            continue;
        }
        let body = line.trim().trim_start_matches('#').trim();
        if let Some((header, _)) = body.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
            is_jev_section = header == "analysis.jev";
        }
        if is_jev_section && super::file_mentions_key(line, key) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layer(body: &str) -> JevLayer {
        normalize(
            &toml::from_str::<toml::Value>(body).unwrap(),
            Path::new("fixture.toml"),
        )
    }
    #[test]
    fn test_jev_defaults_layers_threshold_and_invalid_values() {
        let defaults = merge(layer(""), layer(""));
        assert!(!defaults.is_overview_enabled && !defaults.is_search_filter_enabled);
        assert_eq!(defaults.policy(), crate::jev::Policy::default());
        assert_eq!(defaults.search_filter_min_unrelated_probability, 0.70);
        let config = merge(
            layer("overview_enabled = true\nsearch_filter_min_unrelated_probability = 0.90"),
            layer("search_filter_enabled = true\ntimeout_ms = 5000"),
        );
        assert!(config.is_overview_enabled && config.is_search_filter_enabled);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.search_filter_min_unrelated_probability, 0.90);
        for invalid in ["0.5", "0.0", "1.01", "nan", "inf", "'0.8'"] {
            let config = merge(
                layer(&format!(
                    "search_filter_min_unrelated_probability = {invalid}"
                )),
                layer("search_filter_min_unrelated_probability = 0.9"),
            );
            assert_eq!(config.search_filter_min_unrelated_probability, 0.9);
        }
        let config=merge(layer("timeout_ms=0\nmodel='jev-latest'\nmax_in_flight_requests=4\nrequest_spacing_ms=0\nmax_batch_bytes=90000\npool_idle_timeout_ms=31000\napi_key_env='bad key'"),layer(""));
        assert_eq!(config, defaults);
    }
    #[test]
    fn test_jev_migration_is_localized_additive_and_section_scoped() {
        let input="# codemap-config-version: 23\n[other]\nmodel='custom'\n[analysis.jev]\n# operator note\noverview_enabled=true\n# search_filter_min_unrelated_probability=0.9\n";
        for language in [
            crate::config_locale::ConfigCommentLanguage::English,
            crate::config_locale::ConfigCommentLanguage::Korean,
        ] {
            let output = super::super::apply_migrations_with_language(
                input,
                23,
                24,
                super::super::MIGRATIONS,
                language,
            )
            .unwrap();
            assert!(output.contains("# operator note\noverview_enabled=true"));
            assert!(output.contains("# model = \"jev-1.13.0\""));
            assert_eq!(
                output
                    .matches("search_filter_min_unrelated_probability")
                    .count(),
                1
            );
            assert!(output.contains("# timeout_ms = 45000"));
            assert!(super::super::apply_migrations_with_language(
                &output,
                24,
                24,
                super::super::MIGRATIONS,
                language
            )
            .is_none());
            let config = super::super::merge(
                super::super::normalize(
                    toml::from_str(&output).unwrap(),
                    Path::new("fixture.toml"),
                ),
                Default::default(),
            );
            assert!(config.jev.is_overview_enabled);
            assert_eq!(config.jev.search_filter_min_unrelated_probability, 0.7);
        }
    }
}
