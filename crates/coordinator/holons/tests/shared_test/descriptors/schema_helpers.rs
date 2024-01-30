use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_reference::HolonReference::Local;
use holons::holon_types::Holon;
use holons::relationship::RelationshipTarget;
/// This helper function returns a RelationshipTarget for the specified holon
/// It assumes the holon is Local
fn get_local_target(holon:Holon) ->RelationshipTarget {
    // Define a RelationshipTarget for the provided Holon
    let mut local_reference = LocalHolonReference::from_holon(holon);
    let reference : HolonReference = Local(local_reference);

    let target = RelationshipTarget::One(reference);
    target
}