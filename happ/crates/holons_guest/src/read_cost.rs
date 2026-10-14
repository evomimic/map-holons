//! Exclusive read costs for controlled Commit profiling. Inactive outside a profiled assessment.
use hdk::prelude::sys_time;
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub(crate) enum Component {
    OtherConstruction,
    MembershipService,
    LinkPrefix,
    LinkCountProbe,
    LinkSingleScan,
    LinkBatchedPrefix,
    LinkDecode,
    ReferenceBinding,
    NodeBackend,
    NodeDecode,
}
impl Component {
    pub(crate) const ALL: [Self; 10] = [
        Self::OtherConstruction,
        Self::MembershipService,
        Self::LinkPrefix,
        Self::LinkCountProbe,
        Self::LinkSingleScan,
        Self::LinkBatchedPrefix,
        Self::LinkDecode,
        Self::ReferenceBinding,
        Self::NodeBackend,
        Self::NodeDecode,
    ];
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::OtherConstruction => "OtherConstruction",
            Self::MembershipService => "MembershipService",
            Self::LinkPrefix => "LinkPrefix",
            Self::LinkCountProbe => "LinkCountProbe",
            Self::LinkSingleScan => "LinkSingleScan",
            Self::LinkBatchedPrefix => "LinkBatchedPrefix",
            Self::LinkDecode => "LinkDecode",
            Self::ReferenceBinding => "ReferenceBinding",
            Self::NodeBackend => "NodeBackend",
            Self::NodeDecode => "NodeDecode",
        }
    }
}
#[derive(Default)]
struct Costs {
    stack: Vec<Component>,
    last: Option<i64>,
    micros: [i64; 10],
}
impl Costs {
    fn accrue(&mut self, now: Option<i64>) {
        if let (Some(component), Some(last), Some(now)) = (self.stack.last(), self.last, now) {
            self.micros[*component as usize] += now.saturating_sub(last).max(0);
        }
        self.last = now;
    }
}
thread_local! { static COSTS: RefCell<Costs> = RefCell::new(Costs::default()); }
fn now() -> Option<i64> {
    sys_time().ok().map(|timestamp| timestamp.as_micros())
}
pub(crate) struct Scope(bool);
impl Scope {
    pub(crate) fn assessment(enabled: bool) -> Self {
        if !enabled {
            return Self(false);
        }
        COSTS.with(|costs| *costs.borrow_mut() = Costs::default());
        let started = now();
        COSTS.with(|costs| {
            let mut costs = costs.borrow_mut();
            costs.stack.push(Component::OtherConstruction);
            costs.last = started;
        });
        Self(true)
    }
    pub(crate) fn enter(component: Component) -> Self {
        if !COSTS.with(|costs| !costs.borrow().stack.is_empty()) {
            return Self(false);
        }
        let entered = now();
        COSTS.with(|costs| {
            let mut costs = costs.borrow_mut();
            costs.accrue(entered);
            costs.stack.push(component);
        });
        Self(true)
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if self.0 {
            let completed = now();
            COSTS.with(|costs| {
                let mut costs = costs.borrow_mut();
                costs.accrue(completed);
                costs.stack.pop();
            });
        }
    }
}
pub(crate) fn snapshot() -> [i64; 10] {
    let observed = now();
    COSTS.with(|costs| {
        let mut costs = costs.borrow_mut();
        costs.accrue(observed);
        costs.micros
    })
}
