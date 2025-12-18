use core_types::HolonError;
use holons_receptor::factory::ReceptorFactory;
use holons_client::shared_types::map_request::{MapRequest, MapRequestBody};
use holons_client::shared_types::map_response::MapResponse;
use tauri::{command, State};

#[command]
pub(crate) async fn map_request(
    map_request: MapRequest,
    receptor_factory: State<'_, ReceptorFactory>,
) -> Result<MapResponse, HolonError> {

    tracing::debug!("[TAURI COMMAND] 'map_request' command invoked for space: {:?}", map_request);
    // a map_request is currently using "holochain" receptor type only
    let receptor = receptor_factory.get_receptor_by_type("holochain"); 
        if map_request.name == "load_holons" {
            tracing::info!("[TAURI COMMAND] 'map_request' handling 'load_holons' request");
                return receptor
                .load_holons(map_request)
                .await
                .map_err(HolonError::from);
        } 
        return receptor
            .handle_map_request(map_request)
            .await
            .map_err(HolonError::from);
}




// WORK IN PROGRESS: Refactor to move logic out of command function for easier testing
/* 
pub(crate) async fn map_request_impl(
    map_request: MapRequest,
    receptor_factory: &ReceptorFactory,
) -> Result<MapResponse, HolonError> {
    tracing::debug!(
        "[TAURI COMMAND] 'map_request' impl invoked for space: {:?}",
        map_request
    );

    let receptor = receptor_factory.get_receptor_by_type("holochain");
    receptor
        .handle_map_request(map_request)
        .await
        .map_err(HolonError::from)
}

//try this with a mock (sweet) conductor.. (no plugin required)
#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_map_request_impl() {
        // Build a ReceptorFactory in whatever “test” shape you want:
        let factory = ReceptorFactory::new();
        // maybe load some fake configs or inject a test receptor

        let req = MapRequest {
            // ...
        };

        let res = map_request_impl(req, &factory).await;

        // assert whatever you expect
        assert!(res.is_ok());
    }
}
    //this didnt work .. abandoned for now
#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppBuilder;
    use tauri::{Manager, test::{mock_context, noop_assets}};

    #[tokio::test]
    async fn test_maprequest_command() {
        // 1. Build app using your real AppBuilder
        let builder = AppBuilder::build();

        // 2. Use mock_context + noop_assets instead of real config/assets
        let app = builder
            .build(mock_context(noop_assets()))
            .expect("failed to build mock app");

        let map_request = MapRequest::test_for_stage_new_holon();
        let state: tauri::State<'_, ReceptorFactory> = app.state();

        
        // 3. Call your tauri::command directly
        let result = crate::commands::map_request(map_request, state).await;

        // 4. Assert whatever you expect
        assert!(result.is_ok());
    }
}
*/
// The above test is a work in progress to refactor the command function for easier testing.