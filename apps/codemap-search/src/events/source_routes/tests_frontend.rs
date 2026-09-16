//! Separate front-end obligations; these are not relationship corpus rows.
use super::tests::{read_json, source_sha256};
use super::*;
use engine::{Analyzer, Interpreter};
use model::Value;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn program(
    sources: &BTreeMap<String, String>,
    bindings: &BTreeMap<String, String>,
) -> syntax::Program {
    let sources = sources
        .iter()
        .map(|(p, s)| Arc::new(syntax::Source::parse(p, s).unwrap()))
        .collect();
    syntax::Program::new(
        rust_macros::expand_sources(sources, bindings),
        bindings.clone(),
    )
}
pub(super) fn run() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let corpus = app.join("experiments/event-navigation-poc/language-corpus");
    let fixture = corpus.join("regressions/examples/rust/source-frontier/routes.rs");
    let hash = source_sha256(&fixture);
    let cases = read_json(&corpus.join("regressions/cases.json"));
    let locked = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["path"] == "rust/source-frontier/routes.rs")
        .unwrap();
    assert_eq!(Some(hash.as_str()), locked["source_sha256"].as_str());
    let sources = BTreeMap::from([(
        "routes.rs".into(),
        std::fs::read_to_string(&fixture).unwrap(),
    )]);
    let fixture_program = program(&sources, &BTreeMap::new());
    let mut analyzer = Analyzer::new(&fixture_program);
    analyzer.run();
    let calls = &analyzer.unresolved_calls;
    let direct: Vec<_> = calls.iter().filter(|c| c.via.is_empty()).collect();
    let factories: Vec<_> = direct
        .iter()
        .filter(|c| c.callee.name.ends_with(":factory"))
        .collect();
    let all_operands = factories.len() == 1
        && factories[0].arguments.len() == 10
        && factories[0]
            .arguments
            .iter()
            .take(4)
            .map(|v| v.kind.as_str())
            .collect::<Vec<_>>()
            == ["allocation", "wrapper", "tuple", "parameter"];
    let nested_distinct = factories.len() == 1
        && direct
            .iter()
            .filter(|c| {
                c.callee == factories[0].result.clone().field("id")
                    && c.result != factories[0].result
                    && c.location == factories[0].location
            })
            .count()
            == 1;
    let variables: Vec<_> = direct
        .iter()
        .filter(|c| c.callee.name.ends_with(":opaque"))
        .flat_map(|c| c.generic_types.iter())
        .collect();
    let distinct_scopes = variables.len() == 2
        && variables.iter().all(|v| v.starts_with("T@{"))
        && variables[0] != variables[1];
    let inherited: Vec<_> = calls
        .iter()
        .filter(|c| c.callee.name.ends_with(":missing") && !c.via.is_empty())
        .collect();
    let distinct_contexts = inherited
        .iter()
        .map(|c| &c.result)
        .collect::<BTreeSet<_>>()
        .len()
        == 2
        && inherited
            .iter()
            .map(|c| c.via.last().unwrap().line)
            .collect::<BTreeSet<_>>()
            == BTreeSet::from([58, 59])
        && inherited.iter().all(|c| !c.arguments.is_empty());
    let generic = fixture_program
        .functions
        .iter()
        .position(|f| f.name == "distinct_generic")
        .unwrap();
    let function = &fixture_program.functions[generic];
    let holder = fixture_program.resolve_type(function.source, "Holder", "");
    let mut interpreter = Interpreter::new(&mut analyzer, function.source, Some(generic));
    interpreter
        .rust_type_bindings
        .insert("T".into(), rust_types::RustType::named("nominal", &holder));
    let specialized = interpreter
        .analyzer
        .owner(&interpreter.env["value"], function.source)
        .is_empty();
    let interpreter = Interpreter::new(&mut analyzer, function.source, Some(generic));
    let unbound = interpreter
        .analyzer
        .owner(&interpreter.env["value"], function.source)
        .is_empty();
    let converted = fixture_program
        .functions
        .iter()
        .find(|f| f.name == "convert")
        .unwrap();
    let generic_safe = specialized
        && unbound
        && converted.owner != fixture_program.resolve_type(converted.source, "T", "");
    let mut checks = vec![
        ("all_ten_operands_preserved", all_operands),
        ("nested_calls_same_start_distinct_results", nested_distinct),
        (
            "impl_and_function_type_variables_have_distinct_scopes",
            distinct_scopes,
        ),
        ("source_call_contexts_remain_distinct", distinct_contexts),
        (
            "generic_formal_types_do_not_leak_between_calls",
            generic_safe,
        ),
    ];
    let fixture_calls = serde_json::to_value(&analyzer.unresolved_calls).unwrap();
    let root = PathBuf::from(
        std::env::var("CODEMAP_SOURCE_ROUTES_SOURCE_ROOT")
            .expect("locked checkout root is required"),
    )
    .join("bevy");
    let spec =
        read_json(&corpus.join("remaining-routes/inputs.json"))["variants"]["bevy-modules"].clone();
    let revision = std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(revision.status.success());
    assert_eq!(
        String::from_utf8(revision.stdout).unwrap().trim(),
        spec["revision"].as_str().unwrap()
    );
    let mut sources = BTreeMap::new();
    for (path, hash) in spec["source_sha256"].as_object().unwrap() {
        let file = root.join(path);
        assert_eq!(Some(source_sha256(&file).as_str()), hash.as_str());
        let data = std::fs::read_to_string(file).unwrap();
        assert!(data.len() <= super::super::SOURCE_BYTES_PER_FILE);
        sources.insert(path.clone(), data);
    }
    let bindings = spec["module_bindings"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
        .collect();
    let bevy_program = program(&sources, &bindings);
    let mut analyzer = Analyzer::new(&bevy_program);
    let started = std::time::Instant::now();
    analyzer.run();
    let observed: Vec<_> = analyzer
        .observed_calls
        .values()
        .filter(|(l, _, _)| {
            l.path.ends_with("/system/system_registry.rs")
                && [476, 477, 682, 684, 689, 704, 725].contains(&l.line)
                || l.path.ends_with("/observer/distributed_storage.rs") && l.line == 227
        })
        .collect();
    let option = observed.iter().any(|(_, expression, value)| {
        expression == "Some(system)"
            && value.kind == "wrapper"
            && value.name == "rust:option"
            && value.base().kind == "allocation"
    });
    let entity = observed.iter().any(|(_, expression, value)| {
        expression == "self.spawn(RegisteredSystem::new(system)).id()"
            && value.split_path().0.kind == "allocation"
            && value.kind == "slot"
            && value.key() == &Value::new("key", "entity")
    });
    let constructor = bevy_program
        .functions
        .iter()
        .position(|f| {
            f.name == "new"
                && bevy_program.sources[f.source]
                    .path
                    .ends_with("/observer/distributed_storage.rs")
                && bevy_program.sources[f.source].nodes[f.node].line == 224
        })
        .unwrap();
    let conversion = bevy_program
        .functions
        .iter()
        .position(|f| {
            f.name == "into_observer"
                && bevy_program.sources[f.source]
                    .path
                    .ends_with("/observer/distributed_storage.rs")
                && bevy_program.sources[f.source].nodes[f.node].line == 570
        })
        .unwrap();
    let f = &bevy_program.functions[conversion];
    let interpreter = Interpreter::new(&mut analyzer, f.source, Some(conversion));
    let source = &bevy_program.sources[f.source];
    let call = source
        .walk(f.body.unwrap(), false)
        .into_iter()
        .find(|n| {
            source.nodes[*n].kind == "call_expression"
                && source.text(Some(*n)) == "Observer::new(self)"
        })
        .unwrap();
    let bindings = interpreter.rust_call_bindings(
        constructor,
        source.child(call, &["function"]),
        &[f.receiver()],
    );
    let f = &bevy_program.functions[constructor];
    let mut bound = Interpreter::new(&mut analyzer, f.source, Some(constructor));
    bound.rust_type_bindings = bindings;
    let source = &bevy_program.sources[f.source];
    let params = source.child(f.node, &["parameters"]).unwrap();
    let parameter = source.nodes[params]
        .children
        .iter()
        .find(|n| source.text(source.child(**n, &["pattern"])) == "system")
        .unwrap();
    let nonconcrete = !bound
        .rust_type_expression(source.child(*parameter, &["type"]))
        .is_concrete();
    checks.extend([
        ("option_preserves_system_box_allocation", option),
        ("spawn_id_preserves_allocation_entity_field", entity),
        ("observer_generic_variable_remains_nonconcrete", nonconcrete),
    ]);
    let observed:Vec<_>=analyzer.observed_calls.values().filter(|(l,_,_)|l.path.ends_with("/system/system_registry.rs")&&[476,477,682,684,689,704,725].contains(&l.line)||l.path.ends_with("/observer/distributed_storage.rs")&&l.line==227).map(|(l,e,v)|json!({"location":l,"expression":e,"result":v,"root_kind":v.split_path().0.kind})).collect();
    let passed = checks.iter().all(|(_, v)| *v);
    for (name, ok) in &checks {
        println!("{} {name}", if *ok { "PASS" } else { "FAIL" });
    }
    let report = json!({"group":"frontend","checks":checks.iter().map(|(id,passed)|json!({"id":id,"passed":passed})).collect::<Vec<_>>(),"passed":passed,"fixture_sha256":hash,"fixture_calls":fixture_calls,"source_sha256":spec["source_sha256"],"revision":spec["revision"],"observed_calls":observed,"bevy_notices":analyzer.notices,"bevy_elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"target_programs_executed":false,"compiler_auxiliary_input_used":false,"counted_among_301":false});
    let output = app
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(std::env::var("CODEMAP_SOURCE_ROUTES_REPORT").unwrap());
    super::tests_reporting::write_report(&output, report);
    assert!(passed, "native frontend obligations failed");
}
