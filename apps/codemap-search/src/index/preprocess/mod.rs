#[cfg(test)]
use crate::parser::CodeExtractor;
mod command;
mod mapping;
#[cfg(test)]
mod tests;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, UNIX_EPOCH};

use crate::config::MacroExpansionConfig;
use crate::parser::{
    ExtractedFile, MacroExpansionInfo, MacroInputStamp, MacroSymbolOrigin, TreeSitterExtractor,
};
use command::{CommandCache, Dialect, Invocation};
use mapping::SourceMap;

/// Native preprocessing supplements the source parser by default for supported dialects.
pub struct MacroExpander {
    root: PathBuf,
    config: MacroExpansionConfig,
    commands: CommandCache,
}

impl MacroExpander {
    pub fn new(root: &Path, config: MacroExpansionConfig) -> Self {
        Self {
            root: crate::workspace::canonicalize_path_lenient(root),
            config,
            commands: CommandCache::default(),
        }
    }

    pub fn expand(&mut self, path: &Path, source: &str, extracted: &mut ExtractedFile) {
        let _ = self.expand_for_index(path, source, extracted);
    }

    pub(crate) fn expand_for_index(
        &mut self,
        path: &Path,
        source: &str,
        extracted: &mut ExtractedFile,
    ) -> Option<crate::parser::IndexAuxiliary> {
        if !self.config.is_enabled || Dialect::for_path(path).is_none() {
            return None;
        }
        let path = crate::workspace::canonicalize_path_lenient(&self.root.join(path));
        let mut auxiliary = None;
        let info = match self.expand_inner(&path, source, extracted) {
            Ok((info, expanded_auxiliary)) => {
                auxiliary = Some(expanded_auxiliary);
                info
            }
            Err(error) => MacroExpansionInfo {
                notice: format!(
                    "Macro expansion unresolved: {error}. Original-source declarations are shown; call and constant-reference attribution is unresolved."
                ),
                symbols: Vec::new(),
                inputs: vec![input_stamp(&path)],
                active_lines: None,
            },
        };
        let navigation = extracted.navigation.get_or_insert_with(Default::default);
        navigation.calls.clear();
        navigation.references.clear();
        navigation.local_bindings.clear();
        navigation.imports.clear();
        navigation.macro_expansion = Some(info);
        auxiliary
    }

    fn expand_inner(
        &mut self,
        path: &Path,
        source: &str,
        extracted: &mut ExtractedFile,
    ) -> Result<(MacroExpansionInfo, crate::parser::IndexAuxiliary), String> {
        let invocation = self.commands.invocation(&self.root, path, &self.config)?;
        let output = run_bounded(&invocation, &self.config, None)?;
        if std::fs::read(path).map_err(|e| e.to_string())? != source.as_bytes() {
            return Err("source changed during preprocessing".into());
        }
        let mut map = if invocation.dialect == Dialect::Nasm {
            let listing = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
            let mut assembly = invocation.clone();
            assembly.arguments.retain(|argument| argument != "-E");
            assembly.arguments.extend([
                "-Le".into(),
                "-Lm".into(),
                "-Lf".into(),
                "-l".into(),
                listing.path().display().to_string(),
                "-o".into(),
                if cfg!(windows) { "NUL" } else { "/dev/null" }.into(),
            ]);
            let text = run_bounded(&assembly, &self.config, Some(listing.path()))?;
            if std::fs::read(path).map_err(|e| e.to_string())? != source.as_bytes() {
                return Err("source changed during NASM listing generation".into());
            }
            SourceMap::nasm_listing(&text, &output, path, &invocation.directory)?
        } else {
            SourceMap::parse(&output, path, &invocation.directory)?
        };
        let active_lines = map.active_lines();
        if invocation.dialect == Dialect::Nasm {
            map.prepare_nasm_declarations()?;
        } else if invocation.dialect == Dialect::Gas {
            let is_arm = output.lines().any(|line| {
                line.starts_with("#define __arm__ ") || line.starts_with("#define __aarch64__ ")
            });
            map.prepare_assembly(invocation.dialect == Dialect::Gas, is_arm);
        }
        let mut inputs: Vec<_> = map
            .inputs
            .iter()
            .chain(invocation.inputs.iter())
            .map(|path| input_stamp(path))
            .collect();
        inputs.sort_by(|a, b| a.path.cmp(&b.path));
        inputs.dedup_by(|a, b| a.path == b.path);
        for path in &map.inputs {
            if let Ok(input) = std::fs::read_to_string(path) {
                if mapping::has_explicit_line_directive(&input) {
                    return Err(format!(
                        "explicit line directive prevents physical source mapping: {}",
                        path.display()
                    ));
                }
            }
        }
        let virtual_path = format!("preprocessed.{}", invocation.dialect.extension());
        let spec = crate::lang::spec_for_path(Path::new(&virtual_path))
            .ok_or("unsupported expanded-source grammar")?;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&spec.grammar(invocation.dialect.extension()))
            .map_err(|e| e.to_string())?;
        let tree = crate::parser::parse_source(&mut parser, map.text.as_bytes())?;
        if tree.root_node().has_error() {
            return Err("expanded source contains unsupported syntax; declaration boundaries could not be verified".into());
        }
        let (expanded, mut auxiliary) =
            TreeSitterExtractor::new().extract_for_index(&map.text, &virtual_path)?;
        auxiliary.static_collection_edges.clear();
        let source_lines: Vec<_> = source.lines().collect();
        let mut symbols = Vec::new();
        let mut origins = Vec::new();
        for mut symbol in expanded.symbols {
            let range = map
                .original_range(&symbol.range, &source_lines)
                .ok_or("expanded declaration could not be mapped to original lines")?;
            if let Some(existing) = extracted.symbols.iter().find(|old| {
                old.name == symbol.name
                    && old.kind == symbol.kind
                    && old.owner == symbol.owner
                    && old.range.start_line == range.start_line
                    && old.range.end_line_inclusive() == range.end_line_inclusive()
            }) {
                // Original coordinates and comments survive preprocessing; semantic
                // flags (for example macro-introduced static storage) come from expansion.
                symbol.range = existing.range.clone();
                symbol.docstring = existing.docstring.clone();
                symbol.flags.has_todo |= existing.flags.has_todo;
                symbol.flags.has_fixme |= existing.flags.has_fixme;
                symbol.flags.is_test |= existing.flags.is_test;
                symbol.flags.is_deprecated |= existing.flags.is_deprecated;
            } else {
                symbol.range = range;
                symbol.flags.is_test |= crate::lang::path_indicates_test(&extracted.file_path);
                origins.push(MacroSymbolOrigin {
                    name: symbol.name.clone(),
                    range: symbol.range.clone(),
                });
            }
            symbols.push(symbol);
        }
        // Keep original macro definitions themselves; -E has consumed them. Expanded
        // active declarations replace raw conditional branches and macro-shaped guesses.
        symbols.extend(
            extracted
                .symbols
                .iter()
                .filter(|symbol| {
                    source_lines
                        .get(symbol.range.start_line.saturating_sub(1))
                        .is_some_and(|line| {
                            line.trim_start().starts_with("#define")
                                || line.trim_start().starts_with("%macro")
                        })
                })
                .cloned(),
        );
        symbols.sort_by(|a, b| {
            a.range
                .start_line
                .cmp(&b.range.start_line)
                .then(a.range.start_col.cmp(&b.range.start_col))
                .then(a.name.cmp(&b.name))
        });
        symbols.dedup_by(|a, b| a.name == b.name && a.kind == b.kind && a.range == b.range);
        extracted.symbols = symbols;
        let notice = format!("Macro expansion: {} declaration(s) mapped to original source lines using {} ({}){}. Expanded-token columns are unresolved. Call and constant-reference attribution is unresolved for preprocessed files.", origins.len(), invocation.program, invocation.source,
            if invocation.dialect == Dialect::Nasm { "; NASM listing labels/globals, generated macro-local labels omitted" } else { "" });
        let info = MacroExpansionInfo {
            notice,
            symbols: origins,
            inputs,
            active_lines: Some(active_lines),
        };
        if let Some(navigation) = extracted.navigation.as_mut() {
            navigation
                .calls
                .retain(|site| info.is_active_line(site.range.start_line));
            navigation
                .references
                .retain(|site| info.is_active_line(site.range.start_line));
        }
        Ok((info, auxiliary))
    }
}

