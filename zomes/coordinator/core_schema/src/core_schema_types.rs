// use crate::enum_variant_loader::CoreEnumVariantTypeName;
// use crate::holon_type_loader::CoreHolonTypeName;
// use crate::meta_type_loader::CoreMetaTypeName;
// use crate::property_type_loader::CorePropertyTypeName;
// use crate::value_type_loader::CoreValueTypeName;

// use base_types::MapString;
// use core_types::HolonError;
// use holons_core::{HolonReference, HolonsContextBehavior, StagedReference};

// #[derive(Debug, Clone)]
// pub enum CoreSchemaTypeName {
//     ValueType(CoreValueTypeName),
//     PropertyType(CorePropertyTypeName),
//     EnumVariantType(CoreEnumVariantTypeName),
//     HolonType(CoreHolonTypeName),
//     MetaType(CoreMetaTypeName),
//     // HolonCollectionType(HolonCollectionTypeName),
// }

// pub trait SchemaNamesTrait {
//     /// This function is used to get a HolonReference to the TypeDefinition for a CoreSchemaTypeName
//     /// It first checks if that definition has been stashed in dance_state.
//     /// If not, it searches the persistent store for TypeDefinition whose key is `desired_type_name`
//     /// If still not found, it invokes the core_type_loader method on desired_type_name to stage
//     /// the desired type and return a HolonReference to the staged holon.
//     fn lazy_get_core_type_definition(
//         &self,
//         context: &dyn HolonsContextBehavior,
//         schema: &HolonReference,
//     ) -> Result<HolonReference, HolonError> {
//         // See if definition for this type has already been loaded
//         let key = self.derive_type_name();
//         let definition_reference =
//             context.get_space_manager().get_transient_state().borrow().get_by_key(&key)?;

//         match definition_reference {
//             Some(result) => Ok(result),
//             None => {
//                 // Couldn't get a reference to existing type definition, so load it ourselves
//                 let staged_ref = self.load_core_type(context, schema)?;
//                 Ok(HolonReference::Staged(staged_ref))
//             }
//         }
//     }

//     /// This method stages a type definition for this type
//     fn load_core_type(
//         &self,
//         context: &dyn HolonsContextBehavior,
//         schema: &HolonReference,
//     ) -> Result<StagedReference, HolonError>;

//     /// This method derives the type name as a `MapString`.
//     fn derive_type_name(&self) -> MapString;

//     /// This method returns the unique "descriptor_name" for this type
//     fn derive_descriptor_name(&self) -> MapString;

//     /// This method derives the label for this type as a `MapString`.
//     fn derive_label(&self) -> MapString;

//     /// This method returns a human-readable description of this type. It should
//     /// clarify the purpose of the type and any caveats or considerations to be aware of.
//     fn derive_description(&self) -> MapString;
// }

// impl SchemaNamesTrait for CoreSchemaTypeName {
//     // fn lazy_get_core_type_definition(
//     //     &self,
//     //     context: &dyn HolonsContextBehavior,
//     //     schema: &HolonReference,
//     // ) -> Result<HolonReference, HolonError> {
//     //     match self {
//     //         CoreSchemaTypeName::ValueType(inner) => inner.lazy_get_core_type_definition(context, schema),
//     //         CoreSchemaTypeName::PropertyType(inner) => inner.lazy_get_core_type_definition(context, schema),
//     //         CoreSchemaTypeName::EnumVariantType(inner) => inner.lazy_get_core_type_definition(context, schema),
//     //         CoreSchemaTypeName::HolonType(inner) => inner.lazy_get_core_type_definition(context, schema),
//     //     }
//     // }

//     fn load_core_type(
//         &self,
//         context: &dyn HolonsContextBehavior,
//         schema: &HolonReference,
//     ) -> Result<StagedReference, HolonError> {
//         use CoreSchemaTypeName::*;
//         match self {
//             ValueType(inner) => inner.load_core_type(context, schema),
//             PropertyType(inner) => inner.load_core_type(context, schema),
//             EnumVariantType(inner) => inner.load_core_type(context, schema),
//             HolonType(inner) => inner.load_core_type(context, schema),
//             MetaType(inner) => inner.load_core_type(context, schema),
//         }
//     }

//     fn derive_type_name(&self) -> MapString {
//         use CoreSchemaTypeName::*;
//         match self {
//             ValueType(inner) => inner.derive_type_name(),
//             PropertyType(inner) => inner.derive_type_name(),
//             EnumVariantType(inner) => inner.derive_type_name(),
//             HolonType(inner) => inner.derive_type_name(),
//             MetaType(inner) => inner.derive_type_name(),
//         }
//     }

//     fn derive_descriptor_name(&self) -> MapString {
//         use CoreSchemaTypeName::*;
//         match self {
//             ValueType(inner) => inner.derive_descriptor_name(),
//             PropertyType(inner) => inner.derive_descriptor_name(),
//             EnumVariantType(inner) => inner.derive_descriptor_name(),
//             HolonType(inner) => inner.derive_descriptor_name(),
//             MetaType(inner) => inner.derive_descriptor_name(),
//         }
//     }

//     fn derive_label(&self) -> MapString {
//         use CoreSchemaTypeName::*;
//         match self {
//             ValueType(inner) => inner.derive_label(),
//             PropertyType(inner) => inner.derive_label(),
//             EnumVariantType(inner) => inner.derive_label(),
//             HolonType(inner) => inner.derive_label(),
//             MetaType(inner) => inner.derive_label(),
//         }
//     }

//     fn derive_description(&self) -> MapString {
//         use CoreSchemaTypeName::*;
//         match self {
//             ValueType(inner) => inner.derive_description(),
//             PropertyType(inner) => inner.derive_description(),
//             EnumVariantType(inner) => inner.derive_description(),
//             HolonType(inner) => inner.derive_description(),
//             MetaType(inner) => inner.derive_description(),
//         }
//     }
// }
