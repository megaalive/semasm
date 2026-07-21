; Deliberately wrong: returns length instead of wrapping sum.
; SysV AMD64: rdi=values, rsi=length, returns rax.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    mov rax, rsi
    ret
