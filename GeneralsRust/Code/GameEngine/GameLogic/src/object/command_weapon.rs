//! Switch-weapon / fire-weapon command-button locks (C++ Object::doCommandButton).
//!
//! Called from `object_special_power.rs` so Fix02 can keep routing other
//! command-button arms through `command_buttons.rs`.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// C++ `GUI_COMMAND_SWITCH_WEAPON`: `setWeaponLock(slot, LOCKED_PERMANENTLY)`.
    pub(super) fn lock_switch_weapon_from_command(
        &mut self,
        command_button: &crate::command_button::CommandButton,
    ) {
        self.set_weapon_lock(
            command_button.get_weapon_slot(),
            WeaponLockType::LockedPermanently,
        );
    }

    /// C++ `GUI_COMMAND_FIRE_WEAPON`: `setWeaponLock(slot, LOCKED_TEMPORARILY)`.
    pub(super) fn lock_fire_weapon_from_command(
        &mut self,
        command_button: &crate::command_button::CommandButton,
    ) {
        self.set_weapon_lock(
            command_button.get_weapon_slot(),
            WeaponLockType::LockedTemporarily,
        );
    }
}
