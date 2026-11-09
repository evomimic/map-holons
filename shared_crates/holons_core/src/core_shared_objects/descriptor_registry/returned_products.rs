//! Unpublished, validated products carried from transport binding to Commit reconciliation.
use super::*;
use crate::core_shared_objects::{
    holon::state::{HolonState, ValidationState},
    space_session::SpaceSession,
    Holon, ReadableHolonState, TransientHolon, TransientRelationshipMap,
};
use std::{collections::BTreeMap, sync::RwLock};
use type_names::ToRelationshipName;

/// A detached graph containing only sealed results and saved schema references.
/// Construction is restricted to a validated registry; dropping it has no live-cache effect.
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnedDescriptorProducts {
    entries: Vec<(HolonId, bool, Holon)>,
    root: Holon,
}

impl EffectiveHolonDescriptorRegistry {
    /// Extracts selected products without retaining their temporary transport owner.
    pub fn returned_products(&self) -> Result<Option<ReturnedDescriptorProducts>, HolonError> {
        self.capture(|owner, root_id| {
            let state = self.state.lock().map_err(lock_error)?;
            let pool = owner.export_transient_holons()?;
            let root = pool.get_holon_by_id(root_id)?.read().map_err(lock_error)?.clone();
            let model = root.holon_clone_model();
            let mut metadata = BTreeMap::new();
            if let Some(relationships) = model.relationships {
                for (name, collection) in relationships.iter() {
                    if name == CoreRelationshipTypeName::PreparedDescriptors.to_relationship_name()
                        || name
                            == CoreRelationshipTypeName::PreparedDerivedDescriptors
                                .to_relationship_name()
                    {
                        continue;
                    }
                    let collection = collection.read().map_err(lock_error)?;
                    if collection
                        .get_members()
                        .iter()
                        .any(|reference| !matches!(reference, HolonReference::Smart(_)))
                    {
                        return Err(HolonError::InvalidParameter(
                            "Registry metadata must have saved dependencies".into(),
                        ));
                    }
                    metadata
                        .insert(name, Arc::new(RwLock::new(collection.clone_for_new_source()?)));
                }
            }
            let root = Holon::Transient(TransientHolon::from_parts(
                model.version,
                HolonState::Mutable,
                ValidationState::ValidationRequired,
                model.properties,
                TransientRelationshipMap::new(metadata),
                model.original_id,
            ));
            let mut entries = Vec::new();
            for (derived, selected) in [(false, &state.entries), (true, &state.derived_entries)] {
                for (target, (id, _)) in selected {
                    let raw = pool.get_holon_by_id(id)?.read().map_err(lock_error)?.clone();
                    let reference: HolonReference =
                        TransientReference::from_temporary_id(owner.context_handle(), id).into();
                    validate_returned_product(&reference, derived)?;
                    entries.push((target.clone(), derived, raw));
                }
            }
            Ok(ReturnedDescriptorProducts { entries, root })
        })
    }

    /// Merges a validated return into its originating active Session. Live selections win
    /// duplicate exact identities, including products published while Commit was in flight.
    /// Pool membership, root and both indexes are published together or left untouched.
    pub fn merge_returned_products(
        &self,
        products: ReturnedDescriptorProducts,
        session: &Arc<SpaceSession>,
    ) -> Result<(), HolonError> {
        let space = self
            .space
            .upgrade()
            .ok_or_else(|| HolonError::ServiceNotAvailable("Session space".into()))?;
        if !Arc::ptr_eq(&space.session()?, session) {
            return Err(HolonError::ObsoleteSession);
        }
        session.check_active()?;
        let owner = self.context()?;
        session.publish(|| {
            let mut state = self.state.lock().map_err(lock_error)?;
            let mut entries = state.entries.clone();
            let mut derived_entries = state.derived_entries.clone();
            let mut admitted = state.admitted.clone();
            let mut root_id = state.root.clone();
            owner.update_transient_holons(&mut |pool| {
                for (target, derived, raw) in &products.entries {
                    let index = if *derived { &mut derived_entries } else { &mut entries };
                    if index.contains_key(target) {
                        continue;
                    }
                    let id = pool.insert_holon(raw.clone())?;
                    admitted.insert(id.clone());
                    index.insert(target.clone(), (id, session.clone()));
                }
                let metadata = match &state.root {
                    Some(id) => pool.get_holon_by_id(id)?.read().map_err(lock_error)?.clone(),
                    None => products.root.clone(),
                };
                let model = metadata.holon_clone_model();
                let mut relationships = BTreeMap::new();
                if let Some(existing) = model.relationships {
                    for (name, collection) in existing.iter() {
                        if name
                            != CoreRelationshipTypeName::PreparedDescriptors.to_relationship_name()
                            && name
                                != CoreRelationshipTypeName::PreparedDerivedDescriptors
                                    .to_relationship_name()
                        {
                            relationships.insert(
                                name.clone(),
                                Arc::new(RwLock::new(
                                    collection
                                        .read()
                                        .map_err(lock_error)?
                                        .clone_for_new_source()?,
                                )),
                            );
                        }
                    }
                }
                for (name, index) in [
                    (CoreRelationshipTypeName::PreparedDescriptors, &entries),
                    (CoreRelationshipTypeName::PreparedDerivedDescriptors, &derived_entries),
                ] {
                    let members = index
                        .values()
                        .map(|(id, _)| {
                            TransientReference::from_temporary_id(owner.context_handle(), id).into()
                        })
                        .collect();
                    relationships.insert(
                        name.to_relationship_name(),
                        Arc::new(RwLock::new(crate::HolonCollection::from_parts(
                            crate::CollectionState::Transient,
                            members,
                            Default::default(),
                        ))),
                    );
                }
                let root = Holon::Transient(TransientHolon::from_parts(
                    model.version,
                    HolonState::Mutable,
                    ValidationState::ValidationRequired,
                    model.properties,
                    TransientRelationshipMap::new(relationships),
                    model.original_id,
                ));
                // Replacing a root never replaces any prepared product or construction envelope.
                if let Some(id) = &root_id {
                    let mut holons = pool.holons_by_id().clone();
                    holons.insert(id.clone(), Arc::new(RwLock::new(root)));
                    *pool = crate::core_shared_objects::HolonPool::from_parts(
                        holons,
                        pool.keyed_index().clone(),
                    );
                } else {
                    root_id = Some(pool.insert_holon(root)?);
                }
                Ok(())
            })?;
            state.root = root_id;
            state.entries = entries;
            state.derived_entries = derived_entries;
            state.admitted = admitted;
            Ok(())
        })
    }
}

