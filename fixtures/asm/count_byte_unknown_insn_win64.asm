; count_byte — lowering adversarial (Win64): unmodelled mnemonic in candidate body.
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    vzeroupper
    xor eax, eax
    ret
