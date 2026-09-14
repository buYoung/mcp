use super::*;
use crate::parser::TreeSitterExtractor;
use std::collections::BTreeMap;
use std::sync::Arc;

fn build(entries: &[(&str, &str)]) -> (FlowIndex, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    let mut flows = BTreeMap::new();
    let mut sources = std::collections::HashMap::new();
    for &(path, source) in entries {
        let destination = root.path().join(path);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(destination, source).unwrap();
        let (file, auxiliary) = TreeSitterExtractor::new()
            .extract_for_index(source, path)
            .unwrap();
        if let Some(flow) = auxiliary.flow_file {
            flows.insert(path.into(), flow);
        }
        files.push(file);
        sources.insert(path.into(), source.into());
    }
    (
        FlowIndex::build(Arc::new(files), flows, Arc::new(sources)),
        root,
    )
}

#[test]
fn test_source_wrappers_returned_handles_and_callbacks_need_no_package_rules() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let source="const routes = new Map();\nexport function connect(key) { return { on(handler) { return add(key, handler); } }; }\nfunction add(key, handler) { routes.set(key, handler); }\nfunction dispatch(key, payload) { const callback = routes.get(key); callback(payload); }\nexport function notify(payload) {}\nexport function setup() { const handle = connect('changed'); handle.on(notify); }\nexport function publish() { dispatch('changed', 1); }\n";
    let (index, root) = build(&[("bus.ts", source)]);
    let output = index.for_paths(&[("bus.ts".into(), 6, 6)], None, 12_000, root.path());
    assert!(
        output.contains("argument 1 → parameter")
            && output.contains("returned value → call result"),
        "{output}"
    );
    assert!(
        output.contains("closure captures value") && output.contains("value → collection store"),
        "{output}"
    );
    assert!(
        output.contains("possible callback invocation") && output.contains("notify — bus.ts:5"),
        "{output}"
    );
    assert!(
        output.contains("[source]") && output.contains("[model]") && output.contains("[candidate]"),
        "{output}"
    );
}

#[test]
fn test_instances_keys_external_calls_and_mutable_captures_do_not_create_links() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let cases = [
        ("different instances", "const a = new Map(); const b = new Map(); a.set('x', notify); const f = b.get('x'); f();"),
        ("different keys", "const a = new Map(); a.set('x', notify); const f = a.get('y'); f();"),
        ("typed keys", "const a = new Map(); a.set('1', notify); const f = a.get(1); f();"),
        ("unknown key", "const a = new Map(); a.set('x', notify); const f = a.get(dynamicKey); f();"),
        ("detached method", "const a = new Map(); a.set('x', notify); const get = a.get; const f = get('x'); f();"),
        ("external", "const handle = external(notify); handle.on(notify);"),
        ("nested closure only passed", "const handle = () => notify(); external(handle);"),
        ("spread overwrite", "const handle = { on: notify, ...external() }; handle.on();"),
        ("accessor", "const handle = { get on() { return notify; } }; handle.on();"),
        ("mutated capture", "let f = notify; const invoke = () => f(); f = external; invoke();"),
        ("mutated named capture", "let f = notify; function invoke() { f(); } f = external; invoke();"),
        ("dynamic field write", "const handle = { on: notify }; handle[dynamicKey] = external; handle.on();"),
        ("compound assignment", "let f = external; f += notify; f();"),
        ("constructor returns another object", "class Handle { constructor() { return { fire: external }; } fire() { notify(); } } const handle = new Handle(); handle.fire();"),
        ("overwritten builtin", "Map = external; const a = new Map(); a.set('x', notify); const f = a.get('x'); f();"),
        ("aliased builtin prototype", "const Native = Map; Native.prototype.set = external; const a = new Map(); a.set('x', notify); const f = a.get('x'); f();"),
        ("instance prototype mutation", "class Handle { fire() { notify(); } } const handle = new Handle(); handle.__proto__ = { fire: external }; handle.fire();"),
        ("class prototype mutation", "class Handle { fire() { notify(); } } Handle.prototype.fire = external; const handle = new Handle(); handle.fire();"),
        ("static method mutation", "class Handle { static fire() { notify(); } } Handle.fire = external; Handle.fire();"),
        ("conditional", "let f = notify; if (flag) { f = external; } f();"),
    ];
    for (name, body) in cases {
        let source = format!("function notify() {{}}\nexport function setup() {{ {body} }}\n");
        let (index, root) = build(&[("negative.ts", &source)]);
        let mut query = super::evaluate::Query::new(&index, root.path(), None);
        query.run(&[("negative.ts".into(), 2, 2)]);
        assert!(
            !query
                .steps
                .iter()
                .any(|step| step.relation == "possible callback invocation"
                    || (step.relation == "source-resolved call"
                        && step.from.range.start_line == 1)),
            "{name}: {}",
            super::render::render(&query, 20_000)
        );
    }
}

