; max_usize — intentionally WRONG: always returns a (ignores b).
; Used to verify the harness rejects incorrect implementations.
BITS 64
DEFAULT REL

global max_usize

section .text
max_usize:
    mov rax, rdi
    ret
