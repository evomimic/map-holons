
use hdi::prelude::AgentPubKey;
use holochain::conductor::api::error::ConductorApiError;
use holochain::conductor::api::CellInfo;
use holochain::prelude::{AppBundleSource, CellId, InstalledAppId, Signal}; 
use holochain::sweettest::*;

//use holochain_client::{
 //   AdminWebsocket, AppAuthenticationTokenIssued, AppWebsocket, AuthorizeSigningCredentialsPayload, 
 //   ClientAgentSigner, InstallAppPayload, ConductorApiError
//};
//use holochain_conductor_api::{AppInfoStatus, CellInfo, NetworkInfo};
//use holochain_types::{
  ///  app::{AppBundle, AppManifestV1, DisabledAppReason},
   // websocket::AllowedOrigins,
//};
//use holochain_zome_types::dependencies::holochain_integrity_types::ExternIO;
//use kitsune_p2p_types::{dependencies::proptest::arbitrary, fetch_pool::FetchPoolInfo};
use serde::{Deserialize, Serialize};
use std::{io::Error, io::ErrorKind, net::Ipv4Addr};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Barrier},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestString(pub String);

pub trait ZomeClient: Sized {
 async fn install(app_name:&str, happ_url:&str) -> Result<Self, ConductorApiError>;
 async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str) -> Result<(), ConductorApiError>;
 async fn wait_on_signal(&self, cell_id:CellId)-> Result<(), ConductorApiError>;
}

#[derive(Debug)]
pub struct AppInstallation {
    pub conductor: SweetConductor,
    pub app_id: InstalledAppId,
    pub cells: Vec<CellInfo>,
    //pub signer: ClientAgentSigner,
}

const DNA_FILEPATH: &str = "../../workdir/map_holons.dna";


#[tokio::test(flavor = "multi_thread")]
async fn mytest() {
    let app: AppInstallation = AppInstallation::install("map_holons","../../workdir/map_holons.happ").await.unwrap();
    let cell_id = match app.cells[0].clone() {
            CellInfo::Provisioned(c) => c.cell_id,
            _ => panic!("Invalid cell type"),
        };
    app.zomecall(cell_id,"foo","bar").await.unwrap();
    //app.wait_on_signal(cell_id).await.unwrap();
}

impl ZomeClient for AppInstallation {
    async fn install(app_name:&str, happ_url:&str) -> Result<Self, ConductorApiError> {
        let conductor = Conductor::from_standard_config().await;

        // Connect admin client
        let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
        let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::Io(Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;

        // Set up the test app
        let app_id: InstalledAppId = app_name.into();  //"test-app"
        let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();
        admin_ws
            .install_app(InstallAppPayload {
                agent_key: agent_key,
                installed_app_id: Some(app_id.clone()),
                network_seed: None,
                membrane_proofs: HashMap::new(),
                //hc-0.5 roles_settings: None, 
                source: AppBundleSource::Path(PathBuf::from(happ_url)), //"./fixture/test.happ"
                //hc-0.5 ignore_genesis_failure: false,
               //hc-0.5  allow_throwaway_random_agent_key: false,
            })
            .await?;
        admin_ws.enable_app(app_id.clone()).await?;

        // Connect app agent client
        let app_ws_port = admin_ws
            .attach_app_interface(0, AllowedOrigins::Any, None)
            .await?;
        let token_issued = admin_ws
            .issue_app_auth_token(app_id.clone().into())
            .await?;
        //let signer = ClientAgentSigner::default();
        let app_ws = AppWebsocket::connect(
            (Ipv4Addr::LOCALHOST, app_ws_port),
            token_issued.token,
            //signer.clone().into(),
        )
        .await
        .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
       // 0.5 version: .await.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        if let Some(app_info) = app_ws.app_info().await? {
            if let Some(cells) = app_info.cell_info.into_values().next() {
                return Ok(Self{conductor, app_id, cells, signer});
            }
        } 
        return Err(ConductorApiError::CellNotFound);
    }

    async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str) -> Result<(), ConductorApiError> {
        // ******** SIGNED ZOME CALL  ********

        //const TEST_ZOME_NAME: &str = "foo";
        //const TEST_FN_NAME: &str = "bar";


        // Connect admin client
        let admin_port = self.conductor.get_arbitrary_admin_websocket_port().unwrap();
        let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;

            // 0.5 .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        let credentials = admin_ws
            .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
                cell_id: cell_id.clone(),
                functions: None,
            })
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
            //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;
        self.signer.add_credentials(cell_id.clone(), credentials);

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

        let _response = app_ws
            .call_zome(
                cell_id.clone().into(),
                zome_name.into(),
                fn_name.into(),
                ExternIO::encode(())
                .map_err(|e| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (e.to_string()))))?,
                //.map_err(|e| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(e.to_string())))?,
            )
            .await?;

           // assert_eq!(
           //     ExternIO::decode::<String>(&response).unwrap(),
           //     fn_name.to_string()
           // );
        Ok(())
    }

    async fn wait_on_signal(&self, cell_id:CellId) -> Result<(), ConductorApiError> {

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
    }
}
