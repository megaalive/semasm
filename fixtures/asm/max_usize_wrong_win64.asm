; max_usize — intentionally WRONG (Win64): always returns a (ignores b).
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    mov rax, rcx
    ret
