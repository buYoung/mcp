((function_call
  name: (identifier) @import_method
  arguments: (arguments (string) @import_source)) @nav.import
 (#match? @import_method "^(require|dofile)$"))

((function_call name: (identifier) @call_name) @nav.call
 (#not-match? @call_name "^(require|dofile)$"))
(function_call name: [(dot_index_expression) (method_index_expression)]) @nav.call
(assignment_statement (variable_list name: (identifier))) @local.scope
(parameters (identifier) @local.scope)

(dot_index_expression) @local.reference
(method_index_expression) @local.reference
