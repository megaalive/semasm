; sum_i64 — wrapping sum of i64 elements in values[0..length].
; Microsoft x64: rcx=values, rdx=length, returns rax.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    xor eax, eax        ; sum = 0
    test rdx, rdx
    jz .done
.loop:
    add rax, [rcx]
    add rcx, 8
    dec rdx
    jnz .loop
.done:
    ret
