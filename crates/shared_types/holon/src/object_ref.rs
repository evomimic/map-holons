use hdi::prelude::*;
use shared_types_descriptor::holon_descriptor::HolonDescriptor;

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct HolonReference {
    id: ActionHash,
    // namespace: NamespaceId,
    entry_Id: EntryHash,
    type_name: String, // type_name of the Holon Type being referenced
    referenced_type: HolonDescriptor,
    version_update_policy: VersionUpdatePolicy,
}

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub enum VersionUpdatePolicy {
    AlwaysManual,
    AutoUpdatePatch,
    AutoUpdateWarning,
    AutoUpdateAll, // NOT RECOMMENDED
}