use hdi::prelude::info;
// use holons::smart_reference::SmartReference;
use crate::collection_type_loader::CollectionTypeSpec;
use crate::core_schema_types::SchemaNamesTrait;
use crate::holon_type_loader::CoreHolonTypeName;
use descriptors::{
    collection_descriptor::CollectionSemantic,
    descriptor_types::DeletionSemantic,
    holon_descriptor::{define_holon_type, HolonTypeDefinition},
    type_descriptor::TypeDescriptorDefinition,
};
use holons_core::{HolonReference, HolonsContextBehavior, RelationshipName, StagedReference};
use base_types::{MapBoolean, MapString};
use core_types::HolonError;
use inflector::cases::screamingsnakecase::to_screaming_snake_case;
use strum_macros::EnumIter;

#[derive(Debug, Clone, Default, EnumIter)]
pub enum CoreRelationshipTypeName {
    CoreSchema,
    CoreSchemaFor,
    CollectionFor,
    Components,
    ComponentOf,
    // Dances,
    // DanceOf,
    DescribedBy,
    // ForCollectionType,
    HasInverse,
    HasSubtype,
    Instances,
    InverseOf,
    IsA,
    #[default]
    OwnedBy,
    Owns,
    Predecessor,
    Properties,
    PropertyOf,
    SourceFor,
    SourceHolonType,
    Successor,
    TargetCollectionType,
    TargetHolonType,
    TargetOfCollectionType,
    ValueType,
    ValueTypeFor,
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
    fn load_core_type(
        &self,
        context: &dyn HolonsContextBehavior,
        schema: &HolonReference,
    ) -> Result<StagedReference, HolonError> {
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
        panic!(
            "This trait function is not intended to be used for this type. \
        The 'description' for this type is explicitly defined in get_variant_loader()"
        )
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
            CoreSchemaFor => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("Specifies the HolonSpace(s) for which this Schema is used to create instances of L0 types.")
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::HolonSpaceType,
                },
                has_inverse: Some(CoreSchema),
            },
            CoreSchema => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("Specifies the (single) Core Schema used to create instances of L0 types within this HolonSpace.")
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::SchemaType,
                },
                has_inverse: None, // Should be None because `source_owns_relationship` is `false`

            },
            CollectionFor => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} specifies the RelationshipType for which this collection holds \
                    holons.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::RelationshipType,
                },
                has_inverse: None, // Should be None because `source_owns_relationship` is `false`

            },
            Components => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} can be queried to get all of type descriptors \
                    provided by this Schema.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Block,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::TypeDescriptor,
                },
                has_inverse: Some(ComponentOf),

            },
            ComponentOf => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} can be queried to get the Schema for which this type descriptor \
                    is a component.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::SchemaType,
                },
                has_inverse: Some(Components),

            },
            DescribedBy => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("{} specifies the Type of this Holon and describes its properties, \
                    relationships and dances ",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::OptionalInstance,
                    holon_type: CoreHolonTypeName::HolonType
                },
                has_inverse: Some(Instances),

            },
            HasInverse => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("{} specifies the Relationship that is the inverse of this relationship. \
                    Only relationships that are owned by their source holon type should be the \
                    source for HAS_INVERSE. ",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Cascade,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec {
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::RelationshipType,
                },
                has_inverse: Some(InverseOf),
            },
            HasSubtype => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("{} specifies the Subtypes of this Type. Subtypes inherit the properties, \
                     relationships and dances of their Supertype and may define additional \
                     properties, relationships and dances. Even what a subtype is not the \
                     SOURCE_FOR additional relationships, it may be defined to allow it to be a \
                     type-specific TARGET_OF a HolonCollection.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Block,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec {
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::HolonType
                },
                has_inverse: Some(IsA),
            },


            Instances => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("{} can be queried to get all of the instances of this Holon Type.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Block,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::HolonType, // TODO: this should really by Holon, Not HolonType
                },
                has_inverse: Some(DescribedBy),

            },
            IsA => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("Specifies the supertype of {}.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Cascade,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::TypeDescriptor,
                },
                has_inverse: Some(HasSubtype),

            },
            InverseOf => RelationshipTypeLoader {
                descriptor_name,
                description: MapString(
                    format!("{} can be queried to get the (single) relationship type that is the \
                    inverse of this relationship type. It is not owned by its source holon type, \
                    so it should not be populated directly. Instead links for this relationship \
                    are only established during commit of the HAS_INVERSE relationship.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::RelationshipType,
                },
                has_inverse: None, // None because InverseOf is not owned by its source holon type

            },
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
                    holon_type: CoreHolonTypeName::TypeDescriptor,
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
            Predecessor => RelationshipTypeLoader {
                descriptor_name,
                description: MapString("Specifies the previous version of this Holon".to_string()),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::OptionalInstance,
                    holon_type: CoreHolonTypeName::HolonType,
                },
                has_inverse: Some(Successor),
            },
            Properties => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} can be queried to get all of PropertyTypes for a HolonType.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Cascade,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::PropertyType,
                },
                has_inverse: Some(PropertyOf),

            },


            PropertyOf => RelationshipTypeLoader {
                    descriptor_name,
                    description: MapString(
                        format!("Specifies the Holon Type this Property Type is a {}",
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
                        holon_type: CoreHolonTypeName::HolonType
                    },
                    has_inverse: Some(Properties),

            },

            SourceFor =>RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} can be queried to retrieve all of the Holons for which this Holon is \
                    the source.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Cascade,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::RelationshipType,
                },
                has_inverse: Some(SourceHolonType),

            },
            SourceHolonType => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} specifies the HolonType that is the source for this relationship.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::HolonType,
                },
                has_inverse: None,

            },
            Successor => RelationshipTypeLoader {
                descriptor_name,
                description: MapString("Specifies the a later version of this Holon".to_string()),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Block,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::HolonType,
                },
                has_inverse: Some(Predecessor),
            },
            TargetCollectionType => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} specifies the HolonCollectionType that holds the holons that are the \
                    target of this relationship.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::HolonCollectionType,
                },
                has_inverse: None,

            },
            TargetHolonType => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} specifies the HolonType of the holons that make up this \
                    HolonCollection.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(true),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(true),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::HolonType,
                },
                has_inverse: Some(CollectionFor),

            },
            TargetOfCollectionType => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} specifies the HolonType of the holons that make up this \
                    HolonCollection.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::HolonCollectionType,
                },
                has_inverse: None,

            },
            ValueType => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} specifies the ValueType of this property.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::SingleInstance,
                    holon_type: CoreHolonTypeName::ValueType,
                },
                has_inverse: None,

            },
            ValueTypeFor => RelationshipTypeLoader {
                descriptor_name,
                description : MapString(
                    format!("{} can be queried to retrieve the PropertyTypes for which this is the\
                     ValueType.",
                            relationship_type_name.0.clone())
                ),
                label,
                described_by: None,
                owned_by: None,
                relationship_type_name,
                source_owns_relationship: MapBoolean(false),
                deletion_semantic: DeletionSemantic::Allow,
                load_links_immediate: MapBoolean(false),
                target_collection_type: CollectionTypeSpec{
                    semantic: CollectionSemantic::Set,
                    holon_type: CoreHolonTypeName::PropertyType,
                },
                has_inverse: None,

            },
        }
    }
}

/// This function handles the aspects of staging a new relationship type definition that are common
/// to all relationship types. It assumes the type-specific parameters have been set in the loader.
pub fn load_relationship_type_definition(
    context: &dyn HolonsContextBehavior,
    schema: &HolonReference,
    loader: RelationshipTypeLoader,
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

    info!("Preparing to stage descriptor for {:#?}", loader.relationship_type_name.0.clone());
    let staged_ref = define_holon_type(context, schema, definition)?;

    context
        .get_space_manager()
        .get_transient_state()
        .borrow_mut()
        .add_references(context, vec![HolonReference::Staged(staged_ref.clone())])?;

    Ok(staged_ref)
}
