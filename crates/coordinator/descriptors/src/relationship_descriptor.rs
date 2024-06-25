use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::{BaseType, PropertyName};
use shared_types_holon::value_types::{BaseValue, MapBoolean, MapInteger, MapString};

use crate::descriptor_types::DeletionSemantic;
use crate::type_descriptor::{define_type_descriptor, TypeDefinitionHeader};

pub struct RelationshipDefinition {
    pub header:TypeDefinitionHeader,
    pub relationship_name: MapString,
    pub source_owns_relationship: MapBoolean,
    pub min_target_cardinality: MapInteger,
    pub max_target_cardinality: MapInteger,
    pub load_links_immediate: MapBoolean,
    pub load_holons_immediate: MapBoolean,
    pub deletion_semantic: DeletionSemantic,
    pub affinity: MapInteger,
    pub source_for: HolonReference,
    pub target_of: Option<HolonReference>,
    pub has_inverse: Option<HolonReference>,
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
    definition: RelationshipDefinition,
) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let source_type_name:String = "source_type_name".to_string(); // TODO: = source_for.get_property_value(context, "type_name")?;
    let target_type_name: String = "target_type_name".to_string(); // TODO: = target_of.get_property_value(context, "type_name");
    let type_name= MapString(format!("{}-{}->{}", source_type_name, definition.relationship_name.0,target_type_name.to_string()));
    let mut staged_reference = define_type_descriptor(
        context,
        schema,
        BaseType::Relationship,
        definition.header,
    )?;





    // Add its properties
    let mut mut_holon = staged_reference.get_mut_holon(context)?;

    mut_holon
        .borrow_mut()
        .with_property_value(
            PropertyName(MapString("source_owns_relationship".to_string())),
            BaseValue::BooleanValue(definition.source_owns_relationship),
        )?
        .with_property_value(
            PropertyName(MapString("min_target_cardinality".to_string())),
            BaseValue::IntegerValue(definition.min_target_cardinality),
        )?
        .with_property_value(
            PropertyName(MapString("max_target_cardinality".to_string())),
            BaseValue::IntegerValue(definition.max_target_cardinality),
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
        )?
        .with_property_value(
            PropertyName(MapString("affinity".to_string())),
            BaseValue::IntegerValue(definition.affinity),
        )?;

    // Add its relationships
    staged_reference
        .add_related_holons(
            context,
            RelationshipName(MapString("SOURCE_FOR".to_string())),
            vec![definition.source_for])?;

    if let Some(descriptor_ref) = definition.target_of {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("TARGET_OF".to_string())),
                vec![descriptor_ref])?
    };

    if let Some(inverse) = definition.has_inverse {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("HAS_INVERSE".to_string())),
                vec![inverse])?
    };



    Ok(staged_reference)

}