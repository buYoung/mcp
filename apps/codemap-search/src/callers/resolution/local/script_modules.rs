//! TypeScript/JavaScript module specifiers resolved the way `tsc` does: relative paths from the
//! importing file, then the nearest tsconfig's `compilerOptions.paths` patterns and `baseUrl`.
//! A specifier names a target only when exactly one indexed file matches; package imports and
//! ambiguous candidates stay unresolved.

use super::*;
use serde_json::Value;

const SCRIPT_EXTENSIONS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
/// `extends` chains deeper than this are ignored rather than followed.
const EXTENDS_DEPTH: usize = 5;

/// Module-path settings of one tsconfig after relative `extends`, with directories made absolute.
#[derive(Default)]
pub(in crate::callers::resolution) struct ModulePaths {
    base_url: Option<PathBuf>,
    /// Patterns and their ordered targets from `compilerOptions.paths`.
    paths: Vec<(String, Vec<String>)>,
    /// The directory declaring `paths`; targets resolve from it when no `baseUrl` is set.
    paths_directory: Option<PathBuf>,
}

impl ModulePaths {
    /// Targets of the exact or longest-prefix `paths` pattern, the order `tsc` tries them.
    fn mapped(&self, specifier: &str) -> Vec<PathBuf> {
        let best = self
            .paths
            .iter()
            .filter_map(|(pattern, targets)| {
                capture(pattern, specifier).map(|captured| (pattern, targets, captured))
            })
            .max_by_key(|(pattern, _, _)| {
                (
                    !pattern.contains('*'),
                    pattern.split('*').next().map_or(0, str::len),
                )
            });
        let Some((_, targets, captured)) = best else {
            return Vec::new();
        };
        let Some(directory) = self.base_url.as_ref().or(self.paths_directory.as_ref()) else {
            return Vec::new();
        };
        targets
            .iter()
            .map(|target| directory.join(target.replacen('*', captured, 1)))
            .collect()
    }
}

/// The text a `paths` pattern captures for its `*`, or "" for an exact pattern.
fn capture<'s>(pattern: &str, specifier: &'s str) -> Option<&'s str> {
    match pattern.split_once('*') {
        Some((prefix, suffix)) => (specifier.len() >= prefix.len() + suffix.len()
            && specifier.starts_with(prefix)
            && specifier.ends_with(suffix))
        .then(|| &specifier[prefix.len()..specifier.len() - suffix.len()]),
        None => (pattern == specifier).then_some(""),
    }
}

fn append_extension(path: &Path, extension: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(extension);
    PathBuf::from(name)
}

/// Files a module path may name. A dotted base name (`user.service`) is not an extension, and a
/// `.js` specifier in TypeScript source names the `.ts` file `tsc` compiles to it.
fn module_candidates(base: &Path) -> Vec<PathBuf> {
    let extension = base
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| SCRIPT_EXTENSIONS.contains(extension));
    if let Some(extension) = extension {
        let typed = match extension {
            "js" => Some("ts"),
            "jsx" => Some("tsx"),
            "mjs" => Some("mts"),
            "cjs" => Some("cts"),
            _ => None,
        };
        return std::iter::once(base.to_path_buf())
            .chain(typed.map(|typed| base.with_extension(typed)))
            .collect();
    }
    SCRIPT_EXTENSIONS
        .iter()
        .map(|extension| append_extension(base, extension))
        .chain(
            SCRIPT_EXTENSIONS
                .iter()
                .map(|extension| base.join(format!("index.{extension}"))),
        )
        .collect()
}

