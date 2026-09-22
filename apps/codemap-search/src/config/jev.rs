//! Independent opt-in Jev activation and request policy; credentials remain in the host env.
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
        Self { overview_enabled:false, search_filter_enabled:false,
            model:crate::jev::MODEL.into(), api_key_env:"TYPESAFE_API_KEY".into(), timeout_ms:45_000,
            max_in_flight_requests:3, request_spacing_ms:300, max_batch_bytes:80_000,
            pool_idle_timeout_ms:30_000, search_filter_min_unrelated_probability:0.70 }
    }
}
impl JevConfig {
    pub fn policy(&self) -> crate::jev::Policy {
        crate::jev::Policy { deadline:std::time::Duration::from_millis(self.timeout_ms),
            max_batch_bytes:self.max_batch_bytes, max_in_flight_requests:self.max_in_flight_requests,
            request_spacing:std::time::Duration::from_millis(self.request_spacing_ms),
            pool_idle_timeout:std::time::Duration::from_millis(self.pool_idle_timeout_ms) }
    }
}
#[derive(Default, PartialEq)]
pub(super) struct JevLayer {
    overview_enabled: Option<bool>, search_filter_enabled: Option<bool>,
    model: Option<String>, api_key_env: Option<String>, timeout_ms: Option<u64>,
    max_in_flight_requests: Option<usize>, request_spacing_ms: Option<u64>,
    max_batch_bytes: Option<usize>, pool_idle_timeout_ms: Option<u64>,
    search_filter_min_unrelated_probability: Option<f64>,
}

