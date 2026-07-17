//! Safe process execution and build pipeline for SemASM.
//!
//! Two layers:
//!
//! * **`exec`** — low-level wrapper that runs a single external tool
//!   (assembler, linker, emulator) with timeout, output capture, and
//!   environment control.
//! * **`pipeline`** — higher-level orchestration that assembles, links,
//!   verifies, and runs a fixture for a given target identity.
//! * **`record`** — serialisable [`CommandRecord`] for artifact reports.
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
pub mod pipeline;
pub mod record;

pub use exec::{exec, BuildError, CommandOutput, CommandSpec};
pub use pipeline::Pipeline;
pub use record::CommandRecord;
