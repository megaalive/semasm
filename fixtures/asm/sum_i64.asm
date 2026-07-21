; sum_i64 — wrapping sum of i64 elements in values[0..length].
; SysV AMD64: rdi=values, rsi=length, returns rax.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    xor eax, eax        ; sum = 0
    test rsi, rsi
    jz .done
.loop:
    add rax, [rdi]
    add rdi, 8
    dec rsi
    jnz .loop
.done:
    ret
