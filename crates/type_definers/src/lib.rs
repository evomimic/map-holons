pub mod boolean_definer;
// pub mod collection_definer_deprecated;
pub mod descriptor_types_deprecated;
pub mod enum_definer;
pub mod enum_variant_definer;
pub mod holon_definer;
pub mod integer_definer;
pub mod meta_type_definer;
pub mod property_definer;
pub mod relationship_definer;
pub mod schema;
mod semantic_version;
pub mod string_definer;
pub mod type_header;
pub mod value_type_definer;

// pub fn add(left: u64, right: u64) -> u64 {
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
