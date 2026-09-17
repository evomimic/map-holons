use derive_new::new;
use std::{
    fmt,
    sync::{Arc, RwLock},
};
use tracing::trace;
use type_names::relationship_names::CoreRelationshipTypeName;
use type_names::CorePropertyTypeName;

use crate::core_shared_objects::space_read_handle::SpaceReadHandle;
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        holon::{state::AccessType, HolonCloneModel},
        transient_holon_manager::ToHolonCloneModel,
        Holon, HolonCollection, ReadableHolonState,
    },
    reference_layer::{HolonReference, ReadableHolon, TransientReference},
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue, RelationshipName,
};

#[derive(new, Debug, Clone)]
pub struct SmartReference {
    space_read_handle: SpaceReadHandle,
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

/// Capability token allowing cache access only from this module.
pub(crate) struct SmartRefAccessKey(());

impl SmartRefAccessKey {
    pub(crate) fn new() -> Self {
        Self(())
    }
}

impl SmartReference {
    // *************** CONSTRUCTORS ***************

    /// Constructor for SmartReference that takes a HolonId and sets smart_property_values to None
    pub fn new_from_id(space_read_handle: SpaceReadHandle, holon_id: HolonId) -> Self {
        SmartReference { space_read_handle, holon_id, smart_property_values: None }
    }

    pub fn new_with_properties(
        space_read_handle: SpaceReadHandle,
        holon_id: HolonId,
        smart_property_values: PropertyMap,
    ) -> Self {
        SmartReference {
            space_read_handle,
            holon_id,
            smart_property_values: Some(smart_property_values),
        }
    }

    // *************** ACCESSORS ***************

    /// Returns the persistent holon id for this smart reference.
    pub fn holon_id(&self) -> HolonId {
        self.holon_id.clone()
    }

    /// Returns a borrowed view of the cached smart properties, if present.
    pub fn smart_property_values(&self) -> Option<&PropertyMap> {
        self.smart_property_values.as_ref()
    }

    /// Opens an internal restricted context for descriptor resolution rooted in
    /// this reference's space. It is not retained by the saved reference.
    pub(crate) fn resolution_context(
        &self,
    ) -> Result<Arc<crate::core_shared_objects::transactions::TransactionContext>, HolonError> {
        self.space_read_handle.open_resolution_context()
    }

    /// Returns the persistent holon id (kept for backwards compatibility with existing call sites).
    ///
    /// Prefer `holon_id()` for new code.
    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id())
    }

    /// Returns an owned clone of cached smart properties (kept for backwards compatibility).
    ///
    /// Prefer `smart_property_values()` for new code to avoid cloning.
    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
    }

    // *************** UTILITY METHODS ***************

    fn get_rc_holon(&self) -> Result<Arc<RwLock<Holon>>, HolonError> {
        let rc_holon = self.space_read_handle.get_rc_holon(&self.holon_id)?;
        trace!("Got a reference to rc_holon from the cache manager: {:#?}", rc_holon);

        Ok(rc_holon)
    }

    // Simple string representations for errors/logging
    pub fn reference_kind_string(&self) -> String {
        "SmartReference".to_string()
    }

    pub fn reference_id_string(&self) -> String {
        format!("HolonId={:?}", self.holon_id)
    }
}

impl fmt::Display for SmartReference {
    /// Formats the `SmartReference` for human-readable display.
    ///
    /// The output includes the `HolonId` and a summary of smart property values:
    /// - If `smart_property_values` is `None` or empty, displays `"no props"`.
    /// - If there are 1–2 properties, displays them as key-value pairs.
    /// - If there are more than 2 properties, displays the first two followed by `"+N more"`.
    ///
    /// # Example Outputs
    /// - `SmartReference(Local(…ABC123), no props)`
    /// - `SmartReference(Local(…ABC123), props: [key1:value1, key2:value2])`
    /// - `SmartReference(Local(…ABC123), props: [key1:value1, key2:value2 +1 more])`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.smart_property_values {
            Some(props) if !props.is_empty() => {
                // Display the first 2 properties as a preview, followed by a count if there are more
                let preview: Vec<String> = props
                    .iter()
                    .take(2)
                    .map(|(key, value)| format!("{}:{:?}", key, value))
                    .collect();

                let additional_count = props.len().saturating_sub(preview.len());

                if additional_count > 0 {
                    write!(
                        f,
                        "SmartReference({}, props: [{} +{} more])",
                        self.holon_id,
                        preview.join(", "),
                        additional_count
                    )
                } else {
                    write!(f, "SmartReference({}, props: [{}])", self.holon_id, preview.join(", "))
                }
            }
            _ => write!(f, "SmartReference({}, no props)", self.holon_id),
        }
    }
}

