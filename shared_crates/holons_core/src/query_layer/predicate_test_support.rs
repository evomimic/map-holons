//! Descriptor world for predicate unit tests.
//!
//! Stages the canonical descriptors predicate validation resolves by key
//! (`MapStringValueType.StringValueType`, the Contains/Equals/LessThan
//! operators, and the Query predicate and literal types) and builds transient
//! member types over them. Staged holons cannot reference transient ones, so
//! operator affordances sit on a transient `Text` subtype of the canonical
//! string type, the way an application string type would extend it. The
//! Boolean, enum and bytes types extend staged kind anchors the same way.

use std::sync::Arc;

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use core_types::HolonError;
use type_names::{
    CoreOperatorTypeName, CorePropertyTypeName, CoreRelationshipTypeName, QueryHolonTypeName,
    QueryPropertyTypeName, QueryRelationshipTypeName,
};

use super::authoring;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::test_support::{
    core_operator, new_descriptor_holon, new_holon_type_descriptor, new_kind_descriptor,
    new_property_descriptor_holon, new_schema_holon, new_test_holon,
};
use crate::descriptors::{resolve_core_descriptor, HolonDescriptor, ValueDescriptor};
use crate::reference_layer::{HolonReference, TransientReference, WritableHolon};

pub(crate) struct PredicateWorld {
    pub(crate) context: Arc<TransactionContext>,
    /// The canonical `MapStringValueType.StringValueType` (staged).
    pub(crate) map_string: HolonReference,
    /// A transient string subtype affording Contains, Equals and LessThan.
    pub(crate) text: HolonReference,
    /// A text subtype carrying a configured constraint.
    pub(crate) identifier: HolonReference,
    /// An integer type affording Equals only.
    pub(crate) integer: HolonReference,
    /// A text subtype with no local affordance; it inherits Text's operators.
    pub(crate) headline: HolonReference,
    /// A Boolean type affording Equals.
    pub(crate) flag: HolonReference,
    /// An enum type with variants `Draft` and `Published`, affording Equals.
    pub(crate) status: HolonReference,
    /// A bytes type that explicitly affords Equals.
    pub(crate) digest: HolonReference,
    /// A bytes type affording nothing, as canonical `MapBytesValueType` does.
    pub(crate) raw_digest: HolonReference,
    pub(crate) contains: HolonReference,
    pub(crate) equals: HolonReference,
    pub(crate) less_than: HolonReference,
    /// `Book`: required `Title`, optional `Subtitle`, optional `Code`
    /// (foreign string), optional `Identifier` (constrained), optional
    /// `PageCount`, and optional `Headline`, `IsPublished`, `Status`, `Digest`
    /// and `RawDigest`.
    pub(crate) book: HolonReference,
    /// `Novel` extends `Book` and adds an optional `Narrator`.
    pub(crate) novel: HolonReference,
}

impl PredicateWorld {
    pub(crate) fn new(context: Arc<TransactionContext>) -> Self {
        Self::build(context, false)
    }

    /// A world whose canonical `MapStringValueType` itself carries a
    /// configured constraint, as it would once its length constraint is
    /// attached.
    pub(crate) fn with_constrained_map_string(context: Arc<TransactionContext>) -> Self {
        Self::build(context, true)
    }

    fn build(context: Arc<TransactionContext>, constrain_map_string: bool) -> Self {
        let stage = |holon: TransientReference| -> HolonReference {
            context.mutation().stage_new_holon(holon).unwrap().into()
        };
        for type_name in [
            QueryHolonTypeName::ComparisonPredicate,
            QueryHolonTypeName::AnyOf,
            QueryHolonTypeName::LiteralOperand,
            QueryHolonTypeName::Filter,
        ] {
            canonical_type(&context, &type_name.as_holon_name().0);
        }

        let string_anchor =
            stage(new_kind_descriptor(&context, "StringValueType", None, true).unwrap());
        let mut map_string = new_descriptor_holon(
            &context,
            "MapStringValueType.StringValueType",
            "MapStringValueType",
            "Value",
        )
        .unwrap();
        map_string
            .with_property_value(CorePropertyTypeName::DefinesInstanceTypeKind, false)
            .unwrap();
        map_string
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![string_anchor.clone()])
            .unwrap();
        if constrain_map_string {
            let constraint_type = stage(
                new_holon_type_descriptor(
                    &context,
                    "StringLengthConstraint.ConstraintType",
                    "StringLengthConstraint",
                )
                .unwrap(),
            );
            let constraint =
                stage(new_schema_holon(&context, "Length16k", &constraint_type).unwrap());
            map_string
                .add_related_holons(CoreRelationshipTypeName::Constraints, vec![constraint])
                .unwrap();
        }
        let map_string = stage(map_string);

