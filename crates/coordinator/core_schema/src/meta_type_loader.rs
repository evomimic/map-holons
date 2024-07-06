// use hdk::prelude::{info,debug,trace,warn};
// use descriptors::descriptor_types::CoreSchemaPropertyTypeName::PropertyTypeName;
// use descriptors::holon_descriptor::{define_holon_type, HolonTypeDefinition};
// use descriptors::type_descriptor::TypeDescriptorDefinition;
// use holons::commit_manager::CommitManager;
// use holons::context::HolonsContext;
// use holons::holon_error::HolonError;
// use holons::holon::Holon;
use holons::holon_reference::HolonReference;

// use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapInteger, MapString};

// use crate::core_schema_types::SchemaNamesTrait;


pub enum CoreMetaTypeName {
     MetaHolonType,
     MetaRelationshipType,
     MetaPropertyType,
     MetaDanceType,
     MetaValueType,
     MetaBooleanType,
     MetaEnumType,
     MetaEnumVariantType,
     MetaIntegerType,
     MetaStringType,
     MetaValueArrayType,
}
struct MetaTypeLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub properties: Vec<HolonReference>, // PropertyDescriptors
    pub key_properties: Option<Vec<HolonReference>>, // PropertyDescriptors
    // pub source_for: Vec<HolonReference>, // RelationshipDescriptors
}
// /// The load_meta_types function stages, but does not commit, type descriptors
// /// for each of the built-in meta types. References to the meta types are stored in dance_state
// /// Returns a HolonReference to the Schema containing the newly staged types
// ///
// ///
// impl SchemaNamesTrait for CoreMetaTypeName {
//     fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
//         // Set the type specific variables for this type, then call the load_property_definition
//         let loader = self.get_holon_type_loader();
//         load_meta_type_definition(context, schema, loader)
//
//     }
//     /// This method returns the unique type_name for this property type in "snake_case"
//     fn derive_type_name(&self) -> MapString {
//         // Assume VariantNames have been defined in the proper format (CamelCase)
//         MapString(format!("{:?}", self))
//     }
//
//     /// This method returns the "descriptor_name" for this type in camel_case
//     fn derive_descriptor_name(&self) -> MapString {
//         // this implementation uses a simple naming rule of appending "_descriptor" to the type_name
//         MapString(format!("{}Descriptor", self.derive_type_name().0.clone()))
//     }
//     /// This method returns the human-readable name for this property type
//     fn derive_label(&self) -> MapString {
//         self.derive_type_name()
//     }
//
//
//     /// This method returns the human-readable description of this type
//     fn derive_description(&self) -> MapString {
//         panic!("This trait function is not intended to be used for this type. \
//         The 'description' for this type is explicitly defined in get_variant_loader()")
//     }
// }
// impl CoreMetaTypeName {
//     /// This function returns the variant definition for a given variant type
//     fn get_meta_type_loader(&self) -> MetaTypeLoader {
//         use CoreMetaTypeName::*;
//         match self {
//             MetaHolonType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//
//             MetaRelationshipType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaPropertyType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaDanceType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaValueType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaBooleanType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaEnumType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaEnumVariantType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaIntegerType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaStringType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//             MetaValueArrayType => MetaTypeLoader {
//                 type_name: self.derive_type_name(),
//                 descriptor_name: self.derive_descriptor_name(),
//                 description: MapString("Describes Holon Types".into()),
//                 label: self.derive_label(),
//                 described_by: None,
//                 owned_by: None,
//                 properties: vec![
//                     PropertyTypeName::Name,
//                     PropertyTypeName::Description,
//                 ],
//                 key_properties: Some(vec![
//                     PropertyTypeName::Name,
//                 ]),
//                 // source_for: vec![],
//             },
//         }
//     }
// }
//
// /// This function handles the aspects of staging a new holon type definition that are common
// /// to all holon types. It assumes the type-specific parameters have been set by the caller.
// fn load_meta_type_definition(
//     context: &HolonsContext,
//     schema: &HolonReference,
//     loader: MetaTypeLoader,
// ) -> Result<StagedReference, HolonError> {
//     let type_header = TypeDescriptorDefinition {
//         descriptor_name: loader.descriptor_name,
//         description: loader.description,
//         label: loader.label,
//         // TODO: add base_type: BaseType::EnumVariant
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(false),
//         described_by: loader.described_by,
//         is_subtype_of: None,
//         owned_by: loader.owned_by,
//     };
//
//     let mut definition = HolonTypeDefinition {
//         header: type_header,
//         type_name: loader.type_name,
//         properties: vec![],
//         key_properties: None,
//
//     };
//     // Add HolonReferences to the PropertyDescriptors for this holon type
//     for property in loader.properties {
//         definition.properties.push(property.lazy_get_core_type_definition(
//             context,
//             schema
//         )?);
//     }
//
//     // Add HolonReferences to the Key PropertyDescriptors for this holon type
//     if let Some(key_properties) = loader.key_properties {
//         definition.key_properties = Some(vec![]);
//         for key_property in key_properties {
//             definition.key_properties.push(key_property.lazy_get_core_type_definition(
//                 context,
//                 schema
//             )?);
//         }
//
//     }
//
//     // TODO:  Lazy get source_for references to RelationshipDescriptors
//     // TODO: Lazy get dance_request references to DanceDescriptors (Request & Response)
//
//     info!("Preparing to stage descriptor for {:#?}",
//         loader.type_name.clone());
//     let staged_ref = define_holon_type(
//         context,
//         schema,
//         definition,
//     )?;
//
//     context.add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
//         .expect("Unable to add reference to dance_state");
//
//     Ok(staged_ref)
// }