pub(super) fn assign(layer: &mut JevLayer, key: &str, value: &toml::Value, display: &str, path: &Path) -> bool {
    let invalid = || super::warn(&format!("config '{display}' is invalid: {} — ignored", path.display()));
    match key {
        "overview_enabled" => layer.overview_enabled = super::as_bool(value,display,path),
        "search_filter_enabled" => layer.search_filter_enabled = super::as_bool(value,display,path),
        "model" => { layer.model = match value.as_str() { Some(crate::jev::MODEL) => Some(crate::jev::MODEL.into()), _ => { invalid(); None } }; }
        "api_key_env" => { layer.api_key_env = match value.as_str() {
            Some(name) if !name.is_empty() && name.len() <= 128 && name.bytes().enumerate().all(|(i,b)|
                b.is_ascii_uppercase() || b == b'_' || (i > 0 && b.is_ascii_digit())) => Some(name.into()),
            _ => { invalid(); None }
        }; }
        "timeout_ms" => {layer.timeout_ms = super::as_positive_u64(value,display,path).filter(|value| {let ok=*value<=45_000; if !ok {invalid();} ok});}
        "max_in_flight_requests" => {layer.max_in_flight_requests = super::as_positive_usize(value,display,path).filter(|value| {let ok=*value<=3; if !ok {invalid();} ok});}
        "request_spacing_ms" => {layer.request_spacing_ms = super::as_nonneg_usize(value,display,path).map(|value| value as u64).filter(|value| {let ok=*value<=10_000; if !ok {invalid();} ok});}
        "max_batch_bytes" => {layer.max_batch_bytes = super::as_positive_usize(value,display,path).filter(|value| {let ok=*value<=80_000; if !ok {invalid();} ok});}
        "pool_idle_timeout_ms" => {layer.pool_idle_timeout_ms = super::as_positive_u64(value,display,path).filter(|value| {let ok=*value<=300_000; if !ok {invalid();} ok});}
        "search_filter_min_unrelated_probability" => {
            layer.search_filter_min_unrelated_probability = value.as_float().filter(|value| crate::tools::search::jev::valid_threshold(*value));
            if layer.search_filter_min_unrelated_probability.is_none() {invalid();}
        }
        _ => return false,
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_jev_default_off_per_key_precedence_and_threshold() {
        let root = tempfile::tempdir().unwrap();
        let global = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".codemap")).unwrap();
        std::fs::write(global.path().join("config.toml"), "[analysis.jev]\noverview_enabled=true\nsearch_filter_enabled=true\nsearch_filter_min_unrelated_probability=0.90\n").unwrap();
        std::fs::write(root.path().join(".codemap/config.toml"), "[analysis.jev]\noverview_enabled=false\nsearch_filter_min_unrelated_probability=0.70\n").unwrap();
        let cfg = crate::config::load(root.path(),global.path()).jev;
        assert!(!cfg.overview_enabled && cfg.search_filter_enabled);
        assert_eq!(cfg.search_filter_min_unrelated_probability,0.70);
        assert_eq!(JevConfig::default().search_filter_min_unrelated_probability,0.70);
        assert!(!JevConfig::default().overview_enabled && !JevConfig::default().search_filter_enabled);
        std::fs::write(root.path().join(".codemap/config.toml"), "[analysis.jev]\nsearch_filter_min_unrelated_probability=0.5\nmodel='jev-latest'\n").unwrap();
        let cfg = crate::config::load(root.path(),global.path()).jev;
        assert_eq!(cfg.search_filter_min_unrelated_probability,0.90);
        assert_eq!(cfg.model,crate::jev::MODEL);
        std::fs::write(root.path().join(".codemap/config.toml"), "[analysis.jev]\nsearch_filter_min_unrelated_probability=nan\n").unwrap();
        assert_eq!(crate::config::load(root.path(),global.path()).jev.search_filter_min_unrelated_probability,0.90);
    }
    #[test]
    fn test_jev_localized_templates_and_migration_are_keyless() {
        let en:toml::Value = toml::from_str(crate::config::CONFIG_TEMPLATE).unwrap();
        let ko:toml::Value = toml::from_str(crate::config::CONFIG_TEMPLATE_KO).unwrap();
        assert_eq!(en["analysis"]["jev"],ko["analysis"]["jev"]);
        assert_eq!(en["analysis"]["jev"]["overview_enabled"].as_bool(),Some(false));
        assert_eq!(en["analysis"]["jev"]["search_filter_enabled"].as_bool(),Some(false));
        assert_eq!(en["analysis"]["jev"]["api_key_env"].as_str(),Some("TYPESAFE_API_KEY"));
    }
}

pub(super) fn merge(repo: JevLayer, global: JevLayer) -> JevConfig {
    let defaults = JevConfig::default();
    JevConfig {
        overview_enabled:repo.overview_enabled.or(global.overview_enabled).unwrap_or(defaults.overview_enabled),
        search_filter_enabled:repo.search_filter_enabled.or(global.search_filter_enabled).unwrap_or(defaults.search_filter_enabled),
        model:repo.model.or(global.model).unwrap_or(defaults.model),
        api_key_env:repo.api_key_env.or(global.api_key_env).unwrap_or(defaults.api_key_env),
        timeout_ms:repo.timeout_ms.or(global.timeout_ms).unwrap_or(defaults.timeout_ms),
        max_in_flight_requests:repo.max_in_flight_requests.or(global.max_in_flight_requests).unwrap_or(defaults.max_in_flight_requests),
        request_spacing_ms:repo.request_spacing_ms.or(global.request_spacing_ms).unwrap_or(defaults.request_spacing_ms),
        max_batch_bytes:repo.max_batch_bytes.or(global.max_batch_bytes).unwrap_or(defaults.max_batch_bytes),
        pool_idle_timeout_ms:repo.pool_idle_timeout_ms.or(global.pool_idle_timeout_ms).unwrap_or(defaults.pool_idle_timeout_ms),
        search_filter_min_unrelated_probability:repo.search_filter_min_unrelated_probability.or(global.search_filter_min_unrelated_probability).unwrap_or(defaults.search_filter_min_unrelated_probability),
    }
}
