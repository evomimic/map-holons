// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
pub mod core_schema_types;
pub mod loader;
pub mod property_type_loader;
pub mod value_type_loader;

pub mod boolean_value_type_loader;
pub mod collection_type_loader;
pub mod enum_type_loader;
pub mod enum_variant_loader;
pub mod holon_type_loader;
pub mod integer_value_type_loader;
pub mod meta_type_loader;
pub mod relationship_type_loader;
pub mod string_value_type_loader;
