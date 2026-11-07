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
    pub(super) fn temporary_weapon_fields(&self, name: &str) -> Option<TemporaryWeaponStoreFields> {
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
        self.fire_named_temporary_with_owned_state(source, spec, None, None, effects)
    }

    pub(super) fn fire_named_temporary_with_owned_state(
        &mut self,
        source: ObjectId,
        spec: &crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadEphemeralWeaponSpec,
        state: Option<(i32, u32, u32)>,
        target: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<u32> {
        if let Some(rule) = self.weapon_definitions.as_ref()
            .and_then(|definitions| definitions.get(&spec.weapon_template_name.to_ascii_lowercase()))
            .cloned()
        {
            let object = self.objects.get(&source)?;
            let position = object.get_position();
            let target_position = target.and_then(|id| self.objects.get(&id).map(|o| o.get_position())).unwrap_or(position);
            let bonuses = self.receiving_weapon_bonus_fields(object, Some(&rule));
            let rank = object.experience.level;
            let native_rank = match rank {
                VeterancyLevel::Rookie => gamelogic::common::VeterancyLevel::Regular,
                VeterancyLevel::Veteran => gamelogic::common::VeterancyLevel::Veteran,
                VeterancyLevel::Elite => gamelogic::common::VeterancyLevel::Elite,
                VeterancyLevel::Heroic => gamelogic::common::VeterancyLevel::Heroic,
            };
            let projectile_team_id = object.owner_player_id
                .and_then(|player| self.host_default_team_id(player));
            let projectile_lifecycle = self.projectile_definitions.get(&rule.projectile_name)
                .map(|definition| definition.lifecycle);
            let shot = crate::game_logic::combat::PendingProjectile {
                shooter_id: source,
                shooter_pos: position,
                source_context: Some(crate::game_logic::combat::ProjectileLaunchContext {
                    source_team: object.team,
                    projectile_team_id,
                    source_owner_player_id: object.owner_player_id,
                    source_veterancy: rank,
                    source_orientation: object.get_orientation(),
                    source_velocity: object.movement.velocity,
                    launcher_bonus_mask: object.projectile_launch_bonus_mask(),
                }),
                target_id: target,
                target_pos: Some(target_position),
                self_damage_position: rule.damage_dealt_at_self_position.then_some(position),
                selected_weapon_is_laser: Some(rule.is_laser()),
                damage: rule.primary_damage * bonuses.0,
                speed: rule.weapon_speed,
                speed_unit: crate::game_logic::combat::ProjectileSpeedUnit::DistancePerLogicFrame,
                splash_radius: rule.primary_damage_radius * bonuses.4,
                is_homing: false,
                damage_type: crate::game_logic::host_armor_residual::map_store_damage_type(rule.damage_type),
                death_type: crate::game_logic::host_usa_pilot::HostDeathType::from_store(rule.death_type),
                projectile_object_name: rule.projectile_name.clone(),
                projectile_lifecycle,
                fire_fx_name: rule.get_fire_fx(native_rank).map(|fx| fx.name().to_owned()).unwrap_or_default(),
                fire_ocl_name: rule.get_fire_ocl_name(native_rank).unwrap_or_default().to_owned(),
                detonation_fx_name: rule.get_projectile_detonate_fx(native_rank).map(|fx| fx.name().to_owned()).unwrap_or_default(),
                detonation_ocl_name: rule.get_projectile_detonation_ocl_name(native_rank).unwrap_or_default().to_owned(),
                exhaust_name: rule.get_projectile_exhaust_name(native_rank).unwrap_or_default().to_owned(),
                secondary_damage: rule.secondary_damage * bonuses.0,
                secondary_damage_radius: rule.secondary_damage_radius * bonuses.4,
                shock_wave_amount: rule.shock_wave_amount,
                shock_wave_radius: rule.shock_wave_radius,
                shock_wave_taper_off: rule.shock_wave_taper_off,
                radius_damage_affects: rule.affects_mask.bits(),
                projectile_collides: rule.collide_mask.bits(),
                scatter_radius: rule.scatter_radius,
                scatter_table_offset: None,
                min_weapon_speed: rule.min_weapon_speed,
                scale_weapon_speed: rule.is_scale_weapon_speed,
                attack_range: rule.attack_range,
                min_attack_range: rule.minimum_attack_range,
                historic_weapon_key: rule.name.clone(),
                damage_status_type: Some(rule.damage_status_type.into()),
                historic_bonus_time_frames: rule.historic_bonus_time,
                historic_bonus_count: rule.historic_bonus_count,
                historic_bonus_radius: rule.historic_bonus_radius.max(0.0),
                historic_bonus_weapon: rule.historic_bonus_weapon_name.clone(),
                die_on_detonate: rule.die_on_detonate,
            };
            let object = self.objects.get_mut(&source)?;
            let _ = object.capture_pending_temporary_weapon_visual_dispatch(
                &spec.weapon_template_name,
                state.map_or(0, |weapon| weapon.0),
                state.map_or(0, |weapon| weapon.1),
                state.map(|weapon| weapon.2),
                self.frame, target_position,
            );
            let mut capture = object.take_pending_weapon_visual_capture();
            if let Some(capture) = capture.as_mut() {
                capture.weapon_slot = spec.weapon_slot as u8;
                capture.target_id = target;
            }
            self.deliver_owned_weapon_shot_with_rule_and_effects(spec.weapon_slot as u8, shot, capture, Some(rule), effects);
            return Some(0);
        }
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
