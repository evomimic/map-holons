
use holons::helpers::define_local_target;
use holons::holon_errors::HolonError;
/// MAP Schema objects maintain a set of MAP Descriptors
/// They support  lazy creation of descriptors by offering "get_the_<type_name>" functions
/// that return the descriptor whose type_name is <xxx>, creating it first, if necessary.


use holons::holon_types::{Holon};
use holons::relationship::RelationshipTarget;

use shared_types_holon::holon_node::{PropertyName};
use shared_types_holon::value_types::{BaseValue, MapString};
use crate::descriptor_types::{Schema, TYPE_META_DESCRIPTOR, TypeDescriptor};
use crate::type_descriptor::define_type_descriptor;

impl Schema {
    /// creates an empty (in-memory) Schema Holon
    pub fn new(name: String, description: String) -> Schema {
        let mut schema_holon = Holon::new();
        let name_property_name: MapString = "name".to_string();
        let description_property_name: MapString = "description".to_string();

        schema_holon.with_property_value(name_property_name, BaseValue::StringValue(name))
            .with_property_value(description_property_name, BaseValue::StringValue(description));

        Schema(schema_holon)

    }
    /// Downcasts a Schema to a Holon
    pub fn into_holon(self) -> Holon {
        self.0
    }
    // /// Adds a TypeDescriptor to the Schema
    // pub fn add_descriptor(
    //     &mut self,
    //     descriptor: &TypeDescriptor,
    // )-> &mut Self {
    //     let descriptor_target = define_local_target(&descriptor.0);
    //     self.into_holon().add_related_holon("COMPONENTS".to_string(), Some(descriptor_target));
    //
    //     &mut self
    //
    // }

    // /// Returns the singleton MetaTypeDescriptor instance for this Schema,
    // /// Defining it first, if necessary
    // pub fn get_meta_type_descriptor(&self) ->Result<TypeDescriptor,HolonError> {
    //    // if let Some(meta_descriptor) = self.into_holon().relationship_map.get(TYPE_METADESCRIPTOR) {
    //     if let Some(meta_descriptor) = self.into_holon().relationship_map.get("TypeMetadescriptor") {
    //         Ok(meta_descriptor.clone())
    //     } else {
    //         let schema_target = define_local_target(self.clone().into_holon());
    //         let meta_descriptor = define_type_descriptor(
    //             &self,
    //             TYPE_METADESCRIPTOR,
    //             BaseType::Holon,
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