        let contains =
            core_operator(&context, CoreOperatorTypeName::ContainsOperator, Some(&map_string))
                .unwrap();
        let equals = core_operator(&context, CoreOperatorTypeName::EqualsOperator, None).unwrap();
        let less_than =
            core_operator(&context, CoreOperatorTypeName::LessThanOperator, None).unwrap();

        let value_type = |key: &str, parent: &HolonReference, operators: &[&HolonReference]| {
            let mut holon = new_kind_descriptor(&context, key, Some(parent), false).unwrap();
            if !operators.is_empty() {
                holon
                    .add_related_holons(
                        CoreRelationshipTypeName::AffordsOperator,
                        operators.iter().map(|operator| (*operator).clone()).collect(),
                    )
                    .unwrap();
            }
            holon
        };
        let text: HolonReference =
            value_type("Text", &map_string, &[&contains, &equals, &less_than]).into();
        let mut identifier = value_type("Identifier", &text, &[]);
        identifier
            .add_related_holons(
                CoreRelationshipTypeName::Constraints,
                vec![constraint(&context, "identifier-pattern", "StringPatternConstraint")],
            )
            .unwrap();
        let identifier: HolonReference = identifier.into();
        let foreign_string: HolonReference =
            value_type("ForeignString", &string_anchor, &[&contains]).into();
        let integer_anchor =
            stage(new_kind_descriptor(&context, "IntegerValueType", None, true).unwrap());
        let integer: HolonReference = value_type("Count", &integer_anchor, &[&equals]).into();
        let headline: HolonReference = value_type("Headline", &text, &[]).into();
        let anchor = |kind: &str| stage(new_kind_descriptor(&context, kind, None, true).unwrap());
        let flag: HolonReference =
            value_type("Flag", &anchor("BooleanValueType"), &[&equals]).into();
        let mut status = value_type("Status", &anchor("MapEnumValueType"), &[&equals]);
        status
            .add_related_holons(
                CoreRelationshipTypeName::Variants,
                ["Draft", "Published"]
                    .iter()
                    .map(|variant| {
                        new_descriptor_holon(&context, variant, variant, "EnumVariant")
                            .unwrap()
                            .into()
                    })
                    .collect(),
            )
            .unwrap();
        let status: HolonReference = status.into();
        let bytes_anchor = anchor("BytesValueType");
        let digest: HolonReference = value_type("Digest", &bytes_anchor, &[&equals]).into();
        let raw_digest: HolonReference = value_type("RawDigest", &bytes_anchor, &[]).into();

        let book = holon_type(
            &context,
            "Book",
            None,
            &[
                ("Title", true, &text),
                ("Subtitle", false, &text),
                ("Code", false, &foreign_string),
                ("Identifier", false, &identifier),
                ("PageCount", false, &integer),
                ("Headline", false, &headline),
                ("IsPublished", false, &flag),
                ("Status", false, &status),
                ("Digest", false, &digest),
                ("RawDigest", false, &raw_digest),
            ],
        );
        let novel = holon_type(&context, "Novel", Some(&book), &[("Narrator", false, &text)]);

