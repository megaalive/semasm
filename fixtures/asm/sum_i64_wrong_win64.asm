; Deliberately wrong: returns length instead of wrapping sum.
; Microsoft x64: rcx=values, rdx=length, returns rax.
BITS 64
DEFAULT REL

global sum_i64

section .text
sum_i64:
    mov rax, rdx
    ret
