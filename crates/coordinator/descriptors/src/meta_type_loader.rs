use hdk::prelude::{info,debug,trace,warn};
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapInteger, MapString};
use crate::boolean_descriptor::define_boolean_type;

use crate::descriptor_types::{CoreMetaSchemaName};
use crate::holon_descriptor::{define_holon_type, HolonDefinition};

use crate::integer_descriptor::{define_integer_type, IntegerDefinition};
use crate::string_descriptor::{define_string_type, StringDefinition};
use crate::type_descriptor::TypeDefinitionHeader;

/// The load_meta_types function stages, but does not commit type descriptors
/// for each of the built-in meta types. References to the meta types are stored in dance_state
/// Returns a HolonReference to the Schema containing the newly staged types
///
//  MetaSchemaType,
//  MetaType,
//  MetaHolonType,
//  MetaRelationshipType,
//  MetaPropertyType,
//  MetaDanceType,
//  MetaValueType,
//  MetaBooleanType,
//  MetaEnumType,
//  MetaEnumVariantType,
//  MetaIntegerType,
//  MetaStringType,
//  MetaValueArrayType
///
pub fn load_core_meta_types(context: &HolonsContext, schema: &HolonReference)
    -> Result<(),HolonError> {

    let type_name=CoreMetaSchemaName::MetaHolonType.as_map_string();
    let description = MapString("The meta type that specifies the properties, relationships, \
    and dances of the base HolonType".to_string());
    let label = MapString("Holon Type Descriptor".to_string());

    let type_header = TypeDefinitionHeader {
        descriptor_name: None,
        type_name,
        description,
        label,
        is_dependent: MapBoolean(false),
        is_value_type: MapBoolean(false),
        described_by: None,
        is_subtype_of:None,
        owned_by: None, // Holon Space
    };

    let holon_definition = HolonDefinition {
        header: type_header,
        properties:  vec![],
    };

    let meta_holon_type_ref = define_holon_type(
        context,
        schema,
        holon_definition, // provide property descriptors for this holon type here
    )?;

    // add to DESCRIPTOR_RELATIONSHIPS the relationships that all HolonTypes must populate
    // add to DESCRIPTOR_PROPERTIES the properties that all HolonTypes must populate keys ValueType?



    context.add_references_to_dance_state(vec![HolonReference::Staged(meta_holon_type_ref.clone())])?;






    info!("Staging of Core Meta Types is complete... ");


    Ok(())

}
