use super::options::{IndexSort, Options, Order, ReadsSort, Target, Tool, View};
use clap::{Args, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum AnalyzeCommand {
    /// Inspect an existing index without reparsing source or rebuilding it
    #[command(
        after_help = "Examples:\n  codemap-search analyze index --sort size --limit 10\n  codemap-search analyze index --language rust --filter src/\n  codemap-search analyze index --view summary --format json"
    )]
    Index(IndexArgs),
    /// Analyze recent MCP file-content responses (default: 7 days; retention: 30 days)
    #[command(
        after_help = "Examples:\n  codemap-search analyze reads --sort bytes --limit 10\n  codemap-search analyze reads --days 14 --tool search\n  codemap-search analyze reads --filter src/ --offset 20 --limit 20"
    )]
    Reads(ReadsArgs),
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Table,
    Json,
}

#[derive(Args)]
struct CommonArgs {
    /// Repository whose configured index and SQLite usage records are analyzed
    #[arg(long, default_value = ".")]
    path: PathBuf,
    /// File rows per page; 0 shows all matching files
    #[arg(short = 'n', long, default_value_t = 20)]
    limit: usize,
    /// Zero-based file-row offset after filtering and sorting
    #[arg(long, default_value_t = 0)]
    offset: usize,
    /// Case-sensitive literal substring of file paths (not a glob)
    #[arg(short = 'f', long)]
    filter: Option<String>,
    /// Sort direction; default: ascending paths, descending numeric values
    #[arg(long, value_enum)]
    order: Option<Order>,
    /// summary: totals/groups; files: totals/file rows; full: both plus detailed groups
    #[arg(long, value_enum, default_value = "full")]
    view: View,
    /// Human-readable tables or compact JSON with column names declared once
    #[arg(long, value_enum, default_value = "table")]
    format: Format,
}

#[derive(Args)]
pub struct IndexArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Sort file records by stored JSON bytes, disk size, lines, symbols, literals or path
    #[arg(short = 's', long, value_enum, default_value = "stored")]
    sort: IndexSort,
    /// Indexed language, e.g. rust or typescript
    #[arg(short = 'l', long)]
    language: Option<String>,
}

#[derive(Args)]
pub struct ReadsArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Sort files by reads, result bytes, disk size, last observation or path
    #[arg(short = 's', long, value_enum, default_value = "reads")]
    sort: ReadsSort,
    /// Rolling window in days; maximum is the fixed 30-day retention
    #[arg(short = 'd', long, default_value_t = 7, value_parser = clap::value_parser!(u8).range(1..=30))]
    days: u8,
    /// Include only one MCP tool
    #[arg(short = 't', long, value_enum)]
    tool: Option<Tool>,
    /// Skip comparison with the preceding equal-length window
    #[arg(long)]
    no_compare: bool,
}

impl AnalyzeCommand {
    fn common(&self) -> &CommonArgs {
        match self {
            Self::Index(args) => &args.common,
            Self::Reads(args) => &args.common,
        }
    }

    pub fn path(&self) -> &Path {
        &self.common().path
    }

    pub fn run(&self) -> anyhow::Result<()> {
        let common = self.common();
        let mut options = match self {
            Self::Index(args) => {
                let mut options = Options::new(Target::Index);
                options.index_sort = args.sort;
                options.language = args.language.clone();
                options
            }
            Self::Reads(args) => {
                let mut options = Options::new(Target::Reads);
                options.reads_sort = args.sort;
                options.days = args.days;
                options.tool = args.tool;
                options.should_compare = !args.no_compare;
                options
            }
        };
        options.limit = common.limit;
        options.offset = common.offset;
        options.filter = common.filter.clone();
        options.order = common.order;
        options.view = common.view;
        options.validate().map_err(anyhow::Error::msg)?;
        let report = super::collect(self.path(), &options)?;
        match common.format {
            Format::Table => report.print(),
            Format::Json => println!("{}", report.to_value()),
        }
        Ok(())
    }
}
