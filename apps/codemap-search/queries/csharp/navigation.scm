(using_directive) @nav.import
(invocation_expression) @nav.call
(object_creation_expression) @nav.call
(local_declaration_statement) @local.scope

(base_list (_) @local.reference)
(variable_declaration type: (_) @local.reference)
(object_creation_expression type: (_) @local.reference)
(member_access_expression name: (_) @local.reference)
(qualified_name) @local.reference

