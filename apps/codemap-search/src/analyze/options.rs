use clap::ValueEnum;

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Index,
    Reads,
}

impl Target {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Index => "index",
            Self::Reads => "reads",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Summary,
    Files,
    Full,
}

impl View {
    pub(super) fn has_files(self) -> bool {
        self != Self::Summary
    }
    pub(super) fn has_groups(self) -> bool {
        self != Self::Files
    }
}

#[derive(Clone, Copy, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum IndexSort {
    Stored,
    Size,
    Lines,
    Symbols,
    Literals,
    Path,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ReadsSort {
    Reads,
    Bytes,
    Size,
    Last,
    Path,
}

#[derive(Clone, Copy, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Read,
    Search,
    Grep,
}

impl Tool {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Search => "search",
            Self::Grep => "grep",
        }
    }
}

pub(crate) struct Options {
    pub target: Target,
    pub limit: usize,
    pub offset: usize,
    pub filter: Option<String>,
    pub order: Option<Order>,
    pub view: View,
    pub index_sort: IndexSort,
    pub reads_sort: ReadsSort,
    pub language: Option<String>,
    pub days: u8,
    pub tool: Option<Tool>,
    pub should_compare: bool,
    pub is_mcp: bool,
}

impl Options {
    pub(crate) fn new(target: Target) -> Self {
        Self {
            target,
            limit: 20,
            offset: 0,
            filter: None,
            order: None,
            view: View::Full,
            index_sort: IndexSort::Stored,
            reads_sort: ReadsSort::Reads,
            language: None,
            days: 7,
            tool: None,
            should_compare: true,
            is_mcp: false,
        }
    }

    pub(crate) fn validate(&mut self) -> Result<(), String> {
        if !(1..=30).contains(&self.days) {
            return Err("days must be between 1 and 30 (fixed retention).".into());
        }
        if self.filter.as_ref().is_some_and(|value| value.len() > 512) {
            return Err("filter must be at most 512 UTF-8 bytes.".into());
        }
        self.filter = self.filter.take().filter(|value| !value.is_empty());
        if let Some(language) = &self.language {
            self.language = Some(
                crate::lang::normalize_language_hint(language)
                    .ok_or_else(|| format!("Unknown indexed language: {language}"))?
                    .into(),
            );
        }
        Ok(())
    }

    pub(super) fn sql_limit(&self) -> i64 {
        if self.limit == 0 {
            -1
        } else {
            self.limit.min(i64::MAX as usize) as i64
        }
    }

    pub(super) fn sql_offset(&self) -> i64 {
        self.offset.min(i64::MAX as usize) as i64
    }

    pub(super) fn sort_name(&self) -> String {
        match self.target {
            Target::Index => self.index_sort.to_possible_value(),
            Target::Reads => self.reads_sort.to_possible_value(),
        }
        .unwrap()
        .get_name()
        .to_string()
    }

    pub(super) fn order_name(&self) -> &'static str {
        match self.order {
            Some(Order::Asc) => "asc",
            Some(Order::Desc) => "desc",
            None if self.sort_name() == "path" => "asc",
            None => "desc",
        }
    }

    pub(super) fn sql_order(&self, column: &str) -> String {
        let direction = match self.order {
            Some(Order::Asc) => "ASC",
            Some(Order::Desc) => "DESC",
            None if column == "path" => "ASC",
            None => "DESC",
        };
        // Column names are chosen only from the enums below, never caller text.
        format!("{column} {direction} NULLS LAST, path ASC")
    }

    pub(super) fn index_order(&self) -> String {
        self.sql_order(match self.index_sort {
            IndexSort::Stored => "payload_bytes",
            IndexSort::Size => "size_bytes",
            IndexSort::Lines => "lines",
            IndexSort::Symbols => "symbols",
            IndexSort::Literals => "literals",
            IndexSort::Path => "path",
        })
    }

    pub(super) fn reads_order(&self) -> String {
        self.sql_order(match self.reads_sort {
            ReadsSort::Reads => "reads",
            ReadsSort::Bytes => "result_bytes",
            ReadsSort::Size => "size_bytes",
            ReadsSort::Last => "last_seen",
            ReadsSort::Path => "path",
        })
    }
}
