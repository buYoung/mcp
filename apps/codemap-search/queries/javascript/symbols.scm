;; Runtime declarations supported by the official JavaScript grammar.
(class_declaration
  name: (_) @symbol.name) @symbol.class

(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

(generator_function_declaration
  name: (identifier) @symbol.name) @symbol.fn

(method_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @symbol.name) @symbol.method

;; Variable-backed functions/classes are refined from the initializer in JavaScriptSpec.
(variable_declarator
  name: (identifier) @symbol.name) @symbol.variable

(assignment_expression
  left: [
    (identifier) @symbol.name
    (member_expression
      property: (property_identifier) @symbol.name)
  ]
  right: [
    (arrow_function)
    (function_expression)
  ]) @symbol.fn

(pair
  key: (property_identifier) @symbol.name
  value: [
    (arrow_function)
    (function_expression)
  ]) @symbol.fn

;; Test Call Expressions
(call_expression
  function: [
    (identifier) @fn_name
    (member_expression
      object: (identifier) @fn_name)
  ]
  arguments: (arguments
    [
      (string) @symbol.name
      (template_string) @symbol.name
    ])
) @symbol.test

(string) @literal.string
(template_string) @literal.string
(number) @literal.number
[(true) (false)] @literal.boolean
(null) @literal.null
(undefined) @literal.undefined
