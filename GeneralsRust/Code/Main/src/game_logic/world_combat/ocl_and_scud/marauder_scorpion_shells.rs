//! Marauder and Scorpion shell projectiles.

use super::*;

impl GameLogic {
    /// Apply residual salvage fire-rate tier to a Marauder (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D turret subobject swap.
    pub fn apply_marauder_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_marauder::MarauderWeaponTier,
    ) -> bool {
        use crate::game_logic::host_marauder::{
            delay_frames_to_reload_secs, is_marauder_weapon_name, marauder_weapon_for_tier,
            marauder_weapon_name_for_tier, marauder_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        // Salvage tier swaps the authored WeaponSet slot weapon; gate on it.
        if !obj
            .authored_weapon_name_for_slot(0)
            .is_some_and(is_marauder_weapon_name)
        {
            return false;
        }

        let name = marauder_weapon_name_for_tier(tier);
        let (dmg, range, delay, _splash, speed) = marauder_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| marauder_weapon_for_tier(tier));
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = 0.0;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.projectile_speed = speed;
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        let _ = obj.replace_weapon_set_slot(0, Some(weapon));

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_marauder::MarauderWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Base => {}
        }

        self.marauder_residual_weapon_upgrades =
            self.marauder_residual_weapon_upgrades.saturating_add(1);
        true
    }

    pub fn honesty_marauder_shell_projectile_ok(&self) -> bool {
        self.marauder_shells_spawned > 0
    }

    pub fn apply_marauder_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_marauder_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_marauder_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_marauder::{
            MARAUDER_DAMAGE_TYPE, MARAUDER_DEATH_TYPE, MARAUDER_FIRE_AUDIO, MARAUDER_SPLASH_RADIUS,
            is_legal_marauder_splash_target, marauder_scatter_aim,
            marauder_scatter_misses_infantry, marauder_splash_damage_at,
        };

        // Fire-rate tier residual is encoded on the weapon; damage is constant across tiers.
        let _tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::marauder_tier_from_object);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ MarauderTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = marauder_scatter_aim(impact, true, seed);
            if scattered {
                self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if marauder_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_marauder_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended && intended_is_infantry && dist > MARAUDER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= MARAUDER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = marauder_splash_damage_at(is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        MARAUDER_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        MARAUDER_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.marauder_residual_fires = self.marauder_residual_fires.saturating_add(1);
        self.marauder_residual_units_hit = self.marauder_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(MARAUDER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Apply Scorpion salvage gun tier residual (primary damage 20 → 25).
    pub fn apply_scorpion_salvage_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_scorpion::ScorpionSalvageTier,
    ) -> bool {
        use crate::game_logic::host_scorpion::{
            ScorpionSalvageTier, has_ap_rockets_upgrade, has_scorpion_rocket_upgrade,
            scorpion_gun_weapon, scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !authored_scorpion_gun(obj) {
            return false;
        }

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            ScorpionSalvageTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            ScorpionSalvageTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            ScorpionSalvageTier::Base => {}
        }

        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut gun = scorpion_gun_weapon(tier);
        gun.last_fire_time = last_fire;
        let _ = obj.replace_weapon_set_slot(0, Some(gun));

        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(ap, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            // Salvage changes the selected C++ WeaponSet.  Even when AP
            // Rockets also contributes its in-place stats, this concrete
            // secondary Weapon instance was rebuilt and starts with a fresh
            // barrel cursor.
            let _ = obj.replace_weapon_set_slot(1, Some(missile));
        }

        self.scorpion_residual_salvage_upgrades =
            self.scorpion_residual_salvage_upgrades.saturating_add(1);
        true
    }

    /// Equip Scorpion Rocket secondary residual (Upgrade_GLAScorpionRocket).
    pub fn apply_scorpion_rocket_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            UPGRADE_GLA_SCORPION_ROCKET, has_ap_rockets_upgrade, salvage_tier_from_upgrades,
            scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !authored_scorpion_gun(obj) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_SCORPION_ROCKET.to_string());
        let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
        let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
        let _ = obj.replace_weapon_set_slot(
            1,
            Some(scorpion_missile_weapon(ap, tier.dual_missile_clip())),
        );
        self.scorpion_residual_rocket_upgrades =
            self.scorpion_residual_rocket_upgrades.saturating_add(1);
        true
    }

    /// Apply AP Rockets residual damage mult to Scorpion missile secondary.
    pub fn apply_scorpion_ap_rockets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            UPGRADE_GLA_AP_ROCKETS, has_scorpion_rocket_upgrade, salvage_tier_from_upgrades,
            scorpion_missile_weapon,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !authored_scorpion_gun(obj) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(true, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            // AP Rockets is a C++ WeaponBonus upgrade layered onto the
            // selected Scorpion Rocket WeaponSet; preserve its live barrel
            // cursor while refreshing the boosted stats.
            obj.secondary_weapon = Some(missile);
        }
        true
    }

}