/// tsconfig files are JSON with comments and trailing commas.
fn strip_json_extensions(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut is_string = false;
    while let Some(character) = chars.next() {
        if is_string {
            stripped.push(character);
            if character == '\\' {
                stripped.extend(chars.next());
            } else if character == '"' {
                is_string = false;
            }
            continue;
        }
        match character {
            '"' => {
                is_string = true;
                stripped.push(character);
            }
            '/' if chars.peek() == Some(&'/') => {
                while chars.peek().is_some_and(|next| *next != '\n') {
                    chars.next();
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                for next in chars.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
            }
            _ => stripped.push(character),
        }
    }
    let mut result = String::with_capacity(stripped.len());
    let mut is_string = false;
    let mut characters = stripped.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        if is_string {
            result.push(character);
            if character == '\\' {
                result.extend(characters.next().map(|(_, next)| next));
            } else if character == '"' {
                is_string = false;
            }
            continue;
        }
        if character == '"' {
            is_string = true;
        } else if character == ',' && stripped[index + 1..].trim_start().starts_with(['}', ']']) {
            continue;
        }
        result.push(character);
    }
    result
}

fn read_module_paths(path: &Path, depth: usize) -> Option<ModulePaths> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&strip_json_extensions(&text)).ok()?;
    let directory = path.parent()?;
    let mut settings = value
        .get("extends")
        .and_then(Value::as_str)
        .filter(|parent| {
            depth < EXTENDS_DEPTH && (parent.starts_with("./") || parent.starts_with("../"))
        })
        .and_then(|parent| {
            let parent = directory.join(parent);
            let parent = if parent
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                parent
            } else {
                append_extension(&parent, "json")
            };
            read_module_paths(&parent, depth + 1)
        })
        .unwrap_or_default();
    let options = value.get("compilerOptions");
    if let Some(base) = options
        .and_then(|options| options.get("baseUrl"))
        .and_then(Value::as_str)
    {
        settings.base_url = Some(directory.join(base));
    }
    if let Some(paths) = options
        .and_then(|options| options.get("paths"))
        .and_then(Value::as_object)
    {
        settings.paths = paths
            .iter()
            .map(|(pattern, targets)| {
                let targets = targets
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                (pattern.clone(), targets)
            })
            .collect();
        settings.paths_directory = Some(directory.to_path_buf());
    }
    Some(settings)
}

impl<'a> SourceResolver<'a> {
    /// The single indexed file `specifier` names from `importer`, or `None` for packages and
    /// ambiguous or missing targets.
    pub(crate) fn script_module(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Option<&'a ExtractedFile> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let base = self.root.join(importer).parent()?.join(specifier);
            return self.single_module(&[base]);
        }
        let settings = self.module_paths(importer)?;
        let mapped = settings.mapped(specifier);
        if let Some(file) = self.single_module(&mapped) {
            return Some(file);
        }
        let based: Vec<_> = settings
            .base_url
            .iter()
            .map(|base| base.join(specifier))
            .collect();
        self.single_module(&based)
    }

    fn single_module(&self, bases: &[PathBuf]) -> Option<&'a ExtractedFile> {
        let mut found: Vec<&'a ExtractedFile> = Vec::new();
        for path in bases.iter().flat_map(|base| module_candidates(base)) {
            if let Some(file) = self.file(&crate::workspace::canonicalize_path_lenient(&path)) {
                if !found.iter().any(|known| std::ptr::eq(*known, file)) {
                    found.push(file);
                }
            }
        }
        (found.len() == 1).then(|| found[0])
    }

    /// Settings of the nearest tsconfig.json from the importing file's directory up to the
    /// workspace root, cached per starting directory.
    fn module_paths(&self, importer: &str) -> Option<Rc<ModulePaths>> {
        let start = self.root.join(importer).parent()?.to_path_buf();
        if let Some(cached) = self.module_paths.borrow().get(&start) {
            return cached.clone();
        }
        let mut directory = start.clone();
        let settings = loop {
            let config = directory.join("tsconfig.json");
            if config.is_file() {
                break read_module_paths(&config, 0).map(Rc::new);
            }
            if directory.as_path() == self.root
                || !directory.pop()
                || !directory.starts_with(self.root)
            {
                break None;
            }
        };
        self.module_paths
            .borrow_mut()
            .insert(start, settings.clone());
        settings
    }
}
