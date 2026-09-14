//! Best-effort guest timing shared by the loader controller and resolver.
//! Unit tests exercise deterministic counters without invoking Holochain HDK.

pub(crate) fn performance_timestamp_micros() -> Option<i64> {
    #[cfg(not(test))]
    {
        hdk::prelude::sys_time().ok().map(|timestamp| timestamp.as_micros())
    }
    #[cfg(test)]
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
