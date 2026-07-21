// count_byte — count occurrences of needle in buffer[0..length].
// AAPCS64: x0=buffer, x1=length, x2=needle, returns x0.
// Uses only the modelled A64-002 mnemonic subset (no cbz/subs).
.global count_byte
.text
count_byte:
    mov x3, xzr
    cmp x1, #0
    b.eq .done
.loop:
    ldrb w4, [x0]
    add x0, x0, #1
    cmp w4, w2
    b.ne .skip
    add x3, x3, #1
.skip:
    sub x1, x1, #1
    cmp x1, #0
    b.ne .loop
.done:
    mov x0, x3
    ret
