; count_byte — CFG adversarial (Win64): indirect jmp (leaf policy fail).
BITS 64
DEFAULT REL

global count_byte

section .text
count_byte:
    jmp rax
    ret
