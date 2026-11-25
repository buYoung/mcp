use super::*;
use crate::parser::{CodeRange, ExtractedFile};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(super) struct Location {
    pub path: String,
    pub range: CodeRange,
}
#[derive(Clone, Debug)]
pub(super) struct TypeNode {
    pub file: usize,
    pub unit: usize,
    pub declaration: usize,
    /// When set, indexes `ImplementationIndex::extra_types` (used for C++ classes recovered from
    /// confirmed export decorator macros) instead of `unit.types[declaration]`.
    pub extra_declaration: Option<usize>,
    pub methods: Vec<usize>,
    /// The file, unit and declaration of each further part of a type declared in parts.
    pub parts: Vec<(usize, usize, usize)>,
}
#[derive(Clone, Debug)]
pub(super) struct ExplicitImpl {
    pub contract: usize,
    pub owner_display: String,
    pub location: Location,
    pub evidence: Vec<Location>,
}
#[derive(Clone, Debug)]
pub(super) struct MethodNode {
    pub owner: usize,
    /// The file and the unit in it that declare the method, which need not be the owner's.
    pub file: usize,
    pub unit: usize,
    pub location: Location,
    pub declaration: MethodDeclaration,
    pub contract: Option<usize>,
}
#[derive(Clone, Debug)]
pub(super) struct TypeSubstitution {
    pub parameter: String,
    pub argument: String,
    pub file: usize,
    pub unit: usize,
    pub owner: usize,
    pub namespace: String,
    pub range: CodeRange,
}
#[derive(Clone, Debug)]
pub(super) struct Parent {
    pub owner: usize,
    pub evidence: Vec<Location>,
    pub is_pointer: bool,
    pub substitutions: Vec<TypeSubstitution>,
    pub conditions: Vec<String>,
}
#[derive(Clone, Debug)]
pub(super) struct Link {
    pub declaration: usize,
    pub implementation: usize,
    pub evidence: Vec<Location>,
    pub is_pointer: bool,
    pub conditions: Vec<String>,
}
#[derive(Clone, Debug)]
pub(super) struct Call {
    pub location: Location,
    pub name: String,
    pub declaration: usize,
    pub evidence: Vec<Location>,
}
/// A typed call whose receiver type and member lookup single out one method that is neither
/// abstract nor overridden, so the static target is also the runtime target.
#[derive(Clone, Debug)]
pub(super) struct DirectCall {
    pub range: CodeRange,
    pub name: String,
    pub method: usize,
    pub evidence: Vec<Location>,
}

/// The method a typed call reaches and every file its proof reads.
pub(crate) struct DirectTarget<'a> {
    pub path: &'a str,
    pub range: &'a CodeRange,
    pub proof_paths: Vec<&'a str>,
}

