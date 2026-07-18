
(function_definition) @definition.function
(call_expression) @reference.call

(
  (declaration) @definition.module
  (#match? @definition.module "^(export[[:space:]]+)?module[[:space:]]+")
)

(
  [
    (declaration)
    (labeled_statement)
    (template_type)
  ] @reference.module
  (#match? @reference.module "^(export[[:space:]]+)?import[[:space:]]+")
)
