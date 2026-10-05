//! `OrderBy` — stable, descriptor-backed multi-key ordering for QueryCore.
//!
//! An `OrderBy` expression relates, through the ordered `OrderBySpecs`
//! relationship, one through five `OrderBySpec` holons; relationship target
//! order is sort precedence. Each spec selects the property to sort by **by
//! name** (`PropertyName`) and carries a `SortDirection` and a `NullPlacement`.
//!
//! Argument resolution is read-only. `SortDirection` and `NullPlacement` are
//! resolved with [`crate::descriptors::PropertyDescriptor::effective_value`]: an authored value is
//! validated and used without consulting the default; only an absent value
//! resolves the descriptor-defined default (`Ascending`, `Missing-Last`), which
//! is never written back. Nothing here calls `populate_defaults`, and no
//! resolved descriptor is written to the spec, so neither a successful nor a
//! failed evaluation changes the caller's definitions.
//!
//! Validation precedes any output:
//! - the spec shape (count, kind, a string `PropertyName`, enum values), even for
//!   empty input;
//! - for every input occurrence, including a singleton and keys that do not
//!   decide the final order: `PropertyName` resolves through the member's own
//!   effective property surface (the shared descriptor lookup), requiredness
//!   comes from that member's resolved declaration, and every member of a key
//!   must resolve to the same effective value-type descriptor identity — also
//!   when its value is absent. Distinct `PropertyType`s sharing the name are
//!   accepted on those terms; matching primitive representations alone are not.
//!
//! Empty input has no members to resolve against, so property applicability and
//! comparison domains are only checked once members exist.
//!
//! Supported domains are descriptor-backed integer and string values whose
//! descriptor affords `EqualsOperator` and `LessThanOperator`; comparisons go
//! through those operators. Every other domain (boolean, enum, bytes, arrays)
//! fails with `UnsupportedOperator` — there is no stringification or
//! reference-identity fallback.
//!
//! Ordering is lexicographic over the keys and stable: the first non-tied key
//! decides, fully tied occurrences keep their input order, and duplicate
//! occurrences stay separate. A missing value sorts before (`Missing-First`) or
//! after (`Missing-Last`) every present value regardless of direction, and two
//! missing values tie. Direction reverses only the comparison of present
//! values, per key, so descending keys keep stability.

use std::cmp::Ordering;
use std::collections::HashMap;

use base_types::{BaseValue, BaseValueKind, MapEnumValue};
use core_types::{HolonError, PropertyName};
use type_names::{CoreOperatorTypeName, QueryPropertyTypeName, QueryRelationshipTypeName};

use super::query_core::{related_members, require_described_as};
use crate::descriptors::{
    same_definition, Descriptor, EffectiveValue, EnumValueDescriptor, HolonDescriptor,
    IntegerValueDescriptor, OperatorDescriptor, StringValueDescriptor, ValueDescriptor,
    ValueDescriptorKind,
};
use crate::reference_layer::{HolonReference, ReadableHolon};

const ORDER_BY_SPEC_TYPE_NAME: &str = "OrderBySpec";
const MIN_SPECS: usize = 1;
const MAX_SPECS: usize = 5;

