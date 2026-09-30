//! `OrderBy` — stable, descriptor-backed multi-key ordering for QueryCore (QRY4a).
//!
//! An `OrderBy` expression relates, through the ordered `OrderBySpecs`
//! relationship, one through five `OrderBySpec` holons; relationship target
//! order is sort precedence. Each spec names the `PropertyType` descriptor to
//! sort by (`Property`, matched by descriptor identity, never by name) and
//! carries a `SortDirection` and a `NullPlacement`.
//!
//! Argument resolution is read-only. `SortDirection` and `NullPlacement` are
//! resolved with [`PropertyDescriptor::effective_value`]: an authored value is
//! validated and used without consulting the default; only an absent value
//! resolves the descriptor-defined default (`Ascending`, `Missing-Last`), which
//! is never written back. Nothing here calls `populate_defaults`, so neither a
//! successful nor a failed evaluation changes the caller's definitions.
//!
//! Validation precedes any output:
//! - the spec shape (count, kind, `Property` target, enum values) and each key's
//!   value domain, even for empty input;
//! - for every input occurrence, including a singleton: the referenced property
//!   must be on the member's effective property surface (by identity), a present
//!   value must be valid for the key's value descriptor, and an absent value is
//!   only allowed when the property is optional.
//!
//! Supported domains are descriptor-backed integer and string values whose
//! descriptor affords `EqualsOperator` and `LessThanOperator`; comparisons go
//! through those operators. Every other domain (boolean, enum, bytes, arrays)
//! fails with `UnsupportedOperator` — there is no stringification or
//! reference-identity fallback. Because keys are matched by property identity,
//! all members share that property's value descriptor.
//!
//! Ordering is lexicographic over the keys and stable: the first non-tied key
//! decides, fully tied occurrences keep their input order, and duplicate
//! occurrences stay separate. A missing value sorts before (`Missing-First`) or
//! after (`Missing-Last`) every present value regardless of direction, and two
//! missing values tie. Direction reverses only the comparison of present
//! values, per key, so descending keys keep stability.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use base_types::{BaseValue, BaseValueKind, MapEnumValue};
use core_types::{HolonError, PropertyName};
use type_names::{CoreOperatorTypeName, QueryPropertyTypeName, QueryRelationshipTypeName};

use super::query_core::{exactly_one, related_members, require_described_as};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{
    equals_or_extends, resolve_core_descriptor, same_definition, Descriptor, EffectiveValue,
    EnumValueDescriptor, HolonDescriptor, IntegerValueDescriptor, OperatorDescriptor,
    PropertyDescriptor, StringValueDescriptor, ValueDescriptor, ValueDescriptorKind,
};
use crate::reference_layer::{HolonReference, ReadableHolon};

const ORDER_BY_SPEC_TYPE_NAME: &str = "OrderBySpec";
const PROPERTY_TYPE_DESCRIPTOR_KEY: &str = "PropertyType.TypeDescriptor";
const MIN_SPECS: usize = 1;
const MAX_SPECS: usize = 5;

/// Sorts `members` by the `OrderBy` definition `expression`. Returns the same
/// occurrences, reordered.
pub(crate) fn order_by(
    context: &Arc<TransactionContext>,
    expression: &HolonReference,
    members: &[HolonReference],
) -> Result<Vec<HolonReference>, HolonError> {
    let keys = read_sort_keys(context, expression)?;

    let mut surfaces = PropertySurfaces::default();
    let mut rows = Vec::with_capacity(members.len());
    for (index, member) in members.iter().enumerate() {
        let values = keys
            .iter()
            .map(|key| key.value_of(member, &mut surfaces))
            .collect::<Result<Vec<_>, _>>()?;
        rows.push((index, values));
    }

    let rules: Vec<KeyRule> = keys.iter().map(|key| key.rule).collect();
    let sorted = stable_sort(rows, |(_, left), (_, right)| {
        compare_keys(&rules, left, right, |position, lhs, rhs| {
            keys[position].domain.compare(lhs, rhs)
        })
    })?;
    Ok(sorted.into_iter().map(|(index, _)| members[index].clone()).collect())
}

