use super::{descriptor_package::DescriptorPackages, subjects::BindingRoleMemo};
use crate::{ResolvedValidationBinding, ValidationCollector, ValueValidationContext};
use core_types::{BaseValue, CommitValidationViolationKind, HolonError, ValidationSubjectPath};
use holons_core::{HolonReference, ValueDescriptor};
use type_names::CoreValidationRuleName;

/// Sealed inputs for assessing a local default against its declaring property's contract.
/// Only the selected rule handler invokes value assessment; supporting definitions are not subjects.
pub struct DefaultDeclarationAssessment<'a> {
    property: &'a HolonReference,
    value: &'a BaseValue,
    values: &'a ValueValidationContext,
    roles: &'a BindingRoleMemo,
    packages: &'a DescriptorPackages,
}

impl<'a> DefaultDeclarationAssessment<'a> {
    pub(crate) fn new(
        property: &'a HolonReference,
        value: &'a BaseValue,
        values: &'a ValueValidationContext,
        roles: &'a BindingRoleMemo,
        packages: &'a DescriptorPackages,
    ) -> Self {
        Self { property, value, values, roles, packages }
    }

    /// Returns prepared requiredness or its declaration defect; the outer error indicates
    /// that the required preparation is unavailable.
    pub(crate) fn requiredness(&self) -> Result<Result<bool, String>, HolonError> {
        self.packages.requiredness(self.property)
    }

    /// Emits findings for the default through shared value assessment, attributing
    /// unusable contracts to the binding and propagating operational failures.
    pub(crate) fn assess_value(
        &self,
        binding: &ResolvedValidationBinding,
        collector: &mut ValidationCollector,
    ) -> Result<(), HolonError> {
        let path = ValidationSubjectPath::Value {
            holon_identity: self.property.full_reference_id_string(),
            property: "DefaultValue".into(),
        };
        let target =
            holons_core::descriptors::resolve_default_value_type(self.property, self.packages)?;
        let Some(target) = target else {
            crate::diagnostics::finding(
                collector,
                CommitValidationViolationKind::UnresolvedLocalDependency,
                Some(crate::commitments::required_key(&binding.rule)?),
                &path,
                Some(crate::commitments::declaring_identity(binding)),
                "Default declaration requires one usable ValueType.".into(),
            );
            return Ok(());
        };
        super::subjects::assess_value(
            self.value,
            &ValueDescriptor::from_holon(target),
            &path,
            self.values,
            self.roles,
            collector,
            self.packages,
            CoreValidationRuleName::DefaultValueConformance,
        )
    }
}
