//! Diagnostic clocks are opt-in: each timestamp crosses the WASM/host boundary.
//! WASM_LOG filters at the host, after guest formatting/transport; it is not a gate.

pub(crate) fn timestamp_micros() -> Option<i64> {
    timestamp_with_clock(|| hdk::prelude::sys_time().ok().map(|timestamp| timestamp.as_micros()))
}

fn timestamp_with_clock(clock: impl FnOnce() -> Option<i64>) -> Option<i64> {
    #[cfg(any(
        feature = "commit-performance-profiling",
        feature = "descriptor-performance-profiling"
    ))]
    {
        clock()
    }
    #[cfg(not(any(
        feature = "commit-performance-profiling",
        feature = "descriptor-performance-profiling"
    )))]
    {
        let _ = clock;
        None
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(any(
        feature = "commit-performance-profiling",
        feature = "descriptor-performance-profiling"
    )))]
    #[test]
    fn ordinary_build_never_requests_a_host_clock() {
        assert_eq!(super::timestamp_with_clock(|| panic!("unexpected host clock request")), None);
    }

    #[cfg(any(
        feature = "commit-performance-profiling",
        feature = "descriptor-performance-profiling"
    ))]
    #[test]
    fn profiling_build_requests_the_clock() {
        assert_eq!(super::timestamp_with_clock(|| Some(42)), Some(42));
    }
}
