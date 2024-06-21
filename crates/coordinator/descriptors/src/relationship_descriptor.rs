use holons::context::HolonsContext;
use holons::holon_error::HolonError;

use crate::descriptor_types::{DeletionSemantic, RelationshipDescriptor};
use holons::holon_collection::HolonCollection;

use holons::staged_reference::StagedReference;
use shared_types_holon::{BaseType, PropertyName};
use shared_types_holon::value_types::{BaseValue, MapBoolean, MapInteger, MapString};

use crate::descriptor_types::DeletionSemantic;
use crate::type_descriptor::define_type_descriptor;

/// This function defines and stages (but does not persist) a new RelationshipDescriptor.
/// Values for each of the RelationshipDescriptor properties will be set based on supplied parameters.
///
/// *Naming Rules*:
///     `type_name` := <source_for.type_name>"-"<relationship_name>"->"<target_for.type_name>"
///     `descriptor_name`:= `<type_name>"Descriptor"`
///
/// The descriptor will have the following relationships populated:
/// * DESCRIBED_BY->TypeDescriptor (if supplied)
/// * COMPONENT_OF->Schema (supplied)
/// * VERSION->SemanticVersion (default)
/// * HAS_SUPERTYPE-> HolonType (if supplied)
/// * TARGET_HOLON_TYPE -> HolonType (if supplied)
///
///
pub fn define_relationship_type(
    context: &HolonsContext,
    schema: &HolonReference,
    relationship_name: MapString,
    description: MapString,
    label: MapString,
    is_subtype_of: Option<HolonReference>,
    described_by: Option<HolonReference>,
    owned_by: Option<HolonReference>,
    min_target_cardinality: MapInteger,
    max_target_cardinality: MapInteger,
    deletion_semantic: DeletionSemantic,
    affinity: MapInteger,
    _source_for: HolonCollection, // TODO: switch type to HolonReference
    _target_for: HolonCollection, // TODO: switch type to HolonReference
    has_supertype: Option<StagedReference>,
    described_by: Option<StagedReference>,
    _has_inverse: Option<StagedReference>,

) -> Result<StagedReference, HolonError> {
    // ----------------  GET A NEW TYPE DESCRIPTOR -------------------------------
    let type_name= MapString(format!("{}-{}->{}", "source_for_type_name".to_string(), relationship_name.0,"target_for_type_name".to_string()));
    let mut staged_reference = define_type_descriptor(
        context,
        schema,
        MapString(format!("{}{}", type_name.0, "Descriptor".to_string())),
        type_name,
        BaseType::Relationship,
        description,
        label,
        MapBoolean(false),
        MapBoolean(false),
        described_by,
        is_subtype_of,
        owned_by,
    )?;

    // Add its properties
    let mut mut_holon = staged_reference.get_mut_holon(context)?;

    mut_holon
        .borrow_mut()
        .with_property_value(
            PropertyName(MapString("min_target_cardinality".to_string())),
            BaseValue::IntegerValue(min_target_cardinality),
        )?
        .with_property_value(
            PropertyName(MapString("max_target_cardinality".to_string())),
            BaseValue::IntegerValue(max_target_cardinality),
        )?
        .with_property_value(
            PropertyName(MapString("deletion_semantic".to_string())),
            BaseValue::EnumValue(deletion_semantic.to_enum_variant()),
        )?
        .with_property_value(
            PropertyName(MapString("affinity".to_string())),
            BaseValue::IntegerValue(affinity),
        )?;

    // Add its relationships
    if let Some(descriptor_ref) = target_holon_type {
        staged_reference
            .add_related_holons(
                context,
                RelationshipName(MapString("TARGET_HOLON_TYPE".to_string())),
                vec![descriptor_ref])?
    };

    Ok(staged_reference)

}