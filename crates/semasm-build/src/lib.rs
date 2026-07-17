//! Safe process execution wrapper for SemASM build pipelines.
//!
//! This crate provides a single responsibility: running external tools
//! (assemblers, linkers, disassemblers, emulators) with safety constraints
//! that make build pipelines auditable and reproducible.
//!
//! ## Design constraints
//!
//! * Arguments are always explicit arrays — never shell-concatenated strings.
//! * Every invocation has a wall-clock timeout; the child is killed on expiry.
//! * Stdout and stderr are captured into memory (no file leak).
//! * Environment can be restricted to an explicit allowlist.
//! * No network capability is passed to the child by this crate.
//! * Every execution produces a [`CommandRecord`] suitable for artifact reports.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod exec;
pub mod record;

pub use exec::{exec, BuildError, CommandOutput, CommandSpec};
pub use record::CommandRecord;
