use super::*;

#[test]
#[ignore = "Requires an installed Clang preprocessor"]
fn native_clang_expands_declarations_and_preserves_original_file_positions() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let header = root.join("macros.h");
    std::fs::write(&header, "#define DECLARE(name) int name##_expanded(void) { return VALUE; }\n#define INTERNAL static\nint header_only(void);\n").unwrap();
    let source = "#include \"macros.h\"\n#if FEATURE\nDECLARE(chosen)\n#else\nDECLARE(inactive)\n#endif\nint ordinary(void) { return chosen_expanded(); }\nconst char* text = \"DECLARE(phantom)\";\nINTERNAL int internal_only(void) { return 7; }\n";
    let path = root.join("probe.c");
    std::fs::write(&path, source).unwrap();
    let database = serde_json::json!([{"directory": root, "file": "probe.c", "arguments": ["this-project-wrapper-must-not-run", "-DFEATURE=1", "-DVALUE=7", "-c", "probe.c", "-o", "must-not-exist.o"]}]);
    std::fs::write(root.join("compile_commands.json"), database.to_string()).unwrap();
    let config = MacroExpansionConfig {
        is_enabled: true,
        ..Default::default()
    };
    let mut extracted = TreeSitterExtractor::new()
        .extract(source, "probe.c")
        .unwrap();
    MacroExpander::new(&root, config).expand(&path, source, &mut extracted);
    let info = extracted
        .navigation
        .as_ref()
        .unwrap()
        .macro_expansion
        .as_ref()
        .unwrap();
    assert!(
        info.notice.starts_with("Macro expansion:"),
        "{}",
        info.notice
    );
    let names: Vec<_> = extracted
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(names.contains(&"chosen_expanded"), "{names:?}");
    assert!(
        !extracted
            .symbols
            .iter()
            .find(|symbol| symbol.name == "internal_only")
            .unwrap()
            .flags
            .is_exported
    );
    for absent in ["header_only", "inactive_expanded", "phantom_expanded"] {
        assert!(!names.contains(&absent), "{names:?}");
    }
    let generated = info
        .symbols
        .iter()
        .find(|symbol| symbol.name == "chosen_expanded")
        .unwrap();
    assert_eq!(
        (
            generated.range.start_line,
            generated.range.end_line_inclusive()
        ),
        (3, 3)
    );
    assert!(!info.is_active_line(5));
    assert!(info
        .inputs
        .iter()
        .any(|stamp| stamp.path == header.to_string_lossy()));
    assert!(!root.join("must-not-exist.o").exists());

    std::fs::remove_file(root.join("compile_commands.json")).unwrap();
    let source = "#define METHOD(name) int name() const { return 7; }\nstruct Native { METHOD(expanded_method) };\n";
    let path = root.join("methods.c");
    std::fs::write(&path, source).unwrap();
    let mut extracted = TreeSitterExtractor::new()
        .extract(source, "methods.c")
        .unwrap();
    MacroExpander::new(
        &root,
        MacroExpansionConfig {
            is_enabled: true,
            clang_flags: vec!["-x".into(), "c++".into()],
            ..Default::default()
        },
    )
    .expand(&path, source, &mut extracted);
    assert!(
        extracted
            .symbols
            .iter()
            .any(|symbol| symbol.name == "expanded_method"
                && symbol.owner.as_deref() == Some("Native")
                && symbol.range.start_line == 2),
        "{extracted:?}"
    );

    let source = "#define ENTRY(name) .globl name; name:\nENTRY(actual_entry)\n ret\n";
    let path = root.join("entry.S");
    std::fs::write(&path, source).unwrap();
    let mut extracted = TreeSitterExtractor::new()
        .extract(source, "entry.S")
        .unwrap();
    MacroExpander::new(
        &root,
        MacroExpansionConfig {
            is_enabled: true,
            ..Default::default()
        },
    )
    .expand(&path, source, &mut extracted);
    let info = extracted
        .navigation
        .as_ref()
        .unwrap()
        .macro_expansion
        .as_ref()
        .unwrap();
    assert!(
        info.notice.starts_with("Macro expansion:"),
        "{}",
        info.notice
    );
    assert!(extracted
        .symbols
        .iter()
        .any(|symbol| symbol.name == "actual_entry" && symbol.range.start_line == 2));
}

