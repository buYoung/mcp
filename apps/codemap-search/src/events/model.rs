use crate::parser::CodeRange;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventRole {
    Publish,
    Subscribe,
    Unsubscribe,
}
impl EventRole {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Publish => "publisher",
            Self::Subscribe => "registration",
            Self::Unsubscribe => "removal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventBusRule {
    Receiver,
    Argument,
    Fixed,
    Framework,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRule {
    pub id: String,
    pub language: String,
    pub module: String,
    pub symbol: String,
    #[serde(default)]
    pub method: Option<String>,
    pub role: EventRole,
    #[serde(default)]
    pub event_arg: Option<usize>,
    #[serde(default)]
    pub event_key: Option<String>,
    #[serde(default)]
    pub handler_arg: Option<usize>,
    pub bus: EventBusRule,
    #[serde(default)]
    pub bus_arg: Option<usize>,
    #[serde(default)]
    pub bus_identity: Option<String>,
    #[serde(default)]
    pub target_arg: Option<usize>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub is_once: bool,
    #[serde(skip)]
    pub(crate) is_builtin: bool,
}

impl EventRule {
    pub(crate) fn selector(&self) -> (&str, &str, &str, Option<&str>) {
        (
            &self.language,
            &self.module,
            &self.symbol,
            self.method.as_deref(),
        )
    }
    pub(crate) fn validate(&self) -> bool {
        let valid =
            |s: &str| !s.is_empty() && s.len() <= 256 && !s.contains(['*', '?', '\n', '\r', '\0']);
        valid(&self.id)
            && valid(&self.module)
            && valid(&self.symbol)
            && matches!(self.language.as_str(), "rust" | "typescript" | "javascript")
            && self.method.as_deref().is_none_or(valid)
            && (self.event_arg.is_some() != self.event_key.is_some())
            && self.event_arg.is_none_or(|i| i < 16)
            && self.event_key.as_deref().is_none_or(|key| {
                !key.is_empty() && key.len() <= 256 && !key.contains(['\n', '\r', '\0'])
            })
            && self.channel.as_deref().is_none_or(valid)
            && self.handler_arg.is_none_or(|i| i < 16)
            && self.target_arg.is_none_or(|i| i < 16)
            && self.target.as_deref().is_none_or(valid)
            && !(self.target.is_some() && self.target_arg.is_some())
            && (self.target.is_none() || self.module != "@tauri-apps/api/event")
            && self.bus_arg.is_none_or(|i| i < 16)
            && (self.role != EventRole::Subscribe || self.handler_arg.is_some())
            && match self.bus {
                EventBusRule::Receiver => self.method.is_some(),
                EventBusRule::Argument => self.bus_arg.is_some(),
                EventBusRule::Fixed => self.bus_identity.as_deref().is_some_and(valid),
                EventBusRule::Framework => self.module == "@tauri-apps/api/event",
            }
    }
}

/// Stored with its owning extracted file, not in request-time mutable caches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub source: Option<String>,
    pub unavailable_reason: Option<String>,
}
impl EventInput {
    pub(crate) fn capture(source: &str, path: &str) -> Option<Self> {
        let source_path = std::path::Path::new(path);
        // Rust declaration/implementation navigation shares the same immutable
        // module input. Disabling event output must not disable type resolution.
        let is_type_resolution_input = source_path.extension().is_some_and(|ext| ext == "rs")
            || source_path
                .file_name()
                .is_some_and(|name| name == "Cargo.toml");
        if !super::eligible(path)
            || !crate::config::get().event_navigation.is_enabled && !is_type_resolution_input
        {
            return None;
        }
        Some(if source.len() <= super::SOURCE_BYTES_PER_FILE {
            Self {
                source: Some(source.to_string()),
                unavailable_reason: None,
            }
        } else {
            Self {
                source: None,
                unavailable_reason: Some(format!(
                    "event source exceeds {} bytes",
                    super::SOURCE_BYTES_PER_FILE
                )),
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventLocation {
    pub file_path: String,
    pub range: CodeRange,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventBusEvidence {
    pub identity: String,
    pub description: String,
    pub is_configured_assumption: bool,
    pub locations: Vec<EventLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEndpoint {
    pub role: EventRole,
    pub location: EventLocation,
    pub enclosing_symbol: Option<String>,
    pub api_rule: String,
    pub api_identity: String,
    pub api_definition: Option<EventLocation>,
    pub is_api_configured: bool,
    pub bus: Option<EventBusEvidence>,
    pub event_key: Option<String>,
    pub is_key_configured: bool,
    pub key_evidence: Vec<EventLocation>,
    pub handler: Option<EventLocation>,
    pub handler_reason: Option<String>,
    pub target: Option<String>,
    pub is_target_configured: bool,
    pub channel: String,
    pub conditions: Vec<String>,
    pub unresolved_reasons: Vec<String>,
    pub is_once: bool,
    pub is_inactive: bool,
    pub dependencies: Vec<EventLocation>,
}

impl EventEndpoint {
    pub(crate) fn key(&self) -> Option<(&str, &str, &str, &str)> {
        if self.is_inactive || !self.unresolved_reasons.is_empty() {
            return None;
        }
        Some((
            &self.bus.as_ref()?.identity,
            self.event_key.as_deref()?,
            self.target.as_deref()?,
            &self.channel,
        ))
    }
}
