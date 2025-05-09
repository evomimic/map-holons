use crate::core_schema_types::SchemaNamesTrait;
use descriptors::enum_variant_descriptor::{define_enum_variant_type, EnumVariantTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use hdi::prelude::info;
use holons_core::core_shared_objects::HolonError;
use holons_core::{HolonReference, HolonsContextBehavior, StagedReference};
use base_types::{MapBoolean, MapInteger, MapString};
use strum_macros::EnumIter;
// use crate::enum_variant_loader;
use crate::enum_variant_loader::CoreEnumVariantTypeName::{
    TypeKindCollection, TypeKindEnumVariant, TypeKindHolon, TypeKindProperty, TypeKindRelationship,
    TypeKindValueBoolean, TypeKindValueBooleanArray, TypeKindValueEnum, TypeKindValueEnumArray,
    TypeKindValueInteger, TypeKindValueIntegerArray, TypeKindValueString, TypeKindValueStringArray,
    DeletionSemanticAllow, DeletionSemanticBlock, DeletionSemanticCascade,
};

#[derive(Debug, Clone, Default, EnumIter)]
pub enum CoreEnumVariantTypeName {
    #[default]
    TypeKindHolon,
    TypeKindCollection,
    TypeKindProperty,
    TypeKindRelationship,
    TypeKindEnumVariant,
    TypeKindValueBoolean,
    TypeKindValueEnum,
    TypeKindValueInteger,
    TypeKindValueString,
    TypeKindValueBooleanArray,
    TypeKindValueEnumArray,
    TypeKindValueIntegerArray,
    TypeKindValueStringArray,
    DeletionSemanticAllow,
    DeletionSemanticBlock,
    DeletionSemanticCascade,
}
pub struct EnumVariantLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub variant_order: MapInteger,
}

impl SchemaNamesTrait for CoreEnumVariantTypeName {
    fn load_core_type(
        &self,
        context: &dyn HolonsContextBehavior,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = self.get_variant_loader();
        load_enum_variant_definition(context, schema, loader)
    }
    /// This method returns the unique type_name for this property type in "snake_case"
    fn derive_type_name(&self) -> MapString {
        // Assume VariantNames have been defined in the proper format (CamelCase)
        MapString(format!("{:?}", self))
    }

    /// This method returns the "descriptor_name" for this type in camel_case
    fn derive_descriptor_name(&self) -> MapString {
        // this implementation uses a simple naming rule of appending "_descriptor" to the type_name
        MapString(format!("{}Descriptor", self.derive_type_name().0.clone()))
    }
    /// This method returns the human-readable name for this property type
    fn derive_label(&self) -> MapString {
        panic!(
            "This trait function is not intended to be used for this type. \
        The 'label' for this type is explicitly defined in get_variant_loader()"
        )
    }

    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        panic!(
            "This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_variant_loader()"
        )
    }
}

impl CoreEnumVariantTypeName {
    /// This function returns the variant definition for a given variant type
    fn get_variant_loader(&self) -> EnumVariantLoader {
        // use CoreEnumVariantTypeName::*;
        // use shared_types_holon::MapInteger;
        // use crate::enum_variant_loader::CoreEnumVariantTypeName::{TypeKindCollection, TypeKindEnumVariant, TypeKindHolon, TypeKindProperty, TypeKindRelationship, TypeKindValueBoolean, TypeKindValueBooleanArray, TypeKindValueEnum, TypeKindValueEnumArray, TypeKindValueInteger, TypeKindValueIntegerArray, TypeKindValueString, TypeKindValueStringArray, DeletionSemanticAllow, DeletionSemanticBlock, DeletionSemanticCascade};
        // use crate::enum_variant_loader::EnumVariantLoader;
        match self {
            TypeKindHolon => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Holon".into()),
                label: MapString("Holon".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(1),
            },
            TypeKindCollection => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Collection".into()),
                label: MapString("Collection".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(2),
            },
            TypeKindProperty => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Property".into()),
                label: MapString("Property".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(3),
            },
            TypeKindRelationship => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Enum".into()),
                label: MapString("Enum".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(4),
            },
            TypeKindEnumVariant => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::EnumVariant".into()),
                label: MapString("EnumVariant".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(5),
            },
            TypeKindValueBoolean => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Value(Boolean)".into()),
                label: MapString("BooleanValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(6),
            },
            TypeKindValueEnum => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Value(Enum)".into()),
                label: MapString("EnumValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(7),
            },
            TypeKindValueInteger => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Value(Integer)".into()),
                label: MapString("IntegerValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(8),
            },
            TypeKindValueString => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::Value(String)".into()),
                label: MapString("StringValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(9),
            },
            TypeKindValueBooleanArray => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::ValueArray(Boolean)".into()),
                label: MapString("Holon".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(10),
            },
            TypeKindValueEnumArray => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::ValueArray(Enum)".into()),
                label: MapString("Array of EnumValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(11),
            },
            TypeKindValueIntegerArray => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::ValueArray(".into()),
                label: MapString("Array of IntegerValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(12),
            },
            TypeKindValueStringArray => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString("Describes a TypeKind::ValueArray(String)".into()),
                label: MapString("Array of StringValue".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(13),
            },
            DeletionSemanticAllow => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString(
                    "Deleting a source holon has no impact on the \
                holon(s) for this relationship."
                        .into(),
                ),
                label: MapString("Allow".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(1),
            },
            DeletionSemanticBlock => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString(
                    "Prevent deletion of source_holon if any target_holons \
                are related via this relationship."
                        .into(),
                ),
                label: MapString("Block".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(2),
            },
            DeletionSemanticCascade => EnumVariantLoader {
                type_name: self.derive_type_name(),
                descriptor_name: self.derive_descriptor_name(),
                description: MapString(
                    "if source_holon is deleted, then also delete any \
                target_holons related via this relationship."
                        .into(),
                ),
                label: MapString("Cascade".into()),
                described_by: None,
                owned_by: None,
                variant_order: MapInteger(3),
            },
        }
    }
}

/// This function handles the aspects of staging a new enum variant type definition that are common
/// to all enum variant types. It assumes the type-specific parameters have been set by the caller.
fn load_enum_variant_definition(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    loader: EnumVariantLoader,
) -> Result<StagedReference, HolonError> {
    let type_header = TypeDescriptorDefinition {
        descriptor_name: loader.descriptor_name,
        description: loader.description,
        label: loader.label,
        // TODO: add base_type: TypeKind::EnumVariant
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(false),
        described_by: loader.described_by,
        is_subtype_of: None,
        owned_by: loader.owned_by,
    };

    let definition = EnumVariantTypeDefinition {
        header: type_header,
        type_name: loader.type_name.clone(),
        variant_order: loader.variant_order,
    };

    info!("Preparing to stage descriptor for {:#?}", loader.type_name.clone());
    let staged_ref = define_enum_variant_type(context, schema, definition)?;

    context
        .get_space_manager()
        .get_transient_state()
        .borrow_mut()
        .add_references(context, vec![HolonReference::Staged(staged_ref.clone())])?;

    Ok(staged_ref)
}