/// Coordinates refer to the same published symbol generation. Source text is not duplicated.
#[derive(Clone, Debug)]
pub(crate) struct ImplementationIndex {
    pub(super) files: Arc<Vec<Arc<ExtractedFile>>>,
    pub(super) types: Vec<TypeNode>,
    pub(super) extra_types: Vec<TypeDeclaration>,
    pub(super) methods: Vec<MethodNode>,
    pub(super) parents: Vec<Vec<Parent>>,
    pub(super) links: Vec<Link>,
    pub(super) explicit_impls: Vec<ExplicitImpl>,
    pub(super) calls: Vec<Call>,
    pub(super) direct_calls: HashMap<String, Vec<DirectCall>>,
    pub(super) direct_call_count: usize,
    pub(super) by_name: HashMap<String, Vec<usize>>,
    pub(super) methods_by_path: HashMap<String, Vec<usize>>,
    pub(super) calls_by_path: HashMap<String, Vec<usize>>,
    pub(super) explicit_impls_by_contract: HashMap<usize, Vec<usize>>,
    pub(super) explicit_impls_by_path: HashMap<String, Vec<usize>>,
    pub(super) forward: HashMap<usize, Vec<usize>>,
    pub(super) reverse: HashMap<usize, Vec<usize>>,
    pub(super) references: HashMap<usize, Vec<usize>>,
    /// For a declaration, the same-named methods of types that name its owner as a base and
    /// are linked to no declaration: implementations the source suggests but does not prove.
    pub(super) unproven: HashMap<usize, Vec<usize>>,
    pub(super) proof_digests: HashMap<String, String>,
    /// Last `(len, mtime)` observed when a file's bytes matched `proof_digests`, shared across
    /// queries of this snapshot generation to avoid re-reading unchanged files on every request.
    pub(super) verified_stamps:
        Arc<std::sync::RwLock<HashMap<String, (u64, std::time::SystemTime)>>>,
    pub(super) target_os: Option<String>,
    pub(super) omitted: usize,
    pub(super) dart_libraries: super::DartLibraries,
    /// Indexed files by file name, built when the first include is resolved.
    pub(super) files_by_name: std::sync::OnceLock<HashMap<String, Vec<usize>>>,
    /// Indexed files by exact relative path, built lazily for transitive include resolution.
    pub(super) files_by_path: std::sync::OnceLock<HashMap<String, usize>>,
    /// Go `package` names declared in a directory whose basename differs (e.g. `v5/` -> `package chi`).
    pub(super) go_mismatched_packages: HashSet<String>,
    /// C++ template qualified names that have partial or explicit specializations in the
    /// workspace; non-specialized templates are accepted in base resolution.
    pub(super) cpp_specialized_templates: HashSet<String>,
}

impl ImplementationIndex {
    pub(crate) fn build(
        files: Arc<Vec<Arc<ExtractedFile>>>,
        sources: &HashMap<String, String>,
    ) -> Self {
        Self::build_at(
            files,
            sources,
            &crate::workspace::root_dir().unwrap_or_default(),
        )
    }

