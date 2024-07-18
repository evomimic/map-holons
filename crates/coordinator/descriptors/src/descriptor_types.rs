use inflector::cases::snakecase::to_snake_case;
use inflector::cases::titlecase::to_title_case;

use inflector::Inflector;

use holons::holon::Holon;
use holons::relationship::RelationshipName;
use shared_types_holon::PropertyName;
use shared_types_holon::value_types::{MapEnumValue, MapString};
use crate::boolean_descriptor::BooleanTypeDefinition;
use crate::collection_descriptor::CollectionTypeDefinition;
use crate::enum_descriptor::EnumTypeDefinition;
use crate::holon_descriptor::HolonTypeDefinition;
use crate::integer_descriptor::IntegerTypeDefinition;
use crate::meta_type_descriptor::MetaTypeDefinition;
use crate::property_descriptor::PropertyTypeDefinition;
use crate::relationship_descriptor::RelationshipTypeDefinition;
use crate::string_descriptor::StringTypeDefinition;

/// All MAP Descriptors are stored as TypeDescriptor Holons
/// This file uses the [newtype pattern](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)
/// to wrap TypeDescriptor Holons in specific types in order to allow type-safe references
/// to different kinds of descriptors (assuming that guard functions are provided that check
/// the type of the wrapped holon).
///
/// TODO: Implement a full-blown native Rust types layer for descriptors with adaptors to/from Holon representations
/// TODO: In this type-safe layer, should TypeDescriptor be a Rust Enum with variants for each descriptor type?

pub struct Schema(pub Holon);
// pub struct Schema {
//     pub schema_name: MapString,
//     pub description: MapString,
// }

pub struct TypeDescriptor(pub Holon);
pub struct HolonType(pub Holon);
pub struct RelationshipType(pub Holon);
pub struct PropertyDescriptor(pub Holon);
pub struct StringType(pub Holon);
pub struct IntegerType(pub Holon);
pub struct BooleanType(pub Holon);
pub struct EnumType(pub Holon);

#[derive(Debug)]
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

pub enum SchemaTypeDefinition {
    BooleanType(BooleanTypeDefinition),
    CollectionType(CollectionTypeDefinition),
    EnumType(EnumTypeDefinition),
    HolonType(HolonTypeDefinition),
    IntegerType(IntegerTypeDefinition),
    MetaType(MetaTypeDefinition),
    PropertyType(PropertyTypeDefinition),
    RelationshipType(RelationshipTypeDefinition),
    StringType(StringTypeDefinition),
}

trait SchemaNamesTrait {
    /// This method returns unique type_name for this type
    fn derive_type_name(&self) -> MapString;
    /// This method returns the unique "descriptor_name" for this type
    fn derive_descriptor_name(&self) -> MapString;
    fn derive_label(&self) -> MapString;
    /// This method returns the A human-readable description of this type. It should
    /// clarify the purpose of the type and any caveats or to be aware of.
    fn derive_description(&self) -> MapString;
}


pub enum CoreMetaSchemaName {
    MetaBooleanType,
    MetaDanceType,
    MetaEnumType,
    MetaEnumVariantType,
    MetaHolonType,
    MetaIntegerType,
    MetaPropertyType,
    MetaRelationshipType,
    MetaStringType,
    MetaType,
    MetaValueArrayType,
    MetaValueType,
}

impl CoreMetaSchemaName {
    pub fn as_str(&self) -> &str {
        use CoreMetaSchemaName::*;
        match self {
            MetaBooleanType => "MetaBooleanType",
            MetaDanceType => "MetaDanceType",
            MetaEnumType => "MetaEnumType",
            MetaEnumVariantType => "MetaEnumVariantType",
            MetaHolonType => "MetaHolonType",
            MetaIntegerType => "MetaIntegerType",
            MetaPropertyType => "MetaPropertyType",
            MetaRelationshipType => "MetaRelationshipType",
            MetaStringType => "MetaStringType",
            MetaType => "MetaType",
            MetaValueArrayType => "MetaValueArrayType",
            MetaValueType => "MetaValueType",
        }
    }

