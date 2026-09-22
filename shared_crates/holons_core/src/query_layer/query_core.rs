//! QueryCore — descriptor-backed Query runtime (QRY1 scaffold + QRY2 operators).
//!
//! This module is the internal direct-execution seam for a reusable `Query`
//! definition. It owns the definition/runtime boundary:
//!
//! - typed entries over holons described as `Query`, `HolonCollection`, and
//!   `HolonSpace` ([`QueryReference`], [`HolonCollectionReference`],
//!   [`FocalSpaceReference`]);
//! - transient `ExecutionInstance` and root `QueryExpressionExecution` records
//!   linked to the definition, the invocation's focal space, and the optional
//!   caller-supplied input;
//! - `Pending -> Running -> Complete | Failed` status progression;
//! - execution of the root expression: `SeedHolons` (QRY2a) and `Expand`
//!   (QRY2b), with `HolonError::NotImplemented` for every other concrete kind
//!   and for a root that carries `Next` (chaining is a later slice).
//!
//! Focal space is invocation context, not definition state: it is recorded only
//! on the transient `ExecutionInstance`. Nothing is ever written onto the
//! reusable Query or QueryExpression definitions. Parameter bindings are
//! accepted and discarded (QRY3). The legacy `query.rs` compatibility surface is
//! untouched and lives beside this module.
//!
//! Input contract: the schema leaves `QueryExpressionExecution.Input` optional
//! because the abstract `QueryExpression` cannot know whether its root is a
//! source or a transform. The runtime enforces the operator-specific rule in
//! [`QueryReference::begin_execution`]: a root `SeedHolons` accepts no input (a
//! supplied collection is a contract error, not an ignored operand); a root
//! `Expand` requires exactly one `HolonCollectionReference`. When present, the
//! caller's collection holon is linked as `Input` by identity; it is never copied.
//!
//! Storage boundary: operators resolve the requested relationship through the
//! source member's `HolonDescriptor` (declared or inverse navigation) and then
//! read that relationship through the transaction-bound Holon service. Results
//! preserve storage order and duplicate occurrences; the collection's keyed
//! index is never used to deduplicate.

use std::sync::Arc;

use base_types::{MapEnumValue, MapString};
use core_types::{HolonError, RelationshipName};
use type_names::{
    CoreRelationshipTypeName, QueryPropertyTypeName, QueryRelationshipTypeName, ToRelationshipName,
};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::resolve_core_descriptor;
use crate::reference_layer::{HolonReference, ReadableHolon, TransientReference, WritableHolon};

/// Descriptor type names and keys the runtime relies on from the loaded schemas.
const QUERY_TYPE_NAME: &str = "Query";
const HOLON_COLLECTION_TYPE_NAME: &str = "HolonCollection";
const HOLON_SPACE_TYPE_NAME: &str = "HolonSpace";
const SEED_HOLONS_TYPE_NAME: &str = "SeedHolons";
const EXPAND_TYPE_NAME: &str = "Expand";
const EXECUTION_INSTANCE_DESCRIPTOR_KEY: &str = "ExecutionInstance.HolonType";
const QUERY_EXPRESSION_EXECUTION_DESCRIPTOR_KEY: &str = "QueryExpressionExecution.HolonType";
const HOLON_COLLECTION_DESCRIPTOR_KEY: &str = "HolonCollection.HolonType";

/// Keys of the transient runtime records created per invocation.
const EXECUTION_INSTANCE_KEY: &str = "execution-instance";
const QUERY_EXPRESSION_EXECUTION_KEY: &str = "query-expression-execution";
const QUERY_RESULT_KEY: &str = "query-result";

/// Typed entry over a saved or staged holon described as `Query`.
#[derive(Debug, Clone)]
pub struct QueryReference(HolonReference);

/// Typed entry over a holon described as `HolonCollection`.
///
/// The collection holon is supplied by the caller (or produced by an operator)
/// and linked by identity; this wrapper only proves its kind at the QueryCore
/// boundary.
#[derive(Debug, Clone)]
pub struct HolonCollectionReference(HolonReference);

/// Typed entry over a holon described as `HolonSpace`, supplied by the caller
/// as the invocation's focal space.
#[derive(Debug, Clone)]
pub struct FocalSpaceReference(HolonReference);

