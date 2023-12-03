/// This file loads the MAP Type System as a Vector of In-Memory Holons
// Bootstrap TypeHeader (as part of Holon's PropertyMap
/// This file augments a provided holon's PropertyMap with TypeHeader properties


use holons::holon_types::{Holon};

use shared_types_holon::holon_node::{PropertyValue};
use crate::shared_test::descriptors::enum_descriptor::{define_enum_descriptor, define_enum_variant};


pub fn load_type_system() -> Vec<Holon> {

    // ----------------  USE THE INTERNAL HOLONS API TO ADD SOME PROPERTIES -----------------------
    let mut type_system: Vec<Holon> = Vec::new();

    type_system.push(define_enum_descriptor());
    type_system.push(define_enum_variant());



    type_system


}
