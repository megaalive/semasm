; count_byte — lowering adversarial (Win64): third unmodelled-mnemonic class (H4).
; Timestamp/counter class (`rdtsc`); pairs with SysV twin. decode/lower stay partial.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    rdtsc
    xor eax, eax
    ret
