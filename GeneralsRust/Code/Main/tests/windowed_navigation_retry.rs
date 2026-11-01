#[path = "../src/executable_smoke/windowed_navigation.rs"]
mod windowed_navigation;

use std::time::Duration;
use windowed_navigation::{NavigationWait, windowed_navigation_wait};

#[test]
fn retries_allow_transition_frames_without_extending_the_total_deadline() {
    assert_eq!(
        windowed_navigation_wait(Duration::from_secs(3), Duration::from_millis(100), true),
        NavigationWait::Wait
    );
    assert_eq!(
        windowed_navigation_wait(Duration::from_secs(3), Duration::from_millis(700), false),
        NavigationWait::Wait
    );
    // Four quick misses must not end the attempt while the authored animation runs.
    assert_eq!(
        windowed_navigation_wait(Duration::from_secs(3), Duration::from_millis(700), true),
        NavigationWait::Retry
    );
    // A recent retry must not postpone the fallback indefinitely.
    assert_eq!(
        windowed_navigation_wait(Duration::from_secs(12), Duration::from_millis(100), true),
        NavigationWait::TimedOut
    );
}
