; min_usize — intentionally WRONG (Win64): always returns a (ignores b).
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    mov rax, rcx
    ret
