use super::*;
use crate::callers::resolution::SourceResolver;
use crate::parser::CodeRange;
use std::collections::{HashMap, HashSet};

fn active(
    language: &str,
    path: &str,
    range: &CodeRange,
    conditions: &[String],
    resolver: &SourceResolver<'_>,
) -> bool {
    if language == "rust" {
        resolver.condition_at(path, range) == Some(true)
    } else {
        conditions.is_empty()
    }
}

impl ImplementationIndex {
    pub(super) fn connect(
        &mut self,
        resolver: &SourceResolver<'_>,
        sources: &HashMap<String, String>,
    ) {
        for id in 0..self.types.len() {
            let node = self.types[id].clone();
            let typ = self.typ(id).clone();
            let language = self.unit(id).language.clone();
            let path = self.files[node.file].file_path.clone();
            if !typ.is_complete
                || !typ.parameters.is_empty()
                || !active(&language, &path, &typ.range, &typ.conditions, resolver)
            {
                continue;
            }
            for base in &typ.bases {
                if let Some((parent, mut evidence)) =
                    self.resolve_type(node.file, node.unit, &typ.namespace, base, resolver)
                {
                    if parent == id || self.typ(parent).is_final {
                        continue;
                    }
                    evidence.push(Location {
                        path: path.clone(),
                        range: base.range.clone(),
                    });
                    evidence.push(self.location(id));
                    self.remember_proof(&evidence, sources);
                    self.parents[id].push(Parent {
                        owner: parent,
                        evidence,
                        is_pointer: false,
                    });
                }
            }
        }
        // Detached implementations are attached only after resolving the owner and
        // (for Rust) the exact trait. An inherent method cannot satisfy another impl.
        let files = std::sync::Arc::clone(&self.files);
        for (file_id, file) in files.iter().enumerate() {
            let Some(facts) = file
                .navigation
                .as_ref()
                .and_then(|nav| nav.implementations.as_ref())
            else {
                continue;
            };
            for (unit_id, unit) in facts.units.iter().enumerate() {
                for block in &unit.implementations {
                    if !active(
                        &unit.language,
                        &file.file_path,
                        &block.owner.range,
                        &block.conditions,
                        resolver,
                    ) {
                        continue;
                    }
                    let Some((owner, mut proof)) = self.resolve_type(
                        file_id,
                        unit_id,
                        &block.namespace,
                        &block.owner,
                        resolver,
                    ) else {
                        continue;
                    };
                    let contract = if block.contracts.is_empty() {
                        None
                    } else {
                        if block.contracts.len() != 1 {
                            continue;
                        }
                        let Some((contract, evidence)) = self.resolve_type(
                            file_id,
                            unit_id,
                            &block.namespace,
                            &block.contracts[0],
                            resolver,
                        ) else {
                            continue;
                        };
                        proof.extend(evidence);
                        Some(contract)
                    };
                    proof.push(Location {
                        path: file.file_path.clone(),
                        range: block.owner.range.clone(),
                    });
                    self.remember_proof(&proof, sources);
                    if let Some(contract) = contract {
                        self.parents[owner].push(Parent {
                            owner: contract,
                            evidence: proof,
                            is_pointer: false,
                        });
                    }
                    for method in &block.methods {
                        if active(
                            &unit.language,
                            &file.file_path,
                            &method.range,
                            &method.conditions,
                            resolver,
                        ) {
                            self.add_method(owner, method.clone(), &file.file_path, contract);
                        }
                    }
                }
            }
        }
        self.connect_go();
        for owner in 0..self.types.len() {
            let ancestors = self.ancestors(owner);
            let own_methods = self.types[owner].methods.clone();
            for method in own_methods {
                let implementation = &self.methods[method];
                if implementation.declaration.is_abstract {
                    continue;
                }
                for parent in &ancestors {
                    if implementation
                        .contract
                        .is_some_and(|contract| contract != parent.owner)
                    {
                        continue;
                    }
                    if self.unit(owner).language == "rust" && implementation.contract.is_none() {
                        continue;
                    }
                    let candidates = self.types[parent.owner]
                        .methods
                        .iter()
                        .copied()
                        .filter(|base| self.matches(*base, method))
                        .collect::<Vec<_>>();
                    if candidates.len() != 1 {
                        continue;
                    }
                    let base = candidates[0];
                    if !active(
                        &self.unit(parent.owner).language,
                        &self.methods[base].location.path,
                        &self.methods[base].declaration.range,
                        &self.methods[base].declaration.conditions,
                        resolver,
                    ) || !active(
                        &self.unit(owner).language,
                        &implementation.location.path,
                        &implementation.declaration.range,
                        &implementation.declaration.conditions,
                        resolver,
                    ) {
                        continue;
                    }
                    if self.links.len() >= METHODS_PER_SNAPSHOT * 2 {
                        self.omitted += 1;
                        break;
                    }
                    self.links.push(Link {
                        declaration: base,
                        implementation: method,
                        evidence: parent.evidence.clone(),
                        is_pointer: parent.is_pointer
                            || implementation.declaration.is_pointer_receiver,
                    });
                }
            }
        }
        self.links
            .sort_by_key(|link| (link.declaration, link.implementation));
        self.links
            .dedup_by_key(|link| (link.declaration, link.implementation));
        let declarations = self
            .links
            .iter()
            .map(|link| link.declaration)
            .collect::<HashSet<_>>();
        for (file_id, file) in files.iter().enumerate() {
            let Some(facts) = file
                .navigation
                .as_ref()
                .and_then(|nav| nav.implementations.as_ref())
            else {
                continue;
            };
            for (unit_id, unit) in facts.units.iter().enumerate() {
                for call in &unit.calls {
                    if self.calls.len() >= METHODS_PER_SNAPSHOT {
                        self.omitted += 1;
                        break;
                    }
                    if !active(
                        &unit.language,
                        &file.file_path,
                        &call.range,
                        &call.conditions,
                        resolver,
                    ) {
                        continue;
                    }
                    let Some((owner, mut evidence)) = self.resolve_type(
                        file_id,
                        unit_id,
                        &call.namespace,
                        &call.receiver,
                        resolver,
                    ) else {
                        continue;
                    };
                    let mut candidates = self.call_methods(owner, &call.name, call.argument_count);
                    if candidates.is_empty() {
                        for parent in self.ancestors(owner) {
                            candidates.extend(self.call_methods(
                                parent.owner,
                                &call.name,
                                call.argument_count,
                            ));
                            evidence.extend(parent.evidence);
                        }
                    }
                    candidates.sort_unstable();
                    candidates.dedup();
                    if candidates.len() != 1 {
                        continue;
                    }
                    let declaration = candidates[0];
                    // Ordinary concrete methods remain in direct-call navigation.
                    if !self.methods[declaration].declaration.is_abstract
                        && !declarations.contains(&declaration)
                    {
                        continue;
                    }
                    evidence.push(self.location(owner));
                    self.remember_proof(&evidence, sources);
                    self.calls.push(Call {
                        location: Location {
                            path: file.file_path.clone(),
                            range: call.range.clone(),
                        },
                        name: call
                            .enclosing_symbol
                            .clone()
                            .unwrap_or_else(|| call.name.clone()),
                        declaration,
                        evidence,
                    });
                }
            }
        }
    }

