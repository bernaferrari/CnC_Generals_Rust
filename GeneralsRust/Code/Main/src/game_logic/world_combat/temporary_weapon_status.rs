//! C++ `Weapon::getStatus` / `privateFireWeapon` mutation for temporary Weapons.
//!
//! References: `Weapon.cpp:2736-2751` (getStatus), `2617-2669` (post-fire),
//! `1820-1824` (`loadAmmoNow`), `1877-1913` (`reloadWithBonus`).

use crate::game_logic::host_temporary_weapon_behavior::{
    TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT, TemporaryWeaponConstructionDefaults,
    TemporaryWeaponRuntimeState, TemporaryWeaponStatus,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct TemporaryWeaponStoreFields {
    pub defaults: TemporaryWeaponConstructionDefaults,
    pub delay_between_shots: u32,
    pub auto_reloads_clip: bool,
    pub primary_damage: f32,
    pub primary_radius: f32,
    pub secondary_damage: f32,
    pub secondary_radius: f32,
}

pub(super) fn store_fields_for_weapon_name(name: &str) -> Option<TemporaryWeaponStoreFields> {
    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    gamelogic::weapon::with_weapon_store(|store| {
        store.find_weapon_template(name).map(|template| {
            let auto_reloads_clip = template.get_auto_reloads_clip();
            TemporaryWeaponStoreFields {
                defaults: TemporaryWeaponConstructionDefaults {
                    min_target_pitch: template.min_target_pitch,
                    max_target_pitch: template.max_target_pitch,
                    shots_per_barrel: template.shots_per_barrel,
                    suspend_fx_delay: template.suspend_fx_delay,
                    leech_range_weapon: template.leech_range_weapon,
                    clip_size: template.clip_size.max(0) as u32,
                    clip_reload_frames: template.clip_reload_time.max(0) as u32,
                    scatter_target_count: template.scatter_targets.len() as u32,
                },
                delay_between_shots: template.min_delay_between_shots.max(0) as u32,
                auto_reloads_clip,
                primary_damage: template.primary_damage.max(0.0),
                primary_radius: template.primary_damage_radius.max(0.0),
                secondary_damage: template.secondary_damage.max(0.0),
                secondary_radius: template.secondary_damage_radius.max(0.0),
            }
        })
    })
    .ok()
    .flatten()
}

/// C++ `Weapon::getStatus` (`Weapon.cpp:2736-2751`), including the const-cast
/// promotion to READY_TO_FIRE / OUT_OF_AMMO once the delay elapses.
pub(super) fn promote_temporary_weapon_status(
    state: &mut TemporaryWeaponRuntimeState,
    logic_frame: u32,
) -> TemporaryWeaponStatus {
    if logic_frame < state.when_pre_attack_finished {
        state.status = TemporaryWeaponStatus::PreAttack;
        return state.status;
    }
    if logic_frame >= state.when_we_can_fire_again {
        state.status = if state.ammo_in_clip > 0 {
            TemporaryWeaponStatus::ReadyToFire
        } else {
            TemporaryWeaponStatus::OutOfAmmo
        };
    }
    state.status
}

/// C++ `Weapon::loadAmmoNow` (`Weapon.cpp:1820-1824`): instant clip fill.
pub(super) fn load_ammo_now(
    state: &mut TemporaryWeaponRuntimeState,
    defaults: TemporaryWeaponConstructionDefaults,
    logic_frame: u32,
) {
    let mut instant = defaults;
    instant.clip_reload_frames = 0;
    state.reload_ammo_from_cxx(instant, logic_frame);
    let _ = promote_temporary_weapon_status(state, logic_frame);
}

/// C++ `Weapon::privateFireWeapon` ammo/barrel/status tail (`Weapon.cpp:2617-2669`).
pub(super) fn apply_private_fire_mutation(
    state: &mut TemporaryWeaponRuntimeState,
    fields: TemporaryWeaponStoreFields,
    logic_frame: u32,
) {
    state.last_fire_frame = logic_frame;
    state.ammo_in_clip = state.ammo_in_clip.saturating_sub(1);
    if state.max_shot_count != TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT {
        state.max_shot_count = state.max_shot_count.saturating_sub(1);
    }
    state.num_shots_for_current_barrel = state.num_shots_for_current_barrel.saturating_sub(1);
    if state.num_shots_for_current_barrel <= 0 {
        state.current_barrel = state.current_barrel.saturating_add(1);
        state.num_shots_for_current_barrel = fields.defaults.shots_per_barrel;
    }
    if state.ammo_in_clip == 0 {
        if fields.auto_reloads_clip {
            state.reload_ammo_from_cxx(fields.defaults, logic_frame);
        } else {
            state.status = TemporaryWeaponStatus::OutOfAmmo;
            state.when_we_can_fire_again = i32::MAX as u32;
        }
    } else {
        state.status = TemporaryWeaponStatus::BetweenFiringShots;
        state.when_last_reload_started = logic_frame;
        state.when_we_can_fire_again = logic_frame.wrapping_add(fields.delay_between_shots);
    }
}
