//! QueryCore — descriptor-backed Query runtime (QRY1 scaffold, QRY2 navigation,
//! QRY4a ordering and pagination, QRY4b identity-based Distinct).
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
//!   `SeedHolons` (root only), `Expand`, `OrderBy`, `Distinct`, `Skip`, and `Limit` — with
//!   `HolonError::NotImplemented` for every other concrete kind.
//!
//! Focal space is invocation context, not definition state: it is recorded only
//! on the transient `ExecutionInstance`. Nothing is ever written onto the
//! reusable Query or QueryExpression definitions, which may themselves be
//! transient: a caller can author and run a query graph without staging or
//! committing it. Separate invocation parameter bindings are not supported yet
//! (QRY6): a nonempty binding list is refused before any runtime record exists.
//! The legacy `query.rs` compatibility surface is untouched and lives beside
//! this module.
//!
//! Input contract: the schema leaves `QueryExpressionExecution.Input` optional
//! because the abstract `QueryExpression` cannot know whether its root is a
//! source or a transform. The runtime enforces the operator-specific rule in
//! [`QueryReference::begin_execution`]: a root `SeedHolons` accepts no input (a
//! supplied collection is a contract error, not an ignored operand); a root
//! transform (`Expand`, `OrderBy`, `Distinct`, `Skip`, `Limit`) requires exactly one
//! `HolonCollectionReference`. When present, the
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

use super::{distinct, order_by, pagination};
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::resolve_core_descriptor;
use crate::reference_layer::{HolonReference, ReadableHolon, TransientReference, WritableHolon};

