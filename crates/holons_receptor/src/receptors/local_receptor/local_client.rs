use core_types::{HolonError};
use holons_client::shared_types::holon_space::{HolonSpace, SpaceInfo};
use holons_core::{HolonsContextBehavior, core_shared_objects::{SavedHolon}};

//Local client does not make conductor calls so doesnt need the DanceInitiator

#[derive(Debug, Clone)]
pub struct LocalClient;

impl LocalClient {
    pub fn new() -> Self {
        Self {}
    }
    pub fn fetch_or_create_root_holon(&self, _context: &dyn HolonsContextBehavior) -> Result<SavedHolon, HolonError> {
        // Implement logic to check and create root holon if it doesn't exist
        todo!("Implement fetch_or_create_root_holon to get or create root holon if it doesn't exist");
        //Ok(mock_root_holon)
    }
    pub async fn get_all_spaces(&self) -> Result<SpaceInfo, HolonError> {
        Ok(SpaceInfo::default())
    }

    pub fn convert_to_holonspace(&self, _holon: SavedHolon) -> Result<HolonSpace, HolonError> {
        // Implement conversion logic from SavedHolon to SpaceInfo
        todo!("Implement convert_to_space_info to convert SavedHolon to SpaceInfo");
    }
}


/*#[async_trait]
impl DanceInitiator for LocalClient {
    async fn initiate_dance(
        &self,
        _context: &dyn HolonsContextBehavior,
        _request: DanceRequest,
    ) -> DanceResponse {
        // Implement your local dance logic here
        DanceResponse {
            status_code: ResponseStatusCode::OK,
            description: MapString("Local dance completed".into()),
            body: ResponseBody::None,
            descriptor: None,
            state: None,
        }
    }
}*/