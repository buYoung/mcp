use super::signature::Signatures;
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
            let language = self.unit(id).language.clone();
            let declarations: Vec<_> = self
                .declarations(id)
                .map(|(file, unit, typ)| (file, unit, typ.clone()))
                .collect();
            for (file, unit, typ) in declarations {
                let path = self.files[file].file_path.clone();
                // A type that the language specializes by its arguments has no bases of its own.
                if !super::resolve::accepts(&language, &typ, typ.parameters.len())
                    || !active(&language, &path, &typ.range, &typ.conditions, resolver)
                {
                    continue;
                }
                for base in &typ.bases {
                    if let Some((parent, mut evidence)) =
                        self.resolve_type(file, unit, &typ.namespace, base, resolver)
                    {
                        if parent == id || self.typ(parent).is_final {
                            continue;
                        }
                        evidence.push(Location {
                            path: path.clone(),
                            range: base.range.clone(),
                        });
                        evidence.push(Location {
                            path: path.clone(),
                            range: typ.range.clone(),
                        });
                        self.remember_proof(&evidence, sources);
                        self.parents[id].push(Parent {
                            owner: parent,
                            evidence,
                            is_pointer: false,
                        });
                    }
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
                            self.add_method(owner, method.clone(), (file_id, unit_id), contract);
                        }
                    }
                }
            }
        }
        let signatures = Signatures::new(resolver);
        self.connect_go(&signatures, sources);
        // Declarations read to accept a signature, remembered once the links are in place.
        let mut proofs = Vec::new();
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
                        .filter(|base| self.matches(*base, method, &signatures))
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
                    let is_pointer =
                        parent.is_pointer || implementation.declaration.is_pointer_receiver;
                    let mut proof = self.signature_evidence(base, &signatures);
                    proof.extend(self.signature_evidence(method, &signatures));
                    let mut evidence = parent.evidence.clone();
                    evidence.extend(proof.iter().cloned());
                    proofs.extend(proof);
                    self.links.push(Link {
                        declaration: base,
                        implementation: method,
                        evidence,
                        is_pointer,
                    });
                }
            }
        }
        self.remember_proof(&proofs, sources);
        self.links
            .sort_by_key(|link| (link.declaration, link.implementation));
        self.links
            .dedup_by_key(|link| (link.declaration, link.implementation));
        self.record_unproven();
        let declarations = self
            .links
            .iter()
            .map(|link| link.declaration)
            .collect::<HashSet<_>>();
        let replaceable = self.replaceable_methods(&declarations);
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
                    self.record_direct_call(
                        owner,
                        &file.file_path,
                        call,
                        &evidence,
                        &replaceable,
                        sources,
                    );
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

    /// Methods a runtime receiver may replace: abstract ones, linked declarations and any
    /// method a subtype redeclares under the same name, whatever its signature.
    fn replaceable_methods(&self, declarations: &HashSet<usize>) -> HashSet<usize> {
        let mut replaceable = declarations.clone();
        for owner in 0..self.types.len() {
            if self.types[owner].methods.is_empty() {
                continue;
            }
            let names = self.types[owner]
                .methods
                .iter()
                .map(|id| self.methods[*id].declaration.name.as_str())
                .collect::<HashSet<_>>();
            for parent in self.ancestors(owner) {
                replaceable.extend(
                    self.types[parent.owner]
                        .methods
                        .iter()
                        .copied()
                        .filter(|base| {
                            names.contains(self.methods[*base].declaration.name.as_str())
                        }),
                );
            }
        }
        replaceable
            .extend((0..self.methods.len()).filter(|id| self.methods[*id].declaration.is_abstract));
        replaceable
    }

    /// Remember a typed call whose member lookup singles out a method no runtime receiver can
    /// replace, for caller attribution beyond implementation navigation.
    fn record_direct_call(
        &mut self,
        owner: usize,
        path: &str,
        call: &DispatchCall,
        type_evidence: &[Location],
        replaceable: &HashSet<usize>,
        sources: &HashMap<String, String>,
    ) {
        if self.direct_call_count >= DIRECT_CALLS_PER_SNAPSHOT {
            self.omitted += 1;
            return;
        }
        let Some((method, mut evidence)) = self.direct_method(owner, path, call) else {
            return;
        };
        // Static, `super` and private member calls name the member that runs.
        let is_fixed = call.receiver_kind != ReceiverKind::Value
            || self.methods[method].declaration.is_private;
        if !is_fixed && replaceable.contains(&method) {
            return;
        }
        evidence.extend(type_evidence.iter().cloned());
        evidence.push(self.location(owner));
        self.remember_proof(&evidence, sources);
        self.direct_call_count += 1;
        self.direct_calls
            .entry(path.to_string())
            .or_default()
            .push(DirectCall {
                range: call.range.clone(),
                name: call.name.clone(),
                method,
                evidence,
            });
    }

    /// The one method `call` reaches on `owner`: its own member (private members only from
    /// inside `owner`, static ones from inside or through the type), else the single inherited
    /// candidate.
    fn direct_method(
        &self,
        owner: usize,
        path: &str,
        call: &DispatchCall,
    ) -> Option<(usize, Vec<Location>)> {
        let is_inside = self.declarations(owner).any(|(file, _, typ)| {
            self.files[file].file_path == path && super::resolve::encloses(&typ.range, &call.range)
        });
        let allows_static = is_inside || call.receiver_kind == ReceiverKind::Type;
        let own = self.named_methods(owner, call, is_inside, allows_static);
        if !own.is_empty() {
            return (own.len() == 1 && !self.inherits_overload(owner, own[0], call, allows_static))
                .then(|| (own[0], Vec::new()));
        }
        let mut candidates = Vec::new();
        let mut evidence = Vec::new();
        for parent in self.ancestors(owner) {
            let methods = self.named_methods(parent.owner, call, false, allows_static);
            if !methods.is_empty() {
                candidates.extend(methods);
                evidence.extend(parent.evidence);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        (candidates.len() == 1).then(|| (candidates[0], evidence))
    }

    /// Whether an ancestor of `owner` declares another overload that `call` may reach instead
    /// of `method`: the same name and parameter count with other parameter types. These
    /// languages choose among the inherited and the own overloads by the argument types, which
    /// are unknown here. C# is left out: there a method of the derived type is chosen whenever
    /// the arguments fit it. A parameter that names a type parameter of the ancestor may stand
    /// for the type `method` declares, so it does not tell the two apart.
    fn inherits_overload(
        &self,
        owner: usize,
        method: usize,
        call: &DispatchCall,
        allows_static: bool,
    ) -> bool {
        if !matches!(
            self.unit(owner).language.as_str(),
            "java" | "kotlin" | "scala" | "groovy" | "swift"
        ) {
            return false;
        }
        let own = &self.methods[method].declaration;
        if !own.has_known_signature {
            return false;
        }
        self.ancestors(owner).into_iter().any(|parent| {
            let generic = &self.typ(parent.owner).parameters;
            let is_generic = |typ: &str| {
                typ.split(|character: char| !character.is_alphanumeric() && character != '_')
                    .any(|word| generic.iter().any(|parameter| parameter == word))
            };
            self.types[parent.owner].methods.iter().any(|id| {
                let other = &self.methods[*id].declaration;
                other.name == own.name
                    && other.has_known_signature
                    && !other.is_private
                    && (allows_static || !other.is_static)
                    && other.parameters.len() == own.parameters.len()
                    && call
                        .argument_count
                        .is_none_or(|count| count == other.parameters.len())
                    && other
                        .parameters
                        .iter()
                        .zip(&own.parameters)
                        .any(|(theirs, ours)| theirs != ours && !is_generic(theirs))
            })
        })
    }

    /// Methods of `owner` named like `call`; overloads must be singled out by argument count.
    fn named_methods(
        &self,
        owner: usize,
        call: &DispatchCall,
        allows_private: bool,
        allows_static: bool,
    ) -> Vec<usize> {
        let named = self.types[owner]
            .methods
            .iter()
            .copied()
            .filter(|id| {
                let method = &self.methods[*id].declaration;
                method.name == call.name
                    && (allows_private || !method.is_private)
                    && (allows_static || !method.is_static)
            })
            .collect::<Vec<_>>();
        if named.len() < 2 {
            return named;
        }
        named
            .into_iter()
            .filter(|id| {
                call.argument_count == Some(self.methods[*id].declaration.parameters.len())
            })
            .collect()
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

    /// Records the methods that look like implementations of a declaration and are linked to
    /// none: a type names the declaration's owner as a base, by a proven relationship or only
    /// by spelling, and has a method of the same name. Go types name no base, and a Rust
    /// implementation block enters the index only once its trait is resolved.
    fn record_unproven(&mut self) {
        let linked: HashSet<usize> = self.links.iter().map(|link| link.implementation).collect();
        let mut unproven: HashMap<usize, Vec<usize>> = HashMap::new();
        for owner in 0..self.types.len() {
            let language = self.unit(owner).language.as_str();
            if matches!(language, "go" | "rust") {
                continue;
            }
            let own: Vec<usize> = self.types[owner]
                .methods
                .iter()
                .copied()
                .filter(|method| {
                    let declaration = &self.methods[*method].declaration;
                    !declaration.is_abstract
                        && !declaration.is_private
                        && !declaration.is_static
                        && !linked.contains(method)
                })
                .collect();
            if own.is_empty() {
                continue;
            }
            let mut bases: HashSet<usize> = self
                .ancestors(owner)
                .iter()
                .map(|parent| parent.owner)
                .collect();
            let proven: HashSet<&str> = self.parents[owner]
                .iter()
                .map(|parent| self.typ(parent.owner).name.as_str())
                .collect();
            for base in self.declarations(owner).flat_map(|(_, _, typ)| &typ.bases) {
                let name = base
                    .name
                    .rsplit(['.', ':', '\\'])
                    .next()
                    .unwrap_or(&base.name);
                if proven.contains(name) {
                    continue;
                }
                bases.extend(
                    self.by_name
                        .get(name)
                        .into_iter()
                        .flatten()
                        .copied()
                        .filter(|parent| {
                            *parent != owner
                                && family(&self.unit(*parent).language) == family(language)
                        }),
                );
            }
            for base in bases {
                for &declaration in &self.types[base].methods {
                    let declared = &self.methods[declaration].declaration;
                    if declared.is_private
                        || declared.is_static
                        || declared.is_final
                        || !declared.is_virtual
                    {
                        continue;
                    }
                    let same_named = own.iter().copied().filter(|method| {
                        self.methods[*method].declaration.name == declared.name
                            && (!declared.is_package_scoped
                                || self.shares_package(declaration, *method))
                    });
                    unproven.entry(declaration).or_default().extend(same_named);
                }
            }
        }
        unproven.retain(|_, methods| !methods.is_empty());
        self.unproven = unproven;
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

    /// Whether `other` is declared where a package-scoped `method` is visible: in the same
    /// file, or in another file of the same package. A Dart library is one file here.
    fn shares_package(&self, method: usize, other: usize) -> bool {
        let (path, other_path) = (
            &self.methods[method].location.path,
            &self.methods[other].location.path,
        );
        if path == other_path {
            return true;
        }
        let (unit, other_unit) = (self.method_unit(method), self.method_unit(other));
        unit.language != "dart"
            && unit.namespace == other_unit.namespace
            && (!unit.namespace.is_empty()
                || std::path::Path::new(path).parent() == std::path::Path::new(other_path).parent())
    }

    fn matches(&self, base: usize, implementation: usize, signatures: &Signatures<'_, '_>) -> bool {
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
            || x.is_package_scoped && !self.shares_package(base, implementation)
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
        // The same spelling of a type in two source scopes is one type only when both scopes
        // lead to one declaration, one import or a name the language defines.
        a.location.path == b.location.path
            && self.typ(a.owner).namespace == self.typ(b.owner).namespace
            || x.parameters
                .iter()
                .chain((language == "go").then_some(&x.return_type))
                .all(|typ| self.same_types(base, implementation, typ, signatures))
    }

    fn connect_go(&mut self, signatures: &Signatures<'_, '_>, sources: &HashMap<String, String>) {
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
        let mut proofs = Vec::new();
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
                        .filter(|method| self.matches(*base, **method, signatures))
                        .count()
                        == 1
                }) {
                    let mut evidence = vec![self.location(contract), self.location(owner)];
                    for base in required {
                        evidence.push(self.methods[*base].location.clone());
                        let mut proof = self.signature_evidence(*base, signatures);
                        for method in &self.types[owner].methods {
                            if self.matches(*base, *method, signatures) {
                                evidence.push(self.methods[*method].location.clone());
                                proof.extend(self.signature_evidence(*method, signatures));
                            }
                        }
                        evidence.extend(proof.iter().cloned());
                        proofs.extend(proof);
                    }
                    let is_pointer = required.iter().any(|base| {
                        self.types[owner].methods.iter().any(|method| {
                            self.matches(*base, *method, signatures)
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
        self.remember_proof(&proofs, sources);
    }
}
