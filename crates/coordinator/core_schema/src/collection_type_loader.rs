use hdi::prelude::info;
use descriptors::collection_descriptor::{CollectionTypeDefinition, define_collection_type};
use descriptors::type_descriptor::TypeDescriptorDefinition;
use holons::context::HolonsContext;
use descriptors::collection_descriptor::CollectionSemantic;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::staged_reference::StagedReference;
use shared_types_holon::{MapBoolean, MapInteger, MapString};
use crate::core_schema_types::SchemaNamesTrait;
// use crate::core_schema_types::CoreSchemaTypeName::HolonType;
use crate::holon_type_loader::CoreHolonTypeName;


#[derive(Debug)]
pub struct CollectionTypeSpec {
    pub semantic: CollectionSemantic,
    pub holon_type: CoreHolonTypeName,
}

#[derive(Debug)]
struct CollectionTypeLoader {
    pub type_name: MapString,
    pub descriptor_name: MapString,
    pub description: MapString,
    pub label: MapString, // Human-readable name for this type
    pub described_by: Option<HolonReference>, // Type-DESCRIBED_BY->Type
    pub owned_by: Option<HolonReference>,
    pub is_ordered: MapBoolean,
    pub allows_duplicates: MapBoolean,
    pub min_cardinality: MapInteger,
    pub max_cardinality: MapInteger,
    pub target_holon_type: CoreHolonTypeName,

}

impl SchemaNamesTrait for CollectionTypeSpec {
    fn load_core_type(&self, context: &HolonsContext, schema: &HolonReference) -> Result<StagedReference, HolonError> {
        // Set the type specific variables for this type, then call the load_property_definition
        let loader = self.build_collection_type_loader()?;
        load_collection_type_definition(context, schema, loader)

    }
    /// This method returns the unique type_name for this collection type in "CamelCase"
    fn derive_type_name(&self) -> MapString {
        let holon_type_name = self.holon_type.derive_type_name();
        match self.semantic {
            CollectionSemantic::SingleInstance => {
                MapString(format!("{}Instance", holon_type_name.clone()))
            }
            CollectionSemantic::OptionalInstance => {
                MapString(format!("Optional{}Instance", holon_type_name.clone()))
            },
            CollectionSemantic::UniqueList => {
                MapString(format!("Unique{}List", holon_type_name.clone()))
            },
            CollectionSemantic::List => {
                MapString(format!("{}List", holon_type_name.clone()))
            },
            CollectionSemantic::Set => {
                MapString(format!("{}Set", holon_type_name.clone()))
            },
        }
    }

    /// This method returns the "descriptor_name" for this type in camel_case
    fn derive_descriptor_name(&self) -> MapString {
        // this implementation uses a simple naming rule of appending "_descriptor" to the type_name
        MapString(format!("{}Descriptor", self.derive_type_name().0.clone()))
    }
    /// This method returns the human-readable name for this property type
    fn derive_label(&self) -> MapString {
        self.derive_type_name().clone()
    }


    /// This method returns the human-readable description of this type
    fn derive_description(&self) -> MapString {
        let holon_type_name = self.holon_type.derive_type_name();
        match self.semantic {
           CollectionSemantic::SingleInstance => {
               MapString(format!("Exactly one instance of {}", holon_type_name.clone()))
           },
           CollectionSemantic::OptionalInstance => {
               MapString(format!("An optional instance of {}", holon_type_name.clone()))
           },
           CollectionSemantic::UniqueList => {
               MapString(format!("An ordered list of {} that CANNOT contain duplicates.",
                                 holon_type_name.clone()))
           },
           CollectionSemantic::List => {
               MapString(format!("An ordered list of {} that CAN contain duplicates.)",
                                 holon_type_name.clone()))
           },
           CollectionSemantic::Set => {
               MapString(format!("An unordered set of {} that CANNOT contain duplicates.)",
                                 holon_type_name.clone()))
           },
       }
    }
}

impl CollectionTypeSpec {
    fn build_collection_type_loader(&self)
                                    -> Result<CollectionTypeLoader, HolonError> {

        let type_name = self.derive_type_name();
        let descriptor_name = self.derive_descriptor_name();
        let description = self.derive_description();
        let label = self.derive_label();
        let target_holon_type = self.holon_type.clone();

        Ok(match self.semantic {
            CollectionSemantic::SingleInstance => CollectionTypeLoader {
                type_name,
                descriptor_name,
                description,
                label,
                described_by: None,
                owned_by: None,
                is_ordered: MapBoolean(true),
                allows_duplicates: MapBoolean(false),
                min_cardinality: MapInteger(1),
                max_cardinality: MapInteger(1),
                target_holon_type,
            },

            CollectionSemantic::OptionalInstance => CollectionTypeLoader {
                type_name,
                descriptor_name,
                description,
                label,
                described_by: None,
                owned_by: None,
                is_ordered: MapBoolean(true),
                allows_duplicates: MapBoolean(false),
                min_cardinality: MapInteger(0),
                max_cardinality: MapInteger(1),
                target_holon_type,
            },

            CollectionSemantic::UniqueList => CollectionTypeLoader {
                type_name,
                descriptor_name,
                description,
                label,
                described_by: None,
                owned_by: None,
                is_ordered: MapBoolean(true),
                allows_duplicates: MapBoolean(false),
                min_cardinality: MapInteger(0),
                max_cardinality: MapInteger(i32::MAX.into()),
                target_holon_type,
            },

            CollectionSemantic::List => CollectionTypeLoader {
                type_name,
                descriptor_name,
                description,
                label,
                described_by: None,
                owned_by: None,
                is_ordered: MapBoolean(true),
                allows_duplicates: MapBoolean(true),
                min_cardinality: MapInteger(0),
                max_cardinality: MapInteger(i32::MAX.into()),
                target_holon_type,
            },
            CollectionSemantic::Set => CollectionTypeLoader {
                type_name,
                descriptor_name,
                description,
                label,
                described_by: None,
                owned_by: None,
                is_ordered: MapBoolean(false),
                allows_duplicates: MapBoolean(false),
                min_cardinality: MapInteger(0),
                max_cardinality: MapInteger(i32::MAX.into()),
                target_holon_type,

            },
        })

    }
}



/// This function stages a new collection type definition and adds a reference to it to the dance_state
fn load_collection_type_definition(
    context: &HolonsContext,
    schema: &HolonReference,
    loader: CollectionTypeLoader,
) -> Result<StagedReference, HolonError> {

    let target_holon_type = loader
        .target_holon_type
        .lazy_get_core_type_definition(context, schema)?;

    let type_header = TypeDescriptorDefinition {
        descriptor_name: loader.descriptor_name,
        description: loader.description,
        label: loader.label,
        // TODO: add base_type: BaseType::EnumVariant
        is_dependent: MapBoolean(false),
        is_value_type: MapBoolean(false),
        described_by: loader.described_by,
        is_subtype_of: None,
        owned_by: loader.owned_by,
    };

    let definition = CollectionTypeDefinition {
        header: type_header,

        collection_type_name: Some(loader.type_name.clone()),
        is_ordered: loader.is_ordered,
        allows_duplicates: loader.allows_duplicates,
        min_cardinality: loader.min_cardinality,
        max_cardinality: loader.max_cardinality,
        target_holon_type,
    };

    info!("Preparing to stage descriptor for {:#?}",
        loader.type_name.clone());
    let staged_ref = define_collection_type(
        context,
        schema,
        definition,
    )?;

    context.add_reference_to_dance_state(HolonReference::Staged(staged_ref.clone()))
        .expect("Unable to add reference to dance_state");

    Ok(staged_ref)
}





