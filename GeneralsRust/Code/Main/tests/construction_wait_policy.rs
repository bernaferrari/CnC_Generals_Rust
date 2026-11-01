//! Slow diagnostics wait for the real scaffold; they never synthesize success.
#[path = "../src/executable_smoke/construction_wait.rs"]
mod construction_wait;
use construction_wait::{ConstructionWait, construction_timeout, construction_wait};
use std::time::Duration;

#[test]
fn extended_wait_requires_a_seen_and_completed_scaffold() {
    let default = construction_timeout(None);
    let extended = construction_timeout(Some("240"));
    assert_eq!(default, Duration::from_secs(90));
    assert_eq!(extended, Duration::from_secs(240));
    assert_eq!(construction_timeout(Some("0")), default);
    assert_eq!(construction_timeout(Some("901")), default);
    assert_eq!(construction_timeout(Some("invalid")), default);
    let elapsed = Duration::from_secs(120);
    assert_eq!(
        construction_wait(true, true, 1, elapsed, default),
        ConstructionWait::TimedOut
    );
    assert_eq!(
        construction_wait(true, true, 1, elapsed, extended),
        ConstructionWait::Pending
    );
    assert_eq!(
        construction_wait(true, false, 0, elapsed, extended),
        ConstructionWait::Pending
    );
    assert_eq!(
        construction_wait(false, true, 0, elapsed, extended),
        ConstructionWait::Pending
    );
    assert_eq!(
        construction_wait(true, true, 0, elapsed, extended),
        ConstructionWait::Ready
    );
    assert_eq!(
        construction_wait(true, true, 0, Duration::from_secs(8), extended),
        ConstructionWait::Pending
    );
    assert_eq!(
        construction_wait(true, true, 1, Duration::from_secs(241), extended),
        ConstructionWait::TimedOut
    );
}
