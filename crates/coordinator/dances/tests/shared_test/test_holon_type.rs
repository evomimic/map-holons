use holons::context::HolonsContext;
use holons::holon::{AccessType, EssentialHolonContent};
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use shared_types_holon::{MapString, PropertyMap, PropertyName, PropertyValue};

/// TestHolon is used in data fixtures to hold the data intended to be stored in holons
/// during test execution. Test fixtures don't have access to the persistent store and, therefore,
/// cannot query for descriptors to HolonReferences to include in holon_property_maps. Therefore,
/// TestHolons include name-based maps.
#[derive(Clone, Eq, PartialEq)]
pub struct TestHolon {
    pub property_map: PropertyMap,
    pub key: Option<MapString>,
    // pub relationship_map: RelationshipMap,
    pub descriptor: Option<HolonReference>,
    pub errors: Vec<HolonError>,
}
impl TestHolon {

    pub fn get_property_value(
        &self,
        property_name_str: &str,
    ) -> Option<PropertyValue> {
        self.property_map
            .get(property_name_str.into())
            .cloned()

    }
    pub fn with_property_value(&mut self, property_name_str: &str, value: PropertyValue) -> &mut Self {
        let property_name = PropertyName(MapString(property_name_str.to_string()));
        self.property_map.insert(property_name, value);
        self
    }
    /// This method is used to enable comparisons between Holons and TestHolons. Since the former
    /// use descriptor-keyed maps and the latter name-keyed maps, this method needs to do the
    /// iterate.
    // pub fn essential_content(&mut self,context: &HolonsContext)
    //                          -> Result<EssentialHolonContent, HolonError> {
    //     let key = self.get_key()?;
    //     Ok(EssentialHolonContent {
    //         //property_map: self.property_map.clone(),
    //         property_map: self.holon_property_map.clone(),
    //         //relationship_map: self.relationship_map.clone(),
    //         descriptor: self.descriptor.clone(),
    //         key,
    //         errors: self.errors.clone(),
    //     })
    // }

}