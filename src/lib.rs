//! `est_core`: the shared foundation of the EST product ecosystem.
//!
//! Every EST product builds on this crate: the command-line contract
//! ([`cli`]), the audit-event contract ([`audit`]), `[core]` configuration ([`config`]),
//! the hub's address ([`hub`]), structured logging
//! ([`log`]), layered path resolution ([`paths`]), supervised child
//! processes ([`process`]), in-memory secret handling ([`secret`]), and
//! durable files ([`store`]). Dependency-free by policy.

pub mod audit;
pub mod cli;
pub mod config;
pub mod hub;
pub mod log;
pub mod paths;
pub mod process;
pub mod secret;
pub mod store;

#[cfg(test)]
pub(crate) mod testutil;
