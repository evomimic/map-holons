/// One authored endpoint; comparison uses its inclusivity without adjusting the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bound<T> {
    pub value: T,
    pub inclusive: bool,
}

/// Signed integer interval. An ordered interval need not contain an integer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegerInterval {
    pub minimum: Option<Bound<i64>>,
    pub maximum: Option<Bound<i64>>,
}

/// Non-negative measure interval for length and item-count families.
#[derive(Clone, Debug, PartialEq, Eq)]
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
