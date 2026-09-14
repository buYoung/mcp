use super::*;
use crate::parser::{CodeExtractor, TreeSitterExtractor};

fn build(entries: &[(&str, &str)]) -> ImplementationIndex {
    let sources = entries
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect();
    let files = entries
        .iter()
        .map(|(path, source)| TreeSitterExtractor::new().extract(source, path).unwrap())
        .collect();
    ImplementationIndex::build(std::sync::Arc::new(files), &sources)
}

pub(super) const CASES: &[(&str,&str)] = &[
    ("case.ts", "abstract class Base { abstract run(): number; }\nclass Child extends Base { run(): number { return 1; } }\nfunction invoke(value: Base) { return value.run(); }"),
    ("case.js", "class Base { run() { return 0; } }\nclass Child extends Base { run() { return 1; } }"),
    ("case.java", "abstract class Base { abstract int run(); }\nclass Child extends Base { @Override int run() { return 1; } }"),
    ("case.cs", "abstract class Base { public abstract int run(); }\nclass Child : Base { public override int run() { return 1; } }"),
    ("case.kt", "interface Base {\n fun run(): Int\n}\nclass Child : Base {\n override fun run(): Int = 1\n}\n"),
    ("case.scala", "trait Base { def run(): Int }\nclass Child extends Base { override def run(): Int = 1 }"),
    ("case.groovy", "abstract class Base { abstract int run(); }\nclass Child extends Base { int run() { return 1; } }"),
    ("case.swift", "protocol Base { func run() -> Int }\nclass Child: Base { func run() -> Int { return 1 } }"),
    ("case.dart", "abstract class Base { int run(); }\nclass Child extends Base { @override int run() => 1; }"),
    ("case.php", "<?php abstract class Base { abstract public function run(): int; }\nclass Child extends Base { public function run(): int { return 1; } }"),
    ("case.py", "from abc import ABC, abstractmethod\nclass Base(ABC):\n    @abstractmethod\n    def run(self): pass\nclass Child(Base):\n    def run(self): return 1\n"),
    ("case.rb", "class Base\n  def run\n    0\n  end\nend\nclass Child < Base\n  def run\n    1\n  end\nend\n"),
    ("case.ps1", "class Base { [int] run() { return 0 } }\nclass Child : Base { [int] run() { return 1 } }"),
    ("case.cpp", "class Base { public: virtual int run() = 0; };\nclass Child : public Base { public: int run() override { return 1; } };"),
    ("case.rs", "trait Base { fn run(&self) -> i32; }\nstruct Child;\nimpl Base for Child { fn run(&self) -> i32 { 1 } }"),
    ("case.go", "package example\ntype Base interface { Run() int }\ntype Child struct{}\nfunc (c Child) Run() int { return 1 }\n"),
    ("case.vue", "<script lang=\"ts\">abstract class Base { abstract run(): number; }\nclass Child extends Base { run(): number { return 1; } }</script><template><div /></template>"),
    ("case.astro", "---\nabstract class Base { abstract run(): number; }\nclass Child extends Base { run(): number { return 1; } }\n---\n<div />"),
    ("case.svelte", "<script lang=\"ts\">abstract class Base { abstract run(): number; }\nclass Child extends Base { run(): number { return 1; } }</script><div />"),
];