#[test]
fn test_language_specific_properties_and_constructors_remain_unresolved() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    for (path, source, line, forbidden) in [
        ("Sample.cs", "class Program {\npublic static void Notify() {}\npublic static void Other() {}\npublic static void Setup() { var h = new Holder(Program.Notify); h.Callback(); }\n}\nclass Holder { public System.Action Callback { get { return Program.Other; } set {} } public Holder(System.Action cb) { this.Callback = cb; } }\n", 4, 2),
        ("Sample.java", "class Program {\nstatic void notifyHandler() {}\nstatic void other() {}\nstatic void setup() { new Handler() { public void fire() { Program.other(); } }.fire(); }\n}\nclass Handler { public void fire() { Program.notifyHandler(); } }\n", 4, 2),
        ("sample.py", "def notify(value): pass\ndef setup():\n    handlers = {'get': notify}\n    handlers.get('missing')\n", 4, 1),
    ] {
        let (index, root) = build(&[(path, source)]);
        let mut query = super::evaluate::Query::new(&index, root.path(), None);
        query.run(&[(path.into(), line, line)]);
        assert!(!query.steps.iter().any(|step| step.relation == "source-resolved call" && step.from.path == path && step.from.range.start_line == forbidden), "{path}: {}", super::render::render(&query, 10_000));
        assert!(!query.diagnostics.is_empty(), "{path}: missing unresolved boundary");
    }
}

#[test]
fn test_output_and_analysis_budgets_are_explicit() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let source =
        "function forward(value) { return value; }\nfunction setup() { return forward('ok'); }\n";
    let (index, root) = build(&[("small.ts", source)]);
    for cap in [256, 512, 1024] {
        let output = index.for_paths(&[("small.ts".into(), 2, 2)], None, cap, root.path());
        assert!(output.len() <= cap, "{cap}: {}", output.len());
        if cap <= 512 {
            assert!(
                output.contains("budget reached") && output.contains("Narrow the source window"),
                "{cap}: {output}"
            );
        }
    }
    let source = format!(
        "function huge() {{ {} }}",
        (0..5000)
            .map(|i| format!("const value{i} = {i};"))
            .collect::<String>()
    );
    let (index, root) = build(&[("large.ts", &source)]);
    let output = index.for_paths(&[("large.ts".into(), 1, 1)], None, 2048, root.path());
    assert!(output.contains("budget exceeded"), "{output}");
    let declarations = (0..3000)
        .map(|i| format!("const v{i} = {i};"))
        .collect::<String>();
    let source = format!(
        "<script lang=\"ts\">\n{declarations}\n</script>\n<script>\n{declarations}\n</script>\n"
    );
    let (index, _) = build(&[("Budget.vue", &source)]);
    let summary = index.file("Budget.vue").unwrap();
    assert!(
        summary
            .units
            .iter()
            .map(|unit| unit.nodes.len())
            .sum::<usize>()
            <= NODES_PER_FILE
    );
    assert!(
        summary
            .units
            .iter()
            .map(|unit| unit.bindings.len())
            .sum::<usize>()
            <= NODES_PER_FILE
    );
    assert!(summary
        .omissions
        .iter()
        .any(|issue| issue.reason.contains("budget exceeded")));
}

