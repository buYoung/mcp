//! Request-scoped presentation with automatic, relevant event context.
use super::get_arg;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LiveView {
    #[default]
    Full,
    Source,
    Definitions,
    Relations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LiveOptions {
    pub view: LiveView,
    pub should_list_unresolved: bool,
    pub should_expand_callable: bool,
    pub include_events: Option<bool>,
    pub should_debug: bool,
}

impl Default for LiveOptions {
    fn default() -> Self {
        Self {
            view: LiveView::Full,
            should_list_unresolved: true,
            should_expand_callable: false,
            include_events: None,
            should_debug: false,
        }
    }
}

fn choice<'a>(
    args: &'a Value,
    key: &str,
    default: &'a str,
    choices: &[&str],
) -> Result<&'a str, (i64, String)> {
    match get_arg(args, key) {
        None => Ok(default),
        Some(Value::String(value)) if choices.contains(&value.as_str()) => Ok(value),
        _ => Err((
            -32602,
            format!("Invalid '{key}': expected {}.", choices.join(" | ")),
        )),
    }
}

impl LiveOptions {
    pub fn parse(args: &Value) -> Result<Self, (i64, String)> {
        let view = match choice(
            args,
            "view",
            "full",
            &["full", "source", "definitions", "relations"],
        )? {
            "source" => LiveView::Source,
            "definitions" => LiveView::Definitions,
            "relations" => LiveView::Relations,
            _ => LiveView::Full,
        };
        Ok(Self {
            view,
            should_debug: debug_requested(args)?,
            should_list_unresolved: choice(args, "unresolved", "list", &["list", "count"])?
                == "list",
            should_expand_callable: choice(args, "expand", "none", &["none", "callable"])?
                == "callable",
            include_events: match get_arg(args, "include_events") {
                None => None,
                Some(Value::Bool(value)) => Some(*value),
                _ => {
                    return Err((
                        -32602,
                        "Invalid 'include_events': expected a boolean.".into(),
                    ))
                }
            },
        })
    }

    pub fn should_include_relations(self) -> bool {
        matches!(self.view, LiveView::Full | LiveView::Relations)
    }

    pub fn should_include_events(self) -> bool {
        self.include_events.unwrap_or(true) && self.should_include_relations()
    }

    pub fn validate_grep(self, mode: &str) -> Result<(), (i64, String)> {
        if mode != "content"
            && (self.view != LiveView::Full
                || !self.should_list_unresolved
                || self.should_debug
                || self.should_expand_callable
                || self.include_events == Some(true))
        {
            return Err((
                -32602,
                "view/unresolved/expand/include_events/debug controls require grep output_mode='content'.".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn debug_requested(args: &Value) -> Result<bool, (i64, String)> {
    match get_arg(args, "debug") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err((-32602, "Invalid 'debug': expected a boolean.".into())),
    }
}
