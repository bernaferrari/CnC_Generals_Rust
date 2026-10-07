//! Turret configuration, linked controls, and firing eligibility.

use super::ai_data::UnitAiData;
use super::imports::*;

impl UnitAiData {
    pub(super) fn friend_get_turret_sync(&self) -> TurretType {
        self.turret_sync_flag
    }
    pub(super) fn friend_set_turret_sync(&mut self, turret: TurretType) {
        self.turret_sync_flag = turret;
    }
    pub(super) fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if machine.turret().is_owners_cur_weapon_on_turret() {
                return TurretType::Primary;
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if machine.turret().is_owners_cur_weapon_on_turret() {
                return TurretType::Secondary;
            }
        }
        TurretType::Invalid
    }
    pub(super) fn get_turret_turn_rate(&self, turret: TurretType) -> f32 {
        let machine = match turret {
            TurretType::Primary => self.turret_primary_machine.as_ref(),
            TurretType::Secondary => self.turret_secondary_machine.as_ref(),
            TurretType::Invalid => None,
        };
        machine
            .map(|machine| machine.turret().get_turn_rate())
            .unwrap_or(0.0)
    }
    pub(super) fn get_which_turret_for_weapon_slot(&self, slot: WeaponSlotType) -> TurretType {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if machine.turret().is_weapon_slot_on_turret(slot) {
                return TurretType::Primary;
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if machine.turret().is_weapon_slot_on_turret(slot) {
                return TurretType::Secondary;
            }
        }
        TurretType::Invalid
    }
    pub(super) fn set_turret_enabled(&mut self, turret: TurretType, enabled: bool) {
        match turret {
            TurretType::Primary => {
                self.turret_primary_enabled = enabled;
                if let Some(machine) = self.turret_primary_machine.as_mut() {
                    machine.turret_mut().set_turret_enabled(enabled);
                }
                if self.turrets_linked {
                    self.turret_secondary_enabled = enabled;
                    if let Some(machine) = self.turret_secondary_machine.as_mut() {
                        machine.turret_mut().set_turret_enabled(enabled);
                    }
                }
            }
            TurretType::Secondary => {
                self.turret_secondary_enabled = enabled;
                if let Some(machine) = self.turret_secondary_machine.as_mut() {
                    machine.turret_mut().set_turret_enabled(enabled);
                }
                if self.turrets_linked {
                    self.turret_primary_enabled = enabled;
                    if let Some(machine) = self.turret_primary_machine.as_mut() {
                        machine.turret_mut().set_turret_enabled(enabled);
                    }
                }
            }
            TurretType::Invalid => {}
        }
    }
    pub(super) fn recenter_turret(&mut self, turret: TurretType) {
        match turret {
            TurretType::Primary => {
                self.turret_primary_natural = true;
                if let Some(machine) = self.turret_primary_machine.as_mut() {
                    machine.turret_mut().recenter_turret();
                }
                if self.turrets_linked {
                    self.turret_secondary_natural = true;
                    if let Some(machine) = self.turret_secondary_machine.as_mut() {
                        machine.turret_mut().recenter_turret();
                    }
                }
            }
            TurretType::Secondary => {
                self.turret_secondary_natural = true;
                if let Some(machine) = self.turret_secondary_machine.as_mut() {
                    machine.turret_mut().recenter_turret();
                }
                if self.turrets_linked {
                    self.turret_primary_natural = true;
                    if let Some(machine) = self.turret_primary_machine.as_mut() {
                        machine.turret_mut().recenter_turret();
                    }
                }
            }
            TurretType::Invalid => {}
        }
    }
    pub(super) fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .map(|machine| machine.turret().is_turret_in_natural_position())
                .unwrap_or(false),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .map(|machine| machine.turret().is_turret_in_natural_position())
                .unwrap_or(false),
            TurretType::Invalid => false,
        }
    }
    pub(super) fn is_turret_enabled(&self, turret: TurretType) -> bool {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .map(|machine| machine.turret().is_turret_enabled())
                .unwrap_or(false),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .map(|machine| machine.turret().is_turret_enabled())
                .unwrap_or(false),
            TurretType::Invalid => false,
        }
    }
    pub(super) fn get_turret_rot_and_pitch(&self, turret: TurretType) -> Option<(Real, Real)> {
        match turret {
            TurretType::Primary => self.turret_primary_machine.as_ref().map(|machine| {
                let turret = machine.turret();
                (turret.get_turret_angle(), turret.get_turret_pitch())
            }),
            TurretType::Secondary => self.turret_secondary_machine.as_ref().map(|machine| {
                let turret = machine.turret();
                (turret.get_turret_angle(), turret.get_turret_pitch())
            }),
            TurretType::Invalid => None,
        }
    }
    pub(super) fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        slot: WeaponSlotType,
        target: ObjectID,
    ) -> bool {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            let turret = machine.turret();
            if turret.is_weapon_slot_on_turret(slot) && turret.is_trying_to_aim_at_target(target) {
                return true;
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            let turret = machine.turret();
            if turret.is_weapon_slot_on_turret(slot) && turret.is_trying_to_aim_at_target(target) {
                return true;
            }
        }
        false
    }
    pub(super) fn are_turrets_linked(&self) -> Bool {
        self.turrets_linked
    }
    pub(super) fn is_weapon_slot_ok_to_fire(&self, _wslot: WeaponSlotType) -> Bool {
        if self.turrets_linked {
            return true;
        }

        let has_primary = self.turret_primary_machine.is_some();
        let has_secondary = self.turret_secondary_machine.is_some();
        if !has_primary && !has_secondary {
            return true;
        }

        match _wslot {
            WeaponSlotType::Primary => has_primary && self.turret_primary_enabled,
            WeaponSlotType::Secondary => has_secondary && self.turret_secondary_enabled,
            WeaponSlotType::Tertiary => !has_primary && !has_secondary,
        }
    }
}
