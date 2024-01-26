use holons::helpers::define_local_target;
use crate::descriptor_types::{Schema, TypeDescriptor};



/// The load_core_schema function creates a new Schema Holon and populates it descriptors for all of the
/// MAP L0 Schema Meta Descriptors
///     *  TypeDescriptor
///     *  HolonDescriptor
///     *  Relationship Descriptor
///     *  PropertyDescriptor
///     *  DanceDescriptor
///     *  ValueDescriptor
///     *  BooleanDescriptor
///     *  EnumDescriptor
///     *  EnumVariantDescriptor
///     *  IntegerDescriptor
///     *  StringDescriptor
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
    let schema_target = define_local_target(&schema.into_holon());





    schema


}
