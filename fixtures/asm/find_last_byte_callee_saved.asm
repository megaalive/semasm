; find_last_byte -- ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global find_last_byte

section .text
find_last_byte:
    mov rbx, rdi
    xor eax, eax
    ret
