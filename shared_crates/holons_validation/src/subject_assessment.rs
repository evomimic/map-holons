//! Binding preparation and subject assessment through exact bound references.
use crate::{
    assessment_support::{blocked, path},
    commitments::{required_key, ResolvedConstraint},
    contexts::SubjectLevel,
    handlers::{finding, rule_violation},
    registries::{registration, RuleImplementation, RuleRegistration, SubjectApplicability},
    ConstraintTypeKey, DescriptorRuleProducts, PreparedRuleSubject, ResolvedValidationBinding,
    RuleOutcome, StaticConstraintRegistry, ValidationCollector, ValidationInvocation,
    ValueValidationContext,
};
use core_types::{
    BaseValue, CommitValidationViolationKind, HolonError, PropertyName, ValidationSubjectPath,
};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    descriptors::{effective_relationship_targets, extends_role, resolve_describing_type},
    ContractContributions, DefinitionIdentity, DescribingTypeResolution, Descriptor,
    EffectiveRelationshipMember, HolonReference, PropertyDescriptor, ReadableHolon,
    UniversalDescriptorContract, ValidExtendsLineage, ValueDescriptor, ValueDescriptorKind,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use type_names::{
    CoreDefinitionRole, CorePropertyTypeName, CoreRelationshipTypeName, CoreValidationRuleName,
};

/// Role facts for encountered bindings, prepared before assessment consumes packages.
/// Only exact identity/role pairs are retained, not another descriptor closure.
#[derive(Default)]
pub(crate) struct BindingRoleMemo {
    roles: HashMap<(DefinitionIdentity, CoreDefinitionRole), Result<bool, HolonError>>,
}

impl BindingRoleMemo {
    /// Prepares role inputs for implemented bindings, reusing diagnosed governing lineages.
    pub fn prepare<'a>(
        context: &Arc<TransactionContext>,
        contributions: impl IntoIterator<Item = &'a EffectiveRelationshipMember>,
        governing_lineages: impl IntoIterator<Item = ValidExtendsLineage<'a>>,
    ) -> Result<Self, HolonError> {
        let mut diagnosed = HashMap::new();
        for lineage in governing_lineages {
            let identity = DefinitionIdentity::for_reference(lineage.subject(), context)?;
            if diagnosed.contains_key(&identity) {
                continue;
            }
            for member in lineage.members() {
                diagnosed
                    .entry(DefinitionIdentity::for_reference(member, context)?)
                    .or_insert(lineage);
            }
        }
        let mut memo = Self::default();
        for contribution in contributions {
            // Unsupported or malformed keys need no placement inputs. Required key errors
            // are still returned when the binding itself is consumed.
            let Some(registration) = required_key(&contribution.member)
                .ok()
                .and_then(|key| CoreValidationRuleName::from_key(&key))
                .map(registration)
                .filter(|registration| registration.implementation.is_some())
            else {
                continue;
            };
            if let Ok(DescribingTypeResolution::Unique(family)) =
                resolve_describing_type(&contribution.member)
            {
                memo.prepare_role(context, &family, registration.family, &diagnosed)?;
                memo.prepare_role(
                    context,
                    &contribution.declared_on,
                    registration.declaring,
                    &diagnosed,
                )?;
            }
        }
        Ok(memo)
    }

    fn prepare_role(
        &mut self,
        context: &Arc<TransactionContext>,
        reference: &HolonReference,
        role: CoreDefinitionRole,
        diagnosed: &HashMap<DefinitionIdentity, ValidExtendsLineage<'_>>,
    ) -> Result<(), HolonError> {
        let identity = DefinitionIdentity::for_reference(reference, context)?;
        let lineage = diagnosed.get(&identity);
        self.roles.entry((identity, role)).or_insert_with(|| match lineage {
            Some(lineage) => lineage.contains_role_from(reference, role),
            None => extends_role(reference, role),
        });
        Ok(())
    }

    fn matches(
        &self,
        reference: &HolonReference,
        role: CoreDefinitionRole,
        context: &Arc<TransactionContext>,
    ) -> Result<bool, HolonError> {
        let identity = DefinitionIdentity::for_reference(reference, context)?;
        // A failed required read remains operational when its binding is reached.
        self.roles
            .get(&(identity, role))
            .cloned()
            .ok_or_else(|| HolonError::InvalidState("Binding role was not prepared".into()))?
    }
}

pub(crate) struct PreparedBinding {
    binding: ResolvedValidationBinding,
    name: CoreValidationRuleName,
    registration: RuleRegistration,
}

