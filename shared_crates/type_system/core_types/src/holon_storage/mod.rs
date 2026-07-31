//! Storage-boundary types for version-aware holon node persistence (Storage SL2).
//!
//! These types are the vocabulary the guest storage layer speaks to everything above it.
//! They deliberately contain no Holochain concepts: the substrate's `Action`, `Record`, and
//! `original_action_address` stop at the persistence layer, which projects them into the
//! record-derived facts declared here.
//!
//! # Vocabulary
//!
//! These four terms are distinct and easy to conflate. New code should use them consistently, and
//! prefer them to the older "original" phrasing that predates version-aware storage.
//!
//! - **version id** — identifies one exact persisted version. Every read is version-addressed:
//!   asking for a version id returns *that* version, never the newest one in its lineage.
//! - **lineage id** — identifies a lineage, and is the version id of the record that began it.
//!   A holon's lineage is stable for its whole history; its version id changes with every edit.
//! - **predecessor** — the version an edit was made *from*. Storage takes predecessors as input
//!   to decide which lineage a new version joins, but never persists the relationship: immediate
//!   ordering lives above this layer as `Predecessor` / `Successor` SmartLinks. A predecessor and
//!   a lineage id coincide only for a lineage's second version, and diverge thereafter.
//! - **clone source** — the holon an unsaved (transient or staged) holon was copied from. This is
//!   in-memory provenance with no persistence effect; an unsaved holon has no lineage of its own.
//!
//! Deliberately avoided: **"original"**, which has been used for all four of these at different
//! times. Where it survives — `SavedHolon.original_id`, the scaffolded
//! `original_holon_node_hash` parameters — it is retained for wire or API stability and
//! documented at the point of use, not because it is the intended term.

mod types;

pub use types::*;
