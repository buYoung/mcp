mod engine;
use engine::{compile, Document, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, hint::black_box, time::Instant};

#[derive(Deserialize)]
struct Case { name: String, pattern: String, corpus: String, modes: Vec<String>, origin: String }
#[derive(Deserialize)]
struct Pattern { name: String, pattern: String }
#[derive(Deserialize)]
struct Probe { name: String, pattern: String, text: String }
#[derive(Deserialize)]
struct Input { corpora: BTreeMap<String, Vec<Document>>, cases: Vec<Case>, inventory: Vec<Pattern>, probes: Vec<Probe> }

fn records(engine: &Engine, mode: &str, documents: &[Document]) -> Result<Vec<[usize; 4]>, String> {
    let mut values = Vec::new();
    engine.visit(mode, documents, |value| values.push(value))?;
    Ok(values)
}

fn checksum(engine: &Engine, mode: &str, documents: &[Document]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    engine.visit(mode, black_box(documents), |record| {
        for value in record { hash = (hash ^ value as u64).wrapping_mul(0x100000001b3); }
    }).expect("validated workload must not fail at runtime");
    hash
}

fn run_operation(name: &str, compiled: &Engine, pattern: &str, mode: &str, scan_mode: &str, documents: &[Document]) -> u64 {
    if mode == "compile" {
        black_box(compile(name, black_box(pattern), name == "grep_regex").unwrap());
        1
    } else if mode == "request" {
        let compiled = compile(name, black_box(pattern), scan_mode == "grep").unwrap();
        checksum(&compiled, scan_mode, documents)
    } else {
        checksum(compiled, mode, documents)
    }
}

fn thread_cpu_ns() -> u64 {
    let mut timestamp = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // Both measured engines and the harness execute on this one thread.
    let result = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut timestamp) };
    assert_eq!(result, 0, "thread CPU clock unavailable: {}", std::io::Error::last_os_error());
    timestamp.tv_sec as u64 * 1_000_000_000 + timestamp.tv_nsec as u64
}

fn timed_batch(iterations: u64, mut operation: impl FnMut() -> u64) -> (f64, f64) {
    let start = Instant::now();
    let cpu_start = thread_cpu_ns();
    for _ in 0..iterations { black_box(operation()); }
    let cpu_elapsed = thread_cpu_ns() - cpu_start;
    (start.elapsed().as_nanos() as f64 / iterations as f64, cpu_elapsed as f64 / iterations as f64)
}

fn calibrate(target_ns: f64, mut operation: impl FnMut() -> u64) -> u64 {
    for _ in 0..3 { black_box(operation()); }
    let mut iterations = 1;
    loop {
        let (ns, _) = timed_batch(iterations, &mut operation);
        if ns * iterations as f64 >= 2_000_000.0 || iterations >= 1_048_576 {
            return (target_ns / ns).ceil().clamp(1.0, 1_048_576.0) as u64;
        }
        iterations *= 2;
    }
}

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0
    }
    fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() { let j = self.next() as usize % (i + 1); values.swap(i, j); }
    }
}

fn stats(values: &[f64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        let position = (sorted.len() - 1) as f64 * p;
        let lower = sorted[position.floor() as usize];
        let upper = sorted[position.ceil() as usize];
        lower + (upper - lower) * position.fract()
    };
    let median = percentile(0.5);
    let mut deviations: Vec<_> = sorted.iter().map(|v| (v - median).abs()).collect();
    deviations.sort_by(f64::total_cmp);
    json!({"median_ns": median, "min_ns": sorted[0], "p10_ns": percentile(0.1),
           "p90_ns": percentile(0.9), "max_ns": sorted[sorted.len()-1],
           "mad_ns": (deviations[(deviations.len()-1)/2] + deviations[deviations.len()/2]) / 2.0})
}

