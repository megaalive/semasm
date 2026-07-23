; count_byte — object policy adversarial (Win64): forbidden external import.
BITS 64
DEFAULT REL

global count_byte
extern puts

section .text
count_byte:
    xor eax, eax
    call puts
    ret
