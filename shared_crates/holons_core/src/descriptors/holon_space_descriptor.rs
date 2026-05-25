use crate::descriptors::{accessor_helpers, Descriptor, TransactionDescriptor, TypeHeader};
use crate::reference_layer::HolonReference;
use core_types::HolonError;
use type_names::CoreRelationshipTypeName;

/// Runtime wrapper for the schema-backed `HolonSpaceType` descriptor.
pub struct HolonSpaceDescriptor {
    holon: HolonReference,
}

impl HolonSpaceDescriptor {
    /// Wraps an already-resolved `HolonSpaceType` descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }

    /// Returns the single transaction command-scope model afforded by this holon-space descriptor.
    pub fn transaction_model(&self) -> Result<TransactionDescriptor, HolonError> {
        // Follow the schema cardinality contract through the descriptor relationship.
        let transaction_model = accessor_helpers::require_single_related(
            &self.holon,
            CoreRelationshipTypeName::AffordsTransactionModel,
        )?;
        Ok(TransactionDescriptor::from_holon(transaction_model))
    }
}

impl From<HolonReference> for HolonSpaceDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for HolonSpaceDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
const _: fn() = || {
    // Compile-time guard: this wrapper must continue implementing Descriptor.
    fn assert_impl<T: Descriptor>() {}
    assert_impl::<HolonSpaceDescriptor>();
};