impl ReadableHolonImpl for SmartReference {
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError> {
        Err(HolonError::NotImplemented(
            "Clone a saved holon through TransactionContext::clone_holon".to_owned(),
        ))
    }

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        self.space_read_handle.get_all_related_holons(&self.holon_id)
    }

    fn property_map_impl(&self) -> Result<PropertyMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for property_map_impl: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.property_map_clone())
    }

    fn holon_id_impl(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for into_model_impl: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for is_accessible_impl: {}",
                e
            ))
        })?;
        holon.is_accessible(access_type)?;

        Ok(())
    }

    fn is_committed_source_impl(&self) -> Result<bool, HolonError> {
        Ok(true)
    }

    fn holon_reference_impl(&self) -> HolonReference {
        self.into()
    }

    /// Extracts the Holon's primary key from `smart_property_values` or, if not found there,
    /// from its referenced holon. Returns `Ok(Some(MapString))` if found, or `Ok(None)` if absent.
    fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
        let key_prop = CorePropertyTypeName::Key.as_property_name();

        match self.property_value_impl(&key_prop)? {
            Some(BaseValue::StringValue(s)) => Ok(Some(s.clone())),

            // Key exists but is wrong value type (e.g., enum?)
            Some(other) => Err(HolonError::InvalidType(format!(
                "Key property must be a StringValue, found {:?}",
                other
            ))),

            None => Ok(None),
        }
    }

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let collection_arc = self.related_holons(CoreRelationshipTypeName::Predecessor)?;
        let collection = collection_arc.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon collection in predecessor_impl: {}",
                e
            ))
        })?;
        collection.is_accessible(AccessType::Read)?;
        let members = collection.get_members();
        if members.len() > 1 {
            return Err(HolonError::Misc(format!(
                "related_holons for PREDECESSOR returned multiple members: {:#?}",
                members
            )));
        }
        if members.is_empty() {
            Ok(None)
        } else {
            Ok(Some(members[0].clone()))
        }
    }

    /// `property_value` returns the value for the specified property name
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will do a get_rc_holon from the cache
    /// manager in the context.
    ///
    /// Returns: Option, None if property for given name does not exist in its PropertyMap.
    fn property_value_impl(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        // Check if the property value is available in smart_property_values
        if let Some(smart_map) = &self.smart_property_values {
            if let Some(value) = smart_map.get(property_name) {
                return Ok(Some(value.clone()));
            }
        }

        trace!("unable to get value for {:?} property from smart_property_values. Fetching rc_holon from HolonsCache", property_name);

        self.is_accessible(AccessType::Read)?;

        // Get rc_holon from HolonCacheManager
        let holon = self.get_rc_holon()?;
        let prop_val = holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon in property_value_impl: {}",
                    e
                ))
            })?
            .property_value(property_name)?;
        Ok(prop_val)
    }

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        self.space_read_handle.get_related_holons(&self.holon_id, relationship_name)
    }

    fn summarize_impl(&self) -> Result<String, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on holon for summarize_impl: {}",
                e
            ))
        })?;
        Ok(borrowed_holon.summarize())
    }

    fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let holon = self.get_rc_holon()?;
        let key = holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon for versioned_key_impl: {}",
                    e
                ))
            })?
            .versioned_key()?;

        Ok(key)
    }
}

// Convenience trait implementation for working with HolonReference wrappers.
// Functions will always fail since SmartReferences are immutable.
impl WritableHolonImpl for SmartReference {
    fn add_related_holons_impl(
        &mut self,
        _relationship_name: RelationshipName,
        _holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        _relationship_name: RelationshipName,
        _holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        _property: PropertyName,
        _value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn remove_property_value_impl(&mut self, _name: PropertyName) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        _descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(())
    }

    fn with_predecessor_impl(
        &mut self,
        _predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        Ok(())
    }
}

impl ToHolonCloneModel for SmartReference {
    fn holon_clone_model(&self) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon for holon_clone_model: {}",
                    e
                ))
            })?
            .holon_clone_model();

        Ok(model)
    }
}

// ---------- SmartReference equality ----------
//
// Equality for saved references is based on persistent identity. Local ids are
// scoped by the owning local space; external ids carry that scope themselves.

impl PartialEq for SmartReference {
    fn eq(&self, other: &Self) -> bool {
        if self.holon_id != other.holon_id {
            return false;
        }
        match &self.holon_id {
            HolonId::External(_) => true,
            HolonId::Local(_) => self.space_read_handle.same_space(&other.space_read_handle),
        }
        // NOTE: We intentionally do *not* include `smart_property_values` in equality.
        // Those are cached hints and do not change the identity of the referenced holon.
    }
}

impl Eq for SmartReference {}
