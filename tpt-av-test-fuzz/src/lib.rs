//! Deterministic property-based fuzzing harness for the TPT AV Stack.
//!
//! Guarantees that every parser in the ecosystem **never panics**, even on
//! malicious or corrupt input, and that CRDTs converge regardless of operation
//! ordering. Every failure is reproducible: the failing seed is captured and
//! re-run on every CI build.
//!
//! This crate is intended for use via `[dev-dependencies]` and `cfg(test)`
//! only.
//!
//! ## Modules (Phase 2)
//!
//! - [`parser`] — `fuzz_parser_never_panics!` generic parser fuzzing macro.
//! - [`crdt`] — CRDT commutativity/idempotency proptests (`assert_crdt_commutative`).
//! - [`seed`] — deterministic seed management for reproducible failures.
//! - [`corpus`](crate::corpus) — regression corpus of known-bad inputs.

/// Placeholder version marker; replaced by full module wiring in Phase 2.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
