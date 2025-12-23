pub const TEST_CLIENT_PREFIX: &str = "TEST CLIENT: ";

// These constants allow consistency between the helper function and its callers
pub const BOOK_KEY: &str =
    "Emerging World: The Evolution of Consciousness and the Future of Humanity";
pub const PERSON_1_KEY: &str = "Roger Briggs";
pub const PERSON_2_KEY: &str = "George Smith";
pub const PUBLISHER_KEY: &str = "Publishing Company";
pub const BOOK_TO_PERSON_RELATIONSHIP: &str = "AUTHORED_BY";
pub const PUBLISHED_BY: &str = "PUBLISHED_BY";
pub const EDITOR_FOR: &str = "EDITOR_FOR";
pub const BOOK_DESCRIPTOR_KEY: &str = "Book.HolonType";
pub const PERSON_DESCRIPTOR_KEY: &str = "Person.HolonType";
pub const BOOK_TO_PERSON_RELATIONSHIP_KEY: &str =
    "(Book.HolonType)-[AuthoredBy]->(Person.HolonType)";
pub const PERSON_TO_BOOK_REL_INVERSE: &str = "Authors";
pub const PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY: &str =
    "(Person.HolonType)-[Authors]->(Book.HolonType)";