fn compatibility(input: &Input) -> Value {
    let mut probes = Vec::new();
    for probe in &input.probes {
        let documents = [Document { name: probe.name.clone(), text: probe.text.clone() }];
        let results: Vec<_> = ["regex", "re2", "fancy_regex"].into_iter().map(|name| match compile(name, &probe.pattern, false) {
            Ok(engine) => {
                let start = Instant::now();
                match records(&engine, "captures", &documents) {
                    Ok(values) => json!({"engine": name, "records": values, "elapsed_ns": start.elapsed().as_nanos()}),
                    Err(error) => json!({"engine": name, "runtime_error": error, "elapsed_ns": start.elapsed().as_nanos()}),
                }
            },
            Err(error) => json!({"engine": name, "error": error}),
        }).collect();
        probes.push(json!({"name": probe.name, "pattern": probe.pattern, "text": probe.text,
                          "same_records": results[0].get("records").is_some() && results.iter().all(|r| r["records"] == results[0]["records"]),
                          "results": results}));
    }
    let mut inventory = Vec::new();
    for pattern in &input.inventory {
        inventory.push(json!({"name": pattern.name, "pattern": pattern.pattern,
                              "regex_error": compile("regex", &pattern.pattern, false).err(),
                              "fancy_regex_error": compile("fancy_regex", &pattern.pattern, false).err(),
                              "re2_error": compile("re2", &pattern.pattern, false).err()}));
    }
    json!({"probes": probes, "pii_compile_inventory": inventory})
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() >= 6, "input.json output.json samples sample_ms seed [--validate-only]");
    let input: Input = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let sample_count: usize = args[3].parse().unwrap();
    let sample_ms: u64 = args[4].parse().unwrap();
    let seed: u64 = args[5].parse().unwrap();
    let is_validation = args.get(6).is_some_and(|a| a == "--validate-only");
    assert!(seed != 0 && sample_count >= 3 && sample_ms >= 1);
    let mut random = Random(seed);
    let compatibility = compatibility(&input);
    let mut jobs = Vec::new();
    for (index, case) in input.cases.iter().enumerate() {
        for mode in &case.modes { jobs.push((index, mode.as_str())); }
    }
    random.shuffle(&mut jobs);
    let mut rows = Vec::new();
    for (case_index, mode) in jobs {
        let case = &input.cases[case_index];
        let documents = &input.corpora[&case.corpus];
        let scan_mode = if mode == "request" || mode == "compile" {
            if case.modes.iter().any(|m| m == "grep") { "grep" }
            else if case.modes.iter().any(|m| m == "captures") { "captures" }
            else if case.modes.iter().any(|m| m == "is_match") { "is_match" }
            else { "find" }
        } else { mode };
        let names = if mode == "compile" && scan_mode == "grep" { vec!["regex", "re2", "fancy_regex", "grep_regex"] }
                    else if scan_mode == "grep" { vec!["grep_regex", "re2", "fancy_regex"] }
                    else { vec!["regex", "re2", "fancy_regex"] };
        let mut engines = Vec::new();
        let mut errors = BTreeMap::new();
        for name in &names {
            match compile(name, &case.pattern, scan_mode == "grep") {
                Ok(engine) => engines.push(engine),
                Err(error) => { errors.insert(*name, error); }
            }
        }
        if !errors.is_empty() {
            rows.push(json!({"case": case.name, "mode": mode, "corpus": case.corpus, "errors": errors, "status": "compile_error"}));
            continue;
        }
        // Compare every match/capture/line offset, not only total match counts.
        // Compile timing uses successful construction as its correctness gate.
        let vectors: Result<Vec<_>, _> = if mode == "compile" { Ok(vec![vec![]; names.len()]) }
            else { engines.iter().map(|engine| records(engine, scan_mode, documents)).collect() };
        let vectors = match vectors {
            Ok(values) => values,
            Err(error) => {
                rows.push(json!({"case": case.name, "mode": mode, "status": "runtime_error", "error": error}));
                continue;
            }
        };
        let is_equal = vectors.iter().all(|v| *v == vectors[0]);
        let mut row = json!({"case": case.name, "mode": mode, "scan_mode": scan_mode,
                            "corpus": case.corpus, "pattern": case.pattern, "origin": case.origin,
                            "status": if is_equal {"equal"} else {"mismatch"},
                            "input_bytes": documents.iter().map(|d| d.text.len()).sum::<usize>(),
                            "input_documents": documents.len(),
                            "records": vectors.iter().map(Vec::len).collect::<Vec<_>>(), "engine_order": names});
        if !is_equal {
            let different = vectors.iter().position(|v| *v != vectors[0]).unwrap();
            let first = vectors[0].iter().zip(&vectors[different]).position(|(a,b)| a != b).unwrap_or(vectors[0].len().min(vectors[different].len()));
            row["first_difference"] = json!({"index": first,
                "records": vectors.iter().map(|v| v.get(first)).collect::<Vec<_>>(),
                "documents": vectors.iter().filter_map(|v| v.get(first).map(|r| &documents[r[0]].name)).collect::<Vec<_>>()});
        }
        if is_equal && !is_validation {
            let batches: Vec<_> = engines.iter().enumerate().map(|(i, compiled)| calibrate(sample_ms as f64 * 1e6,
                || run_operation(names[i], compiled, &case.pattern, mode, scan_mode, documents))).collect();
            let mut samples = vec![Vec::new(); names.len()];
            let mut cpu_samples = vec![Vec::new(); names.len()];
            let mut orders = Vec::new();
            for _ in 0..sample_count {
                let mut order: Vec<_> = (0..names.len()).collect();
                random.shuffle(&mut order);
                for &i in &order {
                    let (wall_ns, cpu_ns) = timed_batch(batches[i], || run_operation(names[i], &engines[i], &case.pattern, mode, scan_mode, documents));
                    samples[i].push(wall_ns);
                    cpu_samples[i].push(cpu_ns);
                }
                orders.push(order);
            }
            let metrics: BTreeMap<_, _> = names.iter().enumerate().map(|(i, name)| (*name,
                json!({"iterations_per_sample": batches[i], "samples_ns_per_operation": samples[i], "stats": stats(&samples[i]),
                       "samples_cpu_ns_per_operation": cpu_samples[i], "cpu_stats": stats(&cpu_samples[i])}))).collect();
            row["timings"] = json!(metrics);
            row["sample_orders"] = json!(orders);
        }
        eprintln!("{} {}: {}", case.name, mode, row["status"]);
        rows.push(row);
    }
    let output = json!({"schema_version": 2, "samples": sample_count, "sample_ms": sample_ms, "seed": seed,
                        "validate_only": is_validation, "compatibility": compatibility, "results": rows});
    std::fs::write(&args[2], serde_json::to_vec_pretty(&output).unwrap()).unwrap();
}
