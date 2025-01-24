use descriptors::boolean_descriptor::{define_boolean_type, BooleanTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use hdi::prelude::info;

use holons::core_shared_objects::HolonError;
use holons::reference_layer::{HolonReference, HolonsContextBehavior, StagedReference};
use shared_types_holon::{MapBoolean, MapString};
use strum_macros::EnumIter;
// use crate::boolean_value_type_loader;
// use crate::boolean_value_type_loader::CoreBooleanValueTypeName::MapBooleanType;
use crate::core_schema_types::SchemaNamesTrait;

#[derive(Debug, Clone, EnumIter, Default)]
pub enum CoreBooleanValueTypeName {
    #[default]
    MapBooleanType,
}

#[derive(Debug)]
pub struct BooleanTypeLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
}

impl SchemaNamesTrait for CoreBooleanValueTypeName {
    fn load_core_type(
        &self,
        context: &dyn HolonsContextBehavior,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = self.get_boolean_type_loader();
        load_boolean_type_definition(context, schema, loader)
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
        panic!(
            "This trait function is not intended to be used for this type. \
        The 'label' for this type is explicitly defined in get_variant_loader()"
        )
    }

    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        panic!(
            "This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_variant_loader()"
        )
    }
}

impl CoreBooleanValueTypeName {
    /// This function returns the variant definition for a given variant type
    fn get_boolean_type_loader(&self) -> BooleanTypeLoader {
        // use CoreBooleanValueTypeName::*;
        match self {
            CoreBooleanValueTypeName::MapBooleanType => BooleanTypeLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Built-in MAP Integer Type".into()),
                label: MapString("MapInteger".into()),
                described_by: None,
                owned_by: None,
            },
        }
    }
}

/// This function handles the aspects of staging a new enum variant type definition that are common
/// to all enum variant types. It assumes the type-specific parameters have been set by the caller.
fn load_boolean_type_definition(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    loader: BooleanTypeLoader,
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

    let definition =
        BooleanTypeDefinition { header: type_header, type_name: loader.type_name.clone() };

    info!("Preparing to stage descriptor for {:#?}", loader.type_name.clone());
    let staged_ref = define_boolean_type(context, schema, definition)?;

    context
        .add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}
