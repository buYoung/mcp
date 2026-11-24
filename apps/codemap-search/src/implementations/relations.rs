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
                        let parent_params = &self.typ(parent).parameters;
                        let substitutions = if !parent_params.is_empty()
                            && base.arguments.len() == parent_params.len()
                        {
                            parent_params
                                .iter()
                                .zip(&base.arguments)
                                .map(|(parameter, argument)| TypeSubstitution {
                                    parameter: parameter.clone(),
                                    argument: argument.clone(),
                                    file,
                                    unit,
                                    owner: id,
                                    namespace: typ.namespace.clone(),
                                    range: base.range.clone(),
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };
                        self.parents[id].push(Parent {
                            owner: parent,
                            evidence,
                            is_pointer: false,
                            substitutions,
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
                    let mut block_contract = None;
                    if unit.language == "swift" {
                        // Swift extensions may declare conformance to multiple protocols
                        // (`extension Foo: ProtoA, ProtoB { ... }`) and their methods can satisfy
                        // any protocol `owner` conforms to, not only a single contract here.
                        for contract_ref in &block.contracts {
                            let Some((contract, evidence)) = self.resolve_type(
                                file_id,
                                unit_id,
                                &block.namespace,
                                contract_ref,
                                resolver,
                            ) else {
                                continue;
                            };
                            if contract == owner || self.typ(contract).is_final {
                                continue;
                            }
                            let mut contract_proof = proof.clone();
                            contract_proof.extend(evidence);
                            contract_proof.push(Location {
                                path: file.file_path.clone(),
                                range: block.owner.range.clone(),
                            });
                            contract_proof.push(Location {
                                path: file.file_path.clone(),
                                range: contract_ref.range.clone(),
                            });
                            self.remember_proof(&contract_proof, sources);
                            let parent_params = &self.typ(contract).parameters;
                            let substitutions = if !parent_params.is_empty()
                                && contract_ref.arguments.len() == parent_params.len()
                            {
                                parent_params
                                    .iter()
                                    .zip(&contract_ref.arguments)
                                    .map(|(parameter, argument)| TypeSubstitution {
                                        parameter: parameter.clone(),
                                        argument: argument.clone(),
                                        file: file_id,
                                        unit: unit_id,
                                        owner,
                                        namespace: block.namespace.clone(),
                                        range: contract_ref.range.clone(),
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            };
                            self.parents[owner].push(Parent {
                                owner: contract,
                                evidence: contract_proof,
                                is_pointer: false,
                                substitutions,
                            });
                        }
                    } else if unit.language == "go" && !block.contracts.is_empty() {
                        // Go compile-time interface assertion (`var _ pkg.Iface = (*Impl)(nil)`).
                        if block.contracts.len() != 1 {
                            continue;
                        }
                        let contract_ref = &block.contracts[0];
                        let Some((contract, evidence)) = self.resolve_type(
                            file_id,
                            unit_id,
                            &block.namespace,
                            contract_ref,
                            resolver,
                        ) else {
                            continue;
                        };
                        if contract == owner || !self.typ(contract).is_contract {
                            continue;
                        }
                        proof.extend(evidence);
                        proof.push(Location {
                            path: file.file_path.clone(),
                            range: block.owner.range.clone(),
                        });
                        proof.push(Location {
                            path: file.file_path.clone(),
                            range: contract_ref.range.clone(),
                        });
                        self.remember_proof(&proof, sources);
                        let is_pointer = block.owner.name.starts_with('*');
                        if !self.parents[owner]
                            .iter()
                            .any(|parent| parent.owner == contract && parent.is_pointer == is_pointer)
                        {
                            self.parents[owner].push(Parent {
                                owner: contract,
                                evidence: proof,
                                is_pointer,
                                substitutions: Vec::new(),
                            });
                        }
                    } else {
                        if !block.contracts.is_empty() {
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
                            block_contract = Some(contract);
                        }
                        proof.push(Location {
                            path: file.file_path.clone(),
                            range: block.owner.range.clone(),
                        });
                        self.remember_proof(&proof, sources);
                        if let Some(contract) = block_contract {
                            self.parents[owner].push(Parent {
                                owner: contract,
                                evidence: proof,
                                is_pointer: false,
                                substitutions: Vec::new(),
                            });
                        }
                    }
                    for method in &block.methods {
                        if active(
                            &unit.language,
                            &file.file_path,
                            &method.range,
                            &method.conditions,
                            resolver,
                        ) {
                            self.add_method(owner, method.clone(), (file_id, unit_id), block_contract);
                        }
                    }
                }
            }
        }
        let signatures = Signatures::new(resolver);
        self.connect_go(&signatures, sources);
        // `self.parents` is complete after `connect_go`; compute ancestor lists once and reuse
        // them across link, unproven, replaceable, and call resolution passes.
        let ancestors_by_owner: Vec<Vec<Parent>> = (0..self.types.len())
            .map(|owner| self.ancestors(owner))
            .collect();
        // Declarations read to accept a signature, remembered once the links are in place.
        let mut proofs = Vec::new();
        for owner in 0..self.types.len() {
            let ancestors = &ancestors_by_owner[owner];
            if ancestors.is_empty() {
                continue;
            }
            let own_methods = self.types[owner].methods.clone();
            for method in own_methods {
                let implementation = &self.methods[method];
                if implementation.declaration.is_abstract {
                    continue;
                }
                for parent in ancestors {
                    if implementation
                        .contract
                        .is_some_and(|contract| contract != parent.owner)
                    {
                        continue;
                    }
                    let owner_language = self.unit(owner).language.as_str();
                    if owner_language == "rust" && implementation.contract.is_none() {
                        continue;
                    }
                    // A Go value-receiver interface assertion (`var _ Iface = Impl{}`) only
                    // includes value-receiver methods in the method set of `Impl`.
                    if owner_language == "go"
                        && !parent.is_pointer
                        && implementation.declaration.is_pointer_receiver
                    {
                        continue;
                    }
                    let mut candidates = self.types[parent.owner]
                        .methods
                        .iter()
                        .copied()
                        .filter_map(|base| {
                            self.matches(base, method, &parent.substitutions, &signatures)
                                .map(|sig_proof| (base, sig_proof))
                        })
                        .collect::<Vec<_>>();
                    // Swift protocol extensions add default implementations directly onto the
                    // protocol type alongside the abstract requirement; prefer the requirement.
                    if candidates.len() > 1 && self.typ(parent.owner).is_contract {
                        let abstract_only: Vec<_> = candidates
                            .iter()
                            .filter(|(base, _)| self.methods[*base].declaration.is_abstract)
                            .cloned()
                            .collect();
                        if abstract_only.len() == 1 {
                            candidates = abstract_only;
                        }
                    }
                    if candidates.len() != 1 {
                        continue;
                    }
                    let (base, sig_proof) = candidates.pop().unwrap();
                    if !active(
                        &self.unit(parent.owner).language,
                        &self.methods[base].location.path,
                        &self.methods[base].declaration.range,
                        &self.methods[base].declaration.conditions,
                        resolver,
                    ) || !active(
                        owner_language,
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
                    let mut evidence = parent.evidence.clone();
                    evidence.extend(sig_proof.iter().cloned());
                    proofs.extend(sig_proof);
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
        self.record_unproven(&ancestors_by_owner);
        let declarations = self
            .links
            .iter()
            .map(|link| link.declaration)
            .collect::<HashSet<_>>();
        let replaceable = self.replaceable_methods(&declarations, &ancestors_by_owner);
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
                        &ancestors_by_owner,
                        sources,
                    );
                    let mut candidates = self.call_methods(owner, &call.name, call.argument_count);
                    if candidates.is_empty() {
                        for parent in &ancestors_by_owner[owner] {
                            candidates.extend(self.call_methods(
                                parent.owner,
                                &call.name,
                                call.argument_count,
                            ));
                            evidence.extend(parent.evidence.iter().cloned());
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
    fn replaceable_methods(
        &self,
        declarations: &HashSet<usize>,
        ancestors_by_owner: &[Vec<Parent>],
    ) -> HashSet<usize> {
        let mut replaceable = declarations.clone();
        for owner in 0..self.types.len() {
            if self.types[owner].methods.is_empty() || ancestors_by_owner[owner].is_empty() {
                continue;
            }
            let names = self.types[owner]
                .methods
                .iter()
                .map(|id| self.methods[*id].declaration.name.as_str())
                .collect::<HashSet<_>>();
            for parent in &ancestors_by_owner[owner] {
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
    #[allow(clippy::too_many_arguments)]
    fn record_direct_call(
        &mut self,
        owner: usize,
        path: &str,
        call: &DispatchCall,
        type_evidence: &[Location],
        replaceable: &HashSet<usize>,
        ancestors_by_owner: &[Vec<Parent>],
        sources: &HashMap<String, String>,
    ) {
        if self.direct_call_count >= DIRECT_CALLS_PER_SNAPSHOT {
            self.omitted += 1;
            return;
        }
        let Some((method, mut evidence)) =
            self.direct_method(owner, path, call, ancestors_by_owner)
        else {
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
        ancestors_by_owner: &[Vec<Parent>],
    ) -> Option<(usize, Vec<Location>)> {
        if self.declarations(owner).any(|(_, _, typ)| typ.has_body_error)
            || ancestors_by_owner[owner]
                .iter()
                .any(|parent| self.declarations(parent.owner).any(|(_, _, typ)| typ.has_body_error))
        {
            return None;
        }
        let is_inside = self.declarations(owner).any(|(file, _, typ)| {
            self.files[file].file_path == path && super::resolve::encloses(&typ.range, &call.range)
        });
        let allows_static = is_inside || call.receiver_kind == ReceiverKind::Type;
        let own = self.named_methods(owner, call, is_inside, allows_static);
        if !own.is_empty() {
            return (own.len() == 1
                && !self.inherits_overload(owner, own[0], call, allows_static, ancestors_by_owner))
            .then(|| (own[0], Vec::new()));
        }
        let mut candidates = Vec::new();
        let mut evidence = Vec::new();
        for parent in &ancestors_by_owner[owner] {
            let methods = self.named_methods(parent.owner, call, false, allows_static);
            if !methods.is_empty() {
                candidates.extend(methods);
                evidence.extend(parent.evidence.iter().cloned());
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
        ancestors_by_owner: &[Vec<Parent>],
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
        ancestors_by_owner[owner].iter().any(|parent| {
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
    fn record_unproven(&mut self, ancestors_by_owner: &[Vec<Parent>]) {
        let linked: HashSet<usize> = self.links.iter().map(|link| link.implementation).collect();
        let mut unproven: HashMap<usize, Vec<usize>> = HashMap::new();
        for owner in 0..self.types.len() {
            let language = self.unit(owner).language.as_str();
            if matches!(language, "go" | "rust") {
                continue;
            }
            if ancestors_by_owner[owner].is_empty()
                && self.declarations(owner).all(|(_, _, typ)| typ.bases.is_empty())
            {
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
            let mut bases: HashSet<usize> = ancestors_by_owner[owner]
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
                let substitutions = compose_substitutions(&parent.substitutions, &next.substitutions);
                pending.push((
                    Parent {
                        owner: next.owner,
                        evidence,
                        is_pointer: parent.is_pointer || next.is_pointer,
                        substitutions,
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

    fn matches(
        &self,
        base: usize,
        implementation: usize,
        substitutions: &[TypeSubstitution],
        signatures: &Signatures<'_, '_>,
    ) -> Option<Vec<Location>> {
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
            return None;
        }
        if matches!(language, "javascript" | "python" | "ruby") {
            return Some(Vec::new());
        }
        if !x.has_known_signature
            || !y.has_known_signature
            || x.parameters.len() != y.parameters.len()
            || x.qualifiers != y.qualifiers
        {
            return None;
        }
        let base_scope = self.method_type_scope(base);
        let impl_scope = self.method_type_scope(implementation);
        let mut evidence = Vec::new();
        for (base_param, impl_param) in x.parameters.iter().zip(&y.parameters) {
            evidence.extend(self.same_type_spelling(
                language,
                &base_scope,
                &impl_scope,
                base_param,
                impl_param,
                substitutions,
                signatures,
            )?);
        }
        if language == "go" {
            evidence.extend(self.same_type_spelling(
                language,
                &base_scope,
                &impl_scope,
                &x.return_type,
                &y.return_type,
                substitutions,
                signatures,
            )?);
        }
        Some(evidence)
    }

    /// All required methods of a Go interface when the interface and every embedded interface
    /// it names resolve to complete indexed interfaces (returning `None` if any embedded
    /// interface is external or incomplete, so structural matching never uses a subset).
    fn go_complete_interface_methods(&self, contract: usize) -> Option<(Vec<usize>, Vec<Location>)> {
        let mut required = Vec::new();
        let mut evidence = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = vec![contract];
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                return None;
            }
            let typ = self.typ(id);
            if self.unit(id).language != "go" || !typ.is_contract || !typ.is_complete {
                return None;
            }
            // Every embedded interface must resolve to a complete indexed interface.
            let expected_bases: usize = self.declarations(id).map(|(_, _, d)| d.bases.len()).sum();
            let resolved_bases = &self.parents[id];
            if resolved_bases.len() < expected_bases {
                return None;
            }
            evidence.push(self.location(id));
            required.extend(self.types[id].methods.iter().copied());
            if required.len() > 128 {
                return None;
            }
            for parent in resolved_bases {
                evidence.extend(parent.evidence.iter().cloned());
                stack.push(parent.owner);
            }
        }
        (!required.is_empty()).then_some((required, evidence))
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
            if self.unit(contract).language != "go" || !self.typ(contract).is_contract {
                continue;
            }
            let Some((required, contract_evidence)) = self.go_complete_interface_methods(contract)
            else {
                continue;
            };
            let package = std::path::Path::new(&self.files[self.types[contract].file].file_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy()
                .to_string();
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
                if self.unit(owner).namespace != self.unit(contract).namespace
                    || self.parents[owner]
                        .iter()
                        .any(|parent| parent.owner == contract)
                {
                    continue;
                }
                let mut matched = Vec::with_capacity(required.len());
                let mut all_matched = true;
                for &base in &required {
                    let mut found = None;
                    let mut multiple = false;
                    for &method in &self.types[owner].methods {
                        if let Some(sig_proof) = self.matches(base, method, &[], signatures) {
                            if found.is_some() {
                                multiple = true;
                                break;
                            }
                            found = Some((method, sig_proof));
                        }
                    }
                    if multiple {
                        all_matched = false;
                        break;
                    }
                    let Some((method, sig_proof)) = found else {
                        all_matched = false;
                        break;
                    };
                    matched.push((base, method, sig_proof));
                }
                if all_matched {
                    let mut evidence = contract_evidence.clone();
                    evidence.push(self.location(owner));
                    let mut is_pointer = false;
                    for (base, method, sig_proof) in matched {
                        evidence.push(self.methods[base].location.clone());
                        evidence.push(self.methods[method].location.clone());
                        evidence.extend(sig_proof.iter().cloned());
                        proofs.extend(sig_proof);
                        is_pointer |= self.methods[method].declaration.is_pointer_receiver;
                    }
                    self.parents[owner].push(Parent {
                        owner: contract,
                        evidence,
                        is_pointer,
                        substitutions: Vec::new(),
                    });
                }
            }
        }
        self.remember_proof(&proofs, sources);
    }
}

/// Compose child->parent substitutions with parent->grandparent substitutions.
/// If `parent` maps `U -> Concrete` (at child scope) and `next` maps `T -> U` (at parent scope),
/// the composed mapping is `T -> Concrete` (at child scope).
fn compose_substitutions(
    outer: &[TypeSubstitution],
    inner: &[TypeSubstitution],
) -> Vec<TypeSubstitution> {
    if outer.is_empty() {
        return inner.to_vec();
    }
    inner
        .iter()
        .map(|sub| {
            let trimmed = sub.argument.trim();
            if let Some(mapped) = outer.iter().find(|o| o.parameter == trimmed) {
                return TypeSubstitution {
                    parameter: sub.parameter.clone(),
                    argument: mapped.argument.clone(),
                    file: mapped.file,
                    unit: mapped.unit,
                    owner: mapped.owner,
                    namespace: mapped.namespace.clone(),
                    range: mapped.range.clone(),
                };
            }
            // Substitute bare parameter identifiers inside compound arguments (e.g. `List<U>`)
            // when all referenced outer parameters come from the same scope.
            let mut replaced = String::with_capacity(sub.argument.len());
            let mut bytes = sub.argument.as_bytes();
            let mut changed = false;
            let mut single_outer: Option<&TypeSubstitution> = None;
            let mut mixed_scopes = false;
            while !bytes.is_empty() {
                if bytes[0].is_ascii_alphabetic() || bytes[0] == b'_' {
                    let end = bytes
                        .iter()
                        .position(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
                        .unwrap_or(bytes.len());
                    let word = std::str::from_utf8(&bytes[..end]).unwrap_or("");
                    if let Some(mapped) = outer.iter().find(|o| o.parameter == word) {
                        replaced.push_str(&mapped.argument);
                        changed = true;
                        match single_outer {
                            None => single_outer = Some(mapped),
                            Some(prev)
                                if prev.file == mapped.file
                                    && prev.unit == mapped.unit
                                    && prev.namespace == mapped.namespace => {}
                            Some(_) => mixed_scopes = true,
                        }
                    } else {
                        replaced.push_str(word);
                    }
                    bytes = &bytes[end..];
                } else {
                    let ch = std::str::from_utf8(bytes).unwrap().chars().next().unwrap();
                    replaced.push(ch);
                    bytes = &bytes[ch.len_utf8()..];
                }
            }
            if changed && !mixed_scopes {
                if let Some(mapped) = single_outer {
                    if mapped.file == sub.file && mapped.unit == sub.unit {
                        return TypeSubstitution {
                            parameter: sub.parameter.clone(),
                            argument: replaced,
                            file: mapped.file,
                            unit: mapped.unit,
                            owner: mapped.owner,
                            namespace: mapped.namespace.clone(),
                            range: mapped.range.clone(),
                        };
                    }
                }
            }
            sub.clone()
        })
        .collect()
}
