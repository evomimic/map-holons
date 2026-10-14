//! Opt-in instance assessment experiment. Definition and Schema commits retain the normal path.
//! No saved-key lookup or ReadinessContext construction occurs on this path. Core category
//! references come from the encountered descriptor graph; binding keys select static policy.
use crate::{
    assessment_support::{blocked, path, recover, targets},
    contexts::SubjectLevel,
    descriptor_package::DescriptorPackages,
    outcomes::PreparedAssessment,
    schema_view::SchemaWorkset,
    subject_assessment, ContractKindRoots, DescriptorRuleProducts, ResolvedValidationBinding,
    ValidationCollector, ValidationObservations, ValidationRuleKey, ValueValidationContext,
};
use core_types::HolonError;
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{resolve_describing_type_with_reader, walk_extends_chain_with_reader},
    ContractContributions, DescribingTypeResolution, Descriptor, DescriptorKindRoots,
    DescriptorReader, HolonDescriptor, HolonReference, ProspectiveDescriptorReader,
    ProspectiveIdentity, ReadableHolon, StagedReference, StructuralPrerequisites,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use type_names::CoreRelationshipTypeName as R;

/// Placement is checked against graph-reached families, without rediscovering rule identities.
/// Unknown handlers still fail closed in the ordinary subject evaluator.
pub(crate) fn compatible_binding<D: DescriptorReader>(
    binding: &ResolvedValidationBinding,
    family: &HolonDescriptor,
    key: &ValidationRuleKey,
    governing: &HolonReference,
    level: SubjectLevel,
    reader: &D,
) -> Result<bool, D::Error> {
    let Some((name, expected_family, expected_descriptor, expected_level, _)) =
        crate::contexts::binding_policies().into_iter().find(|entry| entry.0.as_str() == key.0)
    else {
        return Ok(true);
    };
    if level != expected_level
        || !contains_key(family.holon(), expected_family, reader)?
        || !contains_key(binding.declaring_descriptor.holon(), expected_descriptor, reader)?
    {
        return Ok(false);
    }
    if let Some(expected) = crate::handlers::native_rule_kind(name) {
        return Ok(holons_core::ValueDescriptor::from_holon(reader.select(governing)?)
            .value_kind_with_reader(reader)?
            == holons_core::ValueDescriptorKind::BaseValue(expected));
    }
    Ok(true)
}

