use super::*;
use crate::index::{SearchEngine, TantivySearchEngine};
use crate::parser::TreeSitterExtractor;

fn enabled() -> crate::config::ResolvedConfig {
    let mut config = crate::config::ResolvedConfig::default();
    config.event_navigation.is_enabled = true;
    config
}

fn build(entries: &[(&str, &str)]) -> EventIndex {
    let mut inputs = EventInputs::default();
    let mut files = Vec::new();
    for &(path, source) in entries {
        let (file, aux) = TreeSitterExtractor::new()
            .extract_for_index(source, path)
            .unwrap();
        inputs.insert(path.into(), aux.event_input);
        files.push(std::sync::Arc::new(file));
    }
    EventIndex::build(&files, inputs)
}

#[test]
fn test_shared_esm_bus_constants_reexports_and_separate_allocations() {
    let _config = crate::config::pin_test_config(enabled());
    let index=build(&[
        ("src/bus.ts","import { EventEmitter } from 'node:events';\nexport const shared = new EventEmitter();\nexport const other = new EventEmitter();\nexport const KEY = 'saved';\nexport function handleSaved() {}\nshared.on(KEY, handleSaved);\n"),
        ("src/barrel.ts","export { shared as appBus, KEY, handleSaved } from './bus';\n"),
        ("src/users.ts","import {appBus, KEY} from './barrel';\nexport function save() { appBus.emit(KEY); }\n"),
        ("src/other.ts","import {other} from './bus';\nother.emit('saved');\n"),
    ]);
    assert_eq!(index.endpoints.len(), 3, "{:#?}", index.endpoints);
    let registration = index
        .endpoints
        .iter()
        .find(|e| e.role == EventRole::Subscribe)
        .unwrap();
    let publisher = index
        .endpoints
        .iter()
        .find(|e| e.location.file_path == "src/users.ts")
        .unwrap();
    let other = index
        .endpoints
        .iter()
        .find(|e| e.location.file_path == "src/other.ts")
        .unwrap();
    assert!(registration.key().is_some(), "{registration:#?}");
    assert_eq!(registration.key(), publisher.key());
    assert_ne!(registration.key(), other.key());
    assert_eq!(registration.handler.as_ref().unwrap().range.start_line, 5);
    assert_eq!(registration.location.range.start_line, 6);
    assert!(publisher
        .dependencies
        .iter()
        .any(|l| l.file_path == "src/barrel.ts"));
    assert_eq!(index.by_route.len(), 2);
}

#[test]
fn test_negative_controls_do_not_guess_bus_key_or_handler() {
    let _config = crate::config::pin_test_config(enabled());
    let index = build(&[(
        "src/controls.ts",
        r#"
import {EventEmitter} from 'node:events';
const shared = new EventEmitter();
shared.on('saved', unknownHandler);
shared.emit(dynamicKey());
function opaque(shared: EventEmitter) { shared.emit('saved'); }
function local() { const bus = new EventEmitter(); bus.emit('saved'); }
function shadow({shared}: {shared: unknown}) { shared.emit('wrong-shadow'); }
function factory() { const bus: EventEmitter = makeBus(); bus.emit('saved'); }
const unrelated = { emit(key: string) {} }; unrelated.emit('wrong-method');
const prose = "shared.emit('wrong-string')";
// shared.emit('wrong-comment');
if (false) { shared.on('saved', () => {}); }
if (isReady()) { shared.once('saved', () => {}); }
shared.off('saved', unknownHandler);
"#,
    )]);
    assert_eq!(index.endpoints.len(), 8, "{:#?}", index.endpoints);
    assert!(!index.endpoints.iter().any(|e| e
        .event_key
        .as_deref()
        .is_some_and(|k| k.starts_with("wrong"))));
    assert_eq!(
        index.endpoints.iter().filter(|e| e.key().is_some()).count(),
        3
    );
    assert!(index
        .endpoints
        .iter()
        .any(|e| e.is_inactive && e.key().is_none()));
    assert!(index
        .endpoints
        .iter()
        .any(|e| e.is_once && !e.conditions.is_empty()));
    assert!(index
        .endpoints
        .iter()
        .any(|e| e.handler_reason.is_some() && e.handler.is_none()));
    assert!(index
        .endpoints
        .iter()
        .any(|e| e.event_key.is_none() && e.bus.is_some()));
}

