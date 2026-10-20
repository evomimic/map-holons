//! Graph-derived descriptor categories; no category-name table or recursive validation.
use super::{
    definition_identity::same_definition, resolve_assessment_anchor, resolve_describing_type,
    DescribingTypeResolution, StructuralPrerequisites, ValidExtendsLineage,
};
use crate::{
    core_shared_objects::transactions::TransactionContext,
    reference_layer::{assert_reference_transaction_compatible, HolonReference, ReadableHolon},
};
use base_types::BaseValue;
use core_types::HolonError;
use std::sync::Arc;
use type_names::CorePropertyTypeName;

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

/// Canonical identities shared across one assessment, resolved once at its boundary.
#[derive(Clone, Debug)]
pub struct DescriptorKindRoots {
    pub type_descriptor: HolonReference,
    pub holon_type: HolonReference,
    pub meta_type: HolonReference,
    pub meta_holon_type: HolonReference,
    context: Arc<TransactionContext>,
}

impl DescriptorKindRoots {
    /// Resolves assessment anchors through staged-first transaction lookup.
    /// This uses assessment anchor semantics rather than general Core lookup.
    pub fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        Self::from_resolved(
            context,
            resolve_assessment_anchor(context, "TypeDescriptor")?,
            resolve_assessment_anchor(context, "HolonType.TypeDescriptor")?,
            resolve_assessment_anchor(context, "MetaTypeDescriptor.HolonType")?,
            resolve_assessment_anchor(context, "MetaHolonType.MetaTypeDescriptor")?,
        )
    }

    /// Uses already resolved identities, including split saved/staged bootstrap.
    /// The caller must supply the designated Core roots, not name-based substitutes.
    pub fn from_resolved(
        context: &Arc<TransactionContext>,
        type_descriptor: HolonReference,
        holon_type: HolonReference,
        meta_type: HolonReference,
        meta_holon_type: HolonReference,
    ) -> Result<Self, HolonError> {
        for root in [&type_descriptor, &holon_type, &meta_type, &meta_holon_type] {
            assert_reference_transaction_compatible(root, context)?;
        }
        Ok(Self {
            type_descriptor,
            holon_type,
            meta_type,
            meta_holon_type,
            context: Arc::clone(context),
        })
    }

    fn check_lineage(&self, lineage: ValidExtendsLineage<'_>) -> Result<(), HolonError> {
        for item in lineage.members() {
            assert_reference_transaction_compatible(item, &self.context)?;
        }
        Ok(())
    }

    /// Classifies by the designated root's identity, never names or authored flags.
    pub fn is_descriptor(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, HolonError> {
        self.check_lineage(lineage)?;
        lineage.contains(&self.type_descriptor)
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
        if !lineage.contains(&self.type_descriptor)? {
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

    /// Meta-types specialize the designated meta-type root.
    pub fn is_meta_type(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, HolonError> {
        self.check_lineage(lineage)?;
        lineage.contains(&self.meta_type)
    }

    /// Uses the kind anchor's direct describer, with the normative root exception.
    /// A malformed anchor describer has no reliable category and blocks dependent checks.
    pub fn required_describing_category(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.check_lineage(lineage)?;
        self.category_in_lineage(lineage).map_err(KindResolutionError::into_read_error)
    }

    fn category_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, KindResolutionError> {
        if same_definition(lineage.subject(), &self.type_descriptor)? {
            return Ok(Some(self.meta_holon_type.clone()));
        }
        match self.kind_in_lineage(lineage)? {
            Some(anchor) => {
                match resolve_describing_type(&anchor).map_err(KindResolutionError::Read)? {
                    DescribingTypeResolution::Unique(category) => Ok(Some(category)),
                    DescribingTypeResolution::Missing | DescribingTypeResolution::Multiple(_) => {
                        Ok(None)
                    }
                }
            }
            None => Ok(Some(self.holon_type.clone())),
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
                .map(|category| governing.contains(category))
                .transpose()?,
            meta_type_corresponds: subject.contains(&self.type_descriptor)?
                == governing.contains(&self.meta_type)?,
        }))
    }
}
