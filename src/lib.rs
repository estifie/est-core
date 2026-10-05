//! `est_core`: the shared foundation of the EST product ecosystem.
//!
//! Every EST product builds on this crate: the command-line contract
//! ([`cli`]), layered path resolution ([`paths`]), and in-memory secret
//! handling ([`secret`]). Dependency-free by policy.

pub mod cli;
pub mod paths;
pub mod secret;
