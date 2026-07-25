; store_byte0 — store dl at [rcx]; return zero-extended value (Win64).
BITS 64
DEFAULT REL

global store_byte0

section .text
store_byte0:
    mov byte [rcx], dl
    movzx eax, dl
    ret
