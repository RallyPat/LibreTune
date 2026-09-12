//! Local MCP (Model Context Protocol) server.
//!
//! Lets external agents — Claude Code, Claude Desktop, any MCP client —
//! call LibreTune's read-only tune tools over a loopback HTTP transport,
//! so a model outside the app can inspect the same tune the in-app
//! assistant sees.
//!
//! Ported from OpenTune's `ai_mcp*.rs`, retargeted onto LibreTune's
//! [`libretune_core::agent`] tool catalogue.
//!
//! - [`token`] — the per-install bearer token on disk.
//! - [`handler`] — protocol translation (tool list, tool dispatch).
//! - [`server`] — loopback socket, auth middleware, start/stop/reconcile.
//! - [`commands`] — the Tauri command surface the Settings dialog drives.
//!
//! The server is **advisory and read-only**: it exposes no tool that can
//! change a tune or touch the ECU. Off by default; the user turns it on in
//! Settings → AI Assistant → MCP server.

pub mod commands;
pub mod handler;
pub mod server;
pub mod token;

#[cfg(test)]
mod tests;
