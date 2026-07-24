// find_last_byte — last index of needle in buffer[0..length], or length if absent.
// AAPCS64: x0=buffer, x1=length, x2=needle, returns x0.
.global find_last_byte
.text
find_last_byte:
    mov x3, x1
    cbz x1, .done
    mov x4, xzr
.loop:
    ldrb w5, [x0]
    cmp w5, w2
    b.ne .skip
    mov x3, x4
.skip:
    add x0, x0, #1
    add x4, x4, #1
    subs x1, x1, #1
    b.ne .loop
.done:
    mov x0, x3
    ret
