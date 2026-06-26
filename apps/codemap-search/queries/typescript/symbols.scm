
;; Classes
(class_declaration
  name: (type_identifier) @symbol.name) @symbol.class

;; Functions
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

;; Methods & Constructor
(method_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @symbol.name) @symbol.method

;; Interfaces
(interface_declaration
  name: (type_identifier) @symbol.name) @symbol.interface

;; Type Aliases
(type_alias_declaration
  name: (type_identifier) @symbol.name) @symbol.type

;; Enums
(enum_declaration
  name: (identifier) @symbol.name) @symbol.enum

;; Variables
(variable_declarator
  name: (identifier) @symbol.name) @symbol.variable

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
    ]
  )
) @symbol.test

;; Literals
(string) @literal.string
(template_string) @literal.string
(number) @literal.number
[(true) (false)] @literal.boolean
(null) @literal.null
(undefined) @literal.undefined
