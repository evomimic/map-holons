//! Reader-aware traversal for the Commit gate. Rule evaluation stays in existing handlers.
use crate::{
    assessment_support::{blocked, path},
    commitments::{required_key, ResolvedConstraint},
    contexts::{BindingDispatchRoute, SubjectLevel},
    handlers::{finding, native_rule_kind, rule_violation},
    ConstraintTypeKey, DescriptorRuleProducts, PreparedRuleSubject, ResolvedValidationBinding,
    RuleOutcome, StaticConstraintRegistry, StaticRuleHandler, StaticRuleRegistry,
    ValidationCollector, ValidationInvocation, ValidationRuleKey, ValueValidationContext,
};
use core_types::{
    BaseValue, CommitValidationViolationKind, HolonError, PropertyName, ValidationSubjectPath,
};
use holons_core::{
    descriptors::{effective_relationship_targets, resolve_describing_type},
    ContractContributions, DescribingTypeResolution, Descriptor, HolonDescriptor, HolonReference,
    PropertyDescriptor, ReadableHolon, UniversalDescriptorContract, ValueDescriptor,
    ValueDescriptorKind,
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
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<Vec<PreparedBinding>, HolonError> {
    let mut result = Vec::new();
    let contributions = packages.effective_bindings(descriptor)?;
    for contribution in contributions {
        let binding = ResolvedValidationBinding::from(contribution);
        let key = ValidationRuleKey(required_key(&binding.rule)?);
        collector.observations.discovered_rule_keys.insert(key.0.clone());
        let family = resolve_describing_type(&binding.rule)?;
        let compatible = match family {
            DescribingTypeResolution::Unique(family) => compatible_binding(
                &binding,
                &HolonDescriptor::from_holon(family),
                &key,
                descriptor,
                level,
                context,
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

/// Checks binding placement and rule-family lineage in the selected descriptor view.
pub(crate) fn compatible_binding(
    binding: &ResolvedValidationBinding,
    family: &HolonDescriptor,
    key: &ValidationRuleKey,
    governing: &HolonReference,
    level: SubjectLevel,
    context: &ValueValidationContext,
) -> Result<bool, HolonError> {
    use holons_core::descriptors::equals_or_extends;
    let roots = &context.bindings;
    if let Some(root) = roots.entries.iter().find(|entry| entry.name.as_str() == key.0) {
        // A familiar key on another reference cannot impersonate a canonical rule.
        if !holons_core::same_definition(&binding.rule, &root.rule)?
            || root.level != level
            || !equals_or_extends(family.holon(), &root.family)?
            || !equals_or_extends(binding.declaring_descriptor.holon(), &root.descriptor_family)?
        {
            return Ok(false);
        }
        if let Some(expected) = native_rule_kind(root.name) {
            holons_core::reference_layer::assert_reference_transaction_compatible(
                governing,
                &context.context,
            )?;
            return Ok(ValueDescriptor::from_holon(governing.clone()).value_kind()?
                == ValueDescriptorKind::BaseValue(expected));
        }
        return Ok(true);
    }
    // A family may have roots at several declaring descriptors or subject levels.
    // Inspect all matching roots so their registration order cannot change placement.
    let mut matched_family = false;
    let mut admitted = false;
    for root in &roots.entries {
        if equals_or_extends(family.holon(), &root.family)? {
            matched_family = true;
            if root.level == level
                && equals_or_extends(binding.declaring_descriptor.holon(), &root.descriptor_family)?
            {
                admitted = true;
            }
        }
    }
    Ok(!matched_family || admitted)
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
        let key = required_key(&prepared.binding.rule)?;
        collector.observations.dispatched_rule_keys.insert(key);
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
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<(), HolonError> {
    let contributions = packages.effective_constraints(descriptor)?;
    for contribution in contributions {
        collector.observations.effective_constraint_count += 1;
        let constraint = ResolvedConstraint::resolve(contribution)?;
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
/// Consumes a constructed contract without rediscovering its effective member surface.
pub(crate) fn assess_holon(
    subject: &HolonReference,
    descriptor: &HolonReference,
    contributions: &ContractContributions,
    bindings: &[PreparedBinding],
    key_claim: Option<&crate::key_evidence::KeyClaim>,
    values: &ValueValidationContext,
    universal: &UniversalDescriptorContract,
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<(), HolonError> {
    let mut names = HashSet::new();
    let mut properties = Vec::new();
    for member in &contributions.properties {
        let property = PropertyDescriptor::from_holon(member.member.clone());
        let name = property.property_name()?;
        if names.insert(name.clone()) {
            properties.push((name, property));
        }
    }
    let subject_path = path(subject);
    let prepared = PreparedRuleSubject::Holon {
        undescribed_properties: subject.undescribed_property_names_in_contract(&names)?,
        key_claim: key_claim.cloned(),
    };
    dispatch_subject(bindings, &prepared, &subject_path, collector)?;
    subject_constraints(descriptor, &subject_path, collector, packages)?;
    let mut declared_names = HashSet::new();
    for member in &contributions.relationships {
        match member.member.property_value(CorePropertyTypeName::TypeName)? {
            Some(core_types::BaseValue::StringValue(name)) => {
                declared_names.insert(name.to_string());
            }
            _ => return Err(HolonError::EmptyField("TypeName".into()).into()),
        }
    }
    for (name, target) in crate::schema_rules::authored_targets(subject)? {
        if !declared_names.contains(&name.to_string()) {
            // Delivered checks assess declaredness per subject; none reads a combined graph-only bucket.
            finding(collector, CommitValidationViolationKind::RuleViolation { code: "UndeclaredRelationship".into() }, None,
                &ValidationSubjectPath::Relationship { source_identity: subject.full_reference_id_string(), name: name.to_string(), target_identity: target.reference_id_string() },
                Some(descriptor.reference_id_string()), format!("Populated relationship {name} must be declared by the source's effective descriptor."));
        }
    }
    for (name, property) in properties {
        let result =
            assess_property(subject, &name, &property, values, universal, collector, packages);
        // Each property consumes its exact prepared definition.
        result?;
    }
    Ok(())
}

fn assess_property(
    subject: &HolonReference,
    name: &PropertyName,
    property: &PropertyDescriptor,
    values: &ValueValidationContext,
    universal: &UniversalDescriptorContract,
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<(), HolonError> {
    let value = subject.property_value(name)?;
    let path = ValidationSubjectPath::Property {
        holon_identity: subject.full_reference_id_string(),
        name: name.to_string(),
    };
    let bindings = prepare_bindings(
        property.holon(),
        SubjectLevel::Property,
        values,
        &path,
        collector,
        packages,
    )?;
    let missing_required = value.is_none()
        && universal.enforce_minimum(subject, property.holon())?
        && property.is_required()?;
    let facts = PreparedRuleSubject::Property {
        missing_required,
        name: name.to_string(),
        descriptor_identity: property.holon().reference_id_string(),
    };
    dispatch_subject(&bindings, &facts, &path, collector)?;
    subject_constraints(property.holon(), &path, collector, packages)?;
    if let Some(value) = value {
        let targets =
            effective_relationship_targets(property.holon(), CoreRelationshipTypeName::ValueType)?;
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
        let descriptor = ValueDescriptor::from_holon(target.member.clone());
        let path = ValidationSubjectPath::Value {
            holon_identity: subject.full_reference_id_string(),
            property: name.to_string(),
        };
        assess_value(&value, &descriptor, &path, values, collector, packages)?;
    }
    Ok(())
}

fn assess_value(
    value: &BaseValue,
    descriptor: &ValueDescriptor,
    path: &ValidationSubjectPath,
    values: &ValueValidationContext,
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<(), HolonError> {
    let bindings = prepare_bindings(
        descriptor.holon(),
        SubjectLevel::Value,
        values,
        path,
        collector,
        packages,
    )?;
    subject_constraints(descriptor.holon(), path, collector, packages)?;
    holons_core::reference_layer::assert_reference_transaction_compatible(
        descriptor.holon(),
        &values.context,
    )?;
    let facts = PreparedRuleSubject::Value {
        expected: descriptor.value_kind()?,
        actual: value.kind(),
        descriptor_identity: descriptor.holon().reference_id_string(),
    };
    dispatch_subject(&bindings, &facts, path, collector)?;
    Ok(())
}
