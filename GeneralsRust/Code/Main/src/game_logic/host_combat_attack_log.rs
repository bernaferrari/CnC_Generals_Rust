//! Frame-local host combat attack residual for GameWorld SetCombatAttack parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCombatAttackEvent {
    pub object: ObjectId,
    pub pre_attack_target_host: u32,
    pub pre_attack_ready_at: f32,
    pub consecutive_shots_at_target: u32,
    pub max_shots_to_fire: i32,
    pub attack_substate_ordinal: u8,
    pub approach_timestamp: u32,
    pub continuous_fire_victim: u32,
    pub maintain_pos_valid: bool,
    pub maintain_pos: Option<[f32; 3]>,
    pub temporary_move_frames: u32,
    pub group_speed_factor: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostCombatAttackEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    pre_attack_target_host: u32,
    pre_attack_ready_at: f32,
    consecutive_shots_at_target: u32,
    max_shots_to_fire: i32,
    attack_substate_ordinal: u8,
    approach_timestamp: u32,
    continuous_fire_victim: u32,
    maintain_pos_valid: bool,
    maintain_pos: Option<[f32; 3]>,
    temporary_move_frames: u32,
    group_speed_factor: f32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostCombatAttackEvent {
            object,
            pre_attack_target_host,
            pre_attack_ready_at,
            consecutive_shots_at_target,
            max_shots_to_fire,
            attack_substate_ordinal,
            approach_timestamp,
            continuous_fire_victim,
            maintain_pos_valid,
            maintain_pos,
            temporary_move_frames,
            group_speed_factor,
        });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostCombatAttackEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostCombatAttackEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(
    next: Vec<HostCombatAttackEvent>,
) -> Vec<HostCombatAttackEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
