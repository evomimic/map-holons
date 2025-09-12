use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, fmt, rc::Rc, sync::Arc};
use tracing::debug;
use type_names::{
    relationship_names::{CoreRelationshipTypeName, ToRelationshipName},
    ToPropertyName,
};

use base_types::{BaseValue, MapString};
use core_types::{HolonError, HolonId, TemporaryId};
use integrity_core_types::{HolonNodeModel, PropertyName, PropertyValue, RelationshipName};

use crate::{
    core_shared_objects::{
        holon::{
            holon_utils::EssentialHolonContent, state::AccessType, Holon, HolonBehavior,
            HolonCloneModel, TransientHolon,
        },
        transient_holon_manager::ToHolonCloneModel,
        TransientManagerAccess, TransientRelationshipMap,
    },
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, ReadableHolonReferenceLayer,
        WriteableHolon, WriteableHolonReferenceLayer,
    },
    HolonCollection, RelationshipMap,
};

#[derive(new, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransientReference {
    id: TemporaryId,
}

impl TransientReference {
    /// Creates a new `TransientReference` from a given TemporaryId without validation.
    ///
    /// # Arguments
    /// * `id` - A TemporaryId
    ///
    /// # Returns
    /// A new `TransientReference` wrapping the provided id.
    ///
    pub fn from_temporary_id(id: &TemporaryId) -> Self {
        TransientReference::new(id.clone())
    }

    /// Retrieves a shared reference to the holon with interior mutability.
    ///
    /// # Arguments
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
    ///
    /// # Returns
    /// Rc<RefCell<TransientHolon>>>
    ///
    fn get_rc_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Rc<RefCell<TransientHolon>>, HolonError> {
        // Get TransientManagerAccess
        let transient_manager_access = Self::get_transient_manager_access(context);

        let transient_read = transient_manager_access.borrow();

        // Retrieve the holon by its TemporaryId
        let rc_holon = transient_read.get_holon_by_id(&self.id)?;

        // Confirm it references a TransientHolon and return an Rc<RefCell
        let holon = rc_holon.borrow();
        match holon.clone() {
            Holon::Transient(transient_holon) => Ok(Rc::new(RefCell::new(transient_holon))),
            _ => Err(HolonError::InvalidHolonReference("The TemporaryId associated with a TransientReference must return a TransientHolon!".to_string()))

        }
    }

    pub fn get_temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }

    /// Retrieves access to the TransientHolonManager via the provided context.
    ///
    /// # Arguments
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
    ///
    /// # Returns
    /// A reference to an object implementing the `TransientManagerAccess` trait.
    ///
    /// # Panics
    /// This function assumes that the context and space manager will always return valid references.
    pub fn get_transient_manager_access(
        context: &dyn HolonsContextBehavior,
    ) -> Arc<RefCell<dyn TransientManagerAccess>> {
        // Retrieve the space manager from the context
        let space_manager = context.get_space_manager();

        // Get the nursery access
        space_manager.get_transient_manager_access()
    }

    pub fn reset_original_id(&self, context: &dyn HolonsContextBehavior) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut borrow = rc_holon.borrow_mut();
        borrow.update_original_id(None)
    }

    pub fn update_relationship_map(
        &self,
        context: &dyn HolonsContextBehavior,
        map: TransientRelationshipMap,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut borrow = rc_holon.borrow_mut();
        borrow.update_relationship_map(map)
    }
}

impl fmt::Display for TransientReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransientReference(id: {:?})", self.id)
    }
}

// ==========================
//   TRAIT IMPLEMENTATIONS
// ==========================

impl ReadableHolonReferenceLayer for TransientReference {
    fn clone_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        self.is_accessible(context, AccessType::Clone)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.borrow().get_holon_clone_model();

        let transient_behavior_service =
            context.get_space_manager().get_transient_behavior_service();
        let transient_behavior = transient_behavior_service.borrow();

        let cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(holon_clone_model)?;

