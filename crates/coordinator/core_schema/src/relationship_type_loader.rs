use hdi::prelude::info;
use inflector::cases::screamingsnakecase::to_screaming_snake_case;
// use inflector::cases::snakecase::to_snake_case;
use descriptors::descriptor_types::DeletionSemantic;
use descriptors::holon_descriptor::{define_holon_type, HolonTypeDefinition};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapString};
use crate::collection_type_loader::{CollectionSemantic, CollectionTypeSpec};
use crate::core_schema_types::{SchemaNamesTrait};
use crate::holon_type_loader::CoreHolonTypeName;
use crate::string_value_type_loader::CoreStringValueTypeName::RelationshipNameType;

#[derive(Debug)]
pub enum CoreRelationshipTypeName {
    OwnedBy,
    Owns,
    // DescribedBy,
    // Instances,
    // TypeDescriptor,
    //Type,
    // Components,
    // ComponentOf,
    // DescriptorProperties,
    // PropertyTypeFor,
    // DescriptorRelationships,
    // SourceType,
    // Dances,
    // DanceOf,
    // Properties,
    // TargetPropertyType,
    // TargetHolonType,
    // ForCollectionType,
    // TargetCollectionType,
    // CollectionFor,
    // HasInverse,
    // InverseOf
}
#[derive(Debug)]
pub struct RelationshipTypeLoader {
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub relationship_type_name: RelationshipName,
    pub source_owns_relationship: MapBoolean,
    pub deletion_semantic: DeletionSemantic,
    pub load_links_immediate: MapBoolean,
    pub target_collection_type: CollectionTypeSpec,
    pub has_inverse: Option<CoreRelationshipTypeName>,
}

impl SchemaNamesTrait for CoreRelationshipTypeName {
    fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = self.get_relationship_type_loader();
        load_relationship_type_definition(context, schema, loader)

    }
    /// This method returns the unique type_name for this property type in "snake_case"
    fn derive_type_name(&self) -> MapString {
        // SCREAMING_SNAKE_CASE matches OpenCypher standard for Relationship Names
        MapString(to_screaming_snake_case(&format!("{:?}", self)))
    }

    /// This method returns the "descriptor_name" for this type in camel_case
    fn derive_descriptor_name(&self) -> MapString {
        // this implementation uses a simple naming rule of appending "_DESCRIPTOR" to the type_name
        MapString(format!("{}_DESCRIPTOR", self.derive_type_name().0.clone()))
    }
    /// This method returns the human-readable name for this property type
    fn derive_label(&self) -> MapString {
        self.derive_type_name()
    }


    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        panic!("This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_variant_loader()")
    }
}
impl CoreRelationshipTypeName {
    /// This function returns the holon definition for a given holon type
    fn get_relationship_type_loader(&self) -> RelationshipTypeLoader {
        use CoreRelationshipTypeName::*;

        let relationship_type_name = RelationshipName(self.derive_type_name());
        let descriptor_name = self.derive_descriptor_name();
        let label = self.derive_label();

        match self {
            Owns => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("{} can be queried to get all of the Holons owned by this HolonSpace",
                            relationship_type_name.0.clone())),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Block,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::TypeHeader,
                },
                has_inverse: None, // Inverse should only be specified by relationship source owner
            },

            OwnedBy => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("Specifies the HolonSpace this Holon is {}",
                            relationship_type_name.0.clone())),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::HolonSpaceType
                },
                has_inverse: Some(Owns),
            },
            // DescribedBy => RelationshipTypeLoader {
            //     descriptor_name,
            //     description: MapString(
            //         format!("Describes the type of this Holon, including its properties, \
            //         relationships and dances ")
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(true),
            //     deletion_semantic: DeletionSemantic::Allow,
            //     load_links_immediate: MapBoolean(true),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::OptionalInstance,
            //         holon_type: CoreHolonTypeName::HolonType
            //     },
            //     has_inverse: Some(Instances),
            //
            // },
            // Instances => RelationshipTypeLoader {
            //     descriptor_name,
            //     description: MapString(
            //         format!("{} can be queried to get all of the instances of this Holon Type.",
            //                 relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(false),
            //     deletion_semantic: DeletionSemantic::Block,
            //     load_links_immediate: MapBoolean(false),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::Set,
            //         holon_type: CoreHolonTypeName::Holon,
            //     },
            //     has_inverse: None,
            //
            // },
            // TypeDescriptor => RelationshipTypeLoader {
            //     descriptor_name,
            //     description: MapString(
            //         format!("Specifies the defining characteristics of the {}.",
            //                 relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(true),
            //     deletion_semantic: DeletionSemantic::Cascade,
            //     load_links_immediate: MapBoolean(true),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::SingleInstance,
            //         holon_type: CoreHolonTypeName::TypeHeader,
            //     },
            //     has_inverse: Some(Type),
            //
            // },
            // // Type => {
            // //
            // // },
            // Components => RelationshipTypeLoader {
            //     descriptor_name,
            //     description : MapString(
            //         format!("{} can be queried to get all of type descriptors \
            //         provided by this Schema.",
            //         relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(false),
            //     deletion_semantic: DeletionSemantic::Block,
            //     load_links_immediate: MapBoolean(false),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::Set,
            //         holon_type: CoreHolonTypeName::TypeHeader,
            //     },
            //     has_inverse: Some(Type),
            //
            // },
            // // ComponentOf => {
            // //
            // // },
            // DescriptorProperties => RelationshipTypeLoader {
            //     descriptor_name,
            //     description : MapString(
            //         format!("{} can be queried to get the property descriptors of the property types \
            //         defined for this Map Type",
            //                 relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(true),
            //     deletion_semantic: DeletionSemantic::Cascade,
            //     load_links_immediate: MapBoolean(true),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::Set,
            //         holon_type: CoreHolonTypeName::PropertyType,
            //     },
            //     has_inverse: Some(PropertyTypeFor),
            //
            // },
            // PropertyTypeFor => {
            //     RelationshipTypeLoader {
            //         descriptor_name,
            //         description: MapString(
            //             format!("Specifies the HolonSpace this Holon is {}",
            //                     relationship_type_name.0.clone())),
            //         label,
            //         described_by: None,
            //         owned_by: None,
            //         relationship_type_name,
            //         source_owns_relationship: MapBoolean(true),
            //         deletion_semantic: DeletionSemantic::Allow,
            //         load_links_immediate: MapBoolean(true),
            //         target_collection_type: CollectionTypeSpec{
            //             semantic: CollectionSemantic::Set,
            //             holon_type: CoreHolonTypeName::HolonSpaceType
            //         },
            //         has_inverse: Some(Owns),
            //
            // },
            // DescriptorRelationships => RelationshipTypeLoader {
            //     descriptor_name,
            //     description : MapString(
            //         format!("{} can be queried to get all of type descriptors \
            //         provided by this Schema.",
            //                 relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(false),
            //     deletion_semantic: DeletionSemantic::Block,
            //     load_links_immediate: MapBoolean(false),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::Set,
            //         holon_type: CoreHolonTypeName::TypeHeader,
            //     },
            //     has_inverse: Some(Type),
            //
            // },
            // SourceType => {
            //
            // },
            // Dances => RelationshipTypeLoader {
            //     descriptor_name,
            //     description : MapString(
            //         format!("{} can be queried to get all of type descriptors \
            //         provided by this Schema.",
            //                 relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(false),
            //     deletion_semantic: DeletionSemantic::Block,
            //     load_links_immediate: MapBoolean(false),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::Set,
            //         holon_type: CoreHolonTypeName::TypeHeader,
            //     },
            //     has_inverse: Some(Type),
            // },
            // // DanceOf => {
            // //
            // // },
            // Properties => RelationshipTypeLoader {
            //     descriptor_name,
            //     description : MapString(
            //         format!("{} can be queried to get all of type descriptors \
            //         provided by this Schema.",
            //                 relationship_type_name.0.clone())
            //     ),
            //     label,
            //     described_by: None,
            //     owned_by: None,
            //     relationship_type_name,
            //     source_owns_relationship: MapBoolean(false),
            //     deletion_semantic: DeletionSemantic::Block,
            //     load_links_immediate: MapBoolean(false),
            //     target_collection_type: CollectionTypeSpec{
            //         semantic: CollectionSemantic::Set,
            //         holon_type: CoreHolonTypeName::TypeHeader,
            //     },
            //     has_inverse: Some(Type),
            //
            // },
            // TargetPropertyType => {
            //
            // },
            // TargetHolonType => {
            //
            // },
            // ForCollectionType => {
            //
            // },
            // TargetCollectionType => {
            //
            // },
            // CollectionFor => {
            //
            // },
            // HasInverse => {
            //
            // },
            // InverseOf => {
            //
            // },
        }
    }
}

