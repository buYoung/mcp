use super::engine::*;
use super::model::*;
use super::syntax::*;

const POINTER_CONDITIONS: &[&str] = &[
    "pointer_provenance_required",
    "pointer_lifetime_alignment_and_aliasing_unproven",
];
impl Interpreter<'_, '_> {
    fn require(&mut self, conditions: &[&str]) {
        self.conditions
            .extend(conditions.iter().map(|v| (*v).into()));
    }
    fn actual_values(&self, value: &Value) -> Vec<Value> {
        if let Some(current) = self.slots.get(value) {
            return current.options();
        }
        let mut values = value.options();
        values.extend(
            self.analyzer
                .read_values(value)
                .iter()
                .flat_map(Value::options),
        );
        values.sort();
        values.dedup();
        values
    }
    pub fn pointer_values(&self, value: &Value) -> Vec<Value> {
        self.actual_values(value)
            .into_iter()
            .filter(|v| {
                v.kind == "wrapper"
                    && matches!(v.name.as_str(), "rust:nonnull" | "rust:raw_pointer")
            })
            .collect()
    }
    pub fn rust_call(
        &mut self,
        text: &str,
        callee: &Value,
        arguments: &[Value],
        id: NodeId,
    ) -> Option<Value> {
        let receiver = if callee.kind == "slot" {
            callee.base().clone()
        } else {
            Value::unknown()
        };
        let method = if callee.kind == "slot" {
            callee.key().name.as_str()
        } else {
            ""
        };
        let callee_node = self.source.child(id, &["function"]);
        if arguments.is_empty()
            && self.rust_standard_path(
                text,
                &["std::any::TypeId::of", "core::any::TypeId::of"],
                None,
            )
        {
            let types = callee_node.and_then(|n| self.source.child(n, &["type_arguments"]));
            let parts = types
                .map(|n| self.source.nodes[n].children.clone())
                .unwrap_or_default();
            if parts.len() == 1 {
                let ty = self.rust_type_expression(Some(parts[0]));
                if ty.is_concrete() {
                    self.require(&["standard_type_id_identity"]);
                    return Some(Value::new("key", format!("rust:type_id:{}", ty.display())));
                }
            }
            self.analyzer.notices.insert((
                "type_id_type_argument_unresolved".into(),
                self.identifier.clone(),
            ));
            return Some(Value::new(
                "opaque_key",
                format!("{}:type_id:{}", self.identifier, self.source.text(types)),
            ));
        }
        if arguments.is_empty()
            && self.rust_standard_path(
                text,
                &[
                    "std::collections::HashMap::new",
                    "std::collections::hash_map::HashMap::new",
                    "std::collections::BTreeMap::new",
                    "std::collections::btree_map::BTreeMap::new",
                ],
                None,
            )
        {
            let value = self.allocation(id, ":map");
            self.analyzer.kinds.insert(value.clone(), "map".into());
            self.require(&["standard_map_constructor"]);
            return Some(value);
        }
        if let Some(value) = self.rust_pointer_call(text, &receiver, method, arguments, id) {
            return Some(value);
        }
        if arguments.is_empty() && matches!(method, "downcast_ref" | "downcast_mut") {
            let declared = self.analyzer.type_text(&receiver, self.source_index);
            let declared = declared
                .trim_start_matches('&')
                .trim_start_matches("mut ")
                .trim();
            let boxed = declared
                .split_once('<')
                .filter(|(base, _)| base.trim().ends_with("Box"));
            let trait_text = boxed
                .map(|(_, rest)| rest.trim_end_matches('>'))
                .unwrap_or(declared)
                .trim_start_matches("dyn ")
                .split('+')
                .next()
                .unwrap_or_default()
                .trim();
            if self.rust_standard_path(trait_text, &["std::any::Any", "core::any::Any"], None) {
                if let Some((base, _)) = boxed {
                    if !self.rust_standard_path(
                        &format!("{}::new", base.trim()),
                        &["std::boxed::Box::new", "alloc::boxed::Box::new"],
                        Some("Box::new"),
                    ) {
                        return None;
                    }
                }
                let types = callee_node.and_then(|n| self.source.child(n, &["type_arguments"]));
                let parts = types
                    .map(|n| self.source.nodes[n].children.clone())
                    .unwrap_or_default();
                if parts.len() != 1 {
                    return Some(Value::unknown());
                }
                let ty = self.rust_type_expression(Some(parts[0]));
                if !ty.is_concrete() || ty.display().contains('<') {
                    self.analyzer.notices.insert((
                        "any_downcast_target_unresolved".into(),
                        self.identifier.clone(),
                    ));
                    return Some(Value::unknown());
                }
                let identity = ty.display();
                let pointed = if boxed.is_some() {
                    receiver.clone().field("pointee")
                } else {
                    receiver.clone()
                };
                let owners = self
                    .analyzer
                    .read_values(&pointed)
                    .iter()
                    .map(|v| self.analyzer.owner(v, self.source_index))
                    .collect::<std::collections::BTreeSet<_>>();
                if !owners.is_empty() && !owners.contains("") && !owners.contains(&identity) {
                    self.analyzer.notices.insert((
                        "known_any_downcast_type_mismatch".into(),
                        self.identifier.clone(),
                    ));
                    return Some(Value::unknown());
                }
                let mut conditions = self.conditions.clone();
                conditions.push("standard_any_downcast_type_match_required".into());
                if boxed.is_some() {
                    conditions.push("standard_box_deref_semantics".into());
                }
                self.analyzer
                    .value_conditions
                    .insert(pointed.clone(), conditions.into_iter().collect());
                self.analyzer
                    .record_type(&pointed, self.source_index, &identity, "");
                self.analyzer.owners.insert(pointed.clone(), identity);
                return Some(Value::nested("wrapper", "rust:option", pointed, None));
            }
        }
        if callee.kind == "type" {
            if let Some(fields) = self.analyzer.program.tuple_fields.get(&callee.name) {
                if arguments.len() != fields.len() {
                    return Some(Value::unknown());
                }
                let value = self.allocation(id, ":tuple_struct");
                self.analyzer
                    .owners
                    .insert(value.clone(), callee.name.clone());
                for (index, argument) in arguments.iter().enumerate() {
                    self.emit(
                        "store",
                        value.clone().field(&index.to_string()),
                        argument.clone(),
                        id,
                        &[],
                        None,
                    );
                }
                return Some(value);
            }
        }
        for (variant, wrapper) in [
            ("Some", "rust:option"),
            ("Ok", "rust:result_ok"),
            ("Err", "rust:result_err"),
        ] {
            let qualified = if variant == "Some" {
                "option::Option"
            } else {
                "result::Result"
            };
            let std = format!("std::{qualified}::{variant}");
            let core = format!("core::{qualified}::{variant}");
            if arguments.len() == 1 && self.rust_standard_path(text, &[&std, &core], Some(variant))
            {
                return Some(Value::nested(
                    "wrapper",
                    wrapper,
                    arguments[0].clone(),
                    None,
                ));
            }
        }
        if arguments.len() == 1
            && self.rust_standard_path(
                text,
                &["std::boxed::Box::new", "alloc::boxed::Box::new"],
                Some("Box::new"),
            )
        {
            let value = self.allocation(id, ":box");
            self.analyzer
                .record_type(&value, self.source_index, "std::boxed::Box<_>", "");
            self.emit(
                "store",
                value.clone().field("pointee"),
                arguments[0].clone(),
                id,
                &["standard_box_constructor"],
                None,
            );
            return Some(value);
        }
        if method.is_empty() {
            return None;
        }
        let actuals = self.actual_values(&receiver);
        let variants: Vec<_> = actuals
            .iter()
            .filter(|v| {
                v.kind == "wrapper"
                    && matches!(
                        v.name.as_str(),
                        "rust:option" | "rust:option_none" | "rust:result_ok" | "rust:result_err"
                    )
            })
            .cloned()
            .collect();
        let options: Vec<_> = variants
            .iter()
            .filter(|v| matches!(v.name.as_str(), "rust:option" | "rust:option_none"))
            .cloned()
            .collect();
        if !options.is_empty()
            && method == "map"
            && arguments.len() == 1
            && matches!(arguments[0].kind.as_str(), "function" | "closure")
        {
            let value = Value::merge(
                options
                    .iter()
                    .filter(|v| v.name == "rust:option")
                    .map(|v| v.base().clone()),
            );
            self.require(&["option_some_required"]);
            if let Some(target) = self.function_index(&arguments[0].name) {
                let returned = self.apply_source(
                    target,
                    Value::unknown(),
                    vec![value],
                    id,
                    Some(closure_captures(&arguments[0])),
                );
                return Some(Value::nested("wrapper", "rust:option", returned, None));
            }
        }
        if !options.is_empty() && method == "unwrap_or_default" {
            let value = Value::merge(
                options
                    .iter()
                    .filter(|v| v.name == "rust:option")
                    .map(|v| v.base().clone()),
            );
            if value
                .options()
                .iter()
                .all(|v| v.kind == "wrapper" && v.name == "rust:slice_view")
            {
                self.require(&["option_some_required"]);
                return Some(value);
            }
        }
        if !options.is_empty() && method == "take" && arguments.is_empty() {
            let empty = Value::nested("wrapper", "rust:option_none", Value::unknown(), None);
            let returned = Value::merge(options);
            self.emit(
                "remove",
                receiver.clone(),
                Value::unknown(),
                id,
                &["standard_option_take"],
                None,
            );
            if receiver.kind == "slot" {
                self.emit(
                    "store",
                    receiver.clone(),
                    empty,
                    id,
                    &["standard_option_take"],
                    None,
                );
            } else {
                let mut base = callee_node.and_then(|n| self.source.child(n, &["value", "object"]));
                while base.is_some_and(|n| {
                    matches!(
                        self.source.nodes[n].kind.as_str(),
                        "parenthesized_expression" | "reference_expression"
                    )
                }) {
                    base = base.and_then(|n| {
                        self.source
                            .child(n, &["value"])
                            .or_else(|| self.source.nodes[n].children.last().copied())
                    });
                }
                if let Some(base) = base.filter(|n| self.source.nodes[*n].kind == "identifier") {
                    self.env.insert(self.source.text(Some(base)).into(), empty);
                }
            }
            self.require(&["standard_option_take"]);
            return Some(returned);
        }
        if !options.is_empty()
            && matches!(method, "as_ref" | "as_mut" | "as_deref" | "as_deref_mut")
            && arguments.is_empty()
        {
            let mut payloads: Vec<_> = options
                .iter()
                .filter(|v| v.name == "rust:option")
                .map(|v| v.base().clone())
                .collect();
            if payloads.is_empty() {
                return Some(Value::nested(
                    "wrapper",
                    "rust:option_none",
                    Value::unknown(),
                    None,
                ));
            }
            if matches!(method, "as_deref" | "as_deref_mut") {
                for value in &payloads {
                    if !self.actual_values(value).iter().any(|v| {
                        self.analyzer.type_text(v, self.source_index) == "std::boxed::Box<_>"
                    }) {
                        return None;
                    }
                }
                payloads = payloads.into_iter().map(|v| v.field("pointee")).collect();
                self.require(&["standard_box_deref_semantics"]);
            }
            return Some(Value::nested(
                "wrapper",
                "rust:option",
                Value::merge(payloads),
                None,
            ));
        }
        if !options.is_empty() && method == "ok_or" && arguments.len() == 1 {
            return Some(Value::merge(options.iter().map(|v| {
                Value::nested(
                    "wrapper",
                    if v.name == "rust:option" {
                        "rust:result_ok"
                    } else {
                        "rust:result_err"
                    },
                    if v.name == "rust:option" {
                        v.base().clone()
                    } else {
                        arguments[0].clone()
                    },
                    None,
                )
            })));
        }
        if method == "map_err"
            && arguments.len() == 1
            && variants.iter().any(|v| v.name.starts_with("rust:result_"))
        {
            return Some(Value::merge(
                variants
                    .iter()
                    .filter(|v| v.name.starts_with("rust:result_"))
                    .map(|v| {
                        if v.name == "rust:result_ok" {
                            v.clone()
                        } else {
                            Value::nested("wrapper", "rust:result_err", Value::unknown(), None)
                        }
                    }),
            ));
        }
        if matches!(method, "unwrap" | "expect")
            && arguments.len() == usize::from(method == "expect")
            && !variants.is_empty()
        {
            let values = variants
                .iter()
                .filter(|v| matches!(v.name.as_str(), "rust:option" | "rust:result_ok"))
                .map(|v| v.base().clone())
                .collect::<Vec<_>>();
            if values.is_empty() {
                return Some(Value::unknown());
            }
            self.require(&[if options.is_empty() {
                "result_ok_required"
            } else {
                "option_some_required"
            }]);
            return Some(Value::merge(values));
        }
        if actuals.iter().any(|v| {
            v.kind == "allocation"
                && self.analyzer.type_text(v, self.source_index) == "std::boxed::Box<_>"
        }) && matches!(method, "as_ref" | "as_mut")
            && arguments.is_empty()
        {
            self.require(&["standard_box_reference_semantics"]);
            return Some(receiver.field("pointee"));
        }
        let views: Vec<_> = actuals
            .iter()
            .filter(|v| v.kind == "wrapper" && v.name == "rust:slice_view")
            .cloned()
            .collect();
        if !views.is_empty() && matches!(method, "iter" | "iter_mut") {
            self.require(&["slice_range_membership_required"]);
            return Some(Value::nested(
                "iterator",
                "values",
                Value::merge(views),
                None,
            ));
        }
        let iterators: Vec<_> = actuals
            .iter()
            .filter(|v| v.kind == "iterator")
            .cloned()
            .collect();
        if !iterators.is_empty() && method == "chain" && arguments.len() == 1 {
            let other: Vec<_> = arguments[0]
                .options()
                .into_iter()
                .filter(|v| v.kind == "iterator")
                .collect();
            if !other.is_empty() {
                return Some(Value::nested(
                    "iterator",
                    "values",
                    Value::merge(
                        iterators
                            .iter()
                            .chain(other.iter())
                            .map(|v| v.base().clone()),
                    ),
                    None,
                ));
            }
        }
        if !iterators.is_empty() && method == "next" && arguments.is_empty() {
            let mut elements = Vec::new();
            for iterator in iterators {
                for mut base in iterator.base().options() {
                    if base.kind == "wrapper" && base.name == "rust:slice_view" {
                        base = base.base().clone();
                        self.require(&["slice_range_membership_required"]);
                    }
                    elements.push(base.slot(Value::element()));
                }
            }
            self.require(&["iterator_may_be_empty", "stored_iterator_binding_required"]);
            return Some(Value::nested(
                "wrapper",
                "rust:option",
                Value::merge(elements),
                None,
            ));
        }
        let declared = self.analyzer.type_text(&receiver, self.source_index);
        if method == "as_ref"
            && arguments.is_empty()
            && declared.contains("Box")
            && declared.contains("Fn")
            && self.rust_standard_path(
                "Box::new",
                &["std::boxed::Box::new", "alloc::boxed::Box::new"],
                Some("Box::new"),
            )
        {
            self.require(&["standard_box_reference_semantics"]);
            return Some(receiver);
        }
        None
    }
    fn rust_pointer_call(
        &mut self,
        text: &str,
        receiver: &Value,
        method: &str,
        arguments: &[Value],
        id: NodeId,
    ) -> Option<Value> {
        if arguments.len() == 1
            && self.rust_standard_path(
                text,
                &["std::mem::MaybeUninit::new", "core::mem::MaybeUninit::new"],
                None,
            )
        {
            let value = Value::nested(
                "wrapper",
                "rust:maybeuninit_initialized",
                arguments[0].clone(),
                None,
            );
            self.analyzer.value_conditions.insert(
                value.clone(),
                ["standard_maybeuninit_initialized_payload".into()]
                    .into_iter()
                    .collect(),
            );
            return Some(value);
        }
        let initialized: Vec<_> = self
            .actual_values(receiver)
            .into_iter()
            .filter(|v| v.kind == "wrapper" && v.name == "rust:maybeuninit_initialized")
            .collect();
        if !initialized.is_empty()
            && arguments.is_empty()
            && matches!(method, "as_ptr" | "as_mut_ptr")
        {
            self.require(POINTER_CONDITIONS);
            self.require(&["standard_maybeuninit_initialized_payload"]);
            return Some(Value::merge(initialized.iter().map(|v| {
                Value::nested("wrapper", "rust:raw_pointer", v.base().clone(), None)
            })));
        }
        if arguments.len() == 1
            && self.rust_standard_path(
                text,
                &["std::ptr::NonNull::from", "core::ptr::NonNull::from"],
                None,
            )
        {
            let operand = self
                .source
                .child(id, &["arguments"])
                .and_then(|n| self.source.nodes[n].children.first().copied());
            let mut declared = self.analyzer.type_text(&arguments[0], self.source_index);
            if let (Some(operand), Some(function)) = (operand, self.function) {
                if self.source.nodes[operand].kind == "identifier" {
                    if let Some((_, ty)) = self.analyzer.program.functions[function]
                        .parameters
                        .iter()
                        .find(|(name, _)| name == self.source.text(Some(operand)))
                    {
                        declared = ty.clone();
                    }
                }
            }
            if operand.is_some_and(|n| self.source.nodes[n].kind == "reference_expression")
                || declared.starts_with('&')
            {
                let mut value = arguments[0].clone();
                if value.kind == "wrapper" && value.name == "rust:maybeuninit_initialized" {
                    value = value.base().clone();
                    self.require(&["standard_maybeuninit_initialized_payload"]);
                }
                self.require(POINTER_CONDITIONS);
                return Some(Value::nested("wrapper", "rust:nonnull", value, None));
            }
            return Some(Value::unknown());
        }
        for name in ["from_ref", "from_mut"] {
            let std = format!("std::ptr::NonNull::{name}");
            let core = format!("core::ptr::NonNull::{name}");
            if arguments.len() == 1 && self.rust_standard_path(text, &[&std, &core], None) {
                self.require(POINTER_CONDITIONS);
                return Some(Value::nested(
                    "wrapper",
                    "rust:nonnull",
                    arguments[0].clone(),
                    None,
                ));
            }
        }
        if arguments.len() == 1
            && self.rust_standard_path(
                text,
                &[
                    "std::ptr::NonNull::new_unchecked",
                    "core::ptr::NonNull::new_unchecked",
                ],
                None,
            )
        {
            let pointers = self.pointer_values(&arguments[0]);
            if pointers.is_empty() {
                self.analyzer
                    .notices
                    .insert(("pointer_origin_unresolved".into(), self.identifier.clone()));
                return Some(Value::unknown());
            }
            self.require(POINTER_CONDITIONS);
            self.require(&["nonnull_precondition_required"]);
            return Some(Value::merge(pointers.iter().map(|v| {
                Value::nested("wrapper", "rust:nonnull", v.base().clone(), None)
            })));
        }
        if !arguments.is_empty()
            || !matches!(
                method,
                "as_ref" | "as_mut" | "as_ptr" | "cast" | "clone" | "read"
            )
        {
            return None;
        }
        let mut pointers = self.pointer_values(receiver);
        if method == "as_ptr" {
            pointers.retain(|v| v.name == "rust:nonnull");
        }
        if method == "read" {
            pointers.retain(|v| v.name == "rust:raw_pointer");
        }
        if pointers.is_empty() {
            return None;
        }
        self.require(POINTER_CONDITIONS);
        for pointer in &pointers {
            if let Some(conditions) = self.analyzer.value_conditions.get(pointer) {
                self.conditions.extend(conditions.iter().cloned());
            }
        }
        if method == "read" {
            self.require(&[
                "raw_pointer_read_requires_initialized_valid_value",
                "pointer_read_may_invalidate_source",
            ]);
            return Some(Value::merge(pointers.iter().map(|v| v.base().clone())));
        }
        if matches!(method, "as_ref" | "as_mut") {
            return Some(Value::merge(pointers.iter().map(|v| {
                if v.name == "rust:raw_pointer" {
                    Value::nested("wrapper", "rust:option", v.base().clone(), None)
                } else {
                    v.base().clone()
                }
            })));
        }
        if method == "cast" {
            self.require(&["pointer_cast_layout_compatibility_required"]);
            return Some(Value::merge(pointers));
        }
        if method == "clone" {
            return Some(Value::merge(pointers));
        }
        Some(Value::merge(pointers.iter().map(|v| {
            Value::nested("wrapper", "rust:raw_pointer", v.base().clone(), None)
        })))
    }
}
