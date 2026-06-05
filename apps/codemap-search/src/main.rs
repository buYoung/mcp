use clap::{Parser, Subcommand};
use std::path::Path;
use codemap_search::parser::{TreeSitterExtractor, CodeExtractor};
use codemap_search::codemap::CodemapGenerator;
use codemap_search::{parser, index, mcp, benchmark};
use codemap_search::index::SearchEngine;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    
    let cli = Cli::parse();
    
    match &cli.command {
        Commands::Parse { file } => {
            let path = Path::new(&file);
            if !path.exists() {
                eprintln!("Error: File '{}' not found", file);
                std::process::exit(1);
            }
            let content = std::fs::read_to_string(path)?;
            let extractor = TreeSitterExtractor::new();
            let extracted = extractor.extract(&content, &file)?;
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
            
            for entry in walkdir::WalkDir::new(&cwd)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                        if matches!(ext, "rs" | "py" | "ts" | "tsx" | "js" | "jsx") {
                            if let Ok(rel_path) = file_path.strip_prefix(&cwd) {
                                let rel_path_str = rel_path.to_string_lossy().to_string();
                                if let Ok(content) = std::fs::read_to_string(file_path) {
                                    if let Ok(extracted) = extractor.extract(&content, &rel_path_str) {
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
                        if let Some(file) = extracted_files.iter().find(|f| f.file_path == rel_path_str) {
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
            let engine = index::TantivySearchEngine::new(".codemap-index")?;
            let extractor = TreeSitterExtractor::new();
            let mut server = mcp::McpServer::new(engine, extractor);
            server.run().await?;
        }
        Commands::Search { query, limit } => {
            let engine = index::TantivySearchEngine::new(".codemap-index")?;
            let results = engine.search(query, *limit)?;
            for result in results {
                println!("{}", result.file_path);
            }
        }
        Commands::Index { dir } => {
            let mut engine = index::TantivySearchEngine::new(".codemap-index")?;
            engine.index_files(&[dir])?;
            println!("Indexed directory {}", dir);
        }
        Commands::Benchmark { dir, queries } => {
            let extractor = TreeSitterExtractor::new();
            let mut engine = index::TantivySearchEngine::new(".codemap-index")?;
            benchmark::BenchmarkEngine::run_benchmark(queries, &extractor, &mut engine, dir)?;
        }
    }
    
    Ok(())
}