/// Direction of present-value ordering for one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Ascending,
    Descending,
}

/// Placement of missing values for one key, independent of direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NullPlacement {
    MissingFirst,
    MissingLast,
}

impl SortDirection {
    fn from_variant(variant: &str) -> Option<Self> {
        match variant {
            "Ascending" => Some(Self::Ascending),
            "Descending" => Some(Self::Descending),
            _ => None,
        }
    }
}

impl NullPlacement {
    fn from_variant(variant: &str) -> Option<Self> {
        match variant {
            "Missing-First" => Some(Self::MissingFirst),
            "Missing-Last" => Some(Self::MissingLast),
            _ => None,
        }
    }
}

/// The resolved direction and placement of one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyRule {
    direction: SortDirection,
    placement: NullPlacement,
}

/// One validated `OrderBySpec`.
struct SortKey {
    property: PropertyDescriptor,
    property_name: PropertyName,
    required: bool,
    domain: KeyDomain,
    rule: KeyRule,
}

/// Reads and validates the ordered specs of an `OrderBy` definition.
fn read_sort_keys(
    context: &Arc<TransactionContext>,
    expression: &HolonReference,
) -> Result<Vec<SortKey>, HolonError> {
    let specs = related_members(expression, QueryRelationshipTypeName::OrderBySpecs)?;
    if !(MIN_SPECS..=MAX_SPECS).contains(&specs.len()) {
        return Err(HolonError::InvalidParameter(format!(
            "OrderBy requires {MIN_SPECS} through {MAX_SPECS} OrderBySpecs, found {}",
            specs.len()
        )));
    }

    let property_type_root = resolve_core_descriptor(context, PROPERTY_TYPE_DESCRIPTOR_KEY)?;
    specs.iter().map(|spec| read_sort_key(spec, &property_type_root)).collect()
}

fn read_sort_key(
    spec: &HolonReference,
    property_type_root: &HolonReference,
) -> Result<SortKey, HolonError> {
    require_described_as(spec, ORDER_BY_SPEC_TYPE_NAME)?;

    let target = exactly_one(spec, QueryRelationshipTypeName::Property)?;
    if !equals_or_extends(&target, property_type_root)? {
        return Err(HolonError::WrongDescriptorKind {
            expected: "PropertyType".to_string(),
            found: target.holon_descriptor()?.header().type_name()?.to_string(),
            descriptor: target.summarize()?,
        });
    }
    let property = PropertyDescriptor::from_holon(target);

    let spec_descriptor = spec.holon_descriptor()?;
    let direction = read_variant(spec, &spec_descriptor, QueryPropertyTypeName::SortDirection)?;
    let direction = SortDirection::from_variant(&direction)
        .ok_or_else(|| unrecognized_variant(QueryPropertyTypeName::SortDirection, &direction))?;
    let placement = read_variant(spec, &spec_descriptor, QueryPropertyTypeName::NullPlacement)?;
    let placement = NullPlacement::from_variant(&placement)
        .ok_or_else(|| unrecognized_variant(QueryPropertyTypeName::NullPlacement, &placement))?;

    Ok(SortKey {
        property_name: property.property_name()?,
        required: property.is_required()?,
        domain: KeyDomain::resolve(&property.value_type()?)?,
        property,
        rule: KeyRule { direction, placement },
    })
}

/// Resolves a required enum argument of `spec` through the read-only
/// effective-value accessor and validates it against the property's enum
/// descriptor.
///
/// An authored value must already be an enum value; a malformed one fails and
/// never falls back to the default. A descriptor default is the loader's
/// variant-name token, so it is read as that variant before validation.
fn read_variant(
    spec: &HolonReference,
    spec_descriptor: &HolonDescriptor,
    name: QueryPropertyTypeName,
) -> Result<String, HolonError> {
    let property = spec_descriptor.get_property_by_name(name.clone())?;
    let value = match property.effective_value(spec)? {
        Some(EffectiveValue::Authored(value)) => value,
        Some(EffectiveValue::Default(BaseValue::StringValue(token))) => {
            BaseValue::EnumValue(MapEnumValue(token))
        }
        Some(EffectiveValue::Default(value)) => value,
        None => return Err(HolonError::EmptyField(name.as_property_name().to_string())),
    };
    EnumValueDescriptor::from_holon(property.value_type()?.holon().clone()).is_valid(&value)?;
    match value {
        BaseValue::EnumValue(variant) => Ok(variant.0 .0),
        _ => unreachable!("EnumValueDescriptor::is_valid accepts enum values only"),
    }
}

