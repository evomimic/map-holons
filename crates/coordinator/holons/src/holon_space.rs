use hdi::prelude::{Deserialize, Path, Serialize};

use holochain_integrity_types::ActionHash;
use holons_integrity::LinkTypes;
use shared_types_holon::{LocalId, MapString, PropertyName, PropertyValue};

use crate::holon::Holon;
use crate::holon_error::HolonError;
use crate::holon_node::{
    create_path_to_holon_node, get_holon_node_by_path, CreatePathInput, GetPathInput,
};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct HolonSpace(pub Holon);

impl HolonSpace {
    pub fn new(holon: Holon) -> HolonSpace {
        HolonSpace(holon)
    }
    pub fn get_description(&self) -> Result<MapString, HolonError> {
        let property_name = PropertyName(MapString("description".to_string()));

        match self.0.get_property_value(&property_name)? {
            PropertyValue::StringValue(name) => Ok(name),
            _ => Err(HolonError::InvalidType(format!(
                "Expected StringValue for '{}'",
                property_name.0
            ))),
        }
    }
    pub fn get_key(&self) -> Result<Option<MapString>, HolonError> {
        self.0.get_key()
    }
    pub fn get_name(&self) -> Result<MapString, HolonError> {
        let property_name = PropertyName(MapString("name".to_string()));

        match self.0.get_property_value(&property_name)? {
            PropertyValue::StringValue(name) => Ok(name),
            _ => Err(HolonError::InvalidType(format!(
                "Expected StringValue for '{}'",
                property_name.0
            ))),
        }
    }
    fn holon_mut(&mut self) -> &mut Holon {
        &mut self.0 // Return a mutable reference to the inner `Holon`
    }
    pub fn into_holon(self) -> Holon {
        self.0.clone()
    }

    /// get_local_holon_space retrieves the local holon space from the persistent store
    /// This currently does a brute force linear search through all saved holons
    /// TODO: Replace this logic with a fetch based on HolonSpace LinkType
    pub fn with_description(&mut self, description: &MapString) -> Result<&mut Self, HolonError> {
        self.holon_mut().with_property_value(
            PropertyName(MapString("description".to_string())),
            description.clone().into_base_value(),
        )?;
        Ok(self)
    }
    /// Sets the name property for the HolonSpace (and currently the "key" property)
    ///
    pub fn with_name(&mut self, name: &MapString) -> Result<&mut Self, HolonError> {
        self.holon_mut()
            .with_property_value(
                PropertyName(MapString("name".to_string())),
                name.clone().into_base_value(),
            )?
            // TODO: drop this once descriptor-based key support is implemented
            .with_property_value(
                PropertyName(MapString("key".to_string())),
                name.clone().into_base_value(),
            )?;
        Ok(self)
    }

    pub fn create_local_path(target_holon_hash: LocalId) -> Result<ActionHash, HolonError> {
        let path = Path::from("local_holon_space");
        let link_type = LinkTypes::LocalHolonSpace;
        let input = CreatePathInput {
            path: path,
            link_type: link_type,
            target_holon_node_hash: target_holon_hash.0,
        };
        create_path_to_holon_node(input).map_err(|e| HolonError::from(e))
    }

    pub fn get_local_space_holon() -> Result<Holon, HolonError> {
        let path = Path::from("local_holon_space");
        let link_type = LinkTypes::LocalHolonSpace;
        let input = GetPathInput { path: path.clone(), link_type: link_type };
        let record = get_holon_node_by_path(input)
            .map_err(|e| HolonError::from(e))?
            .ok_or_else(|| HolonError::HolonNotFound(format!("at path: {:?}", path)))?;
        Holon::try_from_node(record)
    }
}
