use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::MacroExpansionConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dialect {
    C,
    Cpp,
    Gas,
    Nasm,
}

impl Dialect {
    pub(super) fn for_path(path: &Path) -> Option<Self> {
        match crate::lang::spec_for_path(path)?.language_name() {
            "c" => Some(Self::C),
            "cpp" => Some(Self::Cpp),
            "asm" if path.extension().is_some_and(|ext| ext == "asm") => Some(Self::Nasm),
            "asm" => Some(Self::Gas),
            _ => None,
        }
    }
    pub(super) fn extension(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Gas => "S",
            Self::Nasm => "asm",
        }
    }
}

#[derive(Clone)]
pub(super) struct Invocation {
    pub program: String,
    pub arguments: Vec<String>,
    pub directory: PathBuf,
    pub source: String,
    pub inputs: Vec<PathBuf>,
    pub dialect: Dialect,
}

#[derive(Clone, Deserialize)]
struct Entry {
    directory: PathBuf,
    file: PathBuf,
    arguments: Option<Vec<String>>,
    command: Option<String>,
}

#[derive(Default)]
pub(super) struct CommandCache {
    databases: HashMap<PathBuf, Result<HashMap<PathBuf, Entry>, String>>,
}

fn absolute(path: &Path, directory: &Path) -> PathBuf {
    crate::workspace::canonicalize_path_lenient(&directory.join(path))
}

impl CommandCache {
    pub(super) fn invocation(
        &mut self,
        root: &Path,
        path: &Path,
        config: &MacroExpansionConfig,
    ) -> Result<Invocation, String> {
        let mut dialect = Dialect::for_path(path).ok_or("unsupported source dialect")?;
        let mut directory = root.to_path_buf();
        let mut raw = Vec::new();
        let mut inputs = Vec::new();
        let mut source = "fallback flags".to_string();
        let explicit = config.compilation_database.as_ref().map(|p| {
            let p = absolute(Path::new(p), root);
            if p.is_dir() {
                p.join("compile_commands.json")
            } else {
                p
            }
        });
        let database = if let Some(path) = explicit {
            Some(path)
        } else {
            path.parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .take_while(|parent| parent.starts_with(root))
                .find_map(|parent| {
                    ["compile_commands.json", "compile_flags.txt"]
                        .into_iter()
                        .map(|name| parent.join(name))
                        .find(|p| p.is_file())
                })
        };
        if let Some(database) = database {
            inputs.push(database.clone());
            source = database.display().to_string();
            if database
                .file_name()
                .is_some_and(|name| name == "compile_flags.txt")
            {
                directory = database.parent().unwrap().to_path_buf();
                let text = std::fs::read_to_string(&database)
                    .map_err(|e| format!("cannot read compilation flags: {e}"))?;
                raw = text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(str::to_string)
                    .collect();
            } else {
                let entries = self
                    .databases
                    .entry(database.clone())
                    .or_insert_with(|| {
                        let input = std::fs::File::open(&database)
                            .map_err(|e| format!("cannot read compilation database: {e}"))?;
                        let bytes = super::read_capped(input, 64 * 1024 * 1024)
                            .map_err(|e| format!("cannot read compilation database: {e}"))?;
                        if bytes.len() > 64 * 1024 * 1024 {
                            return Err("compilation database exceeds 64 MiB".into());
                        }
                        let entries: Vec<Entry> = serde_json::from_slice(&bytes)
                            .map_err(|e| format!("invalid compilation database: {e}"))?;
                        let mut by_path = HashMap::new();
                        for mut entry in entries {
                            entry.directory =
                                absolute(&entry.directory, database.parent().unwrap());
                            by_path
                                .entry(absolute(&entry.file, &entry.directory))
                                .or_insert(entry);
                        }
                        Ok(by_path)
                    })
                    .as_ref()
                    .map_err(Clone::clone)?;
                let entry = entries.get(path).ok_or_else(|| format!("no exact compile command for {}; add a matching database entry or point compilation_database to a compile_flags.txt file", path.display()))?;
                directory = entry.directory.clone();
                let args = match (&entry.arguments, &entry.command) {
                    (Some(args), _) => args.clone(),
                    (_, Some(command)) => split_command(command)?,
                    _ => return Err("compile command has neither arguments nor command".into()),
                };
                if args.is_empty() {
                    return Err("empty compile command".into());
                }
                // The recorded compiler is context only; never execute project wrappers.
                if dialect == Dialect::C && args[0].contains("++") {
                    dialect = Dialect::Cpp;
                }
                raw.extend(args.into_iter().skip(1));
            }
        }
        let extra = if dialect == Dialect::Nasm {
            &config.nasm_flags
        } else {
            &config.clang_flags
        };
        raw.extend(extra.iter().cloned());
        let mut arguments = preprocessing_flags(&raw, dialect, path, &directory)?;
        if dialect != Dialect::Nasm {
            for pair in arguments.windows(2) {
                if pair[0] == "-x" {
                    dialect = match pair[1].as_str() {
                        "c" | "c-header" => Dialect::C,
                        "c++" | "c++-header" => Dialect::Cpp,
                        "assembler-with-cpp" => Dialect::Gas,
                        language => {
                            return Err(format!("unsupported preprocessing language: {language}"))
                        }
                    };
                }
            }
        }
        if dialect == Dialect::Nasm {
            arguments.push("-E".into());
        } else {
            if !arguments
                .iter()
                .any(|arg| arg == "-x" || arg.starts_with("-x"))
            {
                arguments.extend([
                    "-x".into(),
                    match dialect {
                        Dialect::C => "c",
                        Dialect::Cpp => "c++",
                        _ => "assembler-with-cpp",
                    }
                    .into(),
                ]);
            }
            arguments.extend(["-E".into(), "-dD".into(), "-fno-color-diagnostics".into()]);
        }
        arguments.push(path.display().to_string());
        Ok(Invocation {
            program: if dialect == Dialect::Nasm {
                config.nasm_path.clone()
            } else {
                config.clang_path.clone()
            },
            arguments,
            directory,
            source,
            inputs,
            dialect,
        })
    }
}