impl HolonCollectionReference {
    /// Wraps a holon reference after verifying it is described as `HolonCollection`.
    pub fn new(reference: HolonReference) -> Result<Self, HolonError> {
        require_described_as(&reference, HOLON_COLLECTION_TYPE_NAME)?;
        Ok(Self(reference))
    }

    /// Returns the underlying collection holon reference.
    pub fn as_holon_reference(&self) -> &HolonReference {
        &self.0
    }
}

impl FocalSpaceReference {
    /// Wraps a holon reference after verifying it is described as `HolonSpace`.
    pub fn new(reference: HolonReference) -> Result<Self, HolonError> {
        require_described_as(&reference, HOLON_SPACE_TYPE_NAME)?;
        Ok(Self(reference))
    }

    /// Returns the underlying space holon reference.
    pub fn as_holon_reference(&self) -> &HolonReference {
        &self.0
    }
}

impl QueryReference {
    /// Wraps a holon reference after verifying it is described as `Query`.
    pub fn new(reference: HolonReference) -> Result<Self, HolonError> {
        require_described_as(&reference, QUERY_TYPE_NAME)?;
        Ok(Self(reference))
    }

    /// Creates the transient runtime records for one invocation without running it.
    ///
    /// `focal_space` is the invocation context, recorded on the `ExecutionInstance`
    /// only. `input` is the caller's explicit collection holon, linked as `Input`
    /// by identity when the root expression takes one (see the module docs for the
    /// per-kind contract). `bindings` are invocation-level `QueryParameterBinding`
    /// references; they are accepted and discarded (QRY3 owns their semantics).
    /// Both records start `Pending`.
    pub fn begin_execution(
        &self,
        context: &Arc<TransactionContext>,
        focal_space: FocalSpaceReference,
        input: Option<HolonCollectionReference>,
        bindings: Vec<HolonReference>,
    ) -> Result<QueryExecution, HolonError> {
        let _ = bindings; // carried unresolved; not recorded anywhere yet
        let root_expression = exactly_one(&self.0, QueryRelationshipTypeName::RootExpression)?;
        let root_kind = ExpressionKind::classify(&root_expression)?;
        let input = root_kind.validate_root_input(&root_expression, input)?;

        let mut instance =
            new_runtime_record(context, EXECUTION_INSTANCE_KEY, EXECUTION_INSTANCE_DESCRIPTOR_KEY)?;
        instance
            .add_related_holons(QueryRelationshipTypeName::ExecutesQuery, vec![self.0.clone()])?
            .add_related_holons(QueryRelationshipTypeName::FocalSpace, vec![focal_space.0])?;

        let mut root_execution = new_runtime_record(
            context,
            QUERY_EXPRESSION_EXECUTION_KEY,
            QUERY_EXPRESSION_EXECUTION_DESCRIPTOR_KEY,
        )?;
        root_execution.add_related_holons(
            QueryRelationshipTypeName::ExecutesExpression,
            vec![root_expression.clone()],
        )?;
        if let Some(input) = input {
            root_execution.add_related_holons(QueryRelationshipTypeName::Input, vec![input.0])?;
        }

        instance.add_related_holons(
            QueryRelationshipTypeName::ExpressionExecutions,
            vec![root_execution.clone().into()],
        )?;

        Ok(QueryExecution { instance, root_execution, root_expression, root_kind })
    }
}

/// Transient runtime state for one Query invocation.
#[derive(Debug)]
pub struct QueryExecution {
    instance: TransientReference,
    root_execution: TransientReference,
    root_expression: HolonReference,
    root_kind: ExpressionKind,
}

impl QueryExecution {
    /// The transient `ExecutionInstance` record.
    pub fn instance(&self) -> &TransientReference {
        &self.instance
    }

    /// The transient root `QueryExpressionExecution` record.
    pub fn root_execution(&self) -> &TransientReference {
        &self.root_execution
    }

