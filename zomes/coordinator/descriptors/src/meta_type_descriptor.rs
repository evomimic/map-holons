use crate::property_descriptor::PropertyTypeDefinition;
use crate::relationship_descriptor::RelationshipTypeDefinition;
use crate::type_descriptor::TypeDescriptorDefinition;

pub struct MetaTypeDefinition {
    pub header: TypeDescriptorDefinition,
    pub properties: Vec<PropertyTypeDefinition>,
    pub relationships: Vec<RelationshipTypeDefinition>,
}
//
// /// This function defines the MetaTypeDescriptor that specifies the properties and relationships
// /// shared by all TypeDescriptors
// pub fn define_meta_type(
//     context: &dyn HolonsContextBehavior,
//     schema: &HolonReference,
//     definition: MetaTypeDefinition,
// ) -> Result<StagedReference, HolonError> {
//
//     // Lookup the required ValueType definitions from the DanceState
//     let string_type_ref = get_core_type_ref(context, CoreSchemaName::MapStringType)?;
//     let semantic_version_type_ref=get_core_type_ref(context, CoreSchemaName::SemanticVersionType)?;
//     let boolean_type_ref=get_core_type_ref(context, CoreSchemaName::MapBooleanType)?;
//
//     // ----------------  STAGE A NEW META TYPE DESCRIPTOR -------------------------------
//     let meta_descriptor_ref = define_type_descriptor(
//         context,
//         schema,
//         TypeKind::Holon,
//         definition.header,
//     )?;
//
//     // build the new type
//     let mut meta_type = Holon::new();
//
//     // Add its properties
//     meta_type
//         .with_property_value(
//             TypeName.as_property_name
//         )
//
//     let descriptor = define_type_descriptor(
//         context,
//         schema,
//         TypeKind::Holon,
//         header,
//     )?;
//
//     // Define and add properties to MetaType
//
//     // Property: key
//     let key_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("KeyDescriptor".to_string())),
//         type_name: MapString("Key".to_string()),
//         description: MapString("This property defines a unique key for this type that can be \
//         used when the type does not have a descriptor that specifies the composition of its \
//         key.".to_string()),
//         label: MapString("key".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None, // this should MetaPropertyType
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let key_property_definition = PropertyTypeDefinition {
//         header: key_property_header,
//         is_required: MapBoolean(true),
//         value_type: string_type_ref.clone(),
//     };
//
//     let key_property_staged = define_property_type(
//         context,
//         schema,
//         key_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(key_property_staged)],
//     )?;
//
//     // Property: type_name
//     let type_name_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("type_nameDescriptor".to_string())),
//         type_name: MapString("type_name".to_string()),
//         description: MapString("Property for type_name".to_string()),
//         label: MapString("type_name".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let type_name_property_definition = PropertyTypeDefinition {
//         header: type_name_property_header,
//         is_required: MapBoolean(true),
//         value_type: string_type_ref.clone(),
//     };
//
//     let type_name_property_staged = define_property_type(
//         context,
//         schema,
//         type_name_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(type_name_property_staged)],
//     )?;
//
//     // Property: descriptor_name
//     let descriptor_name_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("descriptor_nameDescriptor".to_string())),
//         type_name: MapString("descriptor_name".to_string()),
//         description: MapString("Property for descriptor_name".to_string()),
//         label: MapString("descriptor_name".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let descriptor_name_property_definition = PropertyTypeDefinition {
//         header: descriptor_name_property_header,
//
//         value_type: string_type_ref.clone(),
//     };
//
//     let descriptor_name_property_staged = define_property_type(
//         context,
//         schema,
//         descriptor_name_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(descriptor_name_property_staged)],
//     )?;
//
//     // Property: description
//     let description_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("descriptionDescriptor".to_string())),
//         description: MapString("Property for description".to_string()),
//         label: MapString("description".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let description_property_definition = PropertyTypeDefinition {
//         header: description_property_header,
//         is_required: MapBoolean(true),
//         value_type: string_type_ref.clone(),
//     };
//
//     let description_property_staged = define_property_type(
//         context,
//         schema,
//         description_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(description_property_staged)],
//     )?;
//
//     // Property: label
//     let label_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("labelDescriptor".to_string())),
//         type_name: MapString("label".to_string()),
//         description: MapString("Property for label".to_string()),
//         label: MapString("label".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let label_property_definition = PropertyTypeDefinition {
//         header: label_property_header,
//         is_required: MapBoolean(true),
//         value_type: string_type_ref.clone(),
//     };
//
//     let label_property_staged = define_property_type(
//         context,
//         schema,
//         label_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(label_property_staged)],
//     )?;
//
//     // Property: base_type
//     let base_type_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("base_typeDescriptor".to_string())),
//         type_name: MapString("base_type".to_string()),
//         description: MapString("Property for base_type".to_string()),
//         label: MapString("base_type".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let base_type_property_definition = PropertyTypeDefinition {
//         header: base_type_property_header,
//         is_required: MapBoolean(true),
//         value_type: boolean_type_ref.clone(),
//     };
//
//     let base_type_property_staged = define_property_type(
//         context,
//         schema,
//         base_type_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(base_type_property_staged)],
//     )?;
//
//     // Property: is_dependent
//     let is_dependent_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("is_dependentDescriptor".to_string())),
//         type_name: MapString("is_dependent".to_string()),
//         description: MapString("Property for is_dependent".to_string()),
//         label: MapString("is_dependent".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let is_dependent_property_definition = PropertyTypeDefinition {
//         header: is_dependent_property_header,
//         is_required: MapBoolean(true),
//         value_type: boolean_type_ref.clone(),
//     };
//
//     let is_dependent_property_staged = define_property_type(
//         context,
//         schema,
//         is_dependent_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(is_dependent_property_staged)],
//     )?;
//
//     // Property: is_value_descriptor
//     let is_value_descriptor_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("is_value_descriptorDescriptor".to_string())),
//         type_name: MapString("is_value_descriptor".to_string()),
//         description: MapString("Property for is_value_descriptor".to_string()),
//         label: MapString("is_value_descriptor".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let is_value_descriptor_property_definition = PropertyTypeDefinition {
//         header: is_value_descriptor_property_header,
//         is_required: MapBoolean(true),
//         value_type: boolean_type_ref.clone(),
//     };
//
//     let is_value_descriptor_property_staged = define_property_type(
//         context,
//         schema,
//         is_value_descriptor_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(is_value_descriptor_property_staged)],
//     )?;
//
//     // Property: version
//     let version_property_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("versionDescriptor".to_string())),
//         type_name: MapString("version".to_string()),
//         description: MapString("Property for version".to_string()),
//         label: MapString("version".to_string()),
//         is_dependent: MapBoolean(true),
//         is_value_type: MapBoolean(true),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let version_property_definition = PropertyTypeDefinition {
//         header: version_property_header,
//         is_required: MapBoolean(true),
//         value_type: semantic_version_type_ref.clone(),
//     };
//
//     let version_property_staged = define_property_type(
//         context,
//         schema,
//         version_property_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_PROPERTIES".to_string())),
//         vec![HolonReference::Staged(version_property_staged)],
//     )?;
//
//     // Define and add relationships to MetaType
//
//     // Relationship: DESCRIBED_BY
//     // This relationship is from a Type to a Type. I think this may force us to go back to
//     // Actually having a TypeDescriptor supertype (instead of just a shared TypeDescriptorHeader
//     //
//     let described_by_relationship_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("DESCRIBED_BYDescriptor".to_string())),
//         type_name: MapString("DESCRIBED_BY".to_string()),
//         description: MapString("Relationship for DESCRIBED_BY".to_string()),
//         label: MapString("DESCRIBED_BY".to_string()),
//         is_dependent: MapBoolean(false),
//         is_value_type: MapBoolean(false),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let described_by_relationship_definition = RelationshipTypeDefinition {
//         header: described_by_relationship_header,
//         relationship_name: MapString("DESCRIBED_BY".to_string()),
//         source_owns_relationship: MapBoolean(true),
//         min_target_cardinality: MapInteger(0),
//         max_target_cardinality: MapInteger(1),
//         load_links_immediate: MapBoolean(false),
//         load_holons_immediate: MapBoolean(false),
//         deletion_semantic: DeletionSemantic::Cascade,
//         affinity: MapInteger(0),
//         source_for: HolonReference::Staged(StagedReference::default()), // Replace with actual source reference
//         target_of: None,
//         has_inverse: None,
//     };
//
//     let described_by_relationship_staged = define_relationship_type(
//         context,
//         schema,
//         described_by_relationship_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_RELATIONSHIPS".to_string())),
//         vec![HolonReference::Staged(described_by_relationship_staged)],
//     )?;
//
//     // Relationship: IS_SUBTYPE_OF
//     let is_subtype_of_relationship_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("IS_SUBTYPE_OFDescriptor".to_string())),
//         type_name: MapString("IS_SUBTYPE_OF".to_string()),
//         description: MapString("Relationship for IS_SUBTYPE_OF".to_string()),
//         label: MapString("IS_SUBTYPE_OF".to_string()),
//         is_dependent: MapBoolean(false),
//         is_value_type: MapBoolean(false),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let is_subtype_of_relationship_definition = RelationshipTypeDefinition {
//         header: is_subtype_of_relationship_header,
//         relationship_name: MapString("IS_SUBTYPE_OF".to_string()),
//         source_owns_relationship: MapBoolean(true),
//         min_target_cardinality: MapInteger(0),
//         max_target_cardinality: MapInteger(1),
//         load_links_immediate: MapBoolean(false),
//         load_holons_immediate: MapBoolean(false),
//         deletion_semantic: DeletionSemantic::Cascade,
//         affinity: MapInteger(0),
//         source_for: HolonReference::Staged(StagedReference::default()), // Replace with actual source reference
//         target_of: None,
//         has_inverse: None,
//     };
//
//     let is_subtype_of_relationship_staged = define_relationship_type(
//         context,
//         schema,
//         is_subtype_of_relationship_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_RELATIONSHIPS".to_string())),
//         vec![HolonReference::Staged(is_subtype_of_relationship_staged)],
//     )?;
//
//     // Relationship: OwnedBy
//     let owned_by_relationship_header = TypeDescriptorDefinition {
//         descriptor_name: Some(MapString("OWNED_BYDescriptor".to_string())),
//         type_name: MapString("OwnedBy".to_string()),
//         description: MapString("Relationship for OwnedBy".to_string()),
//         label: MapString("OwnedBy".to_string()),
//         is_dependent: MapBoolean(false),
//         is_value_type: MapBoolean(false),
//         described_by: None,
//         is_subtype_of: None,
//         owned_by: None,
//     };
//
//     let owned_by_relationship_definition = RelationshipTypeDefinition {
//         header: owned_by_relationship_header,
//         relationship_name: MapString("OwnedBy".to_string()),
//         source_owns_relationship: MapBoolean(true),
//         min_target_cardinality: MapInteger(0),
//         max_target_cardinality: MapInteger(1),
//         load_links_immediate: MapBoolean(false),
//         load_holons_immediate: MapBoolean(false),
//         deletion_semantic: DeletionSemantic::Cascade,
//         affinity: MapInteger(0),
//         source_for: HolonReference::Staged(StagedReference::default()), // Replace with actual source reference
//         target_of: None,
//         has_inverse: None,
//     };
//
//     let owned_by_relationship_staged = define_relationship_type(
//         context,
//         schema,
//         owned_by_relationship_definition,
//     )?;
//
//     descriptor.add_related_holons(
//         context,
//         RelationshipName(MapString("DESCRIPTOR_RELATIONSHIPS".to_string())),
//         vec![HolonReference::Staged(owned_by_relationship_staged)],
//     )?;
//
//     Ok(descriptor)
// }
