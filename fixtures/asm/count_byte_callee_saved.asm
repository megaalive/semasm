; count_byte — ABI adversarial: write callee-saved RBX without save/restore.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    mov rbx, rdi
    xor eax, eax
    ret
