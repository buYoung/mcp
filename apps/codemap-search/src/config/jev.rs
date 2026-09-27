//! `[analysis.jev]`: optional TypeSafe Jev selection for search/read/grep and non-root overview.
//! All filters are off by default; live tools inherit search unless explicitly configured.
//! Register the full task once through `initial_instructions`; enabled stages automatically
//! use it for eligible calls. The API key comes from the separate `auth.toml` file or,
//! as a fallback, the environment variable named by `api_key_env`, never from this
//! behavior section or tool arguments.
//!
//! Transport keys may only tighten the runtime's initial safety policy: at most
//! `MAX_IN_FLIGHT_REQUESTS` (3) requests in flight, at least `MIN_REQUEST_SPACING`
//! (300 ms) between request starts, at most `MAX_BATCH_BYTES` (168,000) encoded bytes per
//! batch. Timeouts must be positive and representable on the monotonic clock. Any invalid
//! value warns and falls back to the lower layer or the default, like every other section.

use std::path::Path;
use std::time::Duration;

pub const DEFAULT_API_KEY_ENV: &str = "TYPESAFE_API_KEY";

#[derive(Debug, Clone, PartialEq)]
pub struct JevConfig {
    /// Mode #2: filter complete declaration bodies in `search` details.
    pub search_filter_enabled: bool,
    /// Live body filters; omitted configuration inherits the resolved search flag.
    pub read_filter_enabled: bool,
    pub grep_filter_enabled: bool,
    /// Non-root declaration filter; omitted configuration inherits the search flag.
    pub overview_filter_enabled: bool,
    /// Concrete provider model, validated against every response.
    pub model: String,
    /// Environment variable used when repo/global `auth.toml` provides no API key.
    pub api_key_env: String,
    /// Whole-call deadline per tool call, including queue time.
    pub timeout_ms: u64,
    pub max_in_flight_requests: usize,
    pub request_spacing_ms: u64,
    pub max_batch_bytes: usize,
    pub pool_idle_timeout_ms: u64,
    /// Probability of a registered criterion being false (legacy key name); finite and in `(0.5, 1.0]`.
    pub search_filter_min_unrelated_probability: f64,
}

impl Default for JevConfig {
    fn default() -> Self {
        let evaluator = crate::jev::EvaluatorConfig::default();
        let https = crate::jev::HttpsSettings::default();
        Self {
            search_filter_enabled: false,
            read_filter_enabled: false,
            grep_filter_enabled: false,
            overview_filter_enabled: false,
            model: evaluator.model,
            api_key_env: DEFAULT_API_KEY_ENV.into(),
            timeout_ms: evaluator.deadline.as_millis() as u64,
            max_in_flight_requests: evaluator.max_in_flight_requests,
            request_spacing_ms: evaluator.request_spacing.as_millis() as u64,
            max_batch_bytes: evaluator.max_batch_bytes,
            pool_idle_timeout_ms: https.pool_idle_timeout.as_millis() as u64,
            search_filter_min_unrelated_probability:
                crate::tools::search::jev::DEFAULT_MIN_UNRELATED_PROBABILITY,
        }
    }
}

impl JevConfig {
    pub fn is_any_enabled(&self) -> bool {
        self.search_filter_enabled
            || self.read_filter_enabled
            || self.grep_filter_enabled
            || self.overview_filter_enabled
    }

    /// The runtime settings this configuration selects (validated by the evaluator).
    pub fn evaluator_config(&self) -> crate::jev::EvaluatorConfig {
        crate::jev::EvaluatorConfig {
            model: self.model.clone(),
            max_in_flight_requests: self.max_in_flight_requests,
            request_spacing: Duration::from_millis(self.request_spacing_ms),
            max_batch_bytes: self.max_batch_bytes,
            deadline: Duration::from_millis(self.timeout_ms),
            ..crate::jev::EvaluatorConfig::default()
        }
    }

    pub fn https_settings(&self) -> crate::jev::HttpsSettings {
        crate::jev::HttpsSettings {
            pool_idle_timeout: Duration::from_millis(self.pool_idle_timeout_ms),
            max_idle_connections: self.max_in_flight_requests.max(1),
            ..crate::jev::HttpsSettings::default()
        }
    }

