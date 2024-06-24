use holons::holon::Holon;
use shared_types_holon::value_types::{MapEnumValue, MapString};


/// All MAP Descriptors are stored as TypeDescriptor Holons
/// This file uses the [newtype pattern](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)
/// to wrap TypeDescriptor Holons in specific types in order to allow type-safe references
/// to different kinds of descriptors (assuming that guard functions are provided that check
/// the type of the wrapped holon).
///
/// TODO: Implement a full-blown native Rust types layer for descriptors with adaptors to/from Holon representations
/// TODO: In this type-safe layer, should TypeDescriptor be a Rust Enum with variants for each descriptor type?
pub struct Schema(pub Holon);
pub struct TypeDescriptor(pub Holon);
pub struct HolonType(pub Holon);
pub struct RelationshipType(pub Holon);
pub struct PropertyDescriptor(pub Holon);
pub struct StringType(pub Holon);
pub struct IntegerType(pub Holon);
pub struct BooleanType(pub Holon);
pub struct EnumType(pub Holon);
// pub enum BuiltInDescriptorType {
//     HolonDescriptor("HolonDescriptor"),
//     HolonCollectionDescriptor("HolonCollectionDescriptor"),
//     HolonSpaceDescriptor("HolonSpaceDescriptor"),
//     RelationshipDescriptor("RelationshipDescriptor"),
//     PropertyDescriptor("PropertyDescriptor"),
//     DanceDescriptor("DanceDescriptor"),
//     ValueDescriptor("ValueDescriptor"),
//     BooleanDescriptor("BooleanDescriptor"),
//     EnumDescriptor("EnumDescriptor"),
//     EnumVariantDescriptor("EnumVariantDescriptor"),
//     IntegerDescriptor("IntegerDescriptor"),
//     StringDescriptor("StringDescriptor"),
//     ValueArrayDescriptor("ValueArrayDescriptor"),
// }

pub enum DeletionSemantic {
    Allow, // deleting source_holon has no impact on the target_holon(s)
    Block, // prevent deletion of source_holon if any target_holons are related
    Cascade, // if source_holon is deleted, then also delete any related target_holons

}

impl DeletionSemantic {

    pub(crate) fn to_enum_variant(&self) -> MapEnumValue {
        match self {
            DeletionSemantic::Allow => MapEnumValue(MapString("Allow".to_string())),
            DeletionSemantic::Block => MapEnumValue(MapString("Block".to_string())),
            DeletionSemantic::Cascade => MapEnumValue(MapString("Propagate".to_string())),
        }
    }
}

/// The following constants specify the type_names for L0 Schema Components
pub const SCHEMA_NAME: &str = "MAP Core L0 Schema";
pub const META_TYPE_TYPE: &str = "MetaType";
pub const META_HOLON_TYPE: &str = "MetaHolonType";
pub const META_RELATIONSHIP_TYPE: &str = "MetaRelationshipType";
pub const META_PROPERTY_TYPE: &str = "MetaPropertyType";
pub const META_DANCE_TYPE: &str = "MetaDanceType";
pub const META_VALUE_TYPE: &str = "MetaValueType";
pub const META_BOOLEAN_TYPE: &str = "MetaBooleanType";
pub const META_ENUM_TYPE: &str = "MetaEnumType";
pub const META_ENUM_VARIANT_TYPE: &str = "MetaEnumVariantType";
pub const META_INTEGER_TYPE: &str = "MetaIntegerType";
pub const META_STRING_TYPE: &str = "MetaStringType";
pub const META_VALUE_ARRAY_TYPE: &str = "MetaValueArrayType";

pub const MAP_STRING_TYPE:  &str = "MapString";
pub const MAP_INTEGER_TYPE:  &str = "MapInteger";
pub const MAP_BOOLEAN_TYPE:  &str = "MapBoolean";
pub const DELETION_SEMANTIC_ENUM: &str = "DeletionSemanticEnum";

pub enum CoreMetaSchemaName {
    MetaSchemaType,
    MetaType,
    MetaHolonType,
    MetaRelationshipType,
    MetaPropertyType,
    MetaDanceType,
    MetaValueType,
    MetaBooleanType,
    MetaEnumType,
    MetaEnumVariantType,
    MetaIntegerType,
    MetaStringType,
    MetaValueArrayType,
}

impl CoreMetaSchemaName {
    pub fn as_str(&self) -> &str {
        match self {
            CoreMetaSchemaName::MetaType => "MetaType",
            CoreMetaSchemaName::MetaHolonType => "MetaHolonType",
            CoreMetaSchemaName::MetaRelationshipType => "MetaRelationshipType",
            CoreMetaSchemaName::MetaPropertyType => "MetaPropertyType",
            CoreMetaSchemaName::MetaDanceType => "MetaDanceType",
            CoreMetaSchemaName::MetaValueType => "MetaValueType",
            CoreMetaSchemaName::MetaBooleanType => "MetaBooleanType",
            CoreMetaSchemaName::MetaEnumType => "MetaEnumType",
            CoreMetaSchemaName::MetaEnumVariantType => "MetaEnumVariantType",
            CoreMetaSchemaName::MetaIntegerType => "MetaIntegerType",
            CoreMetaSchemaName::MetaStringType => "MetaStringType",
            CoreMetaSchemaName::MetaValueArrayType => "MetaValueArrayType",
            CoreMetaSchemaName::MetaSchemaType => "MetaSchemaType"
        }
    }

    pub fn as_map_string(&self) -> MapString {
        MapString(self.as_str().to_string())
    }
}
pub enum CoreSchemaName {
    SchemaName,
    HolonType,
    MapStringType,
    MapIntegerType,
    MapBooleanType,
    SemanticVersionType,
    DeletionSemanticEnumType,
    DeletionSemanticEnumVariantAllow,
    DeletionSemanticEnumVariantBlock,
    DeletionSemanticEnumVariantPropagate,
    HolonStateEnumType,
    HolonStateEnumVariantNew,
    HolonStateEnumVariantFetched,
    HolonStateEnumVariantChanged,
    HolonStateEnumVariantSaved,
    HolonStateEnumVariantAbandoned,

}
impl CoreSchemaName {
    pub fn as_str(&self) -> &str {
        match self {
            CoreSchemaName::HolonType => "HolonType",
            CoreSchemaName::MapStringType => "MapString",
            CoreSchemaName::MapIntegerType => "MapInteger",
            CoreSchemaName::MapBooleanType => "MapBoolean",
            CoreSchemaName::SemanticVersionType => "SemanticVersion",
            CoreSchemaName::DeletionSemanticEnumType => "DeletionSemanticEnum",
            CoreSchemaName::SchemaName => "MAP Core Schema",
            CoreSchemaName::DeletionSemanticEnumVariantAllow => "DeletionSemantic::Allow",
            CoreSchemaName::DeletionSemanticEnumVariantBlock => "DeletionSemantic::Block",
            CoreSchemaName::DeletionSemanticEnumVariantPropagate => "DeletionSemantic::Propagate",
            CoreSchemaName::HolonStateEnumType => "HolonStateEnum",
            CoreSchemaName::HolonStateEnumVariantNew => "HolonState::New",
            CoreSchemaName::HolonStateEnumVariantFetched => "HolonState::Fetched",
            CoreSchemaName::HolonStateEnumVariantChanged => "HolonState::Changed",
            CoreSchemaName::HolonStateEnumVariantSaved =>"HolonState::Saved",
            CoreSchemaName::HolonStateEnumVariantAbandoned => "HolonState::Abandoned",
        }
    }

    pub fn as_map_string(&self) -> MapString {
        MapString(self.as_str().to_string())
    }
}










