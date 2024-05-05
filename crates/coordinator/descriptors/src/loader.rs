use holons::context::HolonsContext;
use holons::holon_error::HolonError;

use holons::staged_reference::StagedReference;

use crate::descriptor_types::Schema;

/// The load_core_schema function creates a new Schema Holon and populates it descriptors for all of the
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

pub fn load_core_schema(context: &HolonsContext) -> Result<StagedReference, HolonError> {
    let schema = Schema::new(
        "MAP L0 Core Schema".to_string(),
        "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string(),
    );

    let schema_ref = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(schema.0);
    /*

       let type_descriptor = define_type_descriptor(
           &context,
           schema_ref.clone_reference(),
           MapString(META_TYPE_DESCRIPTOR.to_string()),
           MapString("TypeDescriptor".to_string()),
           BaseType::Holon,
           MapString("A meta-descriptor that defines the properties and relationships shared by all MAP descriptors (including itself).".to_string()),
           MapString("Meta Type Descriptor".to_string()),
           MapBoolean(false),
           MapBoolean(false),
           None,
           None,
       );

       let type_descriptor_ref = context.commit_manager.borrow_mut().stage_new_holon(type_descriptor.0);

       // Add to Schema-COMPONENTS->TypeDescriptor relationships

       let meta_holon_descriptor = define_holon_descriptor(
           &context,
           schema_ref.clone_reference(),
           MapString("HolonDescriptor".to_string()),
           MapString("A meta-descriptor that defines the properties and relationships shared by all MAP HolonDescriptors".to_string()),
           MapString("Meta Holon Descriptor".to_string()),
           Some(type_descriptor_ref.clone_reference()),
           None,
       );

       let _meta_holon_descriptor_index = context.commit_manager.borrow_mut().stage_new_holon(meta_holon_descriptor.0);

       let meta_relationship_descriptor = define_type_descriptor(
           &context,
           schema_ref.clone_reference(),
           MapString(META_RELATIONSHIP_DESCRIPTOR.to_string()),
           MapString("RelationshipDescriptor".to_string()),
           BaseType::Holon,
           MapString("A meta-descriptor that defines the properties and relationships shared by all MAP RelationshipDescriptors".to_string()),
           MapString("Meta Relationship Descriptor".to_string()),
           MapBoolean(false),
           MapBoolean(false),
           None,
           Some(&type_descriptor),
       );

       let _meta_relationship_descriptor_index = context.commit_manager.borrow_mut().stage_new_holon(meta_relationship_descriptor.0);

       let meta_property_descriptor = define_type_descriptor(
           &context,
           schema_ref.clone_reference(),
           MapString(META_PROPERTY_DESCRIPTOR.to_string()),
           MapString("PropertyDescriptor".to_string()),
           BaseType::Holon,
           MapString("A meta-descriptor that defines the properties and relationships shared by all MAP PropertyDescriptors".to_string()),
           MapString("Property Meta Descriptor".to_string()),
           MapBoolean(false),
           MapBoolean(false),
           None,
           Some(&type_descriptor),
       );

       let _meta_property_descriptor_index = context.commit_manager.borrow_mut().stage_new_holon(meta_property_descriptor.0);

    */

    //context.commit_manager.borrow_mut().commit();

    Ok(schema_ref)
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
