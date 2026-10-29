use std::collections::HashSet;
use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::TypeHeader;
use crate::reference_layer::{assert_reference_transaction_compatible, HolonReference};

/// Universal property and relationship identities for one validation pass.
///
/// Membership comes from the effective instance contract of
/// `MetaTypeDescriptor.HolonType`, including member inheritance policies. It is
/// computed once, never inferred from a member's name or declaration provenance.
/// Discard this value after the pass, before changing the schema snapshot.
#[derive(Clone, Debug)]
pub struct UniversalDescriptorContract {
    member_ids: HashSet<crate::DefinitionIdentity>,
    context: Arc<TransactionContext>,
}

impl UniversalDescriptorContract {
    /// Computes universal member identities from the exact meta-type definition reached
    /// through a subject's governing lineage. Prepare this before consuming conformance.
    pub fn from_meta_type(
        context: &Arc<TransactionContext>,
        meta_type: &HolonReference,
    ) -> Result<Self, HolonError> {
        assert_reference_transaction_compatible(meta_type, context)?;
        let contributions = super::ContractContributions::resolve(meta_type)?;
        let member_ids = contributions
            .properties
            .iter()
            .chain(&contributions.relationships)
            .map(|contribution| {
                crate::DefinitionIdentity::for_reference(&contribution.member, context)
            })
            .collect::<Result<_, HolonError>>()?;
        Ok(Self { member_ids, context: Arc::clone(context) })
    }

    /// Implements `EnforceMinimum(H, M)` for a member of H's conformance contract.
    ///
    /// Concrete holons enforce every minimum. Abstract descriptor holons may omit
    /// category-specific members, but universal structure remains required. This
    /// predicate does not relax validation of a populated member.
    /// `is_descriptor` comes from the caller's structural diagnosis; this operation
    /// reads abstractness lazily without walking inheritance or recognizing roles.
    pub fn enforce_minimum(
        &self,
        holon: &HolonReference,
        is_descriptor: bool,
        member: &HolonReference,
    ) -> Result<bool, HolonError> {
        assert_reference_transaction_compatible(holon, &self.context)?;
        assert_reference_transaction_compatible(member, &self.context)?;
        let is_abstract = is_descriptor && TypeHeader::new(holon).is_abstract_type()?;
        Ok(!is_abstract
            || self
                .member_ids
                .contains(&crate::DefinitionIdentity::for_reference(member, &self.context)?))
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
        let baseline = context.mutation().stage_new_holon(baseline)?;

        let contract = UniversalDescriptorContract::from_meta_type(&context, &baseline.into())?;
        let category_member: HolonReference = new_test_holon(&context, "category-member")?.into();
        let mut descriptor = new_descriptor_holon(&context, "descriptor", "Example", "Property")?;
        descriptor.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.into()])?;
        assert!(contract.enforce_minimum(&(&descriptor).into(), true, &category_member)?);
        descriptor.with_property_value(CorePropertyTypeName::IsAbstractType, true)?;
        assert!(!contract.enforce_minimum(&(&descriptor).into(), true, &category_member)?);
        assert!(contract.enforce_minimum(
            &(&descriptor).into(),
            true,
            &universal_property.into()
        )?);
        assert!(contract.enforce_minimum(
            &(&descriptor).into(),
            true,
            &universal_relationship.into()
        )?);

        // Neither a familiar label nor an authored Boolean classifies an ordinary holon.
        let mut ordinary = new_descriptor_holon(&context, "ordinary", "TypeDescriptor", "Holon")?;
        ordinary.with_property_value(CorePropertyTypeName::IsAbstractType, true)?;
        assert!(contract.enforce_minimum(&ordinary.into(), false, &category_member)?);
        let ordinary = new_test_holon(&context, "ordinary-without-abstractness")?;
        assert!(contract.enforce_minimum(&ordinary.into(), false, &category_member)?);
        Ok(())
    }

    #[test]
    fn missing_descriptor_abstractness_is_an_error_not_an_exemption() -> Result<(), HolonError> {
        let context = build_context();
        let root: HolonReference = new_test_holon(&context, "root")?.into();
        let baseline = new_test_holon(&context, "baseline")?.into();
        let contract = UniversalDescriptorContract::from_meta_type(&context, &baseline)?;
        let mut descriptor = new_test_holon(&context, "incomplete-descriptor")?;
        descriptor.add_related_holons(CoreRelationshipTypeName::Extends, vec![root])?;
        let member: HolonReference = new_test_holon(&context, "member")?.into();
        assert!(contract.enforce_minimum(&descriptor.into(), true, &member).is_err());
        let foreign = new_test_holon(&build_context(), "foreign")?;
        assert!(matches!(
            contract.enforce_minimum(&foreign.into(), true, &member),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        Ok(())
    }
}