/// A variant the schema declares but this runtime does not interpret.
fn unrecognized_variant(name: QueryPropertyTypeName, variant: &str) -> HolonError {
    HolonError::InvalidParameter(format!(
        "unsupported {} variant for OrderBy: {variant}",
        name.as_property_name()
    ))
}

/// The value domain of one key: its value descriptor and afforded operators.
enum KeyDomain {
    Integer {
        descriptor: IntegerValueDescriptor,
        equals: OperatorDescriptor,
        less_than: OperatorDescriptor,
    },
    String {
        descriptor: StringValueDescriptor,
        equals: OperatorDescriptor,
        less_than: OperatorDescriptor,
    },
}

impl KeyDomain {
    fn resolve(value_type: &ValueDescriptor) -> Result<Self, HolonError> {
        let kind = value_type.value_kind()?;
        let holon = value_type.holon().clone();
        match kind {
            ValueDescriptorKind::BaseValue(BaseValueKind::Integer) => Ok(Self::Integer {
                descriptor: IntegerValueDescriptor::from_holon(holon),
                equals: value_type.get_operator_by_name(CoreOperatorTypeName::EqualsOperator)?,
                less_than: value_type
                    .get_operator_by_name(CoreOperatorTypeName::LessThanOperator)?,
            }),
            ValueDescriptorKind::BaseValue(BaseValueKind::String) => Ok(Self::String {
                descriptor: StringValueDescriptor::from_holon(holon),
                equals: value_type.get_operator_by_name(CoreOperatorTypeName::EqualsOperator)?,
                less_than: value_type
                    .get_operator_by_name(CoreOperatorTypeName::LessThanOperator)?,
            }),
            _ => Err(HolonError::UnsupportedOperator {
                operator: CoreOperatorTypeName::LessThanOperator.as_operator_name().to_string(),
                value_type: value_type.header().type_name()?.to_string(),
                descriptor: value_type.holon().summarize()?,
            }),
        }
    }

    fn validate(&self, value: &BaseValue) -> Result<(), HolonError> {
        match self {
            Self::Integer { descriptor, .. } => descriptor.is_valid(value),
            Self::String { descriptor, .. } => descriptor.is_valid(value),
        }
    }

    /// Orders two present values through the descriptor's afforded operators.
    fn compare(&self, lhs: &BaseValue, rhs: &BaseValue) -> Result<Ordering, HolonError> {
        let (equal, less) = match self {
            Self::Integer { descriptor, equals, less_than } => (
                descriptor.apply_operator(equals, lhs, rhs)?,
                descriptor.apply_operator(less_than, lhs, rhs)?,
            ),
            Self::String { descriptor, equals, less_than } => (
                descriptor.apply_operator(equals, lhs, rhs)?,
                descriptor.apply_operator(less_than, lhs, rhs)?,
            ),
        };
        Ok(if equal {
            Ordering::Equal
        } else if less {
            Ordering::Less
        } else {
            Ordering::Greater
        })
    }
}

/// Per-descriptor cache of each member type's effective property surface.
#[derive(Default)]
struct PropertySurfaces {
    by_descriptor: HashMap<String, Vec<PropertyDescriptor>>,
}

