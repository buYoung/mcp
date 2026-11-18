use super::*;
use crate::parser::{CodeRange, ExtractedFile};
use std::collections::HashMap;
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
    pub methods: Vec<usize>,
    /// The file, unit and declaration of each further part of a type declared in parts.
    pub parts: Vec<(usize, usize, usize)>,
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
pub(super) struct Parent {
    pub owner: usize,
    pub evidence: Vec<Location>,
    pub is_pointer: bool,
}
#[derive(Clone, Debug)]
pub(super) struct Link {
    pub declaration: usize,
    pub implementation: usize,
    pub evidence: Vec<Location>,
    pub is_pointer: bool,
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
    pub(super) methods: Vec<MethodNode>,
    pub(super) parents: Vec<Vec<Parent>>,
    pub(super) links: Vec<Link>,
    pub(super) calls: Vec<Call>,
    pub(super) direct_calls: HashMap<String, Vec<DirectCall>>,
    pub(super) direct_call_count: usize,
    pub(super) by_name: HashMap<String, Vec<usize>>,
    pub(super) methods_by_path: HashMap<String, Vec<usize>>,
    pub(super) calls_by_path: HashMap<String, Vec<usize>>,
    pub(super) forward: HashMap<usize, Vec<usize>>,
    pub(super) reverse: HashMap<usize, Vec<usize>>,
    pub(super) references: HashMap<usize, Vec<usize>>,
    /// For a declaration, the same-named methods of types that name its owner as a base and
    /// are linked to no declaration: implementations the source suggests but does not prove.
    pub(super) unproven: HashMap<usize, Vec<usize>>,
    pub(super) proof_digests: HashMap<String, String>,
    pub(super) target_os: Option<String>,
    pub(super) omitted: usize,
    pub(super) dart_libraries: super::DartLibraries,
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
            methods: Vec::new(),
            parents: Vec::new(),
            links: Vec::new(),
            calls: Vec::new(),
            direct_calls: HashMap::new(),
            direct_call_count: 0,
            by_name: HashMap::new(),
            methods_by_path: HashMap::new(),
            calls_by_path: HashMap::new(),
            forward: HashMap::new(),
            reverse: HashMap::new(),
            references: HashMap::new(),
            unproven: HashMap::new(),
            proof_digests: HashMap::new(),
            target_os,
            omitted: 0,
            dart_libraries: super::DartLibraries::new(
                files.iter().map(|file| file.file_path.as_str()),
            ),
        };
        // The first indexed part of each type that is declared in parts.
        let mut partial: HashMap<(&str, &str, usize, &str), usize> = HashMap::new();
        let projects: std::collections::HashSet<&str> = files
            .iter()
            .filter(|file| file.file_path.ends_with(".csproj"))
            .map(|file| folder(&file.file_path))
            .collect();
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
                    // conditional part stays apart, as every conditional declaration does.
                    let part = (typ.is_partial && typ.is_complete && typ.conditions.is_empty())
                        .then(|| {
                            (
                                family(&unit.language),
                                typ.qualified_name.as_str(),
                                typ.parameters.len(),
                                project_folder(&projects, &file.file_path),
                            )
                        });
                    if let Some(&first) = part.and_then(|part| partial.get(&part)) {
                        index.types[first]
                            .parts
                            .push((file_id, unit_id, declaration));
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
                        methods: Vec::new(),
                        parts: Vec::new(),
                    });
                    index.parents.push(Vec::new());
                    index.by_name.entry(typ.name.clone()).or_default().push(id);
                    for method in &typ.methods {
                        index.add_method(id, method.clone(), (file_id, unit_id), None);
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
        &self.unit(id).types[self.types[id].declaration]
    }
    /// Every declaration of the type with the file and unit that hold it: one, or each part
    /// of a type declared in parts.
    pub(super) fn declarations(
        &self,
        id: usize,
    ) -> impl Iterator<Item = (usize, usize, &TypeDeclaration)> {
        let node = &self.types[id];
        std::iter::once((node.file, node.unit, node.declaration))
            .chain(node.parts.iter().copied())
            .map(|(file, unit, declaration)| {
                let facts = self.files[file].navigation.as_ref().unwrap();
                let unit_facts = &facts.implementations.as_ref().unwrap().units[unit];
                (file, unit, &unit_facts.types[declaration])
            })
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

/// The folder whose sources compile together with `path`: that of the nearest project file
/// above it, else its own.
fn project_folder<'p>(projects: &std::collections::HashSet<&str>, path: &'p str) -> &'p str {
    let own = folder(path);
    let mut current = own;
    loop {
        if projects.contains(current) {
            return current;
        }
        match current.rsplit_once('/') {
            Some((parent, _)) => current = parent,
            None if current.is_empty() => return own,
            None => current = "",
        }
    }
}
