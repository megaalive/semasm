// replace_byte — intentionally WRONG (AArch64): counts matches, never stores.
.global replace_byte
.text
replace_byte:
    mov x4, xzr
    cbz x1, .done
.loop:
    ldrb w5, [x0]
    cmp w5, w2
    b.ne .skip
    add x4, x4, #1
.skip:
    add x0, x0, #1
    subs x1, x1, #1
    b.ne .loop
.done:
    mov x0, x4
    ret
