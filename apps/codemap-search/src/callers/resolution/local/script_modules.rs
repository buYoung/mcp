//! TypeScript/JavaScript module specifiers resolved the way `tsc` does: relative paths from the
//! importing file, then the `compilerOptions.paths` patterns and `baseUrl` of the nearest
//! tsconfig.json, or jsconfig.json in a directory without one, including the configurations it
//! extends. A package import names a workspace package only through its manifest: the one
//! package.json declaring that name, and entry or `exports` paths that reach indexed source
//! rather than build output. A specifier names a target only when exactly one indexed file
//! matches; other packages and ambiguous candidates stay unresolved.

use super::*;
use serde_json::Value;

const SCRIPT_EXTENSIONS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
/// Manifest fields naming a package's entry module.
const ENTRY_FIELDS: [&str; 7] = [
    "main",
    "module",
    "types",
    "typings",
    "source",
    "react-native",
    "browser",
];
/// `extends` chains deeper than this are ignored rather than followed.
const EXTENDS_DEPTH: usize = 5;

/// Module-path settings of one configuration after its `extends`, with directories made absolute.
#[derive(Default)]
pub(in crate::callers::resolution) struct ModulePaths {
    base_url: Option<PathBuf>,
    /// Patterns and their ordered targets from `compilerOptions.paths`.
    paths: Vec<(String, Vec<String>)>,
    /// The directory declaring `paths`; targets resolve from it when no `baseUrl` is set.
    paths_directory: Option<PathBuf>,
}