    /// Builds for a workspace at `root`, where module settings such as tsconfig files live.
    pub(crate) fn build_at(
        files: Arc<Vec<Arc<ExtractedFile>>>,
        sources: &HashMap<String, String>,
        root: &std::path::Path,
    ) -> Self {
        let target_os = crate::config::get().analysis_target_os.clone();
        let mut index = Self {
            files: Arc::clone(&files),
            types: Vec::new(),
            extra_types: Vec::new(),
            methods: Vec::new(),
            parents: Vec::new(),
            links: Vec::new(),
            explicit_impls: Vec::new(),
            calls: Vec::new(),
            direct_calls: HashMap::new(),
            direct_call_count: 0,
            by_name: HashMap::new(),
            methods_by_path: HashMap::new(),
            calls_by_path: HashMap::new(),
            explicit_impls_by_contract: HashMap::new(),
            explicit_impls_by_path: HashMap::new(),
            forward: HashMap::new(),
            reverse: HashMap::new(),
            references: HashMap::new(),
            unproven: HashMap::new(),
            proof_digests: HashMap::new(),
            verified_stamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
            target_os,
            omitted: 0,
            dart_libraries: super::DartLibraries::new(
                files.iter().map(|file| file.file_path.as_str()),
            ),
            files_by_name: std::sync::OnceLock::new(),
            files_by_path: std::sync::OnceLock::new(),
            go_mismatched_packages: HashSet::new(),
            cpp_specialized_templates: HashSet::new(),
        };
        // The first indexed part of each type that is declared in parts.
        let mut partial: HashMap<(&str, &str, usize, String), usize> = HashMap::new();
        // C# project files grouped by folder; multi-csproj folders parse `<Compile Include/Remove>`
        // and `<EnableDefaultCompileItems>` so partial types merge strictly within one project.
        let projects = CsprojProjects::build(&files, sources, root);
        let mut cpp_export_macros: HashMap<&str, Vec<&str>> = HashMap::new();
        for (file_id, file) in files.iter().enumerate() {
            let Some(facts) = file
                .navigation
                .as_ref()
                .and_then(|nav| nav.implementations.as_ref())
            else {
                continue;
            };
            // These facts come from the original source and keep its coordinates after macro
            // expansion. A type on a line the preprocessor dropped belongs to another build
            // configuration, so only types on active lines enter the index.
            let expansion = file.macro_expansion();
            index
                .proof_digests
                .insert(file.file_path.clone(), facts.source_digest.clone());
            index.omitted += facts.omitted;
            for (unit_id, unit) in facts.units.iter().enumerate() {
                if unit.language == "go" && !unit.namespace.is_empty() {
                    let dir_base = std::path::Path::new(&file.file_path)
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    if dir_base != unit.namespace {
                        index.go_mismatched_packages.insert(unit.namespace.clone());
                    }
                }
                if !unit.export_macros.is_empty() {
                    cpp_export_macros
                        .entry(file.file_path.as_str())
                        .or_default()
                        .extend(unit.export_macros.iter().map(String::as_str));
                }
                for specialized in &unit.specialized_templates {
                    index
                        .cpp_specialized_templates
                        .insert(super::resolve::normalized(specialized));
                }
                for (declaration, typ) in unit.types.iter().enumerate() {
                    if expansion.is_some_and(|info| !info.is_active_line(typ.range.start_line)) {
                        continue;
                    }
                    if index.types.len() >= TYPES_PER_SNAPSHOT {
                        index.omitted += 1;
                        continue;
                    }
                    // The parts of one type share its members, so they form one type here.
                    // Parts compile into one assembly: they sit below one project file. A
                    // conditional partial part (such as `JsonReader.Async.cs` inside `#if`)
                    // merges into the same type while its own bases and methods stay gated by
                    // `conditions`, so it never poisons resolution of the unconditional parts.
                    let part = (typ.is_partial && typ.is_complete)
                        .then(|| projects.owner_project(&file.file_path))
                        .flatten()
                        .map(|project| {
                            (
                                family(&unit.language),
                                typ.qualified_name.as_str(),
                                typ.parameters.len(),
                                project,
                            )
                        });
                    if let Some(&first) = part.as_ref().and_then(|part| partial.get(part)) {
                        let primary_conditional = !index.typ(first).conditions.is_empty();
                        if primary_conditional && typ.conditions.is_empty() {
                            let old = (
                                index.types[first].file,
                                index.types[first].unit,
                                index.types[first].declaration,
                            );
                            index.types[first].file = file_id;
                            index.types[first].unit = unit_id;
                            index.types[first].declaration = declaration;
                            index.types[first].parts.push(old);
                        } else {
                            index.types[first]
                                .parts
                                .push((file_id, unit_id, declaration));
                        }
                        for method in &typ.methods {
                            index.add_method(first, method.clone(), (file_id, unit_id), None);
                        }
                        continue;
                    }
                    let id = index.types.len();
                    if let Some(part) = part {
                        partial.insert(part, id);
                    }
                    index.types.push(TypeNode {
                        file: file_id,
                        unit: unit_id,
                        declaration,
                        extra_declaration: None,
                        methods: Vec::new(),
                        parts: Vec::new(),
                    });
                    index.parents.push(Vec::new());
                    if !typ.is_anonymous {
                        index.by_name.entry(typ.name.clone()).or_default().push(id);
                    }
                    for method in &typ.methods {
                        index.add_method(id, method.clone(), (file_id, unit_id), None);
                    }
                }
            }
        }
        // X1: Recover C++ classes/structs decorated by confirmed export macros (e.g.
        // `class LEVELDB_EXPORT Comparator`) proven from `#define` in included headers or the
        // file itself. Never alters ordinary declarations like `struct REAL instance {};`.
        if !cpp_export_macros.is_empty() {
            for (file_id, file) in files.iter().enumerate() {
                let Some(facts) = file
                    .navigation
                    .as_ref()
                    .and_then(|nav| nav.implementations.as_ref())
                else {
                    continue;
                };
                for (unit_id, unit) in facts.units.iter().enumerate() {
                    if unit.language != "cpp" {
                        continue;
                    }
                    let mut confirmed: HashSet<&str> =
                        unit.export_macros.iter().map(String::as_str).collect();
                    let includes = index.cpp_transitive_includes(&file.file_path);
                    for inc_path in includes.keys() {
                        if let Some(macros) = cpp_export_macros.get(inc_path.as_str()) {
                            confirmed.extend(macros.iter().copied());
                        }
                    }
                    if confirmed.is_empty() {
                        continue;
                    }
                    let disk_bytes;
                    let bytes: &[u8] = if let Some(src) = sources.get(&file.file_path) {
                        src.as_bytes()
                    } else if let Ok(raw) = std::fs::read(root.join(&file.file_path)) {
                        disk_bytes = raw;
                        &disk_bytes
                    } else {
                        continue;
                    };
                    let recovered =
                        super::extract::recover_cpp_decorated_types(bytes, unit, &confirmed);
                    for typ in recovered {
                        if index.types.len() >= TYPES_PER_SNAPSHOT {
                            index.omitted += 1;
                            break;
                        }
                        let methods = typ.methods.clone();
                        let name = typ.name.clone();
                        let extra_idx = index.extra_types.len();
                        index.extra_types.push(typ);
                        let id = index.types.len();
                        index.types.push(TypeNode {
                            file: file_id,
                            unit: unit_id,
                            declaration: 0,
                            extra_declaration: Some(extra_idx),
                            methods: Vec::new(),
                            parts: Vec::new(),
                        });
                        index.parents.push(Vec::new());
                        index.by_name.entry(name).or_default().push(id);
                        for method in methods {
                            index.add_method(id, method, (file_id, unit_id), None);
                        }
                    }
                }
            }
        }
        let resolver =
            crate::callers::resolution::SourceResolver::from_stored_sources(&files, root, sources);
        index.connect(&resolver, sources);
        for (id, method) in index.methods.iter().enumerate() {
            index
                .methods_by_path
                .entry(method.location.path.clone())
                .or_default()
                .push(id);
        }
        for (id, call) in index.calls.iter().enumerate() {
            index
                .calls_by_path
                .entry(call.location.path.clone())
                .or_default()
                .push(id);
            index
                .references
                .entry(call.declaration)
                .or_default()
                .push(id);
        }
        for (id, link) in index.links.iter().enumerate() {
            index.forward.entry(link.declaration).or_default().push(id);
            index
                .reverse
                .entry(link.implementation)
                .or_default()
                .push(id);
        }
        for (id, item) in index.explicit_impls.iter().enumerate() {
            index
                .explicit_impls_by_contract
                .entry(item.contract)
                .or_default()
                .push(id);
            index
                .explicit_impls_by_path
                .entry(item.location.path.clone())
                .or_default()
                .push(id);
        }
        index
    }

