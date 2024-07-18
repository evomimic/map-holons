use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::MapString;

use crate::core_schema_types::SchemaNamesTrait;
use crate::holon_type_loader::{HolonTypeLoader, load_holon_type_definition};
use crate::property_type_loader::CorePropertyTypeName;

#[derive(Debug, Clone)]
pub enum CoreMetaTypeName {
     MetaType,
     MetaHolonType,
     MetaRelationshipType,
     MetaHolonCollectionType,
     MetaPropertyType,
     //MetaDanceType,
     // MetaValueType,
     MetaBooleanType,
     MetaEnumType,
     MetaEnumVariantType,
     MetaIntegerType,
     MetaStringType,
     MetaValueArrayType,
}

/// The load_core_type function stages, but does not commit, a holon type descriptor
/// for the type identified by its CoreMetaTypeName.
/// Returns a StagedReference to the newly staged MetaTypeDefinition
///
impl SchemaNamesTrait for CoreMetaTypeName {
    fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = self.get_holon_type_loader();
        load_holon_type_definition(context, schema, loader)

    }
    /// This method returns the unique type_name for this property type in "snake_case"
    fn derive_type_name(&self) -> MapString {
        // Assume VariantNames have been defined in the proper format (CamelCase)
        MapString(format!("{:?}", self))
    }

    /// This method returns the "descriptor_name" for this type in camel_case
    fn derive_descriptor_name(&self) -> MapString {
        // this implementation uses a simple naming rule of appending "_descriptor" to the type_name
        MapString(format!("{}Descriptor", self.derive_type_name().0.clone()))
    }
    /// This method returns the human-readable name for this property type
    fn derive_label(&self) -> MapString {
        self.derive_type_name()
    }


    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        panic!("This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_holon_type_loader()")
    }
}
impl CoreMetaTypeName {
    /// This function returns a HolonType Loader containing the properties and references that
    /// comprise the definition of the `self` meta-type.
    fn get_holon_type_loader(&self) -> HolonTypeLoader {
        use CoreMetaTypeName::*;
        use CorePropertyTypeName::*;
        match self {
            MetaType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Defines the properties, relationships and dances \
                for the TypeDescriptor that all Map Type Definitions share.".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    DescriptorName,
                    Label,
                    BaseType,
                    Description,
                    IsDependent,
                    IsBuiltinType,
                    IsValueType,
                    Version,
                ],
                key_properties: Some(vec![
                    DescriptorName,
                ]),
                // source_for: vec![],
            },
            MetaHolonType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a Holon Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                ],
                key_properties: Some(vec![
                    TypeName,
                ]),
                // source_for: vec![],
            },

            MetaRelationshipType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a Relationship Type as a child of its Source\
                Holon".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    RelationshipName,
                    DeletionSemantic,
                ],
                key_properties: Some(vec![
                    RelationshipName,
                ]),
                // source_for: vec![],
            },

            MetaHolonCollectionType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a Holon Collection Type as a child of \
                a Relationship.".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                    IsOrdered,
                    AllowDuplicates,
                    MinCardinality,
                    MaxCardinality,
                ],
                key_properties: Some(vec![
                    TypeName,
                ]),
                // source_for: vec![],
            },

            MetaPropertyType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a Property Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    PropertyTypeName,
                ],
                key_properties: Some(vec![
                    PropertyTypeName,
                ]),
                // source_for: vec![],
            },
            // MetaDanceType => HolonTypeLoader {
            //     type_name: self.derive_type_name(),
            //     descriptor_name: self.derive_descriptor_name(),
            //     description: MapString("Describes a Dance Type as a child of its Holon Type".into()),
            //     label: self.derive_label(),
            //     described_by: None,
            //     owned_by: None,
            //     properties: vec![
            //         DanceName,
            //     ],
            //     key_properties: Some(vec![
            //         PropertyTypeName::Name,
            //     ]),
            //     // source_for: vec![],
            // },
            // MetaValueType => HolonTypeLoader {
            //     type_name: self.derive_type_name(),
            //     descriptor_name: self.derive_descriptor_name(),
            //     description: MapString("Describes a Value Type".into()),
            //     label: self.derive_label(),
            //     described_by: None,
            //     owned_by: None,
            //     properties: vec![
            //         PropertyTypeName::Name,
            //         PropertyTypeName::Description,
            //     ],
            //     key_properties: Some(vec![
            //         PropertyTypeName::Name,
            //     ]),
            //     // source_for: vec![],
            // },
            MetaBooleanType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a Boolean Value Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                ],
                key_properties: Some(vec![
                    TypeName,
                ]),
                // source_for: vec![],
            },
            MetaEnumType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes an Enum Value Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                ],
                key_properties: Some(vec![
                    TypeName,
                ]),
                // source_for: vec![],
            },
            MetaEnumVariantType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a Variant for an Enum Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    VariantName,
                ],
                key_properties: Some(vec![
                    VariantName,
                ]),
                // source_for: vec![],
            },
            MetaIntegerType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes an Integer Value Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                    MinValue,
                    MaxValue,
                ],
                key_properties: Some(vec![
                    TypeName
                ]),
                // source_for: vec![],
            },
            MetaStringType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a String Value Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                    MinLength,
                    MaxLength,
                ],
                key_properties: Some(vec![
                    TypeName,
                ]),
                // source_for: vec![],
            },
            MetaValueArrayType => HolonTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes an Array of Values Type".into()),
                label: self.derive_label(),
                described_by: None,
                owned_by: None,
                properties: vec![
                    TypeName,
                    MinCardinality,
                    MaxCardinality
                ],
                key_properties: Some(vec![
                    TypeName,
                ]),
                // source_for: vec![],
            },
        }
    }
}

// /// This function handles the aspects of staging a new holon type definition that are common
// /// to all holon types. It assumes the type-specific parameters have been set by the caller.
// fn load_meta_type_definition(
//     context: &HolonsContext,
//     schema: &HolonReference,
//     loader: HolonTypeLoader,
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


