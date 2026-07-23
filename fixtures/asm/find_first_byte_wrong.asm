; find_first_byte — intentionally WRONG: always returns 0 (ignores buffer/needle).
BITS 64
DEFAULT REL

global find_first_byte

section .text
find_first_byte:
    xor eax, eax
    ret
