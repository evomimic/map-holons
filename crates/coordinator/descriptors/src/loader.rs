use holons::holon_types::Holon;
use shared_types_holon::value_types::{BaseType, MapBoolean, MapString};
use crate::descriptor_types::{META_PROPERTY_DESCRIPTOR, META_RELATIONSHIP_DESCRIPTOR, META_TYPE_DESCRIPTOR, Schema};
use crate::holon_descriptor::define_holon_descriptor;
use crate::type_descriptor::define_type_descriptor;


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

pub fn load_core_schema() -> Schema {

    let mut schema = Schema::new(
        "MAP L0 Core Schema".to_string(),
        "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string()
    );

    // let schema_reference = define_local_target(&schema.into_holon());
    let type_descriptor = define_type_descriptor(&schema,
                                                 MapString(META_TYPE_DESCRIPTOR.to_string()),
                                                 MapString("TypeDescriptor".to_string()),
                                                 BaseType::Holon,
                                                 MapString("A meta-descriptor that defines the properties and relationships shared by all MAP descriptors (including itself).".to_string()),
                                                 MapString("Meta Type Descriptor".to_string()),
                                                 MapBoolean(false),
                                                 MapBoolean(false),
                                                 None,
                                                 None);

    // Add to Schema-COMPONENTS->TypeDescriptor relationshios?

    let meta_holon_descriptor = define_holon_descriptor(&schema,
                                                   MapString("HolonDescriptor".to_string()),
                                                   MapString("A meta-descriptor that defines the properties and relationships shared by all MAP HolonDescriptors".to_string()),
                                                   MapString("Meta Holon Descriptor".to_string()),
                                                   Some(&type_descriptor),
                                                   //Some(HolonReference::Local((LocalHolonReference::from_holon((type_descriptor.as_holon()))))),
                                                   None);
    let meta_relationship_descriptor = define_type_descriptor(&schema,
                                                              MapString(META_RELATIONSHIP_DESCRIPTOR.to_string()),
                                                              MapString("RelationshipDescriptor".to_string()),
                                                              BaseType::Holon,
                                                              MapString("A meta-descriptor that defines the properties and relationships shared by all MAP RelationshipDescriptors".to_string()),
                                                              MapString("Meta Relationship Descriptor".to_string()),
                                                              MapBoolean(false),
                                                              MapBoolean(false),
                                                              None,
                                                              Some(&type_descriptor));

    let meta_property_descriptor = define_type_descriptor(&schema,
                                                              MapString(META_PROPERTY_DESCRIPTOR.to_string()),
                                                              MapString("PropertyDescriptor".to_string()),
                                                              BaseType::Holon,
                                                              MapString("A meta-descriptor that defines the properties and relationships shared by all MAP PropertyDescriptors".to_string()),
                                                              MapString("Property Meta Descriptor".to_string()),
                                                              MapBoolean(false),
                                                              MapBoolean(false),
                                                              None,
                                                              Some(&type_descriptor));




    schema


}

