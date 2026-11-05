//! Opt-in guest timing shared by the loader controller and resolver.
//! The Commit profiling build forwards `performance-profiling`; ordinary loads
//! retain semantic counters without issuing diagnostic clock host calls.
//! Unit tests exercise deterministic counters without invoking Holochain HDK.

pub(crate) fn performance_timestamp_micros() -> Option<i64> {
    #[cfg(all(not(test), feature = "performance-profiling"))]
    {
        hdk::prelude::sys_time().ok().map(|timestamp| timestamp.as_micros())
    }
    #[cfg(any(test, not(feature = "performance-profiling")))]
    {
        None
    }
}

pub(crate) fn elapsed_micros(started_at: Option<i64>) -> i64 {
    started_at
        .zip(performance_timestamp_micros())
        .map(|(started, completed)| completed.saturating_sub(started))
        .unwrap_or_default()
}
