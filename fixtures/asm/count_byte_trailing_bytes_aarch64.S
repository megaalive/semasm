// count_byte — decode adversarial: trailing undecoded junk after a valid ret.
.global count_byte
.text
count_byte:
    mov x0, xzr
    ret
    // Incomplete trailing bytes Capstone cannot consume as full A64 insns.
    .byte 0x00, 0xff, 0xfe
