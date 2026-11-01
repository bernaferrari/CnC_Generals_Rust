/// A bounded navigation attempt allows real menu transition frames between
/// retries. Repeated misses must not reset the total fallback deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationWait {
    Wait,
    Retry,
    TimedOut,
}

pub(crate) fn windowed_navigation_wait(
    elapsed: std::time::Duration,
    since_last_command: std::time::Duration,
    received_miss: bool,
) -> NavigationWait {
    if elapsed >= std::time::Duration::from_secs(12) {
        NavigationWait::TimedOut
    } else if received_miss && since_last_command > std::time::Duration::from_millis(600) {
        NavigationWait::Retry
    } else {
        NavigationWait::Wait
    }
}
