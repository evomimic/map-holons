use core_types::{BaseValue, ValidationSubjectPath};
use holons_core::{HolonReference, PropertyDescriptor, ValueDescriptor};

/// A holon whose governing descriptor is resolved at bootstrap navigation.
///
/// Descriptor discovery belongs to `validate_holon` so missing and ambiguous
/// `DescribedBy` can become semantic findings instead of construction errors.
#[derive(Clone, Copy)]
pub struct HolonValidationSubject<'a> {
    /// Bound runtime handle; transient, staged, and saved subjects are supported.
    pub holon: &'a HolonReference,
}

/// One effective property, including the absence of a populated value.
#[derive(Clone, Copy)]
pub struct PropertyValidationSubject<'a> {
    /// The complete-contract member governing this property.
    pub descriptor: &'a PropertyDescriptor,
    /// Absent properties must still reach required-property validation.
    pub value: Option<&'a BaseValue>,
    /// Property diagnostic path, with no parent navigation capability.
    pub path: &'a ValidationSubjectPath,
}

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
