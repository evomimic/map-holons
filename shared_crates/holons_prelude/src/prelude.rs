//! The holons_core prelude.
//!
//! Import this to get the curated public API surface:
//!
//! ```
//! use holons_prelude::prelude::*;
//! ```

pub mod v1;

// Current default prelude points to v1.
// In the future, this may switch to v2 in a new major release.
pub use v1::*;