    fn call_methods(&self, owner: usize, name: &str, count: Option<usize>) -> Vec<usize> {
        self.types[owner]
            .methods
            .iter()
            .copied()
            .filter(|id| {
                let method = &self.methods[*id].declaration;
                method.name == name
                    && !method.is_static
                    && !method.is_private
                    && count.is_none_or(|count| method.parameters.len() == count)
            })
            .collect()
    }

    pub(super) fn ancestors(&self, owner: usize) -> Vec<Parent> {
        let mut pending = self.parents[owner]
            .iter()
            .cloned()
            .map(|parent| (parent, vec![owner]))
            .collect::<Vec<_>>();
        let mut result = Vec::new();
        while let Some((parent, mut visited)) = pending.pop() {
            if visited.contains(&parent.owner) {
                return Vec::new();
            }
            if visited.len() > INHERITANCE_DEPTH || result.len() >= CANDIDATES_PER_QUERY {
                continue;
            }
            visited.push(parent.owner);
            for next in &self.parents[parent.owner] {
                let mut evidence = parent.evidence.clone();
                evidence.extend(next.evidence.clone());
                pending.push((
                    Parent {
                        owner: next.owner,
                        evidence,
                        is_pointer: parent.is_pointer || next.is_pointer,
                    },
                    visited.clone(),
                ));
            }
            result.push(parent);
        }
        result
    }

