use regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RedactRule {
    pub id: String,
    pub pattern: Regex,
}

impl PartialEq for RedactRule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.pattern.as_str() == other.pattern.as_str()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedactException {
    pub rule_id: String,
    pub value: String,
}

#[derive(Debug, Clone, Default)]
pub struct RedactConfig {
    pub pii_entities: Vec<String>,
    pub sensitive_fields: Vec<String>,
    pub rules: Vec<RedactRule>,
    pub exceptions: Vec<RedactException>,
}

#[derive(Default, PartialEq)]
pub(super) struct RedactLayer {
    pii_entities: Option<Vec<String>>,
    sensitive_fields: Option<Vec<String>>,
    rules: Option<Vec<RedactRule>>,
    exceptions: Option<Vec<RedactException>>,
}

pub(crate) fn normalize_field(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

// Report locations, never patterns, exception values or regex parser diagnostics:
// the user's configuration may itself contain credentials.
fn invalid(key: &str, path: &Path) {
    super::warn(&format!(
        "invalid redact.{key}: {} — falling back for this key",
        path.display()
    ));
}

fn rules(value: &toml::Value) -> Option<Vec<RedactRule>> {
    let mut ids = BTreeSet::new();
    value
        .as_array()?
        .iter()
        .map(|entry| {
            let table = entry.as_table()?;
            if table
                .keys()
                .any(|key| !matches!(key.as_str(), "id" | "pattern"))
            {
                return None;
            }
            let id = table.get("id")?.as_str()?;
            if !id.starts_with("custom.")
                || id.len() == 7
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
                || !ids.insert(id.to_string())
            {
                return None;
            }
            let pattern = Regex::new(table.get("pattern")?.as_str()?).ok()?;
            if pattern.is_match("") {
                return None;
            }
            Some(RedactRule {
                id: id.to_string(),
                pattern,
            })
        })
        .collect()
}

fn exceptions(value: &toml::Value) -> Option<Vec<RedactException>> {
    value
        .as_array()?
        .iter()
        .map(|entry| {
            let table = entry.as_table()?;
            if table
                .keys()
                .any(|key| !matches!(key.as_str(), "rule_id" | "value"))
            {
                return None;
            }
            let rule_id = table.get("rule_id")?.as_str()?;
            let value = table.get("value")?.as_str()?;
            if rule_id.is_empty() || value.is_empty() {
                return None;
            }
            Some(RedactException {
                rule_id: rule_id.to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

pub(super) fn normalize(value: &toml::Value, path: &Path) -> RedactLayer {
    let mut layer = RedactLayer::default();
    let Some(table) = value.as_table() else {
        invalid("table", path);
        return layer;
    };
    for (key, value) in table {
        let is_valid = match key.as_str() {
            "pii_entities" => {
                layer.pii_entities = value.as_array().and_then(|values| {
                    values
                        .iter()
                        .map(|value| {
                            let entity = value.as_str()?;
                            crate::redact::is_supported_pii_entity(entity)
                                .then(|| entity.to_string())
                        })
                        .collect()
                });
                layer.pii_entities.is_some()
            }
            "sensitive_fields" => {
                layer.sensitive_fields = value.as_array().and_then(|values| {
                    values
                        .iter()
                        .map(|value| {
                            let name = normalize_field(value.as_str()?);
                            (!name.is_empty()).then_some(name)
                        })
                        .collect()
                });
                layer.sensitive_fields.is_some()
            }
            "rules" => {
                layer.rules = rules(value);
                layer.rules.is_some()
            }
            "exceptions" => {
                layer.exceptions = exceptions(value);
                layer.exceptions.is_some()
            }
            _ => {
                super::warn(&format!(
                    "unknown config key 'redact.{key}': {} — ignored",
                    path.display()
                ));
                continue;
            }
        };
        if !is_valid {
            invalid(key, path);
        }
    }
    layer
}

pub(super) fn merge(repo: RedactLayer, global: RedactLayer) -> RedactConfig {
    RedactConfig {
        pii_entities: repo
            .pii_entities
            .or(global.pii_entities)
            .unwrap_or_default(),
        sensitive_fields: repo
            .sensitive_fields
            .or(global.sensitive_fields)
            .unwrap_or_default(),
        rules: repo.rules.or(global.rules).unwrap_or_default(),
        exceptions: repo.exceptions.or(global.exceptions).unwrap_or_default(),
    }
}
