use hdi::prelude::info;
use strum_macros::EnumIter;
// use descriptors::descriptor_types::CoreSchemaRelationshipTypeName::TargetCollectionType;
use crate::core_schema_types::SchemaNamesTrait;
use descriptors::holon_descriptor::{define_holon_type, HolonTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use holons::reference_layer::{HolonReference, HolonsContextBehavior, StagedReference};
use holons::shared_objects_layer::HolonError;
use shared_types_holon::{MapBoolean, MapString};
// use crate::holon_type_loader::CoreHolonTypeName::{DanceRequestType, DanceResponseType, HolonSpaceType, HolonType, PropertyType, RelationshipType, SchemaType};
use crate::property_type_loader::CorePropertyTypeName;
use crate::property_type_loader::CorePropertyTypeName::{
    Description, DescriptorName, Name, TypeName,
};
use crate::relationship_type_loader::CoreRelationshipTypeName;

#[derive(Debug, Clone, Default, EnumIter)]
pub enum CoreHolonTypeName {
    DanceRequestType,
    DanceResponseType,
    HolonCollectionType,
    HolonSpaceType,
    #[default]
    HolonType,
    MetaType,
    PropertyType,
    RelationshipType,
    SchemaType,
    TypeDescriptor,
    ValueType,
}
#[derive(Debug)]
pub struct HolonTypeLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub properties: Vec<CorePropertyTypeName>, // PropertyDescriptors
    pub key_properties: Option<Vec<CorePropertyTypeName>>, // PropertyDescriptors
    pub source_for: Vec<CoreRelationshipTypeName>, // RelationshipDescriptors
}

impl SchemaNamesTrait for CoreHolonTypeName {
    fn load_core_type(
        &self,
        context: &dyn HolonsContextBehavior,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
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
        panic!(
            "This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_variant_loader()"
        )
    }
}
impl CoreHolonTypeName {
    /// This function returns the holon definition for a given holon type
    fn get_holon_type_loader(&self) -> HolonTypeLoader {
        let type_name = self.derive_type_name();
        let descriptor_name = self.derive_descriptor_name();
        let label = self.derive_label();

        use CoreHolonTypeName::*;
        match self {
            DanceRequestType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in DanceRequest type. This type \
                specifies values all dance request types."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![],
            },

            DanceResponseType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in TypeDescriptor type. This type \
                specifies values for the common characteristics shared by all types."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![],
            },
            HolonCollectionType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in type that serves as the common \
                supertype of all HolonCollectionTypes."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![TypeName],
                key_properties: Some(vec![TypeName]),
                source_for: vec![],
            },

            HolonSpaceType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the purpose and noteworthy aspects of this \
                HolonSpace"
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![Name, Description],
                key_properties: Some(vec![Name]),
                source_for: vec![],
            },

            HolonType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString("Describes the built-in HolonType".into()),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![TypeName],
                key_properties: Some(vec![TypeName]),
                source_for: vec![],
            },

            PropertyType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in PropertyType type that serves as a \
                shared supertype of all built-in PropertyTypes."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![],
            },

            RelationshipType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in TypeDescriptor type. This type \
                specifies values for the common characteristics shared by all types."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![],
            },

            SchemaType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the scope, purpose and noteworthy aspects \
                of this Schema."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![Name, Description],
                key_properties: Some(vec![Name]),
                source_for: vec![],
            },

            TypeDescriptor => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in TypeDescriptor type. This type \
                specifies values for the common characteristics shared by all types."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![],
            },

            ValueType => HolonTypeLoader {
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in TypeDescriptor type. This type \
                specifies values for the common characteristics shared by all types."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![],
            },

            MetaType => HolonTypeLoader {
                // specifies the properties & relationships shared by all TypesDescriptors
                // (COMPONENT_OF, IS_A) + properties
                type_name,
                descriptor_name,
                description: MapString(
                    "Describes the built-in TypeDescriptor type. This type \
                specifies values for the common characteristics shared by all types."
                        .into(),
                ),
                label,
                described_by: None,
                owned_by: None,
                properties: vec![DescriptorName],
                key_properties: Some(vec![DescriptorName]),
                source_for: vec![CoreRelationshipTypeName::ComponentOf],
            },
        }
    }
}

/// This function handles the aspects of staging a new holon type definition that are common
/// to all holon types. It assumes the type-specific parameters have been set by the caller.
pub fn load_holon_type_definition(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    loader: HolonTypeLoader,
) -> Result<StagedReference, HolonError> {
    let type_header = TypeDescriptorDefinition {
        descriptor_name: loader.descriptor_name,
        description: loader.description,
        label: loader.label,
        // TODO: add base_type: BaseType::EnumVariant
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(false),
        described_by: loader.described_by,
        is_subtype_of: None,
        owned_by: loader.owned_by,
    };

    let mut definition = HolonTypeDefinition {
        header: type_header,
        type_name: loader.type_name.clone(),
        properties: vec![],
        key_properties: None,
    };
    // Add HolonReferences to the PropertyDescriptors for this holon type
    for property in loader.properties {
        definition.properties.push(property.lazy_get_core_type_definition(context, schema)?);
    }

    // Add HolonReferences to the Key PropertyDescriptors for this holon type
    if let Some(key_properties) = loader.key_properties {
        // Initialize key_properties as an empty vector if it's None
        definition.key_properties = Some(vec![]);

        for key_property in key_properties {
            // Safely unwrap definition.key_properties and push into the inner vector
            if let Some(ref mut key_props) = definition.key_properties {
                key_props.push(key_property.lazy_get_core_type_definition(context, schema)?);
            }
        }
    }

    // TODO:  Lazy get source_for references to RelationshipDescriptors
    // TODO: Lazy get dance_request references to DanceDescriptors (Request & Response)

    info!("Preparing to stage descriptor for {:#?}", loader.type_name.clone());
    let staged_ref = define_holon_type(context, schema, definition)?;

    context
        .add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}
