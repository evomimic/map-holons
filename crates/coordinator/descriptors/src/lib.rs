pub mod type_descriptor;
pub mod string_descriptor;
mod semantic_version;
pub mod schema;
pub mod loader;
pub mod integer_descriptor;
pub mod descriptor_types;
pub mod holon_descriptor;
pub mod relationship_descriptor;

use hdk::prelude::*;
use holons_integrity::*;


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