fn contains_key<D: DescriptorReader>(
    reference: &HolonReference,
    key: &str,
    reader: &D,
) -> Result<bool, D::Error> {
    for ancestor in walk_extends_chain_with_reader(reference, reader) {
        if ancestor?.key()?.is_some_and(|found| found.0 == key) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Read only Extends and DescribedBy from the required type/member graph. The keys below
/// identify existing kernel roles, not storage lookup requests or a global namespace index.
fn graph_roots(
    context: &Arc<TransactionContext>,
    seeds: Vec<HolonReference>,
    reader: &ProspectiveDescriptorReader,
) -> Result<HashMap<String, HolonReference>, HolonError> {
    let required = [
        "TypeDescriptor",
        "HolonType.TypeDescriptor",
        "MetaTypeDescriptor.HolonType",
        "MetaHolonType.MetaTypeDescriptor",
        "PropertyType.TypeDescriptor",
        "DeclaredRelationshipType.RelationshipType",
        "ValueType.TypeDescriptor",
    ];
    let mut queue = VecDeque::from(seeds);
    let mut seen = HashSet::new();
    let mut found = HashMap::new();
    while let Some(reference) = queue.pop_front() {
        let selected = reader
            .select(&reference)
            .map_err(|error| HolonError::CommitFailure(error.to_string()))?;
        let identity = ProspectiveIdentity::for_reference(&selected, context)?;
        if !seen.insert(identity) {
            continue;
        }
        if let Some(key) = selected.key()? {
            if required.contains(&key.0.as_str()) {
                if let Some(prior) = found.insert(key.0.clone(), selected.clone()) {
                    if !holons_core::same_definition(&prior, &selected) {
                        return Err(HolonError::DuplicateError("Graph category".into(), key.0));
                    }
                }
            }
        }
        if found.len() == required.len() {
            return Ok(found);
        }
        queue.extend(targets(&selected, R::Extends)?);
        queue.extend(targets(&selected, R::DescribedBy)?);
    }
    Err(HolonError::CommitFailure(
        "Instance experiment cannot resolve required categories from the descriptor graph".into(),
    ))
}

/// Select a bounded case by its already-bound governing reference. No persistent acceptance
/// evidence is assumed: the same governing-contract checks still run within this assessment.
pub(crate) fn assess<'a>(
    context: &Arc<TransactionContext>,
    candidates: &'a [StagedReference],
    reader: &ProspectiveDescriptorReader,
    observer: &mut impl FnMut(crate::AssessmentEvent),
) -> Result<Option<(PreparedAssessment<'a>, ValidationObservations)>, HolonError> {
    let mut governors = Vec::new();
    for candidate in candidates {
        let subject: HolonReference = candidate.into();
        let describing = resolve_describing_type_with_reader(&subject, reader)
            .map_err(|error| HolonError::CommitFailure(error.to_string()))?;
        let DescribingTypeResolution::Unique(governor) = describing else {
            return Ok(None);
        };
        if governor.key()?.is_none_or(|key| key.0 != "VisualizerUsage.HolonType")
            || !targets(&subject, R::Extends)?.is_empty()
        {
            return Ok(None);
        }
        governors.push(governor);
    }
    observer(crate::AssessmentEvent::PreparationOperation {
        operation: "RelationshipDerivedInputs",
        role: "InstanceExperiment",
        starting: true,
    });
    let mut seeds = governors.clone();
    for governor in &governors {
        let contract = ContractContributions::resolve_with_reader(governor, reader)
            .map_err(|error| HolonError::CommitFailure(error.to_string()))?;
        for property in &contract.properties {
            seeds.push(property.member.clone());
            for ancestor in walk_extends_chain_with_reader(&property.member, reader) {
                seeds.extend(targets(
                    &ancestor.map_err(|error| HolonError::CommitFailure(error.to_string()))?,
                    R::ValueType,
                )?);
            }
        }
        seeds.extend(contract.relationships.into_iter().map(|member| member.member));
    }
    let graph = graph_roots(context, seeds, reader)?;
    let root = |key: &str| graph.get(key).unwrap().clone();
    let kinds = DescriptorKindRoots::from_resolved(
        context,
        root("TypeDescriptor"),
        root("HolonType.TypeDescriptor"),
        root("MetaTypeDescriptor.HolonType"),
        root("MetaHolonType.MetaTypeDescriptor"),
    )?
    .with_reader(reader);
    let contract_kinds = ContractKindRoots {
        property: root("PropertyType.TypeDescriptor"),
        relationship: root("DeclaredRelationshipType.RelationshipType"),
        value: root("ValueType.TypeDescriptor"),
    };
    let values = ValueValidationContext::from_dispatch_policy(context);
    observer(crate::AssessmentEvent::PreparationOperation {
        operation: "RelationshipDerivedInputs",
        role: "InstanceExperiment",
        starting: false,
    });
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::SubjectPreparation));
    let subjects: Vec<HolonReference> = candidates.iter().map(HolonReference::from).collect();
    let workset = SchemaWorkset { schemas: Vec::new(), ownership: HashMap::new() };
    let packages = DescriptorPackages::construct_with_observer(
        context, reader, &governors, &subjects, &workset, observer,
    )?;
    let collector = evaluate_prepared(
        &subjects,
        &governors,
        reader,
        &kinds,
        &contract_kinds,
        &values,
        &packages,
        context,
        observer,
    )?;
    #[cfg(feature = "warm-validator-probe")]
    for _ in 0..3 {
        observer(crate::AssessmentEvent::PreparationOperation {
            operation: "WarmValidatorAssessment",
            role: "SameTransactionProbe",
            starting: true,
        });
        // No mutation or write occurs between passes. References and leases remain bound to
        // this transaction; this probe does not provide cross-Commit acceptance evidence.
        let warm = evaluate_prepared(
            &subjects,
            &governors,
            reader,
            &kinds,
            &contract_kinds,
            &values,
            &packages,
            context,
            &mut |_| {},
        )?;
        observer(crate::AssessmentEvent::PreparationOperation {
            operation: "WarmValidatorAssessment",
            role: "SameTransactionProbe",
            starting: false,
        });
        if warm.violations() != collector.violations()
            || warm.observations() != collector.observations()
        {
            return Err(HolonError::CommitFailure(
                "Warm validator probe changed findings or coverage".into(),
            ));
        }
    }
    observer(crate::AssessmentEvent::Preparation(packages.preparation_metrics()));
    let observations = collector.observations().clone();
    let prepared = PreparedAssessment::from_scope(candidates, collector.into_report())?;
    Ok(Some((prepared, observations)))
}

