; max_usize — return the larger of two unsigned sizes.
; Microsoft x64: rcx=a, rdx=b, returns rax.
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    cmp rcx, rdx
    jae .a_larger_or_equal
    mov rax, rdx
    ret
.a_larger_or_equal:
    mov rax, rcx
    ret
