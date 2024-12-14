pub mod boolean_descriptor;
pub mod collection_descriptor;
pub mod descriptor_types;
pub mod enum_descriptor;
pub mod enum_variant_descriptor;
mod helper;
pub mod holon_descriptor;
pub mod integer_descriptor;
pub mod meta_type_descriptor;
pub mod property_descriptor;
pub mod relationship_descriptor;
pub mod schema;
mod semantic_version;
pub mod string_descriptor;
pub mod type_descriptor;
pub mod value_type_descriptor;

// pub fn add(left: usize, right: usize) -> usize {
//     left + right
// }
//
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
