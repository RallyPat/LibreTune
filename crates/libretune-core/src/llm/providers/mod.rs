//! Concrete provider implementations.
//!
//! Each module translates the generic [`crate::llm::types`] to/from its own
//! wire format. All use the shared `reqwest::Client`.

pub mod anthropic;
pub mod google;
pub mod openai;
pub mod chatgpt;
