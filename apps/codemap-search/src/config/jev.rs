use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct JevConfig {
    pub overview_enabled: bool,
    pub search_filter_enabled: bool,
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
        Self {
            overview_enabled: false,
            search_filter_enabled: false,
            model: crate::jev::MODEL.into(),
            api_key_env: "TYPESAFE_API_KEY".into(),
            timeout_ms: 45_000,
            max_in_flight_requests: 3,
            request_spacing_ms: 300,
            max_batch_bytes: 80_000,
            pool_idle_timeout_ms: 30_000,
            search_filter_min_unrelated_probability: 0.70,
        }
    }
}

#[derive(Default, PartialEq)]
pub(super) struct JevLayer {
    overview_enabled: Option<bool>,
    search_filter_enabled: Option<bool>,
    model: Option<String>,
    api_key_env: Option<String>,
    timeout_ms: Option<u64>,
    max_in_flight_requests: Option<usize>,
    request_spacing_ms: Option<u64>,
    max_batch_bytes: Option<usize>,
    pool_idle_timeout_ms: Option<u64>,
    search_filter_min_unrelated_probability: Option<f64>,
}

pub(super) fn normalize(value: &toml::Value, path: &Path) -> JevLayer {
    let mut layer = JevLayer::default();
    let Some(table) = value.as_table() else {
        super::warn(&format!(
            "config 'analysis.jev' must be a table: {} — ignored",
            path.display()
        ));
        return layer;
    };
    for (key, value) in table {
        let is_valid = match key.as_str() {
            "overview_enabled" => value
                .as_bool()
                .map(|v| layer.overview_enabled = Some(v))
                .is_some(),
            "search_filter_enabled" => value
                .as_bool()
                .map(|v| layer.search_filter_enabled = Some(v))
                .is_some(),
            "model" => value
                .as_str()
                .filter(|v| *v == crate::jev::MODEL)
                .map(|v| layer.model = Some(v.into()))
                .is_some(),
            "api_key_env" => value
                .as_str()
                .filter(|v| {
                    let mut chars = v.chars();
                    chars
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
                .map(|v| layer.api_key_env = Some(v.into()))
                .is_some(),
            "timeout_ms" => integer(value, 1, 45_000)
                .map(|v| layer.timeout_ms = Some(v))
                .is_some(),
            "max_in_flight_requests" => integer(value, 1, 3)
                .map(|v| layer.max_in_flight_requests = Some(v as usize))
                .is_some(),
            "request_spacing_ms" => integer(value, 300, 60_000)
                .map(|v| layer.request_spacing_ms = Some(v))
                .is_some(),
            "max_batch_bytes" => integer(value, 1, 80_000)
                .map(|v| layer.max_batch_bytes = Some(v as usize))
                .is_some(),
            "pool_idle_timeout_ms" => integer(value, 1, 30_000)
                .map(|v| layer.pool_idle_timeout_ms = Some(v))
                .is_some(),
            "search_filter_min_unrelated_probability" => value
                .as_float()
                .or_else(|| value.as_integer().map(|v| v as f64))
                .filter(|v| v.is_finite() && *v > 0.5 && *v <= 1.0)
                .map(|v| layer.search_filter_min_unrelated_probability = Some(v))
                .is_some(),
            _ => {
                super::warn(&format!(
                    "unknown config key 'analysis.jev.{key}': {} — ignored",
                    path.display()
                ));
                continue;
            }
        };
        if !is_valid {
            super::warn(&format!(
                "invalid config key 'analysis.jev.{key}': {} — using lower layer/default",
                path.display()
            ));
        }
    }
    layer
}

fn integer(value: &toml::Value, minimum: u64, maximum: u64) -> Option<u64> {
    value
        .as_integer()
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| (*value >= minimum) && (*value <= maximum))
}

pub(super) fn merge(repo: JevLayer, global: JevLayer) -> JevConfig {
    let defaults = JevConfig::default();
    JevConfig {
        overview_enabled: repo
            .overview_enabled
            .or(global.overview_enabled)
            .unwrap_or(defaults.overview_enabled),
        search_filter_enabled: repo
            .search_filter_enabled
            .or(global.search_filter_enabled)
            .unwrap_or(defaults.search_filter_enabled),
        model: repo.model.or(global.model).unwrap_or(defaults.model),
        api_key_env: repo
            .api_key_env
            .or(global.api_key_env)
            .unwrap_or(defaults.api_key_env),
        timeout_ms: repo
            .timeout_ms
            .or(global.timeout_ms)
            .unwrap_or(defaults.timeout_ms),
        max_in_flight_requests: repo
            .max_in_flight_requests
            .or(global.max_in_flight_requests)
            .unwrap_or(defaults.max_in_flight_requests),
        request_spacing_ms: repo
            .request_spacing_ms
            .or(global.request_spacing_ms)
            .unwrap_or(defaults.request_spacing_ms),
        max_batch_bytes: repo
            .max_batch_bytes
            .or(global.max_batch_bytes)
            .unwrap_or(defaults.max_batch_bytes),
        pool_idle_timeout_ms: repo
            .pool_idle_timeout_ms
            .or(global.pool_idle_timeout_ms)
            .unwrap_or(defaults.pool_idle_timeout_ms),
        search_filter_min_unrelated_probability: repo
            .search_filter_min_unrelated_probability
            .or(global.search_filter_min_unrelated_probability)
            .unwrap_or(defaults.search_filter_min_unrelated_probability),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_independent_and_off() {
        let defaults = JevConfig::default();
        assert!(!defaults.overview_enabled);
        assert!(!defaults.search_filter_enabled);
        assert_eq!(defaults.model, crate::jev::MODEL);
        assert_eq!(defaults.search_filter_min_unrelated_probability, 0.70);
        assert_eq!(defaults.api_key_env, "TYPESAFE_API_KEY");
    }

    #[test]
    fn threshold_and_transport_policy_use_per_key_precedence() {
        let global: toml::Value = toml::from_str(
            "overview_enabled = true\nsearch_filter_min_unrelated_probability = 0.90\n",
        )
        .unwrap();
        let repo: toml::Value = toml::from_str(
            "search_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.70\n",
        )
        .unwrap();
        let resolved = merge(
            normalize(&repo, Path::new("<repo>")),
            normalize(&global, Path::new("<global>")),
        );
        assert!(resolved.overview_enabled);
        assert!(resolved.search_filter_enabled);
        assert_eq!(resolved.search_filter_min_unrelated_probability, 0.70);
    }

    #[test]
    fn invalid_threshold_and_unsafe_limits_inherit_defaults() {
        for threshold in ["0.5", "1.1", "nan", "inf", "\"invalid\""] {
            let value: toml::Value = toml::from_str(&format!(
                "search_filter_min_unrelated_probability = {threshold}\nmax_in_flight_requests = 4\nrequest_spacing_ms = 0\n"
            )).unwrap();
            let resolved = merge(normalize(&value, Path::new("<repo>")), JevLayer::default());
            assert_eq!(resolved.search_filter_min_unrelated_probability, 0.70);
            assert_eq!(resolved.max_in_flight_requests, 3);
            assert_eq!(resolved.request_spacing_ms, 300);
        }
    }
}
