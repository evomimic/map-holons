use std::collections::HashSet;
use std::sync::Arc;

use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{resolve_core_descriptor, HolonReference, ReadableHolon, WritableHolon};

/// Describe protocol objects for ordinary inspection before admitting a prepared load.
/// Preparation used by schema bootstrap remains independent of installed descriptors.
/// Imported domain types remain relationship-reference data consumed by the mapper.
pub(crate) fn describe_prepared_load(
    context: &Arc<TransactionContext>,
    request: HolonReference,
) -> Result<(), HolonError> {
    let types = [
        "HolonLoadSet",
        "HolonLoaderBundle",
        "LoaderHolon",
        "LoaderRelationshipReference",
        "LoaderHolonReference",
    ];
    let descriptors = types
        .iter()
        .map(|name| resolve_core_descriptor(context, &format!("{name}.HolonType")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pending = vec![(request, 0)];
    let mut seen = HashSet::new();
    while let Some((mut reference, kind)) = pending.pop() {
        if !seen.insert(reference.reference_id_string()) {
            continue;
        }
        if !matches!(&reference, HolonReference::Transient(value) if value.tx_id() == context.tx_id())
        {
            return Err(HolonError::InvalidParameter(
                "Prepared loader objects must be transient in their loader context".into(),
            ));
        }
        reference.with_descriptor(descriptors[kind].clone())?;
        let children: &[(&str, usize)] = match kind {
            0 => &[("Contains", 1)],
            1 => &[("BundleMembers", 2)],
            2 => &[("HasRelationshipReference", 3)],
            3 => &[("ReferenceSource", 4), ("ReferenceTarget", 4)],
            _ => &[],
        };
        for (relationship, child_kind) in children {
            let members = reference.related_holons(*relationship)?;
            let members = members
                .read()
                .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
                .get_members()
                .clone();
            pending.extend(members.into_iter().map(|member| (member, *child_kind)));
        }
    }
    Ok(())
}
