/// MAP Schema objects maintain a set of MAP Descriptors
/// They support  lazy creation of descriptors by offering "get_the_<type_name>" functions
/// that return the descriptor whose type_name is <xxx>, creating it first, if necessary.
use crate::descriptor_types::Schema;
use holons_core::core_shared_objects::holon::TransientHolon;
use holons_core::core_shared_objects::{holon::Holon, HolonError};

use integrity_core_types::PropertyName;
use base_types::{BaseValue, MapString};

impl Schema {
    /// creates an empty (in-memory) Schema Holon
    pub fn new(name: MapString, description: MapString) -> Result<Schema, HolonError> {
        let mut schema_holon = TransientHolon::new();
        let key_property_name: MapString = MapString("key".to_string());
        let name_property_name: MapString = MapString("name".to_string());
        let description_property_name: MapString = MapString("description".to_string());

        schema_holon
            .with_property_value(
                PropertyName(key_property_name),
                Some(BaseValue::StringValue(name.clone())),
            )?
            .with_property_value(
                PropertyName(name_property_name),
                Some(BaseValue::StringValue(name.clone())),
            )?
            .with_property_value(
                PropertyName(description_property_name),
                Some(BaseValue::StringValue(description)),
            )?;

        Ok(Schema(Holon::Transient(schema_holon)))
    }
    /// Downcasts a Schema to a Holon
    pub fn into_holon(self) -> Holon {
        self.0.clone()
    }

    // /// Adds a TypeDescriptor to the Schema
    // pub fn add_descriptor(
    //     &mut self,
    //     descriptor: &TypeDescriptor,
    // )-> &mut Self {
    //     let descriptor_target = define_local_target(&descriptor.0);
    //     self.into_holon().add_related_holon(MapString("COMPONENTS".to_string()), descriptor_target);
    //
    //     &self
    //
    // }

    // /// Returns the requested (singleton) MetaTypeDescriptor instance for this Schema,
    // /// Or return an error if it is not defined.
    // pub fn get_meta_type_descriptor(&self, descriptor_name: MapString) ->Result<TypeDescriptor,HolonError> {
    //
    //     if let Some(meta_descriptor) = self.into_holon().relationship_map.get(descriptor_name) {
    //         Ok(meta_descriptor.clone())
    //     } else {
    //         let schema_target = define_local_target(self.clone().into_holon());
    //         let meta_descriptor = define_type_descriptor(
    //             self,
    //             MapString(TYPE_METADESCRIPTOR),
    //             TypeKind::Holon,
    //             "Metadescriptor for the TypeDescriptor".to_string(),
    //             "Type Metadescriptor".to_string(),
    //             false,
    //             false,
    //         );
    //
    //         Ok(meta_descriptor)
    //     }
    //
    // }
}
