


use holochain_client::{AdminWebsocket, AgentPubKey, AppWebsocket, AuthorizeSigningCredentialsPayload, ClientAgentSigner, ConductorApiError, InstallAppPayload, InstalledAppId};
use holochain_conductor_api::CellInfo;
use holochain_types::prelude::{CellId, ExternIO};
use holochain_types::websocket::AllowedOrigins;
use serde::{Deserialize, Serialize};
use std::{io::Error, io::ErrorKind, net::Ipv4Addr};
use std::{
    collections::HashMap
};

pub trait ZomeClient: Sized {
 async fn init(app_id:String, admin_port:u16) -> Result<Self, ConductorApiError>;
 async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str, payload:ExternIO) -> Result<ExternIO, ConductorApiError>;
 fn get_cell_id_by_role(&self, role: Option<&str>) -> Result<CellId,ConductorApiError>;
 //async fn wait_on_signal(&self, cell_id:CellId)-> Result<(), ConductorApiError>;
}

#[derive(Debug)]
pub struct AppSessionData {
    pub admin_port: u16,
    pub app_id: InstalledAppId,
    pub cells: HashMap<String,Vec<CellInfo>>,
    //pub signer: ClientAgentSigner,
}

const PORT: u16 = 9999; //this will be passed in from the launcher / sandbox - better the os chooses a port than hardcoding
const APP_ID: &str = "map_holons";
const DNA_FILEPATH: &str = "../fixture/test.dna";
const HAPP_PATH: &str = "workdir/hello-world.happ";


impl ZomeClient for AppSessionData {
    async fn init(app_id:String, admin_port:u16) -> Result<Self, ConductorApiError> {
        // Connect admin web socket
        let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(Error::new(ErrorKind::ConnectionRefused, arg0.to_string())))?;

        // Assume App is installed and enabled
        let appdata = admin_ws.enable_app(app_id.clone()).await?;
        let cell_data = appdata.app.cell_info;//.into_values().next().unwrap();
        return Ok(Self{admin_port, app_id, cells: cell_data});
    }

    async fn zomecall(self, cell_id:CellId, zome_name:&str, fn_name:&str, payload:ExternIO) -> Result<ExternIO, ConductorApiError> {
        // ******** SIGNED ZOME CALL  ********

        // Connect admin web socket
        let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, self.admin_port))
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(Error::new(ErrorKind::ConnectionRefused, arg0.to_string())))?;

//            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;

            // 0.5 .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;
//admin_ws.attach_app_interface(port, allowed_origins, installed_app_id)
        let credentials = admin_ws
            .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
                cell_id: cell_id.clone(),
                functions: None,
            })
            .await
            .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(Error::new(ErrorKind::ConnectionRefused, arg0.to_string())))?;
        let signer = ClientAgentSigner::default();
        signer.add_credentials(cell_id.clone(), credentials);

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
            signer.clone().into(),
        )
        .await
        .map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(Error::new(ErrorKind::ConnectionRefused, (arg0.to_string()))))?;
        //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(arg0.to_string())))?;

        let response = app_ws
            .call_zome(
                cell_id.clone().into(),
                zome_name.into(),
                fn_name.into(),
                payload
                //.map_err(|arg0: anyhow::Error| ConductorApiError::WebsocketError(Error::new(ErrorKind::ConnectionRefused, arg0.to_string())))?;
                //.map_err(|e| ConductorApiError::WebsocketError(Error::new(ErrorKind::ConnectionRefused, (e.to_string()))))?,
                //.map_err(|e| ConductorApiError::WebsocketError(holochain_websocket::WebsocketError::Other(e.to_string())))?,
            )
            .await?;
            println!("response: {:?}", ExternIO::decode::<String>(&response));
            
           // assert_eq!(
           //     ExternIO::decode::<String>(&response).unwrap(),
           //     fn_name.to_string()
           // );
        Ok(response)
    }

    // Passing in None will return the first cell_id found, otherwise an error
    fn get_cell_id_by_role(&self, role: Option<&str>) -> Result<CellId,ConductorApiError> {
        if let Some(role) = role {
            if let Some(cell_data) = self.cells.get(role) {
                match cell_data[0].clone() {
                    CellInfo::Provisioned(c) => Ok(c.cell_id),
                    CellInfo::Cloned(c) => Ok(c.cell_id),
                    _ => Err(ConductorApiError::CellNotFound)
                }
            } else {
                Err(ConductorApiError::CellNotFound)
            }
        } else {
            if let Some(cell_data) = self.cells.values().next().clone() {
                match cell_data[0].clone() {
                    CellInfo::Provisioned(c) => Ok(c.cell_id),
                    CellInfo::Cloned(c) => Ok(c.cell_id),
                    _ => Err(ConductorApiError::CellNotFound)
                }
            } else {
                Err(ConductorApiError::CellNotFound)
            }
        }
        
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestString(pub String);

#[tokio::test(flavor = "multi_thread")]
async fn mytest() {
    let app: AppSessionData = AppSessionData::init(APP_ID.to_string(),PORT).await.unwrap();
    let cell_id = app.get_cell_id_by_role(None).unwrap();
    let payload = ExternIO::encode(TestString("commit".to_string())).unwrap();
    app.zomecall(cell_id,"dances","dance",payload).await.unwrap();
}
    /*async fn wait_on_signal(&self, cell_id:CellId) -> Result<(), ConductorApiError> {

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


