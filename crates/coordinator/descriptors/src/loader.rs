use hdk::prelude::{info,debug,trace,warn};
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;

use crate::descriptor_types::Schema;
use crate::value_type_loader::load_core_value_types;

/// The load_core_schema function creates a new Schema Holon and populates it descriptors for all the
/// MAP L0 Schema Meta Descriptors
///     *  MetaTypeDescriptor
///     *  MetaHolonDescriptor
///     *  MetaRelationshipDescriptor
///     *  MetaPropertyDescriptor
///     *  MetaDanceDescriptor
///     *  MetaValueDescriptor
///     *  MetaBooleanDescriptor
///     *  MetaEnumDescriptor
///     *  MetaEnumVariantDescriptor
///     *  MetaIntegerDescriptor
///     *  MetaStringDescriptor
/// And their related types
///     *  SchemaHolonDescriptor
///     *  ConstraintHolonDescriptor
///     *  SemanticVersionHolonDescriptor
///     *  DeletionSemanticEnumDescriptor
///     *  DeletionSemanticEnumVariantAllow
///     *  DeletionSemanticEnumVariantBlock
///     *  DeletionSemanticEnumVariantPropagate
///     *  HolonStateEnumDescriptor
///     *  HolonStateEnumNewVariant
///     *  HolonStateEnumFetchedVariant
///     *  HolonStateEnumChangedVariant
///
/// The full implementation of this function will emerge incrementally... starting with a minimal schema
///

pub fn load_core_schema(context: &HolonsContext) -> Result<HolonReference, HolonError> {

    // Begin by staging and committing `schema`. It's HolonReference becomes the target of
    // the COMPONENT_OF relationship for all schema components

    let schema = Schema::new(
        "MAP L0 Core Schema".to_string(),
        "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string(),
    )?;


    let staged_schema_ref = context.commit_manager.borrow_mut().stage_new_holon(schema.0.clone());
    // TODO: Handle Result
    info!("Committing schema {:#?}", schema.0.clone());

    let response = CommitManager::commit(context);
    // Check if Commit is Complete, get a SmartReference to the saved SchemaHolon





    //load_core_value_types(context, &schema_ref);

    // TODO: Need to retrieve the saved Schema holon by key once get_holon_by_key dance is available.

    Err(HolonError::NotImplemented("Incomplete implementation of  load_core_schema".to_string()))



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
