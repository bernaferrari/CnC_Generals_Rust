//! Per-client LAN / Skirmish shell transition flags.
//!
//! C++ kept these as file-static bools on the LAN and Skirmish menus. Rust owns
//! them on WindowManagerOwner so multiple GameClients never share TLS Cells.

use std::cell::Cell;

/// Per-client LAN + Skirmish shell transition flags.
#[derive(Debug, Default)]
pub struct MenuShellFlags {
    pub lan_button_pushed: Cell<bool>,
    pub lan_is_shutting_down: Cell<bool>,
    pub lan_is_initing: Cell<bool>,
    pub lan_slot_list_updates_enabled: Cell<bool>,
    pub skirmish_button_pushed: Cell<bool>,
    pub skirmish_is_shutting_down: Cell<bool>,
    pub skirmish_is_initing: Cell<bool>,
    pub skirmish_slot_list_updates_enabled: Cell<bool>,
}

impl MenuShellFlags {
    pub fn new() -> Self {
        Self {
            lan_slot_list_updates_enabled: Cell::new(true),
            skirmish_slot_list_updates_enabled: Cell::new(true),
            ..Self::default()
        }
    }
}
