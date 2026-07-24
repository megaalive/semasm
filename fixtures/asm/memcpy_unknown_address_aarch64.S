// memcpy_unknown_address — loads from [x9] without modeling x9 as a param.
// Expected Region/Alias evidence: incomplete (unknown_memory_accesses > 0).
.global memcpy
.text
memcpy:
    mov x0, xzr
    // Deliberate unmodeled address: x9 is not a pointer parameter.
    ldrb w1, [x9]
    ret
