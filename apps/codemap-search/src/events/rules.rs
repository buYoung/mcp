use super::{EventBusRule, EventRole, EventRule};

pub(super) fn effective_rules(config: &crate::config::EventNavigationConfig) -> Vec<EventRule> {
    let mut builtins = Vec::new();
    if config.use_builtin_rules {
        for language in ["javascript", "typescript"] {
            for module in ["node:events", "events"] {
                for (method, role, once) in [
                    ("on", EventRole::Subscribe, false),
                    ("addListener", EventRole::Subscribe, false),
                    ("once", EventRole::Subscribe, true),
                    ("emit", EventRole::Publish, false),
                    ("off", EventRole::Unsubscribe, false),
                    ("removeListener", EventRole::Unsubscribe, false),
                ] {
                    builtins.push(EventRule {
                        id: format!("builtin:{module}:{language}:{method}"),
                        language: language.into(),
                        module: module.into(),
                        symbol: "EventEmitter".into(),
                        method: Some(method.into()),
                        role,
                        event_arg: Some(0),
                        event_key: None,
                        handler_arg: (role != EventRole::Publish).then_some(1),
                        bus: EventBusRule::Receiver,
                        bus_arg: None,
                        bus_identity: None,
                        target_arg: None,
                        target: None,
                        channel: None,
                        is_once: once,
                        is_builtin: true,
                    });
                }
            }
            for (symbol, role, once, event_arg, target_arg) in [
                ("listen", EventRole::Subscribe, false, 0, None),
                ("once", EventRole::Subscribe, true, 0, None),
                ("emit", EventRole::Publish, false, 0, None),
                ("emitTo", EventRole::Publish, false, 1, Some(0)),
            ] {
                builtins.push(EventRule {
                    id: format!("builtin:tauri:{language}:{symbol}"),
                    language: language.into(),
                    module: "@tauri-apps/api/event".into(),
                    symbol: symbol.into(),
                    method: None,
                    role,
                    event_arg: Some(event_arg),
                    event_key: None,
                    handler_arg: (role == EventRole::Subscribe).then_some(1),
                    bus: EventBusRule::Framework,
                    bus_arg: None,
                    bus_identity: None,
                    target_arg,
                    target: None,
                    channel: None,
                    is_once: once,
                    is_builtin: true,
                });
            }
        }
        for symbol in ["AppHandle", "App", "Window", "WebviewWindow", "Webview"] {
            for (method, role, once, event_arg, target_arg) in [
                ("emit", EventRole::Publish, false, 0, None),
                ("emit_to", EventRole::Publish, false, 1, Some(0)),
                ("listen", EventRole::Subscribe, false, 0, None),
                ("once", EventRole::Subscribe, true, 0, None),
            ] {
                builtins.push(EventRule {
                    id: format!("builtin:tauri:rust:{symbol}:{method}"),
                    language: "rust".into(),
                    module: "tauri".into(),
                    symbol: symbol.into(),
                    method: Some(method.into()),
                    role,
                    event_arg: Some(event_arg),
                    event_key: None,
                    handler_arg: (role == EventRole::Subscribe).then_some(1),
                    bus: EventBusRule::Receiver,
                    bus_arg: None,
                    bus_identity: None,
                    target_arg,
                    target: None,
                    channel: None,
                    is_once: once,
                    is_builtin: true,
                });
            }
        }
    }
    builtins.retain(|builtin| {
        !config
            .rules
            .iter()
            .any(|rule| rule.selector() == builtin.selector())
    });
    let mut rules = config.rules.clone();
    rules.extend(builtins);
    rules
}
