use crate::events::EventRule;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventNavigationConfig {
    pub is_enabled: bool,
    pub use_builtin_rules: bool,
    pub rules: Vec<EventRule>,
}
impl Default for EventNavigationConfig {
    fn default() -> Self {
        Self {
            is_enabled: true,
            use_builtin_rules: true,
            rules: Vec::new(),
        }
    }
}

#[derive(Default)]
pub(super) struct EventNavigationLayer {
    is_enabled: Option<bool>,
    use_builtin_rules: Option<bool>,
    rules: Option<Vec<EventRule>>,
}

pub(super) fn normalize(value: &toml::Value, path: &Path) -> EventNavigationLayer {
    let mut layer = EventNavigationLayer::default();
    let Some(table) = value.as_table() else {
        super::warn("event_navigation must be a table — ignored");
        return layer;
    };
    for (key, value) in table {
        match key.as_str() {
            "is_enabled" => layer.is_enabled = super::as_bool(value,"event_navigation.is_enabled",path),
            "use_builtin_rules" => layer.use_builtin_rules = super::as_bool(value,"event_navigation.use_builtin_rules",path),
            "rules" => match value.clone().try_into::<Vec<EventRule>>().map(|mut rules| {
                for rule in &mut rules {rule.module=rule.module.trim_start_matches("./").to_string();}
                rules
            }) {
                Ok(rules) if rules.len() <= 64 && rules.iter().all(EventRule::validate)
                    && rules.iter().enumerate().all(|(i,rule)|rules[..i].iter().all(|other|other.id!=rule.id && other.selector()!=rule.selector())) => layer.rules=Some(rules),
                _ => super::warn(&format!("invalid event_navigation.rules in {}: require unique exact API selectors, valid roles/argument positions and explicit bus rules; using lower layer",path.display())),
            },
            _ => super::warn(&format!("unknown config key 'event_navigation.{key}': {} — ignored",path.display())),
        }
    }
    layer
}

pub(super) fn merge(
    repo: EventNavigationLayer,
    global: EventNavigationLayer,
) -> EventNavigationConfig {
    let defaults = EventNavigationConfig::default();
    EventNavigationConfig {
        is_enabled: repo
            .is_enabled
            .or(global.is_enabled)
            .unwrap_or(defaults.is_enabled),
        use_builtin_rules: repo
            .use_builtin_rules
            .or(global.use_builtin_rules)
            .unwrap_or(defaults.use_builtin_rules),
        rules: repo.rules.or(global.rules).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(body: &str) -> EventNavigationLayer {
        normalize(
            &toml::from_str::<toml::Value>(body).unwrap(),
            Path::new("config.toml"),
        )
    }

    #[test]
    fn test_event_rules_replace_layers_and_reject_conflicting_selectors() {
        assert!(merge(layer(""), layer("")).is_enabled);
        assert!(!merge(layer("is_enabled=false"), layer("is_enabled=true")).is_enabled);
        assert!(!merge(layer(""), layer("is_enabled=false")).is_enabled);
        let rule="{id='custom',language='typescript',module='./src/api.ts',symbol='send',role='publish',event_arg=0,bus='fixed',bus_identity='app'}";
        let global = format!("is_enabled=true\nuse_builtin_rules=false\nrules=[{rule}]\n");
        let inherited = merge(layer(""), layer(&global));
        assert!(inherited.is_enabled);
        assert!(!inherited.use_builtin_rules);
        assert_eq!(inherited.rules[0].module, "src/api.ts");
        let removed = merge(layer("rules=[]"), layer(&global));
        assert!(removed.rules.is_empty());
        assert!(removed.is_enabled);
        let duplicate = rule
            .replace("id='custom'", "id='second'")
            .replace("./src/api.ts", "src/api.ts");
        let bad = format!("rules=[{rule},{duplicate}]");
        let fallback = merge(layer(&bad), layer(&global));
        assert_eq!(fallback.rules, inherited.rules);
        for invalid in [
            rule.replace("event_arg=0", "event_arg=16"),
            rule.replace("symbol='send'", "symbol='*'"),
            rule.replace("event_arg=0", "event_arg=0,event_key='saved'"),
            rule.replace("role='publish'", "role='subscribe'"),
            rule.replace("bus='fixed',bus_identity='app'", "bus='receiver'"),
            rule.replace("event_arg=0", "event_arg=0,unknown=true"),
        ] {
            assert_eq!(
                merge(layer(&format!("rules=[{invalid}]")), layer(&global)).rules,
                inherited.rules
            );
        }
    }
}
