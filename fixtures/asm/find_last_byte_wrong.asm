; find_last_byte -- intentionally WRONG: always returns 0 (ignores buffer/needle).
BITS 64
DEFAULT REL

global find_last_byte

section .text
find_last_byte:
    xor eax, eax
    ret
