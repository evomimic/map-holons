//! Reader-aware traversal for the C2 Commit gate. Rule evaluation stays in existing handlers.
use crate::{
    assessment_support::{blocked, path, recover},
    commitments::{required_key, ResolvedConstraint},
    contexts::{BindingDispatchRoute, SubjectLevel},
    handlers::{finding, rule_violation},
    validators::compatible_binding_with_reader,
    ConstraintTypeKey, DescriptorRuleProducts, PreparedRuleSubject, ResolvedValidationBinding,
    RuleOutcome, StaticConstraintRegistry, StaticRuleHandler, StaticRuleRegistry,
    ValidationCollector, ValidationInvocation, ValidationRuleKey, ValueValidationContext,
};
use core_types::{CommitValidationViolationKind, HolonError, PropertyName, ValidationSubjectPath};
use holons_core::{
    descriptors::{
        effective_relationship_targets_with_reader, resolve_describing_type_with_reader,
    },
    AssessmentReadError, ContractContributions, DescribingTypeResolution, Descriptor,
    DescriptorReader, HolonDescriptor, HolonReference, PropertyDescriptor,
    ProspectiveDescriptorReader, ReadableHolon, UniversalDescriptorContract, ValueDescriptor,
};
use std::collections::HashSet;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName, CoreValidationRuleName};

pub(crate) struct PreparedBinding {
    binding: ResolvedValidationBinding,
    handler: StaticRuleHandler,
    route: BindingDispatchRoute,
}

pub(crate) fn prepare_bindings(
    descriptor: &HolonReference,
    level: SubjectLevel,
    context: &ValueValidationContext,
    reader: &ProspectiveDescriptorReader,
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
) -> Result<Vec<PreparedBinding>, AssessmentReadError> {
    let mut result = Vec::new();
    for contribution in effective_relationship_targets_with_reader(
        descriptor,
        CoreRelationshipTypeName::ValidationBindings,
        reader,
    )? {
        let binding = ResolvedValidationBinding::from(contribution);
        let key = ValidationRuleKey(required_key(&binding.rule)?);
        collector.observations.discovered_rule_keys.insert(key.0.clone());
        let family = resolve_describing_type_with_reader(&binding.rule, reader)?;
        let compatible = match family {
            DescribingTypeResolution::Unique(family) => compatible_binding_with_reader(
                &binding,
                &HolonDescriptor::from_holon(family),
                &key,
                descriptor,
                level,
                context,
                reader,
            )?,
            _ => false,
        };
        if !compatible {
            rule_violation(collector, &binding, path, "IncompatibleValidationBinding",
                "The selected rule needs one compatible describing family and a compatible declaring descriptor before dispatch.".into())?;
            continue;
        }
        if let Some(handler) = StaticRuleRegistry::lookup(&key) {
            let route = context
                .bindings
                .entries
                .iter()
                .find(|entry| entry.name.as_str() == key.0)
                .map(|entry| entry.route)
                .unwrap_or(BindingDispatchRoute::Subject);
            result.push(PreparedBinding { binding, handler, route });
        } else {
            finding(
                collector,
                CommitValidationViolationKind::UnsupportedValidationRule,
                Some(key.0),
                path,
                Some(binding.declaring_descriptor.holon().reference_id_string()),
                "Mandatory validation binding has no registered handler.".into(),
            );
        }
    }
    Ok(result)
}

pub(crate) fn dispatch_schema(
    bindings: &[PreparedBinding],
    products: &crate::SchemaRuleProducts,
    subject: &HolonReference,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let mut dispatched = HashSet::new();
    for prepared in bindings {
        if prepared.route != BindingDispatchRoute::SchemaAggregate {
            continue;
        }
        collector.observations.dispatched_rule_keys.insert(required_key(&prepared.binding.rule)?);
        dispatched.insert(required_key(&prepared.binding.rule)?);
        (prepared.handler)(
            ValidationInvocation::Schema {
                binding: &prepared.binding,
                path: &path(subject),
                products,
            },
            collector,
        )?;
    }
    for rule in products.rules_with_findings() {
        if !dispatched.contains(rule.as_str()) {
            blocked(collector, subject, format!("Schema aggregate found a violation of {}, but its mandatory compatible binding is unavailable.", rule.as_str()));
        }
    }
    Ok(())
}

pub(crate) fn dispatch_descriptor(
    bindings: &[PreparedBinding],
    products: &DescriptorRuleProducts,
    subject: &HolonReference,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let mut dispatched = HashSet::new();
    for prepared in bindings {
        if prepared.route != BindingDispatchRoute::Descriptor {
            continue;
        }
        let key = required_key(&prepared.binding.rule)?;
        collector.observations.dispatched_rule_keys.insert(key.clone());
        if let Some(name) = CoreValidationRuleName::from_key(&key) {
            dispatched.insert(name);
        }
        (prepared.handler)(
            ValidationInvocation::Descriptor {
                binding: &prepared.binding,
                path: &path(subject),
                products,
            },
            collector,
        )?;
    }
    products.emit_prerequisites(&path(subject), &dispatched, collector);
    Ok(())
}

fn dispatch_subject(
    bindings: &[PreparedBinding],
    subject: &PreparedRuleSubject,
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    for prepared in bindings {
        if prepared.route != BindingDispatchRoute::Subject {
            continue;
        }
        collector.observations.dispatched_rule_keys.insert(required_key(&prepared.binding.rule)?);
        if (prepared.handler)(
            ValidationInvocation::Prepared { binding: &prepared.binding, path, subject },
            collector,
        )? == RuleOutcome::StopValueEvaluation
        {
            break;
        }
    }
    Ok(())
}

