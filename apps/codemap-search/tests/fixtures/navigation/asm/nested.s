.include "audit.inc"
.globl submit_order
submit_order:
  call map_order
  call reserve_inventory
  call persist_order
  call audit_finish
  ret
