use crate::descriptors::{Descriptor, TypeHeader};
use crate::reference_layer::HolonReference;

/// Runtime wrapper for holon-type descriptors.
///
/// This is the main descriptor surface that callers will reach from ordinary
/// holon instances via `ReadableHolon::holon_descriptor()`.
pub struct HolonDescriptor {
    holon: HolonReference,
}

impl HolonDescriptor {
    /// Wraps an already-resolved descriptor holon reference.
    pub fn from_holon(holon: HolonReference) -> Self {
        Self { holon }
    }

    /// Projects the shared descriptor header view for this descriptor holon.
    pub fn header(&self) -> TypeHeader<'_> {
        TypeHeader::new(&self.holon)
    }
}

impl From<HolonReference> for HolonDescriptor {
    fn from(holon: HolonReference) -> Self {
        Self::from_holon(holon)
    }
}

impl Descriptor for HolonDescriptor {
    fn holon(&self) -> &HolonReference {
        &self.holon
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_shared_objects::transactions::TransactionContext;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use crate::reference_layer::{ReadableHolon, WritableHolon};
    use crate::TransientReference;
    use base_types::MapString;
    use core_types::HolonError;
    use std::sync::Arc;
    use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

    fn new_descriptor_holon(
        context: &Arc<TransactionContext>,
        key: &str,
        type_name: &str,
    ) -> Result<TransientReference, HolonError> {
        // Descriptor tests only need the shared header surface in this phase.
        let mut descriptor = new_test_holon(context, key)?;
        descriptor
            .with_property_value(CorePropertyTypeName::TypeName, type_name)?
            .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
            .with_property_value(CorePropertyTypeName::InstanceTypeKind, "Holon")?;
        Ok(descriptor)
    }

    fn assert_is_descriptor<T: Descriptor>(descriptor: &T) {
        // Compile-time trait membership plus one trivial runtime use.
        let _ = descriptor.holon().reference_id_string();
    }

    #[test]
    fn wraps_reference_and_exposes_shared_header() -> Result<(), HolonError> {
        let context = build_context();
        let holon =
            HolonReference::from(&new_descriptor_holon(&context, "holon-descriptor", "HolonType")?);

        let descriptor = HolonDescriptor::from_holon(holon.clone());

        assert_eq!(descriptor.holon(), &holon);
        assert_eq!(descriptor.header().type_name()?, MapString("HolonType".to_string()));
        assert_is_descriptor(&descriptor);

        Ok(())
    }

    #[test]
    fn holon_descriptor_resolves_for_transient_source() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor =
            new_descriptor_holon(&context, "descriptor-transient", "TransientDescriptor")?;
        let mut source = new_test_holon(&context, "source-transient")?;
        source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![descriptor.clone().into()],
        )?;

        let resolved = source.holon_descriptor()?;

        assert_eq!(resolved.header().type_name()?, MapString("TransientDescriptor".to_string()));
        assert_eq!(resolved.holon(), &HolonReference::from(&descriptor));
        assert_is_descriptor(&resolved);

        Ok(())
    }

    #[test]
    fn holon_descriptor_resolves_for_staged_source() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor = new_descriptor_holon(&context, "descriptor-staged", "StagedDescriptor")?;
        let staged_descriptor = context.mutation().stage_new_holon(descriptor)?;
        let source = new_test_holon(&context, "source-staged")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![staged_descriptor.into()],
        )?;

        let resolved = staged_source.holon_descriptor()?;

        assert_eq!(resolved.header().type_name()?, MapString("StagedDescriptor".to_string()));
        assert_is_descriptor(&resolved);

        Ok(())
    }

    #[test]
    fn holon_descriptor_errors_when_described_by_missing() -> Result<(), HolonError> {
        let context = build_context();
        let source = new_test_holon(&context, "missing-descriptor")?;

        assert!(matches!(source.holon_descriptor(), Err(HolonError::MissingDescribedBy { .. })));

        Ok(())
    }

    #[test]
    fn holon_descriptor_errors_when_multiple_described_by_present() -> Result<(), HolonError> {
        let context = build_context();
        let descriptor_a = new_descriptor_holon(&context, "descriptor-a", "DescriptorA")?;
        let descriptor_b = new_descriptor_holon(&context, "descriptor-b", "DescriptorB")?;
        let mut source = new_test_holon(&context, "multiple-descriptor-source")?;
        source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![descriptor_a.into(), descriptor_b.into()],
        )?;

        assert!(matches!(
            source.holon_descriptor(),
            Err(HolonError::MultipleDescribedBy { count, .. }) if count == 2
        ));

        Ok(())
    }
}
