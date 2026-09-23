//! `[analysis.jev]`: optional TypeSafe Jev judgments for MCP `overview` and `search`.
//!
//! Both modes default to off and every key resolves independently as `repo > global >
//! default`, except `api_key_env`: a repository file cannot choose which environment variable
//! is sent to the provider as a credential, so only the global file sets it. The file never
//! holds the key itself. Transport ranges match `TransportPolicy::validate` and the search
//! threshold uses the filter's own validation, so an accepted value is always runnable.

use std::path::Path;
use std::time::Duration;

use crate::jev::{
    is_versioned_model, TransportPolicy, DEFAULT_MAX_BATCH_BYTES, DEFAULT_MAX_IN_FLIGHT_REQUESTS,
    DEFAULT_MODEL, DEFAULT_POOL_IDLE_TIMEOUT, DEFAULT_REQUEST_SPACING, DEFAULT_TIMEOUT,
    MAX_BATCH_BYTES_LIMIT, MAX_IN_FLIGHT_REQUESTS_LIMIT, MAX_POOL_IDLE_TIMEOUT,
    MAX_REQUEST_SPACING, MAX_TIMEOUT, MIN_BATCH_BYTES,
};
use crate::tools::search::jev::{
    validate_min_unrelated_probability, DEFAULT_MIN_UNRELATED_PROBABILITY,
};

/// Section path of these settings.
pub(super) const SECTION: &str = "analysis.jev";
/// Environment variable holding the API key unless the global `api_key_env` names another.
pub const DEFAULT_API_KEY_ENV: &str = "TYPESAFE_API_KEY";
const MAX_API_KEY_ENV_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub struct JevConfig {
    /// Root overview file and declaration recommendations (mode #1).
    pub is_overview_enabled: bool,
    /// Omission of displayed search bodies judged unrelated to the task (mode #2).
    pub is_search_filter_enabled: bool,
    /// Pinned versioned model id.
    pub model: String,
    /// Name of the environment variable holding the API key; never the key itself.
    pub api_key_env: String,
    /// Deadline for one tool call's evaluation, including queueing and spacing.
    pub timeout_ms: u64,
    pub max_in_flight_requests: usize,
    pub request_spacing_ms: u64,
    pub max_batch_bytes: usize,
    pub pool_idle_timeout_ms: u64,
    /// Provisional and uncalibrated: a displayed body is omitted only when its Noul
    /// P(unrelated) reaches this value. Finite, `0.5 < value <= 1.0`.
    pub search_filter_min_unrelated_probability: f64,
}

impl Default for JevConfig {
    fn default() -> Self {
        Self {
            is_overview_enabled: false,
            is_search_filter_enabled: false,
            model: DEFAULT_MODEL.to_string(),
            api_key_env: DEFAULT_API_KEY_ENV.to_string(),
            timeout_ms: DEFAULT_TIMEOUT.as_millis() as u64,
            max_in_flight_requests: DEFAULT_MAX_IN_FLIGHT_REQUESTS,
            request_spacing_ms: DEFAULT_REQUEST_SPACING.as_millis() as u64,
            max_batch_bytes: DEFAULT_MAX_BATCH_BYTES,
            pool_idle_timeout_ms: DEFAULT_POOL_IDLE_TIMEOUT.as_millis() as u64,
            search_filter_min_unrelated_probability: DEFAULT_MIN_UNRELATED_PROBABILITY,
        }
    }
}

impl JevConfig {
    pub fn transport_policy(&self) -> TransportPolicy {
        TransportPolicy {
            model: self.model.clone(),
            timeout: Duration::from_millis(self.timeout_ms),
            max_in_flight_requests: self.max_in_flight_requests,
            request_spacing: Duration::from_millis(self.request_spacing_ms),
            max_batch_bytes: self.max_batch_bytes,
            pool_idle_timeout: Duration::from_millis(self.pool_idle_timeout_ms),
        }
    }
}

#[derive(Default, PartialEq)]
pub(super) struct JevLayer {
    is_overview_enabled: Option<bool>,
    is_search_filter_enabled: Option<bool>,
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
            "config '{SECTION}' must be a table: {} — ignored",
            path.display()
        ));
        return layer;
    };
    for (key, value) in table {
        let full = format!("{SECTION}.{key}");
        match key.as_str() {
            "is_overview_enabled" => layer.is_overview_enabled = super::as_bool(value, &full, path),
            "is_search_filter_enabled" => {
                layer.is_search_filter_enabled = super::as_bool(value, &full, path)
            }
            "model" => layer.model = model(value, &full, path),
            "api_key_env" => layer.api_key_env = api_key_env(value, &full, path),
            "timeout_ms" => layer.timeout_ms = milliseconds(value, &full, path, 1, MAX_TIMEOUT),
            "max_in_flight_requests" => {
                layer.max_in_flight_requests =
                    integer_in(value, &full, path, 1, MAX_IN_FLIGHT_REQUESTS_LIMIT as u64)
                        .map(|count| count as usize)
            }
            "request_spacing_ms" => {
                layer.request_spacing_ms = milliseconds(value, &full, path, 0, MAX_REQUEST_SPACING)
            }
            "max_batch_bytes" => layer.max_batch_bytes = batch_bytes(value, &full, path),
            "pool_idle_timeout_ms" => {
                layer.pool_idle_timeout_ms =
                    milliseconds(value, &full, path, 1, MAX_POOL_IDLE_TIMEOUT)
            }
            "search_filter_min_unrelated_probability" => {
                layer.search_filter_min_unrelated_probability = probability(value, &full, path)
            }
            _ => super::warn(&format!(
                "unknown config key '{full}': {} — ignored",
                path.display()
            )),
        }
    }
    layer
}