impl ModulePaths {
    /// Settings a later base configuration declares replace those of earlier ones.
    fn inherit(&mut self, later: ModulePaths) {
        if later.base_url.is_some() {
            self.base_url = later.base_url;
        }
        if later.paths_directory.is_some() {
            self.paths = later.paths;
            self.paths_directory = later.paths_directory;
        }
    }

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

/// TypeScript declaration files, which hold no implementation.
fn is_declaration_file(path: &str) -> bool {
    [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
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

/// The configuration an `extends` entry names: a path relative to the extending file, or a
/// package file in a `node_modules` directory between it and the workspace root.
fn extended_config(directory: &Path, parent: &str, root: &Path) -> Option<PathBuf> {
    if parent.starts_with("./") || parent.starts_with("../") {
        let parent = directory.join(parent);
        return Some(
            if parent
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                parent
            } else {
                append_extension(&parent, "json")
            },
        );
    }
    if Path::new(parent).is_absolute() {
        return None;
    }
    let mut current = directory.to_path_buf();
    loop {
        let package = current.join("node_modules").join(parent);
        // The named file, the name with `.json` added, or the package's tsconfig.json.
        if let Some(config) = [
            package.clone(),
            append_extension(&package, "json"),
            package.join("tsconfig.json"),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file())
        {
            return Some(config);
        }
        if current.as_path() == root || !current.pop() || !current.starts_with(root) {
            return None;
        }
    }
}

fn read_module_paths(path: &Path, depth: usize, root: &Path) -> Option<ModulePaths> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&strip_json_extensions(&text)).ok()?;
    let directory = path.parent()?;
    // One base configuration, or since TypeScript 5.0 a list whose later entries win.
    let parents = match value.get("extends") {
        Some(Value::String(parent)) => vec![parent.as_str()],
        Some(Value::Array(parents)) => parents.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let mut settings = ModulePaths::default();
    if depth < EXTENDS_DEPTH {
        for parent in parents {
            if let Some(inherited) = extended_config(directory, parent, root)
                .and_then(|config| read_module_paths(&config, depth + 1, root))
            {
                settings.inherit(inherited);
            }
        }
    }
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

/// The package name and subpath of a bare specifier: `@scope/name/sub` or `name/sub`.
fn package_specifier(specifier: &str) -> Option<(&str, Option<&str>)> {
    if specifier.contains(':') || specifier.starts_with('/') {
        return None;
    }
    let segments = if specifier.starts_with('@') { 2 } else { 1 };
    let mut end = 0;
    for (count, segment) in specifier.split('/').enumerate().take(segments) {
        if segment.is_empty() {
            return None;
        }
        end += segment.len() + usize::from(count > 0);
    }
    let subpath = specifier[end..]
        .strip_prefix('/')
        .filter(|rest| !rest.is_empty());
    Some((&specifier[..end], subpath))
}

/// Every path string below a manifest value, as nested `exports` conditions hold them.
fn path_leaves<'v>(value: &'v Value, leaves: &mut Vec<&'v str>) {
    match value {
        Value::String(path) => leaves.push(path),
        Value::Array(items) => items.iter().for_each(|item| path_leaves(item, leaves)),
        Value::Object(conditions) => conditions
            .values()
            .for_each(|item| path_leaves(item, leaves)),
        _ => {}
    }
}

/// Paths a manifest gives the package entry (`subpath` `None`) or one subpath: `exports`
/// entries when present, else the entry fields or the subpath itself.
fn manifest_targets(manifest: &Value, subpath: Option<&str>) -> Vec<String> {
    let mut leaves = Vec::new();
    let exports = manifest.get("exports");
    let is_subpath_map = exports
        .and_then(Value::as_object)
        .is_some_and(|map| map.keys().any(|key| key.starts_with('.')));
    match (subpath, exports) {
        (None, Some(exports)) if is_subpath_map => {
            if let Some(entry) = exports.get(".") {
                path_leaves(entry, &mut leaves);
            }
        }
        (None, Some(exports)) => path_leaves(exports, &mut leaves),
        (None, None) => {}
        (Some(subpath), Some(exports)) => {
            let key = format!("./{subpath}");
            let Some(map) = exports.as_object().filter(|_| is_subpath_map) else {
                return Vec::new();
            };
            if let Some(entry) = map.get(&key) {
                path_leaves(entry, &mut leaves);
            } else if let Some((pattern, entry)) = map
                .iter()
                .filter(|(pattern, _)| capture(pattern, &key).is_some())
                .max_by_key(|(pattern, _)| pattern.split('*').next().map_or(0, str::len))
            {
                let captured = capture(pattern, &key).unwrap_or_default();
                let mut patterned = Vec::new();
                path_leaves(entry, &mut patterned);
                return patterned
                    .into_iter()
                    .map(|path| path.replacen('*', captured, 1))
                    .collect();
            }
            return leaves.into_iter().map(str::to_owned).collect();
        }
        (Some(subpath), None) => return vec![subpath.to_string()],
    }
    if subpath.is_none() {
        leaves.extend(
            ENTRY_FIELDS
                .iter()
                .filter_map(|field| manifest.get(*field).and_then(Value::as_str)),
        );
    }
    leaves.into_iter().map(str::to_owned).collect()
}

/// Packages of the workspace by their manifest name, with their directories and manifests.
pub(in crate::callers::resolution) type WorkspacePackages = HashMap<String, Vec<(PathBuf, Value)>>;

impl<'a> SourceResolver<'a> {
    /// The single indexed file `specifier` names from `importer`, or `None` for other packages
    /// and ambiguous or missing targets.
    pub(crate) fn script_module(
        &self,
        importer: &str,
        specifier: &str,
    ) -> Option<&'a ExtractedFile> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let base = self.root.join(importer).parent()?.join(specifier);
            return self.single_module(&[base]);
        }
        if let Some(settings) = self.module_paths(importer) {
            if let Some(file) = self.single_module(&settings.mapped(specifier)) {
                return Some(file);
            }
            let based: Vec<_> = settings
                .base_url
                .iter()
                .map(|base| base.join(specifier))
                .collect();
            if let Some(file) = self.single_module(&based) {
                return Some(file);
            }
        }
        self.package_module(specifier)
    }

    /// The source file a workspace package import names, when the one manifest declaring the
    /// package name points the import at exactly one indexed source file.
    fn package_module(&self, specifier: &str) -> Option<&'a ExtractedFile> {
        let (name, subpath) = package_specifier(specifier)?;
        let packages = self.workspace_packages();
        let [(directory, manifest)] = packages.get(name)?.as_slice() else {
            return None;
        };
        let bases = manifest_targets(manifest, subpath)
            .into_iter()
            .map(|target| directory.join(target))
            .collect::<Vec<_>>();
        // Declaration files describe build output; the implementation lives elsewhere.
        let mut found: Vec<&'a ExtractedFile> = Vec::new();
        for path in bases.iter().flat_map(|base| module_candidates(base)) {
            if let Some(file) = self
                .file(&crate::workspace::canonicalize_path_lenient(&path))
                .filter(|file| !is_declaration_file(&file.file_path))
            {
                if !found.iter().any(|known| std::ptr::eq(*known, file)) {
                    found.push(file);
                }
            }
        }
        (found.len() == 1).then(|| found[0])
    }

    /// Workspace package manifests by declared name, read on first use.
    fn workspace_packages(&self) -> Rc<WorkspacePackages> {
        if let Some(packages) = self.workspace_packages.borrow().as_ref() {
            return Rc::clone(packages);
        }
        let mut packages = WorkspacePackages::new();
        for file in self.files.iter().filter(|file| {
            Path::new(&file.file_path)
                .file_name()
                .is_some_and(|name| name == "package.json")
        }) {
            let Some(manifest) = self
                .read_source(&file.file_path)
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            else {
                continue;
            };
            let Some(name) = manifest.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(directory) = self
                .root
                .join(&file.file_path)
                .parent()
                .map(Path::to_path_buf)
            else {
                continue;
            };
            packages
                .entry(name.to_string())
                .or_default()
                .push((directory, manifest));
        }
        let packages = Rc::new(packages);
        *self.workspace_packages.borrow_mut() = Some(Rc::clone(&packages));
        packages
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

    /// Settings of the nearest tsconfig.json, or jsconfig.json where a directory has no
    /// tsconfig.json, from the importing file's directory up to the workspace root, cached per
    /// starting directory.
    fn module_paths(&self, importer: &str) -> Option<Rc<ModulePaths>> {
        let start = self.root.join(importer).parent()?.to_path_buf();
        if let Some(cached) = self.module_paths.borrow().get(&start) {
            return cached.clone();
        }
        let mut directory = start.clone();
        let settings = loop {
            if let Some(config) = ["tsconfig.json", "jsconfig.json"]
                .into_iter()
                .map(|name| directory.join(name))
                .find(|config| config.is_file())
            {
                break read_module_paths(&config, 0, self.root).map(Rc::new);
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

#[cfg(test)]
mod tests {
    use super::*;

    const HELPER: (&str, &str) = (
        "packages/util/src/helper.ts",
        "export function helper() {}\n",
    );
    const BARREL: (&str, &str) = ("packages/util/src/index.ts", "export * from './helper';\n");
    const CALLER: &str = "apps/web/src/main.ts";

    /// The target of the `helper` call in a workspace with one more package manifest and import.
    fn helper_target(manifests: &[(&str, &str)], specifier: &str) -> Vec<String> {
        let _config = crate::config::pin_test_config(crate::config::ResolvedConfig::default());
        let caller = format!("import {{ helper }} from '{specifier}';\nhelper();\n");
        let mut entries = vec![
            HELPER,
            BARREL,
            (
                "packages/util/index.d.ts",
                "export declare function helper(): void;\n",
            ),
        ];
        entries.extend_from_slice(manifests);
        entries.push((CALLER, &caller));
        targets(&entries, CALLER, 2, "helper")
    }

    #[test]
    fn workspace_package_imports_reach_source_entries_their_manifest_names() {
        let manifest = |body: &str| ("packages/util/package.json", body.to_string());
        for (body, specifier) in [
            (
                r#"{ "name": "@repo/util", "main": "src/index.ts", "types": "index.d.ts" }"#,
                "@repo/util",
            ),
            (
                r#"{ "name": "@repo/util", "main": "build/index.js", "react-native": "src/index" }"#,
                "@repo/util",
            ),
            (
                r#"{ "name": "@repo/util", "exports": { ".": "./build/index.js", "./*": "./src/*.ts" } }"#,
                "@repo/util/helper",
            ),
            (
                r#"{ "name": "@repo/util", "main": "build/index.js" }"#,
                "@repo/util/src/helper",
            ),
        ] {
            let (path, body) = manifest(body);
            assert_eq!(
                helper_target(&[(path, &body)], specifier),
                ["packages/util/src/helper.ts:1"],
                "{body}"
            );
        }
    }

    #[test]
    fn packages_without_source_entries_or_unique_manifests_stay_unresolved() {
        for (manifests, specifier) in [
            (
                &[(
                    "packages/util/package.json",
                    r#"{ "name": "@repo/util", "main": "build/index.js", "types": "index.d.ts" }"#,
                )][..],
                "@repo/util",
            ),
            (
                &[(
                    "packages/util/package.json",
                    r#"{ "name": "@repo/util", "exports": { ".": "./src/index.ts" } }"#,
                )][..],
                "@repo/util/src/helper",
            ),
            (
                &[
                    (
                        "packages/util/package.json",
                        r#"{ "name": "@repo/util", "main": "src/index.ts" }"#,
                    ),
                    (
                        "examples/util/package.json",
                        r#"{ "name": "@repo/util", "main": "index.js" }"#,
                    ),
                ][..],
                "@repo/util",
            ),
        ] {
            assert_eq!(
                helper_target(manifests, specifier),
                ["-"],
                "{manifests:?} {specifier}"
            );
        }
    }
}
