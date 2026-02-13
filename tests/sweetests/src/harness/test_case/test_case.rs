//! This file provides:
//!
//! - [`TestCaseInit`], which initializes the test_case and context, with empty mutable
//!    fixture holons and bindings objects.
//! - [`DancesTestCase`], a container representing a single declarative test
//!   program composed of an ordered sequence of steps.
//! - Builder-style `add_*` methods for constructing test cases in a clear,
//!   sequential, and intention-revealing manner.
//! - [`TestSessionState`], which captures transient holon state produced during
//!   fixture setup and injects it into the test execution context.

use crate::{init_fixture_context, DanceTestStep, FixtureBindings, FixtureHolons};
use holons_boundary::SerializableHolonPool;
use holons_core::core_shared_objects::transactions::TransactionContext;
use std::sync::Arc;

/// Public test case type that collects steps to be executed later.
#[derive(Default, Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: Vec<DanceTestStep>,
    pub test_session_state: TestSessionState,
    pub is_finalized: bool,
}

/// TestCaseInit provides a structured, atomic initialization context for constructing a TestCase together with all required harness-managed fixture-time state.
/// It answers the question: “What must exist, together, in order to author a valid TestCase?”
/// Responsibilities:
/// - Ensure all required harness components are created together
/// - Make initialization explicit and difficult to misuse
/// - Avoid fragile tuple-based or ad-hoc setup
/// - Establish clear ownership boundaries from the outset
/// - Keep the DancesTestCase itself as the primary author-facing artifact
pub struct TestCaseInit {
    pub test_case: DancesTestCase,
    pub fixture_context: Arc<TransactionContext>,
    pub fixture_holons: FixtureHolons,
    pub fixture_bindings: FixtureBindings,
}

impl TestCaseInit {
    pub fn new(name: String, description: String) -> Self {
        let context = init_fixture_context();
        let mut test_case = DancesTestCase::default();
        test_case.name = name;
        test_case.description = description;

        Self {
            test_case,
            fixture_context: context,
            fixture_holons: FixtureHolons::default(),
            fixture_bindings: FixtureBindings::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestSessionState {
    transient_holons: SerializableHolonPool,
}

impl TestSessionState {
    pub fn set_transient_holons(&mut self, transient_holons: SerializableHolonPool) {
        self.transient_holons = transient_holons;
    }

    pub fn get_transient_holons(&self) -> &SerializableHolonPool {
        &self.transient_holons
    }
}
