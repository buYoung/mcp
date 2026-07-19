;; tree-sitter-sequel 0.3.x models the declared object as an object_reference child for
;; CREATE TABLE, CREATE VIEW, CREATE MATERIALIZED VIEW, and CREATE FUNCTION. The locked
;; grammar has no create_procedure node, so procedure syntax is deliberately not declared.
;; SQL has no reliable cross-dialect caller or collection semantics here.
(create_table
  (object_reference) @symbol.name) @symbol.struct

(create_view
  (object_reference) @symbol.name) @symbol.type

(create_materialized_view
  (object_reference) @symbol.name) @symbol.type

(create_function
  (object_reference) @symbol.name) @symbol.fn

;; `literal` also contains numbers, TRUE, FALSE, and NULL. Keep only grammar-confirmed string
;; spellings so low-value scalar constants do not pollute the literal search field.
((literal) @literal.string
  (#match? @literal.string "^(?:[uU]&|[nNeEbBxX])?'|^\"|^\\$|^[A-Za-z_][A-Za-z0-9_]*[[:space:]]+'"))
