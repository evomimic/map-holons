/// This file loads the MAP Type System as a Vector of In-Memory Holons within a Test Fixture
/// Since it is executing within a fixture, it cannot access any Conductor functions


use holons::holon_types::{Holon};
use shared_types_holon::BaseType;

use shared_types_holon::holon_node::{BaseValue};
use crate::shared_test::descriptors::enum_descriptor::{define_enum_descriptor, define_enum_variant_descriptor};
use crate::shared_test::descriptors::holon_descriptor::define_holon_descriptor;
use crate::shared_test::descriptors::holon_space::{define_holon_space_descriptor, new_holon_space};
use crate::shared_test::descriptors::relationship_descriptor::define_relationship_descriptor;
use crate::shared_test::descriptors::schema::define_schema;
use crate::shared_test::descriptors::value_descriptor::{define_boolean_descriptor, define_integer_descriptor, define_string_descriptor};


pub fn load_type_system() -> Vec<Holon> {

    // ----------------  USE THE INTERNAL HOLONS API TO ADD SOME PROPERTIES -----------------------
    let mut type_system: Vec<Holon> = Vec::new();
    type_system.push(define_schema());

    //type_system.push(define_holon_descriptor());
    type_system.push(define_holon_space_descriptor());
    type_system.push(new_holon_space());

    type_system.push(define_relationship_descriptor());


    type_system.push(define_string_descriptor());
    type_system.push(define_integer_descriptor());
    type_system.push(define_boolean_descriptor());

    type_system.push(define_enum_descriptor());
    type_system.push(define_enum_variant_descriptor(BaseType::String));



    type_system


}
