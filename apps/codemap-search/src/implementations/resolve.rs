use super::*;
use crate::callers::resolution::SourceResolver;
use crate::parser::{CodeRange, ImportKind};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

pub(super) fn encloses(outer: &CodeRange, inner: &CodeRange) -> bool {
    (outer.start_line, outer.start_col) <= (inner.start_line, inner.start_col)
        && (inner.end_line, inner.end_col) <= (outer.end_line, outer.end_col)
}
fn normalized(name: &str) -> String {
    name.trim_start_matches(['*', '&'])
        .replace("::", ".")
        .replace('\\', ".")
}
/// `module` joined to the directory of `path`, with `.` and `..` resolved.
pub(crate) fn relative_path(path: &str, module: &str) -> Option<String> {
    let joined = Path::new(path).parent()?.join(module);
    let mut result = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::Normal(value) => result.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(result.to_string_lossy().replace('\\', "/"))
}
fn module_matches(path: &str, module: &str) -> bool {
    path == module
        || [
            "ts", "tsx", "js", "jsx", "mts", "mjs", "cts", "cjs", "py", "dart",
        ]
        .iter()
        .any(|ext| path == format!("{module}.{ext}") || path == format!("{module}/index.{ext}"))
        || path == format!("{module}.rb")
        || path == format!("{module}/__init__.py")
}

impl ImplementationIndex {
    pub(super) fn resolve_type(
        &self,
        file: usize,
        unit_id: usize,
        namespace: &str,
        reference: &TypeReference,
        resolver: &SourceResolver<'_>,
    ) -> Option<(usize, Vec<Location>)> {
        let source = &self.files[file];
        let unit = &source.navigation.as_ref()?.implementations.as_ref()?.units[unit_id];
        let name = normalized(&reference.name);
        // Substitution and conditional constraints need a complete proof. A bare name
        // must never accidentally match a generic specialization.
        if !reference.arguments.is_empty() || name.contains(['<', '>', '[', ']', '+', '?']) {
            return None;
        }
        if unit.language == "rust" {
            resolver.reset_dependency_tracking();
            if let Some(target) =
                resolver.resolve_name(source, &reference.range, &reference.name, "type")
            {
                let candidates = self.by_name.get(&target.symbol.name)?;
                let matches = candidates
                    .iter()
                    .copied()
                    .filter(|id| {
                        self.files[self.types[*id].file].file_path == target.file.file_path
                            && self.typ(*id).range == target.symbol.range
                    })
                    .collect::<Vec<_>>();
                if matches.len() == 1
                    && self.typ(matches[0]).parameters.is_empty()
                    && self.typ(matches[0]).is_complete
                {
                    let evidence = resolver
                        .source_dependencies()
                        .into_iter()
                        .map(|(path, range)| Location { path, range })
                        .collect();
                    return Some((matches[0], evidence));
                }
                return None;
            }
            // A stored source enables module/cfg checks. Do not bypass a rejected
            // resolution with a weaker same-name guess.
            return None;
        }
        let bare = name.rsplit('.').next()?;
        if self
            .by_name
            .get(bare)
            .is_some_and(|ids| ids.len() > CANDIDATES_PER_QUERY)
        {
            return None;
        }
        if unit.shadows.iter().any(|shadow| {
            shadow.name == name.split('.').next().unwrap_or("")
                && encloses(&shadow.scope, &reference.range)
        }) {
            return None;
        }
        let mut local = Vec::new();
        for &id in self.by_name.get(bare).into_iter().flatten() {
            let node = &self.types[id];
            let typ = self.typ(id);
            if node.file != file
                || node.unit != unit_id
                || !typ.parameters.is_empty()
                || !typ.is_complete
                || !encloses(&typ.scope, &reference.range)
            {
                continue;
            }
            let qualified = normalized(&typ.qualified_name);
            let owner_namespace = normalized(&typ.namespace);
            if qualified == name
                || name == typ.name
                    && (namespace == owner_namespace
                        || namespace.starts_with(&format!("{owner_namespace}."))
                        || owner_namespace.is_empty())
            {
                local.push((
                    id,
                    owner_namespace.len(),
                    typ.scope.end_line - typ.scope.start_line,
                ));
            }
        }
        local.sort_by_key(|(_, depth, size)| (std::cmp::Reverse(*depth), *size));
        if let Some(&(id, depth, size)) = local.first() {
            return (local
                .iter()
                .filter(|(_, d, s)| *d == depth && *s == size)
                .count()
                == 1)
                .then(|| (id, vec![self.location(id)]));
        }
        let mut imported = Vec::new();
        let (head, tail) = name.split_once('.').unwrap_or((&name, ""));
        for import in unit.imports.iter().filter(|import| {
            encloses(&import.scope, &reference.range) && import.entry.local_name == head
        }) {
            let entry = &import.entry;
            let Some(module) = entry.source.as_deref() else {
                continue;
            };
            let target = match entry.kind {
                ImportKind::Named if tail.is_empty() => {
                    entry.imported_name.as_deref().unwrap_or(head)
                }
                ImportKind::Namespace if !tail.is_empty() => tail,
                _ => continue,
            };
            let target_bare = target.rsplit('.').next().unwrap_or(target);
            for &id in self.by_name.get(target_bare).into_iter().flatten() {
                let typ = self.typ(id);
                let target_file = &self.files[self.types[id].file].file_path;
                if family(&self.unit(id).language) != family(&unit.language)
                    || !typ.parameters.is_empty()
                    || !typ.is_complete
                    || family(&unit.language) == "script" && !typ.is_exported
                {
                    continue;
                }
                let matches = if matches!(unit.language.as_str(), "typescript" | "javascript")
                    && module.starts_with('.')
                {
                    relative_path(&source.file_path, module)
                        .is_some_and(|module| module_matches(target_file, &module))
                        && typ.namespace.is_empty()
                } else if unit.language == "python" && module.starts_with('.') {
                    let levels = module.chars().take_while(|c| *c == '.').count();
                    let module = format!(
                        "{}{}",
                        "../".repeat(levels.saturating_sub(1)),
                        module[levels..].replace('.', "/")
                    );
                    relative_path(&source.file_path, &module).is_some_and(|module| {
                        module_matches(target_file, module.trim_end_matches('/'))
                    }) && typ.namespace.is_empty()
                } else if matches!(
                    unit.language.as_str(),
                    "java" | "kotlin" | "scala" | "groovy"
                ) {
                    normalized(&typ.qualified_name) == normalized(target)
                } else if matches!(unit.language.as_str(), "php" | "csharp") {
                    // `use App\Store;` and `using Alias = App.Store;` name the type itself.
                    normalized(&typ.qualified_name) == normalized(module)
                } else if family(&unit.language) == "script" {
                    // Package names and tsconfig `paths`/`baseUrl` aliases.
                    resolver
                        .script_module(&source.file_path, module)
                        .is_some_and(|file| file.file_path == *target_file)
                        && typ.namespace.is_empty()
                } else if unit.language == "python" {
                    // An absolute module path, at any source root.
                    let module = module.replace('.', "/");
                    [format!("{module}.py"), format!("{module}/__init__.py")]
                        .iter()
                        .any(|candidate| {
                            target_file == candidate
                                || target_file.ends_with(&format!("/{candidate}"))
                        })
                        && typ.namespace.is_empty()
                } else if unit.language == "dart" {
                    self.dart_libraries
                        .matches(&source.file_path, module, target_file)
                        && typ.namespace.is_empty()
                } else {
                    false
                };
                if matches {
                    imported.push((
                        id,
                        vec![
                            Location {
                                path: source.file_path.clone(),
                                range: entry.range.clone(),
                            },
                            self.location(id),
                        ],
                        same_source_root(&source.file_path, target_file),
                    ));
                }
            }
        }
        imported.sort_by_key(|(id, _, _)| *id);
        imported.dedup_by_key(|(id, _, _)| *id);
        // A qualified name is unique on one classpath; independent monorepo apps may repeat
        // it, and then only the importing app's own declaration is the target.
        if imported.len() > 1 {
            imported.retain(|(_, _, is_same_root)| *is_same_root);
        }
        if imported.len() == 1 {
            return imported.pop().map(|(id, evidence, _)| (id, evidence));
        }
        if !imported.is_empty() {
            return None;
        }
        if let Some(found) = self.required_member(unit, &source.file_path, bare) {
            return Some(found);
        }
        self.namespace_member(unit, &source.file_path, namespace, &name, bare)
    }

