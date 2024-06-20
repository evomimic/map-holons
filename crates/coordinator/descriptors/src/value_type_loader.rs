use hdk::prelude::{info,debug,trace,warn};
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapInteger, MapString};
use crate::boolean_descriptor::define_boolean_type;

use crate::descriptor_types::{MAP_BOOLEAN_TYPE, MAP_INTEGER_TYPE, MAP_STRING_TYPE};

use crate::integer_descriptor::{define_integer_type, IntegerDefinition};
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

    let type_name=MapString(MAP_STRING_TYPE.to_string());
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


    // Define MapInteger

    let type_name=MapString(MAP_INTEGER_TYPE.to_string());
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


    // Define MapBoolean

    let type_name=MapString(MAP_BOOLEAN_TYPE.to_string());
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

    info!("Staging of L0 ValueTypes is complete... ");


    Ok((string_type_ref, integer_type_ref, boolean_type_ref))

}
