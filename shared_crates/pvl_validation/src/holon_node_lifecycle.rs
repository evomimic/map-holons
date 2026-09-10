//! Pure, descriptor-independent validation for `HolonNode` lifecycle targets.
//!
//! Holochain resolves the action named by an update or delete, but the lifecycle
//! rule needs only two facts about that target: which lifecycle action created it
//! and which entry kind it carries. This module owns that narrow, substrate-free
//! model so Integrity and future coordinator preflight can execute the same rule
//! without importing Holochain actions, records, or hashes.
//!
//! The substrate adapter is responsible for resolving the target and classifying
//! it into [`LifecycleTarget`]. These pure rules deliberately cannot inspect entry
//! content: lifecycle validity is determined entirely by action metadata.

use integrity_core_types::PvlViolation;

/// Fixed structured-diagnostic token for a `Create` target action.
pub const CREATE_ACTION_KIND: &str = "Create";

/// Fixed structured-diagnostic token for an `Update` target action.
pub const UPDATE_ACTION_KIND: &str = "Update";

/// Fixed structured-diagnostic token for every unsupported target action.
pub const OTHER_ACTION_KIND: &str = "Other";

/// Fixed structured-diagnostic token for a scoped `HolonNode` app entry.
pub const HOLON_NODE_ENTRY_KIND: &str = "HolonNode";

/// Fixed structured-diagnostic token for an app entry outside the `HolonNode` scope.
pub const OTHER_APP_ENTRY_KIND: &str = "OtherAppEntry";

/// Fixed structured-diagnostic token for an entry that is not an app entry.
pub const NON_APP_ENTRY_KIND: &str = "NonAppEntry";

/// Fixed structured-diagnostic token for a target action with no entry type.
pub const ABSENT_ENTRY_KIND: &str = "Absent";

/// Fixed structured-diagnostic token for the action kinds accepted by a delete.
pub const CREATE_OR_UPDATE_ACTION_KIND: &str = "CreateOrUpdate";

/// Lifecycle-relevant classification of the action that a write targets.
///
/// This is intentionally closed and narrower than Holochain's `Action`: PVL needs
/// to distinguish the two entry-creation actions and treats every other action
/// identically for lifecycle validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetActionKind {
    /// A Holochain `Create` action.
    Create,
    /// A Holochain `Update` action.
    Update,
    /// Any action kind that does not create an entry version.
    Other,
}

impl TargetActionKind {
    /// Returns the stable token recorded in structured PVL diagnostics.
    pub const fn diagnostic_token(self) -> &'static str {
        match self {
            Self::Create => CREATE_ACTION_KIND,
            Self::Update => UPDATE_ACTION_KIND,
            Self::Other => OTHER_ACTION_KIND,
        }
    }
}

/// Lifecycle-relevant classification of the entry type carried by a target action.
///
/// `HolonNode` identity includes its defining integrity-zome scope. The substrate
/// adapter must therefore classify the complete scoped app-entry identity rather
/// than relying on an entry-definition index alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetEntryKind {
    /// The scoped app-entry identity for `HolonNode`.
    HolonNode,
    /// An app entry whose scoped identity is not `HolonNode`.
    OtherAppEntry,
    /// A non-app entry type, such as an agent or capability entry.
    NonAppEntry,
    /// An action that carries no entry type.
    Absent,
}

impl TargetEntryKind {
    /// Returns the stable token recorded in structured PVL diagnostics.
    pub const fn diagnostic_token(self) -> &'static str {
        match self {
            Self::HolonNode => HOLON_NODE_ENTRY_KIND,
            Self::OtherAppEntry => OTHER_APP_ENTRY_KIND,
            Self::NonAppEntry => NON_APP_ENTRY_KIND,
            Self::Absent => ABSENT_ENTRY_KIND,
        }
    }
}

/// Substrate-free facts about the resolved target of an update or delete.
///
/// Resolution failures are not represented here. The substrate adapter must
/// propagate those failures before constructing this value so temporarily
/// unavailable dependencies cannot become permanent PVL violations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LifecycleTarget {
    /// The lifecycle-relevant kind of the resolved target action.
    pub action_kind: TargetActionKind,
    /// The lifecycle-relevant kind of entry carried by the target action.
    pub entry_kind: TargetEntryKind,
}

/// Validates that an update is rooted directly at a `HolonNode` `Create`.
///
/// MAP uses `original_action_address` as a lineage-root pointer, not as an
/// immediate-predecessor pointer. An `Update` target is therefore invalid even
/// when it carries a `HolonNode`.
///
/// Action kind is checked before entry kind so the structured diagnostic reports
/// the first failing lifecycle axis.
pub fn validate_update_target(target: &LifecycleTarget) -> Result<(), PvlViolation> {
    if target.action_kind != TargetActionKind::Create {
        return Err(PvlViolation::InvalidUpdateTarget {
            expected_target_kind: CREATE_ACTION_KIND.into(),
            actual_target_kind: target.action_kind.diagnostic_token().into(),
        });
    }

    if target.entry_kind != TargetEntryKind::HolonNode {
        return Err(PvlViolation::InvalidUpdateTarget {
            expected_target_kind: HOLON_NODE_ENTRY_KIND.into(),
            actual_target_kind: target.entry_kind.diagnostic_token().into(),
        });
    }

    Ok(())
}

