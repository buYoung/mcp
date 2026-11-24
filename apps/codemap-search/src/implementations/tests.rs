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
        .map(std::sync::Arc::new)
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

#[test]
fn workspace_scope_keeps_links_with_external_signature_evidence() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let entries = [
        ("types/item.ts", "export interface Item { id: string; }\n"),
        (
            "pkg/base.ts",
            "import { Item } from '../types/item';\nexport interface Base { handle(item: Item): void; }\n",
        ),
        (
            "pkg/child.ts",
            "import { Item } from '../types/item';\nimport { Base } from './base';\nexport class Child implements Base { handle(item: Item): void {} }\n",
        ),
    ];
    let root = tempfile::tempdir().unwrap();
    for (path, source) in entries {
        let full = root.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, source).unwrap();
    }
    let index = build(&entries);
    let scoped = index.for_paths(&[("pkg/base.ts".into(), 2, 2)], Some("pkg"), 4096, root.path());
    assert!(scoped.contains("Child.handle — pkg/child.ts:3"), "{scoped}");

    std::fs::write(
        root.path().join("types/item.ts"),
        "export interface Item { changed: number; }\n",
    )
    .unwrap();
    let stale = index.for_paths(&[("pkg/base.ts".into(), 2, 2)], Some("pkg"), 4096, root.path());
    assert!(!stale.contains("Child.handle"), "{stale}");
}

#[test]
fn signature_types_resolve_csharp_generics_and_ignore_non_type_shadowing() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let csharp = build(&[
        ("store.cs", "namespace Demo { class Store<T> {} }\n"),
        (
            "base.cs",
            "namespace Demo { interface Base { void Run(Store<int> store); } }\n",
        ),
        (
            "child.cs",
            "namespace Demo { class Child : Base { public void Run(Store<int> store) {} } }\n",
        ),
    ]);
    assert_eq!(csharp.links.len(), 1, "{csharp:#?}");

    let method_same_name = build(&[
        ("base.ts", "export interface Base { fail(err: Error): void; }\n"),
        (
            "child.ts",
            "import { Base } from './base';\nexport class Child implements Base { Error() {} fail(err: Error): void {} }\n",
        ),
    ]);
    assert_eq!(method_same_name.links.len(), 1, "{method_same_name:#?}");

    let type_param_shadow = build(&[
        ("base.ts", "export interface Base { fail(err: Error): void; }\n"),
        (
            "child.ts",
            "import { Base } from './base';\nexport class Child<Error> implements Base { fail(err: Error): void {} }\n",
        ),
    ]);
    assert!(type_param_shadow.links.is_empty(), "{type_param_shadow:#?}");
}

fn build_with_disk(entries: &[(&str, &str)]) -> ImplementationIndex {
    let root = tempfile::tempdir().unwrap();
    for (path, source) in entries {
        let full = root.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, source).unwrap();
    }
    let extractor = TreeSitterExtractor::new();
    let files = std::sync::Arc::new(
        entries
            .iter()
            .filter_map(|(path, source)| {
                extractor
                    .extract(source, path)
                    .ok()
                    .map(std::sync::Arc::new)
            })
            .collect::<Vec<_>>(),
    );
    let sources = entries
        .iter()
        .map(|(path, source)| ((*path).to_string(), (*source).to_string()))
        .collect();
    ImplementationIndex::build_at(files, &sources, root.path())
}

