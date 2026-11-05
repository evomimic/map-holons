//! Real-storage and adapter boundaries; exhaustive query semantics live in holons_core units.
use std::sync::Arc;

use holons_core::dances::DanceInvocation;
use holons_core::descriptors::{resolve_core_descriptor, HolonDescriptor};
use holons_core::query_layer::authoring;
use holons_prelude::prelude::*;
use map_commands_contract::{
    MapCommand, MapResult, SpaceCommand, TransactionAction, TransactionCommand,
};
use map_commands_runtime::{ExecutionPolicy, Runtime};
use type_names::*;

use crate::execution_steps::query_executor::{
    execute_direct, execute_query_dance, query_dance_invocation, ResolvedExpectation as Expected,
    ResolvedInput as Input,
};
use crate::fixture_cases::query_boundary_fixture::*;

/// Runs a compact contract over one persisted dataset and normal production references.
pub async fn run(runtime: &Runtime) {
    let MapResult::TransactionCreated { tx_id } = runtime
        .execute_command(
            MapCommand::Space(SpaceCommand::BeginTransaction),
            ExecutionPolicy::default(),
        )
        .await
        .unwrap()
    else {
        panic!("expected a query transaction")
    };
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let saved = |key: &str| -> HolonReference {
        context.lookup().get_saved_holon_by_key(&MapString::from(key)).unwrap().into()
    };
    let books: Vec<_> = BOOK_KEYS.iter().map(|key| saved(key)).collect();
    let people: Vec<_> = PERSON_KEYS.iter().map(|key| saved(key)).collect();
    let book_type = saved("Book.HolonType");
    let person_type = saved("Person.HolonType");
    let seed = saved(SEED_QUERY);
    let declared = saved(DECLARED_QUERY);
    let inverse = saved(INVERSE_QUERY);
    let book_input = |members| Input::Collection { element_type: book_type.clone(), members };

    direct(
        "saved SeedHolons / real Owns",
        &context,
        seed.clone(),
        Input::None,
        Expected::OwnsOfFocalSpace { must_include: vec![books[0].clone(), seed.clone()] },
    );
    direct(
        "saved declared navigation / order, duplicates, empty membership",
        &context,
        declared.clone(),
        book_input(vec![books[0].clone(), books[1].clone(), books[4].clone(), books[0].clone()]),
        Expected::Members(vec![
            people[0].clone(),
            people[1].clone(),
            people[0].clone(),
            people[0].clone(),
            people[1].clone(),
        ]),
    );
    direct(
        "saved inverse navigation",
        &context,
        inverse,
        Input::Collection { element_type: person_type.clone(), members: vec![people[0].clone()] },
        Expected::Members(vec![books[0].clone(), books[1].clone(), books[2].clone()]),
    );
    direct(
        "single-holon input",
        &context,
        declared,
        Input::SingleHolon(books[0].clone()),
        Expected::Members(people.clone()),
    );

    // One pipeline connects actual enum/string/integer descriptors and every transform family.
    let book_descriptor = HolonDescriptor::from_holon(book_type.clone());
    let published = comparison(
        &context,
        &book_descriptor,
        "PublicationStatus",
        CoreOperatorTypeName::EqualsOperator,
        MapEnumValue(MapString::from("Published")).to_base_value(),
    );
    let roads = comparison(
        &context,
        &book_descriptor,
        "Title",
        CoreOperatorTypeName::ContainsOperator,
        MapString::from("road").to_base_value(),
    );
    let draft = comparison(
        &context,
        &book_descriptor,
        "Subtitle",
        CoreOperatorTypeName::ContainsOperator,
        MapString::from("draft").to_base_value(),
    );
    let not_draft = authoring::new_not(&context, draft).unwrap();
    let road_and_final = authoring::new_all_of(&context, vec![roads, not_draft.into()]).unwrap();
    let predicate =
        authoring::new_any_of(&context, vec![published, road_and_final.into()]).unwrap();
    let mut filter = authoring::new_filter(&context, predicate.into()).unwrap();
    let mut distinct = described(&context, "distinct", QueryHolonTypeName::Distinct);
    let mut order = described(&context, "order", QueryHolonTypeName::OrderBy);
    let mut spec = described(&context, "spec", QueryHolonTypeName::OrderBySpec);
    spec.with_property_value(QueryPropertyTypeName::PropertyName, MapString::from("PageCount"))
        .unwrap();
    order.add_related_holons(QueryRelationshipTypeName::OrderBySpecs, vec![spec.into()]).unwrap();
    let mut skip = described(&context, "skip", QueryHolonTypeName::Skip);
    skip.with_property_value(QueryPropertyTypeName::SkipCount, MapInteger(1)).unwrap();
    let mut limit = described(&context, "limit", QueryHolonTypeName::Limit);
    limit.with_property_value(QueryPropertyTypeName::LimitCount, MapInteger(1)).unwrap();
    skip.add_related_holons(QueryRelationshipTypeName::Next, vec![limit.into()]).unwrap();
    order.add_related_holons(QueryRelationshipTypeName::Next, vec![skip.into()]).unwrap();
    distinct.add_related_holons(QueryRelationshipTypeName::Next, vec![order.into()]).unwrap();
    filter.add_related_holons(QueryRelationshipTypeName::Next, vec![distinct.into()]).unwrap();
    let mut expand = described(&context, "expand", QueryHolonTypeName::Expand);
    expand
        .with_property_value(
            QueryPropertyTypeName::ExpansionRelationshipName,
            MapString::from("AuthorOf"),
        )
        .unwrap();
    expand.add_related_holons(QueryRelationshipTypeName::Next, vec![filter.into()]).unwrap();
    let pipeline = query(&context, "pipeline", expand.into());
    direct(
        "transient compound predicate / Distinct / OrderBy defaults / Skip / Limit",
        &context,
        pipeline,
        Input::Collection {
            element_type: person_type,
            members: vec![people[0].clone(), people[1].clone(), people[0].clone()],
        },
        Expected::Members(vec![books[0].clone()]),
    );

    // A second actual Owns evaluation covers predicate attachment and absent adapter input.
    let root_type = HolonDescriptor::from_holon(saved("HolonType.TypeDescriptor"));
    let key = comparison(
        &context,
        &root_type,
        "Key",
        CoreOperatorTypeName::EqualsOperator,
        MapString::from(BOOK_KEYS[0]).to_base_value(),
    );
    let mut filtered_seed = described(&context, "filtered-seed", QueryHolonTypeName::SeedHolons);
    filtered_seed.add_related_holons(QueryRelationshipTypeName::SeedPredicate, vec![key]).unwrap();
    dance(
        "filtered SeedHolons / absent adapter input",
        runtime,
        &context,
        query(&context, "filtered-seed-query", filtered_seed.into()),
        None,
        vec![],
        Expected::Members(vec![books[0].clone()]),
    )
    .await;

    let mut page = described(&context, "adapter-page", QueryHolonTypeName::Limit);
    page.with_property_value(QueryPropertyTypeName::LimitCount, MapInteger(3)).unwrap();
    let page = query(&context, "adapter-page-query", page.into());
    let page_input = || {
        Some((
            book_type.clone(),
            vec![books[0].clone(), books[1].clone(), books[0].clone(), books[2].clone()],
        ))
    };
    dance(
        "supplied adapter input / response type, order and duplicates",
        runtime,
        &context,
        page.clone(),
        page_input(),
        vec![],
        Expected::Members(vec![books[0].clone(), books[1].clone(), books[0].clone()]),
    )
    .await;
    dance(
        "adapter bindings forwarded to QueryCore",
        runtime,
        &context,
        page.clone(),
        page_input(),
        vec![books[0].clone()],
        Expected::Error(HolonErrorKind::NotImplemented),
    )
    .await;
    dance(
        "source rejects supplied adapter input",
        runtime,
        &context,
        seed,
        page_input(),
        vec![],
        Expected::Error(HolonErrorKind::InvalidParameter),
    )
    .await;

    let mut bogus = described(&context, "bogus-expand", QueryHolonTypeName::Expand);
    bogus
        .with_property_value(
            QueryPropertyTypeName::ExpansionRelationshipName,
            MapString::from("NoSuchRelationship"),
        )
        .unwrap();
    let bogus = query(&context, "bogus-query", bogus.into());
    dance(
        "descriptor errors propagate through the adapter",
        runtime,
        &context,
        bogus,
        Some((book_type.clone(), vec![books[0].clone()])),
        vec![],
        Expected::Error(HolonErrorKind::DescriptorDeclarationNotFound),
    )
    .await;

    // Contract rejection belongs to adapter tests, not the operator truth-table matrix.
    let invocation = query_dance_invocation(&context, page.clone(), page_input(), vec![]);
    let mut request = member(invocation.as_holon_reference(), "Request");
    request.remove_related_holons("RequestedQuery", vec![page.clone()]).unwrap();
    rejects(
        "missing RequestedQuery",
        runtime,
        &context,
        invocation,
        HolonErrorKind::MissingRequiredRelationship,
    )
    .await;

    let invocation = query_dance_invocation(&context, page.clone(), page_input(), vec![]);
    let mut request = member(invocation.as_holon_reference(), "Request");
    let input = member(&request, "InitialInput");
    request
        .remove_related_holons("InitialInput", vec![input])
        .unwrap()
        .add_related_holons("InitialInput", vec![books[0].clone()])
        .unwrap();
    rejects(
        "wrong InitialInput kind",
        runtime,
        &context,
        invocation,
        HolonErrorKind::WrongDescriptorKind,
    )
    .await;

    let invocation = query_dance_invocation(&context, page, page_input(), vec![]);
    let mut holon = invocation.as_holon_reference().clone();
    let request = member(&holon, "Request");
    holon.remove_related_holons("Request", vec![request]).unwrap();
    rejects(
        "missing Request",
        runtime,
        &context,
        invocation,
        HolonErrorKind::MissingRequiredRelationship,
    )
    .await;
}

