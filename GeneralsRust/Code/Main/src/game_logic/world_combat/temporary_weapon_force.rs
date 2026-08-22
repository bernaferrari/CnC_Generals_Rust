//! C++ `forceFireWeapon` / `createAndFireTempWeapon` for temporary runtimes.
//!
//! `Weapon.cpp:2723-2732` and `Weapon.cpp:1513-1520`. Capture runs before
//! ammo mutation; Object WeaponSet barrel is not advanced.

use super::super::*;
use super::temporary_weapon_status::{
    apply_private_fire_mutation, load_ammo_now, promote_temporary_weapon_status,
    store_fields_for_weapon_name,
};
use crate::game_logic::host_temporary_weapon_behavior::{
    FireWeaponWhenDamagedWeaponRole, TemporaryWeaponRuntimeKey, TemporaryWeaponRuntimeSpec,
    TemporaryWeaponRuntimeState, TemporaryWeaponSlot, TemporaryWeaponStatus,
};

impl GameLogic {
    pub(super) fn force_fire_temporary_runtime(
        &mut self,
        source: ObjectId,
        key: TemporaryWeaponRuntimeKey,
    ) -> u32 {
        let (template_name, barrel) = {
            let object = match self.objects.get_mut(&source) {
                Some(object) => object,
                None => return 0,
            };
            let weapon = match object
                .temporary_weapon_runtime
                .damaged
                .iter_mut()
                .find_map(|runtime| runtime.weapon_mut(key))
            {
                Some(weapon) => weapon,
                None => return 0,
            };
            if promote_temporary_weapon_status(weapon, self.frame)
                != TemporaryWeaponStatus::ReadyToFire
            {
                return 0;
            }
            (weapon.weapon_template_name.clone(), weapon.current_barrel)
        };
        let Some(fields) = store_fields_for_weapon_name(&template_name) else {
            return 0;
        };
        self.force_fire_named_temporary(source, &template_name, barrel, Some(key), fields)
    }

    pub(crate) fn create_and_fire_temp_weapon(
        &mut self,
        source: ObjectId,
        spec: &crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec,
    ) -> Option<u32> {
        let fields = store_fields_for_weapon_name(&spec.weapon_template_name)?;
        let runtime_spec = TemporaryWeaponRuntimeSpec {
            key: TemporaryWeaponRuntimeKey {
                module_source_index: spec.module_source_index,
                role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
            },
            weapon_template_name: spec.weapon_template_name.clone(),
            weapon_slot: TemporaryWeaponSlot::Primary,
        };
        let mut ephemeral = TemporaryWeaponRuntimeState::from_cxx_constructor(
            &runtime_spec,
            fields.defaults,
            self.frame,
        );
        load_ammo_now(&mut ephemeral, fields.defaults, self.frame);
        if promote_temporary_weapon_status(&mut ephemeral, self.frame)
            != TemporaryWeaponStatus::ReadyToFire
        {
            return None;
        }
        Some(self.force_fire_named_temporary(
            source,
            &spec.weapon_template_name,
            ephemeral.current_barrel,
            None,
            fields,
        ))
    }

    fn force_fire_named_temporary(
        &mut self,
        source: ObjectId,
        template_name: &str,
        current_barrel: i32,
        persist: Option<TemporaryWeaponRuntimeKey>,
        fields: super::temporary_weapon_status::TemporaryWeaponStoreFields,
    ) -> u32 {
        let (pos, team, suspend_fx_frame, ammo) = self
            .objects
            .get(&source)
            .map(|object| {
                let persisted = persist.and_then(|key| {
                    object
                        .temporary_weapon_runtime
                        .damaged
                        .iter()
                        .find_map(|runtime| runtime.weapon(key))
                });
                (
                    object.get_position(),
                    object.team,
                    persisted.map(|weapon| weapon.suspend_fx_frame).unwrap_or(0),
                    persisted.map(|weapon| weapon.ammo_in_clip),
                )
            })
            .unwrap_or((Vec3::ZERO, Team::Neutral, 0, None));
        if let Some(object) = self.objects.get_mut(&source) {
            let _ = object.capture_pending_temporary_weapon_visual_dispatch(
                template_name,
                current_barrel,
                suspend_fx_frame,
                ammo,
                self.frame,
                pos,
            );
        }
        let hits = self.apply_instant_hit_splash_at(
            pos,
            fields.primary_damage,
            fields.secondary_damage,
            fields.primary_radius,
            fields.secondary_radius,
            source,
            team,
            source,
            Some(template_name),
        );
        if let Some(key) = persist {
            if let Some(object) = self.objects.get_mut(&source) {
                if let Some(weapon) = object
                    .temporary_weapon_runtime
                    .damaged
                    .iter_mut()
                    .find_map(|runtime| runtime.weapon_mut(key))
                {
                    apply_private_fire_mutation(weapon, fields, self.frame);
                }
            }
        }
        let _ = self.record_accepted_weapon_discharge(source, 0);
        hits
    }
}
