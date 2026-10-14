//! Host UpdateModule sleep/wake scheduler.
//!
//! C++ `GameLogic::update` (`GameLogic.cpp:3699-3740`) ticks object logic
//! through `m_sleepyUpdates` with `friend_setNextCallFrame(now + sleepLen)`.
//! `UPDATE_SLEEP_NONE = 1` (next frame), `UPDATE_SLEEP_FOREVER = 0x3fffffff`.
//!
//! The live host still owns residual system ticks (construction, movement, …)
//! rather than per-object `UpdateModule` callbacks. Their scheduler shares the
//! original indexed-heap mechanics; module registration and phase dispatch
//! still require the per-object migration tracked in hq-om90a.

use game_engine::indexed_heap::{IndexedHeap, IndexedHeapEntry};

/// C++ `UPDATE_SLEEP_NONE` (`UpdateModule.h:44`) — run next frame.
pub const UPDATE_SLEEP_NONE: u32 = 1;
/// C++ `UPDATE_SLEEP_FOREVER` (`UpdateModule.h:52`).
pub const UPDATE_SLEEP_FOREVER: u32 = 0x3fff_ffff;

/// C++ update phases (`UpdateModule.h` friend_getPriority phase bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum HostUpdatePhase {
    Initial = 0,
    Physics = 1,
    Normal = 2,
    Final = 3,
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
            HostSleepyKind::SpecialPowers => HostUpdatePhase::Final,
        }
    }
}

/// Owned callback identity; the indexed entry remains in its receiving heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostSleepyToken {
    pub kind: HostSleepyKind,
    registration: u64,
}

pub struct HostSleepyHeap {
    heap: IndexedHeap<HostSleepyKind, u64>,
    current: Option<HostSleepyToken>,
    next_registration: u64,
}

