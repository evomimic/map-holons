use core_types::{BaseValue, ValidationSubjectPath};
use holons_core::ValueDescriptor;

/// A populated native value and its selected value-type descriptor.
#[derive(Clone, Copy)]
pub struct ValueValidationSubject<'a> {
    /// Descriptor governing this value independently of its containing property.
    pub descriptor: &'a ValueDescriptor,
    /// Native representation being assessed.
    pub value: &'a BaseValue,
    /// Immutable diagnostic provenance, with no property or holon handle.
    pub path: &'a ValidationSubjectPath,
}

/// Read-only inputs prepared through the Commit assessment's prospective reader.
/// Handlers consume these facts without navigating to containing subjects.
pub enum PreparedRuleSubject {
    /// Populated names absent from the selected effective contract.
    Holon { undescribed_properties: Vec<core_types::PropertyName> },
    /// A missing property whose minimum and requiredness have already been resolved.
    Property { missing_required: bool, name: String, descriptor_identity: String },
    /// The selected value type's native representation and the populated value kind.
    Value {
        expected: holons_core::ValueDescriptorKind,
        actual: core_types::BaseValueKind,
        descriptor_identity: String,
    },
}
