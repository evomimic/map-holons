
use dances_core::dance_request::DanceRequest;
use dances_core::dance_response::{DanceResponse, ResponseStatusCode};
use hdi::prelude::AgentPubKey;

use hdk::prelude::CellId;
use holochain::conductor::api::error::ConductorApiError;
use holochain::sweettest::{SweetAgents, SweetApp, SweetCell, SweetConductor, SweetDnaFile};
use holons_core::core_shared_objects::HolonError;


pub trait ZomeClient: Sized {
    fn install() -> impl std::future::Future<Output = Result<Self, ConductorApiError>> + Send;
    fn install_app(app_name:&str, happ_url:Option<&str>) -> impl std::future::Future<Output = Result<Self, ConductorApiError>> + Send;
    async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str, request:DanceRequest) -> Result<DanceResponse, HolonError>;
    //async fn wait_on_signal(&self, cell_id:CellId)-> Result<(), ConductorApiError>;
}

#[derive(Debug)]
pub struct AppInstallation {
    pub conductor: SweetConductor,
    pub app: SweetApp,
    pub cells: Vec<SweetCell>,
    //pub signer: ClientAgentSigner,
}

const DNA_FILEPATH: &str = "../../workdir/map_holons.dna";
const APP_ID: &str = "map_holons";
const HAPP_FILEPATH: &str = "../../workdir/map_holons.happ";



impl ZomeClient for AppInstallation {
    async fn install() -> Result<AppInstallation, ConductorApiError> {
        Self::install_app(APP_ID, Some(HAPP_FILEPATH)).await
    }
    async fn install_app(app_id:&str, happ_url:Option<&str>) -> Result<AppInstallation, ConductorApiError> {

        let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH)).await.unwrap();
        let mut conductor = SweetConductor::from_standard_config().await;
        let holo_core_agent = SweetAgents::one(conductor.keystore()).await;
        let app = conductor
            .setup_app_for_agent(app_id, holo_core_agent.clone(), &[dna.clone()])
            .await
            .unwrap();

        let cells = &app.cells().clone();//[0].clone();

        //let agent_hash = holo_core_agent.into_inner();
        //let agent = AgentPubKey::from_raw_39(agent_hash).unwrap();

        Ok(Self{conductor, app, cells:cells.to_vec()})    
    }

    async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str, request:DanceRequest) -> Result<DanceResponse, HolonError> {

        let zome = self.conductor.get_sweet_cell(cell_id).map_err(|err: ConductorApiError|HolonError::WasmError(err.to_string()))?.zome(zome_name);
        println!("{:?}", zome);
        let response: DanceResponse = self.conductor.call::<DanceRequest,DanceResponse>(&zome, fn_name, request).await;
        match response.status_code {
            ResponseStatusCode::OK => return Ok(response),
            ResponseStatusCode::Accepted => return Ok(response),
            _ => return Err(HolonError::WasmError(response.status_code.to_string())),
        };
    }

   /* async fn wait_on_signal(&self, cell_id:CellId) -> Result<(), ConductorApiError> {

        // Connect admin client
        let admin_port = self.conductor.get_arbitrary_admin_websocket_port().unwrap();
        let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
            //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        let credentials = admin_ws
            .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
                cell_id: cell_id.clone(),
                functions: None,
            })
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
            //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;
        self.signer.add_credentials(cell_id.clone(), credentials);


        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();
        // Connect app agent client
        let app_ws_port = admin_ws
          .attach_app_interface(0, AllowedOrigins::Any, None)
          .await?;

        let token_issued = admin_ws
          .issue_app_auth_token(self.app_id.clone().into())
          .await?;
        let app_ws = AppWebsocket::connect(
            (Ipv4Addr::LOCALHOST, app_ws_port),
            token_issued.token,
            self.signer.clone().into(),
        )
        .await
        .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
        //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        app_ws
            .on_signal(move |signal| match signal {
                Signal::App { signal, .. } => {
                    let ts: TestString = signal.into_inner().decode().unwrap();
                    assert_eq!(ts.0.as_str(), "i am a signal");
                    barrier_clone.wait();
                }
                _ => panic!("Invalid signal"),
            })
            .await
            .unwrap(); 
        barrier.wait();
            Ok(())
    }*/
}

/// MOCK CONDUCTOR

pub async fn setup_conductor() -> (SweetConductor, AgentPubKey, SweetCell) {
    let dna = SweetDnaFile::from_bundle(std::path::Path::new(&DNA_FILEPATH)).await.unwrap();

    // let dna_path = std::env::current_dir().unwrap().join(DNA_FILEPATH);
    // println!("{}", dna_path.to_string_lossy());
    // let dna = SweetDnaFile::from_bundle(&dna_path).await.unwrap();

    let mut conductor = SweetConductor::from_standard_config().await;

    let holo_core_agent = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", holo_core_agent.clone(), &[dna.clone()])
        .await
        .unwrap();

    let cell = app.into_cells()[0].clone();

    let agent_hash = holo_core_agent.into_inner();
    let agent = AgentPubKey::from_raw_39(agent_hash).unwrap();

    (conductor, agent, cell)
}

