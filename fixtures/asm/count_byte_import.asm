; count_byte with a forbidden external import (object policy fail).
BITS 64
DEFAULT REL

global count_byte
extern puts

section .text
count_byte:
    xor eax, eax
    call puts
    ret
