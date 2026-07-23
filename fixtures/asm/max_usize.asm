; max_usize — return the larger of two unsigned sizes.
; SysV AMD64: rdi=a, rsi=b, returns rax.
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    cmp rdi, rsi
    jae .a_larger_or_equal
    mov rax, rsi
    ret
.a_larger_or_equal:
    mov rax, rdi
    ret
