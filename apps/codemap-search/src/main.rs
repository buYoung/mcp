use clap::{Parser, Subcommand};
use codemap_search::codemap::CodemapGenerator;
use codemap_search::index::SearchEngine;
use codemap_search::parser::{CodeExtractor, TreeSitterExtractor};
use codemap_search::{benchmark, index, mcp, parser};
use std::path::Path;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP JSON-RPC Server
    Mcp,

    /// Parse a source file and print extracted symbols
    Parse {
        /// File path to parse
        file: String,
    },

    /// Tokenize an identifier into sub-tokens
    Tokenize {
        /// Identifier to tokenize
        identifier: String,
    },

    /// Generate codemap views
    Codemap {
        /// Optional path (file or directory) to view
        #[arg(long)]
        path: Option<String>,

        /// Optional format (e.g. "llms-txt")
        #[arg(long)]
        format: Option<String>,
    },

    /// Perform a single query search using Tantivy index
    Search {
        /// The search query
        query: String,
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Index files in a directory
    Index {
        /// Directory to index
        #[arg(default_value = ".")]
        dir: String,
    },

    /// Run comparison benchmark between Baseline and BM25 index
    Benchmark {
        #[arg(short, long, default_value = ".")]
        dir: String,
        #[arg(short, long)]
        queries: String,
    },
}

// The MCP server is a sequential, single-client stdio loop (read line → handle → write,
// one at a time) and nothing is `tokio::spawn`ed, so there is no concurrent task that
// blocking I/O could starve. A single-threaded runtime right-sizes it — no worker
// threadpool — which is why the brief's "move blocking I/O off the async path" is N/A
// here (Child 04).
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Quiet by default: dependency INFO noise (tantivy commit/GC/`save metas` on every
    // search) stays off stderr. `RUST_LOG` overrides — e.g. `RUST_LOG=debug` restores full
    // diagnostics. Diagnostics always go to stderr; stdout is the MCP JSON-RPC stream.
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,codemap_search=info"));
    tracing_subscriber::fmt()
        .with_env_filter(log_filter)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // Resolve config once (repo `.codemap/config.toml` + global, repo>global>default)
    // before any command runs, so the CLI and `mcp` mode read the same resolved values.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    codemap_search::config::init(&cwd);

    match &cli.command {
        Commands::Parse { file } => {
            let path = Path::new(&file);
            if !path.exists() {
                eprintln!("Error: File '{}' not found", file);
                std::process::exit(1);
            }
            let content = std::fs::read_to_string(path)?;
            let extractor = TreeSitterExtractor::new();
            let extracted = extractor.extract(&content, file)?;
            let json = serde_json::to_string_pretty(&extracted)?;
            println!("{}", json);
        }
        Commands::Tokenize { identifier } => {
            let tokens = parser::split_identifier(identifier);
            for token in tokens {
                println!("{}", token);
            }
        }
        Commands::Codemap { path, format } => {
            let cwd = std::env::current_dir()?;

            if let Some(ref p) = path {
                let target_path = cwd.join(p);
                if !target_path.exists() {
                    eprintln!("Error: Path '{}' not found", p);
                    std::process::exit(1);
                }
            }

            let mut extracted_files = Vec::new();
            let extractor = TreeSitterExtractor::new();

            // Shared walker: EXCLUDED_DIRS + .gitignore/.codemapignore (Child 04), so the
            // CLI codemap matches the MCP overview and never traverses node_modules/.git.
            for entry in codemap_search::tools::build_walker(&cwd, false)
                .build()
                .filter_map(|e| e.ok())
            {
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                        if codemap_search::tools::is_source_extension(ext) {
                            if let Ok(rel_path) = file_path.strip_prefix(&cwd) {
                                let rel_path_str = rel_path.to_string_lossy().to_string();
                                if let Some(content) =
                                    codemap_search::tools::read_source_for_parse(file_path)
                                {
                                    if let Ok(extracted) =
                                        extractor.extract(&content, &rel_path_str)
                                    {
                                        extracted_files.push(extracted);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(ref p) = path {
                let target_path = cwd.join(p);
                if target_path.is_file() {
                    if let Ok(rel_path) = target_path.strip_prefix(&cwd) {
                        let rel_path_str = rel_path.to_string_lossy().to_string();
                        if let Some(file) =
                            extracted_files.iter().find(|f| f.file_path == rel_path_str)
                        {
                            let view = CodemapGenerator::generate_detail_view(file);
                            println!("{}", view);
                            return Ok(());
                        }
                    }
                    eprintln!("Error: Failed to process file '{}'", p);
                    std::process::exit(1);
                } else {
                    let view = CodemapGenerator::generate_folder_view(&extracted_files, p);
                    println!("{}", view);
                }
            } else {
                if format.as_deref() == Some("llms-txt") {
                    let view = CodemapGenerator::generate_llms_txt_view(&extracted_files);
                    println!("{}", view);
                } else {
                    let view = CodemapGenerator::generate_root_view(&extracted_files);
                    println!("{}", view);
                }
            }
        }
        Commands::Mcp => {
            let engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            let extractor = TreeSitterExtractor::new();
            let mut server = mcp::McpServer::new(engine, extractor);
            server.run().await?;
        }
        Commands::Search { query, limit } => {
            let engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            let results = engine.search(query, *limit)?;
            for result in results {
                println!("{}", result.file_path);
            }
        }
        Commands::Index { dir } => {
            let mut engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            engine.index_files(&[dir])?;
            println!("Indexed directory {}", dir);
        }
        Commands::Benchmark { dir, queries } => {
            let extractor = TreeSitterExtractor::new();
            let mut engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            benchmark::BenchmarkEngine::run_benchmark(queries, &extractor, &mut engine, dir)?;
        }
    }

    Ok(())
}