    fn matches(&self, base: usize, implementation: usize) -> bool {
        let a = &self.methods[base];
        let b = &self.methods[implementation];
        let language = self.unit(a.owner).language.as_str();
        let x = &a.declaration;
        let y = &b.declaration;
        if x.name != y.name
            || x.is_private
            || y.is_private
            || x.is_final
            || !x.is_virtual
            || x.is_static != y.is_static
            || x.is_static && language != "rust"
        {
            return false;
        }
        if matches!(language, "javascript" | "python" | "ruby") {
            return true;
        }
        if !x.has_known_signature
            || !y.has_known_signature
            || x.parameters != y.parameters
            || x.qualifiers != y.qualifiers
        {
            return false;
        }
        if language == "go" && x.return_type != y.return_type {
            return false;
        }
        // Same spelling of a custom parameter type in two different source scopes
        // is insufficient identity evidence. Built-ins do not have that ambiguity.
        if (a.location.path != b.location.path
            || self.typ(a.owner).namespace != self.typ(b.owner).namespace)
            && x.parameters
                .iter()
                .chain((language == "go").then_some(&x.return_type))
                .any(|typ| !builtin_signature(typ))
        {
            return false;
        }
        true
    }

    fn connect_go(&mut self) {
        let mut by_method: HashMap<(String, String), HashSet<usize>> = HashMap::new();
        for (owner, node) in self.types.iter().enumerate() {
            if self.unit(owner).language != "go"
                || self.typ(owner).is_contract
                || !self.typ(owner).is_complete
            {
                continue;
            }
            let package = std::path::Path::new(&self.files[node.file].file_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy()
                .to_string();
            for method in &node.methods {
                by_method
                    .entry((
                        package.clone(),
                        self.methods[*method].declaration.name.clone(),
                    ))
                    .or_default()
                    .insert(owner);
            }
        }
        for contract in 0..self.types.len() {
            let typ = self.typ(contract);
            if self.unit(contract).language != "go"
                || !typ.is_contract
                || !typ.is_complete
                || self.types[contract].methods.is_empty()
                || self.types[contract].methods.len() > 128
            {
                continue;
            }
            let package = std::path::Path::new(&self.files[self.types[contract].file].file_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy()
                .to_string();
            let required = &self.types[contract].methods;
            let mut candidates = required
                .iter()
                .map(|id| {
                    by_method.get(&(package.clone(), self.methods[*id].declaration.name.clone()))
                })
                .collect::<Vec<_>>();
            candidates.sort_by_key(|set| set.map_or(0, HashSet::len));
            let Some(Some(candidates)) = candidates.first() else {
                continue;
            };
            let mut owners = candidates.iter().copied().collect::<Vec<_>>();
            owners.sort_unstable();
            for owner in owners.into_iter().take(CANDIDATES_PER_QUERY) {
                if self.unit(owner).namespace != self.unit(contract).namespace {
                    continue;
                }
                if required.iter().all(|base| {
                    self.types[owner]
                        .methods
                        .iter()
                        .filter(|method| self.matches(*base, **method))
                        .count()
                        == 1
                }) {
                    let mut evidence = vec![self.location(contract), self.location(owner)];
                    for base in required {
                        evidence.push(self.methods[*base].location.clone());
                        for method in &self.types[owner].methods {
                            if self.matches(*base, *method) {
                                evidence.push(self.methods[*method].location.clone());
                            }
                        }
                    }
                    let is_pointer = required.iter().any(|base| {
                        self.types[owner].methods.iter().any(|method| {
                            self.matches(*base, *method)
                                && self.methods[*method].declaration.is_pointer_receiver
                        })
                    });
                    self.parents[owner].push(Parent {
                        owner: contract,
                        evidence,
                        is_pointer,
                    });
                }
            }
        }
    }
}

fn builtin_signature(value: &str) -> bool {
    value
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty())
        .all(|part| {
            matches!(
                part,
                "void"
                    | "bool"
                    | "boolean"
                    | "byte"
                    | "char"
                    | "short"
                    | "int"
                    | "long"
                    | "float"
                    | "double"
                    | "number"
                    | "string"
                    | "str"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "usize"
                    | "isize"
                    | "uint"
                    | "uint8"
                    | "uint16"
                    | "uint32"
                    | "uint64"
                    | "int8"
                    | "int16"
                    | "int32"
                    | "int64"
                    | "float32"
                    | "float64"
                    | "complex64"
                    | "complex128"
                    | "rune"
                    | "error"
            )
        })
}