    pub fn as_type_name(&self) -> MapString {
        MapString(self.as_str().to_camel_case())
    }
    pub fn as_descriptor_name(&self) -> MapString {
        MapString(format!("{}Descriptor", self.as_type_name().0.clone()))
    }
}

pub enum CoreValueTypeName {
    BaseTypeEnumType,
    DeletionSemanticEnumType,
    HolonStateEnumType,
    MapBooleanType,
    MapIntegerType,
    MapStringType,
    PropertyNameType,
    RelationshipNameType,
    SemanticVersionType,
}

impl CoreValueTypeName {
    pub fn as_str(&self) -> &str {
        use CoreValueTypeName::*;
        match self {
            BaseTypeEnumType => "BaseTypeEnumType",
            DeletionSemanticEnumType => "DeletionSemanticEnumType",
            HolonStateEnumType => "HolonStateEnumType",
            MapBooleanType => "MapBooleanType",
            MapIntegerType => "MapIntegerType",
            MapStringType => "MapStringType",
            PropertyNameType => "PropertyNameType",
            RelationshipNameType => "RelationshipNameType",
            SemanticVersionType => "SemanticVersionType",
        }
    }
}

#[derive(Debug)]
pub enum CoreSchemaPropertyTypeName {
    AllowsDuplicates, // MapBooleanType
    BaseType, // Enum -- BaseTypeEnumType
    DeletionSemantic, // Enum -- DeletionSemanticEnumType
    DescriptorName, // MapStringType
    Description, // MapStringType
    IsBuiltinType, // MapBooleanType
    IsDependent, // MapBooleanType
    IsOrdered, // MapBooleanType
    IsValueType, // MapBooleanType
    Label, // MapStringType
    MaxCardinality,// MapIntegerType
    MaxLength,// MapIntegerType
    MaxValue,// MapIntegerType
    MinCardinality, // MapIntegerType
    MinLength, // MapIntegerType
    MinValue, // MapIntegerType
    PropertyTypeName, // MapString --PropertyNameType
    RelationshipName, // MapString --RelationshipNameType
    SchemaName, // MapStringType
    TypeName, // MapStringType
    VariantOrder, // MapIntegerType
    Version, // MapString --SemanticVersionType
}
impl SchemaNamesTrait for CoreSchemaPropertyTypeName {
    /// This method returns the unique type_name for this property type in "snake_case"
    fn derive_type_name(&self) -> MapString {
        // this implementation assumes #Debug representation of the VariantNames within this enum
        MapString(to_snake_case(&format!("{:?}", self)))
    }

