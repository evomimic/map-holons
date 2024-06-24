use hdk::prelude::{info,debug,trace,warn};
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapInteger, MapString};
use crate::boolean_descriptor::define_boolean_type;

use crate::descriptor_types::{CoreSchemaName};

use crate::integer_descriptor::{define_integer_type, IntegerDefinition};
use crate::property_descriptor::define_property_type;
use crate::string_descriptor::{define_string_type, StringDefinition};
use crate::type_descriptor::TypeDefinitionHeader;

/// The load_core_value_types function stages, but does not commit type descriptors
/// for each of the built-in ValueTypes
/// Returns a tuple of HolonReferences to the staged descriptors:
/// (string_type_ref, integer_type_ref, boolean_type_ref)
///
pub fn load_core_value_types(context: &HolonsContext, schema: &HolonReference)
    -> Result<(HolonReference, HolonReference, HolonReference), HolonError> {

    // Define MapString

    let type_name=CoreSchemaName::MapStringType.as_map_string();
    let description = MapString("Built-in MAP String Type".to_string());
    let label = MapString("String".to_string());

    let type_header = TypeDefinitionHeader {
        descriptor_name: None,
        type_name:type_name.clone(),
        description,
        label,
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(true),
        described_by: None,
        is_subtype_of:None,
        owned_by: None, // Holon Space
    };

    let definition = StringDefinition {
        header: type_header,
        min_length: MapInteger(0),
        max_length: MapInteger(4096)
    };

    info!("Preparing to stage descriptor for {:#?}", type_name.clone());
    let string_type_ref = HolonReference::Staged(define_string_type(
        context,
        schema,
        definition,
    )?);

    context.add_references_to_dance_state(vec![string_type_ref.clone()])
        .expect("Unable to add reference to dance_state");

    // Define MapInteger

    let type_name=CoreSchemaName::MapIntegerType.as_map_string();
    let description = MapString("Built-in MAP Integer Type".to_string());
    let label = MapString("Integer".to_string());

    info!("Preparing to stage descriptor for {:#?}", type_name.clone());

    let type_header = TypeDefinitionHeader {
        descriptor_name: None,
        type_name,
        description,
        label,
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(true),
        described_by: None,
        is_subtype_of:None,
        owned_by: None, // Holon Space
    };

    let definition = IntegerDefinition {
        header: type_header,
        min_value: MapInteger(i64::MIN),
        max_value: MapInteger(i64::MAX)
    };
    let integer_type_ref = HolonReference::Staged(define_integer_type(
        context,
        schema,
        definition,
    )?);

    context.add_references_to_dance_state(vec![integer_type_ref])
        .expect("Unable to add reference to dance_state");


    // Define MapBoolean

    let type_name=CoreSchemaName::MapBooleanType.as_map_string();
    let description = MapString("Built-in MAP Boolean Type".to_string());
    let label = MapString("Boolean".to_string());

    info!("Preparing to stage descriptor for {:#?}", type_name.clone());

    let type_header = TypeDefinitionHeader {
        descriptor_name: None,
        type_name,
        description,
        label,
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(true),
        described_by: None,
        is_subtype_of:None,
        owned_by: None, // Holon Space
    };


    let boolean_type_ref = HolonReference::Staged(define_boolean_type(
        context,
        schema,
        type_header,
    )?);

    context.add_references_to_dance_state(vec![boolean_type_ref])
        .expect("Unable to add reference to dance_state");


    // Define SemanticVersionType as a MapString

    let type_name=CoreSchemaName::SemanticVersionType.as_map_string();
    let description = MapString("String Type for representing Semantic Versions of the form (<major>.<minor>.<patch>)".to_string());
    let label = MapString("Semantic Version".to_string());

    let type_header = TypeDefinitionHeader {
        descriptor_name: None,
        type_name:type_name.clone(),
        description,
        label,
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(true),
        described_by: None,
        is_subtype_of:None,
        owned_by: None, // Holon Space
    };

    let definition = StringDefinition {
        header: type_header,
        min_length: MapInteger(5),
        max_length: MapInteger(11)
    };

    info!("Preparing to stage descriptor for {:#?}", type_name.clone());
    let semantic_version_type_ref = HolonReference::Staged(define_string_type(
        context,
        schema,
        definition,
    )?);

    context.add_references_to_dance_state(vec![semantic_version_type_ref.clone()])
        .expect("Unable to add reference to dance_state");

    info!("Staging of L0 ValueTypes is complete... ");


    Ok((string_type_ref, integer_type_ref, boolean_type_ref))

}
