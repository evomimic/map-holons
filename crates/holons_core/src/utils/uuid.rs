use shared_types_holon::TemporaryId;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn generate_temporary_id() -> TemporaryId {
    TemporaryId(Uuid::new_v4())
}