pub(super) fn is_eligible(path: &Path) -> bool {
    Dialect::for_path(path).is_some()
}

pub(crate) fn input_stamp(path: &Path) -> MacroInputStamp {
    let metadata = std::fs::metadata(path).ok();
    MacroInputStamp {
        path: path.display().to_string(),
        modified_ns: metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)),
        size_bytes: metadata.map_or(0, |m| m.len()),
    }
}

fn read_capped(mut stream: impl Read, cap: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .by_ref()
        .take((cap as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn run_bounded(
    invocation: &Invocation,
    config: &MacroExpansionConfig,
    output_file: Option<&Path>,
) -> Result<String, String> {
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.arguments)
        .current_dir(&invocation.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", invocation.program))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let cap = config.max_output_bytes;
    let output = std::thread::spawn(move || read_capped(stdout, cap));
    let diagnostics = std::thread::spawn(move || read_capped(stderr, 16_384));
    let started = Instant::now();
    let status = loop {
        if output_file
            .and_then(|path| std::fs::metadata(path).ok())
            .is_some_and(|metadata| metadata.len() > cap as u64)
        {
            stop_child(&mut child);
            break Err(format!("preprocessor listing exceeds {cap} bytes"));
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                stop_child(&mut child);
                break Ok(status);
            }
            Ok(None) if started.elapsed() < Duration::from_millis(config.timeout_ms) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            outcome => {
                stop_child(&mut child);
                break Err(match outcome {
                    Err(e) => e.to_string(),
                    _ => format!("preprocessor exceeded {} ms", config.timeout_ms),
                });
            }
        }
    };
    let output = output
        .join()
        .map_err(|_| "preprocessor output reader stopped")?
        .map_err(|e| e.to_string())?;
    let diagnostics = diagnostics
        .join()
        .map_err(|_| "preprocessor diagnostic reader stopped")?
        .map_err(|e| e.to_string())?;
    if output.len() > cap {
        return Err(format!("preprocessor output exceeds {cap} bytes"));
    }
    if !status?.success() {
        let message = String::from_utf8_lossy(&diagnostics)
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!("preprocessing failed: {message}"));
    }
    let bytes = if let Some(path) = output_file {
        let bytes = read_capped(std::fs::File::open(path).map_err(|e| e.to_string())?, cap)
            .map_err(|e| e.to_string())?;
        if bytes.len() > cap {
            return Err(format!("preprocessor listing exceeds {cap} bytes"));
        }
        bytes
    } else {
        output
    };
    String::from_utf8(bytes).map_err(|_| "preprocessor output is not UTF-8".into())
}

fn stop_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // SAFETY: the child was started in a new process group whose ID is its PID.
        // Terminating that owned group also closes inherited stdout/stderr pipes.
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}
