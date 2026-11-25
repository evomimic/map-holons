use std::borrow::Cow;

use crate::{BaseValue, BaseValueKind, MapEnumValue, MapString};

pub(super) fn supports_kind(kind: BaseValueKind) -> bool {
    match kind {
        BaseValueKind::String
        | BaseValueKind::Boolean
        | BaseValueKind::Integer
        | BaseValueKind::Enum => true,
        BaseValueKind::Bytes => false,
    }
}

/// Converts a prepared scalar to its canonical template text.
pub(super) fn canonical_text(value: &BaseValue) -> Option<Cow<'_, str>> {
    Some(match value {
        BaseValue::StringValue(MapString(text))
        | BaseValue::EnumValue(MapEnumValue(MapString(text))) => Cow::Borrowed(text),
        BaseValue::BooleanValue(value) => Cow::Borrowed(if value.0 { "true" } else { "false" }),
        BaseValue::IntegerValue(value) => Cow::Owned(integer_text(value.0)),
        BaseValue::BytesValue(_) => return None,
    })
}

fn integer_text(value: i64) -> String {
    let mut magnitude = value.unsigned_abs();
    // A signed i64 needs at most 19 digits and a sign.
    let mut digits = [0; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (magnitude % 10) as u8;
        magnitude /= 10;
        if magnitude == 0 {
            break;
        }
    }
    if value < 0 {
        start -= 1;
        digits[start] = b'-';
    }
    digits[start..].iter().map(|&byte| char::from(byte)).collect()
}