#[test]
fn test_composite_scripts_use_original_source_ranges_and_digest() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    for (path, source) in [
        ("Sample.vue", "<script lang=\"ts\">\nfunction forward(value: string) { return value; }\nfunction setup() { return forward('ok'); }\n</script>\n<template><div /></template>\n"),
        ("Sample.svelte", "<script>\nfunction forward(value) { return value; }\nfunction setup() { return forward('ok'); }\n</script>\n<div />\n"),
        ("Sample.astro", "---\nfunction forward(value) { return value; }\nfunction setup() { return forward('ok'); }\n---\n<div />\n"),
    ] {
        let (index, root) = build(&[(path, source)]);
        let output = index.for_paths(&[(path.into(), 3, 3)], None, 4096, root.path());
        assert!(output.contains("argument 1 → parameter") && output.contains(&format!("{path}:2")), "{path}: {output}");
        assert_eq!(index.digest(path), Some(crate::implementations::digest(source.as_bytes()).as_str()));
    }
}

#[test]
fn test_instance_map_initializers_share_only_the_allocated_receiver() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let source = "class Broker {\nroutes = new Map();\nadd(key, handler) { this.routes.set(key, handler); }\nfire(key) { const f = this.routes.get(key); f(); }\n}\nfunction notify() {}\nexport function setup() { const a = new Broker(); const b = new Broker(); a.add('x', notify); b.fire('x'); }\n";
    let (index, root) = build(&[("broker.ts", source)]);
    let output = index.for_paths(&[("broker.ts".into(), 7, 7)], None, 12000, root.path());
    assert!(
        output.contains("value → collection store")
            && !output.contains("possible callback invocation"),
        "{output}"
    );
    let source = source.replace("b.fire('x')", "a.fire('x')");
    let (index, root) = build(&[("broker.ts", &source)]);
    let output = index.for_paths(&[("broker.ts".into(), 7, 7)], None, 12000, root.path());
    assert!(
        output.contains("possible callback invocation") && output.contains("notify — broker.ts:6"),
        "{output}"
    );
}

#[test]
fn test_shell_and_build_conditions_keep_unresolved_boundaries() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    for (path, source) in [
        ("script.sh", "forward() { echo \"$1\"; }\n"),
        ("script.zsh", "forward() { echo \"$1\"; }\n"),
        ("script.ps1", "function Forward($value) { return $value }\n"),
        (
            "special_linux.go",
            "package demo\nfunc forward(value int) int { return value }\n",
        ),
        (
            "special.go",
            "//go:build custom\npackage demo\nfunc forward(value int) int { return value }\n",
        ),
        (
            "conditional.c",
            "#ifdef CUSTOM\nint forward(int value) { return value; }\n#endif\n",
        ),
    ] {
        let (index, root) = build(&[(path, source)]);
        let output = index.for_paths(&[(path.into(), 1, 5)], None, 2048, root.path());
        assert!(
            output.contains("unresolved") && !output.contains("[source]"),
            "{path}: {output}"
        );
    }
}