/// Reassess subject and governing-contract checks from immutable transaction-bound inputs.
/// Each pass owns fresh findings and coverage; no prior validation result selects checks.
#[allow(clippy::too_many_arguments)]
fn evaluate_prepared(
    subjects: &[HolonReference],
    governors: &[HolonReference],
    reader: &ProspectiveDescriptorReader,
    kinds: &DescriptorKindRoots<&ProspectiveDescriptorReader>,
    contract_kinds: &ContractKindRoots,
    values: &ValueValidationContext,
    packages: &DescriptorPackages,
    context: &Arc<TransactionContext>,
    observer: &mut impl FnMut(crate::AssessmentEvent),
) -> Result<ValidationCollector, HolonError> {
    let mut collector = ValidationCollector::default();
    for finding in crate::competing_replacement_findings(reader) {
        collector.record(finding);
    }
    observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::PackageValidation));
    for (subject, governor) in subjects.iter().zip(governors) {
        let Some(subject_lineage) = recover(
            holons_core::ExtendsLineageDiagnosis::assess_with_reader(
                subject,
                &kinds.type_descriptor,
                reader,
            ),
            subject,
            &mut collector,
        )?
        else {
            continue;
        };
        let Some(governing_lineage) = recover(
            holons_core::ExtendsLineageDiagnosis::assess_with_reader(
                governor,
                &kinds.type_descriptor,
                reader,
            ),
            subject,
            &mut collector,
        )?
        else {
            continue;
        };
        let prerequisites = StructuralPrerequisites {
            describing_type: DescribingTypeResolution::Unique(governor.clone()),
            subject_lineage,
            governing_lineage: Some(governing_lineage),
        };
        let mut products = DescriptorRuleProducts::default();
        products.prepare_structure(&prerequisites, &kinds.type_descriptor);
        recover(products.prepare_kind(&prerequisites, &kinds, &reader), subject, &mut collector)?;
        let Some(contract) = recover(
            ContractContributions::resolve_with_reader(governor, reader),
            subject,
            &mut collector,
        )?
        else {
            continue;
        };
        let mut contract_products = DescriptorRuleProducts::default();
        recover(
            contract_products.prepare_contract(&contract, &kinds, &contract_kinds, &reader),
            governor,
            &mut collector,
        )?;
        if contract_products.has_findings() {
            blocked(&mut collector, governor, "Saved governing contract has invalid effective member definitions; correct its descriptor commitments.".into());
            continue;
        }
        let Some(bindings) = recover(
            subject_assessment::prepare_bindings(
                governor,
                SubjectLevel::Holon,
                &values,
                reader,
                &path(subject),
                &mut collector,
                &packages,
            ),
            subject,
            &mut collector,
        )?
        else {
            continue;
        };
        subject_assessment::dispatch_descriptor(&bindings, &products, subject, &mut collector)?;
        observer(crate::AssessmentEvent::Phase(crate::AssessmentPhase::InstanceValidation));
        packages.get(context, governor)?;
        recover(
            subject_assessment::assess_holon_with_minimum_policy(
                subject,
                governor,
                &contract,
                &bindings,
                &values,
                None,
                reader,
                &mut collector,
                &packages,
            ),
            subject,
            &mut collector,
        )?;
    }
    Ok(collector)
}

#[cfg(test)]
#[path = "instance_assessment_experiment_tests.rs"]
mod tests;