/// Sorts `members` by the `OrderBy` definition `expression`. Returns the same
/// occurrences, reordered.
pub(crate) fn order_by(
    expression: &HolonReference,
    members: &[HolonReference],
) -> Result<Vec<HolonReference>, HolonError> {
    let specs = read_sort_specs(expression)?;
    if members.is_empty() {
        return Ok(Vec::new());
    }

    let mut keys: Vec<ResolvedKey> = specs.into_iter().map(ResolvedKey::new).collect();
    let mut declarations = MemberDeclarations::default();
    let mut rows = Vec::with_capacity(members.len());
    for (index, member) in members.iter().enumerate() {
        let values = keys
            .iter_mut()
            .map(|key| key.value_of(member, &mut declarations))
            .collect::<Result<Vec<_>, _>>()?;
        rows.push((index, values));
    }

    let rules: Vec<KeyRule> = keys.iter().map(|key| key.spec.rule).collect();
    let sorted = stable_sort(rows, |(_, left), (_, right)| {
        compare_keys(&rules, left, right, |position, lhs, rhs| keys[position].compare(lhs, rhs))
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

/// One validated `OrderBySpec`: the selected property name and its rule.
struct SortSpec {
    property_name: PropertyName,
    rule: KeyRule,
}

/// Reads and validates the ordered specs of an `OrderBy` definition. Needs no
/// input members, so it applies to empty input too.
fn read_sort_specs(expression: &HolonReference) -> Result<Vec<SortSpec>, HolonError> {
    let specs = related_members(expression, QueryRelationshipTypeName::OrderBySpecs)?;
    if !(MIN_SPECS..=MAX_SPECS).contains(&specs.len()) {
        return Err(HolonError::InvalidParameter(format!(
            "OrderBy requires {MIN_SPECS} through {MAX_SPECS} OrderBySpecs, found {}",
            specs.len()
        )));
    }
    specs.iter().map(read_sort_spec).collect()
}

fn read_sort_spec(spec: &HolonReference) -> Result<SortSpec, HolonError> {
    require_described_as(spec, ORDER_BY_SPEC_TYPE_NAME)?;
    let spec_descriptor = spec.holon_descriptor()?;

    let property_name = read_property_name(spec, &spec_descriptor)?;
    let direction = read_variant(spec, &spec_descriptor, QueryPropertyTypeName::SortDirection)?;
    let direction = SortDirection::from_variant(&direction)
        .ok_or_else(|| unrecognized_variant(QueryPropertyTypeName::SortDirection, &direction))?;
    let placement = read_variant(spec, &spec_descriptor, QueryPropertyTypeName::NullPlacement)?;
    let placement = NullPlacement::from_variant(&placement)
        .ok_or_else(|| unrecognized_variant(QueryPropertyTypeName::NullPlacement, &placement))?;

    Ok(SortSpec { property_name, rule: KeyRule { direction, placement } })
}

/// Resolves the required, default-less `PropertyName` argument of `spec`. It
/// must be a string; there is no fallback.
fn read_property_name(
    spec: &HolonReference,
    spec_descriptor: &HolonDescriptor,
) -> Result<PropertyName, HolonError> {
    let name = QueryPropertyTypeName::PropertyName;
    let property = spec_descriptor.get_property_by_name(name.clone())?;
    match property.effective_value(spec)? {
        Some(EffectiveValue::Authored(BaseValue::StringValue(value)))
        | Some(EffectiveValue::Default(BaseValue::StringValue(value))) => Ok(PropertyName(value)),
        Some(EffectiveValue::Authored(other)) | Some(EffectiveValue::Default(other)) => {
            Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".to_string()))
        }
        None => Err(HolonError::EmptyField(name.as_property_name().to_string())),
    }
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

/// A member type's resolved declaration of one selected property name.
#[derive(Clone)]
struct MemberDeclaration {
    required: bool,
    value_type: HolonReference,
}

/// Caches each member type's resolution of each selected property name, so a
/// collection of one type resolves every name once.
#[derive(Default)]
struct MemberDeclarations {
    by_type_and_name: HashMap<(String, String), MemberDeclaration>,
}

impl MemberDeclarations {
    /// Resolves `property_name` through `member`'s effective property surface
    /// with the shared descriptor lookup, whose undeclared-name and
    /// duplicate-declaration errors propagate unchanged.
    fn resolve(
        &mut self,
        member: &HolonReference,
        property_name: &PropertyName,
    ) -> Result<MemberDeclaration, HolonError> {
        let descriptor = member.holon_descriptor()?;
        let cache_key = (descriptor.holon().reference_id_string(), property_name.to_string());
        if let Some(declaration) = self.by_type_and_name.get(&cache_key) {
            return Ok(declaration.clone());
        }
        let property = descriptor.get_property_by_name(property_name.clone())?;
        let declaration = MemberDeclaration {
            required: property.is_required()?,
            value_type: property.value_type()?.holon().clone(),
        };
        self.by_type_and_name.insert(cache_key, declaration.clone());
        Ok(declaration)
    }
}

/// One sort key while its members are resolved. The first member fixes the
/// key's value type and comparison domain; every later member must resolve to
/// that same value-type identity.
struct ResolvedKey {
    spec: SortSpec,
    binding: Option<(HolonReference, KeyDomain)>,
}

impl ResolvedKey {
    fn new(spec: SortSpec) -> Self {
        Self { spec, binding: None }
    }

    /// This key's value on `member`: `Some` when present and valid, `None`
    /// when the member's own declaration makes the property optional.
    fn value_of(
        &mut self,
        member: &HolonReference,
        declarations: &mut MemberDeclarations,
    ) -> Result<Option<BaseValue>, HolonError> {
        let declaration = declarations.resolve(member, &self.spec.property_name)?;
        self.bind(&declaration.value_type, member)?;
        match member.property_value(&self.spec.property_name)? {
            Some(value) => {
                self.domain().validate(&value)?;
                Ok(Some(value))
            }
            None if declaration.required => {
                Err(HolonError::EmptyField(self.spec.property_name.to_string()))
            }
            None => Ok(None),
        }
    }

    /// Fixes the key's value type on first use and checks identity afterwards,
    /// whether or not the member has a value.
    fn bind(
        &mut self,
        value_type: &HolonReference,
        member: &HolonReference,
    ) -> Result<(), HolonError> {
        match &self.binding {
            Some((bound, _)) if same_definition(bound, value_type) => Ok(()),
            Some((bound, _)) => Err(HolonError::InvalidParameter(format!(
                "OrderBy key {} resolves to value type {} on {}, but to {} on an earlier \
                 member; every member of a key must share one value type",
                self.spec.property_name,
                ValueDescriptor::from_holon(value_type.clone()).header().type_name()?,
                member.summarize()?,
                ValueDescriptor::from_holon(bound.clone()).header().type_name()?,
            ))),
            None => {
                let domain = KeyDomain::resolve(&ValueDescriptor::from_holon(value_type.clone()))?;
                self.binding = Some((value_type.clone(), domain));
                Ok(())
            }
        }
    }

    /// The key's comparison domain. Only used once a member has bound the key.
    fn domain(&self) -> &KeyDomain {
        match &self.binding {
            Some((_, domain)) => domain,
            None => unreachable!("a key is bound by its first member"),
        }
    }

    /// Orders two present values of this key.
    fn compare(&self, lhs: &BaseValue, rhs: &BaseValue) -> Result<Ordering, HolonError> {
        self.domain().compare(lhs, rhs)
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
