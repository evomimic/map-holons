//! Pure, link-type-independent validation for link-delete targets.
//!
//! Holochain link deletion is valid only when the named action created a link.
//! The substrate adapter resolves and classifies that action; this module owns
//! the deterministic rule so Integrity and future coordinator preflight share
//! one substrate-free decision.

use integrity_core_types::PvlViolation;

/// Stable diagnostic token for the only valid link-delete target kind.
pub const CREATE_LINK_ACTION_KIND: &str = "CreateLink";

/// Stable diagnostic token for a link-delete action used as a target.
pub const DELETE_LINK_ACTION_KIND: &str = "DeleteLink";

/// Stable diagnostic token for every other target action kind.
pub const OTHER_LINK_TARGET_ACTION_KIND: &str = "Other";

/// Lifecycle-relevant classification of an action targeted by `DeleteLink`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkDeleteTargetKind {
    /// A Holochain `CreateLink` action.
    CreateLink,
    /// A Holochain `DeleteLink` action.
    DeleteLink,
    /// Any action kind unrelated to link creation or deletion.
    Other,
}

impl LinkDeleteTargetKind {
    /// Returns the stable token recorded in structured PVL diagnostics.
    pub const fn diagnostic_token(self) -> &'static str {
        match self {
            Self::CreateLink => CREATE_LINK_ACTION_KIND,
            Self::DeleteLink => DELETE_LINK_ACTION_KIND,
            Self::Other => OTHER_LINK_TARGET_ACTION_KIND,
        }
    }
}

/// Validates that a link deletion targets a `CreateLink` action.
///
/// Link type is intentionally absent from this rule. A valid `CreateLink` is
/// dispatched by its scoped link type after classification; only an invalid
/// action kind receives `MAP-PVL-2004`.
pub fn validate_link_delete_target(kind: LinkDeleteTargetKind) -> Result<(), PvlViolation> {
    if kind != LinkDeleteTargetKind::CreateLink {
        return Err(PvlViolation::InvalidLinkDeleteTarget {
            expected_target_kind: CREATE_LINK_ACTION_KIND.into(),
            actual_target_kind: kind.diagnostic_token().into(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_link_is_the_only_valid_delete_target() {
        assert_eq!(validate_link_delete_target(LinkDeleteTargetKind::CreateLink), Ok(()));

        for (kind, actual_target_kind) in [
            (LinkDeleteTargetKind::DeleteLink, DELETE_LINK_ACTION_KIND),
            (LinkDeleteTargetKind::Other, OTHER_LINK_TARGET_ACTION_KIND),
        ] {
            assert_eq!(
                validate_link_delete_target(kind),
                Err(PvlViolation::InvalidLinkDeleteTarget {
                    expected_target_kind: CREATE_LINK_ACTION_KIND.into(),
                    actual_target_kind: actual_target_kind.into(),
                })
            );
        }
    }

    #[test]
    fn violation_message_does_not_expose_diagnostic_tokens() {
        let violation = validate_link_delete_target(LinkDeleteTargetKind::DeleteLink).unwrap_err();
        assert_eq!(violation.to_string(), "MAP-PVL-2004: link delete target is invalid");
        assert!(!violation.to_string().contains(DELETE_LINK_ACTION_KIND));
    }
}
