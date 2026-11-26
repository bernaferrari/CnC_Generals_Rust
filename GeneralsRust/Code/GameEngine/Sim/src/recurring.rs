//! Sleep/wake scheduling for the receiving simulation's recurring updates.
//! C++ GameLogic.cpp:3699–3740 keeps each active update indexed through its callback.

use generals_game_domain::indexed_heap::{IndexedHeap, IndexedHeapEntry};

/// C++ `UPDATE_SLEEP_NONE` (`UpdateModule.h:44`) — run next frame.
pub const UPDATE_SLEEP_NONE: u32 = 1;
/// C++ `UPDATE_SLEEP_FOREVER` (`UpdateModule.h:52`).
pub const UPDATE_SLEEP_FOREVER: u32 = 0x3fff_ffff;

/// C++ update phases (`UpdateModule.h` friend_getPriority phase bits).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum UpdatePhase {
    Initial = 0,
    Physics = 1,
    Normal = 2,
    Final = 3,
}

/// Recurring system callbacks installed by the current simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecurringUpdate {
    Construction,
    SellList,
    DozerBoredRepair,
    RebuildHoles,
    Movement,
    SpecialPowers,
}

impl RecurringUpdate {
    pub const ALL: [RecurringUpdate; 6] = [
        RecurringUpdate::Construction,
        RecurringUpdate::SellList,
        RecurringUpdate::DozerBoredRepair,
        RecurringUpdate::RebuildHoles,
        RecurringUpdate::Movement,
        RecurringUpdate::SpecialPowers,
    ];

    fn phase(self) -> UpdatePhase {
        match self {
            RecurringUpdate::Construction
            | RecurringUpdate::SellList
            | RecurringUpdate::DozerBoredRepair
            | RecurringUpdate::RebuildHoles => UpdatePhase::Initial,
            RecurringUpdate::Movement => UpdatePhase::Normal,
            RecurringUpdate::SpecialPowers => UpdatePhase::Final,
        }
    }
}

/// Owned callback identity; the indexed entry remains in its receiving heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeToken {
    pub kind: RecurringUpdate,
    registration: u64,
}

pub struct RecurringUpdates {
    heap: IndexedHeap<RecurringUpdate, u64>,
    current: Option<WakeToken>,
    next_registration: u64,
}

