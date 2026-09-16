//! Development artifacts only. No process or filesystem access is used by the
//! production source analyzer.
use serde_json::{json, Value as Json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn read_report(path: &Path) -> Json {
    let mut report: Json = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    if let Some(artifact) = report["analysis_artifact"].as_str() {
        let artifact = path.parent().unwrap().join(artifact);
        assert_eq!(
            super::tests::source_sha256(&artifact),
            report["analysis_artifact_sha256"].as_str().unwrap()
        );
        let output = Command::new("gzip")
            .arg("-dc")
            .arg(artifact)
            .output()
            .unwrap();
        assert!(output.status.success());
        let raw: Json = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            raw["native_implementation_blake3"],
            report["native_implementation_blake3"]
        );
        assert_eq!(raw["source_sha256"], report["source_sha256"]);
        report["analyses"] = raw["analyses"].clone();
    }
    report
}
pub(super) fn write_report(path: &Path, mut report: Json) {
    let fingerprint = implementation_fingerprint();
    report["native_implementation_blake3"] = json!(fingerprint);
    report["recorded_unix_seconds"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if let Some(analyses) = report.as_object_mut().unwrap().remove("analyses") {
        let artifact = path.with_extension("analyses.json.gz");
        let file = std::fs::File::create(&artifact).unwrap();
        let mut child = Command::new("gzip")
            .args(["-c", "-n"])
            .stdin(Stdio::piped())
            .stdout(file)
            .spawn()
            .unwrap();
        let raw = json!({"analyses":analyses,"native_implementation_blake3":fingerprint,"source_sha256":report["source_sha256"],"max_file_bytes":report["max_file_bytes"]});
        let bytes = serde_json::to_vec(&raw).unwrap();
        child.stdin.take().unwrap().write_all(&bytes).unwrap();
        assert!(child.wait().unwrap().success());
        report["analysis_artifact"] = json!(artifact.file_name().unwrap().to_str().unwrap());
        report["analysis_artifact_sha256"] = json!(super::tests::source_sha256(&artifact));
    }
    std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
// Compiled source bytes bind results to the implementation actually executed,
// even if the working tree is edited while a verification process is running.
pub(super) fn implementation_fingerprint() -> String {
    let mut hash = blake3::Hasher::new();
    for (name, bytes) in IMPLEMENTATION_SOURCES {
        hash.update(name.as_bytes());
        hash.update(&[0]);
        hash.update(bytes.as_bytes());
        hash.update(&[0]);
    }
    hash.finalize().to_hex().to_string()
}
const IMPLEMENTATION_SOURCES: &[(&str, &str)] = &[
    ("Cargo.lock", include_str!("../../../Cargo.lock")),
    ("Cargo.toml", include_str!("../../../Cargo.toml")),
    (
        "src/callers/resolution/local.rs",
        include_str!("../../callers/resolution/local.rs"),
    ),
    (
        "src/callers/resolution/rust_cfg.rs",
        include_str!("../../callers/resolution/rust_cfg.rs"),
    ),
    (
        "src/callers/resolution/rust_files.rs",
        include_str!("../../callers/resolution/rust_files.rs"),
    ),
    (
        "src/callers/test_code.rs",
        include_str!("../../callers/test_code.rs"),
    ),
    ("src/config.rs", include_str!("../../config.rs")),
    ("src/events/extract.rs", include_str!("../extract.rs")),
    ("src/events/index.rs", include_str!("../index.rs")),
    ("src/events/inputs.rs", include_str!("../inputs.rs")),
    ("src/events/javascript.rs", include_str!("../javascript.rs")),
    ("src/events/mod.rs", include_str!("../mod.rs")),
    ("src/events/model.rs", include_str!("../model.rs")),
    ("src/events/rules.rs", include_str!("../rules.rs")),
    (
        "src/events/source_routes/assembly.rs",
        include_str!("assembly.rs"),
    ),
    (
        "src/events/source_routes/callback_bindings.rs",
        include_str!("callback_bindings.rs"),
    ),
    (
        "src/events/source_routes/calls.rs",
        include_str!("calls.rs"),
    ),
    (
        "src/events/source_routes/conditional_syntax.rs",
        include_str!("conditional_syntax.rs"),
    ),
    (
        "src/events/source_routes/dependencies.rs",
        include_str!("dependencies.rs"),
    ),
    (
        "src/events/source_routes/engine.rs",
        include_str!("engine.rs"),
    ),
    (
        "src/events/source_routes/expressions.rs",
        include_str!("expressions.rs"),
    ),
    (
        "src/events/source_routes/generators.rs",
        include_str!("generators.rs"),
    ),
    ("src/events/source_routes/jvm.rs", include_str!("jvm.rs")),
    (
        "src/events/source_routes/manifests.rs",
        include_str!("manifests.rs"),
    ),
    ("src/events/source_routes/mod.rs", include_str!("mod.rs")),
    (
        "src/events/source_routes/model.rs",
        include_str!("model.rs"),
    ),
    (
        "src/events/source_routes/object_projections.rs",
        include_str!("object_projections.rs"),
    ),
    (
        "src/events/source_routes/polyglot.rs",
        include_str!("polyglot.rs"),
    ),
    (
        "src/events/source_routes/polyglot_expressions.rs",
        include_str!("polyglot_expressions.rs"),
    ),
    (
        "src/events/source_routes/polyglot_projections.rs",
        include_str!("polyglot_projections.rs"),
    ),
    (
        "src/events/source_routes/polyglot_statements.rs",
        include_str!("polyglot_statements.rs"),
    ),
    (
        "src/events/source_routes/program.rs",
        include_str!("program.rs"),
    ),
    (
        "src/events/source_routes/projections.rs",
        include_str!("projections.rs"),
    ),
    (
        "src/events/source_routes/prototypes.rs",
        include_str!("prototypes.rs"),
    ),
    (
        "src/events/source_routes/render.rs",
        include_str!("render.rs"),
    ),
    (
        "src/events/source_routes/rust_deref.rs",
        include_str!("rust_deref.rs"),
    ),
    (
        "src/events/source_routes/rust_macros.rs",
        include_str!("rust_macros.rs"),
    ),
    (
        "src/events/source_routes/rust_names.rs",
        include_str!("rust_names.rs"),
    ),
    (
        "src/events/source_routes/rust_returns.rs",
        include_str!("rust_returns.rs"),
    ),
    (
        "src/events/source_routes/rust_types.rs",
        include_str!("rust_types.rs"),
    ),
    (
        "src/events/source_routes/rust_values.rs",
        include_str!("rust_values.rs"),
    ),
    (
        "src/events/source_routes/scala.rs",
        include_str!("scala.rs"),
    ),
    (
        "src/events/source_routes/snapshot.rs",
        include_str!("snapshot.rs"),
    ),
    (
        "src/events/source_routes/statements.rs",
        include_str!("statements.rs"),
    ),
    (
        "src/events/source_routes/syntax.rs",
        include_str!("syntax.rs"),
    ),
    (
        "src/events/source_routes/tests.rs",
        include_str!("tests.rs"),
    ),
    (
        "src/events/source_routes/tests_frontend.rs",
        include_str!("tests_frontend.rs"),
    ),
    (
        "src/events/source_routes/tests_public.rs",
        include_str!("tests_public.rs"),
    ),
    (
        "src/events/source_routes/tests_reporting.rs",
        include_str!("tests_reporting.rs"),
    ),
    (
        "src/events/source_routes/typescript.rs",
        include_str!("typescript.rs"),
    ),
    ("src/events/tests.rs", include_str!("../tests.rs")),
    ("src/index/engine.rs", include_str!("../../index/engine.rs")),
    (
        "src/index/indexer.rs",
        include_str!("../../index/indexer.rs"),
    ),
    (
        "src/index/supervisor.rs",
        include_str!("../../index/supervisor.rs"),
    ),
    ("src/lang/asm.rs", include_str!("../../lang/asm.rs")),
    ("src/lang/bash.rs", include_str!("../../lang/bash.rs")),
    (
        "src/lang/bundled_grammars.rs",
        include_str!("../../lang/bundled_grammars.rs"),
    ),
    (
        "src/lang/c_family/c.rs",
        include_str!("../../lang/c_family/c.rs"),
    ),
    (
        "src/lang/c_family/cpp.rs",
        include_str!("../../lang/c_family/cpp.rs"),
    ),
    (
        "src/lang/c_family/mod.rs",
        include_str!("../../lang/c_family/mod.rs"),
    ),
    ("src/lang/cmake.rs", include_str!("../../lang/cmake.rs")),
    (
        "src/lang/component.rs",
        include_str!("../../lang/component.rs"),
    ),
    (
        "src/lang/containerfile.rs",
        include_str!("../../lang/containerfile.rs"),
    ),
    ("src/lang/csharp.rs", include_str!("../../lang/csharp.rs")),
    ("src/lang/css.rs", include_str!("../../lang/css.rs")),
    ("src/lang/dart.rs", include_str!("../../lang/dart.rs")),
    (
        "src/lang/format_support.rs",
        include_str!("../../lang/format_support.rs"),
    ),
    ("src/lang/go.rs", include_str!("../../lang/go.rs")),
    ("src/lang/graphql.rs", include_str!("../../lang/graphql.rs")),
    ("src/lang/groovy.rs", include_str!("../../lang/groovy.rs")),
    ("src/lang/hcl.rs", include_str!("../../lang/hcl.rs")),
    ("src/lang/html.rs", include_str!("../../lang/html.rs")),
    ("src/lang/java.rs", include_str!("../../lang/java.rs")),
    (
        "src/lang/javascript.rs",
        include_str!("../../lang/javascript.rs"),
    ),
    ("src/lang/json.rs", include_str!("../../lang/json.rs")),
    ("src/lang/kotlin.rs", include_str!("../../lang/kotlin.rs")),
    ("src/lang/less.rs", include_str!("../../lang/less.rs")),
    ("src/lang/lua.rs", include_str!("../../lang/lua.rs")),
    ("src/lang/make.rs", include_str!("../../lang/make.rs")),
    (
        "src/lang/markdown.rs",
        include_str!("../../lang/markdown.rs"),
    ),
    ("src/lang/mod.rs", include_str!("../../lang/mod.rs")),
    ("src/lang/nix.rs", include_str!("../../lang/nix.rs")),
    ("src/lang/php.rs", include_str!("../../lang/php.rs")),
    (
        "src/lang/powershell.rs",
        include_str!("../../lang/powershell.rs"),
    ),
    ("src/lang/proto.rs", include_str!("../../lang/proto.rs")),
    ("src/lang/python.rs", include_str!("../../lang/python.rs")),
    ("src/lang/ruby.rs", include_str!("../../lang/ruby.rs")),
    ("src/lang/rust.rs", include_str!("../../lang/rust.rs")),
    ("src/lang/scala.rs", include_str!("../../lang/scala.rs")),
    ("src/lang/sql.rs", include_str!("../../lang/sql.rs")),
    (
        "src/lang/starlark.rs",
        include_str!("../../lang/starlark.rs"),
    ),
    ("src/lang/swift.rs", include_str!("../../lang/swift.rs")),
    ("src/lang/toml.rs", include_str!("../../lang/toml.rs")),
    (
        "src/lang/typescript.rs",
        include_str!("../../lang/typescript.rs"),
    ),
    ("src/lang/xml.rs", include_str!("../../lang/xml.rs")),
    ("src/lang/yaml.rs", include_str!("../../lang/yaml.rs")),
    ("src/lang/zsh.rs", include_str!("../../lang/zsh.rs")),
    (
        "src/parser/bounded.rs",
        include_str!("../../parser/bounded.rs"),
    ),
    (
        "src/parser/composite.rs",
        include_str!("../../parser/composite.rs"),
    ),
    (
        "src/parser/markdown.rs",
        include_str!("../../parser/markdown.rs"),
    ),
    ("src/parser/mod.rs", include_str!("../../parser/mod.rs")),
    ("src/parser/sass.rs", include_str!("../../parser/sass.rs")),
    (
        "src/parser/tokenize.rs",
        include_str!("../../parser/tokenize.rs"),
    ),
    ("src/parser/types.rs", include_str!("../../parser/types.rs")),
    ("src/tools/grep.rs", include_str!("../../tools/grep.rs")),
    (
        "src/tools/live_symbols.rs",
        include_str!("../../tools/live_symbols.rs"),
    ),
    ("src/tools/read.rs", include_str!("../../tools/read.rs")),
    (
        "src/tools/search/grouped.rs",
        include_str!("../../tools/search/grouped.rs"),
    ),
];
