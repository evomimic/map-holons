pub const TEST_CLIENT_PREFIX: &str = "TEST CLIENT: ";

/// Marker property stamped on the key-only stub snapshot minted by the
/// saved-holon lookup step. Stubs stand in for holons saved outside the
/// fixture's ledger (e.g. by a schema load), so saved-content comparison
/// matches them by key only and never recurses into their graph.
pub const SAVED_LOOKUP_STUB_MARKER: &str = "__saved_lookup_stub__";

// These constants allow consistency between the helper function and its callers
pub const BOOK_KEY: &str =
    "Emerging World: The Evolution of Consciousness and the Future of Humanity";
pub const PERSON_1_KEY: &str = "Roger Briggs";
pub const PERSON_2_KEY: &str = "George Smith";
pub const PUBLISHER_KEY: &str = "Publishing Company";
pub const BOOK_TO_PERSON_RELATIONSHIP: &str = "AuthoredBy";
pub const PUBLISHED_BY: &str = "PublishedBy";
pub const EDITOR_FOR: &str = "EditorFor";
pub const ENSURE_DB_EMPTY: &str = "Ensuring DB is 'empty' (only contains initial LocalHolonSpace).";
pub const BOOK_DESCRIPTOR_KEY: &str = "Book.HolonType";
pub const PERSON_DESCRIPTOR_KEY: &str = "Person.HolonType";
pub const SCHEMA_TYPE_KEY: &str = "Schema.HolonType";
pub const HOLON_SPACE_TYPE_KEY: &str = "HolonSpace.HolonType";
pub const TRANSACTION_TYPE_KEY: &str = "Transaction.HolonType";
pub const HOLON_TYPE_KEY: &str = "HolonType";
pub const DELETION_SEMANTIC_KEY: &str = "DeletionSemantic";
pub const DELETION_SEMANTIC_ALLOW_KEY: &str = "DeletionSemantic.Allow";
pub const DELETION_SEMANTIC_BLOCK_KEY: &str = "DeletionSemantic.Block";
pub const DELETION_SEMANTIC_CASCADE_KEY: &str = "DeletionSemantic.Cascade";
pub const OPERATOR_CATEGORY_KEY: &str = "OperatorCategory";
pub const OPERATOR_CATEGORY_EQUALITY_KEY: &str = "OperatorCategory.Equality";
pub const OPERATOR_CATEGORY_ORDERING_KEY: &str = "OperatorCategory.Ordering";
pub const VARIANTS_RELATIONSHIP: &str = "Variants";
pub const CORE_INSTANCE_PROPERTIES_RELATIONSHIP_KEY: &str =
    "(TypeDescriptor.HolonType)-[InstanceProperties]->(PropertyType)";
pub const CORE_INSTANCE_PROPERTY_FOR_RELATIONSHIP_KEY: &str =
    "(PropertyType)-[InstancePropertyFor]->(TypeDescriptor.HolonType)";
pub const CORE_PREDECESSOR_RELATIONSHIP_KEY: &str = "(HolonType)-[Predecessor]->(HolonType)";
pub const CORE_INVERSE_OF_RELATIONSHIP_KEY: &str =
    "(InverseRelationshipType)-[InverseOf]->(DeclaredRelationshipType)";
pub const BOOK_TO_PERSON_RELATIONSHIP_KEY: &str =
    "(Book.HolonType)-[AuthoredBy]->(Person.HolonType)";
pub const PERSON_TO_BOOK_REL_INVERSE: &str = "Authors";
pub const PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY: &str =
    "(Person.HolonType)-[Authors]->(Book.HolonType)";