impl std::fmt::Debug for RecurringUpdates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecurringUpdates")
            .field("entries", &self.heap.iter().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for RecurringUpdates {
    fn default() -> Self {
        Self::new()
    }
}

impl RecurringUpdates {
    pub fn new() -> Self {
        let mut heap = Self {
            heap: IndexedHeap::new(),
            current: None,
            next_registration: 1,
        };
        // Registration at frame zero becomes frame one. Construction never
        // publishes this receiving scheduler or calls any update callback.
        for kind in RecurringUpdate::ALL {
            heap.register(kind, 1);
        }
        heap
    }

    pub fn register(&mut self, kind: RecurringUpdate, wake_frame: u32) {
        let registration = self.next_registration;
        self.next_registration = self.next_registration.wrapping_add(1).max(1);
        self.heap.push(IndexedHeapEntry {
            key: kind,
            priority: priority(kind, wake_frame.max(1)),
            value: registration,
        });
    }

    pub fn wake(&mut self, kind: RecurringUpdate, now: u32, wake_frame: u32) -> bool {
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

    pub fn erase(&mut self, kind: RecurringUpdate) -> bool {
        self.heap.erase(kind).is_some()
    }

    /// Peek one due receiver, then release the heap borrow before its callback.
    /// C++ keeps the current update at its heap index until post-update rebalance.
    pub fn next_due(&mut self, now: u32) -> Option<WakeToken> {
        if self.current.is_some() {
            return None;
        }
        let entry = self.heap.peek()?;
        if entry.priority >> 2 > now {
            return None;
        }
        let token = WakeToken {
            kind: entry.key,
            registration: entry.value,
        };
        self.current = Some(token);
        Some(token)
    }

    pub fn finish(&mut self, token: WakeToken, now: u32, sleep: u32) -> bool {
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

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
}

fn priority(kind: RecurringUpdate, frame: u32) -> u32 {
    (frame.min(UPDATE_SLEEP_FOREVER) << 2) | kind.phase() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_due(heap: &mut RecurringUpdates, now: u32) -> Vec<RecurringUpdate> {
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
        let mut heap = RecurringUpdates::new();
        assert_eq!(heap.len(), RecurringUpdate::ALL.len());
        let due0 = run_due(&mut heap, 0);
        assert!(due0.is_empty(), "wake seeded at frame 1, not 0");
        let due1 = run_due(&mut heap, 1);
        assert_eq!(due1.len(), RecurringUpdate::ALL.len());
        assert!(due1.contains(&RecurringUpdate::Movement));
        assert!(due1.contains(&RecurringUpdate::Construction));
        let due1_again = run_due(&mut heap, 1);
        assert!(
            due1_again.is_empty(),
            "after NONE sleep, next call is now+1"
        );
        let due2 = run_due(&mut heap, 2);
        assert_eq!(due2.len(), RecurringUpdate::ALL.len());
    }

    #[test]
    fn host_sleepy_heap_phase_orders_construction_before_movement() {
        let mut heap = RecurringUpdates::new();
        let due = run_due(&mut heap, 1);
        let c = due
            .iter()
            .position(|k| *k == RecurringUpdate::Construction)
            .unwrap();
        let m = due
            .iter()
            .position(|k| *k == RecurringUpdate::Movement)
            .unwrap();
        assert!(
            c < m,
            "Initial phase before Normal (C++ priority phase bits)"
        );
    }

    #[test]
    fn current_callback_can_wake_another_entry_before_selection_continues() {
        let mut heap = RecurringUpdates::new();
        heap.wake(RecurringUpdate::SpecialPowers, 0, 50);
        let current = heap.next_due(1).unwrap();
        assert_eq!(heap.len(), 6, "current receiver remains indexed");
        assert!(heap.wake(RecurringUpdate::SpecialPowers, 1, 1));
        assert!(heap.finish(current, 1, UPDATE_SLEEP_NONE));
        assert!(run_due(&mut heap, 1).contains(&RecurringUpdate::SpecialPowers));
        assert_eq!(heap.len(), 6);
    }

    #[test]
    fn erased_callback_is_not_reinserted_and_sleep_clamps_like_cpp() {
        let mut heap = RecurringUpdates::new();
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
        assert_eq!(UpdatePhase::Physics as u8, 1);
        assert_eq!(UpdatePhase::Normal as u8, 2);
        assert_eq!(UpdatePhase::Final as u8, 3);
    }

    #[test]
    fn wake_none_does_not_postpone_due_receiver_or_override_current_return() {
        let mut heap = RecurringUpdates::new();
        assert!(heap.wake(RecurringUpdate::Movement, 1, 2));
        assert_eq!(
            heap.heap.get(RecurringUpdate::Movement).unwrap().priority >> 2,
            1
        );
        let current = heap.next_due(1).unwrap();
        assert!(!heap.wake(current.kind, 1, 20));
        assert!(heap.finish(current, 1, 4));
        assert_eq!(heap.heap.get(current.kind).unwrap().priority >> 2, 5);
    }

    #[test]
    fn registration_during_callback_finishes_original_then_rebalances_root() {
        let mut heap = RecurringUpdates::new();
        assert_eq!(run_due(&mut heap, 1).len(), 6);
        let current = heap.next_due(20).unwrap();
        // The newly registered earlier receiver becomes root during update.
        heap.register(RecurringUpdate::Movement, 1);
        assert_eq!(heap.heap.peek().unwrap().key, RecurringUpdate::Movement);
        let original_index = heap.heap.index_of(current.kind).unwrap();
        assert!(original_index > 0);
        assert!(heap.finish(current, 20, UPDATE_SLEEP_NONE));
        // Literal CPP3728 updates original u, but rebalances root zero.
        // Current is neither popped nor appended as a new heap entry.
        assert_eq!(heap.heap.index_of(current.kind), Some(original_index));
        assert_eq!(heap.heap.get(current.kind).unwrap().priority >> 2, 21);
        assert_eq!(heap.next_due(20).unwrap().kind, RecurringUpdate::Movement);
        assert_eq!(heap.len(), 6);
    }

    #[test]
    fn old_callback_cannot_finish_a_replacement_registration_with_same_kind() {
        let mut heap = RecurringUpdates::new();
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
