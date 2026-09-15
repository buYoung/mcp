bits 64
section .data
Handlers:
    dq Callback ; S
section .text
Fire:
    mov rax, [Handlers]
    call rax ; I
Callback:
    ret
