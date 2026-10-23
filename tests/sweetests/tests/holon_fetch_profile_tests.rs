//! Matched websocket/guest/HDK captures for exact holon and relationship reads.
//! Build the coordinator with `holons/holon-fetch-profiling` before packing the DNA.
use async_trait::async_trait;
use holochain::prelude::*;
use holochain_client::{
    AdminWebsocket, AgentSigner, AllowedOrigins, AppWebsocket, IssueAppAuthenticationTokenPayload,
    LairAgentSigner, WebsocketConfig, ZomeCallTarget,
};
use holons_boundary::envelopes::{DanceRequestEnvelope, DanceResponseEnvelope};
use holons_core::HolonError;
use holons_prelude::prelude::*;
use holons_test::{init_runtime_with_initiator, setup_test_conductor, DancesTestCase};
use holons_trust_channel::{DanceEnvelopeTransport, TrustChannel};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

struct TimedSigner {
    inner: LairAgentSigner,
    micros: AtomicU64,
}
#[async_trait]
impl AgentSigner for TimedSigner {
    async fn sign(
        &self,
        cell: &CellId,
        provenance: AgentPubKey,
        data: Arc<[u8]>,
    ) -> anyhow::Result<Signature> {
        let started = Instant::now();
        let result = self.inner.sign(cell, provenance, data).await;
        self.micros.store(started.elapsed().as_micros() as u64, Ordering::Relaxed);
        result
    }
    fn get_provenance(&self, cell: &CellId) -> Option<AgentPubKey> {
        self.inner.get_provenance(cell)
    }
    fn get_cap_secret(&self, cell: &CellId) -> Option<CapSecret> {
        self.inner.get_cap_secret(cell)
    }
}
struct WebsocketTransport {
    ws: AppWebsocket,
    cell: CellId,
    signer: Arc<TimedSigner>,
}
impl std::fmt::Debug for WebsocketTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebsocketTransport").finish_non_exhaustive()
    }
}
#[async_trait]
impl DanceEnvelopeTransport for WebsocketTransport {
    async fn initiate_dance_envelope(
        &self,
        envelope: DanceRequestEnvelope,
    ) -> Result<DanceResponseEnvelope, HolonError> {
        let dance = envelope.request.transport_label().to_owned();
        let profile = matches!(dance.as_str(), "get_holon_by_id" | "query_relationships");
        let started = Instant::now();
        let payload = ExternIO::encode(envelope).unwrap();
        let encode_us = started.elapsed().as_micros();
        let request_bytes = payload.0.len();
        let started = Instant::now();
        let result = self
            .ws
            .call_zome(
                ZomeCallTarget::CellId(self.cell.clone()),
                "holons".into(),
                "dance_adapter".into(),
                payload,
            )
            .await
            .map_err(|e| HolonError::ConductorError(format!("{e:?}")))?;
        let call_us = started.elapsed().as_micros();
        let started = Instant::now();
        let response = result.decode().unwrap();
        if profile {
            println!("[read-call-client] dance={dance} encode_us={encode_us} call_us={call_us} signer_us={} decode_us={} request_bytes={request_bytes} response_bytes={}", self.signer.micros.load(Ordering::Relaxed), started.elapsed().as_micros(), result.0.len());
        }
        Ok(response)
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "explicit bounded diagnostic; builds and bootstraps an isolated conductor"]
async fn split_read_call_costs() {
    holons_test::init_tracing();
    let backend = setup_test_conductor().await;
    let admin = AdminWebsocket::connect(
        (
            std::net::Ipv4Addr::LOCALHOST,
            backend.conductor.get_arbitrary_admin_websocket_port().unwrap(),
        ),
        None,
    )
    .await
    .unwrap();
    let port =
        admin.attach_app_interface(0, None, AllowedOrigins::Any, Some("app".into())).await.unwrap();
    let token = admin
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            "app".into(),
        ))
        .await
        .unwrap();
    let cell = backend.cell.cell_id().clone();
    let mut inner = LairAgentSigner::new(Arc::new(backend.conductor.keystore().lair_client()));
    inner.add_credentials(cell.clone(), backend.agent.clone());
    let signer = Arc::new(TimedSigner { inner, micros: AtomicU64::new(0) });
    let mut config = WebsocketConfig::CLIENT_DEFAULT;
    config.default_request_timeout = Duration::from_secs(240);
    config.max_frame_size = config.max_message_size;
    let ws = AppWebsocket::connect_with_config(
        (std::net::Ipv4Addr::LOCALHOST, port),
        Arc::new(config),
        token.token,
        signer.clone(),
        None,
    )
    .await
    .unwrap();
    let transport = Arc::new(WebsocketTransport { ws, cell, signer });
    let mut fixture = DancesTestCase::default();
    let (runtime, tx_id) =
        init_runtime_with_initiator(&mut fixture, Arc::new(TrustChannel::new(transport))).await;
    let context = runtime.session().get_transaction(&tx_id).unwrap();
    let subject =
        context.lookup().get_saved_holon_by_key(&MapString::from("HolonSpace.HolonType")).unwrap();
    let id = subject.get_id().unwrap();
    let relationship = RelationshipName(MapString::from("DescribedBy"));
    let expected_target = context
        .lookup()
        .get_saved_holon_by_key(&"MetaHolonType.MetaTypeDescriptor".into())
        .unwrap()
        .holon_id();
    // Direct Dance requests force transport calls instead of serving the client's saved caches.
    for dance in ["get_holon_by_id", "query_relationships"] {
        for sample in 0..5 {
            println!("[read-call-sample] dance={dance} index={sample}");
            let request = if dance == "get_holon_by_id" {
                holon_dance_builders::build_get_holon_by_id_dance_request(id.clone()).unwrap()
            } else {
                use holons_core::query_layer::{Node, NodeCollection, QueryExpression};
                holon_dance_builders::build_query_relationships_dance_request(
                    NodeCollection {
                        members: vec![Node::new(subject.clone().into(), None)],
                        query_spec: None,
                    },
                    QueryExpression::new(relationship.clone()),
                )
                .unwrap()
            };
            let started = Instant::now();
            let response = context.initiate_dance(request).await.unwrap();
            println!(
                "[read-call-total] dance={dance} elapsed_us={}",
                started.elapsed().as_micros()
            );
            assert_eq!(response.status_code, holons_core::dances::ResponseStatusCode::OK);
            if dance == "query_relationships" {
                let holons_core::dances::ResponseBody::NodeCollection(collection) = response.body
                else {
                    panic!("expected a relationship query result");
                };
                assert_eq!(collection.members.len(), 1);
                let relationships = collection.members[0].relationships.as_ref().unwrap();
                let targets = relationships.0.get(&relationship).unwrap();
                assert_eq!(targets.members.len(), 1);
                assert_eq!(targets.members[0].source_holon.holon_id().unwrap(), expected_target);
            }
        }
    }
    runtime.session().archive_transaction(&tx_id).unwrap();
    drop(context);
    drop(runtime);
    backend.shutdown().await;
}
