
use holons::holon_types::{Holon};


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
/// The following constants specify the type_names for the L0 metadescriptors
pub const TYPE_META_DESCRIPTOR: &str = "TypeMetaDescriptor";
pub const HOLON_META_DESCRIPTOR: &str = "HolonMetaDescriptor";
pub const RELATIONSHIP_META_DESCRIPTOR: &str = "RelationshipMetaDescriptor";
pub const PROPERTY_META_DESCRIPTOR: &str = "PropertyMetaDescriptor";
pub const DANCE_META_DESCRIPTOR: &str = "DanceMetaDescriptor";
pub const VALUE_META_DESCRIPTOR: &str = "ValueMetaDescriptor";
pub const BOOLEAN_META_DESCRIPTOR: &str = "BooleanMetaDescriptor";
pub const ENUM_META_DESCRIPTOR: &str = "EnumMetaDescriptor";
pub const ENUM_VARIANT_META_DESCRIPTOR: &str = "MetaDescriptor";
pub const INTEGER_META_DESCRIPTOR: &str = "IntegerMetaDescriptor";
pub const STRING_META_DESCRIPTOR: &str
= "StringMetaDescriptor";
pub const VALUE_ARRAY_META_DESCRIPTOR: &str = "ValueArrayMetaDescriptor";










