use holons::holon::Holon;

#[derive(Clone, Debug)]
pub enum HolonTestCase {
    Creates(HolonCreatesTestCase),
    Updates(HolonUpdatesTestCase),
}
#[derive(Clone, Debug)]
pub struct HolonUpdatesTestCase { // this is equivalent to current HolonDescriptorTestCase
pub original: Holon,
    pub updates: Vec<Holon>,
    // pub message_level: Level,
}
#[derive(Clone, Debug)]
pub struct HolonCreatesTestCase {
    pub creates: Vec<Holon>,
    // pub message_level: Level,
}