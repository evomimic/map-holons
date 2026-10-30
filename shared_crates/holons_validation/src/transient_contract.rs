//! Descriptor-backed structural conformance for transient Dance inputs and derived graphs.
//! Uses ordinary descriptor products and reference operations; it neither stages subjects
//! nor installs Commit outcomes or executes Commit-only validation commitments.
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{
    descriptors::{
        has_type, resolve_core_descriptor, same_definition, Descriptor, EffectiveHolonDescriptor,
        EffectiveSchemaContribution, ValueDescriptorKind,
    },
    DefinitionIdentity, HolonReference, ReadableHolon,
};
use std::{collections::HashSet, sync::Arc};

/// Validates a transient holon's explicit key, required native properties, declared
/// relationships, endpoint compatibility, cardinality and duplicate policy.
/// Operational and descriptor-resolution failures remain errors, never omissions.
pub fn validate_transient_contract(
    context: &Arc<TransactionContext>,
    subject: &HolonReference,
) -> Result<(), HolonError> {
    validate_contract(context, subject, None)
}

/// Exhaustive generated-output conformance used by construction tests, removable
/// when endpoint eligibility supports descriptor-category targets. Production
/// construction does not revalidate its generated graph with this helper.
/// It cannot select arbitrary exclusions.
pub fn validate_effective_descriptor_dance_contract(
    context: &Arc<TransactionContext>,
    subject: &HolonReference,
) -> Result<(), HolonError> {
    let describing = subject.holon_descriptor()?;
    let kind = if same_definition(
        describing.holon(),
        &resolve_core_descriptor(context, "EffectiveHolonDescriptor.HolonType")?,
    )? {
        let result = EffectiveHolonDescriptor::try_from_holon(subject.clone())?;
        // Retain construction-specific category checks for precisely the products
        // whose descriptor endpoints the InstanceOf-only validator cannot express.
        result.instance_properties()?;
        result.instance_relationships()?;
        result.navigable_relationships()?;
        result.afforded_dances()?;
        result.afforded_commands()?;
        result.effective_key_rule()?;
        result.validate_contributions()?;
        Some(ConstructionSubject::Result)
    } else if same_definition(
        describing.holon(),
        &resolve_core_descriptor(context, "EffectiveSchemaContribution.HolonType")?,
    )? {
        let record = EffectiveSchemaContribution::try_from_holon(subject.clone())?;
        record.validate_provenance()?; // Canonical vocabulary, member category, actual origin occurrence.
        Some(ConstructionSubject::Contribution)
    } else {
        // Requests and responses need no endpoint accommodation.
        None
    };
    validate_contract(context, subject, kind)
}

#[derive(Clone, Copy)]
enum ConstructionSubject {
    Result,
    Contribution,
}

impl ConstructionSubject {
    fn deferred_target(self, name: &str) -> Option<(&'static str, &'static str)> {
        match (self, name) {
            (Self::Result, "EffectiveInstanceProperties") => {
                Some(("EffectiveHolonDescriptor.HolonType", "PropertyType.TypeDescriptor"))
            }
            (Self::Result, "EffectiveInstanceRelationships") => Some((
                "EffectiveHolonDescriptor.HolonType",
                "DeclaredRelationshipType.RelationshipType",
            )),
            (Self::Result, "SingularNavigableRelationships" | "PluralNavigableRelationships") => {
                Some(("EffectiveHolonDescriptor.HolonType", "RelationshipType.TypeDescriptor"))
            }
            (Self::Result, "EffectiveAffordsDance") => {
                Some(("EffectiveHolonDescriptor.HolonType", "DanceType.HolonType"))
            }
            (Self::Result, "EffectiveAffordsCommand") => {
                Some(("EffectiveHolonDescriptor.HolonType", "CommandType.HolonType"))
            }
            (Self::Contribution, "ContributionCollection") => {
                Some(("EffectiveSchemaContribution.HolonType", "RelationshipType.TypeDescriptor"))
            }
            _ => None,
        }
    }
}

