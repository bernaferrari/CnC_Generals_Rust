// FireWeaponUpdate - Fires a weapon at its own feet as quickly as the weapon allows
// Author: Graham Smallwood, August 2002
// Ported to Rust

use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct FireWeaponUpdateModuleData {
    pub weapon_template: Option<WeaponTemplateId>,
    pub initial_delay_frames: u32,
    pub exclusive_weapon_delay: u32,
}

impl Default for FireWeaponUpdateModuleData {
    fn default() -> Self {
        Self {
            weapon_template: None,
            initial_delay_frames: 0,
            exclusive_weapon_delay: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FireWeaponUpdate {
    thing: ThingId,
    module_data: FireWeaponUpdateModuleData,
    weapon: Option<WeaponId>,
    initial_delay_frame: u32,
}

impl FireWeaponUpdate {
    pub fn new(
        thing: ThingId,
        module_data: FireWeaponUpdateModuleData,
        ctx: &mut GameLogicContext<'_>,
    ) -> Self {
        let weapon = if let Some(tmpl) = module_data.weapon_template {
            if let Some(weapon_store) = ctx.weapon_store.as_mut() {
                let weapon = weapon_store.allocate_new_weapon(tmpl, WeaponSlotType::Primary);
                if let Some(weapon_any) = weapon_store.get_weapon_mut(weapon) {
                    if let Some(weapon_ref) = weapon_any.downcast_mut::<crate::weapon::Weapon>() {
                        if let Err(err) = weapon_ref.load_ammo_now(thing) {
                            log::debug!(
                                "FireWeaponUpdate::new failed to load initial ammo for {:?}: {}",
                                thing,
                                err
                            );
                        }
                    }
                }
                Some(weapon)
            } else {
                None
            }
        } else {
            None
        };

        let initial_delay_frame = ctx.get_frame() + module_data.initial_delay_frames;

        Self {
            thing,
            module_data,
            weapon,
            initial_delay_frame,
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        if ctx.game_logic.get_frame() < self.initial_delay_frame {
            return UpdateSleepTime::None;
        }

        // If my weapon is ready, shoot it
        if self.is_okay_to_fire(ctx) {
            if let (Some(weapon_id), Some(object)) =
                (self.weapon, ctx.game_logic.find_object(self.thing))
            {
                if let Some(weapon_store) = ctx.weapon_store.as_mut() {
                    if let Some(weapon_any) = weapon_store.get_weapon_mut(weapon_id) {
                        if let Some(weapon) = weapon_any.downcast_mut::<crate::weapon::Weapon>() {
                            let pos = object.get_position();
                            if let Err(err) = weapon.force_fire_weapon(object.get_id(), &pos) {
                                log::debug!(
                                    "FireWeaponUpdate::update force_fire_weapon failed for {:?}: {}",
                                    object.get_id(),
                                    err
                                );
                            }
                        }
                    }
                }
            }
        }

        UpdateSleepTime::None
    }

    fn is_okay_to_fire(&self, ctx: &UpdateContext<'_>) -> bool {
        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return false;
        };

        let Some(weapon_id) = self.weapon else {
            return false;
        };

        let Some(weapon_store) = ctx.weapon_store.as_ref() else {
            return false;
        };

        let Some(weapon_any) = weapon_store.get_weapon(weapon_id) else {
            return false;
        };

        let Some(weapon) = weapon_any.downcast_ref::<crate::weapon::Weapon>() else {
            return false;
        };

        // Weapon is reloading
        if weapon.get_status() != crate::weapon::WeaponStatus::ReadyToFire {
            return false;
        }

        // No hitting with a 0% building, cheater
        if object.test_status(ObjectStatus::UnderConstruction) {
            return false;
        }

        // Firing a real weapon suppresses this module
        if self.module_data.exclusive_weapon_delay > 0 {
            let last_shot_frame = object.get_last_shot_fired_frame();
            if ctx.game_logic.get_frame()
                < last_shot_frame + self.module_data.exclusive_weapon_delay
            {
                return false;
            }
        }

        true
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("FireWeaponUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(2);
        let mut weapon = self.weapon;
        xfer.xfer_option_weapon_id("weapon", &mut weapon);
        let mut initial_delay_frame = self.initial_delay_frame;
        xfer_io(
            xfer.xfer_u32(&mut initial_delay_frame),
            "initial_delay_frame",
        );
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("FireWeaponUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer.xfer_option_weapon_id("weapon", &mut self.weapon);
        }
        if version >= 2 {
            xfer_io(
                xfer.xfer_u32(&mut self.initial_delay_frame),
                "initial_delay_frame",
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStatus {
    ReadyToFire,
    Reloading,
    BetweenShots,
}
