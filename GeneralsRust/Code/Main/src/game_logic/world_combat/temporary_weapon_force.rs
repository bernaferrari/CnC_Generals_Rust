//! C++ `forceFireWeapon` / `createAndFireTempWeapon` for temporary runtimes.
//!
//! `Weapon.cpp:2723-2732` and `Weapon.cpp:1513-1520`. Capture runs before
//! ammo mutation; Object WeaponSet barrel is not advanced.

use super::super::*;
use super::temporary_weapon_status::{
    TemporaryWeaponStoreFields, apply_private_fire_mutation, load_ammo_now,
    promote_temporary_weapon_status, store_fields_for_weapon_name,
};
use crate::game_logic::host_temporary_weapon_behavior::{
    FireWeaponWhenDamagedWeaponRole, TemporaryWeaponRuntimeKey, TemporaryWeaponRuntimeSpec,
    TemporaryWeaponRuntimeState, TemporaryWeaponSlot, TemporaryWeaponStatus,
};

impl GameLogic {
    fn temporary_weapon_fields(&self, name: &str) -> Option<TemporaryWeaponStoreFields> {
        match self.weapon_definitions.as_ref() {
            // A receiving catalog miss is a miss, never another world's rule.
            Some(definitions) => definitions
                .get(&name.trim().to_ascii_lowercase())
                .map(|rule| TemporaryWeaponStoreFields::from_template(rule)),
            None => store_fields_for_weapon_name(name),
        }
    }

    pub(super) fn force_fire_temporary_runtime(
        &mut self,
        source: ObjectId,
        key: TemporaryWeaponRuntimeKey,
    ) -> u32 {
        self.force_fire_temporary_runtime_with_effects(source, key, &mut Default::default())
    }

    pub(super) fn force_fire_temporary_runtime_with_effects(
        &mut self,
        source: ObjectId,
        key: TemporaryWeaponRuntimeKey,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
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
        let Some(fields) = self.temporary_weapon_fields(&template_name) else {
            return 0;
        };
        self.force_fire_named_temporary_with_effects(
            source,
            &template_name,
            barrel,
            Some(key),
            fields,
            effects,
        )
    }

    pub(crate) fn create_and_fire_temp_weapon(
        &mut self,
        source: ObjectId,
        spec: &crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec,
    ) -> Option<u32> {
        self.create_and_fire_temp_weapon_with_effects(source, spec, &mut Default::default())
    }

    pub(crate) fn create_and_fire_temp_weapon_with_effects(
        &mut self,
        source: ObjectId,
        spec: &crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<u32> {
        let fields = self.temporary_weapon_fields(&spec.weapon_template_name)?;
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
        Some(self.force_fire_named_temporary_with_effects(
            source,
            &spec.weapon_template_name,
            ephemeral.current_barrel,
            None,
            fields,
            effects,
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
        self.force_fire_named_temporary_with_effects(
            source,
            template_name,
            current_barrel,
            persist,
            fields,
            &mut Default::default(),
        )
    }

    fn force_fire_named_temporary_with_effects(
        &mut self,
        source: ObjectId,
        template_name: &str,
        current_barrel: i32,
        persist: Option<TemporaryWeaponRuntimeKey>,
        fields: super::temporary_weapon_status::TemporaryWeaponStoreFields,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let Some((pos, team, suspend_fx_frame, ammo, identity)) =
            self.objects.get(&source).map(|object| {
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
                    self.live_draw_playback_identity(object),
                )
            })
        else {
            return 0;
        };
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
        let hits = self.apply_instant_hit_splash_at_with_effects(
            pos,
            fields.primary_damage,
            fields.secondary_damage,
            fields.primary_radius,
            fields.secondary_radius,
            source,
            team,
            source,
            Some(template_name),
            effects,
        );
        if !self
            .objects
            .get(&source)
            .is_some_and(|object| self.live_draw_playback_identity(object) == identity)
        {
            return hits;
        }
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
        let _ = self.record_accepted_weapon_discharge_with_effects(source, 0, effects);
        hits
    }
}
