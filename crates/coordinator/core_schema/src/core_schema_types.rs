use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::MapString;
use crate::boolean_value_type_loader::CoreBooleanValueTypeName;
use crate::enum_type_loader::CoreEnumTypeName;
use crate::enum_variant_loader::CoreEnumVariantTypeName;
use crate::holon_type_loader::CoreHolonTypeName;
use crate::integer_value_type_loader::CoreIntegerValueTypeName;
use crate::property_type_loader::CorePropertyTypeName;
use crate::string_value_type_loader::CoreStringValueTypeName;


pub enum CoreSchemaTypeName {
    ValueType(CoreValueTypeName),
    PropertyType(CorePropertyTypeName),
    EnumVariantType(CoreEnumVariantTypeName),
    HolonType(CoreHolonTypeName),
//     CoreRelationshipType,
//     CoreHolonType,
//     CoreCollectionType,
//     CoreMetaType,
}

#[derive(Debug)]
pub enum CoreValueTypeName {
    StringType(CoreStringValueTypeName),
    IntegerType(CoreIntegerValueTypeName),
    BooleanType(CoreBooleanValueTypeName),
    EnumType(CoreEnumTypeName),
}


pub trait SchemaNamesTrait {
    /// This function is used to get a HolonReference to the TypeDefinition for a CoreSchemaTypeName
    /// It first checks if that definition has been stashed in dance_state.
    /// If not, it searches the persistent store for TypeDefinition whose key is `desired_type_name`
    /// If still not found, it invokes the core_type_loader method on desired_type_name to stage
    /// the desired type and return a HolonReference to the staged holon.
    fn lazy_get_core_type_definition(
        &self,
        context: &HolonsContext,
        schema: &HolonReference,
    ) -> Result<HolonReference, HolonError> {
        // See if definition for this type has already been loaded
        let key = self.derive_type_name();
        let definition_reference = context.get_by_key_from_dance_state(&key)?;

        match definition_reference {
            Some(result) => Ok(result),
            None => { // Couldn't get a reference to existing type definition, so load it ourselves
                let staged_ref = self.load_core_type(context, schema)?;
                Ok(HolonReference::Staged(staged_ref))
            },
        }
    }

    /// This method stages a type definition for this type
    fn load_core_type(
        &self,
        context: &HolonsContext,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError>;

    /// This method derives the type name as a `MapString`.
    fn derive_type_name(&self) -> MapString;

    /// This method returns the unique "descriptor_name" for this type
    fn derive_descriptor_name(&self) -> MapString;

    /// This method derives the label for this type as a `MapString`.
    fn derive_label(&self) -> MapString;

    /// This method returns a human-readable description of this type. It should
    /// clarify the purpose of the type and any caveats or considerations to be aware of.
    fn derive_description(&self) -> MapString;
}

impl SchemaNamesTrait for CoreSchemaTypeName {
    // fn lazy_get_core_type_definition(
    //     &self,
    //     context: &HolonsContext,
    //     schema: &HolonReference,
    // ) -> Result<HolonReference, HolonError> {
    //     match self {
    //         CoreSchemaTypeName::ValueType(inner) => inner.lazy_get_core_type_definition(context, schema),
    //         CoreSchemaTypeName::PropertyType(inner) => inner.lazy_get_core_type_definition(context, schema),
    //         CoreSchemaTypeName::EnumVariantType(inner) => inner.lazy_get_core_type_definition(context, schema),
    //         CoreSchemaTypeName::HolonType(inner) => inner.lazy_get_core_type_definition(context, schema),
    //     }
    // }

    fn load_core_type(
        &self,
        context: &HolonsContext,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
        match self {
            CoreSchemaTypeName::ValueType(inner) => inner.load_core_type(context, schema),
            CoreSchemaTypeName::PropertyType(inner) => inner.load_core_type(context, schema),
            CoreSchemaTypeName::EnumVariantType(inner) => inner.load_core_type(context, schema),
            CoreSchemaTypeName::HolonType(inner) => inner.load_core_type(context, schema),
        }
    }

    fn derive_type_name(&self) -> MapString {
        match self {
            CoreSchemaTypeName::ValueType(inner) => inner.derive_type_name(),
            CoreSchemaTypeName::PropertyType(inner) => inner.derive_type_name(),
            CoreSchemaTypeName::EnumVariantType(inner) => inner.derive_type_name(),
            CoreSchemaTypeName::HolonType(inner) => inner.derive_type_name(),
        }
    }

    fn derive_descriptor_name(&self) -> MapString {
        match self {
            CoreSchemaTypeName::ValueType(inner) => inner.derive_descriptor_name(),
            CoreSchemaTypeName::PropertyType(inner) => inner.derive_descriptor_name(),
            CoreSchemaTypeName::EnumVariantType(inner) => inner.derive_descriptor_name(),
            CoreSchemaTypeName::HolonType(inner) => inner.derive_descriptor_name(),
        }
    }

    fn derive_label(&self) -> MapString {
        match self {
            CoreSchemaTypeName::ValueType(inner) => inner.derive_label(),
            CoreSchemaTypeName::PropertyType(inner) => inner.derive_label(),
            CoreSchemaTypeName::EnumVariantType(inner) => inner.derive_label(),
            CoreSchemaTypeName::HolonType(inner) => inner.derive_label(),
        }
    }

    fn derive_description(&self) -> MapString {
        match self {
            CoreSchemaTypeName::ValueType(inner) => inner.derive_description(),
            CoreSchemaTypeName::PropertyType(inner) => inner.derive_description(),
            CoreSchemaTypeName::EnumVariantType(inner) => inner.derive_description(),
            CoreSchemaTypeName::HolonType(inner) => inner.derive_description(),
        }
    }
}


