use std::collections::HashSet;
use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::resolved_descriptor_roots::resolve_core_descriptor;
use crate::descriptors::{equals_or_extends, Descriptor, HolonDescriptor, TypeHeader};
use crate::reference_layer::{assert_reference_transaction_compatible, HolonReference};

/// Universal property and relationship identities for one validation pass.
///
/// Membership comes from the effective instance contract of
/// `MetaTypeDescriptor.HolonType`, including member inheritance policies. It is
/// computed once, never inferred from a member's name or declaration provenance.
/// Discard this value after the pass, before changing the schema snapshot.
#[derive(Clone, Debug)]
pub struct UniversalDescriptorContract {
    descriptor_root: HolonReference,
    member_ids: HashSet<crate::ProspectiveIdentity>,
    context: Arc<TransactionContext>,
}

impl UniversalDescriptorContract {
    /// Resolves the Core baseline and descriptor-classification root by unique key.
    /// The latter ensures that ordinary holons cannot claim the abstract exemption
    /// merely by carrying an `IsAbstractType` property.
    pub fn resolve(context: &Arc<TransactionContext>) -> Result<Self, HolonError> {
        let meta_type = resolve_core_descriptor(context, "MetaTypeDescriptor.HolonType")?;
        let descriptor_root = resolve_core_descriptor(context, "TypeDescriptor")?;
        Self::from_resolved(context, &HolonDescriptor::from_holon(meta_type), descriptor_root)
    }

    fn from_resolved(
        context: &Arc<TransactionContext>,
        meta_type: &HolonDescriptor,
        descriptor_root: HolonReference,
    ) -> Result<Self, HolonError> {
        assert_reference_transaction_compatible(meta_type.holon(), context)?;
        assert_reference_transaction_compatible(&descriptor_root, context)?;
        let mut member_ids = HashSet::new();
        for property in meta_type.instance_properties()? {
            member_ids
                .insert(crate::ProspectiveIdentity::for_reference(property.holon(), context)?);
        }
        for relationship in meta_type.instance_relationships()? {
            member_ids
                .insert(crate::ProspectiveIdentity::for_reference(relationship.holon(), context)?);
        }
        Ok(Self { descriptor_root, member_ids, context: Arc::clone(context) })
    }

    /// Resolves universal member identities from prospective content for Commit assessment.
    pub fn resolve_with_reader<R: super::DescriptorReader>(
        context: &Arc<TransactionContext>,
        reader: &R,
    ) -> Result<Self, R::Error> {
        let meta_type = super::resolve_core_descriptor_with_reader(
            context,
            "MetaTypeDescriptor.HolonType",
            reader,
        )?;
        let descriptor_root =
            super::resolve_core_descriptor_with_reader(context, "TypeDescriptor", reader)?;
        let contributions = super::ContractContributions::resolve_with_reader(&meta_type, reader)?;
        let member_ids = contributions
            .properties
            .iter()
            .chain(&contributions.relationships)
            .map(|contribution| {
                crate::ProspectiveIdentity::for_reference(&contribution.member, context)
            })
            .collect::<Result<_, HolonError>>()?;
        Ok(Self { descriptor_root, member_ids, context: Arc::clone(context) })
    }

    /// Applies the same minimum policy to the selected subject and member definitions.
    pub fn enforce_minimum_with_reader<R: super::DescriptorReader>(
        &self,
        holon: &HolonReference,
        member: &HolonReference,
        reader: &R,
    ) -> Result<bool, R::Error> {
        let holon = reader.select(holon)?;
        let member = reader.select(member)?;
        assert_reference_transaction_compatible(&holon, &self.context)?;
        let is_abstract =
            super::equals_or_extends_with_reader(&holon, &self.descriptor_root, reader)?
                && TypeHeader::new(&holon).is_abstract_type()?;
        Ok(!is_abstract
            || self
                .member_ids
                .contains(&crate::ProspectiveIdentity::for_reference(&member, &self.context)?))
    }

