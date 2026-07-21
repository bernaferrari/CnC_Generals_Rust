//! Frame-local host AI request residual for GameWorld SetAiRequest parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostAiRequestEvent {
    pub object: ObjectId,
    pub requested_victim_host: u32,
    pub requested_destination: Option<[f32; 3]>,
    pub prev_victim_pos: Option<[f32; 3]>,
    pub crate_created_host: u32,
    pub guard_retaliate_victim_host: u32,
    pub guard_retaliate_anchor: Option<[f32; 3]>,
    pub path_timestamp: u32,
    pub disguise_pending_template: String,
    pub disguise_pending_team_ordinal: u8,
    pub weapon_crate_upgrade: u8,
    pub armor_crate_upgrade: u8,
    pub selection_flash_remaining: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostAiRequestEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    requested_victim_host: u32,
    requested_destination: Option<[f32; 3]>,
    prev_victim_pos: Option<[f32; 3]>,
    crate_created_host: u32,
    guard_retaliate_victim_host: u32,
    guard_retaliate_anchor: Option<[f32; 3]>,
    path_timestamp: u32,
    disguise_pending_template: String,
    disguise_pending_team_ordinal: u8,
    weapon_crate_upgrade: u8,
    armor_crate_upgrade: u8,
    selection_flash_remaining: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAiRequestEvent {
            object,
            requested_victim_host,
            requested_destination,
            prev_victim_pos,
            crate_created_host,
            guard_retaliate_victim_host,
            guard_retaliate_anchor,
            path_timestamp,
            disguise_pending_template,
            disguise_pending_team_ordinal,
            weapon_crate_upgrade,
            armor_crate_upgrade,
            selection_flash_remaining,
        });
    });
}

pub fn drain() -> Vec<HostAiRequestEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
