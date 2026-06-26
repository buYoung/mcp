
;; Function definitions (free and out-of-line methods). `@symbol.cfn` triggers the same
;; C-style name-walk arm that also handles C++ qualified_identifier scopes.
(function_definition) @symbol.cfn

;; Function prototypes / method declarations inside class bodies.
(field_declaration
  declarator: (function_declarator)) @symbol.cfn

(declaration
  declarator: (function_declarator)) @symbol.cfn

;; Reference-returning prototypes / method declarations: `T& f();`, `T&& g();`. The
;; `declarator` field is a `reference_declarator` wrapping the `function_declarator`
;; (tree-sitter-cpp 0.23.4). The definition form is already covered by `(function_definition)`;
;; only the prototype/declaration forms need these explicit patterns. The `@symbol.cfn` arm
;; peels the reference layer via `c_declarator_name`.
(field_declaration
  declarator: (reference_declarator (function_declarator))) @symbol.cfn

(declaration
  declarator: (reference_declarator (function_declarator))) @symbol.cfn

;; Class specifier with a simple type_identifier name (skip template specializations).
(class_specifier
  name: (type_identifier) @symbol.name) @symbol.class

;; Struct and union (same grammar nodes as C).
(struct_specifier
  name: (type_identifier) @symbol.name) @symbol.struct

(union_specifier
  name: (type_identifier) @symbol.name) @symbol.struct

;; Enums and enumerators.
(enum_specifier
  name: (type_identifier) @symbol.name) @symbol.enum

(enumerator
  name: (identifier) @symbol.name) @symbol.variant

;; typedef simple alias: `typedef struct {...} Point` has type_identifier as declarator.
(type_definition
  declarator: (type_identifier) @symbol.name) @symbol.type

;; typedef function-pointer: `typedef int (*cb)(void)` has function_declarator as declarator.
;; The @symbol.cfn arm walks the declarator chain to dig out the type_identifier name.
(type_definition
  declarator: (function_declarator)) @symbol.cfn

;; using X = Y type alias.
(alias_declaration
  name: (type_identifier) @symbol.name) @symbol.type

;; Object-like macros → const; function-like macros → fn.
(preproc_def
  name: (identifier) @symbol.name) @symbol.const

(preproc_function_def
  name: (identifier) @symbol.name) @symbol.fn

;; String literals (including C++ raw string literals).
(string_literal) @literal.string
(raw_string_literal) @literal.string
