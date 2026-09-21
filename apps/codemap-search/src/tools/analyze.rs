//! MCP adapter: strict arguments, shared analytics, bounded compact output.
use crate::analyze::options::{IndexSort, Options, Order, ReadsSort, Target, Tool, View};
use clap::ValueEnum;
use serde::Deserialize;
use serde_json::{json, Value};

const OUTPUT_BYTE_CAP: usize = 8192;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    target: Target,
    limit: Option<usize>,
    offset: Option<usize>,
    sort: Option<String>,
    order: Option<Order>,
    filter: Option<String>,
    view: Option<View>,
    days: Option<u8>,
    tool: Option<Tool>,
    compare: Option<bool>,
    language: Option<String>,
}

pub(super) fn definition() -> Value {
    json!({
        "name": "analyze",
        "description": include_str!("instructions/tools/analyze.md").trim_end(),
        "annotations": {"readOnlyHint": true, "openWorldHint": false},
        "inputSchema": {
            "type": "object", "additionalProperties": false,
            "properties": {
                "target": {"type": "string", "enum": ["index", "reads"], "description": "index: committed footprint; reads: recorded MCP activity."},
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 10, "description": "File rows per page; totals cover all matches."},
                "offset": {"type": "integer", "minimum": 0, "default": 0, "description": "Continue using next_offset with unchanged filters/sort."},
                "sort": {"type": "string", "enum": ["stored", "size", "lines", "symbols", "literals", "path", "reads", "bytes", "last"], "description": "index: stored(default),size,lines,symbols,literals,path. reads: bytes(default),reads,size,last,path."},
                "order": {"type": "string", "enum": ["asc", "desc"], "description": "Default: asc for path, desc otherwise."},
                "filter": {"type": "string", "maxLength": 512, "description": "Case-sensitive literal path substring, at most 512 UTF-8 bytes; not a glob."},
                "view": {"type": "string", "enum": ["summary", "files", "full"], "default": "files", "description": "summary: totals/groups; files: totals/file rows; full: both plus kinds/daily."},
                "days": {"type": "integer", "minimum": 1, "maximum": 30, "default": 7, "description": "reads only: rolling days; storage retention stays 30."},
                "tool": {"type": "string", "enum": ["read", "search", "grep"], "description": "reads only: restrict to one tool."},
                "compare": {"type": "boolean", "default": true, "description": "reads only: preceding equal window; unavailable above 15 days."},
                "language": {"type": "string", "description": "index only: indexed language, e.g. rust or typescript."}
            },
            "required": ["target"]
        }
    })
}

pub(crate) fn run(
    arguments: &Value,
    engine: &crate::index::EngineSupervisor,
) -> Result<String, (i64, String)> {
    let invalid = |message: String| (-32602, message);
    let arguments: Arguments = serde_json::from_value(arguments.clone())
        .map_err(|error| invalid(format!("Invalid analyze arguments: {error}")))?;
    let mut options = Options::new(arguments.target);
    options.is_mcp = true;
    options.limit = arguments.limit.unwrap_or(10);
    options.offset = arguments.offset.unwrap_or(0);
    if !(1..=100).contains(&options.limit) {
        return Err(invalid(
            "analyze limit must be between 1 and 100; use next_offset to continue.".into(),
        ));
    }
    if options.offset > i64::MAX as usize {
        return Err(invalid(
            "analyze offset exceeds the supported range.".into(),
        ));
    }
    options.filter = arguments.filter;
    options.order = arguments.order;
    options.view = arguments.view.unwrap_or(View::Files);
    match arguments.target {
        Target::Index => {
            if arguments.days.is_some() || arguments.tool.is_some() || arguments.compare.is_some() {
                return Err(invalid(
                    "days, tool and compare apply only to target=reads.".into(),
                ));
            }
            options.index_sort =
                IndexSort::from_str(arguments.sort.as_deref().unwrap_or("stored"), false)
                    .map_err(invalid)?;
            options.language = arguments.language;
        }
        Target::Reads => {
            if arguments.language.is_some() {
                return Err(invalid("language applies only to target=index.".into()));
            }
            options.reads_sort =
                ReadsSort::from_str(arguments.sort.as_deref().unwrap_or("bytes"), false)
                    .map_err(invalid)?;
            options.days = arguments.days.unwrap_or(7);
            options.tool = arguments.tool;
            options.should_compare = arguments.compare.unwrap_or(true);
        }
    }
    options.validate().map_err(invalid)?;
    let root = crate::workspace::current_dir()?;
    let report = crate::analyze::collect(&root, &options)
        .map_err(|error| (-32603, format!("Analysis unavailable: {error:#}")))?;
    let mut value = report.to_value();
    if options.target == Target::Index {
        value["index_state"] = json!(if engine.is_dead() {
            "frozen"
        } else if engine.is_warming() {
            "warming"
        } else if engine.last_error().is_some() {
            "stale"
        } else {
            "ready"
        });
    }
    // Mask string values before JSON serialization, preserving numeric metrics and syntax.
    // The dispatcher must not mask the resulting JSON string a second time.
    crate::redact::response(&mut value);
    let cap = crate::config::get()
        .output_byte_cap
        .unwrap_or(OUTPUT_BYTE_CAP)
        .min(OUTPUT_BYTE_CAP);
    crate::analyze::compact(value, cap).map_err(invalid)
}
