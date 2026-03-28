use base_types::{BaseValue, MapString};
use core_types::HolonError;
use holons_core::reference_layer::{HolonReference, ReadableHolon, WritableHolon};

use map_commands_contract::{
    HolonAction, HolonCommand, MapResult, ReadableHolonAction, WritableHolonAction,
};

/// Handles holon-scoped commands.
pub async fn handle_holon(command: HolonCommand) -> Result<MapResult, HolonError> {
    match command.action {
        HolonAction::Read(action) => handle_read(command.target, action),
        HolonAction::Write(action) => handle_write(command.target, action),
    }
}

fn handle_read(
    target: HolonReference,
    action: ReadableHolonAction,
) -> Result<MapResult, HolonError> {
    match action {
        ReadableHolonAction::CloneHolon => {
            let transient = target.clone_holon()?;
            Ok(MapResult::Reference(HolonReference::Transient(transient)))
        }
        ReadableHolonAction::EssentialContent => {
            let content = target.essential_content()?;
            Ok(MapResult::EssentialContent(content))
        }
        ReadableHolonAction::Summarize => {
            let summary = target.summarize()?;
            Ok(MapResult::Value(BaseValue::StringValue(MapString::from(summary))))
        }
        ReadableHolonAction::HolonId => {
            let id = target.holon_id()?;
            Ok(MapResult::HolonId(id))
        }
        ReadableHolonAction::Predecessor => match target.predecessor()? {
            Some(r) => Ok(MapResult::Reference(r)),
            None => Ok(MapResult::None),
        },
        ReadableHolonAction::Key => match target.key()? {
            Some(s) => Ok(MapResult::Value(BaseValue::StringValue(s))),
            None => Ok(MapResult::None),
        },
        ReadableHolonAction::VersionedKey => {
            let key = target.versioned_key()?;
            Ok(MapResult::Value(BaseValue::StringValue(key)))
        }
        ReadableHolonAction::PropertyValue { name } => match target.property_value(name)? {
            Some(v) => Ok(MapResult::Value(v)),
            None => Ok(MapResult::None),
        },
        ReadableHolonAction::RelatedHolons { name } => {
            let collection_arc = target.related_holons(name)?;
            let collection = collection_arc
                .read()
                .map_err(|e| {
                    HolonError::FailedToAcquireLock(format!(
                        "Failed to read-lock HolonCollection: {}",
                        e
                    ))
                })?
                .clone();
            Ok(MapResult::Collection(collection))
        }
    }
}

fn handle_write(
    mut target: HolonReference,
    action: WritableHolonAction,
) -> Result<MapResult, HolonError> {
    match action {
        WritableHolonAction::WithPropertyValue { name, value } => {
            target.with_property_value(name, value)?;
            Ok(MapResult::None)
        }
        WritableHolonAction::RemovePropertyValue { name } => {
            target.remove_property_value(name)?;
            Ok(MapResult::None)
        }
        WritableHolonAction::AddRelatedHolons { name, holons } => {
            target.add_related_holons(name, holons)?;
            Ok(MapResult::None)
        }
        WritableHolonAction::RemoveRelatedHolons { name, holons } => {
            target.remove_related_holons(name, holons)?;
            Ok(MapResult::None)
        }
        WritableHolonAction::WithDescriptor { descriptor } => {
            target.with_descriptor(descriptor)?;
            Ok(MapResult::None)
        }
        WritableHolonAction::WithPredecessor { predecessor } => {
            target.with_predecessor(predecessor)?;
            Ok(MapResult::None)
        }
    }
}