/// Validate serialized prepared metadata without schema reads or semantic construction.
fn validate_returned_product(reference: &HolonReference, derived: bool) -> Result<(), HolonError> {
    use crate::descriptors::{PreparedSchema, ValueDescriptorKind};
    use type_names::CorePropertyTypeName as P;
    fn scalar<T: serde::de::DeserializeOwned>(
        reference: &HolonReference,
        name: P,
    ) -> Result<T, HolonError> {
        match reference.property_value(name)? {
            Some(base_types::BaseValue::StringValue(encoded)) => serde_json::from_str(&encoded.0)
                .map_err(|error| HolonError::InvalidParameter(error.to_string())),
            _ => Err(HolonError::InvalidParameter("Missing prepared descriptor metadata".into())),
        }
    }
    if reference.is_accessible(crate::core_shared_objects::holon::state::AccessType::Write).is_ok()
    {
        return Err(HolonError::InvalidParameter("Returned product must be sealed".into()));
    }
    for (_, collection) in reference.all_related_holons()?.iter() {
        if collection.read().map_err(lock_error)?.get_state() != crate::CollectionState::Immutable {
            return Err(HolonError::InvalidParameter(
                "Returned product collections must be sealed".into(),
            ));
        }
    }
    EffectiveHolonDescriptorRegistry::ensure_saved_references(reference)?;
    if !matches!(reference.get_descriptor()?, Some(HolonReference::Smart(_))) {
        return Err(HolonError::InvalidParameter(
            "Returned product requires a saved describing type".into(),
        ));
    }
    let anchor: Result<Option<usize>, HolonError> = scalar(reference, P::DerivedInstanceTypeKind)?;
    let lineage = if derived {
        let descriptor = DerivedDescriptor::from_constructed_holon(reference.clone());
        let lineage = descriptor.lineage()?;
        let _: Result<ValueDescriptorKind, HolonError> = scalar(reference, P::DerivedValueKind)?;
        let _: Option<
            Result<crate::descriptors::derived_descriptor::DerivedRelationshipPolicy, HolonError>,
        > = scalar(reference, P::DerivedRelationshipPolicy)?;
        descriptor.validate_prepared_collections()?;
        lineage.len()
    } else {
        let prepared: PreparedSchema = scalar(reference, P::PreparedSchema)?;
        let descriptor = EffectiveHolonDescriptor::from_constructed_holon(reference.clone());
        if prepared.lineage.first() != Some(&descriptor.described_holon_type()?.holon().holon_id()?)
        {
            return Err(HolonError::InvalidParameter(
                "Prepared lineage does not start at described type".into(),
            ));
        }
        crate::descriptors::ComputedEffectiveSchema::from_prepared(&descriptor)?;
        prepared.lineage.len()
    };
    if matches!(anchor, Ok(Some(index)) if index >= lineage) {
        return Err(HolonError::InvalidParameter("Derived kind anchor outside lineage".into()));
    }
    Ok(())
}
