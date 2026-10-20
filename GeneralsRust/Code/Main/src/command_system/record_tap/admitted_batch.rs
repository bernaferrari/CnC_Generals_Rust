//! The actual admitted one-frame command list, owned across source clear/reset.
//! C++ GameEngine.cpp301 keeps CommandList outside the subsystem reset list.
use super::*;

/// Internal source entries retain their original relative order. No entry or
/// batch is Clone: a borrowed driver returns its unconsumed owner exactly once.
#[derive(Debug)]
pub(crate) enum AdmittedCommand {
    Queued(GameCommand),
    Message(GameMessage),
}

#[derive(Debug)]
pub(crate) struct AdmittedCommandBatch {
    queued: VecDeque<GameCommand>,
    messages: VecDeque<GameMessage>,
}

impl AdmittedCommandBatch {
    pub(super) fn new(queued: VecDeque<GameCommand>, messages: Vec<GameMessage>) -> Self {
        Self {
            queued,
            messages: messages.into(),
        }
    }

    pub(crate) fn pop_front(&mut self) -> Option<AdmittedCommand> {
        self.queued
            .pop_front()
            .map(AdmittedCommand::Queued)
            .or_else(|| self.messages.pop_front().map(AdmittedCommand::Message))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.queued.is_empty() && self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, ReplayPendingState) {
        let directory = tempfile::tempdir().unwrap();
        let mut state = ReplayPendingState::default();
        state.reset_recorder_owned(game_engine::common::recorder::RecorderOwnerInputs {
            map: "Maps/OrderedAdmission".into(),
            seed: 37,
            replay_directory: directory.path().join("Replays"),
            exe_crc: 0,
            ini_crc: 0,
            save_stats: false,
            stats_directory: directory.path().join("Stats"),
        });
        (directory, state)
    }

    fn next_type(batch: &mut AdmittedCommandBatch) -> GameMessageType {
        match batch.pop_front().expect("remaining admitted message") {
            AdmittedCommand::Message(message) => message.get_type().clone(),
            AdmittedCommand::Queued(_) => panic!("expected raw source message"),
        }
    }

    #[test]
    fn owned_admitted_tail_survives_actual_reset_and_disconnects_future_loan() {
        let (_directory, mut state) = fixture();
        let old_sender = state.sender.clone();
        let mut start = GameMessage::new(GameMessageType::NewGame);
        for argument in [2, 1, 0, 30] {
            start.append_integer_argument(argument);
        }
        state
            .receive_client_messages(vec![
                GameMessage::new(GameMessageType::LogicCRC(1)),
                GameMessage::new(GameMessageType::ClearGameData),
                start,
                GameMessage::new(GameMessageType::LogicCRC(2)),
            ])
            .unwrap();
        assert!(state.receiving_clear_command_pending());
        assert!(state.receiving_clear_command_pending());
        let mut batch = state
            .admit_command_batch_owned(37, &mut VecDeque::new(), 0, -1, None)
            .expect("first actual frame admission");
        assert!(
            state.take_lifecycle_messages_owned().is_empty(),
            "admission does not split or dispatch lifecycle before its source cursor"
        );
        assert!(matches!(
            next_type(&mut batch),
            GameMessageType::LogicCRC(1)
        ));
        assert!(matches!(
            next_type(&mut batch),
            GameMessageType::ClearGameData
        ));
        state.reset_recorder_for_match_owned(String::new(), 19);
        assert!(
            old_sender
                .send(vec![GameMessage::new(GameMessageType::ClearGameData)])
                .is_err()
        );
        assert!(!state.receiving_clear_command_pending());
        match batch.pop_front().expect("source NewGame survives reset") {
            AdmittedCommand::Message(message) => {
                assert!(matches!(message.get_type(), GameMessageType::NewGame));
                assert!(matches!(
                    message.get_argument(0),
                    Some(GameMessageArgumentType::Integer(2))
                ));
                assert!(matches!(
                    message.get_argument(3),
                    Some(GameMessageArgumentType::Integer(30))
                ));
            }
            _ => panic!("source lifecycle ordering changed"),
        }
        assert!(matches!(
            next_type(&mut batch),
            GameMessageType::LogicCRC(2)
        ));
        assert!(batch.is_empty());
    }

    #[test]
    fn repeated_freeze_peek_and_recorder_offer_preserve_one_ordered_admission() {
        let (_directory, mut state) = fixture();
        state
            .receive_client_messages(vec![GameMessage::new(GameMessageType::LogicCRC(11))])
            .unwrap();
        state
            .receive_client_messages(vec![
                GameMessage::new(GameMessageType::ClearGameData),
                GameMessage::new(GameMessageType::LogicCRC(22)),
            ])
            .unwrap();
        assert!(state.receiving_clear_command_pending());
        assert!(state.receiving_clear_command_pending());
        let mut queue = VecDeque::new();
        let mut batch = state
            .admit_command_batch_owned(37, &mut queue, 0, -1, None)
            .unwrap();
        assert!(
            state
                .admit_command_batch_owned(37, &mut queue, 0, -1, None)
                .is_none(),
            "Recorder may be offered this frame exactly once"
        );
        assert!(matches!(
            next_type(&mut batch),
            GameMessageType::LogicCRC(11)
        ));
        assert!(matches!(
            next_type(&mut batch),
            GameMessageType::ClearGameData
        ));
        assert!(matches!(
            next_type(&mut batch),
            GameMessageType::LogicCRC(22)
        ));
        assert!(batch.is_empty());
    }
}
