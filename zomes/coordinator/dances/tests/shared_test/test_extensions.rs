use holons_core::core_shared_objects::HolonPool;
use holons_core::HolonsContextBehavior;
use std::cell::RefCell;
use std::rc::Rc;

pub trait TestContextExtensions {
    fn export_test_store(&self) -> HolonPool;
}

impl TestContextExtensions for dyn HolonsContextBehavior {
    fn export_test_store(&self) -> HolonPool {
        let nursery_access = self.get_space_manager().get_nursery_access();
        let internal_access = nursery_access.borrow().as_internal();
        internal_access.export_store()
    }
}
