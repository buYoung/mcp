// Fixture exercising GAS assembly branch-sensitive extraction.

.globl exported_entry

.macro save_regs
  push %rbp
.endm

exported_entry:
  push %rbp
  ret

internal_label:
  ret