    /// The single concrete method a typed call at `range` in `path` resolves to.
    pub(crate) fn direct_call(
        &self,
        path: &str,
        range: &CodeRange,
        name: &str,
    ) -> Option<DirectTarget<'_>> {
        let (caller_path, calls) = self.direct_calls.get_key_value(path)?;
        let call = calls
            .iter()
            .find(|call| call.range == *range && call.name == name)?;
        let method = &self.methods[call.method];
        let mut proof_paths: Vec<&str> = call
            .evidence
            .iter()
            .map(|location| location.path.as_str())
            .chain([caller_path.as_str(), method.location.path.as_str()])
            .collect();
        proof_paths.sort_unstable();
        proof_paths.dedup();
        Some(DirectTarget {
            path: &method.location.path,
            range: &method.location.range,
            proof_paths,
        })
    }

    /// Digest of the source generation every recorded fact for `path` came from.
    pub(crate) fn proof_digest(&self, path: &str) -> Option<&str> {
        self.proof_digests.get(path).map(String::as_str)
    }

    /// Whether `root.join(path)` still matches the indexed `proof_digest`. Returns `None` when
    /// `root.join(path)` has no metadata on disk. Reuses a cached `(len, mtime)` match only once
    /// the mtime is at least two seconds old, so rapid edits within the same timestamp tick still
    /// verify the full digest.
    pub(crate) fn is_digest_current_on_disk(
        &self,
        root: &std::path::Path,
        path: &str,
        max_size: Option<u64>,
        byte_budget: Option<(&mut usize, usize)>,
    ) -> Option<bool> {
        let Some(expected) = self.proof_digests.get(path) else {
            return Some(false);
        };
        let disk_path = root.join(path);
        let metadata = std::fs::metadata(&disk_path).ok()?;
        let size = metadata.len();
        if max_size.is_some_and(|cap| size > cap) {
            return Some(false);
        }
        let modified = metadata.modified().ok();
        if let Some(modified) = modified {
            let is_racy_clean = std::time::SystemTime::now()
                .duration_since(modified)
                .is_ok_and(|age| age >= std::time::Duration::from_secs(2));
            if is_racy_clean
                && self
                    .verified_stamps
                    .read()
                    .ok()
                    .and_then(|stamps| stamps.get(path).copied())
                    == Some((size, modified))
            {
                return Some(true);
            }
        }
        if let Some((used, cap)) = byte_budget {
            if used.saturating_add(size as usize) > cap {
                return Some(false);
            }
            *used += size as usize;
        }
        let Ok(bytes) = std::fs::read(&disk_path) else {
            return Some(false);
        };
        let is_match = digest(&bytes) == *expected;
        if is_match {
            if let Some(modified) = modified {
                if let Ok(mut stamps) = self.verified_stamps.write() {
                    stamps.insert(path.to_string(), (size, modified));
                }
            }
        }
        Some(is_match)
    }

    pub(super) fn unit(&self, id: usize) -> &ImplementationUnit {
        let node = &self.types[id];
        &self.files[node.file]
            .navigation
            .as_ref()
            .unwrap()
            .implementations
            .as_ref()
            .unwrap()
            .units[node.unit]
    }
    /// The unit that holds the declaration of `method`.
    pub(super) fn method_unit(&self, method: usize) -> &ImplementationUnit {
        let node = &self.methods[method];
        let facts = self.files[node.file].navigation.as_ref().unwrap();
        &facts.implementations.as_ref().unwrap().units[node.unit]
    }
    pub(super) fn typ(&self, id: usize) -> &TypeDeclaration {
        let node = &self.types[id];
        if let Some(extra) = node.extra_declaration {
            &self.extra_types[extra]
        } else {
            &self.unit(id).types[node.declaration]
        }
    }
    /// Every declaration of the type with the file and unit that hold it: one, or each part
    /// of a type declared in parts.
    pub(super) fn declarations(
        &self,
        id: usize,
    ) -> impl Iterator<Item = (usize, usize, &TypeDeclaration)> {
        let node = &self.types[id];
        std::iter::once((node.file, node.unit, self.typ(id))).chain(node.parts.iter().copied().map(
            |(file, unit, declaration)| {
                let facts = self.files[file].navigation.as_ref().unwrap();
                let unit_facts = &facts.implementations.as_ref().unwrap().units[unit];
                (file, unit, &unit_facts.types[declaration])
            },
        ))
    }
    /// Where the declaration that holds `method` starts: its type, or the part of the type
    /// declared around it.
    pub(super) fn declaring(&self, method: usize) -> Location {
        let node = &self.methods[method];
        self.declarations(node.owner)
            .skip(1)
            .find(|(file, unit, part)| {
                (*file, *unit) == (node.file, node.unit)
                    && super::resolve::encloses(&part.range, &node.location.range)
            })
            .map_or_else(
                || self.location(node.owner),
                |(file, _, part)| Location {
                    path: self.files[file].file_path.clone(),
                    range: part.range.clone(),
                },
            )
    }
    pub(super) fn location(&self, id: usize) -> Location {
        Location {
            path: self.files[self.types[id].file].file_path.clone(),
            range: self.typ(id).range.clone(),
        }
    }
    pub(super) fn add_method(
        &mut self,
        owner: usize,
        declaration: MethodDeclaration,
        (file, unit): (usize, usize),
        contract: Option<usize>,
    ) {
        if self.methods.len() >= METHODS_PER_SNAPSHOT {
            self.omitted += 1;
            return;
        }
        let id = self.methods.len();
        self.methods.push(MethodNode {
            owner,
            file,
            unit,
            location: Location {
                path: self.files[file].file_path.clone(),
                range: declaration.range.clone(),
            },
            declaration,
            contract,
        });
        self.types[owner].methods.push(id);
    }
}

