use hdi::prelude::{Deserialize, Serialize};

use shared_types_holon::{MapString, PropertyName, PropertyValue};

use crate::holon::Holon;
use crate::holon_error::HolonError;

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
                "Expected StringValue for '{}'", property_name.0
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
                "Expected StringValue for '{}'", property_name.0
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
        self
            .holon_mut()
            .with_property_value(
                PropertyName(MapString("description".to_string())),
                description.clone().into_base_value(),
            )?;
        Ok(self)
    }
    /// Sets the name property for the HolonSpace (and currently the "key" property)
    ///
    pub fn with_name(&mut self, name: &MapString) -> Result<&mut Self, HolonError> {
        self
            .holon_mut()
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
}

