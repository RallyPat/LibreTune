//! AI Agent Assistant — context aggregation, safety tiering, and orchestration.
//!
//! This module is the home for the "bring your own LLM" assistant feature.
//! It is built as a thin layer over the existing tuning engines
//! ([`crate::autotune`], [`crate::action_scripting`], [`crate::ini`]) and does
//! **not** trust model output: every proposed change is validated, clamped to
//! authority limits, and staged for explicit user approval. Nothing here burns
//! to the ECU automatically.
//!
//! # Submodules
//! - [`summarize`] — aggregates tune-state into one model-facing context
//!   struct (coverage, AFR error, anomalies, predicted cells, region health).
//! - [`tiers`] — classifies constants by safety tier so dangerous config
//!   changes (pin assignments, trigger config) can be flagged for review.
//! - [`safety`] — authority-limit clamping shared by the orchestrator.
//! - [`context`] — context-gathering helpers that build the prompt payload.
//! - [`tools`] — tool definitions exposed to the model (read/propose...).
//! - [`apply`] — pure application of approved table actions to a grid.
//! - [`orchestrator`] — the per-turn agent loop: gather → call provider →
//!   validate → clamp → return a `Proposal`.
//!
//! The LLM provider client lives in [`crate::llm`].

pub mod apply;
pub mod context;
pub mod orchestrator;
pub mod safety;
pub mod summarize;
pub mod tiers;
pub mod tools;

pub use apply::apply_table_actions_to_grid;
pub use summarize::{summarize_tune_context, TuneContextSummary};
pub use tiers::{constant_safety_tier, ConstantSafetyTier};
pub use tools::CapabilityTier;
