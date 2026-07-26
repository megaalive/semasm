; max_i64 — signed max via cmov (no branch).
; SysV AMD64: rdi=a, rsi=b, returns rax.
BITS 64
DEFAULT REL

global max_i64

section .text
max_i64:
    mov rax, rdi
    cmp rsi, rax
    cmovg rax, rsi
    ret
