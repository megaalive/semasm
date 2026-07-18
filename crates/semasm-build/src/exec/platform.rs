//! Narrow platform boundary for child process-tree management.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(super) use unix::{configure, ProcessTree};
#[cfg(windows)]
pub(super) use windows::{configure, ProcessTree};
