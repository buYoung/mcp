bits 64
section .data
Handlers:
    dq Callback ; S
section .text
Fire:
    mov rbx, [Handlers]
    call rbx ; I
Callback:
    ret
