//! Optional phase observation without host clocks or transport-specific instrumentation.

/// Read-only observation of the current Commit assessment phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssessmentPhase {
    Construction,
    SubjectPreparation,
    SchemaWorksetPreparation,
    PackageDiscovery,
    PreparedProducts,
    PackageValidation,
    InstanceValidation,
}

/// Aggregate preparation demand. Counts describe reference requests, not backend cache misses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparationMetrics {
    pub package_roots: usize,
    /// Baseline alias for distinct_content: every reached definition requests a complete schema.
    pub requested_schema_products: usize,
    pub constructed_schema_products: usize,
    pub subjects: usize,
    pub affected_schemas: usize,
    pub distinct_content: usize,
    pub identity_role_visits: usize,
    pub contract_consumers: usize,
    pub constraint_consumers: usize,
    pub binding_consumers: usize,
}

/// Clock-free observation; profiling cannot supply inputs or change validation policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssessmentEvent {
    Phase(AssessmentPhase),
    Preparation(PreparationMetrics),
    PreparationVisitStarted,
    PreparationVisitFinished { type_name: String, distinct_content: bool },
    PreparationOperation { operation: &'static str, role: &'static str, starting: bool },
    PreparationEdge { role: &'static str, edge: String, requests: usize, members: usize },
}

/// Observe an operation on both success and operational failure. Operation spans are siblings.
pub(crate) fn observe_operation<T>(
    observer: &mut impl FnMut(AssessmentEvent),
    operation: &'static str,
    role: &'static str,
    action: impl FnOnce() -> T,
) -> T {
    let observation = observe_operation_scope(observer, operation, role);
    let result = action();
    drop(observation);
    result
}

/// Owns a sibling operation span until completion, including early error returns.
pub(crate) struct OperationObservation<'a> {
    observer: &'a mut dyn FnMut(AssessmentEvent),
    operation: &'static str,
    role: &'static str,
}

/// Opens a span for a block whose local values must remain available after observation.
pub(crate) fn observe_operation_scope<'a>(
    observer: &'a mut impl FnMut(AssessmentEvent),
    operation: &'static str,
    role: &'static str,
) -> OperationObservation<'a> {
    observer(AssessmentEvent::PreparationOperation { operation, role, starting: true });
    OperationObservation { observer, operation, role }
}

impl Drop for OperationObservation<'_> {
    fn drop(&mut self) {
        (self.observer)(AssessmentEvent::PreparationOperation {
            operation: self.operation,
            role: self.role,
            starting: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_and_block_observations_close_on_operational_errors_with_their_role() {
        let mut events = Vec::new();
        let mut observer = |event| events.push(event);
        let result: Result<(), ()> =
            observe_operation(&mut observer, "Closure", "PreparedDefinition", || Err(()));
        assert_eq!(result, Err(()));
        let result = (|| -> Result<(), ()> {
            let _observation = observe_operation_scope(&mut observer, "Block", "Assessment");
            Err(())?;
            Ok(())
        })();
        assert_eq!(result, Err(()));
        assert_eq!(
            events,
            vec![
                AssessmentEvent::PreparationOperation {
                    operation: "Closure",
                    role: "PreparedDefinition",
                    starting: true
                },
                AssessmentEvent::PreparationOperation {
                    operation: "Closure",
                    role: "PreparedDefinition",
                    starting: false
                },
                AssessmentEvent::PreparationOperation {
                    operation: "Block",
                    role: "Assessment",
                    starting: true
                },
                AssessmentEvent::PreparationOperation {
                    operation: "Block",
                    role: "Assessment",
                    starting: false
                },
            ]
        );
    }
}
