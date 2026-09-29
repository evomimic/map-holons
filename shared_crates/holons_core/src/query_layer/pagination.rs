//! `Skip` and `Limit` — occurrence-sequence pagination for QueryCore (QRY4a).
//!
//! Both are concrete `QueryExpression` kinds carrying a required, nonnegative
//! integer count on the expression holon itself (`SkipCount` / `LimitCount`,
//! no default). `Skip` removes the first `count` occurrences; `Limit` retains
//! at most `count`. Both preserve relative order and duplicate occurrences and
//! neither requires a preceding `OrderBy`.
//!
//! The count is validated before the input is inspected, so a missing,
//! negative, or non-integer count fails even for an empty input collection.
//! Counts are clamped to the input length rather than converted blindly, so a
//! count beyond `usize` on a 32-bit (wasm) target cannot overflow an index.

use base_types::BaseValue;
use core_types::HolonError;
use type_names::QueryPropertyTypeName;

use crate::reference_layer::{HolonReference, ReadableHolon};

/// Reads and validates the `count_property` (`SkipCount` or `LimitCount`) of a
/// `Skip` or `Limit` expression.
pub(crate) fn read_count(
    expression: &HolonReference,
    count_property: QueryPropertyTypeName,
) -> Result<usize, HolonError> {
    let value = expression.property_value(count_property.clone())?;
    count_from_value(value, &count_property)
}

/// Validates an authored count value. Missing is `EmptyField`, a non-integer
/// is `UnexpectedValueType`, and a negative integer is `InvalidParameter`.
fn count_from_value(
    value: Option<BaseValue>,
    count_property: &QueryPropertyTypeName,
) -> Result<usize, HolonError> {
    let name = count_property.as_property_name().to_string();
    match value {
        Some(BaseValue::IntegerValue(count)) if count.0 < 0 => Err(HolonError::InvalidParameter(
            format!("{name} must be nonnegative, found {}", count.0),
        )),
        // Nonnegative here; a count past usize::MAX (32-bit targets) saturates,
        // which the slicing below clamps to the input length anyway.
        Some(BaseValue::IntegerValue(count)) => Ok(usize::try_from(count.0).unwrap_or(usize::MAX)),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{other:?}"), "Integer".to_string()))
        }
        None => Err(HolonError::EmptyField(name)),
    }
}

/// `Skip`: drops the first `count` occurrences; a count at or above the length
/// returns empty.
pub(crate) fn skip<T: Clone>(members: &[T], count: usize) -> Vec<T> {
    members[count.min(members.len())..].to_vec()
}

/// `Limit`: keeps at most the first `count` occurrences; zero returns empty and
/// a count at or above the length returns every member.
pub(crate) fn limit<T: Clone>(members: &[T], count: usize) -> Vec<T> {
    members[..count.min(members.len())].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::{MapInteger, MapString};

    const MEMBERS: [&str; 4] = ["a", "b", "a", "c"];

    #[test]
    fn skip_boundaries_preserve_order_and_duplicates() {
        assert_eq!(skip(&MEMBERS, 0), MEMBERS.to_vec());
        assert_eq!(skip(&MEMBERS, 1), vec!["b", "a", "c"]);
        assert!(skip(&MEMBERS, 4).is_empty());
        assert!(skip(&MEMBERS, 5).is_empty());
        assert!(skip(&MEMBERS, usize::MAX).is_empty());
        assert!(skip::<&str>(&[], 0).is_empty());
        assert!(skip::<&str>(&[], 3).is_empty());
    }

    #[test]
    fn limit_boundaries_preserve_order_and_duplicates() {
        assert!(limit(&MEMBERS, 0).is_empty());
        assert_eq!(limit(&MEMBERS, 3), vec!["a", "b", "a"]);
        assert_eq!(limit(&MEMBERS, 4), MEMBERS.to_vec());
        assert_eq!(limit(&MEMBERS, 5), MEMBERS.to_vec());
        assert_eq!(limit(&MEMBERS, usize::MAX), MEMBERS.to_vec());
        assert!(limit::<&str>(&[], 2).is_empty());
    }

    #[test]
    fn count_accepts_nonnegative_integers() {
        let skip_count = QueryPropertyTypeName::SkipCount;
        let value = |n| Some(BaseValue::IntegerValue(MapInteger(n)));
        assert_eq!(count_from_value(value(0), &skip_count).unwrap(), 0);
        assert_eq!(count_from_value(value(7), &skip_count).unwrap(), 7);
        // i64::MAX never overflows: it converts (64-bit) or saturates (32-bit).
        let max = count_from_value(value(i64::MAX), &skip_count).unwrap();
        assert_eq!(max, usize::try_from(i64::MAX).unwrap_or(usize::MAX));
        assert!(skip(&MEMBERS, max).is_empty());
        assert_eq!(limit(&MEMBERS, max), MEMBERS.to_vec());
    }

    #[test]
    fn count_rejects_missing_negative_and_non_integer_values() {
        let limit_count = QueryPropertyTypeName::LimitCount;
        let missing = count_from_value(None, &limit_count).unwrap_err();
        assert!(matches!(&missing, HolonError::EmptyField(name) if name == "LimitCount"));

        for n in [-1, i64::MIN] {
            let negative =
                count_from_value(Some(BaseValue::IntegerValue(MapInteger(n))), &limit_count)
                    .unwrap_err();
            assert!(matches!(negative, HolonError::InvalidParameter(_)));
        }

        let text = count_from_value(
            Some(BaseValue::StringValue(MapString("3".to_string()))),
            &limit_count,
        )
        .unwrap_err();
        assert!(matches!(text, HolonError::UnexpectedValueType(..)));
    }
}
