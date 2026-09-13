//! Request-scoped presentation. Defaults preserve the existing live-tool contract.
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
    pub should_include_events: bool,
}

impl Default for LiveOptions {
    fn default() -> Self {
        Self {
            view: LiveView::Full,
            should_list_unresolved: true,
            should_expand_callable: false,
            should_include_events: false,
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
            should_list_unresolved: choice(args, "unresolved", "list", &["list", "count"])?
                == "list",
            should_expand_callable: choice(args, "expand", "none", &["none", "callable"])?
                == "callable",
            should_include_events: match get_arg(args, "include_events") {
                None => false,
                Some(Value::Bool(value)) => *value,
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

    pub fn validate_grep(self, mode: &str) -> Result<(), (i64, String)> {
        if mode != "content" && self != Self::default() {
            return Err((
                -32602,
                "view/unresolved/expand/include_events controls require grep output_mode='content'.".into(),
            ));
        }
        Ok(())
    }
}
