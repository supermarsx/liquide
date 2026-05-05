//! Shared shell services for Liquide.
//!
//! The first service in this crate is pure ShellExecute-style planning:
//! resolve a shell target, verb, and optional app override into a command
//! plan without spawning processes or touching platform state.

pub mod execute;

pub use execute::{
    ExecExpansionError, ShellApp, ShellAssociationRegistry, ShellExecuteError, ShellExecutePlan,
    ShellExecuteRequest, ShellTarget, ShellVerb, expand_exec_template,
};

#[cfg(test)]
mod tests;