pub(crate) fn prepare_bindings(
    descriptor: &HolonReference,
    level: SubjectLevel,
    context: &ValueValidationContext,
    roles: &BindingRoleMemo,
    path: &ValidationSubjectPath,
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<Vec<PreparedBinding>, HolonError> {
    let mut result = Vec::new();
    let contributions = packages.effective_bindings(descriptor)?;
    for contribution in contributions {
        let binding = ResolvedValidationBinding::from(contribution);
        let key = required_key(&binding.rule)?;
        collector.observations.discovered_rule_keys.insert(key.clone());
        let registered = CoreValidationRuleName::from_key(&key)
            .map(|name| (name, registration(name)))
            .filter(|(_, registration)| registration.implementation.is_some());
        let Some((name, registration)) = registered else {
            finding(
                collector,
                CommitValidationViolationKind::UnsupportedValidationRule,
                Some(key),
                path,
                Some(binding.declaring_descriptor.holon().reference_id_string()),
                "Mandatory validation binding has no registered handler.".into(),
            );
            continue;
        };
        match registration.applicability {
            SubjectApplicability::AnySubject => {}
        }
        let family = resolve_describing_type(&binding.rule)?;
        let compatible = match family {
            DescribingTypeResolution::Unique(family) => compatible_binding(
                &binding,
                &family,
                registration,
                descriptor,
                level,
                context,
                roles,
            )?,
            _ => false,
        };
        if !compatible {
            rule_violation(collector, &binding, path, "IncompatibleValidationBinding",
                "The selected rule needs one compatible describing family and a compatible declaring descriptor before dispatch.".into())?;
            continue;
        }
        result.push(PreparedBinding { binding, name, registration });
    }
    Ok(result)
}

/// Checks binding placement with prepared role facts, preserving exact bound definitions.
pub(crate) fn compatible_binding(
    binding: &ResolvedValidationBinding,
    family: &HolonReference,
    registration: RuleRegistration,
    governing: &HolonReference,
    level: SubjectLevel,
    context: &ValueValidationContext,
    roles: &BindingRoleMemo,
) -> Result<bool, HolonError> {
    if registration.level != level
        || !roles.matches(family, registration.family, &context.context)?
        || !roles.matches(
            binding.declaring_descriptor.holon(),
            registration.declaring,
            &context.context,
        )?
    {
        return Ok(false);
    }
    if let Some(expected) = registration.native_kind {
        holons_core::assert_reference_transaction_compatible(governing, &context.context)?;
        return Ok(ValueDescriptor::from_holon(governing.clone()).value_kind()?
            == ValueDescriptorKind::BaseValue(expected));
    }
    Ok(true)
}

pub(crate) fn dispatch_schema(
    bindings: &[PreparedBinding],
    products: &crate::SchemaRuleProducts,
    subject: &HolonReference,
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let mut dispatched = HashSet::new();
    for prepared in bindings {
        let Some(RuleImplementation::SchemaAggregate(handler)) =
            prepared.registration.implementation
        else {
            continue;
        };
        let key = prepared.name.as_str();
        collector.observations.dispatched_rule_keys.insert(key.into());
        dispatched.insert(key);
        handler(
            ValidationInvocation::Schema {
                code: prepared.registration.code,
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
        if !matches!(
            prepared.registration.implementation,
            Some(RuleImplementation::DescriptorProduct)
        ) {
            continue;
        }
        collector.observations.dispatched_rule_keys.insert(prepared.name.as_str().into());
        dispatched.insert(prepared.name);
        products.emit(
            prepared.name,
            prepared.registration.code,
            &prepared.binding,
            &path(subject),
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
        let Some(RuleImplementation::Subject(handler)) = prepared.registration.implementation
        else {
            continue;
        };
        collector.observations.dispatched_rule_keys.insert(prepared.name.as_str().into());
        if handler(
            ValidationInvocation::Prepared {
                code: prepared.registration.code,
                binding: &prepared.binding,
                path,
                subject,
            },
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
    roles: &BindingRoleMemo,
    universal: Option<&UniversalDescriptorContract>,
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
        let result = assess_property(
            subject, &name, &property, values, roles, universal, collector, packages,
        );
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
    roles: &BindingRoleMemo,
    universal: Option<&UniversalDescriptorContract>,
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
        roles,
        &path,
        collector,
        packages,
    )?;
    let missing_required = value.is_none()
        && match universal {
            // Contracts are prepared only for diagnosed descriptor subjects.
            Some(contract) => contract.enforce_minimum(subject, true, property.holon())?,
            // Ordinary holons and descriptors without a meta-type role get no abstract exemption.
            None => true,
        }
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
        assess_value(&value, &descriptor, &path, values, roles, collector, packages)?;
    }
    Ok(())
}

fn assess_value(
    value: &BaseValue,
    descriptor: &ValueDescriptor,
    path: &ValidationSubjectPath,
    values: &ValueValidationContext,
    roles: &BindingRoleMemo,
    collector: &mut ValidationCollector,
    packages: &crate::descriptor_package::DescriptorPackages,
) -> Result<(), HolonError> {
    let bindings = prepare_bindings(
        descriptor.holon(),
        SubjectLevel::Value,
        values,
        roles,
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
