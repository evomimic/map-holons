use hdi::prelude::debug;

use holons_core::{HolonReference, HolonWritable, HolonsContextBehavior, StagedReference};

use holons_core::core_shared_objects::{Holon, HolonError, RelationshipName};

use crate::descriptor_types_deprecated::{
    CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName, DeletionSemantic,
};
use holons_core::core_shared_objects::stage_new_holon_api;
use base_types::{BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString};
use core_types::TypeKind;
use integrity_core_types::PropertyName;

use crate::type_header::{define_type_header, TypeHeaderSpec};

pub struct RelationshipTypeSpec {
    pub header: TypeHeaderSpec, // header.type_name is relationship_name
    pub relationship_type_name: RelationshipName,
    pub source_owns_relationship: MapBoolean,
    pub deletion_semantic: DeletionSemantic,
    pub load_links_immediate: MapBoolean,
    pub load_holons_immediate: MapBoolean,
    pub target_holon_type: HolonReference,
    pub target_min_cardinality: MapInteger,
    pub target_max_cardinality: MapInteger,
    pub target_semantic: MapEnumValue, // e.g., "Set", "List", "SingleInstance"
    pub has_inverse: Option<HolonReference>, // Inverse RelationshipType
}

/// This function defines and stages (but does not persist) a new RelationshipDescriptor.
/// Values for each of the RelationshipDescriptor properties will be set based on the supplied `RelationshipTypeSpec`.
///
/// # Naming Rules
/// - `type_name` := <source_for.type_name> "-" <relationship_name> "->" <target_of.type_name>
/// - `descriptor_name` := <type_name> "Descriptor"
///
/// # Assigned Properties
/// - `relationship_name`: the canonical name of the relationship (e.g., HAS_MEMBER)
/// - `source_owns_relationship`: whether the source holon owns the relationship
/// - `deletion_semantic`: defines what happens to target holons when the source is deleted
/// - `load_links_immediate`: whether link records are preloaded at runtime
/// - `load_holons_immediate`: whether linked holons are preloaded at runtime
/// - `target_min_cardinality`: minimum number of related target holons
/// - `target_max_cardinality`: maximum number of related target holons
/// - `target_semantic`: collection semantics of the relationship (e.g., "Set", "List", "SingleInstance")
///
/// # Populated Relationships
/// - `DESCRIBED_BY` → TypeDescriptor (internal relationship from RelationshipType to its descriptor holon)
/// - `TARGET_HOLON_TYPE` → HolonType (the HolonType this relationship points to)
/// - `HAS_INVERSE` → RelationshipType (optional inverse relationship, only if source owns it)
///
/// # Notes
/// - Validation enforces that only the owning side may specify an inverse.
/// - The resulting RelationshipType holon and its TypeDescriptor are staged but not yet committed.
pub fn define_relationship_type(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    definition: RelationshipTypeSpec,
) -> Result<StagedReference, HolonError> {
    // Validate the definition
    // TODO: Move this logic to the shared validation rules layer
    // Rule: Only the side of the relationship that "owns" the relationship should specify an inverse
    if !definition.source_owns_relationship.0 && definition.has_inverse.is_some() {
        return Err(HolonError::InvalidParameter(
            "Validation Error: since source does not own the \
        relationship, it should not specify an inverse."
                .into(),
        ));
    }

    // ----------------  GET A NEW TYPE HEADER -------------------------------

    // Stage the TypeHeader
    let type_header_ref =
        define_type_header(context, schema, TypeKind::Relationship, definition.header)?;

    // Build new Relationship Type

    let mut relationship_type = Holon::new();

    // Add its properties
    relationship_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            Some(BaseValue::StringValue(definition.relationship_type_name.0.clone())),
        )?
        .with_property_value(
            CoreSchemaPropertyTypeName::RelationshipName.as_property_name(),
            Some(BaseValue::StringValue(definition.relationship_type_name.0.clone())),
        )?
        .with_property_value(
            PropertyName(MapString("source_owns_relationship".to_string())),
            Some(BaseValue::BooleanValue(definition.source_owns_relationship)),
        )?
        .with_property_value(
            PropertyName(MapString("load_links_immediate".to_string())),
            Some(BaseValue::BooleanValue(definition.load_links_immediate)),
        )?
        .with_property_value(
            PropertyName(MapString("load_holons_immediate".to_string())),
            Some(BaseValue::BooleanValue(definition.load_holons_immediate)),
        )?
        .with_property_value(
            PropertyName(MapString("deletion_semantic".to_string())),
            Some(BaseValue::EnumValue(definition.deletion_semantic.to_enum_variant())),
        )?
        .with_property_value(
            PropertyName(MapString("target_min_cardinality".to_string())),
            Some(BaseValue::IntegerValue(definition.target_min_cardinality)),
        )?
        .with_property_value(
            PropertyName(MapString("target_max_cardinality".to_string())),
            Some(BaseValue::IntegerValue(definition.target_max_cardinality)),
        )?
        .with_property_value(
            PropertyName(MapString("target_semantic".to_string())),
            Some(BaseValue::EnumValue(definition.target_semantic)),
        )?;

    debug!("Staging new relationship_type {:#?}", relationship_type.clone());

    // Stage new holon type
    let relationship_type_ref = stage_new_holon_api(context, relationship_type.clone())?;

    // Add its relationships

    relationship_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_header_ref)],
    )?;
    relationship_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TargetHolonType.as_rel_name(),
        vec![definition.target_holon_type],
    )?;

    if let Some(inverse) = definition.has_inverse {
        relationship_type_ref.add_related_holons(
            context,
            CoreSchemaRelationshipTypeName::HasInverse.as_rel_name(),
            vec![inverse],
        )?
    };

    Ok(relationship_type_ref)
}
