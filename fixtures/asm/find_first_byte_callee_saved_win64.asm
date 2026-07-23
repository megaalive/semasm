; find_first_byte — ABI adversarial (Win64): write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    mov rbx, rcx
    xor eax, eax
    ret