#[test]
fn go_cross_package_assertions_and_embedded_interfaces() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let index = build_with_disk(&[
        ("go.mod", "module example.com/app\n\ngo 1.22\n"),
        (
            "pkg/base/base.go",
            concat!(
                "package base\n",
                "type Item struct{}\n",
                "type Closer interface { Close() error }\n",
                "type Service interface {\n",
                "  Closer\n",
                "  Run(item *Item, payload interface{}) error\n",
                "}\n",
                "type External interface {\n",
                "  UnknownExt\n",
                "  Ping() error\n",
                "}\n",
            ),
        ),
        (
            "pkg/impl/impl.go",
            concat!(
                "package impl\n",
                "import b \"example.com/app/pkg/base\"\n",
                "type PtrSvc struct{}\n",
                "var _ b.Service = (*PtrSvc)(nil)\n",
                "func (s *PtrSvc) Close() error { return nil }\n",
                "func (s *PtrSvc) Run(item *b.Item, payload any) error { return nil }\n",
                "type ValSvc struct{}\n",
                "var _ b.Service = ValSvc{}\n",
                "func (s *ValSvc) Run(item *b.Item, payload any) error { return nil }\n",
                "func (s ValSvc) Close() error { return nil }\n",
            ),
        ),
        (
            "pkg/base/local.go",
            concat!(
                "package base\n",
                "type Local struct{}\n",
                "func (l Local) Close() error { return nil }\n",
                "func (l Local) Run(item *Item, payload any) error { return nil }\n",
                "func (l Local) Ping() error { return nil }\n",
            ),
        ),
    ]);
    let pairs: Vec<(String, String)> = index
        .links
        .iter()
        .map(|link| {
            let decl = &index.methods[link.declaration];
            let imp = &index.methods[link.implementation];
            (
                format!("{}.{}", index.typ(decl.owner).name, decl.declaration.name),
                format!("{}.{}", index.typ(imp.owner).name, imp.declaration.name),
            )
        })
        .collect();
    // Pointer assertion links both `Service.Run` (with `any` == `interface{}` and `*b.Item` == `*Item`)
    // and embedded `Closer.Close`.
    assert!(pairs.contains(&("Service.Run".into(), "PtrSvc.Run".into())), "{pairs:#?}");
    assert!(pairs.contains(&("Closer.Close".into(), "PtrSvc.Close".into())), "{pairs:#?}");
    // Value assertion links value-receiver `Close`, never pointer-receiver `Run`.
    assert!(pairs.contains(&("Closer.Close".into(), "ValSvc.Close".into())), "{pairs:#?}");
    assert!(!pairs.contains(&("Service.Run".into(), "ValSvc.Run".into())), "{pairs:#?}");
    // Same-package complete embedded interface links `Local`, while incomplete `External` never guesses.
    assert!(pairs.contains(&("Service.Run".into(), "Local.Run".into())), "{pairs:#?}");
    assert!(!pairs.iter().any(|(d, _)| d == "External.Ping"), "{pairs:#?}");
}

#[test]
fn generic_base_substitution_and_semantic_signature_equivalence() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let java = build(&[
        ("src/a/Item.java", "package a;\npublic class Item {}\n"),
        (
            "src/a/Base.java",
            "package a;\npublic abstract class Base<T> { public abstract void write(T value); }\n",
        ),
        (
            "src/b/Mid.java",
            "package b;\nimport a.Base;\npublic abstract class Mid<U> extends Base<U> {}\n",
        ),
        (
            "src/c/Child.java",
            "package c;\nimport a.Item;\nimport b.Mid;\npublic class Child extends Mid<Item> { public void write(Item value) {} }\n",
        ),
        (
            "src/c/Wrong.java",
            "package c;\nimport b.Mid;\npublic class Wrong<A, B> extends Mid<A> { public void write(B value) {} }\n",
        ),
    ]);
    assert_eq!(java.links.len(), 1, "{java:#?}");
    assert_eq!(java.typ(java.methods[java.links[0].implementation].owner).name, "Child");

    let csharp = build(&[
        (
            "base.cs",
            "using System;\nnamespace Demo {\n  public abstract class Base {\n    public virtual void Write(string? s) {}\n    public virtual void WriteNum(int? n) {}\n    public virtual void Say(System.String s) {}\n  }\n}\n",
        ),
        (
            "child.cs",
            "namespace Demo {\n  public class Child : Base {\n    public override void Write(string s) {}\n    public override void WriteNum(int n) {}\n    public override void Say(string s) {}\n  }\n}\n",
        ),
    ]);
    let linked: Vec<&str> = csharp
        .links
        .iter()
        .map(|link| csharp.methods[link.declaration].declaration.name.as_str())
        .collect();
    assert!(linked.contains(&"Write"), "{linked:?}");
    assert!(linked.contains(&"Say"), "{linked:?}");
    assert!(!linked.contains(&"WriteNum"), "int? must not match int: {linked:?}");
}

