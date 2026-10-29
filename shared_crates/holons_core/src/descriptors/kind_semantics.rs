//! Graph-derived descriptor categories; no category-name table or recursive validation.
use super::{
    resolve_describing_type, DescribingTypeResolution, StructuralPrerequisites, ValidExtendsLineage,
};
use crate::{
    core_shared_objects::transactions::TransactionContext,
    reference_layer::{assert_reference_transaction_compatible, HolonReference, ReadableHolon},
};
use base_types::BaseValue;
use core_types::HolonError;
use std::sync::Arc;
use type_names::{CoreDefinitionRole, CorePropertyTypeName};

/// Separates a readable, invalid local designation from an inability to read the graph.
#[derive(Debug)]
pub enum KindResolutionError {
    /// `DefinesInstanceTypeKind` is absent or is not Boolean on this descriptor.
    InvalidDesignation { descriptor: Box<HolonReference>, error: HolonError },
    /// The bound reference could not supply the required state.
    Read(HolonError),
}

impl From<HolonError> for KindResolutionError {
    fn from(error: HolonError) -> Self {
        Self::Read(error)
    }
}

impl KindResolutionError {
    fn into_read_error(self) -> HolonError {
        match self {
            Self::InvalidDesignation { error, .. } => error,
            Self::Read(error) => error,
        }
    }
}

/// Results of the independent DS-KIND-004 and DS-KIND-005 propositions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescribingCompatibility {
    /// `None` when the kind anchor has no unique direct describing type.
    pub category_matches: Option<bool>,
    pub meta_type_corresponds: bool,
}

/// Category required independently of the subject's chosen describer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequiredCategory {
    /// A kind anchor's actual direct describer.
    Exact(HolonReference),
    /// A fixed Core role in the governing lineage.
    Role(CoreDefinitionRole),
}

/// Graph-derived kind semantics guarded by one transaction context, without resolved roots.
#[derive(Clone, Debug)]
pub struct DescriptorKindSemantics {
    context: Arc<TransactionContext>,
}

impl DescriptorKindSemantics {
    /// Establishes the transaction guard without resolving Core definitions.
    pub fn new(context: &Arc<TransactionContext>) -> Self {
        Self { context: Arc::clone(context) }
    }

    fn check_lineage(&self, lineage: ValidExtendsLineage<'_>) -> Result<(), HolonError> {
        for item in lineage.members() {
            assert_reference_transaction_compatible(item, &self.context)?;
        }
        Ok(())
    }

    /// Classifies by the TypeDescriptor role in the diagnosed lineage.
    pub fn is_descriptor(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, HolonError> {
        self.check_lineage(lineage)?;
        Ok(lineage.contains_role(CoreDefinitionRole::TypeDescriptor))
    }

    /// Returns the nearest locally designated anchor in an already diagnosed lineage.
    pub fn instance_type_kind(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.assess_instance_type_kind(lineage).map_err(KindResolutionError::into_read_error)
    }

    /// Resolves the same kind product while retaining the cause of malformed designations.
    /// Read failures remain distinct even if their underlying error resembles a field error.
    pub fn assess_instance_type_kind(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, KindResolutionError> {
        self.check_lineage(lineage).map_err(KindResolutionError::Read)?;
        self.kind_in_lineage(lineage)
    }

    fn kind_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, KindResolutionError> {
        if !lineage.contains_role(CoreDefinitionRole::TypeDescriptor) {
            return Ok(None);
        }
        for descriptor in lineage.members() {
            let value = descriptor
                .property_value(CorePropertyTypeName::DefinesInstanceTypeKind)
                .map_err(KindResolutionError::Read)?;
            let error = match value {
                Some(BaseValue::BooleanValue(value)) => {
                    if value.0 {
                        return Ok(Some(descriptor.clone()));
                    }
                    continue;
                }
                Some(other) => {
                    HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into())
                }
                None => HolonError::EmptyField("DefinesInstanceTypeKind".into()),
            };
            return Err(KindResolutionError::InvalidDesignation {
                descriptor: Box::new(descriptor.clone()),
                error,
            });
        }
        Ok(None)
    }

    /// Meta-types have the MetaTypeDescriptor role in their diagnosed lineage.
    pub fn is_meta_type(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, HolonError> {
        self.check_lineage(lineage)?;
        Ok(lineage.contains_role(CoreDefinitionRole::MetaTypeDescriptor))
    }

    /// Uses the kind anchor's direct describer, with the normative root exception.
    /// A malformed anchor describer has no reliable category and blocks dependent checks.
    pub fn required_describing_category(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<RequiredCategory>, HolonError> {
        self.check_lineage(lineage)?;
        self.category_in_lineage(lineage).map_err(KindResolutionError::into_read_error)
    }

    fn category_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<RequiredCategory>, KindResolutionError> {
        if lineage.subject_has_role(CoreDefinitionRole::TypeDescriptor) {
            return Ok(Some(RequiredCategory::Role(CoreDefinitionRole::MetaHolonType)));
        }
        match self.kind_in_lineage(lineage)? {
            Some(anchor) => {
                match resolve_describing_type(&anchor).map_err(KindResolutionError::Read)? {
                    DescribingTypeResolution::Unique(category) => {
                        Ok(Some(RequiredCategory::Exact(category)))
                    }
                    DescribingTypeResolution::Missing | DescribingTypeResolution::Multiple(_) => {
                        Ok(None)
                    }
                }
            }
            None => Ok(Some(RequiredCategory::Role(CoreDefinitionRole::HolonType))),
        }
    }

    /// Computes the two independent compatibility results from diagnosed lineages.
    /// `None` means a structural prerequisite or direct describing selection is invalid.
    pub fn describing_lineages_compatible(
        &self,
        prerequisites: &StructuralPrerequisites,
    ) -> Result<Option<DescribingCompatibility>, HolonError> {
        self.assess_describing_lineages_compatible(prerequisites)
            .map_err(KindResolutionError::into_read_error)
    }

    /// Computes compatibility without conflating malformed designations with read failures.
    pub fn assess_describing_lineages_compatible(
        &self,
        prerequisites: &StructuralPrerequisites,
    ) -> Result<Option<DescribingCompatibility>, KindResolutionError> {
        let Some((subject, governing)) = prerequisites.valid_lineages() else {
            return Ok(None);
        };
        self.check_lineage(subject).map_err(KindResolutionError::Read)?;
        self.check_lineage(governing).map_err(KindResolutionError::Read)?;
        let category = self.category_in_lineage(subject)?;
        Ok(Some(DescribingCompatibility {
            category_matches: category
                .as_ref()
                .map(|category| match category {
                    RequiredCategory::Exact(reference) => governing.contains(reference),
                    RequiredCategory::Role(role) => Ok(governing.contains_role(*role)),
                })
                .transpose()?,
            meta_type_corresponds: subject.contains_role(CoreDefinitionRole::TypeDescriptor)
                == governing.contains_role(CoreDefinitionRole::MetaTypeDescriptor),
        }))
    }
}
