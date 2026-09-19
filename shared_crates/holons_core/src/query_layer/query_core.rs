//! QRY1 — descriptor-backed Query runtime scaffold.
//!
//! This module is the internal direct-execution seam for a reusable `Query`
//! definition. It establishes the definition/runtime boundary only:
//!
//! - a typed entry over a `HolonReference` described as `Query`;
//! - transient `ExecutionInstance` and root `QueryExpressionExecution` records
//!   linked to the definition and to the caller-supplied input;
//! - `Pending -> Running -> Failed` status progression;
//! - `HolonError::NotImplemented` at the (still unimplemented) operator boundary.
//!
//! It executes no expression, traverses no `Next`/`Subtree`, resolves no
//! parameter binding, performs no storage discovery, and never writes runtime
//! state onto the reusable Query or QueryExpression definitions. The legacy
//! `query.rs` compatibility surface is untouched and lives beside this module.
//!
//! The *input carrier* minted here is an internal transient transport: it exists
//! only so the schema-shaped `Input -> HolonCollection` relationship can be
//! represented for a runtime `HolonCollection`. It is not persisted and is not
//! part of the Query definition boundary.

use std::sync::Arc;

use base_types::{MapEnumValue, MapString};
use core_types::HolonError;
use type_names::{
    CoreRelationshipTypeName, QueryPropertyTypeName, QueryRelationshipTypeName, ToRelationshipName,
};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::HolonCollection;
use crate::descriptors::resolve_core_descriptor;
use crate::reference_layer::{
    HolonCollectionApi, HolonReference, ReadableHolon, TransientReference, WritableHolon,
};

/// Descriptor keys the scaffold resolves from the loaded Query and Core schemas.
const QUERY_TYPE_NAME: &str = "Query";
const HOLON_COLLECTION_DESCRIPTOR_KEY: &str = "HolonCollection.HolonType";
const EXECUTION_INSTANCE_DESCRIPTOR_KEY: &str = "ExecutionInstance.HolonType";
const QUERY_EXPRESSION_EXECUTION_DESCRIPTOR_KEY: &str = "QueryExpressionExecution.HolonType";

/// Keys of the transient runtime records minted per invocation.
const EXECUTION_INSTANCE_KEY: &str = "execution-instance";
const QUERY_EXPRESSION_EXECUTION_KEY: &str = "query-expression-execution";
const INPUT_CARRIER_KEY: &str = "query-input";

/// Typed entry over a saved or staged holon described as `Query`.
#[derive(Debug, Clone)]
pub struct QueryReference(HolonReference);

impl QueryReference {
    /// Wraps a holon reference after verifying it is described as `Query`.
    pub fn new(reference: HolonReference) -> Result<Self, HolonError> {
        let descriptor = reference.holon_descriptor()?;
        let found = descriptor.header().type_name()?;
        if found.0 != QUERY_TYPE_NAME {
            return Err(HolonError::WrongDescriptorKind {
                expected: QUERY_TYPE_NAME.to_string(),
                found: found.to_string(),
                descriptor: reference.summarize()?,
            });
        }
        Ok(Self(reference))
    }

    /// Creates the transient runtime records for one invocation without running it.
    ///
    /// `input` is the caller's explicit runtime collection. `bindings` are
    /// invocation-level `QueryParameterBinding` references; QRY1 accepts them
    /// and does nothing with them (QRY3 owns their semantics). Both records
    /// start `Pending`.
    pub fn begin_execution(
        &self,
        context: &Arc<TransactionContext>,
        input: HolonCollection,
        bindings: Vec<HolonReference>,
    ) -> Result<QueryExecution, HolonError> {
        let _ = bindings; // carried unresolved; not recorded anywhere in QRY1
        let root_expression = exactly_one(&self.0, QueryRelationshipTypeName::RootExpression)?;
        let input_carrier = materialize_input_carrier(context, &input)?;

        let mut instance =
            new_runtime_record(context, EXECUTION_INSTANCE_KEY, EXECUTION_INSTANCE_DESCRIPTOR_KEY)?;
        instance
            .add_related_holons(QueryRelationshipTypeName::ExecutesQuery, vec![self.0.clone()])?;

        let mut root_execution = new_runtime_record(
            context,
            QUERY_EXPRESSION_EXECUTION_KEY,
            QUERY_EXPRESSION_EXECUTION_DESCRIPTOR_KEY,
        )?;
        root_execution
            .add_related_holons(
                QueryRelationshipTypeName::ExecutesExpression,
                vec![root_expression],
            )?
            .add_related_holons(QueryRelationshipTypeName::Input, vec![input_carrier.into()])?;

        instance.add_related_holons(
            QueryRelationshipTypeName::ExpressionExecutions,
            vec![root_execution.clone().into()],
        )?;

        Ok(QueryExecution { instance, root_execution })
    }
}

