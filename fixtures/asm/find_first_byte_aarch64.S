// find_first_byte — first index of needle in buffer[0..length], or length if absent.
// AAPCS64: x0=buffer, x1=length, x2=needle, returns x0.
.global find_first_byte
.text
find_first_byte:
    mov x3, xzr
    cbz x1, .done
.loop:
    ldrb w4, [x0]
    cmp w4, w2
    b.eq .found
    add x0, x0, #1
    add x3, x3, #1
    subs x1, x1, #1
    b.ne .loop
.done:
    mov x0, x3
    ret
.found:
    mov x0, x3
    ret