        Self {
            context,
            map_string,
            text,
            identifier,
            integer,
            headline,
            flag,
            status,
            digest,
            raw_digest,
            contains,
            equals,
            less_than,
            book,
            novel,
        }
    }

    pub(crate) fn descriptor(holon: &HolonReference) -> HolonDescriptor {
        HolonDescriptor::from_holon(holon.clone())
    }

    /// A transient member of `holon_type` with the given stored values.
    pub(crate) fn member(
        &self,
        key: &str,
        holon_type: &HolonReference,
        values: &[(&str, BaseValue)],
    ) -> HolonReference {
        let mut holon = new_test_holon(&self.context, key).unwrap();
        holon.with_descriptor(holon_type.clone()).unwrap();
        for (name, value) in values {
            holon.with_property_value(*name, value.clone()).unwrap();
        }
        holon.into()
    }

    /// A literal operand declaring `value` as `value_type`.
    pub(crate) fn literal(&self, value_type: &HolonReference, value: BaseValue) -> HolonReference {
        authoring::new_literal_operand(
            &self.context,
            &ValueDescriptor::from_holon(value_type.clone()),
            value,
        )
        .unwrap()
        .into()
    }

    /// A Contains substring literal declared as the canonical string type.
    pub(crate) fn substring(&self, value: &str) -> HolonReference {
        self.literal(&self.map_string, text(value))
    }

    /// A Contains comparison over `property` of `Book`.
    pub(crate) fn contains(&self, property: &str, value: &str) -> HolonReference {
        self.comparison(property, &self.contains, vec![self.substring(value)])
    }

    /// An Equals comparison over `property` of `Book`, its literal declared as
    /// `value_type`.
    pub(crate) fn equals(
        &self,
        property: &str,
        value_type: &HolonReference,
        value: BaseValue,
    ) -> HolonReference {
        self.comparison(property, &self.equals, vec![self.literal(value_type, value)])
    }

    /// A comparison predicate built directly, so tests can pair any operator
    /// with any property and any operands.
    pub(crate) fn comparison(
        &self,
        property: &str,
        operator: &HolonReference,
        operands: Vec<HolonReference>,
    ) -> HolonReference {
        let mut predicate = self.described("comparison", QueryHolonTypeName::ComparisonPredicate);
        predicate.with_property_value(QueryPropertyTypeName::PropertyName, property).unwrap();
        predicate
            .add_related_holons(QueryRelationshipTypeName::Operator, vec![operator.clone()])
            .unwrap();
        if !operands.is_empty() {
            predicate.add_related_holons(QueryRelationshipTypeName::Operands, operands).unwrap();
        }
        predicate.into()
    }

    /// An `AnyOf` over `predicates`, built directly so an empty group is possible.
    pub(crate) fn any_of(&self, predicates: Vec<HolonReference>) -> HolonReference {
        let mut any_of = self.described("any-of", QueryHolonTypeName::AnyOf);
        if !predicates.is_empty() {
            any_of.add_related_holons(QueryRelationshipTypeName::Predicates, predicates).unwrap();
        }
        any_of.into()
    }

    /// A transient holon described by the canonical Query type `type_name`.
    pub(crate) fn described(&self, key: &str, type_name: QueryHolonTypeName) -> TransientReference {
        let mut holon = new_test_holon(&self.context, key).unwrap();
        holon.with_descriptor(canonical_type(&self.context, &type_name.as_holon_name().0)).unwrap();
        holon
    }
}

pub(crate) fn text(value: &str) -> BaseValue {
    BaseValue::StringValue(MapString(value.to_string()))
}

pub(crate) fn number(value: i64) -> BaseValue {
    BaseValue::IntegerValue(MapInteger(value))
}

pub(crate) fn flag(value: bool) -> BaseValue {
    BaseValue::BooleanValue(MapBoolean(value))
}

pub(crate) fn token(variant: &str) -> BaseValue {
    BaseValue::EnumValue(MapEnumValue(MapString(variant.to_string())))
}

pub(crate) fn bytes(value: &[u8]) -> BaseValue {
    BaseValue::BytesValue(MapBytes(value.to_vec()))
}

/// The canonical `<type_name>.HolonType` descriptor, staged on first use.
pub(crate) fn canonical_type(context: &Arc<TransactionContext>, type_name: &str) -> HolonReference {
    match resolve_core_descriptor(context, &format!("{type_name}.HolonType")) {
        Ok(descriptor) => descriptor,
        Err(HolonError::HolonNotFound(_)) => context
            .mutation()
            .stage_new_holon(
                new_holon_type_descriptor(context, &format!("{type_name}.HolonType"), type_name)
                    .unwrap(),
            )
            .unwrap()
            .into(),
        Err(error) => panic!("resolving {type_name}: {error:?}"),
    }
}

/// A transient configured-constraint holon described by `constraint_type_name`.
fn constraint(
    context: &Arc<TransactionContext>,
    key: &str,
    constraint_type_name: &str,
) -> HolonReference {
    let constraint_type = new_holon_type_descriptor(
        context,
        &format!("{constraint_type_name}.ConstraintType"),
        constraint_type_name,
    )
    .unwrap();
    new_schema_holon(context, key, &constraint_type.into()).unwrap().into()
}

/// A transient holon type declaring `(name, required, value type)` properties,
/// optionally extending `parent`.
fn holon_type(
    context: &Arc<TransactionContext>,
    type_name: &str,
    parent: Option<&HolonReference>,
    properties: &[(&str, bool, &HolonReference)],
) -> HolonReference {
    let declarations: Vec<HolonReference> = properties
        .iter()
        .map(|(name, required, value_type)| {
            new_property_descriptor_holon(
                context,
                &format!("{type_name}.{name}.PropertyType"),
                name,
                *required,
                (*value_type).clone(),
            )
            .unwrap()
            .into()
        })
        .collect();
    let mut holon_type =
        new_holon_type_descriptor(context, &format!("{type_name}.HolonType"), type_name).unwrap();
    holon_type
        .add_related_holons(CoreRelationshipTypeName::InstanceProperties, declarations)
        .unwrap();
    if let Some(parent) = parent {
        holon_type
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![parent.clone()])
            .unwrap();
    }
    holon_type.into()
}