        Ok(cloned_holon_transient_reference)
    }

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        Ok(RelationshipMap::from(borrowed_holon.get_transient_relationship_map()?))
    }

    fn get_holon_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        let local_id = borrowed_holon.get_local_id()?;

        Ok(HolonId::from(local_id))
    }

    fn get_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let collection = self.get_related_holons(context, CoreRelationshipTypeName::Predecessor)?;
        collection.is_accessible(AccessType::Read)?;
        let members = collection.get_members();
        if members.len() > 1 {
            return Err(HolonError::Misc(format!(
                "get_related_holons for PREDECESSOR returned multiple members: {:#?}",
                members
            )));
        }
        if members.is_empty() {
            Ok(None)
        } else {
            Ok(Some(members[0].clone()))
        }
    }

    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.get_property_value(property_name)
    }

    fn get_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.get_key().clone()
    }

    fn get_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow();

        // Use the public `get_related_holons` method on the `TransientRelationshipMap`
        Ok(holon.get_related_holons(relationship_name)?)
    }

    fn get_versioned_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let holon = self.get_rc_holon(context)?;
        let key = holon.borrow().get_versioned_key()?;

        Ok(key)
    }

    fn into_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        Ok(borrowed_holon.into_node())
    }

    fn is_accessible(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow();
        holon.is_accessible(access_type)?;

        Ok(())
    }
}

impl WriteableHolonReferenceLayer for TransientReference {
    fn add_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered TransientReference::add_related_holons");
        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get access to the source holon and its relationshp map
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow_mut();
        let mut transient_relationship_map = holon.get_transient_relationship_map()?;

        debug!(
            "Here is the RelationshipMap before adding related Holons: {:#?}",
            transient_relationship_map
        );

        // Delegate adding holons to the `TransientRelationshipMap`
        transient_relationship_map.add_related_holons(context, relationship_name, holons)?;

        debug!(
            "Here is the RelationshipMap after adding related Holons: {:#?}",
            transient_relationship_map
        );

        Ok(())
    }

    fn remove_property_value_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        name: PropertyName,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.borrow_mut();

        holon_refcell.remove_property_value(&name)?;

        Ok(())
    }

    fn remove_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered TransientReference::remove_related_holons");

        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get access to the source holon and its relationship map
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow_mut();
        let mut staged_relationship_map = holon.get_transient_relationship_map()?;

        debug!(
            "Here is the RelationshipMap before removing related Holons: {:#?}",
            staged_relationship_map
        );

        // Delegate the removal of holons to the `StagedRelationshipMap`
        staged_relationship_map.remove_related_holons(context, &relationship_name, holons)?;

        debug!(
            "Here is the RelationshipMap after removing related Holons: {:#?}",
            staged_relationship_map
        );

        Ok(())
    }

    fn with_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let existing_descriptor_option = descriptor_reference.get_descriptor(context)?;
        if let Some(descriptor) = existing_descriptor_option {
            self.remove_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor.clone()],
            )?;
            debug!("removed existing descriptor: {:#?}", descriptor);
            self.add_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference],
            )?;
            debug!("added descriptor: {:#?}", descriptor);

            Ok(())
        } else {
            self.add_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference.clone()],
            )?;
            debug!("added descriptor: {:#?}", descriptor_reference);

            Ok(())
        }
    }

    fn with_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
        predecessor_reference_option: Option<HolonReference>, // None passed just removes predecessor
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let existing_predecessor_option = self.clone().get_predecessor(context)?;
        if let Some(predecessor) = existing_predecessor_option {
            self.remove_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor.clone()],
            )?;
            debug!("removed existing predecessor: {:#?}", predecessor);
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor_reference.clone()],
            )?;
            debug!("added predecessor: {:#?}", predecessor_reference);
        }

        Ok(())
    }

    fn with_property_value_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.borrow_mut();

        // Call the Holon's with_property_value method
        holon_refcell.with_property_value(property, value)?;

        Ok(())
    }
}

impl WriteableHolon for TransientReference {
    fn add_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        let relationship_name = name.to_relationship_name();
        self.add_related_holons_ref_layer(context, relationship_name, holons)
    }

    fn with_property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        value: BaseValue,
    ) -> Result<(), HolonError> {
        self.with_property_value_ref_layer(context, name.to_property_name(), value)
    }

    fn remove_property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
    ) -> Result<(), HolonError> {
        self.remove_property_value_ref_layer(context, name.to_property_name())
    }

    fn remove_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.remove_related_holons_ref_layer(context, name.to_relationship_name(), holons)
    }
}

impl ToHolonCloneModel for TransientReference {
    fn get_holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.borrow().get_holon_clone_model();

        Ok(holon_clone_model)
    }
}
