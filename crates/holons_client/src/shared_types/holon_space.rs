use holons_core::core_shared_objects::Holon;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum TypeDescriptor {
    String(String),
    Holon(Holon),
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct HolonSpace {
    pub id: String, //holon_id
    pub receptor_id: String,
    pub branch_id: Option<String>, // in holochain this is the cell_id / target role / clone id
    pub name: String,
    pub space_type: String,
    pub description: String,
    pub origin_space_id: String,
    pub descriptor_id: Option<String>, // pub typedescriptor: TypeDescriptor //schema
    /// Optional metadata as raw bytes, serialized efficiently
    pub metadata: Option<ByteBuf>,
    pub enabled: bool,
    //pub children: Option<Vec<HolonSpace>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RootSpace {
    pub name: String,
    pub typedescriptor: TypeDescriptor, //schema
    pub description: String,
    pub children: Option<Vec<HolonSpace>>,
}

//todo rename to spaces
#[derive(Debug, Deserialize, Serialize)]
pub struct SpaceInfo {
    spaces: HashMap<String, HolonSpace>, // Placeholder for actual space data
}

impl SpaceInfo {
    pub fn new() -> Self {
        SpaceInfo { spaces: HashMap::new() }
    }

    pub fn add_space(&mut self, key: String, value: HolonSpace) {
        self.spaces.insert(key, value);
    }

    pub fn get_spaces(&self) -> &HashMap<String, HolonSpace> {
        &self.spaces
    }

    pub fn default() -> Self {
        let mut space_info = Self::new();
        space_info.add_space("default_space".to_string(), HolonSpace::default());
        space_info
    }
}