// pub fn load_core_meta_types(context: &HolonsContext, schema: &HolonReference)
//     -> Result<(),HolonError> {
//
//     // Stage MetaHolonType
//
//     let type_name=CoreMetaSchemaName::MetaHolonType.as_type_name();
//     let description = MapString("The meta type that specifies the properties, relationships, \
//     and dances shared by all HolonTypes".to_string());
//     let label = MapString("Holon Type Descriptor".to_string());
//
//     let type_header = TypeDescriptorDefinition {
//         descriptor_name: None,
//         type_name,
//         description,
//         label,
//         is_dependent: MapBoolean(false),
//         is_value_type: MapBoolean(false),
//         described_by: None,
//         is_subtype_of:None,
//         owned_by: None, // Holon Space
//     };
//
//     let holon_definition = HolonTypeDefinition {
//         header: type_header,
//         properties:  vec![],
//         //source_for: vec![],
//     };
//
//     let meta_meta_type_ref = define_holon_type(
//         context,
//         schema,
//         holon_definition, // provide property descriptors for this holon type here
//     )?;
//
//     // add to DESCRIPTOR_RELATIONSHIPS the relationships that all HolonTypes must populate
//     // add to DESCRIPTOR_PROPERTIES the properties that all HolonTypes must populate keys ValueType?
//
//     context.add_references_to_dance_state(vec![HolonReference::Staged(meta_holon_type_ref.clone())])?;
//
// // Stage MetaRelationshipType
//
//     let type_name=CoreMetaSchemaName::MetaRelationshipType.as_type_name();
//     let description = MapString("The meta type that specifies the properties, relationships, \
//     and dances that all RelationshipDescriptors have ".to_string());
//     let label = MapString("Relationship Type Descriptor".to_string());
//
//     let type_header = TypeDescriptorDefinition {
//         descriptor_name: None,
//         type_name,
//         description,
//         label,
//         is_dependent: MapBoolean(false),
//         is_value_type: MapBoolean(false),
//         described_by: None,
//         is_subtype_of:None,
//         owned_by: None, // Holon Space
//     };
//
//     let holon_definition = HolonTypeDefinition {
//         header: type_header,
//         properties:  vec![],
//         //source_for: vec![],
//     };
//
//     let meta_relationship_type_ref = define_holon_type(
//         context,
//         schema,
//         holon_definition, // provide property descriptors for this holon type here
//     )?;
//
//     // add to DESCRIPTOR_RELATIONSHIPS the relationships that all HolonTypes must populate
//     // add to DESCRIPTOR_PROPERTIES the properties that all HolonTypes must populate keys ValueType?
//
//     context.add_references_to_dance_state(vec![HolonReference::Staged(meta_relationship_type_ref)])?;
//
//
//
//
//     info!("Staging of Core Meta Types is complete... ");
//
//
//     Ok(())
//
// }
