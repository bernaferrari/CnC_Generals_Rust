//! Session-owned idle-worker presentation events, retaining callback order.

use crate::common::{Int, ObjectID};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleWorkerEvent {
    Add { object: ObjectID, player: Int },
    Remove { object: ObjectID, player: Int },
}

#[derive(Debug, Default)]
pub struct IdleWorkerEvents {
    events: Vec<IdleWorkerEvent>,
}

impl IdleWorkerEvents {
    pub fn add(&mut self, object: ObjectID, player: Int) {
        self.events.push(IdleWorkerEvent::Add { object, player });
    }
    pub fn remove(&mut self, object: ObjectID, player: Int) {
        self.events.push(IdleWorkerEvent::Remove { object, player });
    }
    pub fn take(&mut self) -> Vec<IdleWorkerEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opposite_callbacks_keep_exact_order_and_drain_once_without_peer_events() {
        let mut first = IdleWorkerEvents::default();
        let mut peer = IdleWorkerEvents::default();
        first.remove(17, 2);
        first.add(17, 2);
        peer.add(17, 7);
        assert_eq!(
            first.take(),
            vec![
                IdleWorkerEvent::Remove {
                    object: 17,
                    player: 2
                },
                IdleWorkerEvent::Add {
                    object: 17,
                    player: 2
                }
            ]
        );
        assert!(first.take().is_empty());
        assert_eq!(
            peer.take(),
            vec![IdleWorkerEvent::Add {
                object: 17,
                player: 7
            }]
        );
        first.add(17, 2);
        first.remove(17, 2);
        assert_eq!(
            first.take(),
            vec![
                IdleWorkerEvent::Add {
                    object: 17,
                    player: 2
                },
                IdleWorkerEvent::Remove {
                    object: 17,
                    player: 2
                }
            ]
        );
    }
}
