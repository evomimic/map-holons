use hdi::prelude::info;
use descriptors::string_descriptor::{define_string_type, StringTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapInteger, MapString};
use crate::core_schema_types::SchemaNamesTrait;

#[derive(Debug, Clone)]
pub enum CoreStringValueTypeName {
    MapStringType,
    PropertyNameType,
    RelationshipNameType,
    SemanticVersionType,
}
#[derive(Debug)]
pub struct StringTypeLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub min_length: MapInteger,
    pub max_length: MapInteger,
}

impl SchemaNamesTrait for CoreStringValueTypeName {

    fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = self.get_variant_loader();
        load_string_type_definition(context, schema, loader)

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
        panic!("This trait function is not intended to be used for this type. \
        The 'label' for this type is explicitly defined in get_variant_loader()")
    }


    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        panic!("This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_variant_loader()")
    }
}

impl CoreStringValueTypeName {
    /// This function returns the variant definition for a given variant type
    fn get_variant_loader(&self) -> StringTypeLoader {
        use CoreStringValueTypeName::*;
        match self {
            MapStringType => StringTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Built-in MAP String Type".into()),
                label: MapString("MapString".into()),
                described_by: None,
                owned_by: None,
                min_length: MapInteger(0),
                max_length: MapInteger(32768),
            },
            PropertyNameType => StringTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Names for Holon properties. Should be snake_case".into()),
                label: MapString("PropertyName".into()),
                described_by: None,
                owned_by: None,
                min_length: MapInteger(3),
                max_length: MapInteger(72),
            },
            RelationshipNameType => StringTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Names for relationships between Holons. To align \
            with OpenCypher/GQL standard, should be SCREAMING_UPPER_CASE (all uppercase letters with \
            words separated by underscores).".into()),
                label: MapString("RelationshipName".into()),
                described_by: None,
                owned_by: None,
                min_length: MapInteger(3),
                max_length: MapInteger(72),
            },
            SemanticVersionType => StringTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("String Type for representing Semantic Versions of the form (<major>.<minor>.<patch>)".into()),
                label: MapString("SemanticVersion".into()),
                described_by: None,
                owned_by: None,
                min_length: MapInteger(5),
                max_length: MapInteger(14),
            },
        }

    }
}


/// This function handles the aspects of staging a new enum variant type definition that are common
/// to all enum variant types. It assumes the type-specific parameters have been set by the caller.
pub(crate) fn load_string_type_definition(
    context: &HolonsContext,
    schema: &HolonReference,
    loader: StringTypeLoader,
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

    let definition = StringTypeDefinition {
        header: type_header,
        type_name: loader.type_name.clone(),
        min_length: loader.min_length,
        max_length: loader.max_length,
    };

    info!("Preparing to stage descriptor for {:#?}",
        loader.type_name.clone());
    let staged_ref = define_string_type(
        context,
        schema,
        definition,
    )?;

    context.add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}





