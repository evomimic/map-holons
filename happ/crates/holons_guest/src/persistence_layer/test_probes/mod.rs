//! Deliberately unsupported authoring seams for live Integrity tests.
//!
//! # This is not part of the storage API
//!
//! These modules bypass canonical persistence behavior to author only the operations that typed
//! production APIs deliberately cannot construct. Nothing in production may call them, and every
//! probe must remain narrowly fixed to the Integrity rule its sweettest needs to reach. This
//! directory is the complete, grep-able inventory of unsupported ingress compiled into the DNA.
//!
//! # Why these probes exist
//!
//! Some Integrity rules reject operations that canonical coordinator APIs make unreachable. A
//! focused raw-authoring seam is therefore required to prove through a real conductor that those
//! rules are wired into peer validation. The rejection is the assertion; a negative-purpose probe
//! whose call succeeds has failed its purpose.
//!
//! # Why they are not feature-gated
//!
//! Sweettests run against the DNA produced by `npm run build:happ`, the same artifact the host
//! ships. A feature gate would have to be enabled in that artifact to be testable and would
//! therefore be decorative. The honest boundary is to keep these externs always present,
//! conspicuously named, minimally capable, and explicitly unsupported as production ingress.

mod holon_storage;
mod infrastructure_link;
mod smartlink;

pub use holon_storage::*;
pub use infrastructure_link::*;
pub use smartlink::*;
