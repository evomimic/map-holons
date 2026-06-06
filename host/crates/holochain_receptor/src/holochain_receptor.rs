use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use holochain_client::{AdminWebsocket, AgentPubKey, AppWebsocket};
use holochain_types::signal::Signal;

use crate::holochain_conductor_client::HolochainConductorClient;
use crate::host_signal::{decode_signal, HostSignal};
use crate::storage_notification::{to_storage_notification, StorageNotification};
use client_shared_types::holon_space::SpaceInfo;
use client_shared_types::storage_receptor::StorageReceptor;
use client_shared_types::ReceptorType;
use core_types::HolonError;

pub const SIGNAL_CHANNEL_CAPACITY: usize = 64;

/// The Holochain-backed storage receptor.
///
/// Owns the conductor client (websocket transport) and two signal broadcast channels:
/// a raw channel carrying `holochain_types::signal::Signal` and a decoded channel
/// carrying [`HostSignal`]. Neither `TransactionContext` nor request dispatch logic
/// lives here — those are managed by `RuntimeSession` via `init_from_state`.
///
/// `on_signal` is registered during `new()` on the raw `AppWebsocket` before it is
/// wrapped in a mutex. This is the only safe registration window — calling it after
/// the websocket is behind a lock would require holding the lock across an await.
pub struct HolochainReceptor {
    pub receptor_id: String,
    pub receptor_type: ReceptorType,
    pub properties: HashMap<String, String>,
    /// Inner conductor client — shared with the `TrustChannel` written to
    /// `RuntimeInitiatorState` so zome calls flow through the same connection.
    pub client: Arc<HolochainConductorClient>,
    raw_tx: broadcast::Sender<Signal>,
    decoded_tx: broadcast::Sender<HostSignal>,
    notification_tx: broadcast::Sender<StorageNotification>,
}

impl HolochainReceptor {
    /// Construct a new receptor from raw websockets.
    ///
    /// Registers `on_signal` on `app_ws` before wrapping it in a mutex.
    /// Both `app_ws` and `admin_ws` should be `.clone()`d by the caller if the
    /// deprecated path also needs them (both types are Arc-backed).
    pub async fn new(
        receptor_id: String,
        properties: HashMap<String, String>,
        app_ws: AppWebsocket,
        admin_ws: AdminWebsocket,
        rolename: String,
        zomename: String,
        zomefunction: String,
        agent: AgentPubKey,
    ) -> Arc<Self> {
        let (raw_tx, _) = broadcast::channel::<Signal>(SIGNAL_CHANNEL_CAPACITY);
        let (decoded_tx, _) = broadcast::channel::<HostSignal>(SIGNAL_CHANNEL_CAPACITY);
        let (notification_tx, _) =
            broadcast::channel::<StorageNotification>(SIGNAL_CHANNEL_CAPACITY);

        let raw_tx_cb = raw_tx.clone();
        let decoded_tx_cb = decoded_tx.clone();
        let notification_tx_cb = notification_tx.clone();
        let target_zome = zomename.clone();

        // Register before the websocket is wrapped — the only safe point.
        // `send()` errors (no subscribers yet) are intentionally discarded.
        app_ws
            .on_signal(move |signal| {
                let decoded = decode_signal(signal.clone(), &target_zome);
                if let HostSignal::Holons { ref signal, .. } = decoded {
                    let _ = notification_tx_cb.send(to_storage_notification(signal));
                }
                let _ = raw_tx_cb.send(signal);
                let _ = decoded_tx_cb.send(decoded);
            })
            .await;

        let client = Arc::new(HolochainConductorClient {
            app_ws: Arc::new(Mutex::new(Some(app_ws))),
            admin_ws: Arc::new(Mutex::new(Some(admin_ws))),
            rolename,
            zomename,
            zomefunction,
            agent,
        });

        Arc::new(Self {
            receptor_id,
            receptor_type: ReceptorType::Holochain,
            properties,
            client,
            raw_tx,
            decoded_tx,
            notification_tx,
        })
    }

    /// Subscribe to raw `holochain_types::signal::Signal` events.
    /// The receiver will get `RecvError::Lagged` if it falls behind the channel capacity.
    pub fn subscribe_raw(&self) -> broadcast::Receiver<Signal> {
        self.raw_tx.subscribe()
    }

    /// Subscribe to adapter-internal decoded [`HostSignal`] events.
    ///
    /// This is adapter-internal. External consumers should use
    /// [`subscribe_notifications`] which emits identification-only
    /// [`StorageNotification`]s that do not carry holon state.
    pub(crate) fn subscribe_decoded(&self) -> broadcast::Receiver<HostSignal> {
        self.decoded_tx.subscribe()
    }

    /// Subscribe to MAP-facing [`StorageNotification`] events.
    ///
    /// Each notification identifies which holon changed and how, without
    /// carrying holon state. Consumers resolve the current state through the
    /// normal `HolonReference` / cache path using `notification.holon_id`.
    pub fn subscribe_notifications(&self) -> broadcast::Receiver<StorageNotification> {
        self.notification_tx.subscribe()
    }

    /// Query live space info from the conductor (delegates to `HolochainConductorClient`).
    pub async fn get_space_info(&self) -> Result<SpaceInfo, HolonError> {
        self.client.get_all_spaces().await
    }
}

impl StorageReceptor for HolochainReceptor {
    fn receptor_id(&self) -> &str {
        &self.receptor_id
    }

    fn get_space_info(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<SpaceInfo, HolonError>> + Send + '_>,
    > {
        let client = self.client.clone();
        Box::pin(async move { client.get_all_spaces().await })
    }
}

impl fmt::Debug for HolochainReceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HolochainReceptor")
            .field("receptor_id", &self.receptor_id)
            .field("receptor_type", &self.receptor_type)
            .field("properties", &self.properties)
            .finish()
    }
}
