//! Graph-derived descriptor categories; no category-name table or recursive validation.
use super::{
    definition_identity::{lineage_contains, same_definition},
    resolve_core_descriptor, resolve_describing_type, DescribingTypeResolution,
    StructuralPrerequisites, TypeHeader, ValidExtendsLineage,
};
use crate::{
    core_shared_objects::transactions::TransactionContext,
    reference_layer::{assert_reference_transaction_compatible, HolonReference},
};
use core_types::HolonError;
use std::sync::Arc;

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
    /// Resolves roots through staged-first transaction lookup.
    pub fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        Self::from_resolved(
            context,
            resolve_core_descriptor(context, "TypeDescriptor")?,
            resolve_core_descriptor(context, "HolonType.TypeDescriptor")?,
            resolve_core_descriptor(context, "MetaTypeDescriptor.HolonType")?,
            resolve_core_descriptor(context, "MetaHolonType.MetaTypeDescriptor")?,
        )
    }

    /// Uses already resolved identities, including bootstrap and prospective views.
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
        Ok(lineage_contains(lineage.members(), &self.type_descriptor))
    }

    /// Returns the nearest locally designated anchor in an already diagnosed lineage.
    pub fn instance_type_kind(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.check_lineage(lineage)?;
        self.kind_in_lineage(lineage)
    }

    fn kind_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, HolonError> {
        if !lineage_contains(lineage.members(), &self.type_descriptor) {
            return Ok(None);
        }
        for item in lineage.members() {
            if TypeHeader::new(item).defines_instance_type_kind()? {
                return Ok(Some(item.clone()));
            }
        }
        Ok(None)
    }

    /// Meta-types specialize the designated meta-type root.
    pub fn is_meta_type(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, HolonError> {
        self.check_lineage(lineage)?;
        Ok(lineage_contains(lineage.members(), &self.meta_type))
    }

    /// Uses the kind anchor's direct describer, with the normative root exception.
    /// A malformed anchor describer has no reliable category and blocks dependent checks.
    pub fn required_describing_category(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.check_lineage(lineage)?;
        self.category_in_lineage(lineage)
    }

    fn category_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, HolonError> {
        if same_definition(lineage.subject(), &self.type_descriptor) {
            return Ok(Some(self.meta_holon_type.clone()));
        }
        match self.kind_in_lineage(lineage)? {
            Some(anchor) => match resolve_describing_type(&anchor)? {
                DescribingTypeResolution::Unique(category) => Ok(Some(category)),
                DescribingTypeResolution::Missing | DescribingTypeResolution::Multiple(_) => {
                    Ok(None)
                }
            },
            None => Ok(Some(self.holon_type.clone())),
        }
    }

    /// Computes the two independent compatibility results from diagnosed lineages.
    /// `None` means a structural prerequisite or direct describing selection is invalid.
    pub fn describing_lineages_compatible(
        &self,
        prerequisites: &StructuralPrerequisites,
    ) -> Result<Option<DescribingCompatibility>, HolonError> {
        let Some((subject, governing)) = prerequisites.valid_lineages() else {
            return Ok(None);
        };
        self.check_lineage(subject)?;
        self.check_lineage(governing)?;
        let category = self.category_in_lineage(subject)?;
        Ok(Some(DescribingCompatibility {
            category_matches: category
                .as_ref()
                .map(|category| lineage_contains(governing.members(), category)),
            meta_type_corresponds: lineage_contains(subject.members(), &self.type_descriptor)
                == lineage_contains(governing.members(), &self.meta_type),
        }))
    }
}
