use crate::reference_layer::HolonReference;

pub trait Descriptor {
    fn holon(&self) -> &HolonReference;
}
