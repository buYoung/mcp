use super::tests::{endpoint, read_json, source_sha256};
use super::tests_reporting::{implementation_fingerprint, read_report, write_report};
use super::*;
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub(super) fn run(group: &str, languages: &BTreeSet<String>, limit: usize) {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let corpus = app.join("experiments/event-navigation-poc/language-corpus");
    let roots = PathBuf::from(
        std::env::var("CODEMAP_SOURCE_ROUTES_SOURCE_ROOT")
            .expect("locked public source root is required"),
    );
    let composite = PathBuf::from(
        std::env::var("CODEMAP_SOURCE_ROUTES_COMPOSITE_ROOT")
            .unwrap_or_else(|_| "/tmp/codemap-language-composite-sources".into()),
    );
    let labels = read_json(&corpus.join("cases.json"))["cases"]
        .as_array()
        .unwrap()
        .clone();
    let base = read_json(&corpus.join("results/evaluation.json"));
    let specs = read_json(&corpus.join("repositories.json"));
    let lock = read_json(&corpus.join("sources.lock.json"));
    let variants = read_json(&corpus.join("remaining-routes/inputs.json"));
    let selected_variants = std::env::var("CODEMAP_SOURCE_ROUTES_VARIANTS")
        .ok()
        .map(|v| v.split(',').map(str::to_owned).collect::<BTreeSet<_>>());
    if let Some(names) = &selected_variants {
        assert!(
            matches!(group, "expanded" | "consumers")
                && names.iter().all(|n| variants["variants"].get(n).is_some()),
            "unsupported variant selection"
        );
    }
    let mut jobs = Vec::new();
    if group == "public" {
        let selected: BTreeSet<_> = labels
            .iter()
            .filter(|c| {
                languages.contains("all") || languages.contains(c["language"].as_str().unwrap())
            })
            .map(|c| {
                (
                    c["repository"].as_str().unwrap().to_owned(),
                    c["language"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        for (repo, language) in selected {
            jobs.push((
                format!("{repo}--{language}"),
                roots.join(&repo),
                repo.clone(),
                language,
                lock["repositories"][&repo]["source_sha256"].clone(),
                Json::Null,
                base.clone(),
            ));
        }
    } else if matches!(group, "expanded" | "consumers") {
        for (name, spec) in variants["variants"].as_object().unwrap() {
            if selected_variants
                .as_ref()
                .is_some_and(|set| !set.contains(name))
            {
                continue;
            }
            let repo = spec["repository"].as_str().unwrap();
            let language = spec["language"].as_str().unwrap();
            if !languages.contains("all") && !languages.contains(language) {
                continue;
            }
            if group == "consumers" && name != "livestore-effect" {
                continue;
            }
            let root = spec["composite"]
                .as_str()
                .map(|name| composite.join(name))
                .unwrap_or_else(|| roots.join(repo));
            let expected =
                read_json(&corpus.join(format!("remaining-routes/results/{name}.evaluation.json")));
            jobs.push((
                name.clone(),
                root,
                repo.into(),
                language.into(),
                spec["source_sha256"].clone(),
                spec["module_bindings"].clone(),
                expected,
            ));
        }
    } else {
        panic!("native frontend loader is not implemented yet");
    }
    assert!(!jobs.is_empty(), "empty public/expanded selection");
    let consumers = read_json(&corpus.join("remaining-routes/consumers.json"));
    let expected_consumers =
        read_json(&corpus.join("remaining-routes/results/consumers.evaluation.json"));
    let reused = if group == "consumers" {
        let report_path = std::env::var("CODEMAP_SOURCE_ROUTES_EXPANDED_REPORT")
            .unwrap_or_else(|_| "docs/briefs/evidence/source-routes/expanded-native.json".into());
        let report = read_report(&app.parent().unwrap().parent().unwrap().join(report_path));
        assert_eq!(report["group"], "expanded");
        assert_eq!(
            report["native_implementation_blake3"],
            implementation_fingerprint(),
            "consumer evaluation requires expanded evidence from this compiled implementation"
        );
        assert_eq!(report["max_file_bytes"], 1048576);
        Some(report)
    } else {
        None
    };
    let mut rows = Vec::new();
    let mut outputs = BTreeMap::new();
    let mut input_hashes = BTreeMap::new();
    let mut metrics = Vec::new();
    for (name, root, repo, language, hashes, bindings, expected) in jobs {
        let revision = std::process::Command::new("git")
            .arg("-C")
            .arg(roots.join(&repo))
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read selected checkout revision");
        assert!(revision.status.success());
        assert_eq!(
            String::from_utf8(revision.stdout).unwrap().trim(),
            specs[&repo]["revision"].as_str().unwrap()
        );
        let hashes = hashes.as_object().expect("locked source hashes");
        assert!(!hashes.is_empty());
        let mut sources = BTreeMap::new();
        let mut unavailable = Vec::new();
        for (path, expected_hash) in hashes {
            let file = root.join(path);
            assert!(
                !file.is_symlink()
                    && file
                        .canonicalize()
                        .unwrap()
                        .starts_with(root.canonicalize().unwrap())
            );
            let hash = source_sha256(&file);
            assert_eq!(
                Some(hash.as_str()),
                expected_hash.as_str(),
                "{name}: {path}"
            );
            input_hashes.insert(format!("{name}/{path}"), hash);
            let data = std::fs::read_to_string(file).unwrap();
            if data.len() > limit {
                unavailable.push(path.clone());
                continue;
            }
            sources.insert(path.clone(), data);
        }
        let bindings: BTreeMap<_, _> = bindings
            .as_object()
            .map(|map| {
                map.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        if reused.is_some() {
            println!(
                "VERIFY {name}: {} locked source hashes; reuse expanded analysis",
                hashes.len()
            );
        } else {
            println!(
                "ANALYZE {name}: {} files, {} bytes",
                sources.len(),
                sources.values().map(String::len).sum::<usize>()
            );
        }
        let start = std::time::Instant::now();
        let raw = if let Some(report) = &reused {
            for (path, hash) in hashes {
                assert_eq!(
                    &report["source_sha256"][format!("{name}/{path}")],
                    hash,
                    "expanded source population must match"
                );
            }
            let analysis = report["analyses"][&name].clone();
            assert!(analysis.is_object(), "missing expanded analysis");
            analysis
        } else {
            let analysis = analyze_with_bindings(&sources, &bindings);
            let negatives: Vec<_> = labels
                .iter()
                .filter(|c| {
                    c["repository"] == repo
                        && c["language"] == language
                        && c["kind"] == "negative"
                        && c["mode"] == "endpoint_pair"
                })
                .chain(
                    consumers["cases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|c| c["input_variant"] == name && c["kind"] == "negative"),
                )
                .collect();
            let mut checks = serde_json::Map::new();
            for case in negatives {
                let is_endpoint = |fact: &model::Fact, side: &str| {
                    fact.location.path == case[side][0].as_str().unwrap()
                        && fact.location.line >= case[side][1].as_u64().unwrap() as usize
                        && fact.location.line <= case[side][2].as_u64().unwrap() as usize
                };
                let selected: Vec<_> = analysis
                    .facts
                    .iter()
                    .filter(|f| {
                        f.kind == "store" && is_endpoint(f, "storage")
                            || matches!(
                                f.kind.as_str(),
                                "invoke"
                                    | "member_invoke"
                                    | "argument"
                                    | "return"
                                    | "key_lookup"
                                    | "store"
                            ) && is_endpoint(f, "invocation")
                    })
                    .cloned()
                    .collect();
                let (joined, omitted) =
                    model::connect(&selected, super::super::ENDPOINTS_PER_SNAPSHOT);
                let joined: Vec<_> = joined
                    .iter()
                    .filter(|r| {
                        is_endpoint(&r.storage, "storage")
                            && is_endpoint(&r.invocation, "invocation")
                    })
                    .collect();
                let functions: BTreeSet<_> = selected.iter().map(|f| &f.function).collect();
                let mut required_ranges = vec![case["storage"].clone(), case["invocation"].clone()];
                // Validate the native endpoint facts and the supporting locations
                // they actually used. The manifest's contextual excerpts may also
                // contain unrelated declarations (for example macro headers).
                for fact in &selected {
                    required_ranges.extend(
                        fact.via
                            .iter()
                            .map(|location| json!([location.path, location.line, location.line])),
                    );
                }
                let mut inspected_ranges = required_ranges.clone();
                if let Some(evidence) = case["evidence"].as_array() {
                    inspected_ranges.extend(evidence.iter().cloned());
                }
                let mut syntax_errors = Vec::new();
                let mut relevant_syntax_error = false;
                for path in inspected_ranges
                    .iter()
                    .filter_map(|r| r[0].as_str())
                    .collect::<BTreeSet<_>>()
                {
                    if !sources.contains_key(path) {
                        relevant_syntax_error |=
                            required_ranges.iter().any(|range| range[0] == path);
                        continue;
                    }
                    if analysis
                        .notices
                        .iter()
                        .any(|(kind, value)| kind == "parse_error" && value == path)
                    {
                        let errors = parse_error_ranges(path, &sources[path]);
                        relevant_syntax_error |= errors.iter().any(|(start, end, _)| {
                            required_ranges.iter().any(|range| {
                                range[0] == path
                                    && *start <= range[2].as_u64().unwrap() as usize
                                    && *end >= range[1].as_u64().unwrap() as usize
                            })
                        });
                        syntax_errors.extend(errors.into_iter().map(|(start, end, kind)| json!({"path":path,"start_line":start,"end_line":end,"kind":kind})));
                    }
                }
                let complete = omitted == 0
                    && !relevant_syntax_error
                    && !analysis.notices.iter().any(|(kind, value)| {
                        matches!(
                            kind.as_str(),
                            "analysis_work_cap"
                                | "total_fact_cap"
                                | "projection_fact_cap"
                                | "stored_field_projection_cap"
                        ) || kind == "function_fact_cap" && functions.contains(value)
                            || matches!(
                                kind.as_str(),
                                "syntax_node_cap" | "snapshot_syntax_node_cap"
                            ) && (case["storage"][0] == *value
                                || case["invocation"][0] == *value)
                    });
                checks.insert(case["id"].as_str().unwrap().into(),json!({"no_connection":joined.is_empty(),"complete":complete,"pair_relation_omissions":omitted,"matching_relations":joined,"required_source_ranges":required_ranges,"documented_context_ranges":case["evidence"],"endpoint_facts":selected,"file_syntax_errors":syntax_errors,"syntax_errors_overlap_required_ranges":relevant_syntax_error}));
            }
            let mut raw = serde_json::to_value(analysis).unwrap();
            raw["negative_pair_checks"] = Json::Object(checks);
            raw
        };
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        metrics.push(json!({"name":name,"files":sources.len(),"bytes":sources.values().map(String::len).sum::<usize>(),"elapsed_ms":elapsed_ms,"relations":raw["relations"].as_array().unwrap().len(),"facts":raw["facts"].as_array().unwrap().len(),"notices":raw["notices"],"unavailable":unavailable}));
        let selected: Vec<_> = if group == "consumers" {
            consumers["cases"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["input_variant"] == name)
                .cloned()
                .collect()
        } else {
            labels
                .iter()
                .filter(|c| c["repository"] == repo && c["language"] == language)
                .cloned()
                .collect()
        };
        for case in selected {
            let id = case["id"].as_str().unwrap();
            let expected_rows = if group == "consumers" {
                &expected_consumers["cases"]
            } else {
                &expected["cases"]
            };
            let baseline = expected_rows
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == id)
                .expect("baseline case");
            let matches: Vec<_> = raw["relations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| {
                    endpoint(r, "storage", &case["storage"])
                        && endpoint(r, "invocation", &case["invocation"])
                        && case["relation_kinds"]
                            .as_array()
                            .is_none_or(|kinds| kinds.is_empty() || kinds.contains(&r["kind"]))
                })
                .cloned()
                .collect();
            let input_complete = raw["sources"]
                .get(case["storage"][0].as_str().unwrap())
                .is_some()
                && raw["sources"]
                    .get(case["invocation"][0].as_str().unwrap())
                    .is_some();
            let is_scenario = case["mode"]
                .as_str()
                .is_some_and(|mode| mode != "endpoint_pair");
            let kind = case["kind"].as_str().unwrap();
            let calls = matches.iter().any(|r| {
                matches!(
                    r["kind"].as_str().unwrap(),
                    "storage_to_invocation" | "stored_object_method_candidate"
                )
            });
            let returns = matches.iter().any(|r| r["kind"] == "stored_value_return");
            let consumed = matches.iter().any(|r| {
                matches!(
                    r["kind"].as_str().unwrap(),
                    "stored_object_write" | "stored_object_read" | "stored_key_lookup"
                )
            });
            let status = if !input_complete {
                "input_unavailable"
            } else if is_scenario {
                "source_scenario_not_executed"
            } else if kind == "negative" {
                if matches.is_empty() {
                    "negative_pending_positive_control"
                } else {
                    "incorrect_connection"
                }
            } else if calls {
                "conditional_candidate"
            } else if returns {
                "conditional_data_return"
            } else if consumed {
                "conditional_data_consumption"
            } else if matches.iter().any(|r| r["kind"] == "stored_value_argument") {
                "argument_transfer_only"
            } else {
                "unresolved"
            };
            let mut passed = status == baseline["status"].as_str().unwrap_or_default();
            if kind == "negative" && !is_scenario {
                passed = status == "negative_pending_positive_control"
                    && raw["negative_pair_checks"][id]["no_connection"] == true
                    && raw["negative_pair_checks"][id]["complete"] == true;
            }
            if kind == "positive" {
                if let Some(conditions) = case["required_conditions"].as_array() {
                    passed &= matches.iter().any(|r| {
                        conditions
                            .iter()
                            .all(|c| r["conditions"].as_array().unwrap().contains(c))
                    });
                }
            }
            passed &= raw["relations"].as_array().unwrap().iter().all(|r| {
                r["certainty"] == "conditional_source_relation"
                    && r["concrete_instance_proven"] == false
                    && r["event_classification"] == "not_inferred"
            });
            let mut strengthened = Json::Null;
            if group == "expanded" && id == "scala-state-subscriber" {
                let mut conditions = vec!["source_companion_implicit_selection_required"];
                if name.contains("scala2") {
                    conditions.extend([
                        "source_macro_template_preservation_required",
                        "compiler_macro_typing_unproven",
                    ]);
                }
                if name.contains("jvm") {
                    conditions.extend([
                        "qualified_jdk_varhandle_access",
                        "compare_and_set_success_required",
                    ]);
                }
                let correct = matches.iter().any(|r| {
                    r["storage"]["value"]
                        .as_str()
                        .is_some_and(|v| v.ends_with(":subscriber"))
                        && conditions
                            .iter()
                            .all(|c| r["conditions"].as_array().unwrap().contains(&json!(c)))
                });
                passed &= correct;
                strengthened = json!({"new_subscriber":correct,"required_conditions":conditions});
            }
            rows.push(json!({"input_variant":name,"id":id,"language":language,"repository":repo,"expected_status":baseline["status"],"status":status,"kind":kind,"passed":passed,"matching_relations":matches,"strengthened_checks":strengthened,"required_positive_control":case["positive_control"],"negative_pair_check":raw["negative_pair_checks"][id],"unavailable_inputs":if reused.is_some(){Vec::<String>::new()}else{unavailable.clone()}}));
        }
        outputs.insert(name, raw);
    }
    for index in 0..rows.len() {
        if rows[index]["kind"] == "negative"
            && rows[index]["status"] == "negative_pending_positive_control"
        {
            let controls: Vec<_> = rows
                .iter()
                .filter(|row| {
                    row["input_variant"] == rows[index]["input_variant"]
                        && (rows[index]["required_positive_control"].is_null()
                            || row["id"] == rows[index]["required_positive_control"])
                        && row["kind"] == "positive"
                        && row["passed"] == true
                        && matches!(
                            row["status"].as_str().unwrap(),
                            "conditional_candidate"
                                | "conditional_data_return"
                                | "conditional_data_consumption"
                        )
                })
                .map(|r| r["id"].clone())
                .collect();
            rows[index]["passed"] = json!(rows[index]["passed"] == true && !controls.is_empty());
            rows[index]["status"] = json!(if controls.is_empty() {
                "negative_unverified_without_positive_control"
            } else {
                "correctly_unjoined_with_positive_control"
            });
            rows[index]["positive_controls"] = json!(controls);
        }
    }
    let failures: Vec<_> = rows
        .iter()
        .filter(|r| r["passed"] != true)
        .map(|r| {
            format!(
                "{}:{} ({} -> {})",
                r["input_variant"].as_str().unwrap(),
                r["id"].as_str().unwrap(),
                r["expected_status"].as_str().unwrap(),
                r["status"].as_str().unwrap()
            )
        })
        .collect();
    for row in &rows {
        println!(
            "{} {}:{} {}",
            if row["passed"] == true {
                "PASS"
            } else {
                "FAIL"
            },
            row["input_variant"].as_str().unwrap(),
            row["id"].as_str().unwrap(),
            row["status"].as_str().unwrap()
        );
    }
    let report = json!({"group":group,"case_count":rows.len(),"passed":rows.len()-failures.len(),"cases":rows,"analyses":outputs,"metrics":metrics,"source_sha256":input_hashes,"failures":failures,"target_programs_executed":false,"compiler_auxiliary_input_used":false,"max_file_bytes":limit,"reused_expanded_analysis":reused.is_some(),"analysis_input_max_file_bytes":if reused.is_some(){1048576}else{limit}});
    let path = std::env::var("CODEMAP_SOURCE_ROUTES_REPORT")
        .expect("native public report path is required");
    let root = app.parent().unwrap().parent().unwrap();
    let path = root.join(path);
    write_report(&path, report);
    assert!(
        failures.is_empty(),
        "native public/expanded mismatches: {}",
        failures.join(", ")
    );
}

// An error elsewhere in a file does not erase an intact endpoint-pair check.
// Record every error range and reject errors touching endpoints or native proof steps.
fn parse_error_ranges(path: &str, source: &str) -> Vec<(usize, usize, String)> {
    let path = Path::new(path);
    let spec = crate::lang::spec_for_path(path).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(
            &spec.grammar(
                path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default(),
            ),
        )
        .unwrap();
    let tree = parser.parse(source, None).unwrap();
    let mut pending = vec![tree.root_node()];
    let mut errors = Vec::new();
    while let Some(node) = pending.pop() {
        if node.is_error() || node.is_missing() {
            errors.push((
                node.start_position().row + 1,
                node.end_position().row + 1,
                node.kind().into(),
            ));
        }
        pending.extend(node.children(&mut node.walk()));
    }
    errors
}