/// Descriptor type names and keys the runtime relies on from the loaded schemas.
const QUERY_TYPE_NAME: &str = "Query";
const HOLON_COLLECTION_TYPE_NAME: &str = "HolonCollection";
const HOLON_SPACE_TYPE_NAME: &str = "HolonSpace";
const SEED_HOLONS_TYPE_NAME: &str = "SeedHolons";
const EXPAND_TYPE_NAME: &str = "Expand";
const ORDER_BY_TYPE_NAME: &str = "OrderBy";
const DISTINCT_TYPE_NAME: &str = "Distinct";
const SKIP_TYPE_NAME: &str = "Skip";
const LIMIT_TYPE_NAME: &str = "Limit";
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
    /// references. Their resolution is not implemented (QRY6), so a nonempty list
    /// is refused with `HolonError::NotImplemented` as the first operation —
    /// before root/input validation and before any runtime record is created.
    /// This is the single refusal point: the single-holon helper and the
    /// QueryDance adapter forward their bindings here. An empty list proceeds.
    /// Both records start `Pending`.
    pub fn begin_execution(
        &self,
        context: &Arc<TransactionContext>,
        focal_space: FocalSpaceReference,
        input: Option<HolonCollectionReference>,
        bindings: Vec<HolonReference>,
    ) -> Result<QueryExecution, HolonError> {
        if !bindings.is_empty() {
            return Err(HolonError::NotImplemented(format!(
                "query invocation parameter bindings ({} supplied)",
                bindings.len()
            )));
        }
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
            active_step: None,
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
    /// Index of the step currently executing, cleared once it completes. A
    /// failure is charged to this step only; an error raised between steps
    /// (reading `Next`, the cycle check) fails the instance alone.
    active_step: Option<usize>,
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
    /// returned. On any failure the instance becomes `Failed`, no
    /// `ExecutionResult` is recorded, later steps are never created, and the
    /// error propagates unchanged. A step that was executing when the error
    /// occurred becomes `Failed` with no `Result`; a step that already completed
    /// keeps `Complete` and its `Result`, even when the error comes from
    /// continuing past it (an unreadable `Next`, a cycle).
    pub fn run(mut self) -> Result<HolonCollectionReference, HolonError> {
        set_status(&mut self.instance, ExecutionStatus::Running)?;

        match self.execute_chain() {
            Ok(result) => {
                set_status(&mut self.instance, ExecutionStatus::Complete)?;
                Ok(result)
            }
            Err(error) => {
                // Only the step that was executing fails; completed steps keep
                // the `Complete` they legitimately reached.
                if let Some(step) = self.active_step {
                    set_status(&mut self.executions[step], ExecutionStatus::Failed)?;
                }
                set_status(&mut self.instance, ExecutionStatus::Failed)?;
                Err(error)
            }
        }
    }

    fn execute_chain(&mut self) -> Result<HolonCollectionReference, HolonError> {
        let context = self.instance.bound_context();
        let mut expression = self.root_expression.clone();
        // `Next` cardinality permits a cycle (`A -Next-> B -Next-> A` satisfies
        // both `Next` and `Previous` as ZeroOrOne), which no schema or commit
        // check rejects. Terminate on the repeated expression rather than
        // looping forever; this is not a result-size guard — fan-out across a
        // chain is expected until the predicate/operator track lands.
        let mut visited = HashSet::new();
        visited.insert(expression.reference_id_string());

        loop {
            let step = self.executions.len() - 1;
            self.active_step = Some(step);
            set_status(&mut self.executions[step], ExecutionStatus::Running)?;
            // The root was classified by `begin_execution`. A successor is
            // classified only once its own record exists, so a failure here is
            // charged to it rather than to its completed predecessor.
            let kind = if step == 0 {
                self.root_kind.clone()
            } else {
                ExpressionKind::classify(&expression)?
            };

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
                    let input = self.step_input(step)?;
                    expand(&input.members()?, &expansion_name(&expression)?)?
                }
                // Specs are validated before any member is read, so an invalid
                // spec fails even for an empty collection.
                ExpressionKind::OrderBy => {
                    let members = self.step_input(step)?.members()?;
                    order_by::order_by(&expression, &members)?
                }
                // The count is read before the input, so an invalid count fails
                // even for an empty collection.
                // Parameter-free: no argument to validate before reading input.
                ExpressionKind::Distinct => distinct::distinct(&self.step_input(step)?.members()?),
                ExpressionKind::Skip => {
                    let count =
                        pagination::read_count(&expression, QueryPropertyTypeName::SkipCount)?;
                    pagination::skip(&self.step_input(step)?.members()?, count)
                }
                ExpressionKind::Limit => {
                    let count =
                        pagination::read_count(&expression, QueryPropertyTypeName::LimitCount)?;
                    pagination::limit(&self.step_input(step)?.members()?, count)
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
            self.active_step = None;

            // Continuation errors from here to the next iteration concern the
            // expression graph, not an operator: they fail the instance only.
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

    /// The `Input` collection of a transform step. Root: the caller's
    /// collection. Non-root: the predecessor's result, linked when this record
    /// was created.
    fn step_input(&self, step: usize) -> Result<HolonCollectionReference, HolonError> {
        Ok(HolonCollectionReference(exactly_one(
            &self.executions[step].clone().into(),
            QueryRelationshipTypeName::Input,
        )?))
    }
}

/// Concrete expression kinds the runtime recognizes, read from the root
/// expression's descriptor type name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExpressionKind {
    SeedHolons,
    Expand,
    OrderBy,
    Distinct,
    Skip,
    Limit,
    Unsupported(String),
}

impl ExpressionKind {
    fn classify(expression: &HolonReference) -> Result<Self, HolonError> {
        let type_name = expression.holon_descriptor()?.header().type_name()?.0;
        Ok(match type_name.as_str() {
            SEED_HOLONS_TYPE_NAME => Self::SeedHolons,
            EXPAND_TYPE_NAME => Self::Expand,
            ORDER_BY_TYPE_NAME => Self::OrderBy,
            DISTINCT_TYPE_NAME => Self::Distinct,
            SKIP_TYPE_NAME => Self::Skip,
            LIMIT_TYPE_NAME => Self::Limit,
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
            (Self::Expand | Self::OrderBy | Self::Distinct | Self::Skip | Self::Limit, None) => {
                Err(HolonError::MissingRequiredRelationship {
                    relationship: QueryRelationshipTypeName::Input
                        .to_relationship_name()
                        .to_string(),
                    descriptor: expression.summarize()?,
                })
            }
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

pub(crate) fn require_described_as(
    holon: &HolonReference,
    expected: &str,
) -> Result<(), HolonError> {
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
    use type_names::{CorePropertyTypeName, ToPropertyName};

    use super::*;
    use crate::core_shared_objects::holon::SavedHolon;
    use crate::descriptors::test_support::{
        build_context, build_context_with_saved_holons, new_descriptor_holon,
        new_holon_type_descriptor, new_property_descriptor_holon, new_test_holon,
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
        let instance: HolonReference = execution.instance().clone().into();

        let error = execution.run().unwrap_err();
        assert!(
            matches!(&error, HolonError::InvalidParameter(message) if message.contains("cyclic")),
            "unexpected error: {error:?}"
        );
        assert_eq!(
            instance.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Failed.as_enum_value()))
        );
        assert!(related_members(&instance, QueryRelationshipTypeName::ExecutionResult)
            .unwrap()
            .is_empty());

        // Both steps ran to completion before the walk found B -> A. The cycle is
        // a fault of the expression graph, not of either step, so neither is
        // retroactively failed and each keeps the Result it produced.
        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 2, "no record is created for the revisited expression");
        for record in &records {
            assert_eq!(
                record.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
                Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value()))
            );
            assert_eq!(
                related_members(record, QueryRelationshipTypeName::Result).unwrap().len(),
                1
            );
        }
    }

    #[test]
    fn successor_classification_failure_fails_the_successor_not_its_predecessor() {
        let fixture = build_fixture();
        // The successor has no descriptor, so it cannot be classified. That is a
        // fault of the successor: it gets a record, and that record fails.
        let mut first = fixture.expand("first", "AuthoredBy");
        let undescribed = new_test_holon(&fixture.context, "undescribed").unwrap();
        chain(&mut first, &undescribed);
        let query = fixture.query_with_root(&first);
        let input = fixture.collection_of("chain-input", &[20]);

        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance: HolonReference = execution.instance().clone().into();

        execution.run().unwrap_err();

        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 2, "the successor's record exists before it is classified");
        assert_eq!(
            records[0].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value())),
            "the predecessor completed and is not blamed"
        );
        assert_eq!(
            related_members(&records[0], QueryRelationshipTypeName::Result).unwrap().len(),
            1
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

    // ---- QRY4a pagination and invocation bindings --------------------------

    impl Fixture {
        /// A `Skip` or `Limit` definition; `count` of `None` leaves it unset.
        fn paginate(&self, key: &str, type_name: &str, count: Option<i64>) -> TransientReference {
            let mut expression = self.described(key, type_name);
            let property = match type_name {
                SKIP_TYPE_NAME => QueryPropertyTypeName::SkipCount,
                LIMIT_TYPE_NAME => QueryPropertyTypeName::LimitCount,
                other => panic!("not a pagination kind: {other}"),
            };
            if let Some(count) = count {
                expression.with_property_value(property, MapInteger(count)).unwrap();
            }
            expression
        }
    }

    /// Runs `root` over a collection of the given saved holons and returns the
    /// execution instance plus the result members.
    fn run_over(
        fixture: &Fixture,
        root: &TransientReference,
        sources: &[u8],
    ) -> (HolonReference, Result<Vec<HolonReference>, HolonError>) {
        let query = fixture.query_with_root(root);
        let input = fixture.collection_of("paging-input", sources);
        let execution = query
            .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
            .unwrap();
        let instance: HolonReference = execution.instance().clone().into();
        let members = execution.run().and_then(|result| {
            related_members(
                result.as_holon_reference(),
                CoreRelationshipTypeName::CollectionMembers,
            )
        });
        (instance, members)
    }

    #[test]
    fn expand_skip_limit_chain_pages_the_expanded_sequence() {
        let fixture = build_fixture();
        // Expand(AuthoredBy) over [20, 30] is [24, 29, 24, 29, 24]; Skip 1 then
        // Limit 3 keeps [29, 24, 29] — order and duplicates preserved.
        let mut expand = fixture.expand("expand", "AuthoredBy");
        let mut skip = fixture.paginate("skip", SKIP_TYPE_NAME, Some(1));
        let limit = fixture.paginate("limit", LIMIT_TYPE_NAME, Some(3));
        chain(&mut expand, &skip);
        chain(&mut skip, &limit);

        let (instance, members) = run_over(&fixture, &expand, &[20, 30]);
        assert_eq!(ids_of(&members.unwrap()), vec![id(29), id(24), id(29)]);

        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 3, "one record per step");
        for pair in records.windows(2) {
            let result = exactly_one(&pair[0], QueryRelationshipTypeName::Result).unwrap();
            let input = exactly_one(&pair[1], QueryRelationshipTypeName::Input).unwrap();
            assert_eq!(input.reference_id_string(), result.reference_id_string());
        }
        for record in &records {
            assert_eq!(
                record.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
                Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value()))
            );
        }
    }

    #[test]
    fn authored_order_decides_the_page() {
        let fixture = build_fixture();
        // Limit 2 then Skip 1 over [10, 11, 12, 11] keeps [11]; the reverse
        // order (Skip 1 then Limit 2) keeps [11, 12].
        let mut limit = fixture.paginate("limit", LIMIT_TYPE_NAME, Some(2));
        let skip = fixture.paginate("skip", SKIP_TYPE_NAME, Some(1));
        chain(&mut limit, &skip);
        let (_, members) = run_over(&fixture, &limit, &[10, 11, 12, 11]);
        assert_eq!(ids_of(&members.unwrap()), vec![id(11)]);

        let fixture = build_fixture();
        let mut skip = fixture.paginate("skip", SKIP_TYPE_NAME, Some(1));
        let limit = fixture.paginate("limit", LIMIT_TYPE_NAME, Some(2));
        chain(&mut skip, &limit);
        let (_, members) = run_over(&fixture, &skip, &[10, 11, 12, 11]);
        assert_eq!(ids_of(&members.unwrap()), vec![id(11), id(12)]);
    }

    #[test]
    fn pagination_count_boundaries() {
        let sources = [10, 11, 12, 11];
        let cases: [(&str, i64, Vec<u8>); 6] = [
            (SKIP_TYPE_NAME, 0, vec![10, 11, 12, 11]),
            (SKIP_TYPE_NAME, 4, vec![]),
            (SKIP_TYPE_NAME, i64::MAX, vec![]),
            (LIMIT_TYPE_NAME, 0, vec![]),
            (LIMIT_TYPE_NAME, 4, vec![10, 11, 12, 11]),
            (LIMIT_TYPE_NAME, i64::MAX, vec![10, 11, 12, 11]),
        ];
        for (type_name, count, expected) in cases {
            let fixture = build_fixture();
            let root = fixture.paginate("page", type_name, Some(count));
            let (_, members) = run_over(&fixture, &root, &sources);
            let expected: Vec<HolonId> = expected.into_iter().map(id).collect();
            assert_eq!(ids_of(&members.unwrap()), expected, "{type_name} {count}");
        }
    }

    #[test]
    fn pagination_roots_require_input() {
        for type_name in [SKIP_TYPE_NAME, LIMIT_TYPE_NAME] {
            let fixture = build_fixture();
            let root = fixture.paginate("page", type_name, Some(1));
            let query = fixture.query_with_root(&root);
            let error = query
                .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
                .unwrap_err();
            assert!(
                matches!(&error, HolonError::MissingRequiredRelationship { relationship, .. }
                    if relationship == "Input"),
                "{type_name}: unexpected error: {error:?}"
            );
        }
    }

    #[test]
    fn invalid_counts_fail_even_on_empty_input() {
        let cases = [
            (SKIP_TYPE_NAME, None, "missing"),
            (LIMIT_TYPE_NAME, None, "missing"),
            (SKIP_TYPE_NAME, Some(-1), "negative"),
            (LIMIT_TYPE_NAME, Some(-1), "negative"),
        ];
        for (type_name, count, label) in cases {
            let fixture = build_fixture();
            let root = fixture.paginate("page", type_name, count);
            let (instance, members) = run_over(&fixture, &root, &[]);
            let error = members.unwrap_err();
            match label {
                "missing" => assert!(
                    matches!(&error, HolonError::EmptyField(name)
                        if name == &format!("{type_name}Count")),
                    "{type_name} {label}: unexpected error: {error:?}"
                ),
                _ => assert!(
                    matches!(error, HolonError::InvalidParameter(_)),
                    "{type_name} {label}: unexpected error: {error:?}"
                ),
            }
            let records =
                related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions)
                    .unwrap();
            assert_eq!(
                records[0].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
                Some(BaseValue::EnumValue(ExecutionStatus::Failed.as_enum_value()))
            );
            assert!(related_members(&records[0], QueryRelationshipTypeName::Result)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn a_count_is_required_only_by_its_own_kind() {
        let fixture = build_fixture();
        // A Skip carrying LimitCount (and no SkipCount) still lacks SkipCount;
        // an Expand needs neither count.
        let mut expand = fixture.expand("expand", "AuthoredBy");
        let mut skip = fixture.described("skip", SKIP_TYPE_NAME);
        skip.with_property_value(QueryPropertyTypeName::LimitCount, MapInteger(1)).unwrap();
        chain(&mut expand, &skip);

        let (instance, members) = run_over(&fixture, &expand, &[20]);
        let error = members.unwrap_err();
        assert!(
            matches!(&error, HolonError::EmptyField(name) if name == "SkipCount"),
            "unexpected error: {error:?}"
        );
        let records =
            related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Complete.as_enum_value())),
            "the Expand that needs no count completes"
        );
        assert!(related_members(&instance, QueryRelationshipTypeName::ExecutionResult)
            .unwrap()
            .is_empty());
    }

    /// No runtime record exists under the fixed record keys.
    fn assert_no_runtime_records(fixture: &Fixture) {
        for key in
            [EXECUTION_INSTANCE_KEY.to_string(), format!("{QUERY_EXPRESSION_EXECUTION_KEY}-0")]
        {
            assert!(
                fixture
                    .context
                    .lookup()
                    .get_transient_holon_by_base_key(&MapString(key.clone()))
                    .is_err(),
                "{key} must not be created for a refused invocation"
            );
        }
    }

    #[test]
    fn nonempty_bindings_are_refused_before_any_validation_or_record() {
        let fixture = build_fixture();
        // An Expand root with no input would otherwise fail root-input
        // validation; the binding refusal must come first.
        let expand = fixture.expand("expand", "AuthoredBy");
        let query = fixture.query_with_root(&expand);
        let before = fixture.context.lookup().transient_count().unwrap();

        let error = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, vec![fixture.saved(10)])
            .unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
        assert_eq!(
            fixture.context.lookup().transient_count().unwrap(),
            before,
            "a refused invocation creates no transient holon at all"
        );
        assert_no_runtime_records(&fixture);
    }

    /// An `OrderBy` definition relating `spec_count` spec holons.
    fn order_by_with_specs(fixture: &Fixture, spec_count: usize) -> TransientReference {
        let mut expression = fixture.described("order-by", ORDER_BY_TYPE_NAME);
        let specs: Vec<HolonReference> = (0..spec_count)
            .map(|index| fixture.described(&format!("spec-{index}"), "OrderBySpec").into())
            .collect();
        if !specs.is_empty() {
            expression.add_related_holons(QueryRelationshipTypeName::OrderBySpecs, specs).unwrap();
        }
        expression
    }

    #[test]
    fn order_by_spec_count_is_validated_even_for_empty_input() {
        for spec_count in [0, 6] {
            let fixture = build_fixture();
            let root = order_by_with_specs(&fixture, spec_count);
            let (instance, members) = run_over(&fixture, &root, &[]);
            let error = members.unwrap_err();
            assert!(
                matches!(error, HolonError::InvalidParameter(_)),
                "{spec_count} specs: unexpected error: {error:?}"
            );
            let records =
                related_members(&instance, QueryRelationshipTypeName::ExpressionExecutions)
                    .unwrap();
            assert_eq!(
                records[0].property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
                Some(BaseValue::EnumValue(ExecutionStatus::Failed.as_enum_value()))
            );
            assert!(related_members(&records[0], QueryRelationshipTypeName::Result)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn order_by_root_requires_input() {
        let fixture = build_fixture();
        let root = order_by_with_specs(&fixture, 1);
        let query = fixture.query_with_root(&root);
        let error = query
            .begin_execution(&fixture.context, fixture.focal_space(), None, Vec::new())
            .unwrap_err();
        assert!(
            matches!(&error, HolonError::MissingRequiredRelationship { relationship, .. }
                if relationship == "Input"),
            "unexpected error: {error:?}"
        );
    }

    // ---- OrderBy name selection over transient descriptors -----------------

    /// Transient descriptors for OrderBy: value types with afforded operators,
    /// the `OrderBySpec` type, and member types that declare same-named
    /// properties through distinct `PropertyType` descriptors.
    struct SortWorld {
        fixture: Fixture,
        spec_type: TransientReference,
        /// The spec's `PropertyName` and `NullPlacement` declarations, kept so a
        /// test can declare a spec type that omits `SortDirection`.
        property_name: TransientReference,
        null_placement: TransientReference,
        string_type: TransientReference,
        other_string_type: TransientReference,
        integer_type: TransientReference,
        boolean_type: TransientReference,
    }

    impl SortWorld {
        fn new() -> Self {
            Self::with_sort_direction(|_| {})
        }

        /// Builds the world, letting a test adjust the `SortDirection` property
        /// descriptor (its default) before the spec type declares it.
        fn with_sort_direction(adjust: impl FnOnce(&mut TransientReference)) -> Self {
            let fixture = build_fixture();
            let context = fixture.context.clone();
            let operator = |key: &str, name: &str| -> HolonReference {
                new_descriptor_holon(&context, key, name, "Operator").unwrap().into()
            };
            let equals = operator("op-equals", "EqualsOperator");
            let less_than = operator("op-less-than", "LessThanOperator");
            let value_type = |key: &str, kind: &str, operators: &[&HolonReference]| {
                let mut holon = new_descriptor_holon(&context, key, kind, "Value").unwrap();
                holon
                    .with_property_value(CorePropertyTypeName::DefinesInstanceTypeKind, true)
                    .unwrap();
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
            let string_type = value_type("string-type", "StringValueType", &[&equals, &less_than]);
            let other_string_type =
                value_type("other-string-type", "StringValueType", &[&equals, &less_than]);
            let integer_type =
                value_type("integer-type", "IntegerValueType", &[&equals, &less_than]);
            let boolean_type = value_type("boolean-type", "BooleanValueType", &[&equals]);

            let enum_type = |key: &str, variants: &[&str]| -> HolonReference {
                let mut holon = value_type(key, "MapEnumValueType", &[&equals]);
                let variants: Vec<HolonReference> = variants
                    .iter()
                    .map(|variant| {
                        new_descriptor_holon(
                            &context,
                            &format!("{key}.{variant}"),
                            variant,
                            "Value",
                        )
                        .unwrap()
                        .into()
                    })
                    .collect();
                holon.add_related_holons(CoreRelationshipTypeName::Variants, variants).unwrap();
                holon.into()
            };
            let mut sort_direction = new_property_descriptor_holon(
                &context,
                "SortDirection.PropertyType",
                "SortDirection",
                true,
                enum_type("sort-direction-type", &["Ascending", "Descending"]),
            )
            .unwrap();
            sort_direction
                .with_property_value(CorePropertyTypeName::DefaultValue, "Ascending")
                .unwrap();
            adjust(&mut sort_direction);
            let mut null_placement = new_property_descriptor_holon(
                &context,
                "NullPlacement.PropertyType",
                "NullPlacement",
                true,
                enum_type("null-placement-type", &["Missing-First", "Missing-Last"]),
            )
            .unwrap();
            null_placement
                .with_property_value(CorePropertyTypeName::DefaultValue, "Missing-Last")
                .unwrap();
            let property_name = new_property_descriptor_holon(
                &context,
                "PropertyName.PropertyType",
                "PropertyName",
                true,
                string_type.clone().into(),
            )
            .unwrap();
            let mut spec_type =
                new_holon_type_descriptor(&context, "OrderBySpec.HolonType", "OrderBySpec")
                    .unwrap();
            spec_type
                .add_related_holons(
                    CoreRelationshipTypeName::InstanceProperties,
                    vec![
                        property_name.clone().into(),
                        sort_direction.into(),
                        null_placement.clone().into(),
                    ],
                )
                .unwrap();

            Self {
                fixture,
                spec_type,
                property_name,
                null_placement,
                string_type,
                other_string_type,
                integer_type,
                boolean_type,
            }
        }

        /// A member holon type declaring `(property name, required, value type)`
        /// properties, each through its own `PropertyType` descriptor.
        fn member_type(
            &self,
            type_name: &str,
            properties: &[(&str, bool, &TransientReference)],
        ) -> HolonReference {
            let context = &self.fixture.context;
            let declarations: Vec<HolonReference> = properties
                .iter()
                .map(|(name, required, value_type)| {
                    new_property_descriptor_holon(
                        context,
                        &format!("{type_name}.{name}.PropertyType"),
                        name,
                        *required,
                        (*value_type).clone().into(),
                    )
                    .unwrap()
                    .into()
                })
                .collect();
            let mut holon_type =
                new_holon_type_descriptor(context, &format!("{type_name}.HolonType"), type_name)
                    .unwrap();
            holon_type
                .add_related_holons(CoreRelationshipTypeName::InstanceProperties, declarations)
                .unwrap();
            holon_type.into()
        }

        /// A member of `holon_type` with the given stored property values.
        fn member(
            &self,
            key: &str,
            holon_type: &HolonReference,
            values: &[(&str, BaseValue)],
        ) -> HolonReference {
            let mut holon = new_test_holon(&self.fixture.context, key).unwrap();
            holon.with_descriptor(holon_type.clone()).unwrap();
            for (name, value) in values {
                holon.with_property_value(*name, value.clone()).unwrap();
            }
            holon.into()
        }

        /// An `OrderBySpec`; each argument is stored only when given.
        fn spec(
            &self,
            key: &str,
            property_name: Option<BaseValue>,
            direction: Option<&str>,
            placement: Option<&str>,
        ) -> HolonReference {
            let mut spec = new_test_holon(&self.fixture.context, key).unwrap();
            // These query fixtures exercise read-only effective defaults, including
            // malformed defaults; descriptor authoring must leave omissions explicit.
            spec.add_related_holons(
                CoreRelationshipTypeName::DescribedBy,
                vec![self.spec_type.clone().into()],
            )
            .unwrap();
            if let Some(name) = property_name {
                spec.with_property_value(QueryPropertyTypeName::PropertyName, name).unwrap();
            }
            for (property, variant) in [
                (QueryPropertyTypeName::SortDirection, direction),
                (QueryPropertyTypeName::NullPlacement, placement),
            ] {
                if let Some(variant) = variant {
                    spec.with_property_value(
                        property,
                        MapEnumValue(MapString(variant.to_string())),
                    )
                    .unwrap();
                }
            }
            spec.into()
        }

        fn order_by(&self, specs: Vec<HolonReference>) -> TransientReference {
            let mut expression = self.fixture.described("order-by", ORDER_BY_TYPE_NAME);
            expression.add_related_holons(QueryRelationshipTypeName::OrderBySpecs, specs).unwrap();
            expression
        }

        /// Runs `root` over `members` (any phase) and returns the instance and
        /// the result members.
        fn run(
            &self,
            root: &TransientReference,
            members: &[HolonReference],
        ) -> (HolonReference, Result<Vec<HolonReference>, HolonError>) {
            let query = self.fixture.query_with_root(root);
            let mut input = self.fixture.described("sort-input", HOLON_COLLECTION_TYPE_NAME);
            if !members.is_empty() {
                input
                    .add_related_holons(
                        CoreRelationshipTypeName::CollectionMembers,
                        members.to_vec(),
                    )
                    .unwrap();
            }
            let input = HolonCollectionReference::new(input.into()).unwrap();
            let execution = query
                .begin_execution(
                    &self.fixture.context,
                    self.fixture.focal_space(),
                    Some(input),
                    Vec::new(),
                )
                .unwrap();
            let instance: HolonReference = execution.instance().clone().into();
            let members = execution.run().and_then(|result| {
                related_members(
                    result.as_holon_reference(),
                    CoreRelationshipTypeName::CollectionMembers,
                )
            });
            (instance, members)
        }
    }

    fn text(value: &str) -> BaseValue {
        BaseValue::StringValue(MapString(value.to_string()))
    }

    fn number(value: i64) -> BaseValue {
        BaseValue::IntegerValue(MapInteger(value))
    }

    fn refs_of(members: &[HolonReference]) -> Vec<String> {
        members.iter().map(HolonReference::reference_id_string).collect()
    }

    /// The step failed with no Result, the instance has no ExecutionResult.
    fn assert_failed_without_result(instance: &HolonReference) {
        let records =
            related_members(instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap();
        let step = records.last().unwrap();
        assert_eq!(
            step.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            Some(BaseValue::EnumValue(ExecutionStatus::Failed.as_enum_value()))
        );
        assert!(related_members(step, QueryRelationshipTypeName::Result).unwrap().is_empty());
        assert!(related_members(instance, QueryRelationshipTypeName::ExecutionResult)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn distinct_same_named_properties_with_one_value_type_sort_together() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let film = world.member_type("Film", &[("Title", true, &world.string_type)]);
        let members = [
            world.member("book-c", &book, &[("Title", text("Charlie"))]),
            world.member("film-a", &film, &[("Title", text("Alpha"))]),
            world.member("book-b", &book, &[("Title", text("Bravo"))]),
            world.member("film-b", &film, &[("Title", text("Bravo"))]),
        ];
        let root = world.order_by(vec![world.spec("spec", Some(text("Title")), None, None)]);

        let (_, sorted) = world.run(&root, &members);
        let expected = [&members[1], &members[2], &members[3], &members[0]];
        assert_eq!(
            refs_of(&sorted.unwrap()),
            expected.iter().map(|member| member.reference_id_string()).collect::<Vec<_>>(),
            "Book and Film Titles are distinct PropertyTypes over one value type; ties keep input order"
        );
    }

    #[test]
    fn a_second_value_type_identity_is_rejected_even_with_the_same_primitive_kind() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let gadget = world.member_type("Gadget", &[("Title", true, &world.other_string_type)]);
        let members = [
            world.member("book", &book, &[("Title", text("Bravo"))]),
            world.member("gadget", &gadget, &[("Title", text("Alpha"))]),
        ];
        let root = world.order_by(vec![world.spec("spec", Some(text("Title")), None, None)]);

        let (instance, sorted) = world.run(&root, &members);
        let error = sorted.unwrap_err();
        assert!(matches!(error, HolonError::InvalidParameter(_)), "unexpected error: {error:?}");
        assert_failed_without_result(&instance);
    }

    #[test]
    fn value_type_identity_is_enforced_when_the_value_is_absent() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let toy = world.member_type("Toy", &[("Title", false, &world.integer_type)]);
        let members = [
            world.member("book", &book, &[("Title", text("Bravo"))]),
            world.member("toy-without-title", &toy, &[]),
        ];
        let root = world.order_by(vec![world.spec("spec", Some(text("Title")), None, None)]);

        let (instance, sorted) = world.run(&root, &members);
        let error = sorted.unwrap_err();
        assert!(matches!(error, HolonError::InvalidParameter(_)), "unexpected error: {error:?}");
        assert_failed_without_result(&instance);
    }

    #[test]
    fn requiredness_comes_from_each_members_own_declaration() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let film = world.member_type("Film", &[("Title", false, &world.string_type)]);
        let root = world.order_by(vec![world.spec("spec", Some(text("Title")), None, None)]);

        // Film's Title is optional: absent is a missing value, placed last.
        let titled_book = world.member("book", &book, &[("Title", text("Bravo"))]);
        let untitled_film = world.member("film", &film, &[]);
        let (_, sorted) = world.run(&root, &[untitled_film.clone(), titled_book.clone()]);
        assert_eq!(refs_of(&sorted.unwrap()), refs_of(&[titled_book, untitled_film.clone()]));

        // Book's Title is required, even when a Film came first.
        let untitled_book = world.member("untitled-book", &book, &[]);
        let (instance, sorted) = world.run(&root, &[untitled_film, untitled_book]);
        let error = sorted.unwrap_err();
        assert!(
            matches!(&error, HolonError::EmptyField(name) if name == "Title"),
            "unexpected error: {error:?}"
        );
        assert_failed_without_result(&instance);
    }

    #[test]
    fn member_failures_apply_to_every_occurrence_and_every_key() {
        let world = SortWorld::new();
        let book = world.member_type(
            "Book",
            &[("Title", true, &world.string_type), ("Pages", false, &world.integer_type)],
        );
        let pages_then_title = || {
            world.order_by(vec![
                world.spec("pages", Some(text("Pages")), None, None),
                world.spec("title", Some(text("Title")), None, None),
            ])
        };
        let cases: [(&str, Vec<HolonReference>, HolonErrorKindProbe); 4] = [
            (
                "malformed value on a key that does not decide the order",
                vec![
                    world.member("one", &book, &[("Title", text("A")), ("Pages", number(1))]),
                    world.member("two", &book, &[("Title", number(7)), ("Pages", number(2))]),
                ],
                HolonErrorKindProbe::ValueKindMismatch,
            ),
            (
                "malformed singleton",
                vec![world.member("only", &book, &[("Title", number(7))])],
                HolonErrorKindProbe::ValueKindMismatch,
            ),
            (
                "absent required value on a non-deciding key",
                vec![
                    world.member("one", &book, &[("Title", text("A")), ("Pages", number(1))]),
                    world.member("two", &book, &[("Pages", number(2))]),
                ],
                HolonErrorKindProbe::EmptyField,
            ),
            (
                "absent required singleton",
                vec![world.member("only", &book, &[])],
                HolonErrorKindProbe::EmptyField,
            ),
        ];
        for (label, members, expected) in cases {
            let (instance, sorted) = world.run(&pages_then_title(), &members);
            let error = sorted.unwrap_err();
            assert!(expected.matches(&error), "{label}: unexpected error: {error:?}");
            assert_failed_without_result(&instance);
        }

        let undeclared = world.order_by(vec![world.spec("spec", Some(text("Nope")), None, None)]);
        let (instance, sorted) =
            world.run(&undeclared, &[world.member("only", &book, &[("Title", text("A"))])]);
        let error = sorted.unwrap_err();
        assert!(
            matches!(&error, HolonError::DescriptorDeclarationNotFound { name, .. } if name == "Nope"),
            "unexpected error: {error:?}"
        );
        assert_failed_without_result(&instance);
    }

    /// Error kinds the member-failure cases expect.
    enum HolonErrorKindProbe {
        ValueKindMismatch,
        EmptyField,
    }

    impl HolonErrorKindProbe {
        fn matches(&self, error: &HolonError) -> bool {
            match self {
                Self::ValueKindMismatch => matches!(error, HolonError::ValueKindMismatch { .. }),
                Self::EmptyField => matches!(error, HolonError::EmptyField(_)),
            }
        }
    }

    #[test]
    fn unsupported_domains_fail_once_a_member_binds_the_key() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Flag", false, &world.boolean_type)]);
        let root = world.order_by(vec![world.spec("spec", Some(text("Flag")), None, None)]);

        let (_, empty) = world.run(&root, &[]);
        assert!(empty.unwrap().is_empty(), "empty input has no members to bind the domain");

        let (instance, sorted) = world.run(&root, &[world.member("book", &book, &[])]);
        let error = sorted.unwrap_err();
        assert!(
            matches!(error, HolonError::UnsupportedOperator { .. }),
            "unexpected error: {error:?}"
        );
        assert_failed_without_result(&instance);
    }

    #[test]
    fn property_name_must_be_a_present_string_even_for_empty_input() {
        let world = SortWorld::new();
        for (label, property_name) in [("missing", None), ("integer", Some(number(3)))] {
            let root = world.order_by(vec![world.spec("spec", property_name, None, None)]);
            let (instance, sorted) = world.run(&root, &[]);
            let error = sorted.unwrap_err();
            match label {
                "missing" => assert!(
                    matches!(&error, HolonError::EmptyField(name) if name == "PropertyName"),
                    "{label}: unexpected error: {error:?}"
                ),
                _ => assert!(
                    matches!(error, HolonError::UnexpectedValueType(..)),
                    "{label}: unexpected error: {error:?}"
                ),
            }
            assert_failed_without_result(&instance);
        }
    }

    #[test]
    fn five_specs_are_accepted() {
        let world = SortWorld::new();
        let book = world.member_type(
            "Book",
            &[("Title", true, &world.string_type), ("Pages", false, &world.integer_type)],
        );
        let specs = ["Pages", "Title", "Pages", "Title", "Pages"]
            .iter()
            .enumerate()
            .map(|(index, name)| world.spec(&format!("spec-{index}"), Some(text(name)), None, None))
            .collect();
        let root = world.order_by(specs);
        let members = [
            world.member("two", &book, &[("Title", text("B")), ("Pages", number(2))]),
            world.member("one", &book, &[("Title", text("A")), ("Pages", number(1))]),
        ];
        let (_, sorted) = world.run(&root, &members);
        assert_eq!(refs_of(&sorted.unwrap()), refs_of(&[members[1].clone(), members[0].clone()]));
    }

    #[test]
    fn enum_default_failures_are_explicit_and_leave_the_spec_unchanged() {
        // An invalid descriptor default, and a descriptor with no default.
        let invalid_default = SortWorld::with_sort_direction(|property| {
            property.with_property_value(CorePropertyTypeName::DefaultValue, "Sideways").unwrap();
        });
        let no_default = SortWorld::with_sort_direction(|property| {
            property.remove_property_value(CorePropertyTypeName::DefaultValue).unwrap();
        });
        for (label, world) in [("invalid default", invalid_default), ("no default", no_default)] {
            let spec = world.spec("spec", Some(text("Title")), None, None);
            let before = spec.into_model().unwrap();
            let root = world.order_by(vec![spec.clone()]);
            let (instance, sorted) = world.run(&root, &[]);
            let error = sorted.unwrap_err();
            match label {
                "invalid default" => assert!(
                    matches!(error, HolonError::EnumVariantNotInSchema { .. }),
                    "{label}: unexpected error: {error:?}"
                ),
                _ => assert!(
                    matches!(&error, HolonError::EmptyField(name) if name == "SortDirection"),
                    "{label}: unexpected error: {error:?}"
                ),
            }
            assert_failed_without_result(&instance);
            assert_eq!(spec.into_model().unwrap(), before, "{label}: the spec is unchanged");
        }
    }

    #[test]
    fn explicit_enum_values_bypass_a_broken_default_lookup() {
        // Two Extends parents and no local DefaultValue: any default lookup fails.
        let world = SortWorld::with_sort_direction(|property| {
            property.remove_property_value(CorePropertyTypeName::DefaultValue).unwrap();
            let context = property.bound_context();
            let parents: Vec<HolonReference> = ["parent-a", "parent-b"]
                .iter()
                .map(|key| new_descriptor_holon(&context, key, key, "Property").unwrap().into())
                .collect();
            property.add_related_holons(CoreRelationshipTypeName::Extends, parents).unwrap();
        });
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let members = [
            world.member("a", &book, &[("Title", text("A"))]),
            world.member("b", &book, &[("Title", text("B"))]),
        ];

        let explicit = world.order_by(vec![world.spec(
            "valid",
            Some(text("Title")),
            Some("Descending"),
            None,
        )]);
        let (_, sorted) = world.run(&explicit, &members);
        assert_eq!(refs_of(&sorted.unwrap()), refs_of(&[members[1].clone(), members[0].clone()]));

        let invalid = world.order_by(vec![world.spec(
            "invalid",
            Some(text("Title")),
            Some("Sideways"),
            None,
        )]);
        let (_, sorted) = world.run(&invalid, &members);
        let error = sorted.unwrap_err();
        assert!(
            matches!(error, HolonError::EnumVariantNotInSchema { .. }),
            "an invalid explicit value fails on its own, not through the default lookup: {error:?}"
        );

        let omitted = world.order_by(vec![world.spec("omitted", Some(text("Title")), None, None)]);
        let (_, sorted) = world.run(&omitted, &members);
        assert!(sorted.is_err(), "only an absent value reaches the broken lookup");
    }

    #[test]
    fn a_missing_spec_declaration_propagates_the_descriptor_error() {
        let world = SortWorld::new();
        // A spec type declaring a valid PropertyName and NullPlacement but no
        // SortDirection, so resolution passes PropertyName and fails on the
        // undeclared SortDirection rather than defaulting to Ascending.
        let mut spec_type = new_holon_type_descriptor(
            &world.fixture.context,
            "UndirectedSpec.HolonType",
            "OrderBySpec",
        )
        .unwrap();
        spec_type
            .add_related_holons(
                CoreRelationshipTypeName::InstanceProperties,
                vec![world.property_name.clone().into(), world.null_placement.clone().into()],
            )
            .unwrap();
        let mut spec = new_test_holon(&world.fixture.context, "undirected-spec").unwrap();
        spec.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![spec_type.into()])
            .unwrap();
        spec.with_property_value(QueryPropertyTypeName::PropertyName, MapString("Title".into()))
            .unwrap();
        let root = world.order_by(vec![spec.into()]);

        // Sortable members, so only the missing declaration can fail the run.
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let members = [
            world.member("b", &book, &[("Title", text("B"))]),
            world.member("a", &book, &[("Title", text("A"))]),
        ];
        let (instance, sorted) = world.run(&root, &members);
        let error = sorted.unwrap_err();
        assert!(
            matches!(
                &error,
                HolonError::DescriptorDeclarationNotFound { kind, name, .. }
                    if kind == "property" && name == "SortDirection"
            ),
            "expected the undeclared SortDirection, got {error:?}"
        );
        assert_failed_without_result(&instance);
    }

    #[test]
    fn single_holon_convenience_delegates_binding_refusal() {
        let fixture = build_fixture();
        let expand = fixture.expand("expand", "AuthoredBy");
        let query = fixture.query_with_root(&expand);

        let error = query
            .begin_execution_for_holon(
                &fixture.context,
                fixture.focal_space(),
                fixture.saved(20),
                vec![fixture.saved(10)],
            )
            .unwrap_err();
        assert!(matches!(error, HolonError::NotImplemented(_)), "unexpected error: {error:?}");
        assert_no_runtime_records(&fixture);
    }

    // ---- QRY4b identity-based Distinct ---------------------------------------

    fn step_records(instance: &HolonReference) -> Vec<HolonReference> {
        related_members(instance, QueryRelationshipTypeName::ExpressionExecutions).unwrap()
    }

    fn status_value(status: ExecutionStatus) -> Option<BaseValue> {
        Some(BaseValue::EnumValue(status.as_enum_value()))
    }

    fn assert_step(record: &HolonReference, status: ExecutionStatus, has_result: bool) {
        assert_eq!(
            record.property_value(QueryPropertyTypeName::ExecutionStatus).unwrap(),
            status_value(status)
        );
        let results = related_members(record, QueryRelationshipTypeName::Result).unwrap();
        assert_eq!(results.len(), usize::from(has_result));
    }

    #[test]
    fn distinct_root_requires_input() {
        let fixture = build_fixture();
        let root = fixture.described("distinct", DISTINCT_TYPE_NAME);
        let query = fixture.query_with_root(&root);
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
    fn distinct_keeps_the_first_occurrence_of_each_saved_identity() {
        let fixture = build_fixture();
        let root = fixture.described("distinct", DISTINCT_TYPE_NAME);
        let before = root.into_model().unwrap();

        let (instance, members) = run_over(&fixture, &root, &[10, 11, 10, 12, 11]);
        let members = members.unwrap();
        assert_eq!(ids_of(&members), vec![id(10), id(11), id(12)]);
        assert_eq!(members, vec![fixture.saved(10), fixture.saved(11), fixture.saved(12)]);
        assert_step(&step_records(&instance)[0], ExecutionStatus::Complete, true);
        assert_eq!(root.into_model().unwrap(), before, "the definition is unchanged");
    }

    #[test]
    fn distinct_always_publishes_a_new_result_collection() {
        for sources in [&[][..], &[10, 11, 12][..]] {
            let fixture = build_fixture();
            let root = fixture.described("distinct", DISTINCT_TYPE_NAME);
            let query = fixture.query_with_root(&root);
            let input = fixture.collection_of("distinct-input", sources);
            let input_holon = input.as_holon_reference().clone();
            let execution = query
                .begin_execution(&fixture.context, fixture.focal_space(), Some(input), Vec::new())
                .unwrap();
            let result = execution.run().unwrap();

            assert_ne!(result.as_holon_reference(), &input_holon, "{sources:?}: never the input");
            let members = result.members().unwrap();
            assert_eq!(
                ids_of(&members),
                sources.iter().map(|value| id(*value)).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn distinct_keeps_different_identities_with_equal_values() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let first = world.member("first", &book, &[("Title", text("Same"))]);
        let second = world.member("second", &book, &[("Title", text("Same"))]);
        let root = world.fixture.described("distinct", DISTINCT_TYPE_NAME);

        let (_, members) =
            world.run(&root, &[first.clone(), second.clone(), first.clone(), second.clone()]);
        assert_eq!(members.unwrap(), vec![first, second]);
    }

    #[test]
    fn distinct_treats_each_reference_phase_as_its_own_identity() {
        let world = SortWorld::new();
        let context = &world.fixture.context;
        let transient = new_test_holon(context, "lineage").unwrap();
        let staged: HolonReference =
            context.mutation().stage_new_holon(transient.clone()).unwrap().into();
        let transient: HolonReference = transient.into();
        let root = world.fixture.described("distinct", DISTINCT_TYPE_NAME);

        let (_, members) = world
            .run(&root, &[transient.clone(), staged.clone(), transient.clone(), staged.clone()]);
        assert_eq!(members.unwrap(), vec![transient, staged]);
    }

    #[test]
    fn distinct_position_in_the_chain_decides_the_survivors() {
        // Expand(AuthoredBy) over [20, 30] is [24, 29, 24, 29, 24].
        let cases: [(&[(&str, Option<i64>)], Vec<u8>); 5] = [
            (&[(DISTINCT_TYPE_NAME, None)], vec![24, 29]),
            (&[(SKIP_TYPE_NAME, Some(1)), (DISTINCT_TYPE_NAME, None)], vec![29, 24]),
            (&[(DISTINCT_TYPE_NAME, None), (SKIP_TYPE_NAME, Some(1))], vec![29]),
            (&[(LIMIT_TYPE_NAME, Some(1)), (DISTINCT_TYPE_NAME, None)], vec![24]),
            (&[(DISTINCT_TYPE_NAME, None), (LIMIT_TYPE_NAME, Some(1))], vec![24]),
        ];
        for (steps, expected) in cases {
            let fixture = build_fixture();
            let mut expand = fixture.expand("expand", "AuthoredBy");
            let mut tail: Vec<TransientReference> = steps
                .iter()
                .enumerate()
                .map(|(index, (type_name, count))| {
                    let key = format!("step-{index}");
                    match *type_name {
                        DISTINCT_TYPE_NAME => fixture.described(&key, DISTINCT_TYPE_NAME),
                        _ => fixture.paginate(&key, type_name, *count),
                    }
                })
                .collect();
            for index in (1..tail.len()).rev() {
                let next = tail[index].clone();
                chain(&mut tail[index - 1], &next);
            }
            chain(&mut expand, &tail[0]);

            let (_, members) = run_over(&fixture, &expand, &[20, 30]);
            assert_eq!(
                ids_of(&members.unwrap()),
                expected.iter().map(|value| id(*value)).collect::<Vec<_>>(),
                "Expand -> {steps:?}"
            );
        }
    }

    #[test]
    fn distinct_after_order_by_keeps_the_sorted_order_and_never_sorts_itself() {
        let world = SortWorld::new();
        let book = world.member_type("Book", &[("Title", true, &world.string_type)]);
        let [a, b, c] = ["A", "B", "C"]
            .map(|title| world.member(&format!("book-{title}"), &book, &[("Title", text(title))]));
        let input = [c.clone(), a.clone(), c.clone(), b.clone(), a.clone()];

        let mut order_by = world.order_by(vec![world.spec(
            "title",
            Some(text("Title")),
            Some("Descending"),
            None,
        )]);
        let distinct = world.fixture.described("distinct", DISTINCT_TYPE_NAME);
        chain(&mut order_by, &distinct);
        let (_, sorted) = world.run(&order_by, &input);
        assert_eq!(sorted.unwrap(), vec![c.clone(), b.clone(), a.clone()]);

        let unsorted = world.fixture.described("unsorted", DISTINCT_TYPE_NAME);
        let (_, members) = world.run(&unsorted, &input);
        assert_eq!(members.unwrap(), vec![c, a, b], "input order, not sort order");
    }

    #[test]
    fn a_predicate_attached_to_distinct_fails_that_step() {
        let fixture = build_fixture();
        let mut expand = fixture.expand("expand", "AuthoredBy");
        let mut distinct = fixture.described("distinct", DISTINCT_TYPE_NAME);
        let predicate = fixture.described("predicate", "QueryPredicate");
        distinct
            .add_related_holons(QueryRelationshipTypeName::SeedPredicate, vec![predicate.into()])
            .unwrap();
        chain(&mut expand, &distinct);

        let (instance, members) = run_over(&fixture, &expand, &[20]);
        let error = members.unwrap_err();
        assert!(
            matches!(&error, HolonError::NotImplemented(detail) if detail.contains("SeedPredicate")),
            "unexpected error: {error:?}"
        );
        let records = step_records(&instance);
        assert_eq!(records.len(), 2);
        assert_step(&records[0], ExecutionStatus::Complete, true);
        assert_step(&records[1], ExecutionStatus::Failed, false);
        assert!(related_members(&instance, QueryRelationshipTypeName::ExecutionResult)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_failing_successor_leaves_distinct_complete() {
        let fixture = build_fixture();
        let mut distinct = fixture.described("distinct", DISTINCT_TYPE_NAME);
        let skip = fixture.paginate("skip", SKIP_TYPE_NAME, None);
        chain(&mut distinct, &skip);

        let (instance, members) = run_over(&fixture, &distinct, &[10, 10]);
        assert!(
            matches!(members.unwrap_err(), HolonError::EmptyField(name) if name == "SkipCount")
        );
        let records = step_records(&instance);
        assert_step(&records[0], ExecutionStatus::Complete, true);
        assert_step(&records[1], ExecutionStatus::Failed, false);
    }
}