fn validate_contract(
    context: &Arc<TransactionContext>,
    subject: &HolonReference,
    construction: Option<ConstructionSubject>,
) -> Result<(), HolonError> {
    if !subject.is_transient() {
        return Err(HolonError::InvalidHolonReference(
            "Transient contract validation requires a transient subject".into(),
        ));
    }
    let descriptor = subject.holon_descriptor()?;
    if descriptor.header().is_abstract_type()? {
        return Err(invalid("Transient holon has an abstract describing type"));
    }
    let computed = holons_core::descriptors::ComputedEffectiveSchema::resolve(descriptor.holon())?;
    let keyless = computed.effective_key_rule()?.is_keyless()?;
    if keyless != subject.key()?.is_none() {
        return Err(invalid(
            "Transient key does not conform to the describing type's effective key rule",
        ));
    }
    let property_names = computed
        .instance_properties()
        .iter()
        .map(|property| property.property_name())
        .collect::<Result<HashSet<_>, _>>()?;
    if !subject.undescribed_property_names_in_contract(&property_names)?.is_empty() {
        return Err(invalid("Transient holon has undeclared properties"));
    }
    for property in computed.instance_properties() {
        match subject.property_value(property.property_name()?)? {
            None if property.is_required()? => {
                return Err(invalid("Missing required transient property"))
            }
            None => {}
            Some(value) => match property.value_type()?.value_kind()? {
                ValueDescriptorKind::BaseValue(kind) if kind == value.kind() => {}
                ValueDescriptorKind::AnyBaseValue => {}
                other => {
                    return Err(HolonError::UnexpectedValueType(
                        format!("{:?}", value.kind()),
                        format!("{other:?}"),
                    ))
                }
            },
        }
    }
    let mut names = HashSet::new();
    for relationship in computed.instance_relationships() {
        let name = relationship.base_relationship_name()?;
        names.insert(name.clone());
        let relationship = holons_core::descriptors::RelationshipDescriptor::from_holon(
            relationship.holon().clone(),
        );
        if !has_type(subject, relationship.source_type()?.holon())? {
            return Err(invalid(
                "Transient relationship source has an incompatible describing type",
            ));
        }
        let bounds = relationship.effective_cardinality()?;
        let collection = subject.related_holons(name.clone())?;
        let members = collection
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .get_members()
            .clone();
        let count = members.len() as i64;
        if count < bounds.minimum || bounds.maximum.is_some_and(|maximum| count > maximum) {
            return Err(invalid(&format!(
                "Transient {} relationship {name} has {count} targets; expected {}..{:?}",
                descriptor.header().type_name()?,
                bounds.minimum,
                bounds.maximum
            )));
        }
        let endpoint = relationship.target_type()?;
        let mut identities = HashSet::new();
        let allows_duplicates = relationship.allows_duplicates()?;
        let deferred_target = if let Some((source_key, target_key)) =
            construction.and_then(|kind| kind.deferred_target(&name.to_string()))
        {
            // Match the canonical relationship identity as well as the subject
            // type. A same-named custom relationship cannot inherit this exception.
            let key = format!("({source_key})-[{name}]->({target_key})");
            same_definition(relationship.holon(), &resolve_core_descriptor(context, &key)?)?
        } else {
            false
        };
        for target in members {
            if !deferred_target && !has_type(&target, endpoint.holon())? {
                return Err(invalid(&format!(
                    "Transient relationship {name} target has an incompatible describing type"
                )));
            }
            if !allows_duplicates
                && !identities.insert(DefinitionIdentity::for_reference(&target, context)?)
            {
                return Err(invalid("Transient relationship disallows duplicate targets"));
            }
        }
    }
    for (name, collection) in subject.all_related_holons()?.iter() {
        if !names.contains(&name)
            && !collection
                .read()
                .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
                .get_members()
                .is_empty()
        {
            return Err(invalid("Transient holon has an undeclared relationship"));
        }
    }
    Ok(())
}

fn invalid(message: &str) -> HolonError {
    HolonError::InvalidParameter(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_defers_only_explicit_target_checks() {
        let deferred = [
            "EffectiveInstanceProperties",
            "EffectiveInstanceRelationships",
            "SingularNavigableRelationships",
            "PluralNavigableRelationships",
            "EffectiveAffordsDance",
            "EffectiveAffordsCommand",
        ];
        for name in deferred {
            assert!(ConstructionSubject::Result.deferred_target(name).is_some());
            assert!(ConstructionSubject::Contribution.deferred_target(name).is_none());
        }
        assert!(ConstructionSubject::Contribution
            .deferred_target("ContributionCollection")
            .is_some());
        assert!(ConstructionSubject::Result.deferred_target("ContributionCollection").is_none());
        for name in [
            "DescribedBy",
            "DescribedHolonType",
            "ContributionMember",
            "ContributionDeclaredOn",
            "EffectiveInstanceKeyRule",
            "EffectiveConstraints",
            "EffectiveValidationBindings",
            "ResponseBody",
            "RequestedHolonType",
            "EffectiveContributions",
            "Unknown",
        ] {
            assert!(ConstructionSubject::Result.deferred_target(name).is_none());
            assert!(ConstructionSubject::Contribution.deferred_target(name).is_none());
        }
    }
}