#[test]
fn unavailable_preprocessor_preserves_original_source_declarations() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("plain.c");
    let source = "int ordinary(void) { return 7; }\n";
    std::fs::write(&path, source).unwrap();
    let mut file = TreeSitterExtractor::new()
        .extract(source, "plain.c")
        .unwrap();
    let original = file.symbols.clone();
    let config = MacroExpansionConfig {
        is_enabled: true,
        clang_path: root.path().join("missing-clang").display().to_string(),
        ..Default::default()
    };
    MacroExpander::new(root.path(), config).expand(&path, source, &mut file);
    assert_eq!(file.symbols, original);
    assert!(file
        .navigation
        .as_ref()
        .unwrap()
        .macro_expansion
        .as_ref()
        .unwrap()
        .notice
        .contains("cannot start"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}

#[test]
#[ignore = "Requires NASM; CODEMAP_VALIDATION_NASM may select its executable"]
fn native_nasm_expansions_link_to_calls_instead_of_macro_bodies() {
    let root = tempfile::tempdir().unwrap();
    let source =
        "%macro ENTRY 1\n%1:\n ret\n%endmacro\nENTRY generated_entry\nENTRY second_entry\n";
    let path = root.path().join("entry.asm");
    std::fs::write(&path, source).unwrap();
    let config = MacroExpansionConfig {
        is_enabled: true,
        nasm_path: std::env::var("CODEMAP_VALIDATION_NASM").unwrap_or_else(|_| "nasm".into()),
        ..Default::default()
    };
    let mut file = TreeSitterExtractor::new()
        .extract(source, "entry.asm")
        .unwrap();
    MacroExpander::new(root.path(), config.clone()).expand(&path, source, &mut file);
    let info = file
        .navigation
        .as_ref()
        .unwrap()
        .macro_expansion
        .as_ref()
        .unwrap();
    assert!(
        info.notice.starts_with("Macro expansion:"),
        "{}",
        info.notice
    );
    for (name, line) in [("generated_entry", 5), ("second_entry", 6)] {
        assert!(
            file.symbols
                .iter()
                .any(|symbol| symbol.name == name && symbol.range.start_line == line),
            "{file:?}"
        );
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    let header = root.path().join("declarations.inc");
    std::fs::write(&header, "%macro ENTRY 1\n[global %1:function hidden]\n[sectalign 16]\ntimes (0 / 8) db 0\n%%padding:\n%1 :\n ret\n%endmacro\n").unwrap();
    let source = "%include \"declarations.inc\"\nENTRY generated_entry\nENTRY second_entry\ndb \"phantom:\"\n";
    std::fs::write(&path, source).unwrap();
    let mut file = TreeSitterExtractor::new()
        .extract(source, "entry.asm")
        .unwrap();
    let mut config = config;
    config.nasm_flags = vec!["-felf64".into()];
    MacroExpander::new(root.path(), config).expand(&path, source, &mut file);
    let info = file.macro_expansion().unwrap();
    assert!(
        info.notice.starts_with("Macro expansion:"),
        "{}",
        info.notice
    );
    let declarations: Vec<_> = file
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.name.as_str(),
                symbol.range.start_line,
                symbol.flags.is_exported,
            )
        })
        .collect();
    assert_eq!(
        declarations,
        [("generated_entry", 2, true), ("second_entry", 3, true)]
    );
    assert!(info.is_active_line(4));
    assert!(info
        .inputs
        .iter()
        .any(|input| Path::new(&input.path) == header.canonicalize().unwrap()));
}

#[cfg(unix)]
#[test]
fn process_budget_kills_descendants_that_hold_output_pipes() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("slow-preprocessor");
    std::fs::write(&path, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let invocation = Invocation {
        program: path.display().to_string(),
        arguments: Vec::new(),
        directory: root.path().into(),
        source: "test".into(),
        inputs: Vec::new(),
        dialect: Dialect::C,
    };
    let config = MacroExpansionConfig {
        timeout_ms: 50,
        ..Default::default()
    };
    let started = Instant::now();
    let error = run_bounded(&invocation, &config, None).unwrap_err();
    assert!(error.contains("exceeded 50 ms"), "{error}");
    assert!(started.elapsed() < Duration::from_secs(3));
}