    /// Executes the root expression and records its result.
    ///
    /// Both records progress to `Running`; on success the members are
    /// materialized into a transient `HolonCollection` holon linked as
    /// `QueryExpressionExecution.Result` and `ExecutionInstance.ExecutionResult`,
    /// both records become `Complete`, and that collection is returned. On any
    /// failure both records become `Failed`, no `Result` is written, and the
    /// error propagates unchanged.
    pub fn run(mut self) -> Result<HolonCollectionReference, HolonError> {
        set_status(&mut self.instance, ExecutionStatus::Running)?;
        set_status(&mut self.root_execution, ExecutionStatus::Running)?;

        match self.execute_root() {
            Ok(result) => {
                set_status(&mut self.root_execution, ExecutionStatus::Complete)?;
                set_status(&mut self.instance, ExecutionStatus::Complete)?;
                Ok(result)
            }
            Err(error) => {
                set_status(&mut self.root_execution, ExecutionStatus::Failed)?;
                set_status(&mut self.instance, ExecutionStatus::Failed)?;
                Err(error)
            }
        }
    }

    fn execute_root(&mut self) -> Result<HolonCollectionReference, HolonError> {
        // Chaining over `Next` is a later slice; refuse rather than silently
        // executing only the root of a longer expression.
        if zero_or_one(&self.root_expression, QueryRelationshipTypeName::Next)?.is_some() {
            return Err(HolonError::NotImplemented("QueryExpression chaining (Next)".to_string()));
        }

        let context = self.instance.bound_context();
        let members = match &self.root_kind {
            ExpressionKind::SeedHolons => seed_holons(&context, &self.instance.clone().into())?,
            ExpressionKind::Expand => {
                return Err(HolonError::NotImplemented("Expand (QRY2b)".to_string()))
            }
            ExpressionKind::Unsupported(type_name) => {
                return Err(HolonError::NotImplemented(format!(
                    "QueryExpression execution: {type_name}"
                )))
            }
        };

        let result = new_collection_holon(&context, QUERY_RESULT_KEY, members)?;
        let result_reference: HolonReference = result.into();
        self.root_execution.add_related_holons(
            QueryRelationshipTypeName::Result,
            vec![result_reference.clone()],
        )?;
        self.instance.add_related_holons(
            QueryRelationshipTypeName::ExecutionResult,
            vec![result_reference.clone()],
        )?;
        Ok(HolonCollectionReference(result_reference))
    }
}

/// Concrete expression kinds the runtime recognizes, read from the root
/// expression's descriptor type name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExpressionKind {
    SeedHolons,
    Expand,
    Unsupported(String),
}

impl ExpressionKind {
    fn classify(expression: &HolonReference) -> Result<Self, HolonError> {
        let type_name = expression.holon_descriptor()?.header().type_name()?.0;
        Ok(match type_name.as_str() {
            SEED_HOLONS_TYPE_NAME => Self::SeedHolons,
            EXPAND_TYPE_NAME => Self::Expand,
            _ => Self::Unsupported(type_name),
        })
    }

    /// Applies the root input contract: a source takes no collection operand, a
    /// transform requires one, and an unrecognized kind passes the operand through
    /// unchanged so it still reaches the `NotImplemented` boundary at run time.
    fn validate_root_input(
        &self,
        expression: &HolonReference,
        input: Option<HolonCollectionReference>,
    ) -> Result<Option<HolonCollectionReference>, HolonError> {
        match (self, input) {
            (Self::SeedHolons, Some(_)) => Err(HolonError::InvalidParameter(
                "SeedHolons is a source expression and accepts no input collection".to_string(),
            )),
            (Self::Expand, None) => Err(HolonError::MissingRequiredRelationship {
                relationship: QueryRelationshipTypeName::Input.to_relationship_name().to_string(),
                descriptor: expression.summarize()?,
            }),
            (_, input) => Ok(input),
        }
    }
}

/// `SeedHolons`: expands the focal space recorded on the execution instance over
/// its `Owns` relationship. The relationship is resolved through the space's
/// descriptor first so an unsupported or ambiguous name surfaces as the
/// descriptor's own error rather than as an empty result.
fn seed_holons(
    context: &Arc<TransactionContext>,
    instance: &HolonReference,
) -> Result<Vec<HolonReference>, HolonError> {
    let focal_space = exactly_one(instance, QueryRelationshipTypeName::FocalSpace)?;
    let owns = CoreRelationshipTypeName::Owns.to_relationship_name();
    expand_one(context, &focal_space, &owns)
}

