//! Status timers: faerie / repulsor / disable / frenzy / coast / flash / eject / reload / stun / subdual.

use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

impl GameWorldShadow {
    /// Waves 761–765: expire stealth / disable / combat-status timers on one entity.
    pub(super) fn tick_status_stealth(&mut self, eid: EntityId, frame: u32) -> bool {
        let Some(e) = self.world.world_mut().entity_mut(eid) else {
            return false;
        };
        let mut changed = false;
        if e.faerie_fire && e.faerie_fire_until_frame > 0 && frame >= e.faerie_fire_until_frame {
            e.faerie_fire = false;
            e.faerie_fire_until_frame = 0;
            changed = true;
        }
        if e.repulsor && e.repulsor_until_frame > 0 {
            e.repulsor_until_frame = e.repulsor_until_frame.saturating_sub(1);
            if e.repulsor_until_frame == 0 {
                e.repulsor = false;
            }
            changed = true;
        }
        if e.disabled_emp && e.disabled_emp_until_frame > 0 && frame >= e.disabled_emp_until_frame {
            e.disabled_emp = false;
            e.disabled_emp_until_frame = 0;
            changed = true;
        }
        if e.disabled_hacked
            && e.disabled_hacked_until_frame > 0
            && frame >= e.disabled_hacked_until_frame
        {
            e.disabled_hacked = false;
            e.disabled_hacked_until_frame = 0;
            changed = true;
        }
        if e.disabled_paralyzed
            && e.disabled_paralyzed_until_frame > 0
            && frame >= e.disabled_paralyzed_until_frame
        {
            e.disabled_paralyzed = false;
            e.disabled_paralyzed_until_frame = 0;
            changed = true;
        }
        if e.weapon_bonus_frenzy
            && e.weapon_bonus_frenzy_until_frame > 0
            && frame >= e.weapon_bonus_frenzy_until_frame
        {
            e.weapon_bonus_frenzy = false;
            e.weapon_bonus_frenzy_until_frame = 0;
            e.weapon_bonus_frenzy_level = 0;
            changed = true;
        }
        if e.continuous_fire_level > 0 {
            let until = e.continuous_fire_coast_until_frame;
            if until > 0 && frame >= until {
                e.continuous_fire_level = 0;
                e.continuous_fire_consecutive = 0;
                e.continuous_fire_coast_until_frame = 0;
                changed = true;
            }
        }
        if e.selection_flash_remaining > 0 {
            e.selection_flash_remaining = e.selection_flash_remaining.saturating_sub(1);
            changed = true;
        }
        // Wave 762: eject-invulnerable until_frame residual (parity with host tick).
        if e.eject_invulnerable
            && e.eject_invulnerable_until_frame > 0
            && frame >= e.eject_invulnerable_until_frame
        {
            e.eject_invulnerable = false;
            e.eject_invulnerable_until_frame = 0;
            changed = true;
        }
        // Wave 763: force-reload-when-idle residual (C++ FiringTracker).
        if e.frame_to_force_reload > 0 && frame >= e.frame_to_force_reload {
            let needs = e.weapon_clip_size > 0 && e.weapon_ammo < e.weapon_clip_size;
            if needs {
                e.weapon_ammo = e.weapon_clip_size;
            }
            e.frame_to_force_reload = 0;
            changed = true;
        }
        // Wave 764: shock-stun frame countdown residual (physics/rates stay host).
        if e.shock_stun_frames > 0 {
            e.shock_stun_frames = e.shock_stun_frames.saturating_sub(1);
            changed = true;
        }
        // Wave 765: subdual damage heal residual (C++ SubdualDamageHeal*).
        if e.subdual_damage > 0.0 && e.subdual_heal_rate_frames > 0 && e.subdual_heal_amount > 0.0 {
            if e.subdual_heal_countdown > 0 {
                e.subdual_heal_countdown -= 1;
                changed = true;
            } else {
                let was = e.max_health > 0.0 && e.subdual_damage + 1e-3 >= e.max_health;
                e.subdual_damage = (e.subdual_damage - e.subdual_heal_amount).max(0.0);
                e.subdual_heal_countdown = e.subdual_heal_rate_frames;
                let now = e.max_health > 0.0 && e.subdual_damage + 1e-3 >= e.max_health;
                if was && !now {
                    e.disabled_subdued = false;
                }
                changed = true;
            }
        }
        changed
    }
}
