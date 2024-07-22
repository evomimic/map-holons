use hdi::prelude::debug;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::{BaseType, PropertyName};
use shared_types_holon::value_types::{BaseValue, MapBoolean, MapString};


use crate::descriptor_types::{CoreSchemaPropertyTypeName, CoreSchemaRelationshipTypeName, DeletionSemantic};
use crate::integer_descriptor::define_integer_type;
use crate::type_descriptor::{define_type_descriptor, TypeDescriptorDefinition};

pub struct RelationshipTypeDefinition {
    pub header: TypeDescriptorDefinition, // header.type_name is relationship_name
    pub relationship_type_name: RelationshipName,
    pub source_owns_relationship: MapBoolean,
    // pub min_target_cardinality: MapInteger,  // CollectionDefinition
    // pub max_target_cardinality: MapInteger, // CollectionDefinition
    pub deletion_semantic: DeletionSemantic,
    pub load_links_immediate: MapBoolean,
    pub load_holons_immediate: MapBoolean,
    //pub affinity: MapInteger,
    pub target_collection_type: HolonReference, // CollectionType
    pub has_inverse: Option<HolonReference>, // Inverse RelationshipType
}

/// This function defines and stages (but does not persist) a new RelationshipDescriptor.
/// Values for each of the RelationshipDescriptor properties will be set based on supplied parameters.
///
/// *Naming Rules*:
///     `type_name` := <source_for.type_name>"-"<relationship_name>"->"<target_of.type_name>"
///     `descriptor_name`:= `<type_name>"Descriptor"`
///
/// The function assigns values for the following RelationshipDescriptor properties:
///
/// * min_target_cardinality: MapInteger (must be >=0)
/// * max_target_cardinality: MapInteger (must be >= min_target_cardinality AND <= max_collection_size)
/// * owned
/// The function populates the following relationships:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonType (if supplied)
/// * SOURCE_FOR -> HolonType
/// * TARGET_HOLON_TYPE -> HolonType (if supplied)
/// * HAS_INVERSE->RelationshipType
///
///
pub fn define_relationship_type(
    context: &HolonsContext,
    schema: &HolonReference,
    definition: RelationshipTypeDefinition,
) -> Result<StagedReference, HolonError> {
    // Validate the definition
    // TODO: Move this logic to the shared validation rules layer
    // Rule: Only the side of the relationship that "owns" the relationship should specify an inverse
    if !definition.source_owns_relationship.0 && definition.has_inverse.is_some() {
        return Err(HolonError::InvalidParameter("Validation Error: since source does not own the \
        relationship, it should not specify an inverse.".into()));
    }

    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------

    // Stage the TypeDescriptor
    let type_descriptor_ref = define_type_descriptor(
        context,
        schema,
        BaseType::Relationship,
        definition.header,
    )?;

    // Build new Relationship Type

    let mut relationship_type = Holon::new();

    // Add its properties
    relationship_type
        .with_property_value(
            PropertyName(MapString("key".to_string())),
            BaseValue::StringValue(definition.relationship_type_name.0.clone()),
        )?
        .with_property_value(
            CoreSchemaPropertyTypeName::RelationshipName.as_property_name(),
            BaseValue::StringValue(definition.relationship_type_name.0.clone()),
        )?
        .with_property_value(
            PropertyName(MapString("source_owns_relationship".to_string())),
            BaseValue::BooleanValue(definition.source_owns_relationship),
        )?
        .with_property_value(
            PropertyName(MapString("load_links_immediate".to_string())),
            BaseValue::BooleanValue(definition.load_links_immediate),
        )?
        .with_property_value(
            PropertyName(MapString("load_holons_immediate".to_string())),
            BaseValue::BooleanValue(definition.load_holons_immediate),
        )?
        .with_property_value(
            PropertyName(MapString("deletion_semantic".to_string())),
            BaseValue::EnumValue(definition.deletion_semantic.to_enum_variant()),
        )?;

    debug!("Staging new relationship_type {:#?}", relationship_type.clone());

    // Stage new holon type
    let relationship_type_ref = context
        .commit_manager
        .borrow_mut()
        .stage_new_holon(relationship_type.clone())?;

    // Add its relationships

    relationship_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TypeDescriptor.as_rel_name(),
        vec![HolonReference::Staged(type_descriptor_ref)]
    )?;
    relationship_type_ref.add_related_holons(
        context,
        CoreSchemaRelationshipTypeName::TargetCollectionType.as_rel_name(),
        vec![definition.target_collection_type]
    )?;


    if let Some(inverse) = definition.has_inverse {
        relationship_type_ref
            .add_related_holons(
                context,
                CoreSchemaRelationshipTypeName::HasInverse.as_rel_name(),
                vec![inverse])?
    };

    Ok(relationship_type_ref)

}
