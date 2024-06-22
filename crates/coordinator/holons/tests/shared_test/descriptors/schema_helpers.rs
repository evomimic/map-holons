use holons::holon_reference::{HolonReference, LocalHolonReference};
use holons::holon_reference::HolonReference::Local;
use holons::holon_types::Holon;
use holons::relationship::HolonCollection;
/// This helper function returns a HolonCollection for the specified holon
/// It assumes the holon is Local
fn get_local_target(holon:Holon) ->HolonCollection {
    // Define a HolonCollection for the provided Holon
    let mut local_reference = LocalHolonReference::from_holon(holon);
    let reference : HolonReference = Local(local_reference);

    let target = HolonCollection::One(reference);
    target
}