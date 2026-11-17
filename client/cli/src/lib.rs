//! The `pagemd` command-line client: conversion, live preview and browser
//! automation over the [`pagemd_core`] rendering library.

pub mod browser;
pub mod cli;
pub mod convert;
pub mod preview;

pub use cli::run;

/// `CARGO_PKG_VERSION`, plus `-<git>` and `+dirty` when `build.rs` can see a git checkout.
pub const PAGEMD_VERSION: &str = env!("PAGEMD_VERSION");
