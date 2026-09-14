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
fn relative_path(path: &str, module: &str) -> Option<String> {
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
                let matches =
                    if matches!(unit.language.as_str(), "typescript" | "javascript" | "dart")
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
                            && same_source_root(&source.file_path, target_file)
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
                    ));
                }
            }
        }
        imported.sort_by_key(|(id, _)| *id);
        imported.dedup_by_key(|(id, _)| *id);
        if imported.len() == 1 {
            return imported.pop();
        }
        if !imported.is_empty() {
            return None;
        }
        // Package/namespace membership is language-defined; JS, Python and Ruby
        // files do not share a scope merely because they have the same directory.
        if matches!(
            unit.language.as_str(),
            "go" | "java" | "csharp" | "kotlin" | "scala" | "groovy"
        ) {
            let candidates = self
                .by_name
                .get(bare)
                .into_iter()
                .flatten()
                .copied()
                .filter(|id| {
                    let typ = self.typ(*id);
                    let target = &self.files[self.types[*id].file].file_path;
                    self.unit(*id).language == unit.language
                        && typ.parameters.is_empty()
                        && typ.is_complete
                        && Path::new(target).parent() == Path::new(&source.file_path).parent()
                        && (normalized(&typ.qualified_name) == name
                            || name == typ.name && typ.namespace == namespace)
                })
                .collect::<Vec<_>>();
            if candidates.len() == 1 {
                let id = candidates[0];
                return Some((id, vec![self.location(id)]));
            }
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

fn same_source_root(left: &str, right: &str) -> bool {
    // Explicit qualified imports still cannot bridge independent monorepo apps.
    let prefix = |path: &str| {
        path.split_once("/src/")
            .map(|(prefix, _)| prefix.to_string())
    };
    match (prefix(left), prefix(right)) {
        (Some(a), Some(b)) => a == b,
        _ => Path::new(left).parent() == Path::new(right).parent(),
    }
}
