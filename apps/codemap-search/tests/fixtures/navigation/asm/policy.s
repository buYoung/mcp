.include "rules.inc"
.globl evaluate_policy
evaluate_policy:
  call filter_rules
  call check_rule
  call map_names
  call object_keys
  ret
