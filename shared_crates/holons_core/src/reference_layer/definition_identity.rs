use std::sync::Arc;

use core_types::{HolonError, HolonId, TemporaryId};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::assert_reference_transaction_compatible;
use crate::HolonReference;

/// Identity of one exact definition within a transaction-local assessment.
///
/// No-op and graph-only updates retain their saved source's definition identity.
/// Creates and version-producing candidates define their own content, independently
/// of ancestry or matching keys. Candidate IDs remain separate from this identity.
/// Full typed saved IDs retain external space identity. Temporary IDs may only be
/// compared within one transaction; this is not a durable or cross-Commit key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DefinitionIdentity {
    /// One persisted version, also used by its no-op and graph-only handles.
    Saved(HolonId),
    /// A candidate that defines its own content.
    Staged(TemporaryId),
    /// Transient input, distinct from any staged definition.
    Transient(TemporaryId),
}

impl DefinitionIdentity {
    /// Resolves identity without fetching saved content or inspecting semantic keys.
    /// Mutable references must belong to the assessment's active transaction.
    /// Abandoned candidates have no assessable definition identity.
    pub fn for_reference(
        reference: &HolonReference,
        context: &Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        assert_reference_transaction_compatible(reference, context)?;
        Self::for_bound_reference(reference)
    }

    /// Resolves an ordinary bound handle, checking transaction compatibility when
    /// a traversal or comparison has an established transaction context.
    pub(crate) fn for_optional_context(
        reference: &HolonReference,
        context: Option<&Arc<TransactionContext>>,
    ) -> Result<Self, HolonError> {
        match context {
            Some(context) => Self::for_reference(reference, context),
            None => Self::for_bound_reference(reference),
        }
    }

