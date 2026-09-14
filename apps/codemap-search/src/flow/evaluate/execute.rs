use super::*;

impl Query<'_> {
    pub(super) fn expression(&mut self, id: NodeId, frame: &mut Frame, depth: usize) -> ValueId {
        let Some(expression) = self
            .index
            .unit(&frame.key.path, frame.key.unit)
            .and_then(|unit| unit.nodes.get(id).cloned())
        else {
            return 0;
        };
        let mut location = Location {
            path: frame.key.path.clone(),
            range: expression.range,
            name: String::new(),
        };
        if depth > 32 {
            self.diagnostic(&location, "expression evaluation depth budget reached");
            return 0;
        }
        if !self.tick(&location) || !self.allowed(&location) {
            return 0;
        }
        match expression.kind {
            ExpressionKind::Read { name, binding } => {
                location.name = name.clone();
                let value = if let Some(binding) = binding {
                    self.binding_value(
                        BindingKey {
                            path: frame.key.path.clone(),
                            unit: frame.key.unit,
                            binding,
                        },
                        frame,
                        &location,
                        depth + 1,
                    )
                } else {
                    self.unbound(&name, frame, &location)
                };
                self.transfer(value, &location, "value reference", false)
            }
            ExpressionKind::Literal(value) => {
                location.name = match &value {
                    Constant::String(value) => format!("{value:?}"),
                    Constant::Number(value) => value.clone(),
                    Constant::Boolean(value) => value.to_string(),
                    Constant::Null => "null".into(),
                };
                self.value(ValueKind::Constant(value), location, Evidence::Source)
            }
            ExpressionKind::Function(function) => {
                let key = FunctionKey {
                    path: frame.key.path.clone(),
                    unit: frame.key.unit,
                    function,
                };
                let definition = self.index.location(&key).unwrap();
                let receiver = self
                    .index
                    .function(&key)
                    .unwrap()
                    .has_lexical_receiver
                    .then_some(frame.receiver)
                    .flatten();
                let mut captures = frame.values.clone();
                for (binding, value) in &mut captures {
                    if !self.index.users(binding).contains(&key) {
                        continue;
                    }
                    let is_mutated = self
                        .index
                        .unit(&binding.path, binding.unit)
                        .and_then(|unit| unit.bindings.get(binding.binding).cloned())
                        .is_some_and(|binding| binding.is_mutated);
                    if is_mutated {
                        *value = self.unknown(
                            &definition,
                            "mutable closure capture has no proven value version",
                        );
                        self.diagnostic(&definition, "mutable closure capture is unresolved");
                    }
                    self.step(
                        &self.values[*value].location.clone(),
                        &definition,
                        "closure captures value",
                        if is_mutated {
                            Evidence::Candidate
                        } else {
                            Evidence::Source
                        },
                        None,
                        true,
                    );
                }
                let closure = self.closure(key, captures, receiver);
                self.value(ValueKind::Function(closure), definition, Evidence::Source)
            }
            ExpressionKind::Tuple(items) => {
                location.name = format!("tuple ({} values)", items.len());
                let values = items
                    .iter()
                    .map(|item| self.expression(*item, frame, depth + 1))
                    .collect();
                self.value(ValueKind::Tuple(values), location, Evidence::Source)
            }
            ExpressionKind::Block(statements) => self
                .statements(&statements, frame, depth + 1)
                .unwrap_or_else(|| {
                    self.value(ValueKind::Tuple(Vec::new()), location, Evidence::Source)
                }),
            ExpressionKind::Conditional {
                condition,
                consequence,
                alternative,
            } => {
                let condition = self.expression(condition, frame, depth + 1);
                self.branches(
                    condition,
                    frame,
                    &location,
                    |query, frame, is_consequence| {
                        Some(query.expression(
                            if is_consequence {
                                consequence
                            } else {
                                alternative
                            },
                            frame,
                            depth + 1,
                        ))
                    },
                )
                .unwrap_or(0)
            }
            ExpressionKind::Logical {
                left,
                right,
                is_and,
            } => {
                let left = self.expression(left, frame, depth + 1);
                self.branches(left, frame, &location, |query, frame, is_consequence| {
                    Some(if is_consequence == is_and {
                        query.expression(right, frame, depth + 1)
                    } else {
                        left
                    })
                })
                .unwrap_or(0)
            }
            ExpressionKind::Not(operand) => {
                let operand = self.expression(operand, frame, depth + 1);
                match self.values[operand].kind {
                    ValueKind::Constant(Constant::Boolean(value)) => self.value(
                        ValueKind::Constant(Constant::Boolean(!value)),
                        location,
                        Evidence::Source,
                    ),
                    _ => self.unknown(&location, "boolean operand is unresolved"),
                }
            }
            ExpressionKind::Equal {
                left,
                right,
                is_negated,
            } => {
                let left = self.expression(left, frame, depth + 1);
                let right = self.expression(right, frame, depth + 1);
                let equal = match (&self.values[left].kind, &self.values[right].kind) {
                    (
                        ValueKind::Constant(Constant::Boolean(a)),
                        ValueKind::Constant(Constant::Boolean(b)),
                    ) => Some(a == b),
                    (
                        ValueKind::Constant(Constant::String(a)),
                        ValueKind::Constant(Constant::String(b)),
                    ) if matches!(
                        self.index
                            .unit(&frame.key.path, frame.key.unit)
                            .unwrap()
                            .language
                            .as_str(),
                        "rust" | "go" | "python" | "javascript" | "typescript"
                    ) =>
                    {
                        Some(a == b)
                    }
                    (
                        ValueKind::Constant(Constant::Number(a)),
                        ValueKind::Constant(Constant::Number(b)),
                    ) if a == b => Some(true),
                    _ => None,
                };
                equal
                    .map(|equal| {
                        self.value(
                            ValueKind::Constant(Constant::Boolean(equal != is_negated)),
                            location.clone(),
                            Evidence::Source,
                        )
                    })
                    .unwrap_or_else(|| {
                        self.unknown(&location, "comparison/coercion semantics are unresolved")
                    })
            }
            ExpressionKind::Object { class, fields } => {
                let language = self
                    .index
                    .unit(&frame.key.path, frame.key.unit)
                    .unwrap()
                    .language
                    .clone();
                if matches!(language.as_str(), "c" | "cpp" | "go" | "rust") {
                    return self.unknown(
                        &location,
                        "aggregate copy/reference semantics are outside the bounded object summary",
                    );
                }
                location.name = class.clone().unwrap_or_else(|| "returned object".into());
                let class = class.and_then(|name| {
                    self.index
                        .unit(&frame.key.path, frame.key.unit)?
                        .classes
                        .iter()
                        .position(|class| class.name == name)
                        .map(|class| (frame.key.path.clone(), frame.key.unit, class))
                });
                let object = self.objects.len();
                self.objects.push(Object {
                    location: location.clone(),
                    fields: BTreeMap::new(),
                    class,
                    is_map: false,
                    is_invalidated: false,
                });
                for (name, value) in fields {
                    let value = self.expression(value, frame, depth + 1);
                    let field = Location {
                        name: format!("field {name}"),
                        ..location.clone()
                    };
                    let value = self.transfer(value, &field, "value → object field", true);
                    self.objects[object]
                        .fields
                        .insert(Constant::String(name), value);
                }
                self.value(ValueKind::Object(object), location, Evidence::Source)
            }
            ExpressionKind::Field { object, key } => {
                let object = self.expression(object, frame, depth + 1);
                let key = self.expression(key, frame, depth + 1);
                self.field(object, key, frame, &location, depth + 1)
            }
            ExpressionKind::Call { callee, arguments } => {
                self.call(callee, &arguments, false, frame, &location, depth + 1)
            }
            ExpressionKind::Construct { callee, arguments } => {
                self.call(callee, &arguments, true, frame, &location, depth + 1)
            }
            ExpressionKind::Unknown { reason, inputs } => {
                for input in inputs {
                    self.expression(input, frame, depth + 1);
                }
                self.unknown(&location, &reason)
            }
        }
    }
    fn field(
        &mut self,
        object: ValueId,
        key: ValueId,
        frame: &mut Frame,
        location: &Location,
        depth: usize,
    ) -> ValueId {
        let ValueKind::Constant(key) = self.values[key].kind.clone() else {
            return self.unknown(location, "dynamic field key unresolved");
        };
        if matches!(
            self.values[object].kind,
            ValueKind::Tuple(_) | ValueKind::Alternatives(_)
        ) {
            let index = match &key {
                Constant::Number(value) | Constant::String(value) => value.parse::<usize>().ok(),
                _ => None,
            };
            let location = Location {
                name: format!(
                    "{}[{}]",
                    self.values[object].location.name,
                    index.map_or_else(|| "?".into(), |index| index.to_string())
                ),
                ..location.clone()
            };
            return index
                .map(|index| self.tuple_item(object, index, &location))
                .unwrap_or_else(|| self.unknown(&location, "tuple index is unresolved"));
        }
        let key_name = match &key {
            Constant::String(name) => name.clone(),
            Constant::Number(name) => name.clone(),
            _ => return self.unknown(location, "field key is not a static property name"),
        };
        let location = Location {
            name: format!("{}.{}", self.values[object].location.name, key_name),
            ..location.clone()
        };
        match self.values[object].kind.clone() {
            ValueKind::Object(id) => {
                if self.objects[id].is_invalidated {
                    return self.unknown(
                        &location,
                        "object may have been mutated by an unresolved call",
                    );
                }
                if self.objects[id].is_map
                    && self.should_model_collections
                    && !self.objects[id].fields.contains_key(&key)
                    && matches!(key_name.as_str(), "get" | "set" | "delete")
                {
                    return self.value(
                        ValueKind::MapMethod {
                            object: id,
                            method: key_name,
                        },
                        location,
                        Evidence::Model,
                    );
                }
                if let Some(&value) = self.objects[id].fields.get(&key) {
                    if self
                        .field_writes
                        .get(&(id, key.clone()))
                        .is_some_and(|context| *context != 0 && *context != self.root_context)
                    {
                        self.step(&self.values[value].location.clone(), &location, "possible field value", Evidence::Candidate, Some("write and read belong to separate call contexts; order is not proven".into()), true);
                        return self.unknown(
                            &location,
                            "field value version across call contexts is unresolved",
                        );
                    }
                    return self.transfer(value, &location, "object field → value", true);
                }
                if let Some((path, unit, class)) = self.objects[id].class.clone() {
                    if !self.should_model_instances {
                        return self
                            .unknown(&location, "class/prototype identity may have been modified");
                    }
                    return self.method(&path, unit, class, &key_name, frame, &location);
                }
                self.unknown(
                    &location,
                    "object member value is not present in the bounded summary",
                )
            }
            ValueKind::Class { path, unit, class } => {
                if key_name == "prototype" {
                    self.should_model_instances = false;
                }
                if !self.should_model_instances {
                    return self.unknown(&location, "class/prototype semantics are unresolved");
                }
                let is_static = self.index.unit(&path, unit).is_some_and(|unit| {
                    unit.classes[class]
                        .methods
                        .get(&key_name)
                        .is_some_and(|id| unit.functions[*id].is_static)
                });
                if !is_static {
                    return self.unknown(
                        &location,
                        "instance method requires a source-proven receiver",
                    );
                }
                self.method(&path, unit, class, &key_name, frame, &location)
            }
            ValueKind::Namespace { path, unit } => {
                self.export_value(&path, unit, &key_name, frame, &location, depth + 1)
            }
            ValueKind::MapConstructor if key_name == "prototype" => {
                self.value(ValueKind::MapPrototype, location, Evidence::Model)
            }
            ValueKind::Unknown(reason) => {
                self.step(
                    &self.values[object].location.clone(),
                    &location,
                    "opaque result → member use",
                    Evidence::Source,
                    Some(reason.clone()),
                    true,
                );
                self.diagnostic(
                    &location,
                    &format!("member use observed; receiver semantics unresolved: {reason}"),
                );
                self.unknown(&location, &format!("receiver value unavailable: {reason}"))
            }
            _ => self.unknown(&location, "receiver instance/type is not source-proven"),
        }
    }
    fn method(
        &mut self,
        path: &str,
        unit: usize,
        class: usize,
        name: &str,
        frame: &Frame,
        location: &Location,
    ) -> ValueId {
        let Some(function) = self.index.unit(path, unit).and_then(|unit| {
            unit.classes
                .get(class)
                .and_then(|class| class.methods.get(name))
                .copied()
        }) else {
            return self.unknown(
                location,
                "method has no unique source implementation on this object",
            );
        };
        let key = FunctionKey {
            path: path.into(),
            unit,
            function,
        };
        let definition = self.index.location(&key).unwrap();
        if !self.allowed(&definition) {
            return self.unknown(location, "method definition unavailable");
        }
        if !self
            .index
            .function(&key)
            .is_some_and(|function| function.is_available)
        {
            return self.unknown(
                location,
                "method modifiers/body or overload resolution is unsupported",
            );
        }
        let closure = self.closure(key, frame.values.clone(), None);
        self.value(ValueKind::Function(closure), definition, Evidence::Source)
    }
    fn call(
        &mut self,
        callee: usize,
        arguments: &[usize],
        is_constructor: bool,
        frame: &mut Frame,
        location: &Location,
        depth: usize,
    ) -> ValueId {
        let mut receiver = None;
        let callable = if let Some(Expression {
            kind: ExpressionKind::Field { object, key },
            ..
        }) = self
            .index
            .unit(&frame.key.path, frame.key.unit)
            .and_then(|unit| unit.nodes.get(callee).cloned())
        {
            let object = self.expression(object, frame, depth + 1);
            let key = self.expression(key, frame, depth + 1);
            receiver = Some(object);
            self.field(object, key, frame, location, depth + 1)
        } else {
            self.expression(callee, frame, depth + 1)
        };
        let arguments = arguments
            .iter()
            .map(|argument| self.expression(*argument, frame, depth + 1))
            .collect::<Vec<_>>();
        let location = Location {
            name: format!("call {}", self.values[callable].location.name),
            ..location.clone()
        };
        if matches!(self.values[callable].kind, ValueKind::Alternatives(_))
            || self.values[callable].evidence == Evidence::Candidate
                && matches!(self.values[callable].kind, ValueKind::Function(_))
        {
            self.conditional_call(callable, &location);
            for argument in &arguments {
                self.invalidate_escaped(*argument, &location);
            }
            if let Some(receiver) = receiver {
                self.invalidate_escaped(receiver, &location);
            }
            return self.unknown(
                &location,
                "conditional call target; branch selection is unresolved",
            );
        }
        match self.values[callable].kind.clone() {
            ValueKind::Function(closure) => {
                if is_constructor {
                    return self.unknown(
                        &location,
                        "function-as-constructor semantics are not summarized",
                    );
                }
                self.invoke(closure, arguments, receiver, &location)
            }
            ValueKind::Class { path, unit, class } => {
                let language = self.index.unit(&path, unit).unwrap().language.clone();
                if !matches!(language.as_str(), "typescript" | "javascript")
                    || !self.should_model_instances
                {
                    self.diagnostic(&location, "instance layout/copy/descriptor semantics are outside the bounded object summary");
                    return self.unknown(
                        &location,
                        "instance layout/copy/descriptor semantics are outside the bounded object summary",
                    );
                }
                if !is_constructor {
                    return self.unknown(
                        &location,
                        "class invocation does not prove instance construction",
                    );
                }
                let definition = self.index.unit(&path, unit).unwrap().classes[class].clone();
                if !definition.is_available {
                    return self.unknown(
                        &location,
                        "class construction, inheritance or metaclass semantics are unresolved",
                    );
                }
                let class_location = Location {
                    path: path.clone(),
                    range: definition.range,
                    name: definition.name,
                };
                if !self.allowed(&class_location) {
                    return self.unknown(&location, "constructor source unavailable");
                }
                let object = self.objects.len();
                self.objects.push(Object {
                    location: location.clone(),
                    fields: BTreeMap::new(),
                    class: Some((path.clone(), unit, class)),
                    is_map: false,
                    is_invalidated: false,
                });
                let value = self.value(
                    ValueKind::Object(object),
                    location.clone(),
                    Evidence::Source,
                );
                let mut initializer_frame = Frame {
                    key: FunctionKey {
                        path: path.clone(),
                        unit,
                        function: 0,
                    },
                    values: frame.values.clone(),
                    receiver: Some(value),
                };
                for (field, initializer) in definition.fields {
                    let field_value = self.expression(initializer, &mut initializer_frame, 0);
                    self.objects[object]
                        .fields
                        .insert(Constant::String(field), field_value);
                }
                if let Some(function) = definition.constructor {
                    let closure = self.closure(
                        FunctionKey {
                            path,
                            unit,
                            function,
                        },
                        frame.values.clone(),
                        None,
                    );
                    let result = self.invoke(closure, arguments, Some(value), &location);
                    if matches!(language.as_str(), "typescript" | "javascript")
                        && matches!(
                            self.values[result].kind,
                            ValueKind::Object(_) | ValueKind::Function(_)
                        )
                    {
                        return result;
                    }
                    if matches!(&self.values[result].kind, ValueKind::Unknown(reason) if reason != "no summarized return value")
                    {
                        self.objects[object].is_invalidated = true;
                    }
                } else if !arguments.is_empty() {
                    self.diagnostic(
                        &location,
                        "implicit/external constructor argument semantics are unresolved",
                    );
                }
                value
            }
            ValueKind::MapConstructor if is_constructor => {
                if !arguments.is_empty() {
                    return self.unknown(
                        &location,
                        "Map iterable initialization is outside the built-in summary",
                    );
                }
                let object = self.objects.len();
                self.objects.push(Object {
                    location: location.clone(),
                    fields: BTreeMap::new(),
                    class: None,
                    is_map: true,
                    is_invalidated: false,
                });
                self.value(ValueKind::Object(object), location, Evidence::Model)
            }
            ValueKind::MapMethod { object, method } => {
                if !receiver.is_some_and(|receiver| matches!(self.values[receiver].kind, ValueKind::Object(id) if id == object)) {
                    return self.unknown(&location, "detached Map method has no source-proven receiver");
                }
                self.map_call(object, &method, &arguments, &location)
            }
            ValueKind::DeferredRead(read) => {
                self.callback_uses.push(CallbackUse {
                    read,
                    location: location.clone(),
                });
                self.unknown(&location,"collection callback invocation target is a candidate until contents are proven")
            }
            ValueKind::Unknown(reason) => {
                for (i, &argument) in arguments.iter().enumerate() {
                    let value = self.values[argument].clone();
                    let is_function = matches!(value.kind, ValueKind::Function(_));
                    let origin = if let ValueKind::Function(closure) = value.kind {
                        self.index
                            .location(&self.closures[closure].key)
                            .unwrap_or_else(|| value.location.clone())
                    } else {
                        value.location.clone()
                    };
                    let target = Location {
                        name: format!("argument {} of {}", i + 1, location.name),
                        ..location.clone()
                    };
                    self.step(
                        &origin,
                        &target,
                        if is_function {
                            "function value passed"
                        } else {
                            "argument passed"
                        },
                        Evidence::Source,
                        None,
                        is_function,
                    );
                    if is_function {
                        self.diagnostic(&target,&format!("callback passing observed; invocation/parameter/return semantics unresolved: {reason}"));
                    }
                    self.invalidate_escaped(argument, &target);
                }
                if let Some(receiver) = receiver {
                    self.invalidate_escaped(receiver, &location);
                }
                self.unknown(
                    &location,
                    &format!("return semantics unavailable: {reason}"),
                )
            }
            ValueKind::Parameter => {
                self.diagnostic(&location,"call target is an unbound function parameter; passing is not execution evidence");
                self.unknown(
                    &location,
                    "function parameter has no concrete callable value in this context",
                )
            }
            _ => self.unknown(&location, "call target cannot be established from source"),
        }
    }
    pub(super) fn invoke(
        &mut self,
        closure: usize,
        mut arguments: Vec<ValueId>,
        receiver: Option<ValueId>,
        location: &Location,
    ) -> ValueId {
        let closure = self.closures[closure].clone();
        let Some(function) = self.index.function(&closure.key) else {
            return 0;
        };
        let definition = self.index.location(&closure.key).unwrap();
        if self.stack.len() >= CALL_DEPTH || self.stack.contains(&closure.key) {
            self.diagnostic(
                location,
                "recursive/deep function summary stopped at the call boundary",
            );
            return self.unknown(location, "call-summary depth/recursion budget");
        }
        if !self.tick(location) {
            return 0;
        }
        if !self.allowed(&definition) {
            return self.unknown(location, "callee source is stale/excluded/unavailable");
        }
        if !function.is_available {
            self.diagnostic(
                &definition,
                "function body, modifiers or parameter syntax is unavailable for summarization",
            );
            return self.unknown(location, "opaque function summary");
        }
        let is_rust = self
            .index
            .unit(&closure.key.path, closure.key.unit)
            .unwrap()
            .language
            == "rust";
        let condition = if is_rust {
            self.resolver().reset_dependency_tracking();
            let condition = self
                .resolver()
                .condition_at(&closure.key.path, &function.range);
            for (path, range) in self.resolver().source_dependencies() {
                if !self.allowed(&Location {
                    path,
                    range,
                    name: "conditional/module evidence".into(),
                }) {
                    return self.unknown(
                        location,
                        "conditional/module source evidence is stale or unavailable",
                    );
                }
            }
            condition
        } else {
            Some(true)
        };
        if condition != Some(true) {
            self.diagnostic(
                &definition,
                "Rust cfg/module activation is unresolved or inactive",
            );
            return self.unknown(
                location,
                "conditional function is not active in the analysis target",
            );
        }
        let parameter_names = function
            .parameters
            .iter()
            .map(|id| {
                self.index
                    .unit(&closure.key.path, closure.key.unit)
                    .unwrap()
                    .bindings[*id]
                    .name
                    .clone()
            })
            .collect::<Vec<_>>();
        let receiver = if function.has_lexical_receiver {
            closure.receiver
        } else {
            receiver
        };
        if arguments.len() + 1 == function.parameters.len()
            && parameter_names
                .first()
                .is_some_and(|name| matches!(name.as_str(), "self" | "this"))
        {
            if let Some(receiver) = receiver {
                arguments.insert(0, receiver);
            }
        }
        if arguments.len() != function.parameters.len() {
            self.diagnostic(
                location,
                "argument arity/default/variadic mapping is unresolved",
            );
            return self.unknown(location, "argument-to-parameter mapping is not proven");
        }
        if definition.range != location.range || definition.path != location.path {
            self.step(
                &definition,
                location,
                "source-resolved call",
                Evidence::Source,
                None,
                definition.path != location.path
                    || location.name != format!("call {}", function.name),
            );
        }
        let mut frame = Frame {
            key: closure.key.clone(),
            values: closure.captures,
            receiver,
        };
        for (position, (parameter, mut value)) in
            function.parameters.iter().zip(arguments).enumerate()
        {
            let binding = &self
                .index
                .unit(&frame.key.path, frame.key.unit)
                .unwrap()
                .bindings[*parameter];
            let parameter_location = Location {
                path: frame.key.path.clone(),
                range: binding.range.clone(),
                name: format!("{} parameter {}", function.name, binding.name),
            };
            let language = self
                .index
                .unit(&frame.key.path, frame.key.unit)
                .unwrap()
                .language
                .clone();
            if matches!(language.as_str(), "c" | "cpp" | "go" | "rust")
                && matches!(self.values[value].kind, ValueKind::Object(_))
            {
                value = self.unknown(
                    &parameter_location,
                    "object copy/reference parameter semantics are unresolved",
                );
                self.diagnostic(
                    &parameter_location,
                    "object copy/reference parameter semantics are unresolved",
                );
            }
            let is_function = matches!(self.values[value].kind, ValueKind::Function(_));
            let is_concrete = is_function
                || matches!(
                    self.values[value].kind,
                    ValueKind::Constant(_) | ValueKind::Object(_)
                );
            let value = self.transfer(
                value,
                &parameter_location,
                &format!("argument {} → parameter", position + 1),
                is_concrete,
            );
            frame.values.insert(
                BindingKey {
                    path: frame.key.path.clone(),
                    unit: frame.key.unit,
                    binding: *parameter,
                },
                value,
            );
        }
        self.executed.insert(frame.key.clone());
        self.stack.push(frame.key.clone());
        let result = self.statements(&function.body, &mut frame, 0);
        self.stack.pop();
        if let Some(value) = result {
            let is_transfer=function.body.iter().any(|statement|matches!(statement.kind,StatementKind::Return(id) if self.index.unit(&frame.key.path,frame.key.unit).and_then(|unit|unit.nodes.get(id).cloned()).is_some_and(|node|!matches!(node.kind,ExpressionKind::Literal(_)|ExpressionKind::Unknown{..}))));
            self.transfer(value, location, "returned value → call result", is_transfer)
        } else {
            self.unknown(location, "no summarized return value")
        }
    }
    fn statements(
        &mut self,
        statements: &[Statement],
        frame: &mut Frame,
        depth: usize,
    ) -> Option<ValueId> {
        for (position, statement) in statements.iter().enumerate() {
            let location = Location {
                path: frame.key.path.clone(),
                range: statement.range.clone(),
                name: "statement".into(),
            };
            if !self.tick(&location) {
                return Some(0);
            }
            match &statement.kind {
                StatementKind::Branch {
                    condition,
                    consequence,
                    alternative,
                } => {
                    let condition = self.expression(*condition, frame, depth + 1);
                    if let ValueKind::Constant(Constant::Boolean(is_consequence)) =
                        self.values[condition].kind
                    {
                        if let Some(value) = self.statements(
                            if is_consequence {
                                consequence
                            } else {
                                alternative
                            },
                            frame,
                            depth + 1,
                        ) {
                            return Some(value);
                        }
                    } else {
                        return self.branches(
                            condition,
                            frame,
                            &location,
                            |query, frame, is_consequence| {
                                query
                                    .statements(
                                        if is_consequence {
                                            consequence
                                        } else {
                                            alternative
                                        },
                                        frame,
                                        depth + 1,
                                    )
                                    .or_else(|| {
                                        query.statements(
                                            &statements[position + 1..],
                                            frame,
                                            depth + 1,
                                        )
                                    })
                            },
                        );
                    }
                }
                StatementKind::Destructure { bindings, value } => {
                    let value = self.expression(*value, frame, depth + 1);
                    for (binding, path) in bindings {
                        let mut item = value;
                        for &index in path {
                            item = self.tuple_item(item, index, &location);
                        }
                        let name = self
                            .index
                            .unit(&frame.key.path, frame.key.unit)
                            .unwrap()
                            .bindings[*binding]
                            .name
                            .clone();
                        let item = self.transfer(
                            item,
                            &Location {
                                name,
                                ..location.clone()
                            },
                            "tuple → binding",
                            true,
                        );
                        let key = BindingKey {
                            path: frame.key.path.clone(),
                            unit: frame.key.unit,
                            binding: *binding,
                        };
                        frame.values.insert(key.clone(), item);
                        if frame.key.function == 0 {
                            self.globals.insert(key, item);
                        }
                    }
                }
                StatementKind::Bind { binding, value } => {
                    let value = self.expression(*value, frame, depth + 1);
                    let name = self
                        .index
                        .unit(&frame.key.path, frame.key.unit)
                        .unwrap()
                        .bindings[*binding]
                        .name
                        .clone();
                    let location = Location { name, ..location };
                    let value = self.transfer(value, &location, "value → binding", false);
                    let key = BindingKey {
                        path: frame.key.path.clone(),
                        unit: frame.key.unit,
                        binding: *binding,
                    };
                    frame.values.insert(key.clone(), value);
                    if frame.key.function == 0 {
                        self.globals.insert(key, value);
                    }
                }
                StatementKind::Assign { target, value } => {
                    let value = self.expression(*value, frame, depth + 1);
                    self.assign(*target, value, frame, &location, depth + 1);
                }
                StatementKind::Return(value) => {
                    let interesting = self
                        .index
                        .unit(&frame.key.path, frame.key.unit)
                        .and_then(|unit| unit.nodes.get(*value).cloned())
                        .is_some_and(|node| {
                            !matches!(
                                node.kind,
                                ExpressionKind::Literal(_) | ExpressionKind::Unknown { .. }
                            )
                        });
                    let value = self.expression(*value, frame, depth + 1);
                    if let ValueKind::Unknown(reason) = self.values[value].kind.clone() {
                        self.diagnostic(&location, &reason);
                    }
                    return Some(self.transfer(
                        value,
                        &Location {
                            name: "return".into(),
                            ..location
                        },
                        "value → return",
                        interesting,
                    ));
                }
                StatementKind::Evaluate(value) => {
                    self.expression(*value, frame, depth + 1);
                }
                StatementKind::Barrier {
                    reason,
                    expressions,
                } => {
                    for expression in expressions {
                        self.expression(*expression, frame, depth + 1);
                    }
                    self.analysis_limit(&location, reason);
                    return Some(self.unknown(&location, reason));
                }
            }
        }
        None
    }
    fn assign(
        &mut self,
        target: usize,
        value: ValueId,
        frame: &mut Frame,
        location: &Location,
        depth: usize,
    ) {
        let Some(target) = self
            .index
            .unit(&frame.key.path, frame.key.unit)
            .and_then(|unit| unit.nodes.get(target).cloned())
        else {
            return;
        };
        match target.kind {
            ExpressionKind::Read {
                binding: Some(binding),
                name,
            } => {
                let value = self.transfer(
                    value,
                    &Location {
                        name,
                        ..location.clone()
                    },
                    "reassignment → binding",
                    false,
                );
                let key = BindingKey {
                    path: frame.key.path.clone(),
                    unit: frame.key.unit,
                    binding,
                };
                frame.values.insert(key.clone(), value);
                if self.globals.contains_key(&key) {
                    self.globals.insert(key, value);
                }
            }
            ExpressionKind::Field {
                object: object_node,
                key,
            } => {
                let object = self.expression(object_node, frame, depth + 1);
                let key = self.expression(key, frame, depth + 1);
                if matches!(
                    self.values[object].kind,
                    ValueKind::Tuple(_) | ValueKind::Alternatives(_)
                ) {
                    let index = match &self.values[key].kind {
                        ValueKind::Constant(Constant::Number(value) | Constant::String(value)) => {
                            value.parse::<usize>().ok()
                        }
                        _ => None,
                    };
                    let updated = match (self.values[object].kind.clone(), index) {
                        (ValueKind::Tuple(mut items), Some(index))
                            if index < items.len()
                                && self
                                    .index
                                    .unit(&frame.key.path, frame.key.unit)
                                    .unwrap()
                                    .language
                                    == "rust" =>
                        {
                            items[index] = value;
                            self.value(ValueKind::Tuple(items), location.clone(), Evidence::Source)
                        }
                        _ => self.unknown(location, "tuple mutation target is unresolved"),
                    };
                    self.assign(object_node, updated, frame, location, depth + 1);
                    return;
                }
                if matches!(self.values[object].kind, ValueKind::Class { .. }) {
                    self.should_model_instances = false;
                    self.diagnostic(
                        location,
                        "class member mutation invalidates instance summaries",
                    );
                    return;
                }
                if matches!(
                    self.values[object].kind,
                    ValueKind::MapConstructor | ValueKind::MapPrototype
                ) {
                    self.should_model_collections = false;
                    self.diagnostic(
                        location,
                        "Map built-in/prototype mutation invalidates the collection model",
                    );
                    return;
                }
                if let (ValueKind::Object(object), ValueKind::Constant(key)) = (
                    self.values[object].kind.clone(),
                    self.values[key].kind.clone(),
                ) {
                    if key == Constant::String("__proto__".into()) {
                        self.objects[object].is_invalidated = true;
                        self.diagnostic(
                            location,
                            "prototype mutation invalidates this object summary",
                        );
                        return;
                    }
                    let value = self.transfer(
                        value,
                        &Location {
                            name: format!("field {key:?}"),
                            ..location.clone()
                        },
                        "value → object field",
                        true,
                    );
                    let context = if self.loading.is_empty() {
                        self.root_context
                    } else {
                        0
                    };
                    self.field_writes.insert((object, key.clone()), context);
                    self.objects[object].fields.insert(key, value);
                } else {
                    if let ValueKind::Object(object) = self.values[object].kind {
                        self.objects[object].is_invalidated = true;
                    }
                    self.diagnostic(
                        location,
                        "field assignment receiver/key identity is unresolved",
                    );
                }
            }
            _ => self.diagnostic(location, "assignment target is not source-proven"),
        }
    }
    fn invalidate_escaped(&mut self, value: ValueId, location: &Location) {
        let mut pending = vec![value];
        let mut seen = HashSet::new();
        while let Some(value) = pending.pop() {
            if !seen.insert(value) {
                continue;
            }
            if !self.tick(location) {
                break;
            }
            match self.values[value].kind {
                ValueKind::Class { .. } => self.should_model_instances = false,
                ValueKind::Tuple(ref items) | ValueKind::Alternatives(ref items) => {
                    pending.extend(items.iter().copied())
                }
                ValueKind::Object(object) => {
                    self.objects[object].is_invalidated = true;
                    pending.extend(self.objects[object].fields.values().copied());
                }
                ValueKind::Function(closure) => {
                    pending.extend(self.closures[closure].captures.values().copied())
                }
                _ => {}
            }
        }
    }
    fn map_call(
        &mut self,
        object: usize,
        method: &str,
        arguments: &[ValueId],
        location: &Location,
    ) -> ValueId {
        let key = arguments.first().and_then(|value| {
            if let ValueKind::Constant(value) = &self.values[*value].kind {
                Some(value.clone())
            } else {
                None
            }
        });
        match (method, arguments) {
            ("set", [_, value]) => {
                self.stores.push(Store {
                    conditions: self.conditions.clone(),
                    object,
                    key,
                    value: *value,
                    location: location.clone(),
                });
                self.step(
                    &self.values[*value].location.clone(),
                    location,
                    "value → collection store",
                    Evidence::Model,
                    Some(
                        "built-in JavaScript Map.set summary; receiver allocation is source-proven"
                            .into(),
                    ),
                    true,
                );
                self.value(ValueKind::Object(object), location.clone(), Evidence::Model)
            }
            ("get", [_]) => {
                let read = self.reads.len();
                self.reads.push(Read {
                    conditions: self.conditions.clone(),
                    object,
                    key,
                    location: location.clone(),
                });
                self.step(
                    &self.objects[object].location.clone(),
                    location,
                    "collection lookup",
                    Evidence::Model,
                    Some("built-in JavaScript Map.get summary; contents/order not assumed".into()),
                    true,
                );
                self.value(
                    ValueKind::DeferredRead(read),
                    location.clone(),
                    Evidence::Candidate,
                )
            }
            ("delete", [_]) => self.unknown(
                location,
                "collection removal observed; registration lifetime is not simulated",
            ),
            _ => self.unknown(location, "built-in collection argument mapping unresolved"),
        }
    }
}
