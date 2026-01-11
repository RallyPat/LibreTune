//! TS Plugin Compatibility Layer
//!
//! This module provides support for loading and running TS JAR plugins
//! in LibreTune by spawning a headless JVM subprocess that introspects Swing UIs
//! and communicates via JSON-RPC over stdin/stdout.

mod bridge;
mod manager;
mod protocol;

pub use bridge::*;
pub use manager::*;
pub use protocol::*;