/// Validates that a delete names an exact persisted `HolonNode` version.
///
/// Both `Create` and `Update` actions identify exact entry versions and are valid
/// delete targets. Other action kinds, absent entries, non-app entries, and app
/// entries outside the scoped `HolonNode` identity are rejected.
///
/// Action kind is checked before entry kind so the structured diagnostic reports
/// the first failing lifecycle axis.
pub fn validate_delete_target(target: &LifecycleTarget) -> Result<(), PvlViolation> {
    if !matches!(target.action_kind, TargetActionKind::Create | TargetActionKind::Update) {
        return Err(PvlViolation::InvalidDeleteTarget {
            expected_target_kind: CREATE_OR_UPDATE_ACTION_KIND.into(),
            actual_target_kind: target.action_kind.diagnostic_token().into(),
        });
    }

    if target.entry_kind != TargetEntryKind::HolonNode {
        return Err(PvlViolation::InvalidDeleteTarget {
            expected_target_kind: HOLON_NODE_ENTRY_KIND.into(),
            actual_target_kind: target.entry_kind.diagnostic_token().into(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTION_KINDS: [TargetActionKind; 3] =
        [TargetActionKind::Create, TargetActionKind::Update, TargetActionKind::Other];
    const ENTRY_KINDS: [TargetEntryKind; 4] = [
        TargetEntryKind::HolonNode,
        TargetEntryKind::OtherAppEntry,
        TargetEntryKind::NonAppEntry,
        TargetEntryKind::Absent,
    ];

    fn expected_update_violation(target: &LifecycleTarget) -> PvlViolation {
        let (expected_target_kind, actual_target_kind) =
            if target.action_kind != TargetActionKind::Create {
                (CREATE_ACTION_KIND, target.action_kind.diagnostic_token())
            } else {
                (HOLON_NODE_ENTRY_KIND, target.entry_kind.diagnostic_token())
            };

        PvlViolation::InvalidUpdateTarget {
            expected_target_kind: expected_target_kind.into(),
            actual_target_kind: actual_target_kind.into(),
        }
    }

    fn expected_delete_violation(target: &LifecycleTarget) -> PvlViolation {
        let (expected_target_kind, actual_target_kind) =
            if !matches!(target.action_kind, TargetActionKind::Create | TargetActionKind::Update) {
                (CREATE_OR_UPDATE_ACTION_KIND, target.action_kind.diagnostic_token())
            } else {
                (HOLON_NODE_ENTRY_KIND, target.entry_kind.diagnostic_token())
            };

        PvlViolation::InvalidDeleteTarget {
            expected_target_kind: expected_target_kind.into(),
            actual_target_kind: actual_target_kind.into(),
        }
    }

    #[test]
    fn update_rule_exhausts_every_action_and_entry_kind_pair() {
        let mut case_count = 0;

        for action_kind in ACTION_KINDS {
            for entry_kind in ENTRY_KINDS {
                case_count += 1;
                let target = LifecycleTarget { action_kind, entry_kind };
                let result = validate_update_target(&target);

                if target
                    == (LifecycleTarget {
                        action_kind: TargetActionKind::Create,
                        entry_kind: TargetEntryKind::HolonNode,
                    })
                {
                    assert_eq!(result, Ok(()), "valid target was rejected: {target:?}");
                } else {
                    assert_eq!(
                        result,
                        Err(expected_update_violation(&target)),
                        "unexpected verdict for {target:?}"
                    );
                }
            }
        }

        assert_eq!(case_count, 12);
    }

    #[test]
    fn delete_rule_exhausts_every_action_and_entry_kind_pair() {
        let mut case_count = 0;

        for action_kind in ACTION_KINDS {
            for entry_kind in ENTRY_KINDS {
                case_count += 1;
                let target = LifecycleTarget { action_kind, entry_kind };
                let result = validate_delete_target(&target);
                let is_valid = matches!(
                    target.action_kind,
                    TargetActionKind::Create | TargetActionKind::Update
                ) && target.entry_kind == TargetEntryKind::HolonNode;

                if is_valid {
                    assert_eq!(result, Ok(()), "valid target was rejected: {target:?}");
                } else {
                    assert_eq!(
                        result,
                        Err(expected_delete_violation(&target)),
                        "unexpected verdict for {target:?}"
                    );
                }
            }
        }

        assert_eq!(case_count, 12);
    }

    #[test]
    fn action_failure_takes_precedence_over_entry_failure() {
        let target = LifecycleTarget {
            action_kind: TargetActionKind::Other,
            entry_kind: TargetEntryKind::Absent,
        };

        assert_eq!(
            validate_update_target(&target),
            Err(PvlViolation::InvalidUpdateTarget {
                expected_target_kind: CREATE_ACTION_KIND.into(),
                actual_target_kind: OTHER_ACTION_KIND.into(),
            })
        );
        assert_eq!(
            validate_delete_target(&target),
            Err(PvlViolation::InvalidDeleteTarget {
                expected_target_kind: CREATE_OR_UPDATE_ACTION_KIND.into(),
                actual_target_kind: OTHER_ACTION_KIND.into(),
            })
        );
    }

    #[test]
    fn lifecycle_violations_use_stable_codes_and_messages() {
        let update_violation = validate_update_target(&LifecycleTarget {
            action_kind: TargetActionKind::Update,
            entry_kind: TargetEntryKind::HolonNode,
        })
        .expect_err("an update-to-update lineage must be rejected");
        let delete_violation = validate_delete_target(&LifecycleTarget {
            action_kind: TargetActionKind::Other,
            entry_kind: TargetEntryKind::HolonNode,
        })
        .expect_err("a delete must target an entry-creation action");

        assert_eq!(update_violation.code(), "MAP-PVL-1301");
        assert_eq!(update_violation.to_string(), "MAP-PVL-1301: update target is invalid");
        assert_eq!(delete_violation.code(), "MAP-PVL-1303");
        assert_eq!(delete_violation.to_string(), "MAP-PVL-1303: delete target is invalid");
    }
}
