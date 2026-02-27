//! # Dance Test Language
//!
//! This module defines the **declarative language** used by MAP sweetests to
//! describe integration test behavior in terms of *dance execution*.
//!
//! It does **not** execute tests and does **not** define any concrete test
//! scenarios. Instead, it defines the **grammar, structure, and construction
//! API** used by test fixtures to *author* test cases that are executed later
//! by the sweetests harness.
//!
//! Test cases constructed using this language are *pure specifications*:
//! they contain no execution-time context, no concrete runtime identifiers, and no
//! execution logic. Resolution of references, state mutation, and dance
//! invocation are handled entirely by the execution support layer at runtime.
//!
//! ## Architectural Role
//!
//! Within the sweetests harness, this module occupies a middle layer between:
//!
//! - **fixtures_support**, which mints symbolic [`TestReference`] tokens and
//!   assembles test cases using this language, and
//! - **execution_support**, which interprets and executes the resulting test
//!   cases against client- and guest-side contexts.
//!
//! This separation allows test behavior to be described declaratively while
//! remaining independent of runtime identifiers and execution-time handles.

mod adders;
pub mod test_case;
pub mod test_steps;

pub use adders::*;
pub use test_case::*;
pub use test_steps::*;
