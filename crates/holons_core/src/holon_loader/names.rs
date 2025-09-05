// crates/holons_core/src/holon_loader/names.rs

use base_types::MapString;
use integrity_core_types::{PropertyName, RelationshipName};
use type_names::ToRelationshipName;

/// ─────────────────────────────────────────────────────────────────────────────
/// Relationship names (SCREAMING_SNAKE)
/// Keep these local to the loader; can promote to `type_system` later if they stabilize.
/// ─────────────────────────────────────────────────────────────────────────────

// Segment structure
pub const REL_HOLONS_TO_LOAD:               &str = "HOLONS_TO_LOAD";

// LoaderHolon wiring
pub const REL_LOADER_RELATIONSHIPS:         &str = "LOADER_RELATIONSHIPS";
pub const REL_HAS_LOADER_REL_REF:           &str = "HAS_LOADER_RELATIONSHIP_REFERENCE";

// RelationshipReference endpoints
pub const REL_LOADER_SOURCE:                &str = "LOADER_SOURCE";
pub const REL_LOADER_TARGET:                &str = "LOADER_TARGET";

// Response → Error edge
pub const REL_HAS_LOAD_ERROR:               &str = "HAS_LOAD_ERROR";

// Often useful in traversal (optionally used)
pub const REL_INSTANCE_PROPERTIES:          &str = "INSTANCE_PROPERTIES";
pub const REL_INSTANCE_RELATIONSHIPS:       &str = "INSTANCE_RELATIONSHIPS";

/// ─────────────────────────────────────────────────────────────────────────────
/// Property names (snake_case)
/// ─────────────────────────────────────────────────────────────────────────────

// Common / LoaderHolon
pub const PROP_KEY:                         &str = "key";
pub const PROP_TYPE:                        &str = "type";

// HolonLoaderSegment instance properties
pub const PROP_FIRST:                       &str = "first";
pub const PROP_EOF:                         &str = "eof";
pub const PROP_DESCRIPTION:                 &str = "description";
pub const PROP_GENERATOR:                   &str = "generator";

// LoaderRelationshipReference properties
pub const PROP_RELATIONSHIP_NAME:           &str = "relationship_name";
pub const PROP_IS_DECLARED:                 &str = "is_declared";

// LoaderHolonReference properties
pub const PROP_HOLON_KEY:                   &str = "holon_key";
pub const PROP_HOLON_ID:                    &str = "holon_id";
pub const PROP_PROXY_KEY:                   &str = "proxy_key";
pub const PROP_PROXY_ID:                    &str = "proxy_id";

// HolonLoadResponse properties
pub const PROP_RESPONSE_STATUS_CODE:        &str = "response_status_code";
pub const PROP_HOLONS_STAGED:               &str = "holons_staged";
pub const PROP_HOLONS_COMMITTED:            &str = "holons_committed";
pub const PROP_ERRORS_ENCOUNTERED:          &str = "errors_encountered";
pub const PROP_SUMMARY:                     &str = "summary";

// HolonLoadError properties
pub const PROP_ERROR_LINE:                  &str = "error_line";
pub const PROP_ERROR_CODE:                  &str = "error_code";
pub const PROP_ERROR_MESSAGE:               &str = "error_message";

/// ─────────────────────────────────────────────────────────────────────────────
/// Tiny helpers to keep call-sites clean and string-literal free
/// ─────────────────────────────────────────────────────────────────────────────
#[inline]
pub fn rel(name: &str) -> RelationshipName {
    name.to_relationship_name()
}

#[inline]
pub fn prop(name: &str) -> PropertyName {
    PropertyName(MapString(name.to_string()))
}