/// This function handles the aspects of staging a new relationship type definition that are common
/// to all relationship types. It assumes the type-specific parameters have been set in the loader.
pub fn load_relationship_type_definition(
    context: &HolonsContext,
    schema: &HolonReference,
    loader: RelationshipTypeLoader,
) -> Result<StagedReference, HolonError> {
    let type_header = TypeDescriptorDefinition {
        descriptor_name: loader.descriptor_name,
        description: loader.description,
        label: loader.label,
        // TODO: add base_type: BaseType::EnumVariant
        is_dependent: MapBoolean(true),
        is_value_type: MapBoolean(false),
        described_by: loader.described_by,
        is_subtype_of: None,
        owned_by: loader.owned_by,
    };

    let definition = HolonTypeDefinition {
        header: type_header,
        type_name: loader.relationship_type_name.0.clone(),
        properties: vec![],
        key_properties: None,

    };
    // Add HolonReferences to the PropertyDescriptors for this holon type
    // for property in loader.properties {
    //     definition.properties.push(property.lazy_get_core_type_definition(
    //         context,
    //         schema
    //     )?);
    // }

    // Add HolonReferences to the Key PropertyDescriptors for this holon type
    // if let Some(key_properties) = loader.key_properties {
    //     // Initialize key_properties as an empty vector if it's None
    //     definition.key_properties = Some(vec![]);
    //
    //     for key_property in key_properties {
    //         // Safely unwrap definition.key_properties and push into the inner vector
    //         if let Some(ref mut key_props) = definition.key_properties {
    //             key_props.push(key_property.lazy_get_core_type_definition(
    //                 context,
    //                 schema
    //             )?);
    //         }
    //     }
    // }

    // TODO:  Lazy get source_for references to RelationshipDescriptors
    // TODO: Lazy get dance_request references to DanceDescriptors (Request & Response)

    info!("Preparing to stage descriptor for {:#?}",
        loader.relationship_type_name.0.clone());
    let staged_ref = define_holon_type(
        context,
        schema,
        definition,
    )?;

    context.add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}