#[test]
fn test_returned_instance_field_and_relative_reexport_preserve_source_identity() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let (index, root) = build(&[
        ("handle.ts", "class Handle { callback; constructor(handler) { this.callback = handler; } fire() { this.callback(); } }\nexport function connect(handler) { return new Handle(handler); }\n"),
        ("api.ts", "export { connect as create } from './handle';\n"),
        ("main.ts", "import { create } from './api';\nfunction notify() {}\nexport function setup() { const handle = create(notify); handle.fire(); }\n"),
    ]);
    let output = index.for_paths(&[("main.ts".into(), 3, 3)], None, 16_000, root.path());
    assert!(
        output.contains("value → object field")
            && output.contains("object field → value")
            && output.contains("notify — main.ts:2"),
        "{output}"
    );
    assert!(output.contains("connect — handle.ts:2"), "{output}");
    std::fs::write(
        root.path().join("handle.ts"),
        "export function connect(handler) { return external(); }",
    )
    .unwrap();
    let changed = index.for_paths(&[("main.ts".into(), 3, 3)], None, 16_000, root.path());
    assert!(!changed.contains("value → object field"), "{changed}");
    assert!(changed.contains("changed since indexing"), "{changed}");
    let scoped = index.for_paths(
        &[("main.ts".into(), 3, 3)],
        Some("main.ts"),
        16_000,
        root.path(),
    );
    assert!(!scoped.contains("value → object field"), "{scoped}");
}

#[test]
fn test_simple_forwarding_language_profiles() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let cases = [
        ("sample.ts", "function forward(value) { return value; }\nfunction setup() { return forward('ok'); }\n"),
        ("sample.js", "function forward(value) { return value; }\nfunction setup() { return forward('ok'); }\n"),
        ("sample.rs", "fn forward(value: i32) -> i32 { value }\nfn setup() -> i32 { forward(1) }\n"),
        ("sample.go", "package demo\nfunc forward(value int) int { return value }\nfunc setup() int { return forward(1) }\n"),
        ("sample.py", "def forward(value):\n    return value\ndef setup():\n    return forward(1)\n"),
        ("sample.c", "int forward(int value) { return value; }\nint setup(void) { return forward(1); }\n"),
        ("sample.cpp", "int forward(int value) { return value; }\nint setup() { return forward(1); }\n"),
        ("sample.kt", "fun forward(value: Int): Int { return value }\nfun setup(): Int { return forward(1) }\n"),
        ("sample.swift", "func forward(_ value: Int) -> Int { return value }\nfunc setup() -> Int { return forward(1) }\n"),
        ("sample.dart", "int forward(int value) { return value; }\nint setup() { return forward(1); }\n"),
        ("sample.scala", "def forward(value: Int): Int = value\ndef setup(): Int = forward(1)\n"),
        ("sample.groovy", "int forward(int value) { return value; }\nint setup() { return forward(1); }\n"),
        ("sample.php", "<?php\nfunction forward($value) { return $value; }\nfunction setup() { return forward(1); }\n"),
        ("sample.rb", "def forward(value)\n  value\nend\ndef setup()\n  forward(1)\nend\n"),
        ("sample.lua", "function forward(value)\n  return value\nend\nfunction setup()\n  return forward(1)\nend\n"),
        ("Sample.java", "class Sample {\nstatic int forward(int value) { return value; }\nstatic int setup() { return Sample.forward(1); }\n}\n"),
        ("Sample.cs", "class Sample {\nstatic int Forward(int value) { return value; }\nstatic int Setup() { return Sample.Forward(1); }\n}\n"),
    ];
    let mut failures = Vec::new();
    for (path, source) in cases {
        let (index, root) = build(&[(path, source)]);
        let line = source
            .lines()
            .position(|line| line.contains("setup") || line.contains("Setup"))
            .unwrap()
            + 1;
        let output = index.for_paths(&[(path.into(), line, line)], None, 16_000, root.path());
        if !output.contains("returned value → call result")
            || !output.contains("argument 1 → parameter")
        {
            let spec = crate::lang::spec_for_path(std::path::Path::new(path)).unwrap();
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&spec.grammar(path.rsplit('.').next().unwrap()))
                .unwrap();
            let tree = parser.parse(source, None).unwrap();
            failures.push(format!(
                "{path}: {output}\nAST: {}",
                tree.root_node().to_sexp()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
