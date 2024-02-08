use holons::holon_types::Holon;
use shared_types_holon::HolonId;

/// Each DescriptorTestCase specifies a list of steps.
/// Each step corresponds to some operation -- either a Create, Update or Delete
/// For `Create`, the associated TypeDescriptor defines the descriptor to create (and expected result)
/// For `Update`, the associated TypeDescriptor defines the descriptor after update
/// For `Delete , the associated HolonId is the ActionHash of the descriptor to delete
#[derive(Clone, Debug)]
pub struct DescriptorTestCase {
    pub steps : Vec<DescriptorTestStep>,
}
#[derive(Clone, Debug)]
pub enum DescriptorTestStep {
    Create(Holon), // Associated data is expected TypeDescriptor
    Update(Holon), // Associated data is expected TypeDescriptor after update
    Delete(HolonId), // Associated data is id of TypeDescriptor to delete
}
