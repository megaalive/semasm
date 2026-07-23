; find_first_byte — memory adversarial: write to buffer on a read-only leaf.
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    mov byte [rdi], 0
    xor eax, eax
    ret