#[test]
fn declaration_facts_cover_development_inheritance_syntax() {
    let mut failures = Vec::new();
    for &(path, source) in CASES {
        let file = TreeSitterExtractor::new().extract(source, path).unwrap();
        let Some(facts) = file
            .navigation
            .as_ref()
            .and_then(|nav| nav.implementations.as_ref())
        else {
            failures.push(format!("{path}: no facts"));
            continue;
        };
        let types = facts
            .units
            .iter()
            .flat_map(|unit| unit.types.iter())
            .collect::<Vec<_>>();
        let base = types.iter().find(|typ| typ.name == "Base");
        let child = types.iter().find(|typ| typ.name == "Child");
        let methods = types
            .iter()
            .flat_map(|typ| typ.methods.iter())
            .chain(facts.units.iter().flat_map(|unit| {
                unit.implementations
                    .iter()
                    .flat_map(|block| block.methods.iter())
            }))
            .count();
        let parents = types
            .iter()
            .any(|typ| typ.bases.iter().any(|base| base.name == "Base"))
            || facts.units.iter().any(|unit| {
                unit.implementations
                    .iter()
                    .any(|block| block.contracts.iter().any(|base| base.name == "Base"))
            })
            || path.ends_with(".go");
        if base.is_none() || child.is_none() || methods < 2 || !parents {
            failures.push(format!("{path}: {facts:#?}\nsymbols: {:#?}", file.symbols));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn non_inheritance_languages_and_quoted_source_do_not_create_facts() {
    for (path, source) in [
        ("x.c", "struct Base { int run; };"),
        ("x.lua", "local text = 'class Child extends Base {}'"),
        ("x.sh", "echo 'class Child extends Base {}'"),
        ("x.zsh", "print 'class Child extends Base {}'"),
        ("x.sql", "select 'class Child extends Base {}';"),
        ("x.asm", "main: ret"),
        ("x.md", "class Child extends Base {}"),
        ("Dockerfile", "FROM base"),
        ("x.yaml", "class: Child extends Base"),
    ] {
        let file = TreeSitterExtractor::new().extract(source, path).unwrap();
        assert!(
            file.navigation
                .as_ref()
                .and_then(|nav| nav.implementations.as_ref())
                .is_none(),
            "{path}"
        );
    }
    let file=TreeSitterExtractor::new().extract("const text = 'abstract class Base { abstract run(): void; }'; // class Child extends Base {}","quoted.ts").unwrap();
    assert!(file
        .navigation
        .unwrap()
        .implementations
        .unwrap()
        .units
        .iter()
        .all(|unit| unit.types.is_empty()));
}

#[test]
fn implementation_links_cover_all_applicable_language_profiles() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let mut failures = Vec::new();
    for &(path, source) in CASES {
        let index = build(&[(path, source)]);
        if index.links.len() != 1 {
            failures.push(format!(
                "{path}: {} links; types={:#?}; methods={:#?}",
                index.links.len(),
                index.types,
                index.methods
            ));
            continue;
        }
        let link = &index.links[0];
        assert_eq!(
            index.typ(index.methods[link.declaration].owner).name,
            "Base",
            "{path}"
        );
        assert_eq!(
            index.typ(index.methods[link.implementation].owner).name,
            "Child",
            "{path}"
        );
        if !matches!(path, "case.js" | "case.rb" | "case.ps1")
            && !index.methods[link.declaration].declaration.is_abstract
        {
            failures.push(format!(
                "{path}: missing abstract flag: {:?}",
                index.methods[link.declaration]
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn typed_calls_and_explicit_imports_preserve_definition_identity() {
    let index=build(&[
        ("src/base.ts","export abstract class Base { abstract run(): number; }"),
        ("src/child.ts","import { Base as Contract } from './base';\nclass Child extends Contract { run(): number { return 1; } }\nfunction invoke(value: Contract) { return value.run(); }"),
        ("src/other.ts","abstract class Base { abstract run(): number; }\nclass Other { run(): number { return 2; } }"),
    ]);
    assert_eq!(index.links.len(), 1, "{:?}", index.links);
    assert_eq!(
        index.methods[index.links[0].declaration].location.path,
        "src/base.ts"
    );
    assert_eq!(index.calls.len(), 1, "{:?}", index.calls);
    assert_eq!(index.calls[0].declaration, index.links[0].declaration);
}

#[test]
fn unrelated_overloads_and_incomplete_go_method_sets_are_not_implementations() {
    for (path,source) in [
        ("case.ts","abstract class Base { abstract run(x: string): void; }\nclass Child extends Base { run(x: number): void {} }"),
        ("case.java","abstract class Base { abstract void run(int value); }\nclass Child { void run(int value) {} }"),
        ("case.cs","class Base { public void run() {} }\nclass Child : Base { public void run() {} }"),
        ("case.cpp","class Base { public: void run() {} };\nclass Child : public Base { public: void run() {} };"),
        ("case.go","package example\ntype Base interface { Run() int; Stop() }\ntype Child struct{}\nfunc (c Child) Run() int { return 1 }\n"),
        ("case.js","const prose = 'class Child extends Base { run() {} }';\nclass Base { run() {} }\nclass Child { run() {} }"),
    ] {
        let index=build(&[(path,source)]);
        assert!(index.links.is_empty(),"{path}: {:?}",index.links);
    }
}

#[test]
fn output_is_routed_by_anchor_and_suppresses_stale_or_excluded_proofs() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let entries=[("base.ts","export abstract class Base { abstract run(): number; }\nconst prose = 'unrelated';\n"),("child.ts","import {Base} from './base';\nclass Child extends Base { run(): number { return 1; } }\nfunction invoke(value: Base) { return value.run(); }\n")];
    let root = tempfile::tempdir().unwrap();
    for (path, source) in entries {
        std::fs::write(root.path().join(path), source).unwrap();
    }
    let index = build(&entries);
    let output = index.for_paths(&[("base.ts".into(), 1, 1)], None, 4096, root.path());
    assert!(output.contains("Child.run — child.ts:2"), "{output}");
    assert!(
        output.contains("declaration reference: invoke — child.ts:3"),
        "{output}"
    );
    assert!(index
        .for_paths(&[("base.ts".into(), 2, 2)], None, 4096, root.path())
        .is_empty());
    let call = index.for_paths(&[("child.ts".into(), 3, 3)], None, 4096, root.path());
    assert!(
        call.contains("call declaration: Base.run — base.ts:1"),
        "{call}"
    );
    assert!(call.contains("runtime target: unresolved"));
    std::fs::write(root.path().join("child.ts"), "const changed = true;\n").unwrap();
    let output = index.for_paths(&[("base.ts".into(), 1, 1)], None, 4096, root.path());
    assert!(
        !output.contains("Child.run") && !output.contains("invoke —"),
        "{output}"
    );
    assert!(index
        .for_paths(&[("child.ts".into(), 3, 3)], None, 4096, root.path())
        .is_empty());
    assert!(index
        .for_paths(
            &[("base.ts".into(), 1, 1)],
            Some("elsewhere"),
            4096,
            root.path()
        )
        .is_empty());
}

#[test]
fn interface_protocol_extension_and_trait_dispatch_are_source_proven() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    for (path,source) in [
        ("x.ts","interface Base { run(): number; }\nclass Child implements Base { run(): number { return 1; } }\nfunction invoke(value: Base) { value.run(); }"),
        ("x.java","interface Base { int run(); }\nclass Child implements Base { public int run() { return 1; } }"),
        ("x.swift","protocol Base { func run() -> Int }\nclass Child {}\nextension Child: Base { func run() -> Int { return 1 } }"),
        ("x.py","from typing import Protocol\nclass Base(Protocol):\n    def run(self) -> int: ...\nclass Child(Base):\n    def run(self) -> int: return 1\n"),
        ("x.rs","trait Base { fn run(&self) -> i32; }\nstruct Child;\nimpl Base for Child { fn run(&self) -> i32 { 1 } }\nfn invoke(value: &dyn Base) { value.run(); }"),
        ("x.go","package example\ntype Base interface { Run() int }\ntype Child struct{}\nfunc (c *Child) Run() int { return 1 }\nfunc Invoke(value Base) { value.Run() }\n"),
    ] {
        let index=build(&[(path,source)]);
        assert_eq!(index.links.len(),1,"{path}: {index:#?}");
        if matches!(path,"x.ts"|"x.rs"|"x.go") {assert_eq!(index.calls.len(),1,"{path}: {index:#?}");}
    }
}

#[test]
fn shadowing_qualifiers_and_dynamic_receivers_do_not_invent_links() {
    let invalid_import = build(&[
        ("base.ts", "abstract class Base { abstract run(): void; }"),
        (
            "child.ts",
            "import {Base} from './base'; class Child extends Base { run(): void {} }",
        ),
    ]);
    assert!(
        invalid_import.links.is_empty(),
        "An unexported class is not an imported type"
    );
    for (path,source) in [
        ("x.ts","abstract class Base { abstract run(): void; }\nfunction scope() { const Base = factory(); class Child extends Base { run() {} } }"),
        ("x.cpp","class Base { public: virtual void run() const = 0; };\nclass Child : public Base { public: void run() {} };"),
        ("x.cs","interface Base { void run(); }\nclass Child : Base { public static void run() {} }"),
    ] {let index=build(&[(path,source)]);assert!(index.links.is_empty(),"{path}: {index:#?}");}
    let index=build(&[("x.ts","abstract class Base { abstract run(): void; }\nclass Child extends Base { run() {} }\nfunction invoke(value: Base) { function inner(value: unknown) { value.run(); } }\nconst value = factory(); value.run();")]);
    assert!(index.calls.is_empty(), "{index:#?}");
}

#[test]
fn rust_module_imports_resolve_without_crossing_other_traits() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let index=build(&[
        ("src/lib.rs","mod base; mod child; mod other;"),
        ("src/base.rs","pub trait Base { fn run(&self); }"),
        ("src/other.rs","pub trait Base { fn run(&self); }"),
        ("src/child.rs","use crate::base::Base as Contract;\nstruct Child;\nimpl Contract for Child { fn run(&self) {} }\nimpl Child { fn run(&self) {} }"),
    ]);
    assert_eq!(index.links.len(), 1, "{index:#?}");
    assert_eq!(
        index.methods[index.links[0].declaration].location.path,
        "src/base.rs"
    );
    assert_eq!(
        index.methods[index.links[0].implementation]
            .location
            .range
            .start_line,
        3
    );
}

#[test]
fn whole_go_method_set_proofs_must_remain_fresh() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let entries = [
        (
            "contract.go",
            "package example\ntype Base interface { Run(); Stop() }\n",
        ),
        (
            "child.go",
            "package example\ntype Child struct{}\nfunc (c Child) Run() {}\n",
        ),
        ("stop.go", "package example\nfunc (c *Child) Stop() {}\n"),
    ];
    let root = tempfile::tempdir().unwrap();
    for (path, source) in entries {
        std::fs::write(root.path().join(path), source).unwrap();
    }
    let index = build(&entries);
    let output = index.for_paths(&[("contract.go".into(), 2, 2)], None, 4096, root.path());
    assert!(
        output.contains("Child.Run — child.go:3 [pointer receiver; *T method set]"),
        "{output}"
    );
    std::fs::remove_file(root.path().join("stop.go")).unwrap();
    let output = index.for_paths(&[("contract.go".into(), 2, 2)], None, 4096, root.path());
    assert!(!output.contains("implementation candidate:"), "{output}");
}

#[test]
fn conditional_generic_and_document_code_do_not_fabricate_implementations() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    for (path,source) in [
        ("x.ts","abstract class Base<T> { abstract run(value: T): void; }\nclass Child extends Base<string> { run(value: number): void {} }"),
        ("x.rs","trait Base { fn run(&self); }\nstruct Child;\n#[cfg(feature=\"optional\")]\nimpl Base for Child { fn run(&self) {} }"),
        ("x.go","//go:build custom\n\npackage example\ntype Base interface { Run() }\ntype Child struct{}\nfunc (c Child) Run() {}"),
        ("x_windows.go","package example\ntype Base interface { Run() }\ntype Child struct{}\nfunc (c Child) Run() {}"),
        ("x.md","```typescript\nabstract class Base { abstract run(): void; }\nclass Child extends Base { run() {} }\n```\n"),
    ] {let index=build(&[(path,source)]);assert!(index.links.is_empty(),"{path}: {index:#?}");}
}

#[test]
fn bounded_output_keeps_first_candidates_and_late_read_anchors() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let root = tempfile::tempdir().unwrap();
    let mut source = "abstract class Base { abstract run(): void; }\n".to_string();
    for i in 0..80 {
        source.push_str(&format!(
            "class Child{i} extends Base {{ run(): void {{}} }}\n"
        ));
    }
    std::fs::write(root.path().join("many.ts"), &source).unwrap();
    let index = build(&[("many.ts", &source)]);
    let output = index.for_paths(&[("many.ts".into(), 1, 1)], None, 600, root.path());
    assert!(
        output.len() <= 600
            && output.contains("implementation candidate: Child0.run")
            && output.contains("truncated"),
        "{output}"
    );
    // Use one type with many unrelated methods so the file type budget is not
    // the reason for dropping the late declaration's relationship.
    let mut source = "class Ordinary {\n".to_string();
    for i in 0..600 {
        source.push_str(&format!(" method{i}() {{}}\n"));
    }
    source.push_str("}\nabstract class Base { abstract run(): void; }\nclass Child extends Base { run(): void {} }\n");
    std::fs::write(root.path().join("late.ts"), &source).unwrap();
    let index = build(&[("late.ts", &source)]);
    let output = index.for_paths(&[("late.ts".into(), 603, 603)], None, 2000, root.path());
    assert!(
        output.contains("implementation candidate: Child.run"),
        "{output}"
    );
}
