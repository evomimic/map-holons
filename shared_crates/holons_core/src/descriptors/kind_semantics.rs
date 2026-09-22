//! Graph-derived descriptor categories; no category-name table or recursive validation.
use super::{
    definition_identity::{lineage_contains, same_definition},
    resolve_core_descriptor, resolve_describing_type_with_reader, CurrentDescriptorReader,
    DescribingTypeResolution, DescriptorReader, StructuralPrerequisites, ValidExtendsLineage,
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
pub enum KindResolutionError<E> {
    /// `DefinesInstanceTypeKind` is absent or is not Boolean on this descriptor.
    InvalidDesignation { descriptor: Box<HolonReference>, error: HolonError },
    /// The assessment reader or reference could not supply the required state.
    Read(E),
}

impl<E: From<HolonError>> KindResolutionError<E> {
    fn into_read_error(self) -> E {
        match self {
            Self::InvalidDesignation { error, .. } => error.into(),
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
pub struct DescriptorKindRoots<R = CurrentDescriptorReader> {
    reader: R,
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
            reader: CurrentDescriptorReader,
            type_descriptor,
            holon_type,
            meta_type,
            meta_holon_type,
            context: Arc::clone(context),
        })
    }

    /// Shares canonical identities with an explicit Commit assessment reader.
    /// Supply lineages diagnosed with this same reader and unchanged input snapshot.
    pub fn with_reader<R: DescriptorReader>(self, reader: R) -> DescriptorKindRoots<R> {
        DescriptorKindRoots {
            type_descriptor: self.type_descriptor,
            holon_type: self.holon_type,
            meta_type: self.meta_type,
            meta_holon_type: self.meta_holon_type,
            context: self.context,
            reader,
        }
    }
}

impl<R: DescriptorReader> DescriptorKindRoots<R> {
    fn check_lineage(&self, lineage: ValidExtendsLineage<'_>) -> Result<(), R::Error> {
        for item in lineage.members() {
            assert_reference_transaction_compatible(item, &self.context)?;
        }
        Ok(())
    }

    /// Classifies by the designated root's identity, never names or authored flags.
    pub fn is_descriptor(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, R::Error> {
        self.check_lineage(lineage)?;
        Ok(lineage_contains(lineage.members(), &self.reader.select(&self.type_descriptor)?))
    }

    /// Returns the nearest locally designated anchor in an already diagnosed lineage.
    pub fn instance_type_kind(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, R::Error> {
        self.assess_instance_type_kind(lineage).map_err(KindResolutionError::into_read_error)
    }

    /// Resolves the same kind product while retaining the cause of malformed designations.
    /// Read failures remain distinct even if their underlying error resembles a field error.
    pub fn assess_instance_type_kind(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, KindResolutionError<R::Error>> {
        self.check_lineage(lineage).map_err(KindResolutionError::Read)?;
        self.kind_in_lineage(lineage)
    }

    fn kind_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, KindResolutionError<R::Error>> {
        let root = self.reader.select(&self.type_descriptor).map_err(KindResolutionError::Read)?;
        if !lineage_contains(lineage.members(), &root) {
            return Ok(None);
        }
        for item in lineage.members() {
            let descriptor = self.reader.select(item).map_err(KindResolutionError::Read)?;
            let value = descriptor
                .property_value(CorePropertyTypeName::DefinesInstanceTypeKind)
                .map_err(|error| KindResolutionError::Read(error.into()))?;
            let error = match value {
                Some(BaseValue::BooleanValue(value)) => {
                    if value.0 {
                        return Ok(Some(descriptor));
                    }
                    continue;
                }
                Some(other) => {
                    HolonError::UnexpectedValueType(format!("{other:?}"), "Boolean".into())
                }
                None => HolonError::EmptyField("DefinesInstanceTypeKind".into()),
            };
            return Err(KindResolutionError::InvalidDesignation {
                descriptor: Box::new(descriptor),
                error,
            });
        }
        Ok(None)
    }

    /// Meta-types specialize the designated meta-type root.
    pub fn is_meta_type(&self, lineage: ValidExtendsLineage<'_>) -> Result<bool, R::Error> {
        self.check_lineage(lineage)?;
        Ok(lineage_contains(lineage.members(), &self.reader.select(&self.meta_type)?))
    }

    /// Uses the kind anchor's direct describer, with the normative root exception.
    /// A malformed anchor describer has no reliable category and blocks dependent checks.
    pub fn required_describing_category(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, R::Error> {
        self.check_lineage(lineage)?;
        self.category_in_lineage(lineage).map_err(KindResolutionError::into_read_error)
    }

    fn category_in_lineage(
        &self,
        lineage: ValidExtendsLineage<'_>,
    ) -> Result<Option<HolonReference>, KindResolutionError<R::Error>> {
        if same_definition(
            lineage.subject(),
            &self.reader.select(&self.type_descriptor).map_err(KindResolutionError::Read)?,
        ) {
            return Ok(Some(
                self.reader.select(&self.meta_holon_type).map_err(KindResolutionError::Read)?,
            ));
        }
        match self.kind_in_lineage(lineage)? {
            Some(anchor) => match resolve_describing_type_with_reader(&anchor, &self.reader)
                .map_err(KindResolutionError::Read)?
            {
                DescribingTypeResolution::Unique(category) => Ok(Some(category)),
                DescribingTypeResolution::Missing | DescribingTypeResolution::Multiple(_) => {
                    Ok(None)
                }
            },
            None => {
                Ok(Some(self.reader.select(&self.holon_type).map_err(KindResolutionError::Read)?))
            }
        }
    }

    /// Computes the two independent compatibility results from diagnosed lineages.
    /// `None` means a structural prerequisite or direct describing selection is invalid.
    pub fn describing_lineages_compatible(
        &self,
        prerequisites: &StructuralPrerequisites,
    ) -> Result<Option<DescribingCompatibility>, R::Error> {
        self.assess_describing_lineages_compatible(prerequisites)
            .map_err(KindResolutionError::into_read_error)
    }

    /// Computes compatibility without conflating malformed designations with read failures.
    pub fn assess_describing_lineages_compatible(
        &self,
        prerequisites: &StructuralPrerequisites,
    ) -> Result<Option<DescribingCompatibility>, KindResolutionError<R::Error>> {
        let Some((subject, governing)) = prerequisites.valid_lineages() else {
            return Ok(None);
        };
        self.check_lineage(subject).map_err(KindResolutionError::Read)?;
        self.check_lineage(governing).map_err(KindResolutionError::Read)?;
        let category = self.category_in_lineage(subject)?;
        Ok(Some(DescribingCompatibility {
            category_matches: category
                .as_ref()
                .map(|category| lineage_contains(governing.members(), category)),
            meta_type_corresponds: lineage_contains(
                subject.members(),
                &self.reader.select(&self.type_descriptor).map_err(KindResolutionError::Read)?,
            ) == lineage_contains(
                governing.members(),
                &self.reader.select(&self.meta_type).map_err(KindResolutionError::Read)?,
            ),
        }))
    }
}
