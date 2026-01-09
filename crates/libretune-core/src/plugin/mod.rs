//! TS Plugin Compatibility Layer
//!
//! This module provides support for loading and running TS JAR plugins
//! in LibreTune by spawning a headless JVM subprocess that introspects Swing UIs
//! and communicates via JSON-RPC over stdin/stdout.

mod protocol;
mod bridge;
mod manager;

pub use protocol::*;
pub use bridge::*;
pub use manager::*;
