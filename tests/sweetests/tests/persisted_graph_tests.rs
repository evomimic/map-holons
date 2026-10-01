//! Conductor-free checks of persisted edge occurrences and exact lineage assertions.

// These executors and read helpers also compile into conductor targets, which use their full surface.
#[allow(dead_code)]
mod execution_steps {
    pub mod persisted_graph_executor;
    pub mod persisted_read_support;
}
