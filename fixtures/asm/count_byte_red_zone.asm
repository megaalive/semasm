; count_byte — ABI adversarial: non-leaf red-zone use (SysV).
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    call .helper
    mov rax, [rsp - 8]
    ret
.helper:
    ret