#[test]
fn swift_cross_file_modules_multi_protocol_extensions_and_defaults() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let index = build(&[
        (
            "Sources/Core/Protos.swift",
            concat!(
                "public protocol ProtoA { func run() }\n",
                "extension ProtoA { public func run() {} }\n",
                "public protocol ProtoB { func stop() }\n",
                "public protocol CondProto { func cond() }\n",
            ),
        ),
        (
            "Sources/Core/Impl.swift",
            concat!(
                "struct Impl<T> {}\n",
                "extension Impl: ProtoA, ProtoB {\n",
                "  func run() {}\n",
                "  func stop() {}\n",
                "}\n",
                "extension Impl: CondProto where T: Equatable {\n",
                "  func cond() {}\n",
                "}\n",
            ),
        ),
        (
            "Sources/App/Client.swift",
            "import Core\nstruct Client: ProtoA { func run() {} }\n",
        ),
        (
            "Sources/Other/Unimported.swift",
            "struct Unimported: ProtoA { func run() {} }\n",
        ),
    ]);
    let pairs: Vec<(String, String)> = index
        .links
        .iter()
        .map(|link| {
            let d = &index.methods[link.declaration];
            let i = &index.methods[link.implementation];
            (
                format!("{}.{}", index.typ(d.owner).name, d.declaration.name),
                format!("{}.{}", index.typ(i.owner).name, i.declaration.name),
            )
        })
        .collect();
    assert!(pairs.contains(&("ProtoA.run".into(), "Impl.run".into())), "{pairs:#?}");
    assert!(pairs.contains(&("ProtoB.stop".into(), "Impl.stop".into())), "{pairs:#?}");
    assert!(pairs.contains(&("ProtoA.run".into(), "Client.run".into())), "{pairs:#?}");
    assert!(!pairs.iter().any(|(d, _)| d == "CondProto.cond"), "{pairs:#?}");
    assert!(!pairs.iter().any(|(_, i)| i == "Unimported.run"), "{pairs:#?}");
}

#[test]
fn php_implements_jvm_wildcards_nested_names_and_ts_default_exports() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let php = build(&[
        (
            "src/Iface.php",
            "<?php\nnamespace App;\ninterface Iface { public function send(\\Psr\\Req $r): void; }\n",
        ),
        (
            "src/Client.php",
            "<?php\nnamespace App;\nuse Psr\\Req;\nclass Client implements Iface { public function send(Req $r): void {} }\n",
        ),
    ]);
    assert_eq!(php.links.len(), 1, "{php:#?}");

    let kotlin = build(&[
        (
            "src/main/kotlin/p/Outer.kt",
            "package p\ninterface Outer {\n  interface Builder<T> {\n    fun add(v: T)\n  }\n}\n",
        ),
        (
            "src/main/kotlin/p/Sub.kt",
            "package p\nclass Sub {\n  class Builder : Outer.Builder<Builder> {\n    override fun add(v: Builder) {}\n  }\n}\n",
        ),
    ]);
    assert_eq!(kotlin.links.len(), 1, "{kotlin:#?}");

    let scala = build(&[
        ("src/a/Base.scala", "package a\ntrait Base { def run(): Unit }\n"),
        (
            "src/b/Ok.scala",
            "package b\nimport a.{Base => B}\nclass Ok extends B { def run(): Unit = () }\n",
        ),
        (
            "src/b/Hidden.scala",
            "package b\nimport a.{Base => _}\nclass Hidden extends Base { def run(): Unit = () }\n",
        ),
    ]);
    assert_eq!(scala.links.len(), 1, "{scala:#?}");
    assert_eq!(scala.typ(scala.methods[scala.links[0].implementation].owner).name, "Ok");

    let ts = build_with_disk(&[
        (
            "src/base.ts",
            "export default abstract class Base { abstract run(): void; }\n",
        ),
        (
            "src/child.ts",
            "import MyBase from './base';\nexport class Child extends MyBase { run(): void {} }\n",
        ),
    ]);
    assert_eq!(ts.links.len(), 1, "{ts:#?}");
}

#[test]
fn csharp_partial_with_conditional_part_and_body_parse_error() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
    let index = build_with_disk(&[
        ("App.csproj", "<Project Sdk=\"Microsoft.NET.Sdk\"></Project>\n"),
        (
            "Reader.Async.cs",
            "#if HAVE_ASYNC\nnamespace Demo {\n  public abstract partial class Reader {\n    public virtual void ReadAsync() {}\n  }\n}\n#endif\n",
        ),
        (
            "Reader.cs",
            concat!(
                "namespace Demo {\n",
                "  public abstract partial class Reader {\n",
                "    public abstract bool Read();\n",
                "#if HAVE_EXTRA\n",
                "    /// doc\n",
                "    public virtual void Extra() {}\n",
                "#endif\n",
                "  }\n",
                "}\n",
            ),
        ),
        (
            "SubReader.cs",
            "namespace Demo {\n  public class SubReader : Reader {\n    public override bool Read() { return true; }\n    public override void ReadAsync() {}\n  }\n}\n",
        ),
    ]);
    let linked: Vec<&str> = index
        .links
        .iter()
        .map(|link| index.methods[link.declaration].declaration.name.as_str())
        .collect();
    assert_eq!(linked, vec!["Read"], "{linked:?} | types={:#?}", index.types);
}