#[test]
fn test_js_destructured_parameters_mutations_and_conflicting_exports() {
    let _config = crate::config::pin_test_config(enabled());
    let index=build(&[
        ("src/bus.js","import {EventEmitter} from 'events'; export const shared = new EventEmitter(); shared.emit('positive');"),
        ("src/shadow.js","import {shared} from './bus.js'; function f({shared}) { shared.emit('wrong-object'); } function g([shared]) {shared.emit('wrong-array');} function h(shared = makeBus()) {shared.emit('wrong-default');}"),
        ("src/conflict.js","export { shared as selected } from './bus.js'; export { shared as selected } from './other.js';"),
        ("src/other.js","import {EventEmitter} from 'events'; export const shared = new EventEmitter();"),
        ("src/use.js","import {selected} from './conflict.js'; selected.emit('wrong-export');"),
        ("src/mutated.js","import {EventEmitter} from 'events'; const bus = new EventEmitter(); bus.emit = replacement; bus.emit('mutated');"),
    ]);
    assert_eq!(index.endpoints.len(), 2, "{:#?}", index.endpoints);
    assert_eq!(
        index.endpoints.iter().filter(|e| e.key().is_some()).count(),
        1
    );
    assert!(index
        .endpoints
        .iter()
        .any(|e| e.event_key.as_deref() == Some("mutated") && e.key().is_none()));
}

#[test]
fn test_custom_api_rules_string_enums_and_qualified_targets() {
    let mut config = enabled();
    config.event_navigation.rules=serde_json::from_value(serde_json::json!([
        {"id":"known-on","language":"typescript","module":"src/known.ts","symbol":"KnownBus","method":"on","role":"subscribe","event_arg":0,"handler_arg":1,"bus":"receiver"},
        {"id":"known-emit","language":"typescript","module":"src/known.ts","symbol":"KnownBus","method":"emit","role":"publish","event_arg":0,"bus":"receiver"}
    ])).unwrap();
    let _config = crate::config::pin_test_config(config);
    let index=build(&[
        ("src/known.ts","export class KnownBus { on(key: string, handler: () => void) {} emit(key: string) {} }"),
        ("src/events.ts","import {KnownBus} from './known'; export const appBus = new KnownBus(); enum Keys { Saved = 'saved' } function handleSaved() {} appBus.on(Keys.Saved,handleSaved);"),
        ("src/users.ts","import {appBus} from './events'; appBus.emit('saved');"),
        ("package.json","{\"name\":\"event-test\"}"),
        ("src/tauri.ts","import {listen, emit, emitTo} from '@tauri-apps/api/event'; listen('saved',()=>{}); emit('saved'); emitTo('child','saved'); listen('saved',()=>{},{target:'child'}); listen('saved',()=>{},{target:dynamic});"),
    ]);
    assert_eq!(index.endpoints.len(), 7, "{:#?}", index.endpoints);
    let custom: Vec<_> = index
        .endpoints
        .iter()
        .filter(|e| e.is_api_configured)
        .collect();
    assert_eq!(custom.len(), 2);
    assert!(custom[0].key().is_some());
    assert_eq!(custom[0].key(), custom[1].key());
    let tauri: Vec<_> = index
        .endpoints
        .iter()
        .filter(|e| !e.is_api_configured)
        .collect();
    assert_eq!(tauri[0].key(), tauri[1].key());
    assert_eq!(tauri[2].key(), tauri[3].key());
    assert_ne!(tauri[0].key(), tauri[2].key());
    assert!(tauri[4].key().is_none());
}

