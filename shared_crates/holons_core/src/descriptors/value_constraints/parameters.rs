use super::ScalarConstraintFamily;

/// One authored endpoint; comparison uses its inclusivity without adjusting the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound<T> {
    pub value: T,
    pub inclusive: bool,
}

/// Signed integer interval. An ordered interval need not contain an integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegerInterval {
    pub minimum: Option<Bound<i64>>,
    pub maximum: Option<Bound<i64>>,
}

/// Non-negative measure interval for length and item-count families.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasureBounds {
    pub minimum: Option<Bound<u64>>,
    pub maximum: Option<Bound<u64>>,
}

/// Owned configuration after family validation, independent of schema references and findings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintParameters {
    StringLength(MeasureBounds),
    BytesLength(MeasureBounds),
    NumericRange(IntegerInterval),
    ItemCount(MeasureBounds),
    Cardinality { minimum: u64, maximum: Option<u64> },
    UniqueItems,
}

/// Owned parameters for the supported scalar families only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScalarConstraintParameters {
    StringLength(MeasureBounds),
    BytesLength(MeasureBounds),
    NumericRange(IntegerInterval),
}

impl ConstraintParameters {
    /// Returns executable scalar parameters; deferred families return `None`.
    pub fn scalar_parameters(&self) -> Option<ScalarConstraintParameters> {
        match self {
            Self::StringLength(bounds) => Some(ScalarConstraintParameters::StringLength(*bounds)),
            Self::BytesLength(bounds) => Some(ScalarConstraintParameters::BytesLength(*bounds)),
            Self::NumericRange(bounds) => Some(ScalarConstraintParameters::NumericRange(*bounds)),
            Self::ItemCount(_) | Self::Cardinality { .. } | Self::UniqueItems => None,
        }
    }
}

impl ScalarConstraintParameters {
    /// Identifies the executable family for agreement with concrete-type support lookup.
    pub fn family(&self) -> ScalarConstraintFamily {
        match self {
            Self::StringLength(_) => ScalarConstraintFamily::StringLength,
            Self::BytesLength(_) => ScalarConstraintFamily::BytesLength,
            Self::NumericRange(_) => ScalarConstraintFamily::NumericRange,
        }
    }
}
