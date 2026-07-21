; min_usize — return the smaller of two unsigned sizes.
; SysV AMD64: rdi=a, rsi=b, returns rax.
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    cmp rdi, rsi
    jbe .a_smaller_or_equal
    mov rax, rsi
    ret
.a_smaller_or_equal:
    mov rax, rdi
    ret
