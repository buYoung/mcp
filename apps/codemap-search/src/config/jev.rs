//! `[output.jev]`: optional TypeSafe Jev selection for search/read/grep and non-root overview.
//! `enabled` is the master switch; `scope` selects eligible output tools. Off by default.
//! Register the full task once through `register_task`; enabled stages automatically
//! use it for eligible calls. The API key comes from the separate `auth.toml` file or,
//! as a fallback, the fixed `TYPESAFE_API_KEY` environment variable, never from this
//! behavior section or tool arguments.
//!
//! Transport keys may only tighten the runtime's initial safety policy: at most
//! `MAX_IN_FLIGHT_REQUESTS` (3) requests in flight, at least `MIN_REQUEST_SPACING`
//! (300 ms) between request starts, at most `MAX_BATCH_BYTES` (168,000) encoded bytes per
//! batch. Timeouts must be positive and representable on the monotonic clock. Any invalid
//! value warns and falls back to the lower layer or the default, like every other section.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

pub(super) const TOOLS: [&str; 4] = ["overview", "search", "read", "grep"];

#[derive(Debug, Clone, PartialEq)]
pub struct JevConfig {
    /// Master switch for every Jev output filter.
    pub is_enabled: bool,
    /// Tool names eligible for filtering when the master switch is on.
    pub scope: Vec<String>,
    /// Concrete provider model, validated against every response.
    pub model: String,
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
            is_enabled: false,
            scope: TOOLS.into_iter().map(str::to_string).collect(),
            model: evaluator.model,
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
        self.is_enabled && !self.scope.is_empty()
    }

    pub fn is_enabled_for(&self, tool: &str) -> bool {
        self.is_enabled && self.scope.iter().any(|name| name == tool)
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
    is_enabled: Option<bool>,
    scope: Option<Vec<String>>,
    /// Read compatibility only; never used by runtime consumers or new templates.
    legacy_filters: BTreeMap<String, bool>,
    model: Option<String>,
    timeout_ms: Option<u64>,
    max_in_flight_requests: Option<usize>,
    request_spacing_ms: Option<u64>,
    max_batch_bytes: Option<usize>,
    pool_idle_timeout_ms: Option<u64>,
    search_filter_min_unrelated_probability: Option<f64>,
}

fn as_scope(value: &toml::Value, path: &Path) -> Option<Vec<String>> {
    let Some(values) = value.as_array() else {
        super::warn(&format!(
            "config 'output.jev.scope' must be a string array: {} — ignored",
            path.display()
        ));
        return None;
    };
    let mut scope = Vec::new();
    for value in values {
        let Some(tool) = value.as_str().filter(|tool| TOOLS.contains(tool)) else {
            super::warn(&format!(
                "config 'output.jev.scope' accepts only overview, search, read and grep: {} — ignored",
                path.display()
            ));
            return None;
        };
        if !scope.iter().any(|name| name == tool) {
            scope.push(tool.to_string());
        }
    }
    Some(scope)
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
            "config 'output.jev' must be a table: {} — ignored",
            path.display()
        ));
        return layer;
    };
    for (key, value) in table {
        let label = format!("output.jev.{key}");
        match key.as_str() {
            "enabled" => layer.is_enabled = super::as_bool(value, &label, path),
            "scope" => layer.scope = as_scope(value, path),
            legacy
                if legacy
                    .strip_suffix("_filter_enabled")
                    .is_some_and(|tool| TOOLS.contains(&tool)) =>
            {
                if let Some(is_enabled) = super::as_bool(value, &label, path) {
                    let tool = legacy.trim_end_matches("_filter_enabled");
                    layer.legacy_filters.insert(tool.to_string(), is_enabled);
                }
            }
            "model" => layer.model = super::as_nonempty_string(value, &label, path),
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

/// Preserve old per-tool inheritance until an existing file is migrated to enabled/scope.
fn legacy_selection(filters: &BTreeMap<String, bool>) -> (bool, Vec<String>) {
    let is_search_enabled = filters.get("search").copied().unwrap_or(false);
    let scope: Vec<String> = TOOLS
        .into_iter()
        .filter(|tool| filters.get(*tool).copied().unwrap_or(is_search_enabled))
        .map(str::to_string)
        .collect();
    if scope.is_empty() {
        (false, TOOLS.into_iter().map(str::to_string).collect())
    } else {
        (true, scope)
    }
}

fn selection(repo: &JevLayer, global: &JevLayer) -> (bool, Vec<String>) {
    let (is_global_legacy_enabled, global_legacy_scope) = legacy_selection(&global.legacy_filters);
    let is_global_enabled = global.is_enabled.unwrap_or(is_global_legacy_enabled);
    let global_scope = global.scope.clone().unwrap_or(global_legacy_scope);
    let (is_inherited_enabled, inherited_scope) = if repo.legacy_filters.is_empty() {
        (is_global_enabled, global_scope)
    } else {
        let mut filters = if global.is_enabled.is_some() || global.scope.is_some() {
            TOOLS
                .into_iter()
                .map(|tool| {
                    (
                        tool.to_string(),
                        is_global_enabled && global_scope.iter().any(|name| name == tool),
                    )
                })
                .collect()
        } else {
            global.legacy_filters.clone()
        };
        filters.extend(repo.legacy_filters.clone());
        legacy_selection(&filters)
    };
    (
        repo.is_enabled.unwrap_or(is_inherited_enabled),
        repo.scope.clone().unwrap_or(inherited_scope),
    )
}

pub(super) fn merge(repo: JevLayer, global: JevLayer) -> JevConfig {
    let defaults = JevConfig::default();
    let (is_enabled, scope) = selection(&repo, &global);
    JevConfig {
        is_enabled,
        scope,
        model: repo.model.or(global.model).unwrap_or(defaults.model),
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
        assert!(!config.is_enabled);
        assert_eq!(config.scope, TOOLS);
        assert!(!config.is_any_enabled());
        assert_eq!(config.model, crate::jev::DEFAULT_MODEL);
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
            "enabled = true\nscope = ['read', 'grep']\nsearch_filter_min_unrelated_probability = 0.9\nmodel = 'jev-1.12.0'\n",
        );
        let repo = layer("scope = ['search']\nsearch_filter_min_unrelated_probability = 0.75\n");
        let config = merge(repo, global);
        assert!(config.is_enabled);
        assert!(config.is_enabled_for("search"));
        assert!(!config.is_enabled_for("read"));
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
            layer("timeout_ms = 0\nmax_batch_bytes = 168001\nrequest_spacing_ms = 299\nmax_in_flight_requests = 4\npool_idle_timeout_ms = -5\nmodel = ''\n"),
            global,
        );
        assert_eq!(config.timeout_ms, 1000);
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
                "request_spacing_ms = 1000\nmax_in_flight_requests = 1\nmax_batch_bytes = '2kb'\n",
            ),
            JevLayer::default(),
        );
        assert_eq!(config.request_spacing_ms, 1000);
        assert_eq!(config.max_in_flight_requests, 1);
        assert_eq!(config.max_batch_bytes, 2048);
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
