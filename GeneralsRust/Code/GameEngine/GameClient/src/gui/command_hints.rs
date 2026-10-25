//! Client-owned issued-order feedback. C++ InGameUI.cpp:2141-2179 and
//! W3DInGameUI.cpp:444-561: 256 move slots and elapsed client frames <=40.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandHintKind {
    Move,
    Attack,
}
#[derive(Debug, Clone, Copy)]
pub struct CommandHint {
    pub kind: CommandHintKind,
    pub position: glam::Vec3,
    pub creation_frame: u32,
    pub source_id: u32,
}
#[derive(Default)]
pub struct CommandHintState {
    hints: Vec<CommandHint>,
}
impl CommandHintState {
    pub fn clear(&mut self) {
        self.hints.clear();
    }
    pub fn record(
        &mut self,
        kind: CommandHintKind,
        position: glam::Vec3,
        source_id: u32,
        client_frame: u32,
        eligible: bool,
    ) {
        if kind == CommandHintKind::Move {
            self.hints
                .retain(|hint| hint.kind != kind || hint.source_id != source_id);
        }
        if !eligible {
            return;
        }
        // Attack rings retain the existing declared host adaptation; retail's
        // createAttackHint body is empty. They never select a Core UI registry.
        if self.hints.len() >= 256 {
            self.hints.remove(0);
        }
        self.hints.push(CommandHint {
            kind,
            position,
            creation_frame: client_frame,
            source_id,
        });
    }
    pub fn snapshot(&self, client_frame: u32) -> Vec<CommandHint> {
        self.hints
            .iter()
            .copied()
            .filter(|hint| client_frame.wrapping_sub(hint.creation_frame) <= 40)
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_frame_lifetime_dedup_and_peer_reset_are_independent() {
        let mut a = CommandHintState::default();
        let mut b = CommandHintState::default();
        a.record(CommandHintKind::Move, glam::Vec3::ONE, 7, 100, true);
        b.record(CommandHintKind::Move, glam::Vec3::ZERO, 7, 2, true);
        b.clear();
        assert_eq!(a.snapshot(140).len(), 1);
        assert!(a.snapshot(141).is_empty());
        a.record(CommandHintKind::Move, glam::Vec3::ZERO, 7, 110, true);
        assert_eq!(a.snapshot(110).len(), 1);
        assert_eq!(a.snapshot(110)[0].position, glam::Vec3::ZERO);
        a.record(CommandHintKind::Move, glam::Vec3::ONE, 7, 110, false);
        assert!(a.snapshot(110).is_empty());
    }
}
