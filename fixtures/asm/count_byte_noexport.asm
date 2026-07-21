; count_byte without exporting the required routine symbol (object policy fail).
BITS 64
DEFAULT REL

section .text
count_byte:
    xor eax, eax
    ret
