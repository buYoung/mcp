//! One measured report, with human table and compact, typed JSON renderers.
use super::{options::Options, table};
use serde_json::{json, Value};

pub(super) struct Cell {
    pub value: Value,
    pub display: String,
}

impl Cell {
    pub fn bytes(value: Option<i64>) -> Self {
        Self {
            value: json!(value),
            display: table::size(value),
        }
    }
    pub fn decimal(value: f64) -> Self {
        let value = (value * 100.0).round() / 100.0;
        Self {
            value: json!(value),
            display: format!("{value:.2}"),
        }
    }
    pub fn percent(value: Option<f64>) -> Self {
        let value = value.map(|value| (value * 10.0).round() / 10.0);
        Self {
            value: json!(value),
            display: value.map_or_else(|| "n/a".into(), |value| format!("{value:.1}%")),
        }
    }
    pub fn share(part: i64, total: i64) -> Self {
        Self::percent((total > 0).then(|| part as f64 * 100.0 / total as f64))
    }
}

impl From<i64> for Cell {
    fn from(value: i64) -> Self {
        Self {
            value: json!(value),
            display: value.to_string(),
        }
    }
}
impl From<String> for Cell {
    fn from(value: String) -> Self {
        Self {
            display: table::display_text(&value),
            value: json!(value),
        }
    }
}
impl From<&str> for Cell {
    fn from(value: &str) -> Self {
        value.to_string().into()
    }
}

pub(super) struct Dataset {
    pub key: &'static str,
    pub title: &'static str,
    pub columns: Vec<(&'static str, &'static str, bool)>,
    pub rows: Vec<Vec<Cell>>,
}

impl Dataset {
    pub fn new(
        key: &'static str,
        title: &'static str,
        columns: &[(&'static str, &'static str, bool)],
        rows: Vec<Vec<Cell>>,
    ) -> Self {
        Self {
            key,
            title,
            columns: columns.to_vec(),
            rows,
        }
    }
}

pub(crate) struct Report {
    pub(super) metadata: Value,
    pub(super) summary: Vec<(&'static str, &'static str, Cell)>,
    pub(super) tables: Vec<Dataset>,
    pub(super) notes: Vec<String>,
}

impl Report {
    pub(super) fn new(options: &Options, status: &str) -> Self {
        Self {
            metadata: json!({"target": options.target.as_str(), "status": status, "retention_days": 30,
                "sort": options.sort_name(), "order": options.order_name()}),
            summary: Vec::new(),
            tables: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub(super) fn page(&mut self, options: &Options, total: i64, returned: usize) {
        let next = options.offset.saturating_add(returned);
        self.metadata["page"] = json!({"offset": options.offset, "limit": options.limit,
            "returned": returned, "total": total,
            "next_offset": if next < total as usize { Some(next) } else { None }});
    }

    pub(crate) fn to_value(&self) -> Value {
        let mut value = self.metadata.clone();
        value["summary"] = Value::Object(
            self.summary
                .iter()
                .map(|(key, _, cell)| (key.to_string(), cell.value.clone()))
                .collect(),
        );
        for dataset in &self.tables {
            value[dataset.key] = json!({
                "columns": dataset.columns.iter().map(|(key, _, _)| key).collect::<Vec<_>>(),
                "rows": dataset.rows.iter().map(|row| row.iter().map(|cell| &cell.value).collect::<Vec<_>>()).collect::<Vec<_>>()
            });
        }
        value["notes"] = json!(self.notes);
        value
    }

    pub(crate) fn print(&self) {
        println!(
            "{}",
            if self.metadata["target"] == "index" {
                "Index analysis"
            } else {
                "Reading activity"
            }
        );
        let status = match self.metadata["status"].as_str() {
            Some("index_missing") => Some("No committed index"),
            Some("no_records") => Some("No usage records yet"),
            Some("no_activity") => Some("No matching calls in this window"),
            Some("no_matches") => Some("No matching indexed files"),
            _ => None,
        };
        if let Some(status) = status {
            println!("Status: {status}");
        }
        if let Some(window) = self.metadata.get("window") {
            println!(
                "Window: {} through {} UTC ({} days)",
                window["from_utc"].as_str().unwrap_or(""),
                window["to_utc"].as_str().unwrap_or(""),
                window["days"]
            );
        }
        if let Some(scope) = self.metadata.get("scope") {
            let filters = [
                ("filter", "path contains"),
                ("language", "language"),
                ("tool", "tool"),
            ]
            .into_iter()
            .filter_map(|(key, label)| {
                scope[key]
                    .as_str()
                    .map(|value| format!("{label}: {}", table::display_text(value)))
            })
            .collect::<Vec<_>>();
            println!(
                "Scope: {}",
                if filters.is_empty() {
                    "entire workspace".into()
                } else {
                    filters.join("; ")
                }
            );
        }
        if self.metadata.get("page").is_some() {
            println!(
                "File order: {} ({})",
                self.metadata["sort"].as_str().unwrap_or(""),
                self.metadata["order"].as_str().unwrap_or("")
            );
        }
        if !self.summary.is_empty() {
            table::print(
                "Summary",
                &[("Metric", false), ("Value", false)],
                &self
                    .summary
                    .iter()
                    .map(|(_, label, cell)| vec![label.to_string(), cell.display.clone()])
                    .collect::<Vec<_>>(),
            );
        }
        for dataset in &self.tables {
            table::print(
                dataset.title,
                &dataset
                    .columns
                    .iter()
                    .map(|(_, label, is_number)| (*label, *is_number))
                    .collect::<Vec<_>>(),
                &dataset
                    .rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| cell.display.clone())
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            );
        }
        if let Some(page) = self.metadata.get("page") {
            println!(
                "Files: {} shown of {}; offset {}. Totals include all matching files.",
                page["returned"], page["total"], page["offset"]
            );
            if !page["next_offset"].is_null() {
                println!(
                    "Next page: --offset {} (keep the same filters and sort).",
                    page["next_offset"]
                );
            }
        }
        for note in &self.notes {
            println!("{note}");
        }
        println!("Retention: fixed at 30 days; cleanup on startup, recording, analysis and every minute while MCP runs.");
    }
}

/// Trim whole rows/tables only; keep valid JSON, complete totals and usable continuation.
pub(crate) fn compact(mut value: Value, cap: usize) -> Result<String, String> {
    loop {
        let text = value.to_string();
        if text.len() <= cap {
            return Ok(text);
        }
        value["truncated"] = json!("response_byte_cap");
        if let Some(rows) = value
            .get_mut("files")
            .and_then(|table| table.get_mut("rows"))
            .and_then(Value::as_array_mut)
        {
            if rows.len() > 1 {
                rows.pop();
                let returned = rows.len();
                let offset = value["page"]["offset"].as_u64().unwrap_or(0);
                value["page"]["returned"] = json!(returned);
                value["page"]["next_offset"] = json!(offset.saturating_add(returned as u64));
                continue;
            }
        }
        let removable = [
            "daily",
            "symbol_kinds",
            "languages",
            "tools",
            "freshness",
            "comparison",
        ]
        .into_iter()
        .find(|key| value.get(*key).is_some());
        if let Some(key) = removable {
            value.as_object_mut().unwrap().remove(key);
            if value.get("omitted_tables").is_none() {
                value["omitted_tables"] = json!([]);
            }
            value["omitted_tables"]
                .as_array_mut()
                .unwrap()
                .push(json!(key));
            continue;
        }
        return Err(format!("Analysis cannot fit within {cap} bytes without losing its summary or a file row. Use view=summary or raise output.max_bytes."));
    }
}
