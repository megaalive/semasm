; min_usize — return the smaller of two unsigned sizes.
; Microsoft x64: rcx=a, rdx=b, returns rax.
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    cmp rcx, rdx
    jbe .a_smaller_or_equal
    mov rax, rdx
    ret
.a_smaller_or_equal:
    mov rax, rcx
    ret
