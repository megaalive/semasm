; load_byte0 — return zero-extended byte at buffer+0 (Win64: rcx).
BITS 64
DEFAULT REL

global load_byte0

section .text
load_byte0:
    movzx eax, byte [rcx]
    ret
