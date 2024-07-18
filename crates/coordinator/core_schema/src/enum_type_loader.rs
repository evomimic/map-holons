use hdi::prelude::info;
use inflector::cases::snakecase::to_snake_case;
use inflector::cases::titlecase::to_title_case;
use descriptors::enum_descriptor::{define_enum_type, EnumTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapString};
use crate::core_schema_types::SchemaNamesTrait;

use crate::enum_type_loader::CoreEnumTypeName::{DeletionSemanticType, MapBaseType};
use crate::enum_variant_loader::CoreEnumVariantTypeName;
use crate::enum_variant_loader::CoreEnumVariantTypeName::*;

#[derive(Debug, Clone)]
pub enum CoreEnumTypeName {
    MapBaseType, // Enum -- BaseTypeEnumType
    DeletionSemanticType, // Enum -- DeletionSemanticEnumType
}
pub struct EnumTypeLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub variants: Vec<CoreEnumVariantTypeName>,
}
impl SchemaNamesTrait for CoreEnumTypeName {

    fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = EnumTypeLoader {
            type_name: self.derive_type_name(),
            descriptor_name: self.derive_descriptor_name(),
            description: self.derive_description(),
            label: self.derive_label(),
            described_by: None, // TODO: Lazy get MetaPropertyDescriptor
            owned_by: None,
            variants: self.specify_variants(),
        };
        load_enum_type_definition(context, schema, loader)

    }
    /// This method returns the unique type_name for this property type in "snake_case"
    fn derive_type_name(&self) -> MapString {
        // this implementation assumes #Debug representation of the VariantNames within this enum
        MapString(to_snake_case(&format!("{:?}", self)))
    }

    /// This method returns the "descriptor_name" for this type in snake_case
    fn derive_descriptor_name(&self) -> MapString {
        // this implementation uses a simple naming rule of appending "_descriptor" to the type_name
        MapString(format!("{}Descriptor", self.derive_type_name().0.clone()))
    }
    /// This method returns the human-readable name for this property type
    fn derive_label(&self) -> MapString {
        // this implementation uses a simple naming rule simply converting the type name to
        // "Title Case" -- i.e., separating the type_name into (mostly) capitalized words.
        MapString(to_title_case(&format!("{:?}", self)))
    }


    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        // use CoreEnumTypeName::*;
        // use crate::enum_type_loader::CoreEnumTypeName::{DeletionSemanticType, MapBaseType};
        match self {
            MapBaseType => MapString("Specifies the MAP BaseType of this object. ".to_string()),
            DeletionSemanticType => MapString("Offers different options handling requests to delete a \
            source Holon of  relationship.".to_string()),

        }
    }
}

impl CoreEnumTypeName {
    /// This function returns the list of type names for the variants defined for this enum type
    fn specify_variants(&self) -> Vec<CoreEnumVariantTypeName> {
        // use CoreEnumTypeName::*;
        // use crate::enum_type_loader::CoreEnumTypeName::{DeletionSemanticType, MapBaseType};
        match self {
            MapBaseType => {
                vec![
                    BaseTypeHolon,
                    BaseTypeCollection,
                    BaseTypeProperty,
                    BaseTypeRelationship,
                    BaseTypeEnumVariant,
                    BaseTypeValueBoolean,
                    BaseTypeValueEnum,
                    BaseTypeValueInteger,
                    BaseTypeValueString,
                    BaseTypeValueBooleanArray,
                    BaseTypeValueEnumArray,
                    BaseTypeValueIntegerArray,
                    BaseTypeValueStringArray,
                ]
            }

            ,
            DeletionSemanticType => {
                vec![
                    DeletionSemanticAllow,
                    DeletionSemanticBlock,
                    DeletionSemanticCascade,
                ]

            },
        }
    }
}


/// This function handles the aspects of staging a new enum type definition that are common
/// to all enum types. It assumes the type-specific parameters have been set by the caller.
fn load_enum_type_definition(
    context: &HolonsContext,
    schema: &HolonReference,
    loader: EnumTypeLoader,
) -> Result<StagedReference, HolonError> {
    let type_header = TypeDescriptorDefinition {
        descriptor_name: loader.descriptor_name,
        description: loader.description,
        label: loader.label,
        // TODO: add base_type: BaseType::Enum,
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(true),
        described_by: loader.described_by,
        is_subtype_of: None,
        owned_by: loader.owned_by,
    };
    let mut definition = EnumTypeDefinition {
        header: type_header,
        type_name: loader.type_name.clone(),
        variants: vec![],
    };

    // Add HolonReferences to the variants for this enum type
    for variant in loader.variants {
        definition.variants.push(variant.lazy_get_core_type_definition(
                context,
                schema
            )?);
    }

    info!("Preparing to stage descriptor for {:#?}",
        loader.type_name.clone());
    let staged_ref = define_enum_type(
        context,
        schema,
        definition,
    )?;


    context.add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}





