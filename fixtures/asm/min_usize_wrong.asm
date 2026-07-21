; min_usize — intentionally WRONG: always returns a (ignores b).
; Used to verify the harness rejects incorrect implementations.
BITS 64
DEFAULT REL

global min_usize

section .text
min_usize:
    mov rax, rdi
    ret
