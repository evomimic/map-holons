use core_types::{HolonError, RelationshipName};
use holons_core::{HolonReference, ReadableHolon};

fn members(subject: &HolonReference, name: &str) -> Result<Vec<HolonReference>, HolonError> {
    Ok(subject
        .related_holons(RelationshipName(name.into()))?
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone())
}

/// A fixed Operator identity promises its entire input domain and behavior.
/// Selection reads semantic records only; callable and behavioral checks are separate.
pub(crate) fn fulfills_operators(
    slot: &HolonReference,
    candidate: &HolonReference,
) -> Result<bool, HolonError> {
    let offered = members(candidate, "OffersOperator")?;
    for required in members(slot, "RequiresOperator")? {
        for role in ["InputType", "ResultType"] {
            if members(&required, role)?.len() != 1 {
                return Err(HolonError::InvalidParameter(format!(
                    "Visualizer Operator must declare exactly one {role}"
                )));
            }
        }
        if !offered.contains(&required) {
            return Ok(false);
        }
    }
    Ok(true)
}
