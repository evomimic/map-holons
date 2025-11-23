pub mod abandon_staged_changes_fixture;
pub mod delete_holon_fixture;
pub mod ergonomic_add_remove_properties_fixture;
pub mod ergonomic_add_remove_related_holons_fixture;
pub mod load_holons_fixture;
<<<<<<< HEAD
pub mod loader_client_fixture;
pub mod simple_add_remove_related_fixture;
=======
pub mod setup_book_and_authors_fixture;
pub mod simple_add_remove_related_holons_fixture;
>>>>>>> 655b3cb (ready to start initial testing..)
pub mod simple_create_holon_fixture;
pub mod stage_new_from_clone_fixture;
pub mod stage_new_version_fixture;

pub use abandon_staged_changes_fixture::*;
pub use delete_holon_fixture::*;
pub use ergonomic_add_remove_properties_fixture::*;
pub use ergonomic_add_remove_related_holons_fixture::*;
pub use load_holons_fixture::*;
pub use setup_book_and_authors_fixture::*;
pub use simple_add_remove_related_holons_fixture::*;
pub use simple_create_holon_fixture::*;
pub use stage_new_from_clone_fixture::*;
pub use stage_new_version_fixture::*;
