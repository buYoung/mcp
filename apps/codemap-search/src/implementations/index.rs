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
}
#[derive(Clone, Debug)]
pub(super) struct MethodNode {
    pub owner: usize,
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

/// Coordinates refer to the same published symbol generation. Source text is not duplicated.
#[derive(Clone, Debug)]
pub(crate) struct ImplementationIndex {
    pub(super) files: Arc<Vec<ExtractedFile>>,
    pub(super) types: Vec<TypeNode>,
    pub(super) methods: Vec<MethodNode>,
    pub(super) parents: Vec<Vec<Parent>>,
    pub(super) links: Vec<Link>,
    pub(super) calls: Vec<Call>,
    pub(super) by_name: HashMap<String, Vec<usize>>,
    pub(super) methods_by_path: HashMap<String, Vec<usize>>,
    pub(super) calls_by_path: HashMap<String, Vec<usize>>,
    pub(super) forward: HashMap<usize, Vec<usize>>,
    pub(super) reverse: HashMap<usize, Vec<usize>>,
    pub(super) references: HashMap<usize, Vec<usize>>,
    pub(super) proof_digests: HashMap<String, String>,
    pub(super) target_os: Option<String>,
    pub(super) omitted: usize,
}

impl ImplementationIndex {
    pub(crate) fn build(files: Arc<Vec<ExtractedFile>>, sources: &HashMap<String, String>) -> Self {
        let root = std::env::current_dir().unwrap_or_default();
        let target_os = crate::config::get().analysis_target_os.clone();
        let mut index = Self {
            files: Arc::clone(&files),
            types: Vec::new(),
            methods: Vec::new(),
            parents: Vec::new(),
            links: Vec::new(),
            calls: Vec::new(),
            by_name: HashMap::new(),
            methods_by_path: HashMap::new(),
            calls_by_path: HashMap::new(),
            forward: HashMap::new(),
            reverse: HashMap::new(),
            references: HashMap::new(),
            proof_digests: HashMap::new(),
            target_os,
            omitted: 0,
        };
        for (file_id, file) in files.iter().enumerate() {
            let Some(facts) = file
                .navigation
                .as_ref()
                .and_then(|nav| nav.implementations.as_ref())
            else {
                continue;
            };
            // Preprocessed ranges require a complete type-evidence source map. Original
            // declarations remain available through ordinary symbol navigation.
            if file
                .navigation
                .as_ref()
                .is_some_and(|nav| nav.macro_expansion.is_some())
            {
                continue;
            }
            index
                .proof_digests
                .insert(file.file_path.clone(), facts.source_digest.clone());
            index.omitted += facts.omitted;
            for (unit_id, unit) in facts.units.iter().enumerate() {
                for (declaration, typ) in unit.types.iter().enumerate() {
                    if index.types.len() >= TYPES_PER_SNAPSHOT {
                        index.omitted += 1;
                        continue;
                    }
                    let id = index.types.len();
                    index.types.push(TypeNode {
                        file: file_id,
                        unit: unit_id,
                        declaration,
                        methods: Vec::new(),
                    });
                    index.parents.push(Vec::new());
                    index.by_name.entry(typ.name.clone()).or_default().push(id);
                    for method in &typ.methods {
                        index.add_method(id, method.clone(), &file.file_path, None);
                    }
                }
            }
        }
        let resolver =
            crate::callers::resolution::SourceResolver::from_stored_sources(&files, &root, sources);
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
    pub(super) fn typ(&self, id: usize) -> &TypeDeclaration {
        &self.unit(id).types[self.types[id].declaration]
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
        path: &str,
        contract: Option<usize>,
    ) {
        if self.methods.len() >= METHODS_PER_SNAPSHOT {
            self.omitted += 1;
            return;
        }
        let id = self.methods.len();
        self.methods.push(MethodNode {
            owner,
            location: Location {
                path: path.into(),
                range: declaration.range.clone(),
            },
            declaration,
            contract,
        });
        self.types[owner].methods.push(id);
    }
}
