; find_last_byte -- ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global find_last_byte

section .text
find_last_byte:
    mov rbx, rcx
    xor eax, eax
    ret
