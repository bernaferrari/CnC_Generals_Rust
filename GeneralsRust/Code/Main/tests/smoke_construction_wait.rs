#[path = "../src/executable_smoke/construction_wait.rs"]
mod construction_wait;

use construction_wait::{ConstructionWait, construction_wait};
use std::time::Duration;

#[test]
fn waits_for_an_observed_scaffold_to_finish_before_training() {
    assert_eq!(
        construction_wait(true, true, 1, Duration::from_secs(89), Duration::from_secs(90)),
        ConstructionWait::Pending
    );
    assert_eq!(
        construction_wait(true, true, 0, Duration::from_secs(8), Duration::from_secs(90)),
        ConstructionWait::Pending
    );
    assert_eq!(
        construction_wait(true, true, 0, Duration::from_secs(9), Duration::from_secs(90)),
        ConstructionWait::Ready
    );
    assert_eq!(
        construction_wait(true, true, 0, Duration::from_secs(91), Duration::from_secs(90)),
        ConstructionWait::Ready
    );
}

#[test]
fn timeout_does_not_treat_an_unfinished_or_unobserved_build_as_ready() {
    for (construct_ok, observed, active) in [(true, true, 1), (true, false, 0), (false, true, 0)] {
        assert_eq!(
            construction_wait(construct_ok, observed, active, Duration::from_secs(91), Duration::from_secs(90)),
            ConstructionWait::TimedOut
        );
    }
}