impl std::fmt::Debug for HostSleepyHeap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostSleepyHeap")
            .field("entries", &self.heap.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for HostSleepyHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl HostSleepyHeap {
    pub fn new() -> Self {
        let mut heap = Self {
            heap: IndexedHeap::new(),
            current: None,
            next_registration: 1,
        };
        // Registration at frame zero becomes frame one. Construction never
        // publishes this receiving scheduler or calls any update callback.
        for kind in HostSleepyKind::ALL {
            heap.register(kind, 1);
        }
        heap
    }

    pub fn register(&mut self, kind: HostSleepyKind, wake_frame: u32) {
        let registration = self.next_registration;
        self.next_registration = self.next_registration.wrapping_add(1).max(1);
        self.heap.push(IndexedHeapEntry {
            key: kind,
            priority: priority(kind, wake_frame.max(1)),
            value: registration,
        });
    }

    pub fn wake(&mut self, kind: HostSleepyKind, now: u32, wake_frame: u32) -> bool {
        debug_assert!(wake_frame >= now, "wake frame is in the past");
        let Some(entry) = self.heap.get(kind) else {
            return false;
        };
        // GameLogic.cpp2964-2983: current callback wake is ignored; NONE
        // must not postpone a receiver that is already due this frame.
        if self
            .current
            .is_some_and(|token| token.kind == kind && token.registration == entry.value)
        {
            return false;
        }
        let existing = entry.priority >> 2;
        if existing == wake_frame
            || (now > 0 && existing == now && wake_frame == now.wrapping_add(1))
        {
            return true;
        }
        self.heap.reschedule(kind, priority(kind, wake_frame))
    }

    pub fn erase(&mut self, kind: HostSleepyKind) -> bool {
        self.heap.erase(kind).is_some()
    }

    /// Peek one due receiver, then release the heap borrow before its callback.
    /// C++ keeps the current update at its heap index until post-update rebalance.
    pub fn next_due(&mut self, now: u32) -> Option<HostSleepyToken> {
        if self.current.is_some() {
            return None;
        }
        let entry = self.heap.peek()?;
        if entry.priority >> 2 > now {
            return None;
        }
        let token = HostSleepyToken {
            kind: entry.key,
            registration: entry.value,
        };
        self.current = Some(token);
        Some(token)
    }

    pub fn finish(&mut self, token: HostSleepyToken, now: u32, sleep: u32) -> bool {
        // GameLogic.cpp3728 normalizes invalid zero sleeps to NONE, adds in
        // unsigned arithmetic, then UpdateModule.h181 clamps the frame.
        if self.current != Some(token) {
            return false;
        }
        self.current = None;
        if !self
            .heap
            .get(token.kind)
            .is_some_and(|entry| entry.value == token.registration)
        {
            return false;
        }
        let next = now.wrapping_add(sleep.max(UPDATE_SLEEP_NONE));
        self.heap
            .complete_root_update(token.kind, priority(token.kind, next))
    }

    pub fn peek_wake_frame(&self) -> Option<u32> {
        self.heap.peek().map(|entry| entry.priority >> 2)
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
}

fn priority(kind: HostSleepyKind, frame: u32) -> u32 {
    (frame.min(UPDATE_SLEEP_FOREVER) << 2) | kind.phase() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_due(heap: &mut HostSleepyHeap, now: u32) -> Vec<HostSleepyKind> {
        let mut due = Vec::new();
        while let Some(token) = heap.next_due(now) {
            due.push(token.kind);
            assert!(heap.finish(token, now, UPDATE_SLEEP_NONE));
        }
        due
    }

    #[test]
    fn host_sleepy_heap_matches_cpp_none_means_next_frame() {
        // C++ UPDATE_SLEEP_NONE=1 → friend_setNextCallFrame(now+1).
        let mut heap = HostSleepyHeap::new();
        assert_eq!(heap.len(), HostSleepyKind::ALL.len());
        let due0 = run_due(&mut heap, 0);
        assert!(due0.is_empty(), "wake seeded at frame 1, not 0");
        let due1 = run_due(&mut heap, 1);
        assert_eq!(due1.len(), HostSleepyKind::ALL.len());
        assert!(due1.contains(&HostSleepyKind::Movement));
        assert!(due1.contains(&HostSleepyKind::Construction));
        let due1_again = run_due(&mut heap, 1);
        assert!(
            due1_again.is_empty(),
            "after NONE sleep, next call is now+1"
        );
        let due2 = run_due(&mut heap, 2);
        assert_eq!(due2.len(), HostSleepyKind::ALL.len());
    }

    #[test]
    fn host_sleepy_heap_phase_orders_construction_before_movement() {
        let mut heap = HostSleepyHeap::new();
        let due = run_due(&mut heap, 1);
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

    #[test]
    fn current_callback_can_wake_another_entry_before_selection_continues() {
        let mut heap = HostSleepyHeap::new();
        heap.wake(HostSleepyKind::SpecialPowers, 0, 50);
        let current = heap.next_due(1).unwrap();
        assert_eq!(heap.len(), 6, "current receiver remains indexed");
        assert!(heap.wake(HostSleepyKind::SpecialPowers, 1, 1));
        assert!(heap.finish(current, 1, UPDATE_SLEEP_NONE));
        assert!(run_due(&mut heap, 1).contains(&HostSleepyKind::SpecialPowers));
        assert_eq!(heap.len(), 6);
    }

    #[test]
    fn erased_callback_is_not_reinserted_and_sleep_clamps_like_cpp() {
        let mut heap = HostSleepyHeap::new();
        let current = heap.next_due(1).unwrap();
        assert!(heap.erase(current.kind));
        assert!(!heap.finish(current, 1, 1));
        assert_eq!(heap.len(), 5);
        heap.register(current.kind, 1);
        // Finish only an actual selected current receiver.
        let current = heap.next_due(1).unwrap();
        assert!(heap.finish(current, 1, UPDATE_SLEEP_FOREVER));
        assert_eq!(
            heap.heap.get(current.kind).unwrap().priority >> 2,
            UPDATE_SLEEP_FOREVER
        );
        let current = heap.next_due(u32::MAX).unwrap();
        assert!(heap.finish(current, u32::MAX, 2));
        assert_eq!(heap.heap.get(current.kind).unwrap().priority >> 2, 1);
        assert_eq!(HostUpdatePhase::Physics as u8, 1);
        assert_eq!(HostUpdatePhase::Normal as u8, 2);
        assert_eq!(HostUpdatePhase::Final as u8, 3);
    }

    #[test]
    fn wake_none_does_not_postpone_due_receiver_or_override_current_return() {
        let mut heap = HostSleepyHeap::new();
        assert!(heap.wake(HostSleepyKind::Movement, 1, 2));
        assert_eq!(
            heap.heap.get(HostSleepyKind::Movement).unwrap().priority >> 2,
            1
        );
        let current = heap.next_due(1).unwrap();
        assert!(!heap.wake(current.kind, 1, 20));
        assert!(heap.finish(current, 1, 4));
        assert_eq!(heap.heap.get(current.kind).unwrap().priority >> 2, 5);
    }

    #[test]
    fn registration_during_callback_finishes_original_then_rebalances_root() {
        let mut heap = HostSleepyHeap::new();
        assert_eq!(run_due(&mut heap, 1).len(), 6);
        let current = heap.next_due(20).unwrap();
        // The newly registered earlier receiver becomes root during update.
        heap.register(HostSleepyKind::Movement, 1);
        assert_eq!(heap.heap.peek().unwrap().key, HostSleepyKind::Movement);
        let original_index = heap.heap.index_of(current.kind).unwrap();
        assert!(original_index > 0);
        assert!(heap.finish(current, 20, UPDATE_SLEEP_NONE));
        // Literal CPP3728 updates original u, but rebalances root zero.
        // Current is neither popped nor appended as a new heap entry.
        assert_eq!(heap.heap.index_of(current.kind), Some(original_index));
        assert_eq!(heap.heap.get(current.kind).unwrap().priority >> 2, 21);
        assert_eq!(heap.next_due(20).unwrap().kind, HostSleepyKind::Movement);
        assert_eq!(heap.len(), 6);
    }

    #[test]
    fn old_callback_cannot_finish_a_replacement_registration_with_same_kind() {
        let mut heap = HostSleepyHeap::new();
        let current = heap.next_due(1).unwrap();
        assert!(heap.erase(current.kind));
        heap.register(current.kind, 7);
        assert!(!heap.finish(current, 1, UPDATE_SLEEP_NONE));
        assert_eq!(heap.heap.get(current.kind).unwrap().priority >> 2, 7);
        let other_due = run_due(&mut heap, 1);
        assert!(!other_due.contains(&current.kind));
        assert!(run_due(&mut heap, 7).contains(&current.kind));
    }
}
