; count_byte — intentionally WRONG: always returns length (ignores needle).
; Used to verify the harness rejects incorrect implementations.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    mov rax, rsi        ; return length regardless of needle
    ret
