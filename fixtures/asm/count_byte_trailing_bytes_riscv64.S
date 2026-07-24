# count_byte — decode adversarial: trailing undecoded junk after a valid ret.
.global count_byte
.text
count_byte:
    li a0, 0
    ret
    # Incomplete trailing bytes Capstone cannot consume as full RV insns.
    .byte 0x00, 0xff, 0xfe
