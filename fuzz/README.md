# SemASM fuzz targets

This standalone `cargo-fuzz` package is excluded from the release workspace so
`libfuzzer-sys` never enters normal build or binary dependencies.

Install and run a bounded smoke campaign with:

```text
cargo install cargo-fuzz
cargo +nightly fuzz run contract -- -max_total_time=30 -rss_limit_mb=2048
```

Targets cover contract TOML, expressions, object parsing, decoder wrappers,
CFG construction, architecture lowering, and report canonicalization. CI runs
short bounded campaigns; longer campaigns may reuse `fixtures/negative` as
seed material.
