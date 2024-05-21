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
pub struct HolonDescriptor(pub Holon);
pub struct RelationshipDescriptor(pub Holon);
pub struct PropertyDescriptor(pub Holon);
pub struct StringDescriptor(pub Holon);
pub struct IntegerDescriptor(pub Holon);
pub struct BooleanDescriptor(pub Holon);
pub struct EnumDescriptor(pub Holon);
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
    Propagate, // if source_holon is deleted, then also delete any related target_holons

}

impl DeletionSemantic {
    pub(crate) fn to_enum_variant(&self) -> MapEnumValue {
        match self {
            DeletionSemantic::Allow => MapEnumValue(MapString("Allow".to_string())),
            DeletionSemantic::Block => MapEnumValue(MapString("Block".to_string())),
            DeletionSemantic::Propagate => MapEnumValue(MapString("Propagate".to_string())),
        }
    }
}
/// The following constants specify the type_names for the L0 metadescriptors
pub const META_TYPE_DESCRIPTOR: &str = "MetaTypeDescriptor";
pub const META_HOLON_DESCRIPTOR: &str = "MetaHolonDescriptor";
pub const META_RELATIONSHIP_DESCRIPTOR: &str = "MetaRelationshipDescriptor";
pub const META_PROPERTY_DESCRIPTOR: &str = "MetaPropertyDescriptor";
pub const META_DANCE_DESCRIPTOR: &str = "MetaDanceDescriptor";
pub const META_VALUE_DESCRIPTOR: &str = "MetaValueDescriptor";
pub const META_BOOLEAN_DESCRIPTOR: &str = "MetaBooleanDescriptor";
pub const META_ENUM_DESCRIPTOR: &str = "MetaEnumDescriptor";
pub const META_ENUM_VARIANT_DESCRIPTOR: &str = "MetaEnumVariantDescriptor";
pub const META_INTEGER_DESCRIPTOR: &str = "MetaIntegerDescriptor";
pub const META_STRING_DESCRIPTOR: &str = "MetaStringDescriptor";
pub const META_VALUE_ARRAY_DESCRIPTOR: &str = "MetaValueArrayDescriptor";










