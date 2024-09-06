
use crate::holon::Holon;


pub struct HolonSpace(pub Holon);

impl HolonSpace {
    pub fn new(holon: Holon) -> HolonSpace {
        HolonSpace(holon)
    }

    pub fn into_holon(self) -> Holon {
        self.0
    }
}
