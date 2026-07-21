use clap::{Parser, Subcommand};
use codemap_search::codemap::CodemapGenerator;
use codemap_search::index::{SearchEngine, SearchQueryContext};
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
        #[arg(long)]
        language_hint: Option<String>,
        #[arg(long)]
        extension_hint: Option<String>,
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
// one at a time) and nothing is `tokio::spawn`ed, so a single-threaded runtime right-sizes
// it — no worker threadpool. Indexing is the one heavy blocking job, and it runs on a
// dedicated OS thread (`codemap-indexer`, see `indexer.rs`) that is independent of the tokio
// runtime, so the async request path never blocks on it and `current_thread` stays correct.
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
                let target_path = cwd.join(codemap_search::workspace::path_from_workspace_input(p));
                if !target_path.exists() {
                    eprintln!("Error: Path '{}' not found", p);
                    std::process::exit(1);
                }
            }

            let mut extracted_files = Vec::new();
            let extractor = TreeSitterExtractor::new();

            // Shared walker: EXCLUDED_DIRS + .gitignore/.codemapignore (Child 04), so the
            // CLI codemap matches the MCP overview and never traverses node_modules/.git.
            for entry in codemap_search::workspace::build_walker(&cwd, false)
                .build()
                .filter_map(|e| e.ok())
            {
                let file_path = entry.path();
                if !file_path.is_file()
                    || !codemap_search::workspace::is_supported_source_path(file_path)
                {
                    continue;
                }
                let rel_path_str =
                    codemap_search::workspace::workspace_relative_key(file_path, &cwd);
                if let Some(content) = codemap_search::workspace::read_source_for_parse(file_path) {
                    if let Ok(extracted) = extractor.extract(&content, &rel_path_str) {
                        extracted_files.push(extracted);
                    }
                }
            }

            if let Some(ref p) = path {
                let target_path = cwd.join(codemap_search::workspace::path_from_workspace_input(p));
                if target_path.is_file() {
                    let rel_path_str =
                        codemap_search::workspace::workspace_relative_key(&target_path, &cwd);
                    if let Some(file) = extracted_files.iter().find(|f| f.file_path == rel_path_str)
                    {
                        let view = CodemapGenerator::generate_detail_view(file);
                        println!("{}", view);
                        return Ok(());
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
                } else if let Some(view) =
                    codemap_search::codemap::generate_monorepo_root_view(&extracted_files)
                {
                    println!("{}", view);
                } else {
                    let view = CodemapGenerator::generate_root_view(&extracted_files);
                    println!("{}", view);
                }
            }
        }
        Commands::Mcp => {
            codemap_search::workspace::ensure_index_root_is_not_user_home(&cwd)?;
            // When `[update].config_auto_update` is true, scaffold `.codemap/config.toml`
            // when absent, else incrementally sync it to the current schema version
            // (additive, presence-guarded, a no-op when already current) — `mcp` only,
            // never on parse/search/etc. Never-exit: a read/write failure warns and the
            // server still boots.
            codemap_search::config::ensure_repo_config(&cwd);
            codemap_search::config::reload(&cwd);
            let _config_watcher = codemap_search::config::spawn_config_watcher(&cwd);
            let engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            // Read-only search handle for the request loop; the engine (the single tantivy
            // writer) moves into the background indexer, which starts the initial index pass
            // immediately so the first request need not block on it.
            let searcher = engine.searcher_handle();
            let indexer = index::spawn_indexer(engine);
            // Health gate shared with the server: stays unhealthy when `watch = false` or
            // the watch fails to start, which keeps the request-triggered fallback active.
            let watcher_status = std::sync::Arc::new(index::WatcherStatus::default());
            let watcher = codemap_search::config::get().watch.then(|| {
                index::spawn_watcher(
                    &cwd,
                    indexer.command_sender(),
                    std::sync::Arc::clone(&watcher_status),
                )
            });
            // The supervisor owns all the handles so it can rebuild them when the indexer
            // dies (`indexer_auto_restart`); its field order guarantees the shutdown
            // sequence — watcher dropped (thread joined, its command-sender clone
            // released) before IndexerHandle::drop closes the channel and joins the
            // indexer, whose recv loop ends only when ALL senders are gone.
            let supervisor =
                index::EngineSupervisor::new(searcher, watcher.flatten(), indexer, watcher_status);
            let mut server = mcp::McpServer::new(supervisor);
            server.run().await?;
            // server drop → EngineSupervisor drop → watcher field drop → indexer field drop.
        }
        Commands::Search {
            query,
            limit,
            language_hint,
            extension_hint,
        } => {
            codemap_search::workspace::ensure_index_root_is_not_user_home(&cwd)?;
            let engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            let search_context = SearchQueryContext {
                language_hint: language_hint.clone(),
                extension_hint: extension_hint.clone(),
            };
            let results = engine.search_with_context(query, *limit, &search_context)?;
            for result in results {
                println!("{}", result.file_path);
            }
        }
        Commands::Index { dir } => {
            codemap_search::workspace::ensure_index_root_is_not_user_home(Path::new(dir))?;
            let mut engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            engine.index_files(&[dir])?;
            println!("Indexed directory {}", dir);
        }
        Commands::Benchmark { dir, queries } => {
            codemap_search::workspace::ensure_index_root_is_not_user_home(Path::new(dir))?;
            let extractor = TreeSitterExtractor::new();
            let mut engine =
                index::TantivySearchEngine::new(&codemap_search::config::get().index_path)?;
            benchmark::BenchmarkEngine::run_benchmark(queries, &extractor, &mut engine, dir)?;
        }
    }

    Ok(())
}
