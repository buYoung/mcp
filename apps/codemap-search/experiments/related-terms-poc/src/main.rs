//! Related-terms phase-1 PoC (internal experiment; not a product CLI).
//!
//! Serving commands (`lookup`, `stats-words`) read only the dataset SQLite, published
//! statistics, and the index `meta.json`. Index-time commands (`index`, `verify-stats`,
//! `raw-doc-freq`) own every Tantivy access.

mod dataset;
mod lookup;
mod normalize;
mod stats;
mod vocabulary;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

const USAGE: &str = "usage:
  rtpoc index --state DIR [--fail-before-publish]      (run from the workspace root)
  rtpoc verify-stats --state DIR
  rtpoc raw-doc-freq --state DIR WORD...
  rtpoc stats-words --state DIR WORD...
  rtpoc lookup --dataset FILE --policy FILE [--state DIR] [--expect-dataset-id ID]
               [--repeat N] [--output FILE] [--timing-output FILE] (--terms-file FILE | TERM...)
  rtpoc normalize-check --dataset FILE";

struct Args {
    command: String,
    options: std::collections::HashMap<String, String>,
    flags: std::collections::HashSet<String>,
    positional: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut raw = std::env::args().skip(1);
    let command = raw.next().ok_or(USAGE)?;
    let mut options = std::collections::HashMap::new();
    let mut flags = std::collections::HashSet::new();
    let mut positional = Vec::new();
    let raw: Vec<String> = raw.collect();
    let mut index = 0;
    while index < raw.len() {
        let arg = &raw[index];
        if arg == "--fail-before-publish" {
            flags.insert(arg.clone());
        } else if let Some(name) = arg.strip_prefix("--") {
            let value = raw.get(index + 1).ok_or(format!("--{name} needs a value"))?;
            options.insert(name.to_string(), value.clone());
            index += 1;
        } else {
            positional.push(arg.clone());
        }
        index += 1;
    }
    Ok(Args {
        command,
        options,
        flags,
        positional,
    })
}

impl Args {
    fn path(&self, name: &str) -> Result<PathBuf, String> {
        self.options
            .get(name)
            .map(PathBuf::from)
            .ok_or(format!("--{name} is required\n{USAGE}"))
    }
}

fn absolute_state_dir(path: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
    path.canonicalize().map_err(|error| error.to_string())
}

fn print_json(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
}

fn run() -> Result<(), String> {
    let process_started = Instant::now();
    let args = parse_args()?;
    match args.command.as_str() {
        "index" => {
            let state = absolute_state_dir(&args.path("state")?)?;
            let report = vocabulary::index_and_publish(&state, args.flags.contains("--fail-before-publish"))?;
            print_json(&report);
        }
        "verify-stats" => {
            let report = vocabulary::verify_published(&args.path("state")?)?;
            let is_pass = report["status"] == "pass";
            print_json(&report);
            if !is_pass {
                return Err("statistics verification failed".to_string());
            }
        }
        "raw-doc-freq" => print_json(&vocabulary::raw_doc_freq(&args.path("state")?, &args.positional)?),
        "stats-words" => {
            let repository = stats::RepositoryStats::open(&args.path("state")?);
            let counts: serde_json::Map<String, serde_json::Value> = args
                .positional
                .iter()
                .map(|word| (word.clone(), serde_json::json!(repository.file_count(word))))
                .collect();
            print_json(&serde_json::json!({"repository": repository.state, "file_counts": counts}));
        }
        "lookup" => run_lookup(&args, process_started)?,
        "normalize-check" => print_json(&normalize_check(&dataset::Dataset::open(&args.path("dataset")?)?)?),
        _ => return Err(USAGE.to_string()),
    }
    Ok(())
}

