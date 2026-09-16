use super::*;
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Development-only oracle adapter. Expected labels never enter the analyzer.
#[test]
fn test_native_corpus() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let corpus = app.join("experiments/event-navigation-poc/language-corpus");
    let root = app.parent().unwrap().parent().unwrap();
    let ledger = read_json(&root.join("docs/briefs/evidence/source-routes/coverage.json"));
    for (path, hash) in ledger["manifests"].as_object().unwrap() {
        assert_eq!(
            Some(source_sha256(&root.join(path)).as_str()),
            hash.as_str(),
            "immutable manifest {path}"
        );
    }
    let lock = read_json(&corpus.join("sources.lock.json"));
    let groups = selection("CODEMAP_SOURCE_ROUTES_GROUPS", "regressions,fixtures");
    let languages = selection("CODEMAP_SOURCE_ROUTES_LANGUAGES", "all");
    assert!(
        languages
            .iter()
            .all(|language| language == "all" || syntax::supported(language)),
        "unsupported language selection"
    );
    let limit = std::env::var("CODEMAP_SOURCE_ROUTES_MAX_FILE_BYTES")
        .ok()
        .map(|s| s.parse::<usize>().expect("integer byte limit"))
        .unwrap_or(super::super::SOURCE_BYTES_PER_FILE);
    assert!(
        limit == 524_288 || (limit == 1_048_576 && groups == BTreeSet::from(["expanded".into()])),
        "1 MiB is reserved for the expanded validation invocation"
    );
    for group in &groups {
        assert!(
            matches!(
                group.as_str(),
                "regressions" | "fixtures" | "public" | "expanded" | "consumers" | "frontend"
            ),
            "unsupported corpus group: {group}"
        );
    }
    if groups == BTreeSet::from(["frontend".into()]) {
        super::tests_frontend::run();
        return;
    }
    if groups
        .iter()
        .any(|g| matches!(g.as_str(), "public" | "expanded" | "consumers" | "frontend"))
    {
        assert_eq!(
            groups.len(),
            1,
            "run public, expanded, consumers and frontend as separate invocations"
        );
        super::tests_public::run(groups.first().unwrap(), &languages, limit);
        return;
    }
    let mut cases = Vec::new();
    if groups.contains("regressions") {
        for case in read_json(&corpus.join("regressions/cases.json"))
            .as_array()
            .unwrap()
        {
            cases.push(("regressions", case.clone()));
        }
    }
    if groups.contains("fixtures") {
        for case in read_json(&corpus.join("fixture-cases.json"))["cases"]
            .as_array()
            .unwrap()
        {
            cases.push(("fixtures", case.clone()));
        }
    }
    cases.retain(|(_, c)| {
        languages.contains("all") || languages.contains(c["language"].as_str().unwrap())
    });
    assert!(!cases.is_empty(), "empty native corpus selection");
    let mut analyses: BTreeMap<String, Json> = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut results = Vec::new();
    for (group, case) in cases {
        let (root, relative, storage, invocation, expected) = if group == "regressions" {
            let path = Path::new(case["path"].as_str().unwrap());
            let root = corpus
                .join("regressions/examples")
                .join(path.parent().unwrap());
            let relative = path.file_name().unwrap().to_str().unwrap().to_owned();
            let storage = json!([
                case["storage_file"].as_str().unwrap_or(&relative),
                case["storage_line"],
                case["storage_line"]
            ]);
            let invocation = json!([
                case["invocation_file"].as_str().unwrap_or(&relative),
                case["invocation_line"],
                case["invocation_line"]
            ]);
            (
                root,
                relative,
                storage,
                invocation,
                case["expected"].as_str().unwrap() == "connected",
            )
        } else {
            (
                corpus.clone(),
                case["storage"][0].as_str().unwrap().to_owned(),
                case["storage"].clone(),
                case["invocation"].clone(),
                case["kind"] == "positive",
            )
        };
        let cache_key = format!("{}|{relative}", root.display());
        if !analyses.contains_key(&cache_key) {
            println!("ANALYZE {}", case["id"].as_str().unwrap());
            let mut sources = BTreeMap::new();
            let primary = root.join(&relative);
            let data = std::fs::read_to_string(&primary).expect("locked source file");
            assert!(
                data.len() <= limit,
                "input exceeds selected validation size: {}",
                primary.display()
            );
            let hash = source_sha256(&primary);
            if group == "regressions" {
                assert_eq!(
                    Some(hash.as_str()),
                    case["source_sha256"].as_str(),
                    "source hash: {}",
                    primary.display()
                );
            } else {
                assert_eq!(
                    Some(hash.as_str()),
                    lock["fixtures"][&relative].as_str(),
                    "fixture source hash: {relative}"
                );
            }
            hashes.insert(primary.display().to_string(), hash);
            sources.insert(relative.clone(), data);
            for kind in ["additional_sources", "support_sources"] {
                if let Some(additional) = case[kind].as_object() {
                    for (path, expected_hash) in additional {
                        let file = root.join(path).canonicalize().unwrap();
                        assert!(
                            file.starts_with(root.canonicalize().unwrap()),
                            "fixture source escapes input root"
                        );
                        let expected_hash = if kind == "support_sources" {
                            &expected_hash["sha256"]
                        } else {
                            expected_hash
                        };
                        let hash = source_sha256(&file);
                        assert_eq!(Some(hash.as_str()), expected_hash.as_str());
                        hashes.insert(file.display().to_string(), hash);
                        let data = std::fs::read_to_string(file).unwrap();
                        assert!(data.len() <= limit);
                        sources.insert(path.clone(), data);
                    }
                }
            }
            let bindings = case["module_bindings"]
                .as_object()
                .map(|values| {
                    values
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                        .collect()
                })
                .unwrap_or_default();
            let analysis = analyze_with_bindings(&sources, &bindings);
            analyses.insert(cache_key.clone(), serde_json::to_value(analysis).unwrap());
        }
        let analysis = &analyses[&cache_key];
        let matches: Vec<_> = analysis["relations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                endpoint(r, "storage", &storage)
                    && endpoint(r, "invocation", &invocation)
                    && r["storage"]["value"]
                        .as_str()
                        .unwrap_or_default()
                        .ends_with(case["storage_value_suffix"].as_str().unwrap_or_default())
                    && case["invocation_via"].as_array().is_none_or(|required| {
                        required.iter().all(|point| {
                            r["invocation"]["via"].as_array().is_some_and(|via| {
                                via.iter().any(|actual| {
                                    actual["path"] == point[0] && actual["line"] == point[1]
                                })
                            })
                        })
                    })
            })
            .collect();
        let positives: Vec<_> = matches
            .iter()
            .filter(|r| {
                let kind = r["kind"].as_str().unwrap();
                let relevant = match case["semantic_kind"].as_str().unwrap_or("callback") {
                    "data_return" => kind == "stored_value_return",
                    "data_consumption" => matches!(
                        kind,
                        "stored_object_write" | "stored_object_read" | "stored_key_lookup"
                    ),
                    _ => matches!(
                        kind,
                        "storage_to_invocation" | "stored_object_method_candidate"
                    ),
                };
                relevant
                    && case["relation_kinds"]
                        .as_array()
                        .is_none_or(|kinds| kinds.contains(&r["kind"]))
                    && case["required_conditions"]
                        .as_array()
                        .is_none_or(|required| {
                            required
                                .iter()
                                .all(|c| r["conditions"].as_array().unwrap().contains(c))
                        })
            })
            .collect();
        let parse_errors: Vec<_> = analysis["notices"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n[0] == "parse_error")
            .collect();
        let has_contract_error = analysis["relations"].as_array().unwrap().iter().any(|r| {
            r["certainty"] != "conditional_source_relation"
                || r["concrete_instance_proven"] != false
                || r["event_classification"] != "not_inferred"
        });
        let passed = (if expected {
            !positives.is_empty()
        } else {
            matches.is_empty()
        }) && parse_errors.is_empty()
            && !has_contract_error;
        results.push(json!({"group":group,"id":case["id"],"language":case["language"],"expected":case,"passed":passed,"matching_relations":matches,"positive_matches":positives.len(),"parse_errors":parse_errors,"has_contract_error":has_contract_error,"analysis_key":cache_key}));
    }
    let control_status: BTreeMap<_, _> = results
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap().to_owned(),
                row["passed"] == true && row["positive_matches"].as_u64().unwrap() > 0,
            )
        })
        .collect();
    for row in &mut results {
        if let Some(control) = row["expected"]["positive_control"].as_str() {
            if control != row["id"].as_str().unwrap() {
                let has_control = control_status.get(control) == Some(&true);
                row["has_positive_control"] = json!(has_control);
                row["passed"] = json!(row["passed"] == true && has_control);
            }
        }
    }
    let failures: Vec<_> = results
        .iter()
        .filter(|r| r["passed"] != true)
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    for row in &results {
        println!(
            "{} {}: {} matches",
            if row["passed"] == true {
                "PASS"
            } else {
                "FAIL"
            },
            row["id"].as_str().unwrap(),
            row["matching_relations"].as_array().unwrap().len()
        );
    }
    let report = json!({"groups":groups,"languages":languages,"max_file_bytes":limit,"cases":results,"analyses":analyses,"source_sha256":hashes,"case_count":results.len(),"passed":results.len()-failures.len(),"failures":failures,"source_digest_algorithm":"blake3","target_programs_executed":false,"compiler_auxiliary_input_used":false});
    if let Ok(path) = std::env::var("CODEMAP_SOURCE_ROUTES_REPORT") {
        let root = app.parent().unwrap().parent().unwrap();
        let path = root.join(path);
        super::tests_reporting::write_report(&path, report);
    }
    assert!(
        failures.is_empty(),
        "native corpus: {} of {} failed: {}",
        failures.len(),
        results.len(),
        failures.join(", ")
    );
}
pub(super) fn read_json(path: &Path) -> Json {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn selection(key: &str, default: &str) -> BTreeSet<String> {
    std::env::var(key)
        .unwrap_or_else(|_| default.into())
        .split(',')
        .map(str::to_owned)
        .collect()
}
pub(super) fn endpoint(relation: &Json, side: &str, point: &Json) -> bool {
    let location = &relation[side]["location"];
    location["path"] == point[0]
        && location["line"].as_u64().is_some_and(|line| {
            line >= point[1].as_u64().unwrap() && line <= point[2].as_u64().unwrap()
        })
}
pub(super) fn source_sha256(path: &Path) -> String {
    // Developer-only hash verification, using the existing system utility; the
    // production analyzer uses the project's blake3 dependency and no process.
    let output = std::process::Command::new("shasum")
        .arg("-a")
        .arg("256")
        .arg(path)
        .output()
        .expect("shasum for immutable PoC source verification");
    assert!(output.status.success(), "source hash command failed");
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