/// Transient runtime state for one Query invocation.
#[derive(Debug)]
pub struct QueryExecution {
    instance: TransientReference,
    root_execution: TransientReference,
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

    /// Progresses both records to `Running`, reaches the unimplemented operator
    /// boundary, records `Failed` on both, and returns `NotImplemented`.
    ///
    /// No `Result`, `ExecutionResult`, or success collection is created.
    pub fn run(mut self) -> Result<HolonCollection, HolonError> {
        set_status(&mut self.instance, ExecutionStatus::Running)?;
        set_status(&mut self.root_execution, ExecutionStatus::Running)?;

        // QRY2 introduces the first executable expressions here.
        set_status(&mut self.root_execution, ExecutionStatus::Failed)?;
        set_status(&mut self.instance, ExecutionStatus::Failed)?;

        Err(HolonError::NotImplemented("QueryExpression execution (QRY2)".to_string()))
    }
}

/// Lifecycle states QRY1 records, mirrored from `QueryExecutionStatus.MapEnumValueType`.
#[derive(Debug, Clone, Copy)]
enum ExecutionStatus {
    Pending,
    Running,
    Failed,
}

impl ExecutionStatus {
    fn as_enum_value(self) -> MapEnumValue {
        let variant = match self {
            Self::Pending => "Pending",
            Self::Running => "Running",
            Self::Failed => "Failed",
        };
        MapEnumValue(MapString(variant.to_string()))
    }
}

/// Reads the runtime input back out of an input carrier holon.
///
/// Used by the QueryDance adapter to turn a request's `InitialInput` into the
/// explicit runtime collection the direct seam accepts.
pub(crate) fn read_input_carrier(carrier: &HolonReference) -> Result<HolonCollection, HolonError> {
    let members = related_members(carrier, CoreRelationshipTypeName::CollectionMembers)?;
    let mut collection = HolonCollection::new_transient();
    collection.add_references(members)?;
    Ok(collection)
}

fn materialize_input_carrier(
    context: &Arc<TransactionContext>,
    input: &HolonCollection,
) -> Result<TransientReference, HolonError> {
    let mut carrier =
        context.mutation().new_holon(Some(MapString(INPUT_CARRIER_KEY.to_string())))?;
    carrier.with_descriptor(resolve_core_descriptor(context, HOLON_COLLECTION_DESCRIPTOR_KEY)?)?;
    let members = input.get_members().clone();
    if !members.is_empty() {
        carrier.add_related_holons(CoreRelationshipTypeName::CollectionMembers, members)?;
    }
    Ok(carrier)
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
    let members = related_members(holon, relationship)?;
    match members.as_slice() {
        [single] => Ok(single.clone()),
        [] => Err(HolonError::MissingRequiredRelationship {
            relationship: relationship_name,
            descriptor: holon.summarize()?,
        }),
        many => Err(HolonError::MultipleRelatedHolons {
            relationship: relationship_name,
            descriptor: holon.summarize()?,
            count: many.len(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{
        build_context, new_holon_type_descriptor, new_test_holon,
    };

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
    fn query_reference_accepts_query_descriptor() {
        let context = build_context();
        let query_descriptor =
            new_holon_type_descriptor(&context, "Query.HolonType", "Query").unwrap();
        let mut holon = new_test_holon(&context, "a-query").unwrap();
        holon.with_descriptor(query_descriptor.into()).unwrap();

        assert!(QueryReference::new(holon.into()).is_ok());
    }
}
