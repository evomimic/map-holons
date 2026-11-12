//! Opt-in request diagnostics. Counts host-call attempts, including calls that fail.
#[cfg(feature = "commit-audit")]
mod enabled {
    use hdk::prelude::*;
    use holons_validation::{AssessmentEvent, ValidationObservations};
    use std::cell::RefCell;

    #[derive(Debug, Default, Serialize)]
    pub(super) struct Audit {
        kind: &'static str,
        node_creates: u64,
        node_updates: u64,
        smartlink_creates: u64,
        prepared_parameters: usize,
        pub(super) violations: Option<usize>,
        reached_constraints: usize,
        evaluated_constraints: usize,
        property_conformance_dispatched: bool,
    }
    thread_local! { pub(super) static AUDIT: RefCell<Option<Audit>> = const { RefCell::new(None) }; }

    pub(super) fn observe(event: &AssessmentEvent) {
        AUDIT.with_borrow_mut(|audit| {
            let Some(audit) = audit else { return };
            match event {
                AssessmentEvent::Preparation(metrics) => {
                    audit.prepared_parameters = metrics.prepared_constraint_parameters
                }
                AssessmentEvent::Coverage(ValidationObservations {
                    effective_constraint_count,
                    evaluated_value_constraint_count,
                    dispatched_rule_keys,
                    ..
                }) => {
                    audit.reached_constraints = *effective_constraint_count;
                    audit.evaluated_constraints = *evaluated_value_constraint_count;
                    audit.property_conformance_dispatched = dispatched_rule_keys.contains(
                        type_names::CoreValidationRuleName::PropertyValueConformance.as_str(),
                    );
                }
                _ => {}
            }
        });
    }
    pub(super) enum WriteKind {
        NodeCreate,
        NodeUpdate,
        SmartLinkCreate,
    }
    pub(super) fn count(kind: WriteKind) {
        AUDIT.with_borrow_mut(|audit| {
            if let Some(audit) = audit {
                match kind {
                    WriteKind::NodeCreate => audit.node_creates += 1,
                    WriteKind::NodeUpdate => audit.node_updates += 1,
                    WriteKind::SmartLinkCreate => audit.smartlink_creates += 1,
                }
            }
        });
    }
    pub(super) fn start() {
        AUDIT.with_borrow_mut(|audit| {
            *audit = Some(Audit { kind: "CommitAudit", ..Audit::default() })
        });
    }
    pub(super) fn finish() {
        if let Some(audit) = AUDIT.with_borrow_mut(Option::take) {
            // Diagnostics never change Commit acceptance or persistence behavior.
            if let Err(error) = emit_signal(audit) {
                warn!("Commit audit signal failed: {error:?}");
            }
        }
    }
}

/// Emits one diagnostic at every Commit exit without retaining state between calls.
pub(crate) struct Scope;
impl Scope {
    pub(crate) fn start() -> Self {
        #[cfg(feature = "commit-audit")]
        enabled::start();
        Self
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        #[cfg(feature = "commit-audit")]
        enabled::finish();
    }
}

#[inline]
pub(crate) fn node_create() {
    #[cfg(feature = "commit-audit")]
    enabled::count(enabled::WriteKind::NodeCreate);
}
#[inline]
pub(crate) fn node_update() {
    #[cfg(feature = "commit-audit")]
    enabled::count(enabled::WriteKind::NodeUpdate);
}
#[inline]
pub(crate) fn smartlink_create() {
    #[cfg(feature = "commit-audit")]
    enabled::count(enabled::WriteKind::SmartLinkCreate);
}
#[inline]
pub(crate) fn observe(_event: &holons_validation::AssessmentEvent) {
    #[cfg(feature = "commit-audit")]
    enabled::observe(_event);
}

pub(crate) fn violations(_count: usize) {
    #[cfg(feature = "commit-audit")]
    enabled::AUDIT.with_borrow_mut(|audit| {
        if let Some(audit) = audit {
            audit.violations = Some(_count);
        }
    });
}