/// Resolves `relationship_name` as effective outbound navigation from `source`'s
/// descriptor, then reads that relationship for `source` through the Holon
/// service. Returns the members in storage order, duplicates included.
fn expand_one(
    context: &Arc<TransactionContext>,
    source: &HolonReference,
    relationship_name: &RelationshipName,
) -> Result<Vec<HolonReference>, HolonError> {
    source.holon_descriptor()?.resolve_available_relationship(relationship_name.clone())?;
    let collection = context.fetch_related_holons(&source.holon_id()?, relationship_name)?;
    Ok(collection.get_members().clone())
}

/// Materializes `members` as a transient holon described by `HolonCollection`,
/// appended in the given order (duplicates retained).
fn new_collection_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    members: Vec<HolonReference>,
) -> Result<TransientReference, HolonError> {
    let mut collection = context.mutation().new_holon(Some(MapString(key.to_string())))?;
    collection
        .with_descriptor(resolve_core_descriptor(context, HOLON_COLLECTION_DESCRIPTOR_KEY)?)?;
    if !members.is_empty() {
        collection.add_related_holons(CoreRelationshipTypeName::CollectionMembers, members)?;
    }
    Ok(collection)
}

/// Lifecycle states the runtime records, mirrored from `QueryExecutionStatus.MapEnumValueType`.
#[derive(Debug, Clone, Copy)]
enum ExecutionStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

impl ExecutionStatus {
    fn as_enum_value(self) -> MapEnumValue {
        let variant = match self {
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Complete => "Complete",
            Self::Failed => "Failed",
        };
        MapEnumValue(MapString(variant.to_string()))
    }
}

fn new_runtime_record(
    context: &Arc<TransactionContext>,
    key: &str,
    descriptor_key: &str,
) -> Result<TransientReference, HolonError> {
    let mut record = context.mutation().new_holon(Some(MapString(key.to_string())))?;
    record.with_descriptor(resolve_core_descriptor(context, descriptor_key)?)?;
    set_status(&mut record, ExecutionStatus::Pending)?;
    Ok(record)
}

fn set_status(record: &mut TransientReference, status: ExecutionStatus) -> Result<(), HolonError> {
    record.with_property_value(QueryPropertyTypeName::ExecutionStatus, status.as_enum_value())?;
    Ok(())
}

fn require_described_as(holon: &HolonReference, expected: &str) -> Result<(), HolonError> {
    let found = holon.holon_descriptor()?.header().type_name()?;
    if found.0 != expected {
        return Err(HolonError::WrongDescriptorKind {
            expected: expected.to_string(),
            found: found.to_string(),
            descriptor: holon.summarize()?,
        });
    }
    Ok(())
}

pub(crate) fn related_members<T: ToRelationshipName>(
    holon: &HolonReference,
    relationship: T,
) -> Result<Vec<HolonReference>, HolonError> {
    let collection = holon.related_holons(relationship)?;
    let members = collection
        .read()
        .map_err(|error| HolonError::FailedToAcquireLock(format!("{error}")))?
        .get_members()
        .clone();
    Ok(members)
}

pub(crate) fn exactly_one<T: ToRelationshipName + Clone>(
    holon: &HolonReference,
    relationship: T,
) -> Result<HolonReference, HolonError> {
    let relationship_name = relationship.clone().to_relationship_name().to_string();
    zero_or_one(holon, relationship)?.ok_or_else(|| HolonError::MissingRequiredRelationship {
        relationship: relationship_name,
        descriptor: holon.summarize().unwrap_or_default(),
    })
}