fn described(
    context: &Arc<TransactionContext>,
    label: &str,
    kind: QueryHolonTypeName,
) -> TransientReference {
    let mut holon = context
        .mutation()
        .new_holon(Some(MapString::from(format!("QueryBoundary.{label}"))))
        .unwrap();
    holon
        .with_descriptor(
            resolve_core_descriptor(context, &holon_type_descriptor_key(&kind.as_holon_name()).0)
                .unwrap(),
        )
        .unwrap();
    holon
}

fn query(context: &Arc<TransactionContext>, label: &str, root: HolonReference) -> HolonReference {
    let mut query = described(context, label, QueryHolonTypeName::Query);
    query.with_property_value("QueryName", MapString::from(label)).unwrap();
    query.add_related_holons(QueryRelationshipTypeName::RootExpression, vec![root]).unwrap();
    query.into()
}

fn comparison(
    context: &Arc<TransactionContext>,
    subject: &HolonDescriptor,
    property: &str,
    operator: CoreOperatorTypeName,
    value: BaseValue,
) -> HolonReference {
    let value_type = subject
        .effective_descriptor()
        .unwrap()
        .get_property_by_name(property)
        .unwrap()
        .value_type()
        .unwrap();
    let operand = authoring::new_literal_operand(context, &value_type, value).unwrap();
    authoring::new_comparison_predicate(context, subject, property, operator, vec![operand.into()])
        .unwrap()
        .into()
}