    /// A type that the reference's package or namespace makes visible without an import.
    /// Membership is language-defined: Go packages are directories, JVM packages stay in one
    /// source root, and C# and PHP namespaces span the repository but must be unambiguous.
    /// JS, Python and Ruby files do not share a scope merely because they share a directory.
    fn namespace_member(
        &self,
        unit: &ImplementationUnit,
        path: &str,
        namespace: &str,
        name: &str,
        bare: &str,
    ) -> Option<(usize, Vec<Location>)> {
        let language = unit.language.as_str();
        if !matches!(
            language,
            "go" | "java" | "csharp" | "kotlin" | "scala" | "groovy" | "php"
        ) {
            return None;
        }
        // A reference inside a type carries that type in its namespace; the package itself
        // is the namespace the reference sits in or under.
        let scope = normalized(namespace);
        let encloses =
            |package: &str| scope == package || scope.starts_with(&format!("{package}."));
        // C# also sees every enclosing namespace, the global one and each `using`.
        let usings = unit
            .imports
            .iter()
            .filter(|import| import.entry.kind == ImportKind::Namespace)
            .filter_map(|import| import.entry.source.as_deref())
            .map(normalized)
            .collect::<Vec<_>>();
        let candidates = self
            .by_name
            .get(bare)
            .into_iter()
            .flatten()
            .copied()
            .filter(|id| {
                let typ = self.typ(*id);
                let target = &self.files[self.types[*id].file].file_path;
                if self.unit(*id).language != language
                    || !typ.parameters.is_empty()
                    || !typ.is_complete
                {
                    return false;
                }
                if normalized(&typ.qualified_name) == name {
                    return true;
                }
                let package = normalized(&typ.namespace);
                let is_same_directory = Path::new(target).parent() == Path::new(path).parent();
                name == typ.name
                    && match language {
                        "go" => is_same_directory && (package.is_empty() || encloses(&package)),
                        "csharp" => {
                            package.is_empty() || encloses(&package) || usings.contains(&package)
                        }
                        "php" => package == normalized(&unit.namespace),
                        _ if package.is_empty() => is_same_directory,
                        _ => same_source_root(path, target) && encloses(&package),
                    }
            })
            .collect::<Vec<_>>();
        let [id] = candidates.as_slice() else {
            return None;
        };
        Some((*id, vec![self.location(*id)]))
    }

