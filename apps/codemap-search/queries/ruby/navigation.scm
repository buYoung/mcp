((call
  method: (identifier) @import_method
  arguments: (argument_list (string) @import_source)) @nav.import
 (#match? @import_method "^(require|require_relative|load)$"))

((call method: (identifier) @call_method) @nav.call
 (#not-match? @call_method "^(require|require_relative|load|attr_reader|attr_writer|attr_accessor|public|private|protected)$"))
(assignment left: [(identifier) (instance_variable) (class_variable)]) @local.scope
(method_parameters (_) @local.scope)

(constant) @local.reference
(scope_resolution) @local.reference