/// Existing fail-closed subject evaluation. Declaration assessment has separate counters
/// and never calls this traversal, even for a declaration with no evaluator.
fn subject_constraints(
    descriptor: &HolonReference,
    path: &ValidationSubjectPath,
    reader: &ProspectiveDescriptorReader,
    collector: &mut ValidationCollector,
) -> Result<(), AssessmentReadError> {
    for contribution in effective_relationship_targets_with_reader(
        descriptor,
        CoreRelationshipTypeName::Constraints,
        reader,
    )? {
        collector.observations.effective_constraint_count += 1;
        let constraint = ResolvedConstraint::with_reader(contribution, reader)?;
        let key = ConstraintTypeKey(required_key(constraint.constraint_type.holon())?);
        if StaticConstraintRegistry::lookup(&key).is_none() {
            finding(
                collector,
                CommitValidationViolationKind::UnsupportedConstraintType {
                    constraint_identity: constraint.constraint.reference_id_string(),
                    constraint_type_identity: constraint
                        .constraint_type
                        .holon()
                        .reference_id_string(),
                },
                None,
                path,
                Some(constraint.declaring_descriptor.holon().reference_id_string()),
                format!("Mandatory constraint type {} has no registered evaluator.", key.0),
            );
        }
    }
    Ok(())
}

/// Conformance of H through D(H), never through the contract H defines for its instances.
/// The caller has already diagnosed the governing structure and established readiness.
pub(crate) fn assess_subject(
    subject: &HolonReference,
    descriptor: &HolonReference,
    bindings: &[PreparedBinding],
    values: &ValueValidationContext,
    universal: &UniversalDescriptorContract,
    reader: &ProspectiveDescriptorReader,
    collector: &mut ValidationCollector,
) -> Result<(), AssessmentReadError> {
    let contributions = ContractContributions::resolve_with_reader(descriptor, reader)?;
    let mut names = HashSet::new();
    let mut properties = Vec::new();
    for member in contributions.properties {
        let property = PropertyDescriptor::from_holon(member.member);
        let name = property.property_name()?;
        if names.insert(name.clone()) {
            properties.push((name, property));
        }
    }
    let subject_path = path(subject);
    let prepared = PreparedRuleSubject::Holon {
        undescribed_properties: subject.undescribed_property_names_in_contract(&names)?,
    };
    dispatch_subject(bindings, &prepared, &subject_path, collector)?;
    subject_constraints(descriptor, &subject_path, reader, collector)?;
    let mut declared_names = HashSet::new();
    for member in contributions.relationships {
        match member.member.property_value(CorePropertyTypeName::TypeName)? {
            Some(core_types::BaseValue::StringValue(name)) => {
                declared_names.insert(name.to_string());
            }
            _ => return Err(HolonError::EmptyField("TypeName".into()).into()),
        }
    }
    for (name, target) in crate::schema_rules::authored_targets(subject, reader)? {
        if !declared_names.contains(&name.to_string()) {
            finding(collector, CommitValidationViolationKind::RuleViolation { code: "UndeclaredRelationship".into() }, None,
                &ValidationSubjectPath::Relationship { source_identity: subject.reference_id_string(), name: name.to_string(), target_identity: target.reference_id_string() },
                Some(descriptor.reference_id_string()), format!("Populated relationship {name} must be declared by the source's effective descriptor."));
        }
    }
    for (name, property) in properties {
        let result =
            assess_property(subject, &name, &property, values, universal, reader, collector);
        // A contested member does not suppress independent property checks.
        recover(result, subject, collector)?;
    }
    Ok(())
}

fn assess_property(
    subject: &HolonReference,
    name: &PropertyName,
    property: &PropertyDescriptor,
    values: &ValueValidationContext,
    universal: &UniversalDescriptorContract,
    reader: &ProspectiveDescriptorReader,
    collector: &mut ValidationCollector,
) -> Result<(), AssessmentReadError> {
    let value = subject.property_value(name)?;
    let path = ValidationSubjectPath::Property {
        holon_identity: subject.reference_id_string(),
        name: name.to_string(),
    };
    let bindings = prepare_bindings(
        property.holon(),
        SubjectLevel::Property,
        values,
        reader,
        &path,
        collector,
    )?;
    let missing_required = value.is_none()
        && universal.enforce_minimum_with_reader(subject, property.holon(), reader)?
        && property.is_required_with_reader(reader)?;
    let facts = PreparedRuleSubject::Property {
        missing_required,
        name: name.to_string(),
        descriptor_identity: property.holon().reference_id_string(),
    };
    dispatch_subject(&bindings, &facts, &path, collector)?;
    subject_constraints(property.holon(), &path, reader, collector)?;
    if let Some(value) = value {
        let targets = effective_relationship_targets_with_reader(
            property.holon(),
            CoreRelationshipTypeName::ValueType,
            reader,
        )?;
        let [target] = targets.as_slice() else {
            blocked(
                collector,
                subject,
                format!(
                    "Property {} requires one ValueType before value assessment.",
                    property.holon().reference_id_string()
                ),
            );
            return Ok(());
        };
        let descriptor = ValueDescriptor::from_holon(reader.select(&target.member)?);
        let path = ValidationSubjectPath::Value {
            holon_identity: subject.reference_id_string(),
            property: name.to_string(),
        };
        let bindings = prepare_bindings(
            descriptor.holon(),
            SubjectLevel::Value,
            values,
            reader,
            &path,
            collector,
        )?;
        subject_constraints(descriptor.holon(), &path, reader, collector)?;
        let facts = PreparedRuleSubject::Value {
            expected: descriptor.value_kind_with_reader(&values.roots, reader)?,
            actual: value.kind(),
            descriptor_identity: descriptor.holon().reference_id_string(),
        };
        dispatch_subject(&bindings, &facts, &path, collector)?;
    }
    Ok(())
}
