//! Source order of an admitted logic frame, including across host reset.

use super::GameCommand;
use std::collections::VecDeque;

/// Internal source entries retain their original relative order. No entry or
/// batch is Clone: a borrowed driver returns its unconsumed owner exactly once.
#[derive(Debug)]
pub enum AdmittedCommand<M> {
    Queued(GameCommand),
    Message(M),
}

#[derive(Debug)]
pub struct AdmittedCommandBatch<M> {
    queued: VecDeque<GameCommand>,
    messages: VecDeque<M>,
}

impl<M> AdmittedCommandBatch<M> {
    pub fn new(queued: VecDeque<GameCommand>, messages: Vec<M>) -> Self {
        Self {
            queued,
            messages: messages.into(),
        }
    }

    pub fn pop_front(&mut self) -> Option<AdmittedCommand<M>> {
        self.queued
            .pop_front()
            .map(AdmittedCommand::Queued)
            .or_else(|| self.messages.pop_front().map(AdmittedCommand::Message))
    }

    pub fn is_empty(&self) -> bool {
        self.queued.is_empty() && self.messages.is_empty()
    }
}
