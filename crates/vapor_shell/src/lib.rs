#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod app;
mod prompt;
mod terminal;

pub mod cargo_metadata;
pub mod command;
pub mod discovery;
pub mod distribution;
pub mod documentation;
pub mod manifest;
pub mod metadata;
pub mod path_setup;
pub mod state;
pub mod steam;
pub mod toolchain;
pub mod workflow;
pub mod workspace;

pub use app::run;