pub(crate) fn zero_or_one<T: ToRelationshipName + Clone>(
    holon: &HolonReference,
    relationship: T,
) -> Result<Option<HolonReference>, HolonError> {
    let relationship_name = relationship.clone().to_relationship_name().to_string();
    let members = related_members(holon, relationship)?;
    match members.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(single.clone())),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: relationship_name,
            descriptor: holon.summarize()?,
            count: many.len(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use base_types::{BaseValue, MapInteger};
    use core_types::{HolonId, LocalId, PropertyMap};
    use type_names::ToPropertyName;

    use super::*;
    use crate::core_shared_objects::holon::SavedHolon;
    use crate::descriptors::test_support::{
        build_context, build_context_with_saved_holons, new_holon_type_descriptor, new_test_holon,
    };

    // ---- QRY1 constructor guards -------------------------------------------

    #[test]
    fn query_reference_rejects_non_query_descriptor() {
        let context = build_context();
        let not_query_descriptor =
            new_holon_type_descriptor(&context, "NotAQuery.HolonType", "NotAQuery").unwrap();
        let mut holon = new_test_holon(&context, "some-holon").unwrap();
        holon.with_descriptor(not_query_descriptor.into()).unwrap();

        let error = QueryReference::new(holon.into()).unwrap_err();
        assert!(
            matches!(&error, HolonError::WrongDescriptorKind { expected, found, .. }
                if expected == "Query" && found == "NotAQuery"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn holon_collection_reference_rejects_non_collection_descriptor() {
        let context = build_context();
        let not_collection_descriptor =
            new_holon_type_descriptor(&context, "NotACollection.HolonType", "NotACollection")
                .unwrap();
        let mut holon = new_test_holon(&context, "some-holon").unwrap();
        holon.with_descriptor(not_collection_descriptor.into()).unwrap();

        let error = HolonCollectionReference::new(holon.into()).unwrap_err();
        assert!(
            matches!(&error, HolonError::WrongDescriptorKind { expected, found, .. }
                if expected == "HolonCollection" && found == "NotACollection"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn holon_collection_reference_accepts_collection_descriptor() {
        let context = build_context();
        let collection_descriptor =
            new_holon_type_descriptor(&context, "HolonCollection.HolonType", "HolonCollection")
                .unwrap();
        let mut holon = new_test_holon(&context, "a-collection").unwrap();
        holon.with_descriptor(collection_descriptor.into()).unwrap();

        assert!(HolonCollectionReference::new(holon.into()).is_ok());
    }

    #[test]
    fn query_reference_accepts_query_descriptor() {
        let context = build_context();
        let query_descriptor =
            new_holon_type_descriptor(&context, "Query.HolonType", "Query").unwrap();
        let mut holon = new_test_holon(&context, "a-query").unwrap();
        holon.with_descriptor(query_descriptor.into()).unwrap();

        assert!(QueryReference::new(holon.into()).is_ok());
    }

    #[test]
    fn focal_space_reference_rejects_non_space_descriptor() {
        let context = build_context();
        let descriptor =
            new_holon_type_descriptor(&context, "NotASpace.HolonType", "NotASpace").unwrap();
        let mut holon = new_test_holon(&context, "some-holon").unwrap();
        holon.with_descriptor(descriptor.into()).unwrap();

        let error = FocalSpaceReference::new(holon.into()).unwrap_err();
        assert!(
            matches!(&error, HolonError::WrongDescriptorKind { expected, found, .. }
                if expected == "HolonSpace" && found == "NotASpace"),
            "unexpected error: {error:?}"
        );
    }

    // ---- QRY2a execution fixture -------------------------------------------
    //
    // Saved graph (ids are LocalId byte values):
    //   1  space            DescribedBy -> 2 ; Owns -> [10, 11, 12, 11]  (duplicate on purpose)
    //   2  HolonSpace type  SourceOf   -> [3]  (materialized inverse index)
    //   3  Owns inverse     Extends    -> [4]
    //   4  InverseRelationshipType
    //   5  ExecutionInstance.HolonType, 6 QueryExpressionExecution.HolonType,
    //   7  HolonCollection.HolonType   (resolved by key for the runtime records)
    //   10, 11, 12  owned holons
    // Definitions (Query, SeedHolons, Expand, …) are transient, described by
    // transient descriptors: the runtime classifies by descriptor type name only.

    fn id(value: u8) -> HolonId {
        HolonId::Local(LocalId(vec![value; 39]))
    }

    fn properties(values: &[(&str, &str)]) -> PropertyMap {
        values
            .iter()
            .map(|(name, value)| {
                (name.to_property_name(), BaseValue::StringValue(MapString((*value).to_owned())))
            })
            .collect()
    }

    fn rel(source: u8, name: CoreRelationshipTypeName) -> (HolonId, RelationshipName) {
        (id(source), name.to_relationship_name())
    }

    struct Fixture {
        context: Arc<TransactionContext>,
        space: HolonReference,
    }

    fn build_fixture() -> Fixture {
        let snapshots = [
            (1, properties(&[("Key", "space")])),
            (2, properties(&[("Key", "HolonSpace.HolonType"), ("TypeName", "HolonSpace")])),
            (3, properties(&[("Key", "Owns.Inverse"), ("TypeName", "Owns")])),
            (
                4,
                properties(&[
                    ("Key", "InverseRelationshipType.RelationshipType"),
                    ("TypeName", "InverseRelationshipType"),
                ]),
            ),
            (
                5,
                properties(&[
                    ("Key", EXECUTION_INSTANCE_DESCRIPTOR_KEY),
                    ("TypeName", "ExecutionInstance"),
                ]),
            ),
            (
                6,
                properties(&[
                    ("Key", QUERY_EXPRESSION_EXECUTION_DESCRIPTOR_KEY),
                    ("TypeName", "QueryExpressionExecution"),
                ]),
            ),
            (
                7,
                properties(&[
                    ("Key", HOLON_COLLECTION_DESCRIPTOR_KEY),
                    ("TypeName", HOLON_COLLECTION_TYPE_NAME),
                ]),
            ),
            (10, properties(&[("Key", "owned-a")])),
            (11, properties(&[("Key", "owned-b")])),
            (12, properties(&[("Key", "owned-c")])),
        ]
        .into_iter()
        .map(|(value, properties)| {
            SavedHolon::new(LocalId(vec![value; 39]), properties, None, MapInteger(1))
        })
        .collect();
        let relationships = HashMap::from([
            (rel(1, CoreRelationshipTypeName::DescribedBy), vec![id(2)]),
            (rel(1, CoreRelationshipTypeName::Owns), vec![id(10), id(11), id(12), id(11)]),
            (rel(2, CoreRelationshipTypeName::SourceOf), vec![id(3)]),
            (rel(3, CoreRelationshipTypeName::Extends), vec![id(4)]),
        ]);
        let context = build_context_with_saved_holons(snapshots, relationships);
        let space = HolonReference::smart_from_id(context.space_read_handle(), id(1));
        Fixture { context, space }
    }

    impl Fixture {
        fn focal_space(&self) -> FocalSpaceReference {
            FocalSpaceReference::new(self.space.clone()).expect("saved space is a HolonSpace")
        }

        /// A transient holon described by a transient descriptor with `type_name`.
        fn described(&self, key: &str, type_name: &str) -> TransientReference {
            let descriptor = new_holon_type_descriptor(
                &self.context,
                &format!("{type_name}.HolonType/{key}"),
                type_name,
            )
            .unwrap();
            let mut holon = new_test_holon(&self.context, key).unwrap();
            holon.with_descriptor(descriptor.into()).unwrap();
            holon
        }

        fn query_with_root(&self, root: &TransientReference) -> QueryReference {
            let mut query = self.described("query", QUERY_TYPE_NAME);
            query
                .add_related_holons(QueryRelationshipTypeName::RootExpression, vec![root.into()])
                .unwrap();
            QueryReference::new(query.into()).unwrap()
        }

        fn collection(&self, key: &str) -> HolonCollectionReference {
            let holon = self.described(key, HOLON_COLLECTION_TYPE_NAME);
            HolonCollectionReference::new(holon.into()).unwrap()
        }
    }

    fn status_of(record: &TransientReference) -> String {
        match record.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap() {
            Some(BaseValue::EnumValue(value)) => value.0 .0,
            other => panic!("unexpected status value: {other:?}"),
        }
    }

    fn ids_of(members: &[HolonReference]) -> Vec<HolonId> {
        members.iter().map(|member| member.holon_id().unwrap()).collect()
    }

    #[test]
    fn seed_holons_root_expands_owns_in_order_with_duplicates() {
        let fixture = build_fixture();
        let seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        let query = fixture.query_with_root(&seed);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .expect("SeedHolons root begins without input");
        let instance: HolonReference = execution.instance().clone().into();
        let root_execution: HolonReference = execution.root_execution().clone().into();
        assert_eq!(status_of(execution.instance()), "Pending");
        assert_eq!(
            ids_of(&related_members(&instance, QueryRelationshipTypeName::FocalSpace).unwrap()),
            vec![id(1)],
            "focal space is recorded on the instance"
        );
        assert!(
            related_members(&root_execution, QueryRelationshipTypeName::Input).unwrap().is_empty(),
            "a source root records no Input"
        );

        let result = execution.run().expect("SeedHolons executes");

        let members = related_members(
            result.as_holon_reference(),
            CoreRelationshipTypeName::CollectionMembers,
        )
        .unwrap();
        assert_eq!(ids_of(&members), vec![id(10), id(11), id(12), id(11)]);
        let recorded_result =
            exactly_one(&root_execution, QueryRelationshipTypeName::Result).unwrap();
        assert_eq!(
            recorded_result.summarize().unwrap(),
            result.as_holon_reference().summarize().unwrap(),
            "Result links the returned collection holon by identity"
        );
        let execution_result =
            exactly_one(&instance, QueryRelationshipTypeName::ExecutionResult).unwrap();
        assert_eq!(
            execution_result.summarize().unwrap(),
            result.as_holon_reference().summarize().unwrap()
        );
        assert_eq!(
            instance.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value()))
        );
        assert_eq!(
            root_execution.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value()))
        );
        // The definitions carry no runtime state.
        assert!(related_members(&query.0, QueryRelationshipTypeName::FocalSpace)
            .unwrap()
            .is_empty());
        assert!(seed.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap().is_none());
    }

    #[test]
    fn seed_holons_rejects_supplied_input() {
        let fixture = build_fixture();
        let seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        let query = fixture.query_with_root(&seed);

        let error = query
            .begin_execution(
                &fixture.context,
                fixture.focal_space(),
                Some(fixture.collection("caller-input")),
                Vec::new(),
            )
            .unwrap_err();
        assert!(matches!(error, HolonError::InvalidParameter(_)), "unexpected error: {error:?}");
    }

    #[test]
    fn expand_root_requires_input() {
        let fixture = build_fixture();
        let expand = fixture.described("expand", EXPAND_TYPE_NAME);
        let query = fixture.query_with_root(&expand);

        let error = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .unwrap_err();
        assert!(
            matches!(&error, HolonError::MissingRequiredRelationship { relationship, .. }
                if relationship == "Input"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn next_on_root_is_not_implemented() {
        let fixture = build_fixture();
        let mut seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        let next = fixture.described("next", EXPAND_TYPE_NAME);
        seed.add_related_holons(QueryRelationshipTypeName::Next, vec![next.into()]).unwrap();
        let query = fixture.query_with_root(&seed);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .unwrap();
        let instance = execution.instance().clone();
        let root_execution = execution.root_execution().clone();

        let error = execution.run().unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
        assert_eq!(status_of(&instance), "Failed");
        assert_eq!(status_of(&root_execution), "Failed");
        assert!(related_members(
            &HolonReference::from(root_execution),
            QueryRelationshipTypeName::Result
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn unsupported_expression_still_not_implemented() {
        let fixture = build_fixture();
        let unknown = fixture.described("unknown", "UnimplementedQueryExpression");
        let query = fixture.query_with_root(&unknown);

        let execution = query
            .begin_execution(
                &fixture.context,
                fixture.focal_space(),
                Some(fixture.collection("caller-input")),
                Vec::new(),
            )
            .expect("unknown kinds pass the operand through to the run-time boundary");
        let instance = execution.instance().clone();

        let error = execution.run().unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
        assert_eq!(status_of(&instance), "Failed");
    }

    #[test]
    fn expand_root_is_not_implemented_until_qry2b() {
        let fixture = build_fixture();
        let expand = fixture.described("expand", EXPAND_TYPE_NAME);
        let query = fixture.query_with_root(&expand);

        let execution = query
            .begin_execution(
                &fixture.context,
                fixture.focal_space(),
                Some(fixture.collection("caller-input")),
                Vec::new(),
            )
            .unwrap();
        let error = execution.run().unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
    }

    #[test]
    fn execution_status_enum_values_match_schema_variants() {
        let expected = [
            (ExecutionStatus::Pending, "Pending"),
            (ExecutionStatus::Running, "Running"),
            (ExecutionStatus::Complete, "Complete"),
            (ExecutionStatus::Failed, "Failed"),
        ];
        for (status, variant) in expected {
            assert_eq!(status.as_enum_value().0 .0, variant);
        }
    }
}
