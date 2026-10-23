//! Developer-only JSONL driver. Reads JSON strings from stdin when no terms are given.
use clap::Parser;
use codemap_search::related_terms::{prepare_dataset, LookupOptions, RelatedTermsLookup};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
struct Args {
    dataset: PathBuf,
    terms: Vec<String>,
    #[arg(long)]
    global_dir: Option<PathBuf>,
    /// Treat a source as managed/downloaded instead of user-owned.
    #[arg(long)]
    managed: bool,
    #[arg(long)]
    prepare_only: bool,
    /// Emit per-call timings to stderr, without JSON serialization or preparation time.
    #[arg(long)]
    timings: bool,
    #[arg(long)]
    skip_syntax_parts: bool,
    #[arg(long, default_value_t = 1)]
    batch_size: usize,
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    if args.batch_size == 0 {
        return Err("batch size must be positive".into());
    }
    let temporary = if args.global_dir.is_none() {
        Some(tempfile::tempdir().map_err(|e| e.to_string())?)
    } else {
        None
    };
    let global = args
        .global_dir
        .as_deref()
        .or_else(|| temporary.as_ref().map(|d| d.path()))
        .ok_or("global directory missing")?;
    let prepared = prepare_dataset(&args.dataset, global, !args.managed)?;
    if args.prepare_only {
        println!("{}", prepared.path().display());
        return Ok(());
    }
    let mut lookup = RelatedTermsLookup::with_options(
        prepared,
        LookupOptions {
            skip_syntax_parts: args.skip_syntax_parts,
        },
    );
    let stdin = io::stdin();
    let mut inputs: Box<dyn Iterator<Item = Result<String, String>>> = if args.terms.is_empty() {
        Box::new(stdin.lock().lines().map(|line| {
            line.map_err(|e| e.to_string())
                .and_then(|s| serde_json::from_str::<String>(&s).map_err(|e| e.to_string()))
        }))
    } else {
        Box::new(args.terms.into_iter().map(Ok))
    };
    let mut output = io::BufWriter::new(io::stdout().lock());
    loop {
        let batch = inputs
            .by_ref()
            .take(args.batch_size)
            .collect::<Result<Vec<_>, _>>()?;
        if batch.is_empty() {
            break;
        }
        let start = Instant::now();
        let answers = lookup.lookup_batch(&batch)?;
        if args.timings {
            eprintln!(
                "{}",
                serde_json::json!({"terms":batch,"milliseconds":start.elapsed().as_secs_f64()*1000.0})
            );
        }
        for answer in answers {
            serde_json::to_writer(&mut output, &answer).map_err(|e| e.to_string())?;
            writeln!(output).map_err(|e| e.to_string())?;
        }
        output.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
