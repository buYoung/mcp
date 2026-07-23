(namespace_definition name: (namespace_name) @name) @definition.module
(class_declaration name: (name) @name) @definition.class
(interface_declaration name: (name) @name) @definition.interface
(trait_declaration name: (name) @name) @definition.interface
(enum_declaration name: (name) @name) @definition.enum
(function_definition name: (name) @name) @definition.function
(method_declaration name: (name) @name) @definition.method
(property_declaration
  (property_element (variable_name (name) @name))) @definition.field
(function_call_expression) @reference.call
(member_call_expression) @reference.call
(scoped_call_expression) @reference.call
(object_creation_expression (_) @reference.class)
(class_interface_clause [(name) (qualified_name)] @reference.implementation)