/// Compilation database command strings use shell quoting, but are never shell scripts.
fn split_command(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut is_word = false;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (None, '\'' | '"') => {
                quote = Some(ch);
                is_word = true;
            }
            (Some('\''), _) => {
                word.push(ch);
            }
            (_, '\\') => {
                let next = chars.next().ok_or("unfinished escape in compile command")?;
                if quote == Some('"') && !matches!(next, '"' | '\\' | '$' | '`' | '\n') {
                    word.push('\\');
                }
                if next != '\n' {
                    word.push(next);
                }
                is_word = true;
            }
            (None, ch) if ch.is_whitespace() => {
                if is_word {
                    words.push(std::mem::take(&mut word));
                    is_word = false;
                }
            }
            (_, ch) => {
                word.push(ch);
                is_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unterminated quote in compile command".into());
    }
    if is_word {
        words.push(word);
    }
    Ok(words)
}

fn preprocessing_flags(
    raw: &[String],
    dialect: Dialect,
    path: &Path,
    directory: &Path,
) -> Result<Vec<String>, String> {
    let path = absolute(path, directory);
    let mut output = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        let arg = &raw[index];
        index += 1;
        if !arg.starts_with('-') {
            if absolute(Path::new(arg), directory) == path {
                continue;
            }
            return Err(format!("unsupported compile argument: {arg}"));
        }
        let takes_value = if dialect == Dialect::Nasm {
            ["-I", "-i", "-D", "-d", "-U", "-u", "-P", "-p", "-f"].contains(&arg.as_str())
        } else {
            [
                "-I",
                "-D",
                "-U",
                "-isystem",
                "-iquote",
                "-idirafter",
                "-include",
                "-imacros",
                "-isysroot",
                "--sysroot",
                "-target",
                "--target",
                "-arch",
                "-x",
                "-std",
            ]
            .contains(&arg.as_str())
        };
        if takes_value {
            let value = raw
                .get(index)
                .ok_or_else(|| format!("missing value for {arg}"))?;
            output.extend([arg.clone(), value.clone()]);
            index += 1;
            continue;
        }
        if ["-o", "-MF", "-MT", "-MQ", "-MJ"].contains(&arg.as_str()) {
            if raw.get(index).is_none() {
                return Err(format!("missing value for {arg}"));
            }
            index += 1;
            continue;
        }
        if ["-c", "-S", "-E", "-MD", "-MMD", "-MP", "-MG", "-M", "-MM"].contains(&arg.as_str())
            || arg.starts_with("-g")
            || (arg.starts_with("-W") && !arg.starts_with("-Wp,") && !arg.starts_with("-Wa,"))
        {
            continue;
        }
        let joined = if dialect == Dialect::Nasm {
            ["-I", "-i", "-D", "-d", "-U", "-u", "-P", "-p", "-f"]
                .iter()
                .any(|prefix| arg.starts_with(prefix) && arg.len() > prefix.len())
        } else {
            [
                "-I",
                "-D",
                "-U",
                "-std=",
                "--sysroot=",
                "--target=",
                "-isystem",
                "-iquote",
                "-idirafter",
            ]
            .iter()
            .any(|prefix| arg.starts_with(prefix) && arg.len() > prefix.len())
                || [
                    "-nostdinc",
                    "-nostdinc++",
                    "-undef",
                    "-pthread",
                    "-m32",
                    "-m64",
                    "-fPIC",
                    "-fpic",
                    "-fPIE",
                    "-fpie",
                    "-fms-extensions",
                    "-fms-compatibility",
                    "-fdeclspec",
                    "-O0",
                    "-O1",
                    "-O2",
                    "-O3",
                    "-Os",
                    "-Oz",
                    "-Og",
                    "-Ofast",
                ]
                .contains(&arg.as_str())
        };
        if joined {
            output.push(arg.clone());
        } else {
            return Err(format!("unsupported preprocessing option: {arg}"));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_are_tokenized_without_shell_expansion_and_write_flags_are_removed() {
        assert_eq!(
            split_command("clang -DNAME='a b' \"file name.c\" $(touch sentinel)").unwrap(),
            ["clang", "-DNAME=a b", "file name.c", "$(touch", "sentinel)"]
        );
        let path = Path::new("/tmp/probe.c");
        let raw = [
            "-Iinc",
            "-D",
            "VALUE=7",
            "-c",
            "probe.c",
            "-o",
            "artifact.o",
            "-MF",
            "deps.d",
        ]
        .map(str::to_string);
        assert_eq!(
            preprocessing_flags(&raw, Dialect::C, path, Path::new("/tmp")).unwrap(),
            ["-Iinc", "-D", "VALUE=7"]
        );
        for flag in [
            "-fplugin=plugin.so",
            "-Xclang",
            "@options.rsp",
            "--config=project.cfg",
            "-Wp,-MD,output",
        ] {
            assert!(
                preprocessing_flags(&[flag.into()], Dialect::C, path, Path::new("/tmp")).is_err(),
                "{flag}"
            );
        }
    }
}
