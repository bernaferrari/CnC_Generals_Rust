// Readiness for the windowed smoke's physical construction and training chain.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstructionWait {
    Pending,
    Ready,
    TimedOut,
}

pub(crate) fn construction_wait(
    construct_ok: bool,
    observed_scaffold: bool,
    under_construction: u32,
    elapsed: std::time::Duration,
) -> ConstructionWait {
    let complete = construct_ok && observed_scaffold && under_construction == 0;
    if complete && elapsed > std::time::Duration::from_secs(8) {
        ConstructionWait::Ready
    } else if elapsed > std::time::Duration::from_secs(90) {
        ConstructionWait::TimedOut
    } else {
        ConstructionWait::Pending
    }
}
