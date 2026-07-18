# Negative corpus

These fixtures are intentionally malformed. Required workspace tests consume
them and assert rejection without panic. They also seed the optional fuzz
targets under `fuzz/`.

- `contracts/`: missing versions, unknown fields, and duplicate names.
- `expressions/`: malformed syntax, integer overflow, and excessive nesting.
- `objects/`: empty, truncated ELF/PE, and arbitrary non-text bytes encoded as
  hexadecimal so the corpus remains reviewable in Git.
