; find_first_byte — ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    mov rbx, rdi
    xor eax, eax
    ret
