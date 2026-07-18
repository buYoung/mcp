(
  (instruction
    kind: (word) @asm.call.kind) @nav.call
  (#match? @asm.call.kind "^(call|callq|bl|blx|jal|jalr)$")
)

(
  (meta
    kind: (meta_ident) @asm.import.kind) @nav.import
  (#eq? @asm.import.kind ".include")
)
