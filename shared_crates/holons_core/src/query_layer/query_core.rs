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
//! - execution of the root expression and each `Next` successor in turn —
//!   `SeedHolons` (root only) and `Expand` — with `HolonError::NotImplemented`
//!   for every other concrete kind.
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
//! Read boundary: operators resolve the requested relationship through the
//! source member's `HolonDescriptor` (declared or inverse navigation) and then
//! read that relationship through the source reference's ordinary
//! `related_holons` operation, so relationship-cache eligibility, TTL, and any
//! explicit caller freshness requirement govern reuse. The read preserves
//! member order and duplicate occurrences, and the collection's keyed index is
//! never used to deduplicate.
//!
//! Predicate contract: `SeedHolons.SeedPredicate` and `Expand.ExpansionPredicate`
//! are attachment points whose evaluation is a later slice. Until filtered
//! expansion exists, an attached predicate is refused with
//! `HolonError::NotImplemented` rather than silently answered with unfiltered
//! members. Both operators apply the same rule.

use std::collections::HashSet;
use std::sync::Arc;

use base_types::{BaseValue, MapEnumValue, MapString};
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
const QUERY_INPUT_COLLECTION_KEY: &str = "query-input-collection";

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

    /// Inflates the runtime member view of this collection holon.
    ///
    /// Only operators that iterate call this; the holon itself stays the
    /// identity carrier across every boundary. Members come back in authored
    /// order with duplicate occurrences intact.
    pub fn members(&self) -> Result<Vec<HolonReference>, HolonError> {
        related_members(&self.0, CoreRelationshipTypeName::CollectionMembers)
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
            &format!("{QUERY_EXPRESSION_EXECUTION_KEY}-0"),
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

        Ok(QueryExecution {
            instance,
            executions: vec![root_execution],
            root_expression,
            root_kind,
        })
    }
}

/// Explicit direct-invocation convenience for a single source holon.
///
/// Normalizes `source` into a transient singleton `HolonCollection` holon and
/// delegates to the canonical collection-reference path, so QueryCore and every
/// expression continue to consume collection references only. Deliberately not
/// a `From<HolonReference>` conversion and not available on the Dance route:
/// the wire/schema contract stays collection-shaped.
impl QueryReference {
    pub fn begin_execution_for_holon(
        &self,
        context: &Arc<TransactionContext>,
        focal_space: FocalSpaceReference,
        source: HolonReference,
        bindings: Vec<HolonReference>,
    ) -> Result<QueryExecution, HolonError> {
        let collection = new_collection_holon(context, QUERY_INPUT_COLLECTION_KEY, vec![source])?;
        let input = HolonCollectionReference(collection.into());
        self.begin_execution(context, focal_space, Some(input), bindings)
    }
}

