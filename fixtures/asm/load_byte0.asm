; load_byte0 — return zero-extended byte at buffer+0 (SysV: rdi).
BITS 64
DEFAULT REL

global load_byte0

section .text
load_byte0:
    movzx eax, byte [rdi]
    ret
