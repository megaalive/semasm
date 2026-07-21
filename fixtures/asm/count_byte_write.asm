; count_byte — memory adversarial: write to buffer on a read-only leaf.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    mov byte [rdi], 0
    xor eax, eax
    ret
