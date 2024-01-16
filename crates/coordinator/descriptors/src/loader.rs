use holons::helpers::define_local_target;
/// This file creates an In-Memory Schema Holon and populates it with
/// all of the holons comprising the MAP Core L0 Schema
/// It does not depend on any Conductor functions.



use crate::descriptor_types::Schema;



// pub fn load_core_schema() -> Schema {
//
//     let mut schema = Schema::new(
//         "MAP L0 Core Schema".to_string(),
//         "The foundational MAP type descriptors for the L0 layer of the MAP Schema".to_string()
//     );
//     let schema_target = define_local_target(&schema.into_holon());
//
//
//
//
//
//     schema
//
//
// }