    /// This method returns the "descriptor_name" for this type in snake_case
    fn derive_descriptor_name(&self) -> MapString {
        // this implementation uses a simple naming rule of appending "_descriptor" to the type_name
        MapString(format!("{}_descriptor", self.derive_type_name().0.clone()))
    }
    /// This method returns the human-readable name for this property type
    fn derive_label(&self) -> MapString {
        // this implementation uses a simple naming rule simply converting the type name to
        // "Title Case" -- i.e., separating the type_name into (mostly) capitalized words.
        MapString(to_title_case(&format!("{:?}", self)))
    }
    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        use CoreSchemaPropertyTypeName::*;
        match self {
            AllowsDuplicates => MapString("If true, this collection can contain duplicate items.".to_string()),
            BaseType => MapString("Specifies the MAP BaseType of this object. ".to_string()),
            DeletionSemantic => MapString("Offers different options for whether requests to delete a \
            source Holon (i.e., mark as deleted) should be allowed for a given relationship.".to_string()),
            DescriptorName => MapString("The name for the unique key for the descriptor of MAP type.".to_string()),
            Description => MapString("A human readable description of this type that should \
            clarify the purpose of the type and any caveats or to aware of.".to_string()),
            IsBuiltinType => MapString("If `true`, this a type offered by Map Core. Otherwise \
            this is an agent-defined type that extends the MapCore Schema.".to_string()),
            IsDependent => MapString("If true, then instances of this type cannot exist \
            independently of some parent. For example, properties can not exist independently \
            of their holon.".to_string()),
            IsOrdered => MapString("If true, then the position of members of this collection \
            conforms to some order. In other words, this collection behaves like an array".to_string()),
            IsValueType => MapString("If true, this type can be used as the value type for a \
            property.".to_string()),
            Label => MapString("A human readable name for this property. Typically used in when \
            displaying a property in the Human Experience of the map as part of a label/value pair"
                .to_string()),
            MaxCardinality => MapString("Specifies the maximum number of members allowed in this \
            collection. max_cardinality must be greater than or equal to min_cardinality.".to_string()),
            MaxLength => MapString("max_length is a property of a value type based on the BaseType \
            MapString. It defines the maximum allowed length for string instances of this value \
            type. max_length must be greater than or equal to min_length.".to_string()),
            MaxValue => MapString("max_value is a property of a value type based on the BaseType \
            MapInteger. It defines the largest allowed value for this integer instances of this value \
            type. max_value must be greater than or equal to min_value.".to_string()),
            MinCardinality => MapString("Specifies the minimum number of members allowed in this \
            collection. min_cardinality must be greater than or equal to zero.".to_string()),
            MinLength => MapString("min_length is a property of a value type based on the BaseType \
            MapString. It defines the minimum allowed length for string instances of this value \
            type. min_length must be greater than or equal to zero.".to_string()),
            MinValue => MapString("min_value is a property of a value type based on the BaseType \
            MapInteger. It defines the smallest allowed value for this integer instances of this \
            value type. min_value can be negative and must be less than or equal to max_value."
                .to_string()),
            PropertyTypeName => MapString("Specifies the (internal) name for this property type."
                .to_string()),
            RelationshipName => MapString("Specifies the (internal) name for this relationship \
             type.".to_string()),
            SchemaName => MapString("Specifies the human-readable name for this schema.".to_string()),
            TypeName => MapString("Specifies the (internal) name for this type.".to_string()),
            VariantOrder => MapString("Specifies the ordering (e.g., for sorting or salience \
            purposes) for this specific variant relative to other variants in this enum.".to_string()),
            Version => MapString("Specifies the semantic version of this type descriptor.".to_string()),
        }
    }
}

impl CoreSchemaPropertyTypeName {
    pub fn as_snake_case(&self) -> &str {
        use CoreSchemaPropertyTypeName::*;
        match self {
            AllowsDuplicates => "allows_duplicates_property",
            BaseType => "base_type",
            DeletionSemantic => "deletion_semantic_property",
            DescriptorName => "descriptor_name_property",
            Description => "description_property",
            IsBuiltinType => "is_builtin_type_property",
            IsDependent => "is_dependent_property",
            IsOrdered => "is_ordered_property",
            IsValueType => "is_value_type_property",
            Label => "label_property",
            MaxCardinality => "max_cardinality_property",
            MaxLength => "max_length_property",
            MaxValue => "max_value_property",
            MinCardinality => "min_cardinality_property",
            MinLength => "min_length_property",
            MinValue => "min_value_property",
            PropertyTypeName => "property_type_name_property",
            RelationshipName => "relationship_name_property",
            SchemaName => "schema_name_property",
            TypeName => "type_name_property",
            VariantOrder => "variant_order",
            Version => "version_property",
        }
    }

    pub fn as_property_name(&self) -> PropertyName {
        PropertyName(MapString(self.as_snake_case().to_string()))
    }
    pub fn as_property_descriptor_name(&self) -> MapString {
        MapString(format!("{}_descriptor", self.as_snake_case().to_string()))
    }

}

pub enum CoreSchemaName {
    DeletionSemanticEnumType,
    DeletionSemanticEnumVariantAllow,
    DeletionSemanticEnumVariantBlock,
    DeletionSemanticEnumVariantPropagate,
    HolonStateEnumType,
    HolonStateEnumVariantAbandoned,
    HolonStateEnumVariantChanged,
    HolonStateEnumVariantFetched,
    HolonStateEnumVariantNew,
    HolonStateEnumVariantSaved,
    HolonType,
    MapBooleanType,
    MapIntegerType,
    MapStringType,
    SchemaName,
    SchemaType,
    SemanticVersionType,
}

