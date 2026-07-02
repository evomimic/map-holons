//! Source-neutral literal values for schema metadata preserved in the semantic IR.
//!
//! These values intentionally resemble common data notation without depending on JSON or any other
//! concrete source format. Format-specific adapters are responsible for converting their native
//! literal syntax into this small vocabulary before descriptors enter the Canonical Holon IR.

use std::fmt;

/// Ordered object literal content used by the Canonical Holon IR.
///
/// Object entries keep source order so round-trip tooling can preserve human-authored metadata
/// layout where possible. Lookup still behaves like a map: inserting an existing key replaces the
/// value without moving the entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LiteralObject {
    entries: Vec<(String, LiteralValue)>,
}

impl LiteralObject {
    /// Creates an empty ordered object.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Returns `true` when the object has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterates over entries in source/insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &LiteralValue)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Looks up an entry by key without exposing the internal ordered storage.
    pub fn get(&self, key: &str) -> Option<&LiteralValue> {
        self.entries.iter().find(|(entry_key, _)| entry_key == key).map(|(_, value)| value)
    }

    /// Inserts or replaces an entry, preserving insertion order for new keys.
    pub fn insert(&mut self, key: impl Into<String>, value: LiteralValue) -> Option<LiteralValue> {
        let key = key.into();
        if let Some((_, existing)) =
            self.entries.iter_mut().find(|(entry_key, _)| *entry_key == key)
        {
            return Some(std::mem::replace(existing, value));
        }
        self.entries.push((key, value));
        None
    }

    /// Extends this object with ordered entries from another source.
    pub fn extend<I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = (String, LiteralValue)>,
    {
        for (key, value) in entries {
            self.insert(key, value);
        }
    }
}

/// Source-neutral literal value used by the Canonical Holon IR.
///
/// `Number` is stored as text so adapters can preserve source spelling and avoid committing this
/// shared semantic layer to one parser's floating-point behavior. Use `Integer` when the source
/// value is known to be integral.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LiteralValue {
    /// Absence/null literal from a source format that supports it.
    Null,
    /// Boolean literal.
    Boolean(bool),
    /// Integral numeric literal with exact `i64` representation.
    Integer(i64),
    /// Non-integral or otherwise text-preserved numeric literal.
    Number(String),
    /// Text literal.
    String(String),
    /// Ordered list literal.
    Array(Vec<LiteralValue>),
    /// Ordered object literal.
    Object(LiteralObject),
}

impl LiteralValue {
    /// Returns the contained boolean when this is a boolean literal.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the contained integer when this is an integer literal or a parseable numeric string.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            Self::Number(text) => text.parse().ok(),
            _ => None,
        }
    }

    /// Returns the contained string only when this is a string literal.
    ///
    /// Numeric text is deliberately not exposed here; callers that need numeric conversion should
    /// choose the specific numeric accessor they intend.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }
}

impl fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Boolean(value) => write!(f, "{value}"),
            Self::Integer(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value}"),
            Self::Array(_) => write!(f, "[array]"),
            Self::Object(_) => write!(f, "{{object}}"),
        }
    }
}
