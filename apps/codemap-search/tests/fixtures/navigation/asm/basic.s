.include "runtime.inc"
.globl run
run:
  call init_runtime
  callq flush_runtime
  ret