impl PropertySurfaces {
    /// Whether `property` (by identity) is on `member`'s effective property surface.
    fn declares(
        &mut self,
        member: &HolonReference,
        property: &PropertyDescriptor,
    ) -> Result<bool, HolonError> {
        let descriptor = member.holon_descriptor()?;
        let key = descriptor.holon().reference_id_string();
        if !self.by_descriptor.contains_key(&key) {
            self.by_descriptor.insert(key.clone(), descriptor.instance_properties()?);
        }
        Ok(self.by_descriptor[&key]
            .iter()
            .any(|candidate| same_definition(candidate.holon(), property.holon())))
    }
}

impl SortKey {
    /// This key's value on `member`: `Some` when present and valid, `None`
    /// when an optional property is absent.
    fn value_of(
        &self,
        member: &HolonReference,
        surfaces: &mut PropertySurfaces,
    ) -> Result<Option<BaseValue>, HolonError> {
        if !surfaces.declares(member, &self.property)? {
            return Err(HolonError::DescriptorDeclarationNotFound {
                kind: "property".to_string(),
                name: self.property_name.to_string(),
                descriptor: member.summarize()?,
            });
        }
        match member.property_value(&self.property_name)? {
            Some(value) => {
                self.domain.validate(&value)?;
                Ok(Some(value))
            }
            None if self.required => Err(HolonError::EmptyField(self.property_name.to_string())),
            None => Ok(None),
        }
    }
}

/// Lexicographic comparison of two key rows: the first non-tied key decides.
/// Missing values are placed by the key's rule regardless of direction; present
/// values are ordered by `compare_present`, reversed for descending keys.
fn compare_keys<V>(
    rules: &[KeyRule],
    left: &[Option<V>],
    right: &[Option<V>],
    compare_present: impl Fn(usize, &V, &V) -> Result<Ordering, HolonError>,
) -> Result<Ordering, HolonError> {
    for (position, rule) in rules.iter().enumerate() {
        let missing_first = rule.placement == NullPlacement::MissingFirst;
        let ordering = match (&left[position], &right[position]) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) if missing_first => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) if missing_first => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(lhs), Some(rhs)) => {
                let ordering = compare_present(position, lhs, rhs)?;
                match rule.direction {
                    SortDirection::Ascending => ordering,
                    SortDirection::Descending => ordering.reverse(),
                }
            }
        };
        if ordering != Ordering::Equal {
            return Ok(ordering);
        }
    }
    Ok(Ordering::Equal)
}

