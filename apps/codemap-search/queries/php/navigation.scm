(namespace_use_declaration) @nav.import
(include_expression (string)) @nav.import
(include_once_expression (string)) @nav.import
(require_expression (string)) @nav.import
(require_once_expression (string)) @nav.import

(function_call_expression) @nav.call
(member_call_expression) @nav.call
(nullsafe_member_call_expression) @nav.call
(scoped_call_expression) @nav.call
(object_creation_expression) @nav.call

(simple_parameter) @local.scope
(variadic_parameter) @local.scope
(static_variable_declaration) @local.scope

(qualified_name) @local.reference
(relative_name) @local.reference
(class_constant_access_expression) @local.reference

