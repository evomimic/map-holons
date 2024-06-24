use hdk::prelude::{info,debug,trace,warn};
use holons::commit_manager::{CommitManager, CommitResponse};
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapString};

use crate::descriptor_types::{CoreSchemaName, CoreMetaSchemaName, Schema};
use crate::holon_descriptor::{define_holon_type, HolonDefinition};
use crate::meta_type_loader::load_core_meta_types;
use crate::type_descriptor::TypeDefinitionHeader;
use crate::value_type_loader::load_core_value_types;

/// The load_core_schema function creates a new Schema Holon and populates it descriptors for all the
/// MAP L0 Schema Descriptors defined in `CoreSchemaNames`
///
/// It uses the transient collection in context's dance_state to support lookup of previously
/// created schema components so they may be referenced in relationship definition

///
/// The full implementation of this function will emerge incrementally... starting with a minimal schema
///

pub fn load_core_schema(context: &HolonsContext) -> Result<CommitResponse, HolonError> {

    info!("vvvvvvvv Entered: load_core_schema vvvvvvvvv");
    // Begin by staging `schema`. It's HolonReference becomes the target of
    // the COMPONENT_OF relationship for all schema components

    let schema = Schema::new(
        CoreSchemaName::SchemaName.as_map_string(),
        MapString("The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string()),
    )?;

    info!("Staging Schema...");
    let staged_schema_ref = HolonReference::Staged(context
        .commit_manager
        .borrow_mut().
        stage_new_holon(schema.0.clone()
        )?);

    context.add_references_to_dance_state(vec![staged_schema_ref.clone()])?;

    // Load the ValueTypes
    let (string_type_ref, integer_type_ref, boolean_type_ref)
        = load_core_value_types(context, &staged_schema_ref)?;

    // Load the MetaTypes
    load_core_meta_types(context, &staged_schema_ref)?;

    let type_name = CoreMetaSchemaName::MetaHolonType.as_map_string();

    debug!("Staging {:?}",type_name);
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

    let meta_holon_type_ref = HolonReference::Staged(define_holon_type(
        context,
        &staged_schema_ref,
        holon_definition, // provide property descriptors for this holon type here
    )?);

    context.add_references_to_dance_state(vec![meta_holon_type_ref.clone()])?;

    let type_name = CoreSchemaName::HolonType.as_map_string();
    let description = MapString("This type specifies the properties, relationships, and dances \
    for a type of Holon.".to_string());
    let label = MapString("Holon Type Descriptor".to_string());

    let type_header = TypeDefinitionHeader {
        descriptor_name: None,
        type_name,
        description,
        label,
        is_dependent: MapBoolean(false),
        is_value_type: MapBoolean(false),
        described_by: Some(meta_holon_type_ref),
        is_subtype_of:None,
        owned_by: None, // Holon Space
    };

    let
        holon_definition = HolonDefinition {
        header: type_header,
        properties:  vec![],
    };

    let holon_type_ref = HolonReference::Staged(define_holon_type(
        context,
        &staged_schema_ref,
        holon_definition,
    )?);






    info!("^^^^^^^ STAGING COMPLETE: Committing schema...");

    let response = CommitManager::commit(context);

    let r = response.clone();

    info!("Commit Response: {:#?}", r.status);
    info!("Commits Attempted: {:#?}", r.commits_attempted.0.to_string());
    info!("Holons Saved: {:#?}", r.saved_holons.len());
    info!("Abandoned: {:#?}", r.abandoned_holons.len());

    Ok(response)
}

// pub fn load_core_schema(context: &HolonsContext) -> Result<StagedReference, HolonError> {
//     let mut schema = Schema::new(
//         "MAP L0 Core Schema".to_string(),
//         "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string(),
//     );
//
//     let rc_schema = context.commit_manager.borrow_mut().stage_new_holon(schema.0); // Borrow_mut() allows mutation
//
//     // let schema_reference = define_local_target(&schema.into_holon());
//     let type_descriptor = define_type_descriptor(
//         &context,
//         rc_schema.clone(),
//         MapString(META_TYPE_DESCRIPTOR.to_string()),
//         MapString("TypeDescriptor".to_string()),
//         BaseType::Holon,
//         MapString("A meta-descriptor that defines the properties and relationships shared by all MAP descriptors (including itself).".to_string()),
//         MapString("Meta Type Descriptor".to_string()),
//         MapBoolean(false),
//         MapBoolean(false),
//         None,
//         None,
//     );
//
//     let _rc_type_descriptor = context.commit_manager.borrow_mut().stage_new_holon(type_descriptor.0.clone());
//     // Add to Schema-COMPONENTS->TypeDescriptor relationships?
//
//     let meta_holon_descriptor = define_holon_descriptor(
//         &context,
//         rc_schema.clone(),
//         MapString("HolonDescriptor".to_string()),
//         MapString("A meta-descriptor that defines the properties and relationships shared by all MAP HolonDescriptors".to_string()),
//         MapString("Meta Holon Descriptor".to_string()),
//         Some(&type_descriptor),
//         //Some(HolonReference::Local((LocalHolonReference::from_holon((type_descriptor.as_holon()))))),
//         None);
//     let _rc_meta_holon_descriptor = context.commit_manager.borrow_mut().stage_new_holon(meta_holon_descriptor.0);
//
//     let meta_relationship_descriptor = define_type_descriptor(
//         &context,
//         rc_schema.clone(),
//         MapString(META_RELATIONSHIP_DESCRIPTOR.to_string()),
//         MapString("RelationshipDescriptor".to_string()),
//         BaseType::Holon,
//         MapString("A meta-descriptor that defines the properties and relationships shared by all MAP RelationshipDescriptors".to_string()),
//         MapString("Meta Relationship Descriptor".to_string()),
//         MapBoolean(false),
//         MapBoolean(false),
//         None,
//         Some(&type_descriptor));
//     context.commit_manager.borrow_mut().stage_new_holon(meta_relationship_descriptor.0);
//
//     let meta_property_descriptor = define_type_descriptor(
//         &context,
//         rc_schema.clone(),
//         MapString(META_PROPERTY_DESCRIPTOR.to_string()),
//         MapString("PropertyDescriptor".to_string()),
//         BaseType::Holon,
//         MapString("A meta-descriptor that defines the properties and relationships shared by all MAP PropertyDescriptors".to_string()),
//         MapString("Property Meta Descriptor".to_string()),
//         MapBoolean(false),
//         MapBoolean(false),
//         None,
//         Some(&type_descriptor));
//
//
//     context.commit_manager.borrow_mut().stage_new_holon(meta_property_descriptor.0);
//
//     context.commit_manager.borrow_mut().commit();
//
//
//     Ok(rc_schema)
// }