    pub fn deadline(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

#[derive(Default, PartialEq)]
pub(super) struct JevLayer {
    search_filter_enabled: Option<bool>,
    read_filter_enabled: Option<bool>,
    grep_filter_enabled: Option<bool>,
    overview_filter_enabled: Option<bool>,
    model: Option<String>,
    api_key_env: Option<String>,
    timeout_ms: Option<u64>,
    max_in_flight_requests: Option<usize>,
    request_spacing_ms: Option<u64>,
    max_batch_bytes: Option<usize>,
    pool_idle_timeout_ms: Option<u64>,
    search_filter_min_unrelated_probability: Option<f64>,
}

fn as_env_var_name(value: &toml::Value, key: &str, path: &Path) -> Option<String> {
    match value.as_str() {
        Some(name)
            if !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') =>
        {
            Some(name.to_string())
        }
        _ => {
            super::warn(&format!(
                "config '{key}' must be an environment variable name (letters, digits, underscores): {} — ignored",
                path.display()
            ));
            None
        }
    }
}

/// A positive integer count of milliseconds that the runtime can turn into a deadline.
fn as_safe_timeout_ms(value: &toml::Value, key: &str, path: &Path) -> Option<u64> {
    match value.as_integer() {
        Some(n) if n > 0 && crate::jev::is_safe_duration(Duration::from_millis(n as u64)) => {
            Some(n as u64)
        }
        _ => {
            super::warn(&format!(
                "config '{key}' must be a positive integer of milliseconds within the supported time range: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_in_flight_requests(value: &toml::Value, key: &str, path: &Path) -> Option<usize> {
    match value.as_integer() {
        Some(n) if n >= 1 && n <= crate::jev::MAX_IN_FLIGHT_REQUESTS as i64 => Some(n as usize),
        _ => {
            super::warn(&format!(
                "config '{key}' must be an integer between 1 and {}: {} — ignored",
                crate::jev::MAX_IN_FLIGHT_REQUESTS,
                path.display()
            ));
            None
        }
    }
}

fn as_request_spacing_ms(value: &toml::Value, key: &str, path: &Path) -> Option<u64> {
    let minimum = crate::jev::MIN_REQUEST_SPACING.as_millis() as i64;
    match value.as_integer() {
        Some(n)
            if n >= minimum && crate::jev::is_safe_duration(Duration::from_millis(n as u64)) =>
        {
            Some(n as u64)
        }
        _ => {
            super::warn(&format!(
                "config '{key}' must be an integer of at least {minimum} milliseconds within the supported time range: {} — ignored",
                path.display()
            ));
            None
        }
    }
}

fn as_batch_bytes(value: &toml::Value, key: &str, path: &Path) -> Option<usize> {
    let bytes: usize = super::as_positive_byte_size(value, key, path)?;
    if bytes > crate::jev::MAX_BATCH_BYTES {
        super::warn(&format!(
            "config '{key}' must be at most {} bytes: {} — ignored",
            crate::jev::MAX_BATCH_BYTES,
            path.display()
        ));
        return None;
    }
    Some(bytes)
}

fn as_unrelated_probability(value: &toml::Value, key: &str, path: &Path) -> Option<f64> {
    let number = match value {
        toml::Value::Float(number) => Some(*number),
        toml::Value::Integer(number) => Some(*number as f64),
        _ => None,
    };
    match number.map(crate::tools::search::jev::validate_threshold) {
        Some(Ok(threshold)) => Some(threshold),
        _ => {
            super::warn(&format!(
                "config '{key}' must be a finite number above 0.5 and at most 1.0: {} — ignored",
                path.display()
            ));
            None
        }
    }
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
        let label = format!("analysis.jev.{key}");
        match key.as_str() {
            "search_filter_enabled" => {
                layer.search_filter_enabled = super::as_bool(value, &label, path)
            }
            "read_filter_enabled" => {
                layer.read_filter_enabled = super::as_bool(value, &label, path)
            }
            "grep_filter_enabled" => {
                layer.grep_filter_enabled = super::as_bool(value, &label, path)
            }
            "overview_filter_enabled" => {
                layer.overview_filter_enabled = super::as_bool(value, &label, path)
            }
            "model" => layer.model = super::as_nonempty_string(value, &label, path),
            "api_key_env" => layer.api_key_env = as_env_var_name(value, &label, path),
            "timeout_ms" => layer.timeout_ms = as_safe_timeout_ms(value, &label, path),
            "max_in_flight_requests" => {
                layer.max_in_flight_requests = as_in_flight_requests(value, &label, path)
            }
            "request_spacing_ms" => {
                layer.request_spacing_ms = as_request_spacing_ms(value, &label, path)
            }
            "max_batch_bytes" => layer.max_batch_bytes = as_batch_bytes(value, &label, path),
            "pool_idle_timeout_ms" => {
                layer.pool_idle_timeout_ms = as_safe_timeout_ms(value, &label, path)
            }
            "search_filter_min_unrelated_probability" => {
                layer.search_filter_min_unrelated_probability =
                    as_unrelated_probability(value, &label, path)
            }
            _ => super::warn(&format!(
                "unknown config key '{label}': {} — ignored",
                path.display()
            )),
        }
    }
    layer
}

pub(super) fn merge(repo: JevLayer, global: JevLayer) -> JevConfig {
    let defaults = JevConfig::default();
    let search_filter_enabled = repo
        .search_filter_enabled
        .or(global.search_filter_enabled)
        .unwrap_or(defaults.search_filter_enabled);
    JevConfig {
        search_filter_enabled,
        read_filter_enabled: repo
            .read_filter_enabled
            .or(global.read_filter_enabled)
            .unwrap_or(search_filter_enabled),
        grep_filter_enabled: repo
            .grep_filter_enabled
            .or(global.grep_filter_enabled)
            .unwrap_or(search_filter_enabled),
        overview_filter_enabled: repo
            .overview_filter_enabled
            .or(global.overview_filter_enabled)
            .unwrap_or(search_filter_enabled),
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

    fn layer(text: &str) -> JevLayer {
        let value: toml::Value = toml::from_str(text).unwrap();
        normalize(&value, Path::new("config.toml"))
    }

    #[test]
    fn defaults_are_off_and_mirror_the_runtime_ceilings() {
        let config = merge(JevLayer::default(), JevLayer::default());
        assert!(!config.search_filter_enabled);
        assert!(!config.is_any_enabled());
        assert_eq!(config.model, crate::jev::DEFAULT_MODEL);
        assert_eq!(config.api_key_env, "TYPESAFE_API_KEY");
        assert_eq!(config.timeout_ms, 45_000);
        assert_eq!(config.max_in_flight_requests, 3);
        assert_eq!(config.request_spacing_ms, 300);
        assert_eq!(config.max_batch_bytes, 168_000);
        assert_eq!(config.pool_idle_timeout_ms, 30_000);
        assert_eq!(config.search_filter_min_unrelated_probability, 0.70);
        assert!(config.evaluator_config().validate().is_ok());
        assert_eq!(config.deadline(), Duration::from_millis(45_000));
    }

    #[test]
    fn layers_merge_per_key_with_repo_precedence() {
        let global = layer(
            "search_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.9\nmodel = 'jev-1.12.0'\n",
        );
        let repo =
            layer("search_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.75\n");
        let config = merge(repo, global);
        assert!(config.search_filter_enabled);
        assert_eq!(config.search_filter_min_unrelated_probability, 0.75);
        assert_eq!(config.model, "jev-1.12.0");
    }

    #[test]
    fn invalid_values_warn_and_fall_back_to_the_lower_layer() {
        let global = layer("search_filter_min_unrelated_probability = 0.9\ntimeout_ms = 1000\n");
        for invalid in [
            "search_filter_min_unrelated_probability = 0.5\n",
            "search_filter_min_unrelated_probability = 1.01\n",
            "search_filter_min_unrelated_probability = 'high'\n",
            "search_filter_min_unrelated_probability = nan\n",
            "search_filter_min_unrelated_probability = inf\n",
            "search_filter_min_unrelated_probability = -0.7\n",
        ] {
            let config = merge(
                layer(invalid),
                layer("search_filter_min_unrelated_probability = 0.9\n"),
            );
            assert_eq!(
                config.search_filter_min_unrelated_probability, 0.9,
                "{invalid:?} must be ignored"
            );
        }
        assert_eq!(
            merge(
                layer("search_filter_min_unrelated_probability = 1\n"),
                JevLayer::default()
            )
            .search_filter_min_unrelated_probability,
            1.0,
            "an integer 1 is the inclusive upper bound"
        );
        let config = merge(
            layer("timeout_ms = 0\napi_key_env = 'not a name'\nmax_batch_bytes = 168001\nrequest_spacing_ms = 299\nmax_in_flight_requests = 4\npool_idle_timeout_ms = -5\nmodel = ''\n"),
            global,
        );
        assert_eq!(config.timeout_ms, 1000);
        assert_eq!(config.api_key_env, "TYPESAFE_API_KEY");
        assert_eq!(config.max_batch_bytes, 168_000);
        assert_eq!(config.request_spacing_ms, 300);
        assert_eq!(config.max_in_flight_requests, 3);
        assert_eq!(config.pool_idle_timeout_ms, 30_000);
        assert_eq!(config.model, crate::jev::DEFAULT_MODEL);
        // Time values must stay representable on the monotonic clock.
        let config = merge(
            layer(&format!(
                "timeout_ms = {}\npool_idle_timeout_ms = {}\n",
                i64::MAX,
                i64::MAX
            )),
            JevLayer::default(),
        );
        assert_eq!(config.timeout_ms, 45_000);
        assert_eq!(config.pool_idle_timeout_ms, 30_000);
        // Tightening within the policy is accepted: one request in flight, a second
        // between starts, a 2 KB batch, a one-byte batch.
        let config = merge(
            layer(
                "request_spacing_ms = 1000\nmax_in_flight_requests = 1\nmax_batch_bytes = '2kb'\napi_key_env = 'MY_KEY_1'\n"
            ),
            JevLayer::default(),
        );
        assert_eq!(config.request_spacing_ms, 1000);
        assert_eq!(config.max_in_flight_requests, 1);
        assert_eq!(config.max_batch_bytes, 2048);
        assert_eq!(config.api_key_env, "MY_KEY_1");
        assert!(config.evaluator_config().validate().is_ok());
        assert_eq!(
            merge(layer("max_batch_bytes = 1\n"), JevLayer::default()).max_batch_bytes,
            1
        );
    }

    #[test]
    fn a_non_table_section_is_ignored() {
        let value: toml::Value = toml::from_str("jev = 3\n").unwrap();
        assert!(normalize(&value["jev"], Path::new("config.toml")) == JevLayer::default());
    }
}