/// Warns about a repository `api_key_env`; [`merge`] reads that key from the global layer only.
pub(super) fn warn_repository_credential_source(repo: &JevLayer, path: &Path) {
    if repo.api_key_env.is_some() {
        super::warn(&format!(
            "config '{SECTION}.api_key_env' is read only from the global config: {} — ignored",
            path.display()
        ));
    }
}

pub(super) fn merge(repo: JevLayer, global: JevLayer) -> JevConfig {
    let defaults = JevConfig::default();
    JevConfig {
        is_overview_enabled: repo
            .is_overview_enabled
            .or(global.is_overview_enabled)
            .unwrap_or(defaults.is_overview_enabled),
        is_search_filter_enabled: repo
            .is_search_filter_enabled
            .or(global.is_search_filter_enabled)
            .unwrap_or(defaults.is_search_filter_enabled),
        model: repo.model.or(global.model).unwrap_or(defaults.model),
        api_key_env: global.api_key_env.unwrap_or(defaults.api_key_env),
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

fn model(value: &toml::Value, key: &str, path: &Path) -> Option<String> {
    let model = value.as_str().filter(|model| is_versioned_model(model));
    if model.is_none() {
        super::warn(&format!(
            "config '{key}' must be a versioned model id such as \"jev-1.13.0\"; aliases are rejected: {} — ignored",
            path.display()
        ));
    }
    model.map(ToString::to_string)
}

/// A portable variable name (`A-Z`, `0-9`, `_`). The value is never echoed: a key pasted
/// here by mistake must not reach logs.
fn api_key_env(value: &toml::Value, key: &str, path: &Path) -> Option<String> {
    let name = value.as_str().filter(|name| {
        name.len() <= MAX_API_KEY_ENV_BYTES
            && name
                .bytes()
                .next()
                .is_some_and(|first| first.is_ascii_uppercase() || first == b'_')
            && name
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    });
    if name.is_none() {
        super::warn(&format!(
            "config '{key}' must name an environment variable with A-Z, 0-9 and _ such as \"{DEFAULT_API_KEY_ENV}\"; put the key in that variable, never in config: {} — ignored",
            path.display()
        ));
    }
    name.map(ToString::to_string)
}

fn integer_in(value: &toml::Value, key: &str, path: &Path, min: u64, max: u64) -> Option<u64> {
    let number = value
        .as_integer()
        .and_then(|number| u64::try_from(number).ok())
        .filter(|number| (min..=max).contains(number));
    if number.is_none() {
        super::warn(&format!(
            "config '{key}' must be an integer from {min} to {max}: {} — ignored",
            path.display()
        ));
    }
    number
}

fn milliseconds(
    value: &toml::Value,
    key: &str,
    path: &Path,
    min_ms: u64,
    max: Duration,
) -> Option<u64> {
    integer_in(value, key, path, min_ms, max.as_millis() as u64)
}

fn batch_bytes(value: &toml::Value, key: &str, path: &Path) -> Option<usize> {
    let bytes = super::parse_byte_size(value)
        .filter(|bytes| (MIN_BATCH_BYTES as u64..=MAX_BATCH_BYTES_LIMIT as u64).contains(bytes))
        .map(|bytes| bytes as usize);
    if bytes.is_none() {
        super::warn(&format!(
            "config '{key}' must be a byte count from {MIN_BATCH_BYTES} to {MAX_BATCH_BYTES_LIMIT}, or a quoted size using b/kb/mb/gb in that range: {} — ignored",
            path.display()
        ));
    }
    bytes
}

fn probability(value: &toml::Value, key: &str, path: &Path) -> Option<f64> {
    let probability = value
        .as_float()
        .or_else(|| value.as_integer().map(|number| number as f64))
        .filter(|probability| validate_min_unrelated_probability(*probability).is_ok());
    if probability.is_none() {
        super::warn(&format!(
            "config '{key}' must be a finite number with 0.5 < value <= 1.0: {} — ignored",
            path.display()
        ));
    }
    probability
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(body: &str) -> JevLayer {
        normalize(
            &toml::from_str::<toml::Value>(body).unwrap(),
            Path::new("config.toml"),
        )
    }

    #[test]
    fn both_modes_are_off_and_the_policy_matches_the_runtime_defaults() {
        let resolved = merge(layer(""), layer(""));
        assert_eq!(resolved, JevConfig::default());
        assert!(!resolved.is_overview_enabled);
        assert!(!resolved.is_search_filter_enabled);
        assert_eq!(resolved.api_key_env, "TYPESAFE_API_KEY");
        assert_eq!(resolved.search_filter_min_unrelated_probability, 0.70);
        assert_eq!(resolved.transport_policy(), TransportPolicy::default());
    }

    #[test]
    fn modes_resolve_independently_per_key() {
        let resolved = merge(
            layer("is_search_filter_enabled = true\nsearch_filter_min_unrelated_probability = 0.9"),
            layer(
                "is_overview_enabled = true\nis_search_filter_enabled = false\ntimeout_ms = 1000",
            ),
        );
        assert!(resolved.is_overview_enabled);
        assert!(resolved.is_search_filter_enabled);
        assert_eq!(resolved.search_filter_min_unrelated_probability, 0.9);
        assert_eq!(resolved.timeout_ms, 1000);
        let only_overview = merge(layer("is_overview_enabled = true"), layer(""));
        assert!(only_overview.is_overview_enabled && !only_overview.is_search_filter_enabled);
    }

    #[test]
    fn threshold_accepts_only_finite_values_above_one_half_up_to_one() {
        for accepted in ["0.70", "0.9", "1.0", "1", "0.5000001"] {
            let resolved = merge(
                layer(&format!(
                    "search_filter_min_unrelated_probability = {accepted}"
                )),
                layer(""),
            );
            assert_eq!(
                resolved.search_filter_min_unrelated_probability,
                accepted.parse::<f64>().unwrap(),
                "{accepted}"
            );
        }
        for rejected in [
            "0.5", "0.2", "1.01", "2", "-0.9", "nan", "inf", "-inf", "\"0.9\"", "true",
        ] {
            let repo = format!("search_filter_min_unrelated_probability = {rejected}");
            let inherited = merge(
                layer(&repo),
                layer("search_filter_min_unrelated_probability = 0.8"),
            );
            assert_eq!(
                inherited.search_filter_min_unrelated_probability, 0.8,
                "{rejected}"
            );
            let defaulted = merge(layer(&repo), layer(""));
            assert_eq!(
                defaulted.search_filter_min_unrelated_probability, 0.70,
                "{rejected}"
            );
        }
    }

    #[test]
    fn transport_keys_stay_within_the_runtime_ranges() {
        let resolved = merge(
            layer(
                "model = \"jev-1.14.2\"\ntimeout_ms = 600000\nmax_in_flight_requests = 16\nrequest_spacing_ms = 0\nmax_batch_bytes = \"64kb\"\npool_idle_timeout_ms = 1",
            ),
            layer(""),
        );
        assert_eq!(resolved.model, "jev-1.14.2");
        assert_eq!(resolved.request_spacing_ms, 0);
        assert_eq!(resolved.max_batch_bytes, 64 * 1024);
        assert!(resolved.transport_policy().validate().is_ok());
        for invalid in [
            "model = \"jev-latest\"",
            "model = 1",
            "timeout_ms = 0",
            "timeout_ms = 600001",
            "max_in_flight_requests = 0",
            "max_in_flight_requests = 17",
            "request_spacing_ms = -1",
            "request_spacing_ms = 10001",
            "max_batch_bytes = 4095",
            "max_batch_bytes = \"1mb\"",
            "pool_idle_timeout_ms = 0",
            "timeout_ms = 1.5",
        ] {
            let resolved = merge(layer(invalid), layer(""));
            assert_eq!(resolved, JevConfig::default(), "{invalid}");
        }
    }

    #[test]
    fn only_the_global_layer_names_the_key_variable() {
        let global = layer("api_key_env = \"CODEMAP_JEV_KEY\"");
        assert_eq!(merge(layer(""), global).api_key_env, "CODEMAP_JEV_KEY");
        let repo = layer("api_key_env = \"GITHUB_TOKEN\"");
        assert_eq!(merge(repo, layer("")).api_key_env, "TYPESAFE_API_KEY");
        for invalid in [
            "\"\"",
            "\"typesafe_key\"",
            "\"1KEY\"",
            "\"KEY-NAME\"",
            "\"sk live\"",
            "42",
        ] {
            let resolved = merge(layer(""), layer(&format!("api_key_env = {invalid}")));
            assert_eq!(resolved.api_key_env, "TYPESAFE_API_KEY", "{invalid}");
        }
    }
}
