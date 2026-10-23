
;; Classes
(class_declaration
  name: (type_identifier) @symbol.name) @symbol.class

(abstract_class_declaration
  name: (type_identifier) @symbol.name) @symbol.class

;; Functions
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

(function_signature
  name: (identifier) @symbol.name) @symbol.fn

;; Methods & Constructor
(method_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @symbol.name) @symbol.method

(method_signature
  name: (property_identifier) @symbol.name) @symbol.method

(abstract_method_signature
  name: (property_identifier) @symbol.name) @symbol.method

(module
  name: (identifier) @symbol.name) @symbol.mod

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

;; Data members: related-terms evidence only (`member_names`), not indexed as symbols.
(public_field_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @member.name)

(property_signature
  name: (property_identifier) @member.name)

;; Constructor parameter properties (`private readonly service: Service`).
(required_parameter
  (accessibility_modifier)
  pattern: (identifier) @member.name)

(optional_parameter
  (accessibility_modifier)
  pattern: (identifier) @member.name)

(enum_body
  name: (property_identifier) @member.name)

(enum_assignment
  name: (property_identifier) @member.name)

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
