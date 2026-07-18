
(call_expression) @nav.call
(preproc_include) @nav.import
(
  (declaration) @local.scope
  (#not-match? @local.scope "^(export[[:space:]]+|import[[:space:]]+|module[[:space:]]+)")
)

(
  [
    (declaration)
    (labeled_statement)
    (template_type)
  ] @nav.import
  (#match? @nav.import "^(export[[:space:]]+)?import[[:space:]]+")
)