#[test]
fn test_event_inputs_persist_refresh_delete_and_rebuild_old_format() {
    let _config = crate::config::pin_test_config(enabled());
    let root = std::env::current_dir().unwrap();
    let temp = tempfile::Builder::new()
        .prefix("event-lifecycle-")
        .tempdir_in(&root)
        .unwrap();
    let key_path = temp.path().join("keys.ts");
    let source_path = temp.path().join("source.ts");
    let source="import {EventEmitter} from 'node:events'; import {KEY} from './keys'; const bus = new EventEmitter(); bus.on(KEY,()=>{}); bus.emit(KEY);";
    std::fs::write(&key_path, "export const KEY='before';").unwrap();
    std::fs::write(&source_path, source).unwrap();
    let index_path = temp.path().join("index");
    let mut engine = TantivySearchEngine::new(index_path.to_str().unwrap()).unwrap();
    let paths = [key_path.to_str().unwrap(), source_path.to_str().unwrap()];
    engine.index_files(&paths).unwrap();
    let initial = engine.load_published_snapshot().unwrap();
    assert_eq!(initial.events().endpoints.len(), 2);
    assert_eq!(initial.events().by_key["before"].len(), 2);
    std::fs::write(&key_path, "export const KEY='after';").unwrap();
    let stale = initial.events().for_key("before", None, 8000, &root);
    assert!(!stale.contains("- publisher:"), "{stale}");
    assert!(stale.contains("stale/unverified"), "{stale}");
    engine
        .refresh_paths(std::slice::from_ref(&key_path))
        .unwrap();
    let refreshed = engine.load_published_snapshot().unwrap();
    assert!(!refreshed.events().by_key.contains_key("before"));
    assert_eq!(refreshed.events().by_key["after"].len(), 2);
    drop(engine);
    let mut engine = TantivySearchEngine::new(index_path.to_str().unwrap()).unwrap();
    assert_eq!(
        engine.load_published_snapshot().unwrap().events().by_key["after"].len(),
        2
    );
    std::fs::remove_file(&source_path).unwrap();
    engine.refresh_paths(&[source_path]).unwrap();
    assert!(engine
        .load_published_snapshot()
        .unwrap()
        .events()
        .endpoints
        .is_empty());
    std::fs::write(temp.path().join("source.ts"), source).unwrap();
    engine
        .refresh_paths(&[temp.path().join("source.ts")])
        .unwrap();
    assert_eq!(
        engine
            .load_published_snapshot()
            .unwrap()
            .events()
            .endpoints
            .len(),
        2
    );
    drop(engine);
    std::fs::write(
        index_path.join("codemap.format"),
        "v27-native-declarations-and-groovy",
    )
    .unwrap();
    let engine = TantivySearchEngine::new(index_path.to_str().unwrap()).unwrap();
    assert!(engine
        .load_published_snapshot()
        .unwrap()
        .events()
        .endpoints
        .is_empty());
}

#[test]
fn test_rust_wrapper_uses_stored_import_constant_and_explicit_target() {
    let mut config = enabled();
    config.analysis_target_os = Some("macos".into());
    config.event_navigation.rules=serde_json::from_value(serde_json::json!([
        {"id":"dispatch","language":"rust","module":"src/output.rs","symbol":"dispatch_event_json","role":"publish","event_arg":0,"bus":"fixed","bus_identity":"application"}
    ])).unwrap();
    let _config = crate::config::pin_test_config(config);
    let index=build(&[
        ("Cargo.toml","[package]\nname='event-test'\nversion='0.1.0'\nedition='2021'\n"),
        ("src/lib.rs","mod output; mod publisher;"),
        ("src/output.rs","pub const EVENT: &str = \"saved\"; pub fn dispatch_event_json(key: &str) {}"),
        ("src/publisher.rs","use crate::output::{dispatch_event_json as send, EVENT}; use tauri::{AppHandle, Emitter};\n#[cfg(target_os=\"macos\")]\npub fn publish(app: &AppHandle) { send(EVENT); app.emit(EVENT, ()); }\n"),
    ]);
    assert_eq!(index.endpoints.len(), 2, "{:#?}", index.endpoints);
    let wrapper = index
        .endpoints
        .iter()
        .find(|e| e.is_api_configured)
        .unwrap();
    assert!(wrapper.key().is_some(), "{wrapper:#?}");
    assert_eq!(wrapper.event_key.as_deref(), Some("saved"));
    assert!(wrapper.bus.as_ref().unwrap().is_configured_assumption);
    assert!(wrapper
        .dependencies
        .iter()
        .any(|l| l.file_path == "src/lib.rs"));
    let native = index
        .endpoints
        .iter()
        .find(|e| !e.is_api_configured)
        .unwrap();
    assert!(native.key().is_none());
    assert!(native
        .unresolved_reasons
        .iter()
        .any(|r| r.contains("opaque")));
}

