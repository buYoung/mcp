(label) @definition.function

;; Instruction/macro names are searchable references, without claiming a callable
;; declaration or a call edge for ENTRY/EALIGN and other preprocessor invocations.
(instruction kind: (word) @reference.instruction)

(
  (instruction
    kind: (word) @asm.call.kind) @reference.call
  (#match? @asm.call.kind "^(call|callq|bl|blx|jal|jalr)$")
)