    /// Implements `EnforceMinimum(H, M)` for a member of H's conformance contract.
    ///
    /// Concrete holons enforce every minimum. Abstract descriptor holons may omit
    /// category-specific members, but universal structure remains required. This
    /// predicate does not relax validation of a populated member.
    pub fn enforce_minimum(
        &self,
        holon: &HolonReference,
        member: &HolonReference,
    ) -> Result<bool, HolonError> {
        assert_reference_transaction_compatible(holon, &self.context)?;
        assert_reference_transaction_compatible(member, &self.context)?;
        let is_abstract = equals_or_extends(holon, &self.descriptor_root)?
            && TypeHeader::new(holon).is_abstract_type()?;
        Ok(!is_abstract
            || self
                .member_ids
                .contains(&crate::ProspectiveIdentity::for_reference(member, &self.context)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_descriptor_holon, new_test_holon};
    use crate::reference_layer::WritableHolon;
    use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};

    #[test]
    fn minimum_enforcement_uses_descriptor_identity_and_universal_contract(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let root =
            context.mutation().stage_new_holon(new_test_holon(&context, "TypeDescriptor")?)?;
        let universal_property =
            context.mutation().stage_new_holon(new_test_holon(&context, "universal-property")?)?;
        let universal_relationship = context
            .mutation()
            .stage_new_holon(new_test_holon(&context, "universal-relationship")?)?;
        let mut baseline_parent = new_test_holon(&context, "baseline-parent")?;
        baseline_parent.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![(&universal_property).into()],
        )?;
        baseline_parent.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![(&universal_relationship).into()],
        )?;
        let baseline_parent = context.mutation().stage_new_holon(baseline_parent)?;
        let mut baseline = new_test_holon(&context, "MetaTypeDescriptor.HolonType")?;
        baseline
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![baseline_parent.into()])?;
        context.mutation().stage_new_holon(baseline)?;

        let contract = UniversalDescriptorContract::resolve(&context)?;
        let category_member: HolonReference = new_test_holon(&context, "category-member")?.into();
        let mut descriptor = new_descriptor_holon(&context, "descriptor", "Example", "Property")?;
        descriptor.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        assert!(contract.enforce_minimum(&(&descriptor).into(), &category_member)?);
        descriptor.with_property_value(CorePropertyTypeName::IsAbstractType, true)?;
        assert!(!contract.enforce_minimum(&(&descriptor).into(), &category_member)?);
        assert!(contract.enforce_minimum(&(&descriptor).into(), &universal_property.into())?);
        assert!(contract.enforce_minimum(&(&descriptor).into(), &universal_relationship.into())?);

        // Neither a familiar label nor an authored Boolean classifies an ordinary holon.
        let mut ordinary = new_descriptor_holon(&context, "ordinary", "TypeDescriptor", "Holon")?;
        ordinary.with_property_value(CorePropertyTypeName::IsAbstractType, true)?;
        assert!(contract.enforce_minimum(&ordinary.into(), &category_member)?);
        let ordinary = new_test_holon(&context, "ordinary-without-abstractness")?;
        assert!(contract.enforce_minimum(&ordinary.into(), &category_member)?);
        Ok(())
    }

    #[test]
    fn missing_descriptor_abstractness_is_an_error_not_an_exemption() -> Result<(), HolonError> {
        let context = build_context();
        let root: HolonReference = new_test_holon(&context, "root")?.into();
        let baseline = HolonDescriptor::from_holon(new_test_holon(&context, "baseline")?.into());
        let contract =
            UniversalDescriptorContract::from_resolved(&context, &baseline, root.clone())?;
        let mut descriptor = new_test_holon(&context, "incomplete-descriptor")?;
        descriptor.add_related_holons(CoreRelationshipTypeName::Extends, vec![root])?;
        let member: HolonReference = new_test_holon(&context, "member")?.into();
        assert!(contract.enforce_minimum(&descriptor.into(), &member).is_err());
        let foreign = new_test_holon(&build_context(), "foreign")?;
        assert!(matches!(
            contract.enforce_minimum(&foreign.into(), &member),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        Ok(())
    }
}