#[test]
fn test_event_endpoint_and_input_caps_report_omissions() {
    let _config = crate::config::pin_test_config(enabled());
    let source = format!(
        "import {{EventEmitter}} from 'events'; const bus=new EventEmitter();\n{}",
        "bus.emit('saved');\n".repeat(300)
    );
    let index = build(&[("src/large.ts", &source)]);
    assert_eq!(index.endpoints.len(), super::super::ENDPOINTS_PER_FILE);
    assert_eq!(index.omitted_endpoints, 44);
    let oversized = " ".repeat(super::super::SOURCE_BYTES_PER_FILE + 1);
    let input = EventInput::capture(&oversized, "src/oversized.ts").unwrap();
    assert!(input.source.is_none());
    assert!(input.unavailable_reason.is_some());
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/oversized.ts"), &oversized).unwrap();
    let mut inputs = EventInputs::default();
    inputs.insert("src/oversized.ts".into(), Some(input));
    let index = EventIndex::build(&[], inputs);
    let output = index.for_paths(
        &[("src/oversized.ts".into(), 1, 4)],
        None,
        1024,
        root.path(),
    );
    assert!(
        output.contains("src/oversized.ts:1")
            && output.contains("exceeds")
            && !output.contains("Next: read"),
        "{output}"
    );
    assert!(
        !output.contains("Event relationships"),
        "unavailable input must not look like an empty event result"
    );
    assert!(index
        .for_paths(
            &[("src/unrelated.ts".into(), 1, 4)],
            None,
            1024,
            root.path()
        )
        .is_empty());
}

