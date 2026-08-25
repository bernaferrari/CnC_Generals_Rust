//! Host UpdateModule sleep/wake scheduler.
//!
//! C++ `GameLogic::update` (`GameLogic.cpp:3699-3740`) ticks object logic
//! through `m_sleepyUpdates` with `friend_setNextCallFrame(now + sleepLen)`.
//! `UPDATE_SLEEP_NONE = 1` (next frame), `UPDATE_SLEEP_FOREVER = 0x3fffffff`.
//!
//! The live host still owns residual system ticks (construction, movement, …)
//! rather than crate `UpdateModule` trait objects. This heap is the host
//! equivalent: each residual is a scheduled module with a next-call frame.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// C++ `UPDATE_SLEEP_NONE` (`UpdateModule.h:44`) — run next frame.
pub const UPDATE_SLEEP_NONE: u32 = 1;
/// C++ `UPDATE_SLEEP_FOREVER` (`UpdateModule.h:52`).
pub const UPDATE_SLEEP_FOREVER: u32 = 0x3fff_ffff;

/// C++ update phases (`UpdateModule.h` friend_getPriority phase bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum HostUpdatePhase {
    Initial = 0,
    Normal = 1,
    Late = 2,
}

/// Residual systems that C++ would run as per-object UpdateModules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostSleepyKind {
    Construction,
    SellList,
    DozerBoredRepair,
    RebuildHoles,
    Movement,
    SpecialPowers,
}

impl HostSleepyKind {
    pub const ALL: [HostSleepyKind; 6] = [
        HostSleepyKind::Construction,
        HostSleepyKind::SellList,
        HostSleepyKind::DozerBoredRepair,
        HostSleepyKind::RebuildHoles,
        HostSleepyKind::Movement,
        HostSleepyKind::SpecialPowers,
    ];

    fn phase(self) -> HostUpdatePhase {
        match self {
            HostSleepyKind::Construction
            | HostSleepyKind::SellList
            | HostSleepyKind::DozerBoredRepair
            | HostSleepyKind::RebuildHoles => HostUpdatePhase::Initial,
            HostSleepyKind::Movement => HostUpdatePhase::Normal,
            HostSleepyKind::SpecialPowers => HostUpdatePhase::Late,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SleepyEntry {
    wake_frame: u32,
    phase: HostUpdatePhase,
    kind: HostSleepyKind,
}

impl PartialOrd for SleepyEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SleepyEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap: earlier wake, then earlier phase (C++ `(frame<<2)|phase`).
        other
            .wake_frame
            .cmp(&self.wake_frame)
            .then_with(|| other.phase.cmp(&self.phase))
    }
}

#[derive(Debug, Default)]
pub struct HostSleepyHeap {
    heap: BinaryHeap<SleepyEntry>,
}

impl HostSleepyHeap {
    pub fn new() -> Self {
        let mut heap = Self {
            heap: BinaryHeap::new(),
        };
        // C++ registerObject maps wake 0 → now (min 1). First logic frame is 1
        // after the increment at the end of update, so seed wake=1.
        for kind in HostSleepyKind::ALL {
            heap.heap.push(SleepyEntry {
                wake_frame: 1,
                phase: kind.phase(),
                kind,
            });
        }
        heap
    }

    /// Pop every module whose `wake_frame <= now`. Requeue at `now + sleep`.
    pub fn drain_due(&mut self, now: u32) -> Vec<HostSleepyKind> {
        let mut due = Vec::new();
        while let Some(top) = self.heap.peek() {
            if top.wake_frame > now {
                break;
            }
            let entry = self.heap.pop().expect("peeked");
            due.push(entry.kind);
            let sleep = UPDATE_SLEEP_NONE;
            let next = now.saturating_add(sleep).min(UPDATE_SLEEP_FOREVER);
            self.heap.push(SleepyEntry {
                wake_frame: next.max(now.saturating_add(1)),
                phase: entry.kind.phase(),
                kind: entry.kind,
            });
        }
        due
    }

    pub fn peek_wake_frame(&self) -> Option<u32> {
        self.heap.peek().map(|e| e.wake_frame)
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_sleepy_heap_matches_cpp_none_means_next_frame() {
        // C++ UPDATE_SLEEP_NONE=1 → friend_setNextCallFrame(now+1).
        let mut heap = HostSleepyHeap::new();
        assert_eq!(heap.len(), HostSleepyKind::ALL.len());
        let due0 = heap.drain_due(0);
        assert!(due0.is_empty(), "wake seeded at frame 1, not 0");
        let due1 = heap.drain_due(1);
        assert_eq!(due1.len(), HostSleepyKind::ALL.len());
        assert!(due1.contains(&HostSleepyKind::Movement));
        assert!(due1.contains(&HostSleepyKind::Construction));
        let due1_again = heap.drain_due(1);
        assert!(
            due1_again.is_empty(),
            "after NONE sleep, next call is now+1"
        );
        let due2 = heap.drain_due(2);
        assert_eq!(due2.len(), HostSleepyKind::ALL.len());
    }

    #[test]
    fn host_sleepy_heap_phase_orders_construction_before_movement() {
        let mut heap = HostSleepyHeap::new();
        let due = heap.drain_due(1);
        let c = due
            .iter()
            .position(|k| *k == HostSleepyKind::Construction)
            .unwrap();
        let m = due
            .iter()
            .position(|k| *k == HostSleepyKind::Movement)
            .unwrap();
        assert!(
            c < m,
            "Initial phase before Normal (C++ priority phase bits)"
        );
    }
}
