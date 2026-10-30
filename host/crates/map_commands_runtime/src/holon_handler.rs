use base_types::{BaseValue, MapString};
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::descriptors::Descriptor;
use holons_core::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use holons_core::{CollectionState, HolonCollection};
use std::collections::BTreeMap;
use std::sync::Arc;
use type_names::{CoreRelationshipTypeName, ToRelationshipName};

use map_commands_contract::{
    HolonAction, HolonCommand, MapResult, ReadableHolonAction, WritableHolonAction,
};

/// Handles holon-scoped commands.
pub async fn handle_holon(command: HolonCommand) -> Result<MapResult, HolonError> {
    match command.action {
        HolonAction::Read(action) => handle_read(&command.context, command.target, action),
        HolonAction::Write(action) => handle_write(command.target, action),
    }
}

fn handle_read(
    context: &Arc<TransactionContext>,
    target: HolonReference,
    action: ReadableHolonAction,
) -> Result<MapResult, HolonError> {
    match action {
        ReadableHolonAction::GetValidationFindings => match target {
            HolonReference::Staged(reference) => {
                Ok(MapResult::ValidationFindings(reference.validation_findings()?))
            }
            _ => {
                Err(HolonError::InvalidType("Validation findings require a staged carrier".into()))
            }
        },
        ReadableHolonAction::GetDescribedRelatedHolons { name } => {
            let descriptor = target.effective_descriptor()?;
            let relationship = descriptor.get_relationship_by_name(&name)?;
            let available =
                descriptor.available_relationships(&target)?.into_iter().any(|candidate| {
                    candidate.holon().reference_id_string()
                        == relationship.holon().reference_id_string()
                });
            if !available {
                return Err(HolonError::InvalidState(
                    "Relationship is unavailable in this lifecycle state".into(),
                ));
            }
            let element_type = relationship.target_type()?.holon().clone();
            let members = target
                .related_holons_with_hint(name, holons_core::RelationshipReadHint::SchemaDefault)?
                .read()
                .map_err(|error| HolonError::FailedToAcquireLock(error.to_string()))?
                .clone();
            Ok(MapResult::DescribedCollection(map_commands_contract::DescribedHolonCollection {
                members,
                element_type,
            }))
        }
        ReadableHolonAction::GetInstanceProperties => {
            let members =
                holons_core::descriptors::EffectiveHolonDescriptor::from_constructed_holon(target)
                    .instance_properties()?
                    .into_iter()
                    .map(|property| property.holon().clone())
                    .collect();
            Ok(MapResult::Collection(HolonCollection::from_parts(
                CollectionState::Fetched,
                members,
                BTreeMap::new(),
            )))
        }
        ReadableHolonAction::GetPropertyValueKind => {
            use holons_core::descriptors::ValueDescriptorKind;
            let kind = holons_core::descriptors::PropertyDescriptor::from_holon(target)
                .value_type()?
                .value_kind()?;
            let name = match kind {
                ValueDescriptorKind::BaseValue(kind) => format!("{kind}Value"),
                ValueDescriptorKind::AnyBaseValue => "AnyBaseValue".into(),
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Expected scalar property descriptor".into(),
                    ))
                }
            };
            Ok(MapResult::Value(BaseValue::StringValue(name.into())))
        }
        ReadableHolonAction::CloneHolon => {
            let transient = context.clone_holon(&target)?;
            Ok(MapResult::Reference(HolonReference::Transient(transient)))
        }
        ReadableHolonAction::Summarize => {
            let summary = target.summarize()?;
            Ok(MapResult::Value(BaseValue::StringValue(MapString::from(summary))))
        }
        ReadableHolonAction::GetHolonId => {
            let id = target.holon_id()?;
            Ok(MapResult::HolonId(id))
        }
        ReadableHolonAction::GetPredecessor => match target.predecessor()? {
            Some(r) => Ok(MapResult::Reference(r)),
            None => Ok(MapResult::None),
        },
        ReadableHolonAction::GetKey => match target.key()? {
            Some(s) => Ok(MapResult::Value(BaseValue::StringValue(s))),
            None => Ok(MapResult::None),
        },
        ReadableHolonAction::GetVersionedKey => {
            let key = target.versioned_key()?;
            Ok(MapResult::Value(BaseValue::StringValue(key)))
        }
        ReadableHolonAction::GetPropertyValue { name } => match target.property_value(name)? {
            Some(v) => Ok(MapResult::Value(v)),
            None => Ok(MapResult::None),
        },
        ReadableHolonAction::GetRelatedHolons { name, hint } => {
            let collection_arc = target.related_holons_with_hint(name, hint)?;
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
        ReadableHolonAction::GetReferenceIdentity => {
            Ok(MapResult::Value(BaseValue::StringValue(target.full_reference_id_string().into())))
        }
        ReadableHolonAction::GetEffectiveMember { collection, name } => {
            let descriptor =
                holons_core::descriptors::EffectiveHolonDescriptor::from_constructed_holon(target);
            let member = match collection {
                collection_name
                    if collection_name
                        == CoreRelationshipTypeName::EffectiveInstanceProperties
                            .to_relationship_name() =>
                {
                    descriptor.get_property_by_name(name).map(|entry| entry.holon().clone())
                }
                collection_name
                    if collection_name
                        == CoreRelationshipTypeName::NavigableRelationships
                            .to_relationship_name() =>
                {
                    descriptor.get_relationship_by_name(name).map(|entry| entry.holon().clone())
                }
                collection_name
                    if collection_name
                        == CoreRelationshipTypeName::EffectiveAffordsDance
                            .to_relationship_name() =>
                {
                    descriptor.get_dance_by_name(name).map(|entry| entry.holon().clone())
                }
                collection_name
                    if collection_name
                        == CoreRelationshipTypeName::EffectiveAffordsCommand
                            .to_relationship_name() =>
                {
                    descriptor.get_command_by_name(name).map(|entry| entry.holon().clone())
                }
                _ => {
                    return Err(HolonError::InvalidParameter(
                        "Unsupported effective member collection".into(),
                    ))
                }
            };
            match member {
                Ok(member) => Ok(MapResult::Reference(member)),
                Err(HolonError::DescriptorDeclarationNotFound { .. }) => Ok(MapResult::None),
                Err(error) => Err(error),
            }
        }
        ReadableHolonAction::GetEffectiveDescriptorForType => Ok(MapResult::Reference(
            holons_core::descriptors::EffectiveHolonDescriptor::acquire(target)?.holon().clone(),
        )),
        ReadableHolonAction::GetEffectiveDescriptor => {
            Ok(MapResult::Reference(target.effective_descriptor()?.holon().clone()))
        }
        ReadableHolonAction::GetHolonDescriptor => {
            Ok(MapResult::Reference(target.holon_descriptor()?.holon().clone()))
        }
        ReadableHolonAction::GetEffectiveCardinality => Ok(MapResult::EffectiveCardinality(
            holons_core::descriptors::RelationshipDescriptor::from_holon(target)
                .effective_cardinality()?,
        )),
        ReadableHolonAction::GetHasInstanceKey => Ok(MapResult::Value(BaseValue::BooleanValue(
            (!holons_core::descriptors::EffectiveHolonDescriptor::from_constructed_holon(target)
                .effective_key_rule()?
                .is_keyless()?)
            .into(),
        ))),
        ReadableHolonAction::GetRelationshipIsOrdered => {
            Ok(MapResult::Value(BaseValue::BooleanValue(
                holons_core::descriptors::RelationshipDescriptor::from_holon(target)
                    .is_ordered()?
                    .into(),
            )))
        }
        ReadableHolonAction::GetPropertyIsArray => Ok(MapResult::Value(BaseValue::BooleanValue(
            holons_core::descriptors::PropertyDescriptor::from_holon(target)
                .value_type()?
                .is_array()?
                .into(),
        ))),
        ReadableHolonAction::GetAvailableDances => {
            let members = target
                .effective_descriptor()?
                .afforded_dances()?
                .into_iter()
                .map(|descriptor| descriptor.holon().clone())
                .collect();
            Ok(MapResult::Collection(HolonCollection::from_parts(
                CollectionState::Fetched,
                members,
                BTreeMap::new(),
            )))
        }
        ReadableHolonAction::GetAvailableProperties => {
            let members = target
                .effective_descriptor()?
                .instance_properties()?
                .into_iter()
                .map(|descriptor| descriptor.holon().clone())
                .collect();
            Ok(MapResult::Collection(HolonCollection::from_parts(
                CollectionState::Fetched,
                members,
                BTreeMap::new(),
            )))
        }
        ReadableHolonAction::GetAvailableRelationshipsFromEffective { descriptor, singular } => {
            let descriptor =
                holons_core::descriptors::EffectiveHolonDescriptor::try_from_holon(descriptor)?;
            let mut relationships = descriptor.available_relationships(&target)?;
            if let Some(singular) = singular {
                let partition = if singular {
                    descriptor.singular_navigable_relationships()?
                } else {
                    descriptor.plural_navigable_relationships()?
                };
                let identities = partition
                    .into_iter()
                    .map(|entry| entry.holon().full_reference_id_string())
                    .collect::<std::collections::HashSet<_>>();
                relationships
                    .retain(|entry| identities.contains(&entry.holon().full_reference_id_string()));
            }
            Ok(MapResult::QualifiedRelationships(
                relationships.into_iter().map(Into::into).collect(),
            ))
        }
        ReadableHolonAction::GetAvailableRelationships => Ok(MapResult::QualifiedRelationships(
            target
                .effective_descriptor()?
                .available_relationships(&target)?
                .into_iter()
                .map(Into::into)
                .collect(),
        )),
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
    }
}
