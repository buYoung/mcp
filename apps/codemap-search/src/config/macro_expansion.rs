use std::path::Path;

/// Optional native preprocessing; normal indexing remains self-contained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansionConfig {
    pub is_enabled: bool,
    pub compilation_database: Option<String>,
    pub clang_path: String,
    pub nasm_path: String,
    pub clang_flags: Vec<String>,
    pub nasm_flags: Vec<String>,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl Default for MacroExpansionConfig {
    fn default() -> Self {
        Self {
            is_enabled: false,
            compilation_database: None,
            clang_path: "clang".into(),
            nasm_path: "nasm".into(),
            clang_flags: Vec::new(),
            nasm_flags: Vec::new(),
            timeout_ms: 5_000,
            max_output_bytes: 8_388_608,
        }
    }
}

#[derive(Default)]
pub(super) struct MacroExpansionLayer {
    is_enabled: Option<bool>,
    compilation_database: Option<String>,
    clang_path: Option<String>,
    nasm_path: Option<String>,
    clang_flags: Option<Vec<String>>,
    nasm_flags: Option<Vec<String>>,
    timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
}

pub(super) fn normalize(value: &toml::Value, path: &Path) -> MacroExpansionLayer {
    let mut layer = MacroExpansionLayer::default();
    let Some(table) = value.as_table() else {
        super::warn(&format!(
            "macro_expansion must be a table: {}",
            path.display()
        ));
        return layer;
    };
    for (key, value) in table {
        let label = format!("macro_expansion.{key}");
        match key.as_str() {
            "is_enabled" => layer.is_enabled = super::as_bool(value, &label, path),
            "compilation_database" => {
                layer.compilation_database = super::as_nonempty_string(value, &label, path)
            }
            "clang_path" => layer.clang_path = super::as_nonempty_string(value, &label, path),
            "nasm_path" => layer.nasm_path = super::as_nonempty_string(value, &label, path),
            "clang_flags" => layer.clang_flags = super::as_string_array(value, &label, path),
            "nasm_flags" => layer.nasm_flags = super::as_string_array(value, &label, path),
            "timeout_ms" => layer.timeout_ms = super::as_positive_u64(value, &label, path),
            "max_output_bytes" => {
                layer.max_output_bytes = super::as_positive_usize(value, &label, path)
            }
            _ => super::warn(&format!(
                "unknown config key '{label}': {} — ignored",
                path.display()
            )),
        }
    }
    layer
}

pub(super) fn merge(
    repo: MacroExpansionLayer,
    global: MacroExpansionLayer,
) -> MacroExpansionConfig {
    let defaults = MacroExpansionConfig::default();
    MacroExpansionConfig {
        is_enabled: repo
            .is_enabled
            .or(global.is_enabled)
            .unwrap_or(defaults.is_enabled),
        compilation_database: repo.compilation_database.or(global.compilation_database),
        clang_path: repo
            .clang_path
            .or(global.clang_path)
            .unwrap_or(defaults.clang_path),
        nasm_path: repo
            .nasm_path
            .or(global.nasm_path)
            .unwrap_or(defaults.nasm_path),
        clang_flags: repo.clang_flags.or(global.clang_flags).unwrap_or_default(),
        nasm_flags: repo.nasm_flags.or(global.nasm_flags).unwrap_or_default(),
        timeout_ms: repo
            .timeout_ms
            .or(global.timeout_ms)
            .unwrap_or(defaults.timeout_ms),
        max_output_bytes: repo
            .max_output_bytes
            .or(global.max_output_bytes)
            .unwrap_or(defaults.max_output_bytes),
    }
}