/// Stable sort with a fallible comparator. The first comparison error aborts
/// the result: it is returned instead of a partially ordered sequence.
fn stable_sort<T>(
    mut rows: Vec<T>,
    compare: impl Fn(&T, &T) -> Result<Ordering, HolonError>,
) -> Result<Vec<T>, HolonError> {
    let mut failure = None;
    rows.sort_by(|left, right| {
        if failure.is_some() {
            return Ordering::Equal;
        }
        compare(left, right).unwrap_or_else(|error| {
            failure = Some(error);
            Ordering::Equal
        })
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(rows),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASC_LAST: KeyRule =
        KeyRule { direction: SortDirection::Ascending, placement: NullPlacement::MissingLast };
    const ASC_FIRST: KeyRule =
        KeyRule { direction: SortDirection::Ascending, placement: NullPlacement::MissingFirst };
    const DESC_LAST: KeyRule =
        KeyRule { direction: SortDirection::Descending, placement: NullPlacement::MissingLast };
    const DESC_FIRST: KeyRule =
        KeyRule { direction: SortDirection::Descending, placement: NullPlacement::MissingFirst };

    /// Rows are `(label, keys)`; sorts them and returns the labels in order.
    fn sort(rules: &[KeyRule], rows: Vec<(&'static str, Vec<Option<i64>>)>) -> Vec<&'static str> {
        stable_sort(rows, |(_, left), (_, right)| {
            compare_keys(rules, left, right, |_, lhs, rhs| Ok(lhs.cmp(rhs)))
        })
        .unwrap()
        .into_iter()
        .map(|(label, _)| label)
        .collect()
    }

    #[test]
    fn ascending_and_descending_keep_ties_in_input_order() {
        let rows = || {
            vec![
                ("a", vec![Some(2)]),
                ("b", vec![Some(1)]),
                ("c", vec![Some(2)]),
                ("d", vec![Some(1)]),
            ]
        };
        assert_eq!(sort(&[ASC_LAST], rows()), vec!["b", "d", "a", "c"]);
        // Descending is per-key reversal, not a reversed ascending result: the
        // tied pairs keep a-before-c and b-before-d.
        assert_eq!(sort(&[DESC_LAST], rows()), vec!["a", "c", "b", "d"]);
    }

    #[test]
    fn missing_placement_is_independent_of_direction() {
        let rows = || {
            vec![
                ("x", vec![None]),
                ("one", vec![Some(1)]),
                ("y", vec![None]),
                ("two", vec![Some(2)]),
            ]
        };
        assert_eq!(sort(&[ASC_LAST], rows()), vec!["one", "two", "x", "y"]);
        assert_eq!(sort(&[ASC_FIRST], rows()), vec!["x", "y", "one", "two"]);
        assert_eq!(sort(&[DESC_LAST], rows()), vec!["two", "one", "x", "y"]);
        assert_eq!(sort(&[DESC_FIRST], rows()), vec!["x", "y", "two", "one"]);
    }

    #[test]
    fn later_keys_break_ties_and_missing_values_tie_with_each_other() {
        // Name ascending (missing last), then age descending: equal names group
        // by decreasing age; missing names follow every present name and are
        // ordered by age among themselves; full ties keep input order.
        let rows = vec![
            ("ann-30", vec![Some(1), Some(30)]),
            ("none-20", vec![None, Some(20)]),
            ("bob-40", vec![Some(2), Some(40)]),
            ("ann-50", vec![Some(1), Some(50)]),
            ("none-60", vec![None, Some(60)]),
            ("ann-30-dup", vec![Some(1), Some(30)]),
        ];
        assert_eq!(
            sort(&[ASC_LAST, DESC_LAST], rows),
            vec!["ann-50", "ann-30", "ann-30-dup", "bob-40", "none-60", "none-20"]
        );
    }

    #[test]
    fn empty_and_singleton_inputs_sort_trivially() {
        assert!(sort(&[ASC_LAST], vec![]).is_empty());
        assert_eq!(sort(&[DESC_FIRST], vec![("only", vec![None])]), vec!["only"]);
    }

    #[test]
    fn a_comparison_error_aborts_the_sort() {
        let rows = vec![("a", vec![Some(1)]), ("b", vec![Some(2)]), ("c", vec![Some(3)])];
        let result = stable_sort(rows, |(_, left), (_, right)| {
            compare_keys(&[ASC_LAST], left, right, |_, _, _| {
                Err(HolonError::InvalidParameter("comparison failed".to_string()))
            })
        });
        assert!(
            matches!(result, Err(HolonError::InvalidParameter(message)) if message == "comparison failed")
        );
    }

    #[test]
    fn missing_values_never_reach_the_value_comparator() {
        let rows = vec![("x", vec![None]), ("y", vec![None]), ("z", vec![Some(1)])];
        let sorted = stable_sort(rows, |(_, left), (_, right)| {
            compare_keys(&[ASC_FIRST], left, right, |_, _, _| {
                Err(HolonError::InvalidParameter("present-only comparator".to_string()))
            })
        })
        .unwrap();
        let labels: Vec<_> = sorted.into_iter().map(|(label, _)| label).collect();
        assert_eq!(labels, vec!["x", "y", "z"]);
    }

    #[test]
    fn enum_variants_match_the_schema_tokens() {
        assert_eq!(SortDirection::from_variant("Ascending"), Some(SortDirection::Ascending));
        assert_eq!(SortDirection::from_variant("Descending"), Some(SortDirection::Descending));
        assert_eq!(NullPlacement::from_variant("Missing-First"), Some(NullPlacement::MissingFirst));
        assert_eq!(NullPlacement::from_variant("Missing-Last"), Some(NullPlacement::MissingLast));
        assert_eq!(SortDirection::from_variant("ascending"), None);
        assert_eq!(NullPlacement::from_variant("MissingLast"), None);
    }
}
