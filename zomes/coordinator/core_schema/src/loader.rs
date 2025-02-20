use hdk::prelude::*;

use holons_core::core_shared_objects::{CommitResponse, HolonError};
use holons_core::{stage_new_holon_api, HolonReference, HolonsContextBehavior, StagedReference};

use crate::boolean_value_type_loader::CoreBooleanValueTypeName;
use crate::core_schema_types::{CoreSchemaTypeName, SchemaNamesTrait};
use crate::enum_type_loader::CoreEnumTypeName;
use crate::integer_value_type_loader::CoreIntegerValueTypeName;
use crate::meta_type_loader::CoreMetaTypeName;
use crate::property_type_loader::CorePropertyTypeName;
use crate::relationship_type_loader::CoreRelationshipTypeName;
use crate::string_value_type_loader::CoreStringValueTypeName;
use crate::value_type_loader::CoreValueTypeName;
use descriptors::descriptor_types::{CoreSchemaName, Schema};
use holons_core::core_shared_objects::stage_new_holon_api;
use holons_core::core_shared_objects::{CommitResponse, HolonError};
use shared_types_holon::MapString;
use strum::IntoEnumIterator;

/// The load_core_schema function creates a new Schema Holon and populates it descriptors for all the
/// MAP L0 Schema Descriptors defined in `CoreSchemaNames`
///
/// It uses the transient collection in context's dance_state to support lookup of previously
/// created schema components, so they may be referenced in relationship definition

///
/// The full implementation of this function will emerge incrementally... starting with a minimal schema
///

pub fn load_core_schema(context: &dyn HolonsContextBehavior) -> Result<CommitResponse, HolonError> {
    info!("vvvvvvvv Entered: load_core_schema vvvvvvvvv");
    // Begin by staging `schema`. It's HolonReference becomes the target of
    // the COMPONENT_OF relationship for all schema components
    let _ = context
        .get_space_manager()
        .get_space_holon()
        .ok_or(HolonError::HolonNotFound("Local holon space not found".to_string()));

    let schema = Schema::new(
        CoreSchemaName::SchemaName.as_map_string(),
        MapString(
            "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string(),
        ),
    )?;

    info!("Staging Schema...");
    let staged_schema_ref = HolonReference::Staged(stage_new_holon_api(context, schema.0.clone())?);

    context
        .get_space_manager()
        .get_transient_state()
        .borrow_mut()
        .add_references(context, vec![staged_schema_ref.clone()])?;

    let initial_load_set = get_initial_load_set();

    for type_name in initial_load_set {
        info!("Attempting to load {:?}", type_name);
        let _type_ref = type_name.lazy_get_core_type_definition(context, &staged_schema_ref)?;
    }
    // Let's add all the CoreRelationshipTypes to the initial load set

    for variant in CoreRelationshipTypeName::iter() {
        info!("Attempting to load {:?}", variant);
        let _type_ref = variant.lazy_get_core_type_definition(context, &staged_schema_ref)?;
    }

    // Let's add all the CorePropertyTypes to the initial load set

    for variant in CorePropertyTypeName::iter() {
        info!("Attempting to load {:?}", variant);
        let _type_ref = variant.lazy_get_core_type_definition(context, &staged_schema_ref)?;
    }

    info!("^^^^^^^ STAGING COMPLETE: Committing schema...");

    let holon_service = context.get_space_manager().get_holon_service();

    let response = holon_service.commit(context)?;

    let r = response.clone();

    info!("Commit Response: {:#?}", r.status);
    info!("Commits Attempted: {:#?}", r.commits_attempted.0.to_string());
    info!("Holons Saved: {:#?}", r.saved_holons.len());
    info!("Abandoned: {:#?}", r.abandoned_holons.len());

    info!("DATABASE DUMP (max 300 records)");

    // TODO: Restore this functionality once support for get_all_holons is provided
    // let holons = Holon::get_all_holons()?;
    // // for holon in holons.iter().take(30) {
    // //     info!("Holon:\n{}",as_json(holon));
    // // }
    //
    // for holon in holons.iter().take(300) {
    //     let key_result = holon.get_key();
    //     let property_name = PropertyName(MapString("base_type".to_string()));
    //     let base_type = holon.get_property_value(&property_name);
    //     match key_result {
    //         Ok(key) => {
    //             info!(
    //                 "key = {:?}, base_type= {:?}",
    //                 key.unwrap_or_else(|| MapString("<None>".to_string())).0,
    //                 base_type,
    //             );
    //             debug!("Holon {}", as_json(&holon));
    //         }
    //         Err(holon_error) => {
    //             panic!("Attempt to get_key() resulted in error {:?}", holon_error,);
    //         }
    //     }
    // }

    Ok(response)
}

fn get_initial_load_set() -> Vec<CoreSchemaTypeName> {
    let mut result: Vec<CoreSchemaTypeName> = vec![

        // CoreSchemaTypeName::HolonType(HolonType),

        // ValueType(StringType(MapStringType)),
        // ValueType(StringType(PropertyNameType)),
        // ValueType(StringType(RelationshipNameType)),
        // ValueType(StringType(SemanticVersionType)),
        // ValueType(IntegerType(MapIntegerType)),
        // ValueType(BooleanType(MapBooleanType)),
        // ValueType(EnumType(MapBaseType)),
        // ValueType(EnumType(DeletionSemanticType)),
        // HolonType(HolonSpaceType),
        // HolonType(SchemaType),
        // MetaType(CoreMetaTypeName::MetaType),
        // MetaType(MetaHolonType),
        // MetaType(MetaRelationshipType),
        // MetaType(MetaHolonCollectionType),
        // MetaType(MetaPropertyType),
        // //MetaDanceType,
        // //MetaValueType,
        // MetaType(MetaBooleanType),
        // MetaType(MetaEnumType),
        // MetaType(MetaEnumVariantType),
        // MetaType(MetaIntegerType),
        // MetaType(MetaStringType),
        // MetaType(MetaValueArrayType),

    ];

    // Let's add all the CoreSchemaValueTypes to the initial load set

    for variant in CoreStringValueTypeName::iter() {
        result.push(CoreSchemaTypeName::ValueType(CoreValueTypeName::StringType(variant)));
    }

    // Add all CoreIntegerValueTypeName variants
    for variant in CoreIntegerValueTypeName::iter() {
        result.push(CoreSchemaTypeName::ValueType(CoreValueTypeName::IntegerType(variant)));
    }

    // Add all CoreBooleanValueTypeName variants
    for variant in CoreBooleanValueTypeName::iter() {
        result.push(CoreSchemaTypeName::ValueType(CoreValueTypeName::BooleanType(variant)));
    }

    // Add all CoreEnumTypeName variants
    for variant in CoreEnumTypeName::iter() {
        result.push(CoreSchemaTypeName::ValueType(CoreValueTypeName::EnumType(variant)));
    }

    // Add all CoreMetaTypeName variants
    for variant in CoreMetaTypeName::iter() {
        result.push(CoreSchemaTypeName::MetaType(variant));
    }

    result
}