/// Transient runtime state for one Query invocation.
#[derive(Debug)]
pub struct QueryExecution {
    instance: TransientReference,
    /// One record per executed step, in chain order. The root is `executions[0]`;
    /// later entries are created as the `Next` walk reaches them.
    executions: Vec<TransientReference>,
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
        &self.executions[0]
    }

    /// The transient `QueryExpressionExecution` records in chain order.
    pub fn executions(&self) -> &[TransientReference] {
        &self.executions
    }

    /// Executes the root expression and each `Next` successor in turn.
    ///
    /// Every record progresses to `Running` as its step begins. On success each
    /// step's members are materialized into a transient `HolonCollection` holon
    /// linked as that step's `Result`, the step becomes `Complete`, and the
    /// holon becomes the next step's `Input` by identity. The final step's
    /// result is also linked as `ExecutionInstance.ExecutionResult` and
    /// returned. On any failure the failing step and the instance become
    /// `Failed`, no `Result` is written for that step, no `ExecutionResult` is
    /// recorded, later steps are never created, and the error propagates
    /// unchanged.
    pub fn run(mut self) -> Result<HolonCollectionReference, HolonError> {
        set_status(&mut self.instance, ExecutionStatus::Running)?;

        match self.execute_chain() {
            Ok(result) => {
                set_status(&mut self.instance, ExecutionStatus::Complete)?;
                Ok(result)
            }
            Err(error) => {
                // The failing step is the last one created; earlier steps keep
                // the `Complete` they legitimately reached.
                if let Some(step) = self.executions.last_mut() {
                    set_status(step, ExecutionStatus::Failed)?;
                }
                set_status(&mut self.instance, ExecutionStatus::Failed)?;
                Err(error)
            }
        }
    }

    fn execute_chain(&mut self) -> Result<HolonCollectionReference, HolonError> {
        let context = self.instance.bound_context();
        let mut expression = self.root_expression.clone();
        let mut kind = self.root_kind.clone();
        // `Next` cardinality permits a cycle (`A -Next-> B -Next-> A` satisfies
        // both `Next` and `Previous` as ZeroOrOne), which no schema or commit
        // check rejects. Terminate on the repeated expression rather than
        // looping forever; this is not a result-size guard — fan-out across a
        // chain is expected until the predicate/operator track lands.
        let mut visited = HashSet::new();
        visited.insert(expression.reference_id_string());

        loop {
            let step = self.executions.len() - 1;
            set_status(&mut self.executions[step], ExecutionStatus::Running)?;

            // Position is a contract violation independent of any predicate, so
            // it is reported first.
            if matches!(kind, ExpressionKind::SeedHolons) && step > 0 {
                return Err(HolonError::InvalidParameter(
                    "SeedHolons is a source expression and must be the root".to_string(),
                ));
            }
            // Every step is checked, not just the root: a predicate attached to a
            // chained successor must be refused too.
            reject_attached_predicates(&expression)?;

            let members = match &kind {
                ExpressionKind::SeedHolons => seed_holons(&self.instance.clone().into())?,
                ExpressionKind::Expand => {
                    // Root: the caller's collection. Non-root: the predecessor's
                    // result, linked when this record was created.
                    let input = HolonCollectionReference(exactly_one(
                        &self.executions[step].clone().into(),
                        QueryRelationshipTypeName::Input,
                    )?);
                    expand(&input.members()?, &expansion_name(&expression)?)?
                }
                ExpressionKind::Unsupported(type_name) => {
                    return Err(HolonError::NotImplemented(format!(
                        "QueryExpression execution: {type_name}"
                    )))
                }
            };

            let result =
                new_collection_holon(&context, &format!("{QUERY_RESULT_KEY}-{step}"), members)?;
            let result_reference: HolonReference = result.into();
            self.executions[step].add_related_holons(
                QueryRelationshipTypeName::Result,
                vec![result_reference.clone()],
            )?;
            set_status(&mut self.executions[step], ExecutionStatus::Complete)?;

            let Some(next) = zero_or_one(&expression, QueryRelationshipTypeName::Next)? else {
                self.instance.add_related_holons(
                    QueryRelationshipTypeName::ExecutionResult,
                    vec![result_reference.clone()],
                )?;
                return Ok(HolonCollectionReference(result_reference));
            };
            if !visited.insert(next.reference_id_string()) {
                return Err(HolonError::InvalidParameter(format!(
                    "QueryExpression chain revisits {} through Next; the expression graph is cyclic",
                    next.summarize()?
                )));
            }

            kind = ExpressionKind::classify(&next)?;
            let mut record = new_runtime_record(
                &context,
                &format!("{QUERY_EXPRESSION_EXECUTION_KEY}-{}", step + 1),
                QUERY_EXPRESSION_EXECUTION_DESCRIPTOR_KEY,
            )?;
            record
                .add_related_holons(
                    QueryRelationshipTypeName::ExecutesExpression,
                    vec![next.clone()],
                )?
                .add_related_holons(QueryRelationshipTypeName::Input, vec![result_reference])?;
            self.instance.add_related_holons(
                QueryRelationshipTypeName::ExpressionExecutions,
                vec![record.clone().into()],
            )?;
            self.executions.push(record);
            expression = next;
        }
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
/// its `Owns` relationship, unfiltered. The relationship is resolved through the
/// space's descriptor first so an unsupported or ambiguous name surfaces as the
/// descriptor's own error rather than as an empty result.
///
/// This is a thin wrapper: it supplies the focal space and `Owns`, and otherwise
/// shares [`expand_one`] with `Expand`. A `SeedPredicate` selects filtered
/// expansion, which does not exist yet; [`reject_attached_predicates`] refuses it
/// before this runs.
fn seed_holons(instance: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    let focal_space = exactly_one(instance, QueryRelationshipTypeName::FocalSpace)?;
    let owns = CoreRelationshipTypeName::Owns.to_relationship_name();
    expand_one(&focal_space, &owns)
}

/// Unfiltered expansion. Resolves `relationship_name` as effective outbound
/// navigation from `source`'s descriptor, then reads that relationship through
/// `source`'s ordinary `related_holons` operation, which applies the established
/// relationship-cache policy. Returns the members in read order, duplicates
/// included.
fn expand_one(
    source: &HolonReference,
    relationship_name: &RelationshipName,
) -> Result<Vec<HolonReference>, HolonError> {
    source.holon_descriptor()?.resolve_available_relationship(relationship_name.clone())?;
    related_members(source, relationship_name)
}

/// Refuses an attached predicate instead of answering with unfiltered members.
///
/// Predicate evaluation is a later slice. Returning the unfiltered result would
/// silently ignore a caller-supplied filter, so the operator fails and its
/// execution records stay `Failed` with no `Result`.
///
/// Refusal is a property of the expression, not of its operator kind: both
/// attachment points are checked whatever the kind, because a predicate on the
/// "wrong" operator is ill-formed per schema but still constructible on a
/// transient definition, and would otherwise pass through unfiltered.
fn reject_attached_predicates(expression: &HolonReference) -> Result<(), HolonError> {
    reject_attached_predicate(expression, QueryRelationshipTypeName::SeedPredicate)?;
    reject_attached_predicate(expression, QueryRelationshipTypeName::ExpansionPredicate)
}

/// Tests presence rather than cardinality, so several attached predicates
/// report the unimplemented feature instead of a `MultipleRelatedHolons`.
fn reject_attached_predicate<T: ToRelationshipName + Clone>(
    expression: &HolonReference,
    predicate_relationship: T,
) -> Result<(), HolonError> {
    let relationship_name = predicate_relationship.clone().to_relationship_name().to_string();
    if !related_members(expression, predicate_relationship)?.is_empty() {
        return Err(HolonError::NotImplemented(format!(
            "predicate evaluation ({relationship_name})"
        )));
    }
    Ok(())
}

/// `Expand`: navigates every source member over `relationship_name`.
///
/// Each member resolves the name through its own `HolonDescriptor`, so declared
/// and inverse navigation are both available and the descriptor's errors
/// (`DescriptorDeclarationNotFound`, `AmbiguousRelationshipTraversal`,
/// `UnsupportedStagedTraversal`) propagate unchanged. There is no whole-collection
/// preflight: the first failing member fails the execution. Results append in
/// source-traversal order then storage order; a member with no targets
/// contributes nothing and is not an error.
///
/// Unfiltered, like [`expand_one`] it delegates to: an attached
/// `ExpansionPredicate` is refused by the operator before this is reached.
fn expand(
    sources: &[HolonReference],
    relationship_name: &RelationshipName,
) -> Result<Vec<HolonReference>, HolonError> {
    let mut members = Vec::new();
    for source in sources {
        members.extend(expand_one(source, relationship_name)?);
    }
    Ok(members)
}

/// Reads the relationship name an `Expand` definition navigates.
fn expansion_name(expression: &HolonReference) -> Result<RelationshipName, HolonError> {
    match expression.property_value(QueryPropertyTypeName::ExpansionRelationshipName)? {
        Some(BaseValue::StringValue(name)) => Ok(RelationshipName(name)),
        Some(other) => {
            Err(HolonError::UnexpectedValueType(format!("{other:?}"), "String".to_string()))
        }
        None => Err(HolonError::EmptyField(
            QueryPropertyTypeName::ExpansionRelationshipName.as_property_name().to_string(),
        )),
    }
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

    use base_types::{BaseValue, MapBoolean, MapInteger};
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

    // ---- QRY2 execution fixture ---------------------------------------------
    //
    // Saved graph (ids are LocalId byte values):
    //   1  space            DescribedBy -> 2 ; Owns -> [10, 11, 12, 11]  (duplicate on purpose)
    //   2  HolonSpace type  SourceOf   -> [3]  (materialized inverse index)
    //   3  Owns inverse     Extends    -> [4]
    //   4  InverseRelationshipType
    //   5  ExecutionInstance.HolonType, 6 QueryExpressionExecution.HolonType,
    //   7  HolonCollection.HolonType   (resolved by key for the runtime records)
    //   10, 11, 12  owned holons, described as Books so a chain can navigate
    //               off them: 10 AuthoredBy -> [24], 11 -> [29], 12 -> none
    //
    // Expand graph — a declared name licensed on the source type and an inverse
    // name reached through the target type's materialized SourceOf index:
    //   20 book-a   DescribedBy -> 21 ; AuthoredBy -> [24, 29]
    //   27 book-b   DescribedBy -> 21 ; AuthoredBy -> [24]
    //   28 book-c   DescribedBy -> 21 ; (no AuthoredBy occurrence — legal empty)
    //   21 BookType      InstanceRelationships -> [22]
    //   22 AuthoredBy declared   Extends -> [23]
    //   23 DeclaredRelationshipType
    //   24 person-1  DescribedBy -> 25 ; AuthorOf -> [20, 27]
    //   29 person-2  DescribedBy -> 25
    //   25 PersonType     SourceOf -> [26]
    //   26 AuthorOf inverse       Extends -> [4]
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

    /// Like [`properties`], plus boolean entries. A declared `RelationshipType`
    /// descriptor must carry `IsDefinitional`: the cache-policy classifier reads
    /// it to choose between `Reuse` and an age-bounded policy, so a declared name
    /// read through the ordinary relationship path fails without it.
    ///
    /// Values here mirror the real descriptor being modelled, not
    /// `descriptors::test_support`'s generic `false`. `AuthoredBy` is
    /// `IsDefinitional: true` in `generated/json-imports/test/`, so the fixture
    /// exercises the same `Reuse` policy production does. Do not "normalize" it
    /// to `false`: that silently moves these tests onto the `Fresh` path and
    /// leaves declared-relationship cache reuse untested.
    fn properties_with_flags(values: &[(&str, &str)], flags: &[(&str, bool)]) -> PropertyMap {
        let mut map = properties(values);
        for (name, value) in flags {
            map.insert(name.to_property_name(), BaseValue::BooleanValue(MapBoolean(*value)));
        }
        map
    }

    fn rel(source: u8, name: CoreRelationshipTypeName) -> (HolonId, RelationshipName) {
        (id(source), name.to_relationship_name())
    }

    fn named(source: u8, name: &str) -> (HolonId, RelationshipName) {
        (id(source), RelationshipName(MapString(name.to_string())))
    }

    fn authored_by(source: u8) -> (HolonId, RelationshipName) {
        named(source, "AuthoredBy")
    }

    fn author_of(source: u8) -> (HolonId, RelationshipName) {
        named(source, "AuthorOf")
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
            (20, properties(&[("Key", "book-a")])),
            (21, properties(&[("Key", "Book.HolonType"), ("TypeName", "Book")])),
            (
                22,
                properties_with_flags(
                    &[("Key", "AuthoredBy.Declared"), ("TypeName", "AuthoredBy")],
                    &[("IsDefinitional", true)],
                ),
            ),
            (
                23,
                properties(&[
                    ("Key", "DeclaredRelationshipType.RelationshipType"),
                    ("TypeName", "DeclaredRelationshipType"),
                ]),
            ),
            (24, properties(&[("Key", "person-1")])),
            (25, properties(&[("Key", "Person.HolonType"), ("TypeName", "Person")])),
            (26, properties(&[("Key", "AuthorOf.Inverse"), ("TypeName", "AuthorOf")])),
            (27, properties(&[("Key", "book-b")])),
            (28, properties(&[("Key", "book-c")])),
            (30, properties(&[("Key", "book-d")])),
            (29, properties(&[("Key", "person-2")])),
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
            // Owned holons are Books, so `SeedHolons -Next-> Expand` is navigable.
            (rel(10, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (rel(11, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (rel(12, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (authored_by(10), vec![id(24)]),
            (authored_by(11), vec![id(29)]),
            // Expand: declared AuthoredBy licensed on BookType
            (rel(20, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (rel(27, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (rel(28, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (rel(30, CoreRelationshipTypeName::DescribedBy), vec![id(21)]),
            (rel(21, CoreRelationshipTypeName::InstanceRelationships), vec![id(22)]),
            (rel(22, CoreRelationshipTypeName::Extends), vec![id(23)]),
            (authored_by(20), vec![id(24), id(29)]),
            (authored_by(27), vec![id(24)]),
            // book-d repeats person-1: a duplicate *within one* membership entry, so a
            // second read served from the cache must reproduce it in place.
            (authored_by(30), vec![id(24), id(29), id(24)]),
            // Expand: inverse AuthorOf reached through PersonType's SourceOf index
            (rel(24, CoreRelationshipTypeName::DescribedBy), vec![id(25)]),
            (rel(29, CoreRelationshipTypeName::DescribedBy), vec![id(25)]),
            (rel(25, CoreRelationshipTypeName::SourceOf), vec![id(26)]),
            (rel(26, CoreRelationshipTypeName::Extends), vec![id(4)]),
            (author_of(24), vec![id(20), id(27)]),
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

        /// A saved holon in the fixture graph.
        fn saved(&self, value: u8) -> HolonReference {
            HolonReference::smart_from_id(self.context.space_read_handle(), id(value))
        }

        /// A collection holon whose members are the given saved holons, in order.
        fn collection_of(&self, key: &str, members: &[u8]) -> HolonCollectionReference {
            let mut holon = self.described(key, HOLON_COLLECTION_TYPE_NAME);
            let members: Vec<HolonReference> =
                members.iter().map(|value| self.saved(*value)).collect();
            if !members.is_empty() {
                holon
                    .add_related_holons(CoreRelationshipTypeName::CollectionMembers, members)
                    .unwrap();
            }
            HolonCollectionReference::new(holon.into()).unwrap()
        }

        /// An `Expand` definition navigating `relationship_name`.
        fn expand(&self, key: &str, relationship_name: &str) -> TransientReference {
            let mut expression = self.described(key, EXPAND_TYPE_NAME);
            expression
                .with_property_value(
                    QueryPropertyTypeName::ExpansionRelationshipName,
                    MapString(relationship_name.to_string()),
                )
                .unwrap();
            expression
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

    /// Links `expression -Next-> next`.
    fn chain(expression: &mut TransientReference, next: &TransientReference) {
        expression.add_related_holons(QueryRelationshipTypeName::Next, vec![next.into()]).unwrap();
    }

    #[test]
    fn next_chain_threads_each_result_into_the_successor_input() {
        let fixture = build_fixture();
        // SeedHolons(Owns) -> Expand(AuthoredBy). Owns is [10, 11, 12, 11] —
        // 12 has no author and 11 appears twice, so the duplicate propagates.
        let mut seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        let expand = fixture.expand("expand", "AuthoredBy");
        chain(&mut seed, &expand);
        let query = fixture.query_with_root(&seed);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .unwrap();
        let instance: HolonReference = execution.instance().clone().into();
        let result = execution.run().unwrap();

        // Two records, in chain order, both Complete.
        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 2, "one record per step");
        for record in &records {
            assert_eq!(
                record.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
                Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value()))
            );
        }

        // The successor consumes its predecessor's Result holon by identity.
        let first_result = exactly_one(&records[0], QueryRelationshipTypeName::Result).unwrap();
        let second_input = exactly_one(&records[1], QueryRelationshipTypeName::Input).unwrap();
        assert_eq!(
            second_input.reference_id_string(),
            first_result.reference_id_string(),
            "non-root Input IS the predecessor's Result holon"
        );

        // ExecutionResult is the last step's result, not the first's.
        let second_result = exactly_one(&records[1], QueryRelationshipTypeName::Result).unwrap();
        let execution_result =
            exactly_one(&instance, QueryRelationshipTypeName::ExecutionResult).unwrap();
        assert_eq!(execution_result.reference_id_string(), second_result.reference_id_string());
        assert_eq!(
            result.as_holon_reference().reference_id_string(),
            second_result.reference_id_string()
        );

        let members = related_members(
            result.as_holon_reference(),
            CoreRelationshipTypeName::CollectionMembers,
        )
        .unwrap();
        assert_eq!(
            ids_of(&members),
            vec![id(24), id(29), id(29)],
            "10 -> person-1, 11 -> person-2, 12 -> none, 11 again -> person-2"
        );
    }

    #[test]
    fn next_chain_runs_three_steps() {
        let fixture = build_fixture();
        // Expand(AuthoredBy) -> Expand(AuthorOf) -> Expand(AuthoredBy):
        // book-a's authors, their books, then those books' authors.
        let mut first = fixture.expand("first", "AuthoredBy");
        let mut second = fixture.expand("second", "AuthorOf");
        let third = fixture.expand("third", "AuthoredBy");
        chain(&mut second, &third);
        chain(&mut first, &second);
        let query = fixture.query_with_root(&first);
        let input = fixture.collection_of("chain-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance: HolonReference = execution.instance().clone().into();
        let result = execution.run().unwrap();

        assert_eq!(
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions)
                .unwrap()
                .len(),
            3
        );
        // [person-1, person-2] -> person-1 authors [book-a, book-b], person-2
        // has no AuthorOf occurrence -> [person-1, person-2, person-1].
        let members = related_members(
            result.as_holon_reference(),
            CoreRelationshipTypeName::CollectionMembers,
        )
        .unwrap();
        assert_eq!(ids_of(&members), vec![id(24), id(29), id(24)]);
    }

    #[test]
    fn seed_holons_off_root_is_a_contract_error() {
        let fixture = build_fixture();
        let mut expand = fixture.expand("expand", "AuthoredBy");
        let seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        chain(&mut expand, &seed);
        let query = fixture.query_with_root(&expand);
        let input = fixture.collection_of("chain-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance = execution.instance().clone();

        let error = execution.run().unwrap_err();
        assert!(matches!(error, HolonError::InvalidParameter(_)), "unexpected error: {error:?}");
        assert_eq!(status_of(&instance), "Failed");
        assert!(related_members(
            &HolonReference::from(instance),
            QueryRelationshipTypeName::ExecutionResult
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn mid_chain_failure_keeps_earlier_steps_complete_and_records_no_result() {
        let fixture = build_fixture();
        let mut first = fixture.expand("first", "AuthoredBy");
        let second = fixture.expand("second", "NoSuchRelationship");
        chain(&mut first, &second);
        let query = fixture.query_with_root(&first);
        let input = fixture.collection_of("chain-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance: HolonReference = execution.instance().clone().into();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. }
                if name == "NoSuchRelationship"),
            "unexpected error: {error:?}"
        );

        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 2, "the failing step exists; no step after it was created");
        assert_eq!(
            records[0].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value())),
            "a step that legitimately completed keeps Complete"
        );
        assert_eq!(
            records[1].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Failed.as_enum_value()))
        );
        assert!(related_members(&records[1], QueryRelationshipTypeName::Result)
            .unwrap()
            .is_empty());
        assert!(related_members(&instance, QueryRelationshipTypeName::ExecutionResult)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn predicate_on_a_chained_successor_is_not_implemented() {
        let fixture = build_fixture();
        // The root carries no predicate and would succeed on its own; only the
        // successor carries one. A root-only check would run the whole chain and
        // return a result that silently ignored the filter.
        let mut first = fixture.expand("first", "AuthoredBy");
        let mut second = fixture.expand("second", "AuthorOf");
        let predicate = fixture.described("predicate", "QueryPredicate");
        second
            .add_related_holons(
                QueryRelationshipTypeName::ExpansionPredicate,
                vec![predicate.into()],
            )
            .unwrap();
        chain(&mut first, &second);
        let query = fixture.query_with_root(&first);
        let input = fixture.collection_of("chain-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance: HolonReference = execution.instance().clone().into();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail)
                if detail.contains("ExpansionPredicate")),
            "a predicate on a chained successor is refused, not ignored: {error:?}"
        );

        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 2, "the refused step exists; no step after it was created");
        assert_eq!(
            records[0].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value())),
            "the predicate-free root step still completed"
        );
        assert_eq!(
            records[1].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Failed.as_enum_value()))
        );
        assert!(related_members(&records[1], QueryRelationshipTypeName::Result)
            .unwrap()
            .is_empty());
        assert!(related_members(&instance, QueryRelationshipTypeName::ExecutionResult)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn cyclic_next_chain_terminates_with_a_contract_error() {
        let fixture = build_fixture();
        // `Next`/`Previous` are both ZeroOrOne, so A -> B -> A is structurally
        // legal; the walk must terminate rather than loop forever.
        let mut first = fixture.expand("first", "AuthoredBy");
        let mut second = fixture.expand("second", "AuthorOf");
        chain(&mut second, &first);
        chain(&mut first, &second);
        let query = fixture.query_with_root(&first);
        let input = fixture.collection_of("chain-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance = execution.instance().clone();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::InvalidParameter(message) if message.contains("cyclic")),
            "unexpected error: {error:?}"
        );
        assert_eq!(status_of(&instance), "Failed");
    }

    #[test]
    fn attached_seed_predicate_is_not_implemented() {
        let fixture = build_fixture();
        let mut seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        let predicate = fixture.described("predicate", "QueryPredicate");
        seed.add_related_holons(QueryRelationshipTypeName::SeedPredicate, vec![predicate.into()])
            .unwrap();
        let query = fixture.query_with_root(&seed);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .unwrap();
        let instance = execution.instance().clone();
        let root_execution = execution.root_execution().clone();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail) if detail.contains("SeedPredicate")),
            "an attached predicate is refused, not silently ignored: {error:?}"
        );
        assert_eq!(status_of(&instance), "Failed");
        assert_eq!(status_of(&root_execution), "Failed");
        assert!(
            related_members(
                &HolonReference::from(root_execution),
                QueryRelationshipTypeName::Result
            )
            .unwrap()
            .is_empty(),
            "a refused execution records no Result"
        );
        assert!(related_members(
            &HolonReference::from(instance),
            QueryRelationshipTypeName::ExecutionResult
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn predicate_on_the_wrong_operator_is_not_implemented() {
        let fixture = build_fixture();
        // Ill-formed per schema (`SeedPredicate` belongs to `SeedHolons`), but
        // constructible on a transient definition. A kind-keyed check would read
        // only `ExpansionPredicate` here and return an unfiltered result.
        let mut expand = fixture.expand("expand", "AuthoredBy");
        let predicate = fixture.described("predicate", "QueryPredicate");
        expand
            .add_related_holons(QueryRelationshipTypeName::SeedPredicate, vec![predicate.into()])
            .unwrap();
        let query = fixture.query_with_root(&expand);
        let input = fixture.collection_of("expand-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance = execution.instance().clone();
        let root_execution = execution.root_execution().clone();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail) if detail.contains("SeedPredicate")),
            "a predicate on either attachment point is refused, whatever the kind: {error:?}"
        );
        assert_eq!(status_of(&instance), "Failed");
        assert_eq!(status_of(&root_execution), "Failed");
        assert!(related_members(
            &HolonReference::from(root_execution),
            QueryRelationshipTypeName::Result
        )
        .unwrap()
        .is_empty());
        assert!(related_members(
            &HolonReference::from(instance),
            QueryRelationshipTypeName::ExecutionResult
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn two_attached_predicates_are_not_implemented() {
        let fixture = build_fixture();
        let mut seed = fixture.described("seed", SEED_HOLONS_TYPE_NAME);
        let first = fixture.described("predicate-1", "QueryPredicate");
        let second = fixture.described("predicate-2", "QueryPredicate");
        seed.add_related_holons(
            QueryRelationshipTypeName::SeedPredicate,
            vec![first.into(), second.into()],
        )
        .unwrap();
        let query = fixture.query_with_root(&seed);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .unwrap();
        let instance = execution.instance().clone();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail) if detail.contains("SeedPredicate")),
            "the refusal names the unimplemented feature, not the count: {error:?}"
        );
        assert_eq!(status_of(&instance), "Failed");
    }

    #[test]
    fn repeated_expansion_preserves_order_and_duplicates() {
        let fixture = build_fixture();

        // Inverse name: resolution finds no declared descriptor, so policy falls
        // back to `Fresh` and each read refetches.
        let owns = CoreRelationshipTypeName::Owns.to_relationship_name();
        let first = expand_one(&fixture.space, &owns).unwrap();
        let second = expand_one(&fixture.space, &owns).unwrap();
        assert_eq!(ids_of(&first), vec![id(10), id(11), id(12), id(11)]);
        assert_eq!(ids_of(&second), ids_of(&first), "the refetched read agrees member for member");

        // Declared name marked `IsDefinitional`: policy is `Reuse`, so the entry is
        // retained and the second read is served from the cache. This is the path
        // the bypass used to skip, and `book-d`'s membership repeats `person-1`, so
        // it exercises order *and* an in-entry duplicate surviving the sealed view.
        let authored_by = RelationshipName(MapString("AuthoredBy".to_string()));
        let book_d = fixture.saved(30);
        let first = expand_one(&book_d, &authored_by).unwrap();
        let second = expand_one(&book_d, &authored_by).unwrap();
        assert_eq!(ids_of(&first), vec![id(24), id(29), id(24)]);
        assert_eq!(ids_of(&second), ids_of(&first), "the cached read agrees member for member");
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

    /// Runs an `Expand` root over `sources` and returns the result members.
    fn run_expand(
        fixture: &Fixture,
        relationship_name: &str,
        sources: &[u8],
    ) -> Result<Vec<HolonReference>, HolonError> {
        let expand = fixture.expand("expand", relationship_name);
        let query = fixture.query_with_root(&expand);
        let input = fixture.collection_of("expand-input", sources);
        let execution = query.begin_execution(
            &fixture.context,
            fixture.focal_space(),
            Some(input),
            Vec::new(),
        )?;
        let result = execution.run()?;
        related_members(result.as_holon_reference(), CoreRelationshipTypeName::CollectionMembers)
    }

    #[test]
    fn expand_declared_name_preserves_source_and_target_order() {
        let fixture = build_fixture();
        // book-a -> [person-1, person-2], book-b -> [person-1], book-c -> none.
        let members = run_expand(&fixture, "AuthoredBy", &[20, 27, 28]).unwrap();
        assert_eq!(
            ids_of(&members),
            vec![id(24), id(29), id(24)],
            "source order then storage order, duplicates retained, empty contributes nothing"
        );
    }

    #[test]
    fn expand_inverse_name_resolves_through_descriptor() {
        let fixture = build_fixture();
        let members = run_expand(&fixture, "AuthorOf", &[24]).unwrap();
        assert_eq!(ids_of(&members), vec![id(20), id(27)]);
    }

    #[test]
    fn expand_over_an_empty_input_is_an_empty_result() {
        let fixture = build_fixture();
        assert!(run_expand(&fixture, "AuthoredBy", &[]).unwrap().is_empty());
    }

    #[test]
    fn expand_unknown_name_propagates_descriptor_error() {
        let fixture = build_fixture();
        let expand = fixture.expand("expand", "NoSuchRelationship");
        let query = fixture.query_with_root(&expand);
        let input = fixture.collection_of("expand-input", &[20]);
        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance = execution.instance().clone();
        let root_execution = execution.root_execution().clone();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. }
                if name == "NoSuchRelationship"),
            "unexpected error: {error:?}"
        );
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
    fn expand_unsaved_endpoint_propagates_unsupported_staged_traversal() {
        let fixture = build_fixture();
        // A member described by a transient descriptor has no materialized
        // SourceOf index, so inverse discovery is not answerable yet.
        let member = fixture.described("unsaved-member", "UnsavedType");
        let mut input_holon = fixture.described("expand-input", HOLON_COLLECTION_TYPE_NAME);
        input_holon
            .add_related_holons(CoreRelationshipTypeName::CollectionMembers, vec![member.into()])
            .unwrap();
        let expand = fixture.expand("expand", "AuthorOf");
        let query = fixture.query_with_root(&expand);
        let execution = query
            .begin_execution(
                &fixture.context,
                fixture.focal_space(),
                Some(HolonCollectionReference::new(input_holon.into()).unwrap()),
                Vec::new(),
            )
            .unwrap();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(error, HolonError::UnsupportedStagedTraversal { .. }),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn expand_without_a_relationship_name_is_an_empty_field() {
        let fixture = build_fixture();
        // `ExpansionRelationshipName` is schema-required, but a definition can
        // reach the runtime without it; fail on the definition, not the data.
        let expand = fixture.described("expand", EXPAND_TYPE_NAME);
        let query = fixture.query_with_root(&expand);
        let input = fixture.collection_of("expand-input", &[20]);
        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();

        let error = execution.run().unwrap_err();
        assert!(matches!(error, HolonError::EmptyField(_)), "unexpected error: {error:?}");
    }

    #[test]
    fn single_holon_convenience_wraps_a_transient_singleton_collection() {
        let fixture = build_fixture();
        let expand = fixture.expand("expand", "AuthoredBy");
        let query = fixture.query_with_root(&expand);

        let execution = query
            .begin_execution_for_holon(
                &fixture.context,
                fixture.focal_space(),
                fixture.saved(20),
                Vec::new(),
            )
            .unwrap();
        let root_execution: HolonReference = execution.root_execution().clone().into();

        // Input is a HolonCollection holon holding the source, never the source itself.
        let input = exactly_one(&root_execution, QueryRelationshipTypeName::Input).unwrap();
        require_described_as(&input, HOLON_COLLECTION_TYPE_NAME)
            .expect("the convenience wraps its source in a collection holon");
        let input_members =
            related_members(&input, CoreRelationshipTypeName::CollectionMembers).unwrap();
        assert_eq!(ids_of(&input_members), vec![id(20)]);

        let result = execution.run().unwrap();
        let members = related_members(
            result.as_holon_reference(),
            CoreRelationshipTypeName::CollectionMembers,
        )
        .unwrap();
        assert_eq!(ids_of(&members), vec![id(24), id(29)]);
    }

    #[test]
    fn attached_expansion_predicate_is_not_implemented() {
        let fixture = build_fixture();
        let predicate = fixture.described("predicate", "QueryPredicate");
        let mut expand = fixture.expand("expand", "AuthoredBy");
        expand
            .add_related_holons(
                QueryRelationshipTypeName::ExpansionPredicate,
                vec![predicate.into()],
            )
            .unwrap();
        let query = fixture.query_with_root(&expand);
        let input = fixture.collection_of("expand-input", &[20, 27]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance = execution.instance().clone();
        let root_execution = execution.root_execution().clone();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail)
                if detail.contains("ExpansionPredicate")),
            "an attached predicate is refused, not silently ignored: {error:?}"
        );
        assert_eq!(status_of(&instance), "Failed");
        assert_eq!(status_of(&root_execution), "Failed");
        assert!(
            related_members(
                &HolonReference::from(root_execution),
                QueryRelationshipTypeName::Result
            )
            .unwrap()
            .is_empty(),
            "a refused execution records no Result"
        );
        assert!(related_members(
            &HolonReference::from(instance),
            QueryRelationshipTypeName::ExecutionResult
        )
        .unwrap()
        .is_empty());
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
