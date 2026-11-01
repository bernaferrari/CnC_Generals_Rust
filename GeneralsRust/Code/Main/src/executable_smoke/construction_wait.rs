// Readiness for the windowed smoke's physical construction and training chain.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstructionWait {
    Pending,
    Ready,
    TimedOut,
}

/// Keep the normal smoke bounded, while allowing longer diagnostic runs on
/// slow builds. Invalid or unreasonable input retains the normal timeout.
pub(crate) fn construction_timeout(value: Option<&str>) -> std::time::Duration {
    let seconds = value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (9..=900).contains(seconds))
        .unwrap_or(90);
    std::time::Duration::from_secs(seconds)
}

pub(crate) fn construction_wait(
    construct_ok: bool,
    observed_scaffold: bool,
    under_construction: u32,
    elapsed: std::time::Duration,
    timeout: std::time::Duration,
) -> ConstructionWait {
    let complete = construct_ok && observed_scaffold && under_construction == 0;
    if complete && elapsed > std::time::Duration::from_secs(8) {
        ConstructionWait::Ready
    } else if elapsed > timeout {
        ConstructionWait::TimedOut
    } else {
        ConstructionWait::Pending
    }
}
