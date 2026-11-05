//! Explicit-ID cache population through the real host/wire/guest boundary.
use async_trait::async_trait;
use core_types::LocalId;
use holons_boundary::envelopes::{
    DanceEnvelopeRequest, DanceRequestEnvelope, DanceResponseEnvelope,
};
use holons_boundary::RequestBodyWire;
use holons_client::ClientHolonService;
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager, transactions::TransactionContext,
};
use holons_core::descriptors::Descriptor;
use holons_core::reference_layer::HolonSpaceBehavior;
use holons_core::ServiceRoutingPolicy;
use holons_prelude::prelude::*;
use holons_test::{
    init_runtime_with_initiator, init_tracing, setup_test_conductor, DancesTestCase,
    MockConductorConfig,
};
use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug)]
struct MeasuredTransport {
    backend: Arc<MockConductorConfig>,
    calls: Mutex<Vec<(String, Vec<HolonId>)>>,
}
#[async_trait]
impl DanceEnvelopeTransport for MeasuredTransport {
    async fn initiate_dance_envelope(
        &self,
        envelope: DanceRequestEnvelope,
    ) -> Result<DanceResponseEnvelope, HolonError> {
        let label = envelope.request.transport_label().to_owned();
        let ids = match &envelope.request {
            DanceEnvelopeRequest::Legacy(request) => match &request.body {
                RequestBodyWire::SavedHolonIds(ids) => {
                    assert!(envelope.session.as_ref().unwrap().is_restricted_cache_read());
                    ids.clone()
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        self.calls.lock().unwrap().push((label, ids));
        self.backend.initiate_dance_envelope(envelope).await
    }
}

fn fresh_context(
    initiator: Arc<dyn holons_core::dances::DanceInitiator>,
    anchor: HolonId,
) -> (Arc<HolonSpaceManager>, Arc<TransactionContext>) {
    let space = HolonSpaceManager::new_with_managers(
        Some(initiator),
        Arc::new(ClientHolonService::development_default()),
        Some(anchor),
        ServiceRoutingPolicy::Combined,
    );
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    (space, context)
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_cache_batches_are_missing_only_errors_only_and_shared_across_requests() {
    init_tracing();
    let backend = setup_test_conductor().await;
    let measured = Arc::new(MeasuredTransport { backend, calls: Mutex::new(Vec::new()) });
    let initiator = Arc::new(TrustChannel::new(measured.clone()));
    let (runtime, tx) =
        init_runtime_with_initiator(&mut DancesTestCase::default(), initiator.clone()).await;
    let context = runtime.session().get_transaction(&tx).unwrap();
    let anchor = runtime.session().space_manager().get_space_holon_id().unwrap().unwrap();
    let context = if context.is_open() {
        context
    } else {
        let tx = runtime.session().begin_transaction().await.unwrap();
        runtime.session().get_transaction(&tx).unwrap()
    };
    let ids: Vec<_> = context
        .lookup()
        .get_all_holons()
        .unwrap()
        .get_members()
        .iter()
        .map(|reference| reference.holon_id().unwrap())
        .take(64)
        .collect();
    assert_eq!(ids.len(), 64, "bootstrap supplies the explicit saved-ID fixture");
    println!(
        "[ensure_cached evidence] revision={} environment=real-Holochain-conductor input=64",
        option_env!("CACHE_TEST_REVISION").unwrap_or("working-tree")
    );
    for warm in [0, 54, 64] {
        let (space, context) = fresh_context(initiator.clone(), anchor.clone());
        assert!(context.ensure_cached(&ids[..warm]).is_empty());
        let mut draft =
            context.mutation().new_holon(Some("CachePopulation.UnrelatedDraft".into())).unwrap();
        draft.with_property_value("Description", "uncommitted").unwrap();
        let draft = context.mutation().stage_new_holon(draft).unwrap();
        let transient_before = context.lookup().transient_count().unwrap();
        let reference = HolonReference::smart_from_id(context.space_read_handle(), ids[0].clone());
        measured.calls.lock().unwrap().clear();
        let started = Instant::now();
        let mut duplicates = ids.clone();
        duplicates.extend(ids.clone());
        let errors = context.ensure_cached(&duplicates);
        assert!(errors.is_empty(), "{errors:?}");
        let calls = measured.calls.lock().unwrap().clone();
        if warm == 64 {
            assert!(calls.is_empty(), "{calls:?}");
        } else {
            assert_eq!(calls, vec![("get_holons_by_ids".to_string(), ids[warm..].to_vec())]);
        }
        println!("[ensure_cached evidence] requested=128 distinct=64 hits={warm} missing={} guest_invocations={} errors={} elapsed_us={} storage_reads=not-measured", 64-warm, calls.len(), errors.len(), started.elapsed().as_micros());
        reference.key().unwrap();
        let next = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
        assert!(next.ensure_cached(&ids).is_empty());
        assert!(next.ensure_cached(&[]).is_empty());
        assert_eq!(
            *measured.calls.lock().unwrap(),
            calls,
            "ordinary references and subsequent requests reuse populated cache"
        );
        assert_eq!(context.lookup().staged_count().unwrap(), 1);
        assert_eq!(context.lookup().transient_count().unwrap(), transient_before);
        assert_eq!(
            draft.property_value("Description").unwrap(),
            Some("uncommitted".to_base_value())
        );
        assert!(context.is_open());
    }
    // A real prepared descriptor warms only its plural targets at navigation time.
    let (_space, context) = fresh_context(initiator.clone(), anchor.clone());
    let source = context.get_space_holon().unwrap().unwrap();
    let effective = source.effective_descriptor().unwrap();
    let members = effective
        .holon()
        .related_holons("PluralNavigableRelationships")
        .unwrap()
        .read()
        .unwrap()
        .get_members()
        .clone();
    let mut seen = std::collections::HashSet::new();
    let saved_ids: Vec<_> = members
        .iter()
        .filter_map(|member| match member {
            HolonReference::Smart(saved) => Some(saved.holon_id()),
            _ => None,
        })
        .filter(|id| seen.insert(id.clone()))
        .collect();
    measured.calls.lock().unwrap().clear();
    effective.singular_navigable_relationships().unwrap();
    assert!(measured.calls.lock().unwrap().is_empty());
    let plural = effective.plural_navigable_relationships().unwrap();
    let calls = measured.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1, "cold plural targets use one bulk invocation");
    assert_eq!(calls[0].0, "get_holons_by_ids");
    let missing = &calls[0].1;
    assert!(!missing.is_empty(), "fixture must exercise cold plural targets");
    assert!(missing.iter().all(|id| saved_ids.contains(id)));
    assert_eq!(missing.iter().collect::<std::collections::HashSet<_>>().len(), missing.len());
    assert_eq!(
        plural.iter().map(|entry| entry.holon().full_reference_id_string()).collect::<Vec<_>>(),
        members.iter().map(|member| member.full_reference_id_string()).collect::<Vec<_>>()
    );
    measured.calls.lock().unwrap().clear();
    for descriptor in plural {
        descriptor.holon().property_value("DisplayName").unwrap();
        descriptor.holon().property_value("Description").unwrap();
    }
    effective.plural_navigable_relationships().unwrap();
    assert!(
        measured.calls.lock().unwrap().is_empty(),
        "labels and repeated navigation reuse the warmed host cache"
    );
    println!("[plural navigation evidence] saved_targets={} cold_targets={} bulk_guest_invocations=1 subsequent_label_guest_invocations=0", saved_ids.len(), missing.len());

    // A malformed ID is a per-ID error, while a valid target is still populated.
    let (_space, context) = fresh_context(initiator, anchor);
    let bad = HolonId::Local(LocalId(vec![1]));
    let errors = context.ensure_cached(&[ids[0].clone(), bad.clone()]);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].ids, vec![bad]);
    measured.calls.lock().unwrap().clear();
    HolonReference::smart_from_id(context.space_read_handle(), ids[0].clone()).key().unwrap();
    assert!(measured.calls.lock().unwrap().is_empty(), "successful population is reused");
}