fn run_lookup(args: &Args, process_started: Instant) -> Result<(), String> {
    let terms: Vec<String> = match args.options.get("terms-file") {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| error.to_string())?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect(),
        None => args.positional.clone(),
    };
    if terms.is_empty() {
        return Err(format!("no lookup terms\n{USAGE}"));
    }
    let repeat: usize = args
        .options
        .get("repeat")
        .map(|value| value.parse().map_err(|_| "--repeat must be a number".to_string()))
        .transpose()?
        .unwrap_or(1)
        .max(1);

    let dataset_started = Instant::now();
    let dataset = dataset::Dataset::open(&args.path("dataset")?)?;
    let dataset_open_us = dataset_started.elapsed().as_micros();
    if let Some(expected) = args.options.get("expect-dataset-id") {
        if &dataset.identity.dataset_id != expected {
            return Err(format!(
                "dataset incompatible: {} != expected {expected}",
                dataset.identity.dataset_id
            ));
        }
    }
    let policy = lookup::LookupPolicy::load(&args.path("policy")?)?;
    let stats_started = Instant::now();
    let repository = match args.options.get("state") {
        Some(state) => stats::RepositoryStats::open(Path::new(state)),
        None => stats::RepositoryStats::not_configured(),
    };
    let stats_open_us = stats_started.elapsed().as_micros();
    let ready_us = process_started.elapsed().as_micros();

    let mut first_pass = Vec::new();
    let mut passes = Vec::new();
    let mut is_warm_identical = true;
    for pass in 0..repeat {
        let pass_started = Instant::now();
        let mut per_lookup_us = Vec::with_capacity(terms.len());
        for (position, term) in terms.iter().enumerate() {
            let output = lookup::lookup(&dataset, &repository, &policy, term)?;
            per_lookup_us.push(output.lookup_us);
            let mut value = serde_json::to_value(&output).map_err(|error| error.to_string())?;
            value["lookup_us"] = serde_json::Value::Null;
            if pass == 0 {
                first_pass.push((output, value));
            } else if first_pass[position].1 != value {
                is_warm_identical = false;
            }
        }
        passes.push(serde_json::json!({
            "pass": pass,
            "condition": if pass == 0 { "first_in_process" } else { "repeat_in_process" },
            "lookups": terms.len(),
            "total_us": pass_started.elapsed().as_micros(),
            "per_lookup_us": per_lookup_us,
        }));
    }

    let mut writer: Box<dyn Write> = match args.options.get("output") {
        Some(path) => Box::new(std::fs::File::create(path).map_err(|error| error.to_string())?),
        None => Box::new(std::io::stdout()),
    };
    for (output, _) in &first_pass {
        serde_json::to_writer(&mut writer, output).map_err(|error| error.to_string())?;
        writeln!(writer).map_err(|error| error.to_string())?;
    }
    if let Some(path) = args.options.get("timing-output") {
        let timing = serde_json::json!({
            "dataset_id": dataset.identity.dataset_id,
            "repository_status": repository.state.status,
            "process_start_to_ready_us": ready_us,
            "dataset_open_us": dataset_open_us,
            "stats_open_us": stats_open_us,
            "repeat_outputs_identical": is_warm_identical,
            "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "passes": passes,
        });
        std::fs::write(path, serde_json::to_vec_pretty(&timing).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    }
    if !is_warm_identical {
        return Err("repeated lookups produced different outputs".to_string());
    }
    Ok(())
}

/// Recompute every dataset key with this reader's `rtn-2` implementation.
fn normalize_check(dataset: &dataset::Dataset) -> Result<serde_json::Value, String> {
    let mut checked = 0usize;
    let mut mismatches = Vec::new();
    for (exact_key, token_key, is_lossy) in dataset.all_expressions()? {
        checked += 1;
        let (rust_token_key, rust_is_lossy) = normalize::token_key(&exact_key);
        if normalize::exact_key(&exact_key) != exact_key || rust_token_key != token_key || rust_is_lossy != is_lossy {
            mismatches.push(serde_json::json!({
                "exact_key": exact_key, "token_key": token_key, "lossy": is_lossy,
                "rust_token_key": rust_token_key, "rust_lossy": rust_is_lossy,
            }));
        }
    }
    let mut surfaces_checked = 0usize;
    let mut surface_mismatches = Vec::new();
    for (surface, expression_id) in dataset.all_surfaces()? {
        surfaces_checked += 1;
        if format!("x:{}", normalize::exact_key(&surface)) != expression_id {
            surface_mismatches.push(serde_json::json!({"surface": surface, "expression_id": expression_id}));
        }
    }
    Ok(serde_json::json!({
        "status": if mismatches.is_empty() && surface_mismatches.is_empty() { "pass" } else { "fail" },
        "normalization_version": normalize::NORMALIZATION_VERSION,
        "expressions_checked": checked,
        "expression_mismatches": mismatches.len(),
        "expression_mismatch_examples": mismatches.into_iter().take(20).collect::<Vec<_>>(),
        "surfaces_checked": surfaces_checked,
        "surface_mismatches": surface_mismatches.len(),
        "surface_mismatch_examples": surface_mismatches.into_iter().take(20).collect::<Vec<_>>(),
    }))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("rtpoc: {error}");
        std::process::exit(1);
    }
}
