//! Script-visible completion history (C++ ScriptEngine.cpp:7194–7485).

use super::program::ConditionType;
use crate::time::LOGIC_FRAMES_PER_SECOND;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug)]
pub enum PlayerEventKind {
    PowerTriggered,
    PowerMidway,
    PowerCompleted,
    UpgradeCompleted,
}

impl PlayerEventKind {
    /// Resolve the event family and whether its condition names a source.
    pub fn condition(condition: ConditionType) -> Option<(Self, bool)> {
        use ConditionType::*;
        Some(match condition {
            PlayerTriggeredSpecialPower => (Self::PowerTriggered, false),
            PlayerMidwaySpecialPower => (Self::PowerMidway, false),
            PlayerCompletedSpecialPower => (Self::PowerCompleted, false),
            PlayerBuiltUpgrade => (Self::UpgradeCompleted, false),
            PlayerTriggeredSpecialPowerFromNamed => (Self::PowerTriggered, true),
            PlayerMidwaySpecialPowerFromNamed => (Self::PowerMidway, true),
            PlayerCompletedSpecialPowerFromNamed => (Self::PowerCompleted, true),
            PlayerBuiltUpgradeFromNamed => (Self::UpgradeCompleted, true),
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum TimedMediaKind {
    Speech,
    Audio,
}

/// Ordered lists used by the existing save adapters. Each outer player list
/// has one entry per slot when held by a live interpreter.
#[derive(Clone, Default)]
pub struct ScriptEventHistory {
    pub completed_video: Vec<String>,
    pub testing_speech: Vec<(String, u32)>,
    pub testing_audio: Vec<(String, u32)>,
    pub triggered_special_powers: Vec<Vec<(String, u32)>>,
    pub midway_special_powers: Vec<Vec<(String, u32)>>,
    pub finished_special_powers: Vec<Vec<(String, u32)>>,
    pub completed_upgrades: Vec<Vec<(String, u32)>>,
    pub acquired_sciences: Vec<Vec<i32>>,
}

impl ScriptEventHistory {
    fn named_players_mut(&mut self) -> [&mut Vec<Vec<(String, u32)>>; 4] {
        [
            &mut self.triggered_special_powers,
            &mut self.midway_special_powers,
            &mut self.finished_special_powers,
            &mut self.completed_upgrades,
        ]
    }

    fn resize_players(&mut self, players: usize) {
        for slots in self.named_players_mut() {
            slots.resize_with(players, Vec::new);
        }
        self.acquired_sciences.resize_with(players, Vec::new);
    }

    fn named_list_mut(
        &mut self,
        kind: PlayerEventKind,
        player: usize,
    ) -> Option<&mut Vec<(String, u32)>> {
        let players = match kind {
            PlayerEventKind::PowerTriggered => &mut self.triggered_special_powers,
            PlayerEventKind::PowerMidway => &mut self.midway_special_powers,
            PlayerEventKind::PowerCompleted => &mut self.finished_special_powers,
            PlayerEventKind::UpgradeCompleted => &mut self.completed_upgrades,
        };
        players.get_mut(player)
    }

    fn media_list_mut(&mut self, kind: TimedMediaKind) -> &mut Vec<(String, u32)> {
        match kind {
            TimedMediaKind::Speech => &mut self.testing_speech,
            TimedMediaKind::Audio => &mut self.testing_audio,
        }
    }
}

/// Mutable history for one interpreter. Immediate callbacks can borrow this
/// independently of the interpreter's frame and program state.
pub struct ScriptEvents {
    history: RefCell<ScriptEventHistory>,
}

impl ScriptEvents {
    pub fn new(players: usize) -> Self {
        let mut history = ScriptEventHistory::default();
        history.resize_players(players);
        Self {
            history: RefCell::new(history),
        }
    }

    pub fn clear(&mut self) {
        let history = self.history.get_mut();
        history.completed_video.clear();
        history.testing_speech.clear();
        history.testing_audio.clear();
        for slots in history.named_players_mut() {
            slots.iter_mut().for_each(Vec::clear);
        }
        history.acquired_sciences.iter_mut().for_each(Vec::clear);
    }

    pub fn snapshot(&self) -> ScriptEventHistory {
        self.history.borrow().clone()
    }

    pub fn restore(&self, mut saved: ScriptEventHistory) {
        let mut history = self.history.borrow_mut();
        saved.resize_players(history.acquired_sciences.len());
        *history = saved;
    }

    /// Legacy Xfer visits these lists in their original wire order.
    pub fn legacy_history_mut(&mut self) -> &mut ScriptEventHistory {
        self.history.get_mut()
    }

    pub fn record_player_event(
        &self,
        kind: PlayerEventKind,
        player: usize,
        name: &str,
        source: u32,
    ) {
        let mut history = self.history.borrow_mut();
        if let Some(list) = history.named_list_mut(kind, player) {
            list.push((name.to_owned(), source));
        } else {
            log::warn!("Script event {kind:?}: player index {player} out of range");
        }
    }

    pub fn has_player_event(
        &self,
        kind: PlayerEventKind,
        player: usize,
        name: &str,
        source: Option<u32>,
        consume: bool,
    ) -> bool {
        self.history
            .borrow_mut()
            .named_list_mut(kind, player)
            .is_some_and(|list| {
                match_first(list, consume, |(event_name, id)| {
                    event_name == name && source.is_none_or(|source| source == *id)
                })
            })
    }

    pub fn record_science(&self, player: usize, science: i32) {
        if let Some(list) = self.history.borrow_mut().acquired_sciences.get_mut(player) {
            list.push(science);
            log::debug!("Player {player} acquired science: {science}");
        }
    }

    pub fn has_science(&self, player: usize, science: i32, consume: bool) -> bool {
        self.history
            .borrow_mut()
            .acquired_sciences
            .get_mut(player)
            .is_some_and(|list| match_first(list, consume, |entry| *entry == science))
    }

    pub fn record_video(&self, name: &str) {
        self.history
            .borrow_mut()
            .completed_video
            .push(name.to_owned());
        log::debug!("Video completed: {name}");
    }

    pub fn has_video(&self, name: &str, consume: bool) -> bool {
        match_first(
            &mut self.history.borrow_mut().completed_video,
            consume,
            |video| video == name,
        )
    }

    /// Begin timing a sound only on the first condition query for its name.
    pub fn timed_media_complete(
        &self,
        kind: TimedMediaKind,
        name: &str,
        frame: u32,
        consume: bool,
        duration_ms: impl FnOnce() -> f32,
    ) -> bool {
        let mut history = self.history.borrow_mut();
        let list = history.media_list_mut(kind);
        let position = list
            .iter()
            .position(|(event_name, _)| event_name == name)
            .unwrap_or_else(|| {
                let due = frame.wrapping_add(duration_frames(duration_ms()));
                list.insert(0, (name.to_owned(), due));
                0
            });
        let complete = frame >= list[position].1;
        if complete && consume {
            list.remove(position);
        }
        complete
    }
}

fn match_first<T>(rows: &mut Vec<T>, consume: bool, predicate: impl FnMut(&T) -> bool) -> bool {
    let Some(position) = rows.iter().position(predicate) else {
        return false;
    };
    if consume {
        rows.remove(position);
    }
    true
}

fn duration_frames(duration_ms: f32) -> u32 {
    // C++ divides by a rounded Real milliseconds-per-frame constant first.
    (duration_ms.max(0.0) / (1_000.0 / LOGIC_FRAMES_PER_SECOND)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_frame_conversion_uses_cpp_milliseconds_per_frame_division() {
        let one_frame_ms = f32::from_bits(0x4205_5555);
        assert_eq!(duration_frames(one_frame_ms), 1);
        assert_eq!(
            duration_frames(f32::from_bits(one_frame_ms.to_bits() - 1)),
            0
        );
        assert_eq!(duration_frames(100.0), 3);
        assert_eq!(duration_frames(1_000.0), 30);
    }

    #[test]
    fn restoring_history_preserves_duplicate_order_and_normalizes_player_slots() {
        let saved = ScriptEventHistory {
            triggered_special_powers: vec![vec![("Power".into(), 5), ("Power".into(), 9)]],
            acquired_sciences: vec![vec![63, 63], vec![], vec![91]],
            ..ScriptEventHistory::default()
        };
        let mut events = ScriptEvents::new(2);
        events.restore(saved);
        assert!(events.has_player_event(PlayerEventKind::PowerTriggered, 0, "Power", None, true));
        assert!(!events.has_player_event(
            PlayerEventKind::PowerTriggered,
            0,
            "Power",
            Some(5),
            false
        ));
        assert!(events.has_player_event(
            PlayerEventKind::PowerTriggered,
            0,
            "Power",
            Some(9),
            false
        ));
        assert!(!events.has_player_event(PlayerEventKind::PowerCompleted, 0, "Power", None, false));
        assert!(events.has_science(0, 63, true));
        assert!(events.has_science(0, 63, false));
        assert!(!events.has_science(2, 91, false));

        events.record_science(1, 8);
        assert!(events.has_science(1, 8, false));
        events.clear();
        assert!(
            events
                .snapshot()
                .acquired_sciences
                .iter()
                .all(Vec::is_empty)
        );
        assert_eq!(events.snapshot().triggered_special_powers, [vec![], vec![]]);
    }
}
