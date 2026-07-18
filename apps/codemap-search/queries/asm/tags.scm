(label) @definition.function

(
  (instruction
    kind: (word) @asm.call.kind) @reference.call
  (#match? @asm.call.kind "^(call|callq|bl|blx|jal|jalr)$")
)