    /// Uses a handle's bound transaction for ordinary descriptor operations.
    /// Callers comparing temporary identities must ensure transaction compatibility.
    pub(crate) fn for_bound_reference(reference: &HolonReference) -> Result<Self, HolonError> {
        use crate::core_shared_objects::holon::state::StagedState;
        match reference {
            HolonReference::Smart(saved) => Ok(Self::Saved(saved.holon_id())),
            HolonReference::Staged(staged) => {
                let (state, source) = staged.staged_state_and_source()?;
                Ok(match state {
                    StagedState::ForUpdate | StagedState::ForUpdateGraphOnly => {
                        let source = source.ok_or_else(|| {
                            HolonError::InvalidState(format!(
                                "Staged update {:?} has no saved source",
                                staged.temporary_id()
                            ))
                        })?;
                        Self::Saved(HolonId::Local(source))
                    }
                    StagedState::Committed(id) => Self::Saved(HolonId::Local(id)),
                    StagedState::ForCreate | StagedState::ForUpdateNewVersion => {
                        Self::Staged(staged.temporary_id())
                    }
                    StagedState::Abandoned => {
                        return Err(HolonError::InvalidState(format!(
                            "Abandoned candidate {:?} has no assessable definition",
                            staged.temporary_id()
                        )))
                    }
                })
            }
            HolonReference::Transient(transient) => Ok(Self::Transient(transient.temporary_id())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use core_types::{ExternalId, LocalId, OutboundProxyId};
    use std::collections::HashSet;

    #[test]
    fn saved_identity_uses_full_ids_and_preserves_external_space() -> Result<(), HolonError> {
        let context = build_context();
        let first = LocalId(vec![1; 39]);
        let mut second = first.clone();
        second.0[0] = 2;
        assert_eq!(first.to_string(), second.to_string());
        let ids = [
            HolonId::Local(first.clone()),
            HolonId::Local(second.clone()),
            HolonId::External(ExternalId {
                space_id: OutboundProxyId(first.clone()),
                local_id: first.clone(),
            }),
            HolonId::External(ExternalId { space_id: OutboundProxyId(second), local_id: first }),
        ];
        let identities = ids
            .into_iter()
            .map(|id| {
                let reference = HolonReference::smart_from_id(context.space_read_handle(), id);
                DefinitionIdentity::for_reference(&reference, &context)
            })
            .collect::<Result<HashSet<_>, HolonError>>()?;
        // No saved content exists in the fixture: identity needs no storage reads.
        assert_eq!(identities.len(), 4);
        Ok(())
    }

    #[test]
    fn mutable_identity_requires_active_transaction() -> Result<(), HolonError> {
        let first = build_context();
        let second = build_context();
        let transient = new_test_holon(&first, "transient")?;
        let staged = first.mutation().stage_new_holon(new_test_holon(&first, "staged")?)?;
        for reference in [HolonReference::from(transient), HolonReference::from(staged)] {
            DefinitionIdentity::for_reference(&reference, &first)?;
            assert!(matches!(
                DefinitionIdentity::for_reference(&reference, &second),
                Err(HolonError::CrossTransactionReference { .. })
            ));
        }
        Ok(())
    }

    #[test]
    fn committed_handle_uses_publication_id_without_saved_reads() -> Result<(), HolonError> {
        use crate::core_shared_objects::Holon;
        use crate::descriptors::test_support::stage_update_snapshot;

        let context = build_context();
        let source = LocalId(vec![1; 39]);
        let published = LocalId(vec![2; 39]);
        let staged =
            stage_update_snapshot(&context, source.clone(), new_test_holon(&context, "update")?)?;
        {
            let rc = staged.get_holon_to_commit(&context)?;
            let mut holon = rc.write().unwrap();
            let Holon::Staged(holon) = &mut *holon else { panic!("staged update") };
            holon.to_committed(published.clone())?;
        }
        let reference = staged.into();
        assert_eq!(
            DefinitionIdentity::for_reference(&reference, &context)?,
            DefinitionIdentity::Saved(HolonId::Local(published.clone()))
        );
        assert!(crate::same_definition(
            &reference,
            &HolonReference::smart_from_id(context.space_read_handle(), HolonId::Local(published))
        )?);
        assert!(!crate::same_definition(
            &reference,
            &HolonReference::smart_from_id(context.space_read_handle(), HolonId::Local(source))
        )?);
        Ok(())
    }

    #[test]
    fn definition_comparison_preserves_reference_errors() -> Result<(), HolonError> {
        use crate::StagedReference;

        let context = build_context();
        let other = build_context();
        let local: HolonReference = new_test_holon(&context, "same-key")?.into();
        let foreign: HolonReference = new_test_holon(&other, "same-key")?.into();
        assert!(matches!(
            crate::same_definition(&local, &foreign),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        let staged = context.mutation().stage_new_holon(new_test_holon(&context, "existing")?)?;
        let missing: HolonReference =
            StagedReference::from_temporary_id(context.context_handle(), &TemporaryId::default())
                .into();
        assert!(matches!(
            crate::same_definition(&staged.into(), &missing),
            Err(HolonError::HolonNotFound(_))
        ));
        Ok(())
    }

    #[test]
    fn membership_search_short_circuits_and_preserves_reference_errors() -> Result<(), HolonError> {
        use crate::descriptors::lineage_contains;
        use crate::StagedReference;

        let context = build_context();
        let other = build_context();
        let anchor: HolonReference =
            context.mutation().stage_new_holon(new_test_holon(&context, "anchor")?)?.into();
        let foreign: HolonReference =
            other.mutation().stage_new_holon(new_test_holon(&other, "anchor")?)?.into();
        let missing: HolonReference =
            StagedReference::from_temporary_id(context.context_handle(), &TemporaryId::default())
                .into();

        assert!(lineage_contains([&anchor, &missing], &anchor)?);
        assert!(!lineage_contains(std::iter::empty::<&HolonReference>(), &missing)?);
        assert!(matches!(
            lineage_contains([&missing, &anchor], &anchor),
            Err(HolonError::HolonNotFound(_))
        ));
        assert!(matches!(
            lineage_contains([&foreign], &anchor),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        Ok(())
    }

    #[test]
    fn extends_walk_distinguishes_definition_identity_from_candidate_identity(
    ) -> Result<(), HolonError> {
        use crate::core_shared_objects::holon::SavedHolon;
        use crate::descriptors::test_support::{
            build_context_with_saved_holons, stage_update_snapshot,
        };
        use crate::{ReadableHolon, WritableHolon};
        use base_types::MapInteger;
        use core_types::PropertyMap;
        use type_names::CoreRelationshipTypeName;

        let source = LocalId(vec![1; 39]);
        let context = build_context_with_saved_holons(
            vec![SavedHolon::new(source.clone(), PropertyMap::new(), None, MapInteger(1))],
            std::collections::HashMap::new(),
        );
        let saved = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(source.clone()),
        );
        let mut snapshot = new_test_holon(&context, "update")?;
        snapshot.add_related_holons(CoreRelationshipTypeName::Extends, vec![saved.clone()])?;
        let mut staged = stage_update_snapshot(&context, source, snapshot)?;
        // An unclassified snapshot aliasing its saved definition cannot yield that
        // definition a second time through another handle.
        assert!(matches!(
            crate::ancestors(&staged.clone().into()),
            Err(HolonError::CyclicExtends { .. })
        ));
        staged.with_property_value("Description", "new definition")?;
        let lineage = crate::ancestors(&staged.clone().into())?;
        assert_eq!(lineage.len(), 2);
        assert_eq!(lineage[1].holon_id()?, saved.holon_id()?);
        assert!(!crate::same_definition(&lineage[0], &lineage[1])?);
        Ok(())
    }

    #[test]
    fn inherited_obligations_retain_exact_definition_and_declaring_identity(
    ) -> Result<(), HolonError> {
        use crate::descriptors::test_support::stage_update_snapshot;
        use crate::{EffectiveRelationshipMember, WritableHolon};

        let context = build_context();
        let source = LocalId(vec![1; 39]);
        let owner_id = LocalId(vec![2; 39]);
        let saved = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(source.clone()),
        );
        let owner = HolonReference::smart_from_id(
            context.space_read_handle(),
            HolonId::Local(owner_id.clone()),
        );
        let mut staged =
            stage_update_snapshot(&context, source, new_test_holon(&context, "member")?)?;
        let mut staged_owner =
            stage_update_snapshot(&context, owner_id, new_test_holon(&context, "owner")?)?;
        let mut contributions = crate::descriptors::ConstraintContributions {
            parent: None,
            inherited: vec![EffectiveRelationshipMember { member: saved, declared_on: owner }],
            effective: vec![EffectiveRelationshipMember {
                member: staged.clone().into(),
                declared_on: staged_owner.clone().into(),
            }],
        };
        assert!(contributions.missing_inherited()?.is_empty());
        staged.with_property_value("Description", "new definition")?;
        assert_eq!(contributions.missing_inherited()?.len(), 1);
        // A same-key create also cannot supply the saved obligation.
        contributions.effective[0].member =
            context.mutation().stage_new_holon(new_test_holon(&context, "member")?)?.into();
        assert_eq!(contributions.missing_inherited()?.len(), 1);
        contributions.effective[0].member = contributions.inherited[0].member.clone();
        staged_owner.with_property_value("Description", "new declaring definition")?;
        assert_eq!(contributions.missing_inherited()?.len(), 1);

        let other = build_context();
        contributions.effective[0].member =
            other.mutation().stage_new_holon(new_test_holon(&other, "foreign member")?)?.into();
        assert!(matches!(
            contributions.missing_inherited(),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        Ok(())
    }
}
