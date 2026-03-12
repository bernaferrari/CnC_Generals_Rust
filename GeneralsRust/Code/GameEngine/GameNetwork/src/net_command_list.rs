//! C++ compatibility implementation for NetCommandList.cpp.
//!
//! Maintains a sorted list of NetCommandRef entries. Sorting order matches
//! C++: command type, then player id, then command id.

use crate::command_types::NetCommandType;
use crate::commands::cpp_compat_serialization::NetCommandRef;
use crate::commands::{CommandPayload, NetCommand};

#[derive(Debug, Default, Clone)]
pub struct NetCommandList {
    commands: Vec<NetCommandRef>,
    last_insert_index: Option<usize>,
}

impl NetCommandList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self) {
        self.reset();
    }

    pub fn reset(&mut self) {
        self.commands.clear();
        self.last_insert_index = None;
    }

    pub fn length(&self) -> usize {
        self.commands.len()
    }

    pub fn get_first_message(&self) -> Option<&NetCommandRef> {
        self.commands.first()
    }

    pub fn append_list(&mut self, other: &NetCommandList) {
        for cmd in &other.commands {
            let _ = self.add_message_ref(cmd.clone());
        }
    }

    pub fn add_message(&mut self, command: NetCommand) -> Option<NetCommandRef> {
        let cmd_ref = NetCommandRef::from_net_command(&command);
        self.add_message_ref(cmd_ref)
    }

    pub fn add_message_ref(&mut self, cmd_ref: NetCommandRef) -> Option<NetCommandRef> {
        if self.commands.is_empty() {
            self.commands.push(cmd_ref.clone());
            self.last_insert_index = Some(0);
            return Some(cmd_ref);
        }

        if let Some(index) = self.last_insert_index {
            if self.can_fast_insert_after(index, &cmd_ref) {
                self.commands.insert(index + 1, cmd_ref.clone());
                self.last_insert_index = Some(index + 1);
                return Some(cmd_ref);
            }
        }

        // Fast path: append at end or insert at front.
        if self.should_insert_after(self.commands.last().unwrap(), &cmd_ref) {
            if self.is_equal_command_msg(self.commands.last().unwrap(), &cmd_ref) {
                return None;
            }
            self.commands.push(cmd_ref.clone());
            self.last_insert_index = Some(self.commands.len() - 1);
            return Some(cmd_ref);
        }

        if self.should_insert_before(self.commands.first().unwrap(), &cmd_ref) {
            if self.is_equal_command_msg(self.commands.first().unwrap(), &cmd_ref) {
                return None;
            }
            self.commands.insert(0, cmd_ref.clone());
            self.last_insert_index = Some(0);
            return Some(cmd_ref);
        }

        // Find insertion point.
        let mut insert_at = self.commands.len();
        for (idx, existing) in self.commands.iter().enumerate() {
            if self.should_insert_before(existing, &cmd_ref) {
                insert_at = idx;
                break;
            }
        }

        if insert_at < self.commands.len() {
            if self.is_equal_command_msg(&self.commands[insert_at], &cmd_ref) {
                return None;
            }
            self.commands.insert(insert_at, cmd_ref.clone());
            self.last_insert_index = Some(insert_at);
            return Some(cmd_ref);
        }

        // Default append.
        self.commands.push(cmd_ref.clone());
        self.last_insert_index = Some(self.commands.len() - 1);
        Some(cmd_ref)
    }

    pub fn remove_message(&mut self, command_id: u16, player_id: u8) -> Option<NetCommandRef> {
        let idx = self.commands.iter().position(|cmd| {
            if self.requires_command_id(cmd.command_type) {
                cmd.id == command_id && cmd.player_id == player_id
            } else {
                false
            }
        })?;
        let removed = self.commands.remove(idx);
        if let Some(last) = self.last_insert_index {
            if last == idx {
                self.last_insert_index = None;
            } else if last > idx {
                self.last_insert_index = Some(last - 1);
            }
        }
        Some(removed)
    }

    pub fn find_message(&self, command_id: u16, player_id: u8) -> Option<&NetCommandRef> {
        self.commands.iter().find(|cmd| {
            if self.requires_command_id(cmd.command_type) {
                cmd.id == command_id && cmd.player_id == player_id
            } else {
                false
            }
        })
    }

    pub fn clear_commands_except_from(&mut self, player_id: u8) {
        self.commands.retain(|cmd| cmd.player_id == player_id);
        self.last_insert_index = None;
    }

    fn can_fast_insert_after(&self, index: usize, cmd_ref: &NetCommandRef) -> bool {
        let current = &self.commands[index];
        if current.command_type == cmd_ref.command_type
            && current.player_id == cmd_ref.player_id
            && current.id < cmd_ref.id
        {
            let next = self.commands.get(index + 1);
            return match next {
                None => !self.is_equal_command_msg(current, cmd_ref),
                Some(next_cmd) => {
                    self.should_insert_before(next_cmd, cmd_ref)
                        && !self.is_equal_command_msg(current, cmd_ref)
                }
            };
        }
        false
    }

    fn should_insert_before(&self, existing: &NetCommandRef, candidate: &NetCommandRef) -> bool {
        let key_existing = self.sort_key(existing);
        let key_candidate = self.sort_key(candidate);
        key_candidate < key_existing
    }

    fn should_insert_after(&self, existing: &NetCommandRef, candidate: &NetCommandRef) -> bool {
        let key_existing = self.sort_key(existing);
        let key_candidate = self.sort_key(candidate);
        key_candidate > key_existing
    }

    fn sort_key(&self, cmd: &NetCommandRef) -> (i32, u8, u16) {
        (cmd.command_type as i32, cmd.player_id, cmd.id)
    }

    fn requires_command_id(&self, command_type: NetCommandType) -> bool {
        matches!(
            command_type,
            NetCommandType::GameCommand
                | NetCommandType::FrameInfo
                | NetCommandType::PlayerLeave
                | NetCommandType::DestroyPlayer
                | NetCommandType::RunAheadMetrics
                | NetCommandType::RunAhead
                | NetCommandType::Chat
                | NetCommandType::DisconnectVote
                | NetCommandType::LoadComplete
                | NetCommandType::TimeoutStart
                | NetCommandType::Wrapper
                | NetCommandType::File
                | NetCommandType::FileAnnounce
                | NetCommandType::FileProgress
                | NetCommandType::DisconnectPlayer
                | NetCommandType::DisconnectFrame
                | NetCommandType::DisconnectScreenOff
                | NetCommandType::FrameResendRequest
        )
    }

    fn is_equal_command_msg(&self, msg1: &NetCommandRef, msg2: &NetCommandRef) -> bool {
        let requires_1 = self.requires_command_id(msg1.command_type);
        let requires_2 = self.requires_command_id(msg2.command_type);
        if requires_1 != requires_2 {
            return false;
        }

        if requires_1 {
            return msg1.player_id == msg2.player_id && msg1.id == msg2.id;
        }

        if msg1.command_type != msg2.command_type {
            return false;
        }
        if msg1.player_id != msg2.player_id {
            return false;
        }

        match msg1.command_type {
            NetCommandType::AckStage1 | NetCommandType::AckStage2 | NetCommandType::AckBoth => {
                match (&msg1.payload, &msg2.payload) {
                    (CommandPayload::Ack(a), CommandPayload::Ack(b)) => {
                        a.command_id == b.command_id
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}
