bits 64
section .data
Primary:
    dq Callback ; @S1
Secondary:
    dq Other ; @S2
section .text
Fire:
    mov rbx, [Primary]
    mov rax, [Secondary]
    call rbx ; @I1
Clobbered:
    mov rbx, [Primary]
    xor ebx, ebx
    call rbx ; @I2
Copied:
    mov rax, [Primary]
    mov rbx, rax
    call rbx ; @I3
NarrowWrite:
    mov rbx, [Primary]
    mov bl, 0
    call rbx ; @I4
Callback:
    ret
Other:
    ret
