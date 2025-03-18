use shared_types_holon::TemporaryId;
use uuid::Uuid;

pub fn generate_temporary_id() -> TemporaryId {
    TemporaryId(Uuid::new_v4())
}
