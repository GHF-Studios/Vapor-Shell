#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod app;
mod cross_toolchain;
mod diagnostics;
mod git_provider;
mod ide;
mod launch_session;
mod prompt;
mod source;

pub mod app_local_tools;
pub mod cargo_metadata;
pub mod command;
pub mod content;
pub mod discovery;
pub mod distribution;
pub mod documentation;
pub mod manifest;
pub mod metadata;
pub mod source_registry;
pub mod state;
pub mod steam;
pub mod workflow;
pub mod workspace;

pub use app::run;