    /// A type declared in a file whose names this one loads as a whole: a Ruby file loaded
    /// with `require_relative` or a Dart library imported without a prefix.
    fn required_member(
        &self,
        unit: &ImplementationUnit,
        path: &str,
        bare: &str,
    ) -> Option<(usize, Vec<Location>)> {
        let language = unit.language.as_str();
        if !matches!(language, "ruby" | "dart") {
            return None;
        }
        let mut candidates = Vec::new();
        for import in &unit.imports {
            let Some(module) = import.entry.source.as_deref() else {
                continue;
            };
            let loads = |target: &str| match language {
                "ruby" => {
                    (module.starts_with("./") || module.starts_with("../"))
                        && relative_path(path, module)
                            .is_some_and(|module| module_matches(target, &module))
                }
                _ => {
                    import.entry.kind == ImportKind::Glob
                        && self.dart_libraries.matches(path, module, target)
                }
            };
            for &id in self.by_name.get(bare).into_iter().flatten() {
                let typ = self.typ(id);
                if self.unit(id).language == language
                    && typ.parameters.is_empty()
                    && typ.is_complete
                    && loads(&self.files[self.types[id].file].file_path)
                {
                    candidates.push((
                        id,
                        vec![
                            Location {
                                path: path.to_string(),
                                range: import.entry.range.clone(),
                            },
                            self.location(id),
                        ],
                    ));
                }
            }
        }
        candidates.sort_by_key(|(id, _)| *id);
        candidates.dedup_by_key(|(id, _)| *id);
        if candidates.len() == 1 {
            return candidates.pop();
        }
        None
    }

    pub(super) fn remember_proof(
        &mut self,
        evidence: &[Location],
        sources: &HashMap<String, String>,
    ) {
        for location in evidence {
            if !self.proof_digests.contains_key(&location.path) {
                if let Some(source) = sources.get(&location.path) {
                    self.proof_digests
                        .insert(location.path.clone(), digest(source.as_bytes()));
                }
            }
        }
    }
}

/// Indexed Dart files counted by their path below a package's `lib/` directory, to tell
/// which workspace package a `package:` import names when several repeat that path.
#[derive(Clone, Debug, Default)]
pub(crate) struct DartLibraries(HashMap<String, usize>);

impl DartLibraries {
    pub(crate) fn new<'p>(paths: impl Iterator<Item = &'p str>) -> Self {
        let mut counts = HashMap::new();
        for (_, below) in paths
            .filter(|path| path.ends_with(".dart"))
            .filter_map(package_library)
        {
            *counts.entry(below.to_string()).or_insert(0) += 1;
        }
        Self(counts)
    }

    /// Whether a Dart import URI names `target`. `package:name/x.dart` is `lib/x.dart` of that
    /// package: the only indexed one, else the one in a directory named after the package. A
    /// URI without a scheme is relative, and `dart:` libraries are not indexed.
    pub(crate) fn matches(&self, path: &str, uri: &str, target: &str) -> bool {
        if let Some(rest) = uri.strip_prefix("package:") {
            let Some((package, file)) = rest.split_once('/') else {
                return false;
            };
            return package_library(target).is_some_and(|(directory, below)| {
                below == file
                    && (self.0.get(file) == Some(&1)
                        || directory.rsplit('/').next() == Some(package))
            });
        }
        !uri.contains(':') && relative_path(path, uri).is_some_and(|module| target == module)
    }
}

/// A path split at its package's `lib/` directory into the package directory and the rest.
fn package_library(path: &str) -> Option<(&str, &str)> {
    match path.strip_prefix("lib/") {
        Some(below) => Some(("", below)),
        None => path.split_once("/lib/"),
    }
}

pub(crate) fn same_source_root(left: &str, right: &str) -> bool {
    // Explicit qualified imports still cannot bridge independent monorepo apps.
    let prefix = |path: &str| {
        if path.starts_with("src/") {
            return Some(String::new());
        }
        path.split_once("/src/")
            .map(|(prefix, _)| prefix.to_string())
    };
    match (prefix(left), prefix(right)) {
        (Some(a), Some(b)) => a == b,
        _ => Path::new(left).parent() == Path::new(right).parent(),
    }
}
