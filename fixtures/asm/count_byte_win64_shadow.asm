; count_byte — Win64 ABI adversarial: call without shadow space / alignment.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    call .helper
    xor eax, eax
    ret
.helper:
    ret
