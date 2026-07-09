#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod app;
mod ide;
mod prompt;
mod setup_packages;
mod terminal;

pub mod cargo_metadata;
pub mod command;
pub mod discovery;
pub mod distribution;
pub mod documentation;
pub mod manifest;
pub mod metadata;
pub mod path_setup;
pub mod setup;
pub mod source_registry;
pub mod state;
pub mod steam;
pub mod workflow;
pub mod workspace;

pub use app::run;