fn member(source: &HolonReference, name: &str) -> HolonReference {
    let members = source.related_holons(name).unwrap().read().unwrap().get_members().clone();
    assert_eq!(members.len(), 1, "expected one {name}");
    members[0].clone()
}

fn direct(
    label: &str,
    context: &Arc<TransactionContext>,
    query: HolonReference,
    input: Input,
    expected: Expected,
) {
    tracing::info!(case = label, "query contract starting");
    execute_direct(context, query, input, vec![], expected);
    tracing::info!(case = label, "query contract passed");
}

async fn dance(
    label: &str,
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    query: HolonReference,
    input: Option<(HolonReference, Vec<HolonReference>)>,
    bindings: Vec<HolonReference>,
    expected: Expected,
) {
    tracing::info!(case = label, "query contract starting");
    execute_query_dance(runtime, context, query, input, bindings, expected).await;
    tracing::info!(case = label, "query contract passed");
}

async fn rejects(
    label: &str,
    runtime: &Runtime,
    context: &Arc<TransactionContext>,
    invocation: DanceInvocation,
    expected: HolonErrorKind,
) {
    let result = runtime
        .execute_command(
            MapCommand::Transaction(TransactionCommand {
                context: context.clone(),
                action: TransactionAction::DanceV2 { invocation },
            }),
            ExecutionPolicy::default(),
        )
        .await;
    let error = result.expect_err(label);
    assert_eq!(HolonErrorKind::from(&error), expected, "{label}: {error:?}");
    tracing::info!(case = label, "query contract passed");
}
