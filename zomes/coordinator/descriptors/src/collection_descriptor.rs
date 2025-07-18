use hdi::prelude::debug;

use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName};
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};
use base_types::{BaseValue, MapBoolean, MapInteger, MapString};
use core_types::{HolonError, TypeKind};
use holons_core::core_shared_objects::stage_new_holon_api;
use holons_core::{
    core_shared_objects::holon::TransientHolon, HolonReference, HolonsContextBehavior,
    ReadableHolon, StagedReference, WriteableHolon,
};
use integrity_core_types::PropertyName;

pub struct CollectionTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub collection_type_name: Option<MapString>,
    pub is_ordered: MapBoolean,
    pub allows_duplicates: MapBoolean,
    pub min_cardinality: MapInteger,
    pub max_cardinality: MapInteger,
    pub target_holon_type: HolonReference,
}

#[derive(Debug)]
pub enum CollectionSemantic {
    SingleInstance,
    OptionalInstance,
    UniqueList,
    List,
    Set,
}

/// This function defines and stages (but does not persist) a new CollectionType and its
/// associated TypeDescriptor.
/// This function sets values for each of the CollectionDescriptor properties and adds a
/// TARGET_HOLON_TYPE -> HolonType relationship to the CollectionType
///
/// *Naming Rules*: If `collection_type_name` is `None`, the `collection_type_name` will be automatically
/// derived using the following rule:
///     if max_cardinality =1, use target_holon_type name and append "Collection",
///     if max_cardinality >1, append an "s" to the target_holon_type name and append "Collection",
///
/// Example, assume collection_type_name=None:
///      Target Holon's type_name = "PropertyType", max_cardinality=1:
///           collection_type_name = PropertyTypeCollection
///      Target Holon's type_name = "PropertyType", max_cardinality=200:
///           collection_type_name = PropertyTypesCollection
///

pub fn define_collection_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: CollectionTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // Stage the new TypeDescriptor

    let type_descriptor_ref =
        define_type_descriptor(context, schema, TypeKind::Collection, definition.header.clone())?;

    // Build the new type

    let collection_type_name = generate_collection_type_name(context, &definition)?;

    let mut collection_type = TransientHolon::new();

    debug!("{:#?}", collection_type.clone());
    // Add its properties

    collection_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(collection_type_name.clone())),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::MaxCardinality.as_snake_case().to_string(),
            )),
            Some(BaseValue::IntegerValue(definition.max_cardinality)),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::MinCardinality.as_snake_case().to_string(),
            )),
            Some(BaseValue::IntegerValue(definition.min_cardinality)),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::AllowsDuplicates.as_snake_case().to_string(),
            )),
            Some(BaseValue::BooleanValue(definition.allows_duplicates)),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::IsOrdered.as_snake_case().to_string(),
            )),
            Some(BaseValue::BooleanValue(definition.is_ordered)),
        )?
        .with_property_value(
            PropertyName(MapString(
                CoreSchemaPropertyTypeName::TypeName.as_snake_case().to_string(),
            )),
            Some(BaseValue::StringValue(collection_type_name)),
        )?;

    // Stage the type

    debug!("{:#?}", collection_type.clone());

    let collection_type_ref = stage_new_holon_api(context, collection_type.clone())?;

    // Add its relationships

    collection_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)],
    )?;
    collection_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TargetHolonType.as_rel_name(),
        vec![definition.target_holon_type],
    )?;

    Ok(collection_type_ref)
}
/// Helper function that generates the collection_type_name per rules, if None provided
/// otherwise it just returns the supplied collection_name
fn generate_collection_type_name(
    context: &dyn HolonsContextBehavior,
    definition: &CollectionTypeDefinition,
) -> Result<MapString, HolonError> {
    // let mut name = target_type.get_property_value(context, PropertyName(MapString("type_name".to_string())))?;
    // append "Collection"
    match &definition.collection_type_name {
        Some(name) => Ok(MapString(name.0.clone())),
        None => {
            let holon_type_name = PropertyName(MapString("type_name".to_string()));
            let base_name =
                &definition.target_holon_type.get_property_value(context, &holon_type_name)?;
            if definition.max_cardinality.0 == 1 {
                Ok(MapString(format!("{:?}Collection", base_name)))
            } else {
                Ok(MapString(format!("{:?}sCollection", base_name)))
            }
        }
    }
}
