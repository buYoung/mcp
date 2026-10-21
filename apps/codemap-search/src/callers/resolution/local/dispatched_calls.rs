//! Typed member calls proven by the implementation index: the receiver's declared type,
//! inheritance and imports single out one method that no subtype redeclares.

use super::*;

impl<'a> SourceResolver<'a> {
    pub(in crate::callers::resolution) fn dispatched_call(
        &self,
        file: &'a ExtractedFile,
        call: &CallSite,
    ) -> Option<Target<'a>> {
        let index = self.implementations?;
        let target = index.direct_call(&file.file_path, &call.range, &call.name)?;
        // A local function or variable of the same name shadows an implicit member.
        if call.receiver.is_none() {
            let source = self.source(&file.file_path)?;
            let at = node_at(&source, &call.range)?;
            if has_binding(&source, at, &call.name, None) {
                return None;
            }
        }
        if !target
            .proof_paths
            .iter()
            .all(|path| self.is_indexed_generation(index, path))
        {
            return None;
        }
        let target_file = self.files_by_path.get(target.path).copied()?;
        let symbol = target_file.symbols.iter().find(|symbol| {
            symbol.kind == "fn" && symbol.name == call.name && symbol.range == *target.range
        })?;
        Some(Target {
            file: target_file,
            symbol,
            is_precise: true,
        })
    }

    /// Whether `path` still holds the source generation the implementation index read.
    fn is_indexed_generation(
        &self,
        index: &crate::implementations::ImplementationIndex,
        path: &str,
    ) -> bool {
        if let Some(is_current) = self.indexed_generations.borrow().get(path) {
            return *is_current;
        }
        let is_current = index.proof_digest(path).is_some_and(|expected| {
            self.read_source(path)
                .is_some_and(|text| crate::implementations::digest(text.as_bytes()) == expected)
        });
        self.indexed_generations
            .borrow_mut()
            .insert(path.to_string(), is_current);
        is_current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{CodeExtractor, TreeSitterExtractor};

    /// A source file and the expected targets of named calls, by call line where given.
    type OneLineCase<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);
    type LineCase<'a> = (&'a str, &'a str, &'a [(usize, &'a str, &'a str)]);

    /// Targets of the calls named `name` on `line` of `caller`, as `path:line`, or `-`.
    fn targets(entries: &[(&str, &str)], caller: &str, line: usize, name: &str) -> Vec<String> {
        let sources: HashMap<String, String> = entries
            .iter()
            .map(|(path, source)| (path.to_string(), source.to_string()))
            .collect();
        let files = std::sync::Arc::new(
            entries
                .iter()
                .map(|(path, source)| TreeSitterExtractor::new().extract(source, path).unwrap())
                .collect::<Vec<_>>(),
        );
        let index = crate::implementations::ImplementationIndex::build(
            std::sync::Arc::clone(&files),
            &sources,
        );
        let resolver = SourceResolver::from_stored_sources(&files, Path::new("."), &sources)
            .with_implementations(&index);
        let file = files.iter().find(|file| file.file_path == caller).unwrap();
        resolver
            .calls(file)
            .iter()
            .filter(|call| call.range.start_line == line && call.name == name)
            .map(|call| {
                resolver
                    .resolve_call(file, call)
                    .map_or("-".into(), |target| {
                        format!(
                            "{}:{}",
                            target.file.file_path, target.symbol.range.start_line
                        )
                    })
            })
            .collect()
    }

    #[test]
    fn self_inherited_and_typed_parameter_calls_reach_one_method_in_each_language() {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        // Line 1 declares `shared` on the base; the worker's `helper` is on line 3 and the
        // calls on line 4 are self, inherited and through a parameter of the worker type.
        let one_line: &[OneLineCase<'_>] = &[
            ("case.java", "class Base { void shared() {} }\nclass Worker extends Base {\n  private void helper() {}\n  void run(Worker other) { this.helper(); shared(); other.helper(); }\n}\n", &[("helper", "case.java:3"), ("shared", "case.java:1")]),
            ("case.cs", "class Base { public void shared() {} }\nclass Worker : Base {\n  private void helper() {}\n  public void run(Worker other) { this.helper(); shared(); other.helper(); }\n}\n", &[("helper", "case.cs:3"), ("shared", "case.cs:1")]),
            ("case.scala", "class Base { def shared(): Unit = {} }\nclass Worker extends Base {\n  private def helper(): Unit = {}\n  def run(other: Worker): Unit = { this.helper(); shared(); other.helper() }\n}\n", &[("helper", "case.scala:3"), ("shared", "case.scala:1")]),
            ("case.swift", "class Base { func shared() {} }\nclass Worker: Base {\n  private func helper() {}\n  func run(other: Worker) { self.helper(); shared(); other.helper() }\n}\n", &[("helper", "case.swift:3"), ("shared", "case.swift:1")]),
            ("case.cpp", "class Base { public: void shared() {} };\nclass Worker : public Base {\n  void helper() {}\n  public: void run(Worker& other) { this->helper(); shared(); other.helper(); }\n};\n", &[("helper", "case.cpp:3"), ("shared", "case.cpp:1")]),
            ("case.ts", "class Base { shared() {} }\nclass Worker extends Base {\n  private helper() {}\n  run(other: Worker) { this.helper(); this.shared(); other.helper(); }\n}\n", &[("helper", "case.ts:3"), ("shared", "case.ts:1")]),
            ("case.js", "class Base { shared() {} }\nclass Worker extends Base {\n  helper() {}\n  run() { this.helper(); this.shared(); }\n}\n", &[("helper", "case.js:3"), ("shared", "case.js:1")]),
            ("case.php", "<?php class Base { public function shared() {} }\nclass Worker extends Base {\n  private function helper() {}\n  public function run(Worker $other) { $this->helper(); $this->shared(); $other->helper(); }\n}\n", &[("helper", "case.php:3"), ("shared", "case.php:1")]),
            ("case.rb", "class Base; def shared; end; end\nclass Worker < Base\n  def helper; end\n  def run; self.helper; shared(); end\nend\n", &[("helper", "case.rb:3"), ("shared", "case.rb:1")]),
            ("case.ps1", "class Base { [void] shared() {} }\nclass Worker : Base {\n  [void] helper() {}\n  [void] run([Worker] $other) { $this.helper(); $this.shared(); $other.helper() }\n}\n", &[("helper", "case.ps1:3"), ("shared", "case.ps1:1")]),
        ];
        // Grammars that need statement separators keep one call per line.
        let multi_line: &[LineCase<'_>] = &[
            ("case.kt", "open class Base {\n  fun shared() {}\n}\nclass Worker : Base() {\n  private fun helper() {}\n  fun run(other: Worker) {\n    this.helper()\n    shared()\n    other.helper()\n  }\n}\n", &[(7, "helper", "case.kt:5"), (8, "shared", "case.kt:2"), (9, "helper", "case.kt:5")]),
            ("case.groovy", "class Base {\n  void shared() {}\n}\nclass Worker extends Base {\n  private void helper() {}\n  void run(Worker other) {\n    this.helper()\n    shared()\n    other.helper()\n  }\n}\n", &[(7, "helper", "case.groovy:5"), (8, "shared", "case.groovy:2"), (9, "helper", "case.groovy:5")]),
            ("case.py", "class Base:\n    def shared(self): pass\nclass Worker(Base):\n    def helper(self): pass\n    def run(self, other: Worker):\n        self.helper()\n        self.shared()\n        other.helper()\n", &[(6, "helper", "case.py:4"), (7, "shared", "case.py:2"), (8, "helper", "case.py:4")]),
            ("case.dart", "class Base {\n  void shared() {}\n}\nclass Worker extends Base {\n  void _helper() {}\n  void run(Worker other) {\n    this._helper();\n    shared();\n    other._helper();\n  }\n}\n", &[(7, "_helper", "case.dart:5"), (8, "shared", "case.dart:2"), (9, "_helper", "case.dart:5")]),
        ];
        let mut failures = Vec::new();
        let calls = one_line
            .iter()
            .flat_map(|(path, source, expected)| {
                expected
                    .iter()
                    .map(move |(name, target)| (*path, *source, 4, *name, *target))
            })
            .chain(multi_line.iter().flat_map(|(path, source, expected)| {
                expected
                    .iter()
                    .map(move |(line, name, target)| (*path, *source, *line, *name, *target))
            }));
        for (path, source, line, name, target) in calls {
            let found = targets(&[(path, source)], path, line, name);
            if found.is_empty() || found.iter().any(|found| found != target) {
                failures.push(format!(
                    "{path}:{line} {name}: {found:?}, expected {target}"
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn field_static_and_base_receivers_reach_one_method_in_each_language() {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        let cases: &[LineCase<'_>] = &[
            ("case.java", "class Store {\n  void save() {}\n  static Store create() { return new Store(); }\n}\nclass Worker {\n  private final Store store = Store.create();\n  void run() {\n    store.save();\n    this.store.save();\n  }\n}\n", &[(6, "create", "case.java:3"), (8, "save", "case.java:2"), (9, "save", "case.java:2")]),
            ("case.cs", "class Store {\n  public void Save() {}\n  public static Store Create() { return new Store(); }\n}\nclass Worker {\n  private readonly Store store = Store.Create();\n  public void Run() {\n    store.Save();\n    this.store.Save();\n  }\n}\n", &[(6, "Create", "case.cs:3"), (8, "Save", "case.cs:2"), (9, "Save", "case.cs:2")]),
            ("case.kt", "class Store {\n  fun save() {}\n}\nclass Worker(private val store: Store) {\n  fun run() {\n    store.save()\n    this.store.save()\n  }\n}\n", &[(6, "save", "case.kt:2"), (7, "save", "case.kt:2")]),
            ("case.scala", "class Store {\n  def save(): Unit = {}\n}\nclass Worker(store: Store) {\n  def run(): Unit = {\n    store.save()\n  }\n}\n", &[(6, "save", "case.scala:2")]),
            ("case.groovy", "class Store {\n  void save() {}\n}\nclass Worker {\n  private Store store\n  void run() {\n    store.save()\n    this.store.save()\n  }\n}\n", &[(7, "save", "case.groovy:2"), (8, "save", "case.groovy:2")]),
            ("case.swift", "class Store {\n  func save() {}\n}\nclass Worker {\n  let store: Store\n  init(store: Store) { self.store = store }\n  func run() {\n    store.save()\n    self.store.save()\n  }\n}\n", &[(8, "save", "case.swift:2"), (9, "save", "case.swift:2")]),
            ("case.dart", "class Store {\n  void save() {}\n}\nclass Worker {\n  final Store store;\n  Worker(this.store);\n  void run() {\n    store.save();\n    this.store.save();\n  }\n}\n", &[(8, "save", "case.dart:2"), (9, "save", "case.dart:2")]),
            ("case.php", "<?php\nclass Store {\n  public function save() {}\n}\nclass Worker {\n  public function __construct(private Store $store) {}\n  public function run() {\n    $this->store->save();\n  }\n}\n", &[(8, "save", "case.php:3")]),
            ("case.py", "class Store:\n    def save(self): pass\nclass Worker:\n    def __init__(self, store: Store):\n        self.store = store\n    def run(self):\n        self.store.save()\n", &[(7, "save", "case.py:2")]),
            ("case.rb", "class Store\n  def save\n  end\nend\nclass Worker\n  def initialize\n    @store = Store.new\n  end\n  def run\n    @store.save\n  end\nend\n", &[(10, "save", "case.rb:2")]),
            ("case.cpp", "class Store {\n public:\n  void save() {}\n};\nclass Worker {\n  Store store;\n  Store* other;\n public:\n  void run() {\n    store.save();\n    this->store.save();\n    other->save();\n  }\n};\n", &[(10, "save", "case.cpp:3"), (11, "save", "case.cpp:3"), (12, "save", "case.cpp:3")]),
            ("case.ps1", "class Store {\n  [void] save() {}\n}\nclass Worker {\n  [Store] $store\n  [void] run() {\n    $this.store.save()\n  }\n}\n", &[(7, "save", "case.ps1:2")]),
            ("case.ts", "class Store {\n  save() {}\n  static create() { return new Store(); }\n}\nclass Base {\n  shared() {}\n}\nclass Worker extends Base {\n  shared() { super.shared(); Store.create(); }\n}\n", &[(9, "shared", "case.ts:6"), (9, "create", "case.ts:3")]),
            ("case.java", "class Base {\n  void shared() {}\n}\nclass Worker extends Base {\n  void shared() { super.shared(); }\n}\n", &[(5, "shared", "case.java:2")]),
        ];
        let mut failures = Vec::new();
        for (path, source, expected) in cases {
            for (line, name, target) in *expected {
                let found = targets(&[(path, source)], path, *line, name);
                if found.is_empty() || found.iter().any(|found| found != target) {
                    failures.push(format!(
                        "{path}:{line} {name}: {found:?}, expected {target}"
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn imported_receiver_types_resolve_across_files() {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        let java = [
            ("src/main/java/app/store/Store.java", "package app.store;\npublic class Store {\n  public void save() {}\n}\n"),
            ("src/main/java/app/api/Api.java", "package app.api;\nimport app.store.Store;\nclass Api {\n  void run(Store store) { store.save(); }\n}\n"),
        ];
        assert_eq!(
            targets(&java, "src/main/java/app/api/Api.java", 4, "save"),
            ["src/main/java/app/store/Store.java:3"]
        );
        let python = [
            ("pkg/store.py", "class Store:\n    def save(self): pass\n"),
            (
                "pkg/api.py",
                "from .store import Store\ndef run(store: Store):\n    store.save()\n",
            ),
        ];
        assert_eq!(
            targets(&python, "pkg/api.py", 3, "save"),
            ["pkg/store.py:2"]
        );
        let script = [
            ("src/store.ts", "export class Store {\n  save() {}\n}\n"),
            ("src/api.ts", "import { Store } from './store';\nexport function run(store: Store) {\n  store.save();\n}\n"),
        ];
        assert_eq!(
            targets(&script, "src/api.ts", 3, "save"),
            ["src/store.ts:2"]
        );
        // An explicit import reaches another module; a repeated qualified name stays
        // within the importing module.
        let modules = [
            ("transmitter/src/main/java/app/admin/Config.java", "package app.admin;\npublic class Config {\n  public void apply() {}\n}\n"),
            ("services/src/main/java/app/api/Api.java", "package app.api;\nimport app.admin.Config;\nclass Api {\n  void run(Config config) { config.apply(); }\n}\n"),
        ];
        assert_eq!(
            targets(
                &modules,
                "services/src/main/java/app/api/Api.java",
                4,
                "apply"
            ),
            ["transmitter/src/main/java/app/admin/Config.java:3"]
        );
        let repeated = [
            modules[0],
            modules[1],
            (
                "other/src/main/java/app/admin/Config.java",
                "package app.admin;\npublic class Config {\n  public void apply() {}\n}\n",
            ),
        ];
        assert_eq!(
            targets(
                &repeated,
                "services/src/main/java/app/api/Api.java",
                4,
                "apply"
            ),
            ["-"]
        );
        // Same-package types need no import, also from inside a class.
        let package = [
            ("app/Store.java", "package app;\npublic class Store {\n  public void save() {}\n}\n"),
            ("app/Worker.java", "package app;\nclass Worker {\n  private final Store store = new Store();\n  void run() {\n    store.save();\n  }\n}\n"),
            ("app/Ledger.kt", "package app\nclass Ledger {\n  fun close() {}\n}\n"),
            ("app/Closer.kt", "package app\nclass Closer(private val ledger: Ledger) {\n  fun run() {\n    ledger.close()\n  }\n}\n"),
        ];
        assert_eq!(
            targets(&package, "app/Worker.java", 5, "save"),
            ["app/Store.java:3"]
        );
        assert_eq!(
            targets(&package, "app/Closer.kt", 4, "close"),
            ["app/Ledger.kt:3"]
        );
    }

    #[test]
    fn stale_sources_and_local_names_keep_typed_calls_unresolved() {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        let entries = [
            ("case.kt", "open class Base {\n  fun shared() {}\n}\nclass Worker : Base() {\n  fun run() {\n    shared()\n  }\n  fun local() {\n    val shared = { 1 }\n    shared()\n  }\n}\n"),
        ];
        assert_eq!(targets(&entries, "case.kt", 6, "shared"), ["case.kt:2"]);
        // A local function value named like an inherited member shadows it.
        assert_eq!(targets(&entries, "case.kt", 10, "shared"), ["-"]);
        // The same call after an edit is no longer covered by the indexed proof.
        let sources: HashMap<String, String> = entries
            .iter()
            .map(|(path, source)| (path.to_string(), source.to_string()))
            .collect();
        let files = std::sync::Arc::new(vec![TreeSitterExtractor::new()
            .extract(entries[0].1, entries[0].0)
            .unwrap()]);
        let index = crate::implementations::ImplementationIndex::build(
            std::sync::Arc::clone(&files),
            &sources,
        );
        let edited: HashMap<String, String> = [(
            "case.kt".to_string(),
            format!("{}// edited\n", entries[0].1),
        )]
        .into();
        let resolver = SourceResolver::from_stored_sources(&files, Path::new("."), &edited)
            .with_implementations(&index);
        let call = resolver
            .calls(&files[0])
            .into_iter()
            .find(|call| call.range.start_line == 6)
            .unwrap();
        assert!(resolver.resolve_call(&files[0], &call).is_none());
    }
}
