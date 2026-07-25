; store_byte0 — store sil at [rdi]; return zero-extended value (SysV).
BITS 64
DEFAULT REL

global store_byte0

section .text
store_byte0:
    mov byte [rdi], sil
    movzx eax, sil
    ret
