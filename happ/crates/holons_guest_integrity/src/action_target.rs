//! Action-only target classification shared by Holochain-aware adapters.

use hdi::prelude::*;
use shared_validation::{LifecycleTarget, TargetActionKind, TargetEntryKind};

/// Projects a resolved action into the closed lifecycle model without reading entry content.
///
/// The predicate supplies scoped app-entry identity. Keeping it injected lets callers derive
/// scope from their own operation facts or `zome_info` without hiding another host call here.
pub(crate) fn classify_target(
    action: &Action,
    is_holon_node: impl FnOnce(&AppEntryDef) -> bool,
) -> LifecycleTarget {
    let action_kind = match action {
        Action::Create(_) => TargetActionKind::Create,
        Action::Update(_) => TargetActionKind::Update,
        _ => TargetActionKind::Other,
    };
    let entry_kind = match action.entry_type() {
        Some(EntryType::App(entry_def)) if is_holon_node(entry_def) => TargetEntryKind::HolonNode,
        Some(EntryType::App(_)) => TargetEntryKind::OtherAppEntry,
        Some(_) => TargetEntryKind::NonAppEntry,
        None => TargetEntryKind::Absent,
    };

    LifecycleTarget { action_kind, entry_kind }
}