#[test]
fn test_exact_event_key_zero_append_failure_at_512_bytes() {
    let _config = crate::config::pin_test_config(enabled());
    // The workspace exclusion walker resolves visibility relative to the process cwd.
    let root = tempfile::Builder::new()
        .prefix("event-zero-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let source = "import {EventEmitter} from 'node:events'; const bus = new EventEmitter(); bus.emit('missing');";
    std::fs::create_dir(root.path().join("node_modules")).unwrap();
    std::fs::write(root.path().join("node_modules/events.ts"), source).unwrap();
    for (index, coverage) in [
        (
            build(&[(
                "src/unconfigured.ts",
                "const bus = { emit(key: string) {} }; bus.emit('missing');",
            )]),
            None,
        ),
        (
            build(&[("src/stale.ts", source)]),
            Some("1 stale/unverified endpoints hidden"),
        ),
        (
            build(&[("node_modules/events.ts", source)]),
            // ProofBudget marks walker-hidden sources non-current before event filtering.
            Some("1 stale/unverified endpoints hidden"),
        ),
    ] {
        let full = index.for_key("missing", None, 8000, root.path());
        assert!(
            full.contains("this is not proof that a route is absent"),
            "{full}"
        );
        assert!(
            full.contains("This event-key lookup did not run ranked search for query."),
            "{full}"
        );
        assert!(full.contains("matched configured API rules"), "{full}");
        assert!(
            full.contains("snapshot-wide, not counts for this key or scope"),
            "{full}"
        );
        assert!(
            full.contains("Next: grep a known source identifier, or omit event_key"),
            "{full}"
        );
        if let Some(coverage) = coverage {
            assert!(full.contains(coverage), "{full}");
        }
        let output = index.for_key("missing", None, 512, root.path());
        assert_eq!(
            output, "[Event output budget unavailable; narrow the source request.]\n",
            "a rejected zero-hit append must not return a header-only success"
        );
        assert!(output.len() <= 512);
    }
}

#[test]
fn test_exact_event_key_zero_guidance_and_counters_exceed_cap() {
    let _config = crate::config::pin_test_config(enabled());
    let mut inputs = EventInputs::default();
    inputs.insert("src/unavailable.ts".into(), None);
    let index = EventIndex::build(&[], inputs);
    let root = tempfile::tempdir().unwrap();
    let full = index.for_key("missing", None, 8000, root.path());
    for statement in [
        "No eligible event endpoints for this exact key in the inspected map; this is not proof that a route is absent.",
        "This event-key lookup did not run ranked search for query.",
        "Event extraction covers Rust/JavaScript/TypeScript and matched configured API rules, not arbitrary methods or transports.",
        "Input/extraction coverage counts below are snapshot-wide, not counts for this key or scope.",
        "Next: grep a known source identifier, or omit event_key to run ranked search.",
    ] {
        assert_eq!(full.matches(statement).count(), 1, "{full}");
    }

    // Pin the advertised languages to the private extraction gate without exposing it.
    let gate = include_str!("extract.rs")
        .split("fn language(")
        .nth(1)
        .unwrap()
        .split("\nfn arguments(")
        .next()
        .unwrap();
    let mut extracted_languages: Vec<_> = gate
        .split("Some(\"")
        .skip(1)
        .map(|arm| arm.split('"').next().unwrap())
        .collect();
    extracted_languages.sort_unstable();
    let mut advertised_languages: Vec<_> = full
        .split("Event extraction covers ")
        .nth(1)
        .unwrap()
        .split(" and ")
        .next()
        .unwrap()
        .split('/')
        .map(str::to_ascii_lowercase)
        .collect();
    advertised_languages.sort_unstable();
    assert_eq!(advertised_languages, extracted_languages);

    let coverage_start = full.find("\n[Event coverage:").unwrap();
    assert!(
        full.contains("1 unavailable/capped source inputs"),
        "{full}"
    );
    let cap = full.len() - 1;
    assert!(cap >= 512);
    assert!(coverage_start <= cap, "guidance alone must fit: {full}");
    assert!(
        full.len() > cap,
        "guidance plus counters must exceed the cap"
    );
    let output = index.for_key("missing", None, cap, root.path());
    assert_eq!(
        output, "[Event output budget unavailable; narrow the source request.]\n",
        "guidance and coverage must not be silently dropped: {output}"
    );
    assert!(output.len() <= cap);
}

#[test]
fn test_imported_target_proof_and_var_hoisting_do_not_reuse_outer_bus() {
    let _config = crate::config::pin_test_config(enabled());
    let index=build(&[
        ("package.json","{\"name\":\"targets\"}"),
        ("src/target.ts","export const TARGET='child';"),
        ("src/tauri.ts","import {emitTo,listen} from '@tauri-apps/api/event'; import {TARGET} from './target'; emitTo(TARGET,'saved'); listen('saved',()=>{},{target:TARGET});"),
        ("src/vars.js","import {EventEmitter} from 'events'; const bus=new EventEmitter(); bus.emit('positive'); function f(){if (ready){var bus=makeBus();} bus.emit('wrong-hoist');}"),
    ]);
    assert_eq!(index.endpoints.len(), 3, "{:#?}", index.endpoints);
    assert!(!index
        .endpoints
        .iter()
        .any(|e| e.event_key.as_deref() == Some("wrong-hoist")));
    for endpoint in index
        .endpoints
        .iter()
        .filter(|e| e.location.file_path == "src/tauri.ts")
    {
        assert_eq!(endpoint.target.as_deref(), Some("label:child"));
        assert!(
            endpoint
                .dependencies
                .iter()
                .any(|l| l.file_path == "src/target.ts"),
            "{endpoint:#?}"
        );
    }
}
