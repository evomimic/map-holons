use hdi::prelude::info;
use inflector::cases::snakecase::to_snake_case;
use inflector::cases::titlecase::to_title_case;
use descriptors::property_descriptor::{define_property_type, PropertyTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapString, PropertyName};
use crate::core_schema_types::SchemaNamesTrait;
use CorePropertyTypeName::*;
use crate::value_type_loader::CoreValueTypeName::*;
use crate::enum_type_loader::CoreEnumTypeName::*;
use crate::string_value_type_loader::CoreStringValueTypeName::*;
use crate::boolean_value_type_loader::CoreBooleanValueTypeName::*;
use crate::integer_value_type_loader::CoreIntegerValueTypeName::*;
// use crate::enum_type_loader::*;
// use crate::integer_value_type_loader::*;
// use crate::property_type_loader::CorePropertyTypeName::{TypeName, VariantName};
// use crate::string_value_type_loader::*;
use crate::value_type_loader::CoreValueTypeName;
//use crate::value_type_loader::load_core_value_type;


#[derive(Debug, Clone)]
pub enum CorePropertyTypeName {
    AllowDuplicates, // MapBooleanType
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
    Name, // MapDescriptorType
    PropertyTypeName, // MapString --PropertyNameType
    RelationshipName, // MapString --RelationshipNameType
    SchemaName, // MapStringType
    TypeName, // MapStringType
    VariantName, // MapStringType
    VariantOrder, // MapIntegerType
    Version, // MapString --SemanticVersionType
}
#[derive(Debug)]
pub struct PropertyTypeLoader {
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub property_name: PropertyName,
    pub value_type_name: CoreValueTypeName,
}

impl SchemaNamesTrait for CorePropertyTypeName {
     fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = PropertyTypeLoader {
            descriptor_name: self.derive_descriptor_name(),
            description: self.derive_description(),
            label: self.derive_label(),
            described_by: None, // TODO: Lazy get MetaPropertyDescriptor
            owned_by: None,
            property_name: PropertyName(self.derive_type_name()),
            value_type_name: self.specify_value_type(),
        };
        load_property_type_definition(context, schema, loader)

    }
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
        use CorePropertyTypeName::*;
        match self {
            AllowDuplicates => MapString("If true, this collection can contain duplicate items.".to_string()),
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
            Name => MapString("Specifies the human-readable name for this Holon."
                .to_string()),
            PropertyTypeName => MapString("Specifies the (internal) name for this property type."
                .to_string()),
            RelationshipName => MapString("Specifies the (internal) name for this relationship \
             type.".to_string()),
            SchemaName => MapString("Specifies the human-readable name for this schema.".to_string()),
            TypeName => MapString("Specifies the (internal) name for this type.".to_string()),
            VariantName => MapString("Specifies the (internal) name for this Variant.".to_string()),
            VariantOrder => MapString("Specifies the ordering (e.g., for sorting or salience \
            purposes) for this specific variant relative to other variants in this enum.".to_string()),
            Version => MapString("Specifies the semantic version of this type descriptor.".to_string()),
        }
    }
}
impl CorePropertyTypeName {
    /// This function returns the ValueType for this property type

    fn specify_value_type(&self) -> CoreValueTypeName {


        match self {
            AllowDuplicates => BooleanType(MapBooleanType),
            BaseType => EnumType(MapBaseType),
            DeletionSemantic => EnumType(DeletionSemanticType),
            DescriptorName => StringType(MapStringType),
            Description => StringType(MapStringType),
            IsBuiltinType => BooleanType(MapBooleanType),
            IsDependent => BooleanType(MapBooleanType),
            IsOrdered => BooleanType(MapBooleanType),
            IsValueType => BooleanType(MapBooleanType),
            Label => StringType(MapStringType),
            MaxCardinality => IntegerType(MapIntegerType),
            MaxLength => IntegerType(MapIntegerType),
            MaxValue => IntegerType(MapIntegerType),
            MinCardinality => IntegerType(MapIntegerType),
            MinLength => IntegerType(MapIntegerType),
            MinValue => IntegerType(MapIntegerType),
            Name => StringType(MapStringType),
            PropertyTypeName => StringType(PropertyNameType),
            RelationshipName => StringType(RelationshipNameType),
            SchemaName => StringType(MapStringType),
            TypeName => StringType(MapStringType),
            VariantName => StringType(MapStringType),
            VariantOrder => IntegerType(MapIntegerType),
            Version => StringType(SemanticVersionType),
        }
    }
}

/// This function handles the aspects of staging a new property type definition that are common
/// to all property types. It assumes the type-specific parameters have been set by the caller.
fn load_property_type_definition(
    context: &HolonsContext,
    schema: &HolonReference,
    loader: PropertyTypeLoader,
) -> Result<StagedReference, HolonError> {
    let type_header = TypeDescriptorDefinition {
        descriptor_name: loader.descriptor_name,
        description: loader.description,
        label: loader.label,
        // TODO: add base_type: BaseType::Property,
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(true),
        described_by: loader.described_by,
        is_subtype_of: None,
        owned_by: loader.owned_by,
    };
    // let value_type = HolonReference::Staged(load_core_value_type(
    //     context,
    //     schema,
    //     loader.value_type_name
    // )?);
    let value_type = HolonReference::Staged(loader.value_type_name.load_core_type(
        context,
        schema,
    )?);

    let definition = PropertyTypeDefinition {
        header: type_header,
        property_name: loader.property_name.clone(),
        value_type
    };

    info!("Preparing to stage descriptor for {:#?}",
        loader.property_name.clone());
    let staged_ref = define_property_type(
        context,
        schema,
        definition,
    )?;

    context.add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}





