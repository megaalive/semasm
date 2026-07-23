; count_byte — decode adversarial (Win64): trailing undecoded junk after a valid ret.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    xor eax, eax
    ret
    ; Incomplete/invalid trailing prefix bytes Capstone cannot consume fully.
    db 0x66, 0x67, 0xf0