impl CoreSchemaName {
    pub fn as_str(&self) -> &str {
        use CoreSchemaName::*;
        match self {
            DeletionSemanticEnumType => "DeletionSemanticEnum",
            DeletionSemanticEnumVariantAllow => "DeletionSemantic::Allow",
            DeletionSemanticEnumVariantBlock => "DeletionSemantic::Block",
            DeletionSemanticEnumVariantPropagate => "DeletionSemantic::Propagate",
            HolonStateEnumType => "HolonStateEnum",
            HolonStateEnumVariantAbandoned => "HolonState::Abandoned",
            HolonStateEnumVariantChanged => "HolonState::Changed",
            HolonStateEnumVariantFetched => "HolonState::Fetched",
            HolonStateEnumVariantNew => "HolonState::New",
            HolonStateEnumVariantSaved => "HolonState::Saved",
            HolonType => "HolonType",
            MapBooleanType => "MapBoolean",
            MapIntegerType => "MapInteger",
            MapStringType => "MapString",
            SchemaName => "MAP Core Schema",
            SchemaType => "MapSchemaType",
            SemanticVersionType => "SemanticVersion",
        }
    }

    pub fn as_map_string(&self) -> MapString {
        MapString(self.as_str().to_string())
    }
}

pub enum CoreSchemaRelationshipTypeName {
    CollectionFor,
    ComponentOf,
    Components,
    DanceOf,
    Dances,
    DescribedBy,
    DescriptorProperties,
    DescriptorRelationships,
    ForCollectionType,
    HasInverse,
    InverseOf,
    Instances,
    KeyProperties,
    KeyPropertyOf,
    OwnedBy,
    Owns,
    Properties,
    PropertyTypeFor,
    SourceType,
    TargetCollectionType,
    TargetHolonType,
    TargetPropertyType,
    Type,
    TypeDescriptor,
    ValueType,
    ValueTypeFor,
    VariantOf,
    Variants,
}

impl CoreSchemaRelationshipTypeName {
    pub fn as_str(&self) -> &str {
        use CoreSchemaRelationshipTypeName::*;
        match self {
            CollectionFor => "COLLECTION_FOR",
            ComponentOf => "COMPONENT_OF",
            Components => "COMPONENTS",
            DanceOf => "DANCE_OF",
            Dances => "DANCES",
            DescribedBy => "DESCRIBED_BY",
            DescriptorProperties => "DESCRIPTOR_PROPERTIES",
            DescriptorRelationships => "DESCRIPTOR_RELATIONSHIPS",
            ForCollectionType => "FOR_COLLECTION_TYPE",
            HasInverse => "HAS_INVERSE",
            InverseOf => "INVERSE_OF",
            Instances => "INSTANCES",
            KeyProperties => "KEY_PROPERTIES",
            KeyPropertyOf => "KEY_PROPERTY_OF",
            OwnedBy => "OwnedBy",
            Owns => "OWNS",
            Properties => "PROPERTIES",
            PropertyTypeFor => "PROPERTY_TYPE_FOR",
            SourceType => "SOURCE_TYPE",
            TargetCollectionType => "TARGET_COLLECTION_TYPE",
            TargetHolonType => "TARGET_HOLON_TYPE",
            TargetPropertyType => "TARGET_PROPERTY_TYPE",
            Type => "TYPE",
            TypeDescriptor => "TYPE_DESCRIPTOR",
            ValueType => "VALUE_TYPE",
            ValueTypeFor => "VALUE_TYPE_FOR",
            VariantOf => "VARIANT_OF",
            Variants => "VARIANTS",
        }
    }

    pub fn as_type_name(&self) -> MapString {
        MapString(self.as_str().to_string())
    }

    pub fn as_label(&self) -> MapString {
        MapString(self.as_str().to_string())
    }

    pub fn as_rel_name(&self) -> RelationshipName {
        RelationshipName(MapString(self.as_str().to_string()))
    }

}













