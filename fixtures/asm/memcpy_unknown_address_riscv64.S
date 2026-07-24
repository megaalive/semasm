# memcpy_unknown_address — loads from 0(t6) without modeling t6 as a param.
# Expected Region/Alias evidence: incomplete (unknown_memory_accesses > 0).
.global memcpy
.text
memcpy:
    li a0, 0
    # Deliberate unmodeled address: t6 is not a pointer parameter.
    lbu t0, 0(t6)
    ret