/// The folder of `path`, empty at the workspace root.
fn folder(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

struct CsprojInfo {
    path: String,
    enable_default_compile: bool,
    includes: Vec<String>,
    removes: Vec<String>,
}

impl CsprojInfo {
    fn parse(path: String, xml: &str) -> Self {
        let lower = xml.to_ascii_lowercase();
        let is_sdk = lower
            .find("<project")
            .and_then(|start| {
                lower[start..]
                    .find('>')
                    .map(|end| &lower[start..start + end])
            })
            .is_some_and(|tag| tag.contains("sdk="));
        let explicit_default = extract_xml_tag_value(&lower, "enabledefaultcompileitems")
            .map(|val| val.trim() != "false");
        let mut includes = Vec::new();
        let mut removes = Vec::new();
        let mut cursor = xml;
        while let Some(pos) = find_ascii_ci(cursor, "<compile") {
            let rest = &cursor[pos + "<compile".len()..];
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = &rest[..end];
            // Require whitespace or `/` immediately after `<Compile` so `<CompileSomething>` is ignored.
            if tag.starts_with(|c: char| c.is_ascii_whitespace() || c == '/') {
                if let Some(val) = extract_xml_attr(tag, "include") {
                    for item in val.split(';') {
                        let norm = normalize_msbuild_path(item);
                        if !norm.is_empty() {
                            includes.push(norm);
                        }
                    }
                }
                if let Some(val) = extract_xml_attr(tag, "remove") {
                    for item in val.split(';') {
                        let norm = normalize_msbuild_path(item);
                        if !norm.is_empty() {
                            removes.push(norm);
                        }
                    }
                }
            }
            cursor = &rest[end + 1..];
        }
        let enable_default_compile = explicit_default.unwrap_or(is_sdk || includes.is_empty());
        Self {
            path,
            enable_default_compile,
            includes,
            removes,
        }
    }

    fn owns_relative(&self, rel: &str) -> bool {
        let included = self.enable_default_compile
            || self
                .includes
                .iter()
                .any(|pattern| msbuild_glob_matches(pattern, rel));
        included
            && !self
                .removes
                .iter()
                .any(|pattern| msbuild_glob_matches(pattern, rel))
    }
}

struct CsprojProjects {
    /// Folder -> either a single `.csproj` path or parsed rules for each `.csproj` in that folder.
    by_folder: HashMap<String, Vec<CsprojInfo>>,
}

impl CsprojProjects {
    fn build(
        files: &[Arc<ExtractedFile>],
        sources: &HashMap<String, String>,
        root: &std::path::Path,
    ) -> Self {
        let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
        let mut seen: HashSet<&str> = HashSet::new();
        for path in files
            .iter()
            .map(|f| f.file_path.as_str())
            .chain(sources.keys().map(String::as_str))
        {
            if path.ends_with(".csproj") && seen.insert(path) {
                grouped
                    .entry(folder(path).to_string())
                    .or_default()
                    .push(path.to_string());
            }
        }
        let mut by_folder = HashMap::with_capacity(grouped.len());
        for (dir, mut paths) in grouped {
            paths.sort();
            if paths.len() == 1 {
                by_folder.insert(
                    dir,
                    vec![CsprojInfo {
                        path: paths.pop().unwrap(),
                        enable_default_compile: true,
                        includes: Vec::new(),
                        removes: Vec::new(),
                    }],
                );
                continue;
            }
            let infos = paths
                .into_iter()
                .map(|path| {
                    let xml = sources
                        .get(&path)
                        .cloned()
                        .or_else(|| std::fs::read_to_string(root.join(&path)).ok())
                        .unwrap_or_default();
                    CsprojInfo::parse(path, &xml)
                })
                .collect();
            by_folder.insert(dir, infos);
        }
        Self { by_folder }
    }

    /// The owning `.csproj` identity for `path`, or its own folder when no `.csproj` exists above
    /// it. In folders with multiple `.csproj` files, parses `<EnableDefaultCompileItems>` and
    /// `<Compile Include/Remove>` to assign `path` only when a single project claims it.
    fn owner_project(&self, path: &str) -> Option<String> {
        let own = folder(path);
        let mut current = own;
        loop {
            if let Some(projects) = self.by_folder.get(current) {
                if projects.len() == 1 {
                    return Some(projects[0].path.clone());
                }
                let rel = if current.is_empty() {
                    path
                } else {
                    path.strip_prefix(current)
                        .and_then(|rest| rest.strip_prefix('/'))
                        .unwrap_or(path)
                };
                let mut owners = projects.iter().filter(|proj| proj.owns_relative(rel));
                let first = owners.next()?;
                return owners.next().is_none().then(|| first.path.clone());
            }
            match current.rsplit_once('/') {
                Some((parent, _)) => current = parent,
                None if current.is_empty() => return Some(own.to_string()),
                None => current = "",
            }
        }
    }
}

fn find_ascii_ci(haystack: &str, needle_lower: &str) -> Option<usize> {
    let h = haystack.as_bytes();
    let n = needle_lower.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| {
        h[i..i + n.len()]
            .iter()
            .zip(n)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

fn extract_xml_tag_value<'a>(lower: &'a str, tag_lower: &str) -> Option<&'a str> {
    let open = format!("<{tag_lower}>");
    let close = format!("</{tag_lower}>");
    let start = lower.find(&open)? + open.len();
    let end = lower[start..].find(&close)?;
    Some(&lower[start..start + end])
}

fn extract_xml_attr<'a>(tag: &'a str, attr_lower: &str) -> Option<&'a str> {
    let mut cursor = tag;
    while let Some(pos) = find_ascii_ci(cursor, attr_lower) {
        let before_ok = pos == 0 || cursor.as_bytes()[pos - 1].is_ascii_whitespace();
        let after = &cursor[pos + attr_lower.len()..];
        if before_ok {
            let trimmed = after.trim_start();
            if let Some(after_eq) = trimmed.strip_prefix('=') {
                let val_start = after_eq.trim_start();
                let quote = val_start.chars().next()?;
                if quote == '"' || quote == '\'' {
                    let inner = &val_start[1..];
                    if let Some(end) = inner.find(quote) {
                        return Some(&inner[..end]);
                    }
                }
            }
        }
        cursor = &cursor[pos + 1..];
    }
    None
}

fn normalize_msbuild_path(raw: &str) -> String {
    let mut s = raw.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    s
}

fn msbuild_glob_matches(pattern: &str, path: &str) -> bool {
    let p: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let s: Vec<char> = path.to_ascii_lowercase().chars().collect();
    glob_match_slice(&p, &s)
}

fn glob_match_slice(p: &[char], s: &[char]) -> bool {
    let (mut pi, mut si) = (0, 0);
    while pi < p.len() {
        if p[pi] == '*' {
            if pi + 1 < p.len() && p[pi + 1] == '*' {
                let mut next_pi = pi + 2;
                if next_pi < p.len() && p[next_pi] == '/' {
                    next_pi += 1;
                }
                for skip in si..=s.len() {
                    if glob_match_slice(&p[next_pi..], &s[skip..]) {
                        return true;
                    }
                }
                return false;
            }
            for skip in si..=s.len() {
                if skip > si && s[skip - 1] == '/' {
                    break;
                }
                if glob_match_slice(&p[pi + 1..], &s[skip..]) {
                    return true;
                }
            }
            return false;
        }
        if si >= s.len() || (p[pi] != '?' && p[pi] != s[si]) {
            return false;
        }
        pi += 1;
        si += 1;
    }
    si == s.len()
}
