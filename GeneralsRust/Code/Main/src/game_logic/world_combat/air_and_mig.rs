//! Host combat `impl GameLogic` — `air_and_mig`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn honesty_stealth_jet_missile_projectile_ok(&self) -> bool {
        self.stealth_jet_missiles_spawned > 0
    }

    pub fn apply_stealth_fighter_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_stealth_fighter_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_stealth_fighter_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_bunker_buster::{
            UPGRADE_AMERICA_BUNKER_BUSTERS, should_apply_bunker_buster,
        };
        use crate::game_logic::host_stealth_fighter::{
            STEALTH_FIGHTER_DAMAGE, STEALTH_FIGHTER_DAMAGE_TYPE, STEALTH_FIGHTER_DEATH_TYPE,
            STEALTH_FIGHTER_FIRE_AUDIO, STEALTH_FIGHTER_PRIMARY_RADIUS,
            is_legal_stealth_fighter_target,
            stealth_fighter_damage_at, stealth_jet_scatter_aim,
            stealth_jet_scatter_misses_infantry,
        };

        let (source_team, has_bunker_buster, is_carrier) = {
            if let Some(sid) = source {
                if let Some(obj) = self.objects.get(&sid) {
                    (
                        obj.team,
                        obj.has_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS)
                            || obj.has_upgrade_tag("Upgrade_AmericaBunkerBusters"),
                        obj.authors_stealth_jet_missile_weapon(),
                    )
                } else {
                    (Team::Neutral, false, false)
                }
            } else {
                (Team::Neutral, false, false)
            }
        };

        let intended_is_structure = intended_target
            .and_then(|tid| self.objects.get(&tid))
            .map(|t| t.is_kind_of(KindOf::Structure))
            .unwrap_or(false);
        let bunker_buster_hit =
            should_apply_bunker_buster(has_bunker_buster, is_carrier, intended_is_structure);

        // C++ StealthJetMissileWeapon ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut intended_scatter_miss = false;
        if intended_is_infantry && !bunker_buster_hit {
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
            let (new_impact, scattered) = stealth_jet_scatter_aim(impact, true, seed);
            if scattered {
                self.stealth_jet_scatter_applied =
                    self.stealth_jet_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if stealth_jet_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > STEALTH_FIGHTER_PRIMARY_RADIUS {
                        self.stealth_jet_scatter_misses =
                            self.stealth_jet_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        // Bunker-buster residual owns structure impact (garrison kill + bunker mult).
        // Skip splash so garrisoned occupants are not pre-killed outside contain bookkeeping.
        if bunker_buster_hit {
            if let Some(tid) = intended_target {
                let (kills, _structure_dmg, destroyed) = self
                    .apply_bunker_buster_to_target_with_effects(
                        tid,
                        source_team,
                        STEALTH_FIGHTER_DAMAGE,
                        source,
                        presentation_effects,
                    );
                hits = hits.saturating_add(1).saturating_add(kills);
                if destroyed {
                    any_destroyed = true;
                }
            }
        } else {
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
                    if !is_legal_stealth_fighter_target(
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
                    if is_intended && intended_scatter_miss {
                        return None;
                    }
                    if is_intended || dist <= STEALTH_FIGHTER_PRIMARY_RADIUS {
                        Some((*id, dist, is_intended))
                    } else {
                        None
                    }
                })
                .collect();

            for (id, dist, is_intended) in candidates {
                let dmg = stealth_fighter_damage_at(if is_intended { 0.0 } else { dist });
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
                            STEALTH_FIGHTER_DAMAGE_TYPE,
                        ),
                        crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                            STEALTH_FIGHTER_DEATH_TYPE,
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
        }

        self.stealth_fighter_residual_fires = self.stealth_fighter_residual_fires.saturating_add(1);
        self.stealth_fighter_residual_units_hit =
            self.stealth_fighter_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(STEALTH_FIGHTER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Apply the Uranium Shells tag to a carrier of the authored Overlord gun.
    pub fn apply_overlord_gun_uranium_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_overlord_gun::UPGRADE_CHINA_URANIUM_SHELLS;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !obj.authors_overlord_tank_gun() {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
        self.overlord_gun_residual_uranium_upgrades = self
            .overlord_gun_residual_uranium_upgrades
            .saturating_add(1);
        true
    }

    pub fn honesty_overlord_shell_projectile_ok(&self) -> bool {
        self.overlord_shells_spawned > 0
    }

    pub fn apply_overlord_gun_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_overlord_gun_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_overlord_gun_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_overlord_gun::{
            OVERLORD_DAMAGE_TYPE, OVERLORD_DEATH_TYPE, OVERLORD_FIRE_AUDIO,
            OVERLORD_SECONDARY_RADIUS, has_uranium_shells_upgrade,
            is_legal_overlord_gun_splash_target, overlord_damage_at,
            overlord_scatter_aim, overlord_scatter_misses_infantry,
        };

        let (source_team, has_uranium) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            if !obj.authors_overlord_tank_gun() {
                return (0, false);
            }
            (obj.team, has_uranium_shells_upgrade(&obj.applied_upgrades))
        };

        // C++ OverlordTankGun ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = overlord_scatter_aim(impact, true, seed);
            if scattered {
                self.overlord_scatter_applied = self.overlord_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if overlord_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > OVERLORD_SECONDARY_RADIUS {
                        self.overlord_scatter_misses =
                            self.overlord_scatter_misses.saturating_add(1);
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
                if !is_legal_overlord_gun_splash_target(
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
                if is_intended && intended_is_infantry && dist > OVERLORD_SECONDARY_RADIUS {
                    return None;
                }
                if is_intended || dist <= OVERLORD_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = overlord_damage_at(if is_intended { 0.0 } else { dist }, has_uranium);
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
                        OVERLORD_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        OVERLORD_DEATH_TYPE,
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

        self.overlord_gun_residual_fires = self.overlord_gun_residual_fires.saturating_add(1);
        self.overlord_gun_residual_units_hit =
            self.overlord_gun_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(OVERLORD_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Apply AP Bullets residual tag + rebind Jarmen Kell sniper.
    pub fn apply_jarmen_kell_ap_bullets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_jarmen_kell::{
            JARMEN_KELL_RIFLE, UPGRADE_GLA_AP_BULLETS, has_ap_bullets_upgrade, jarmen_kell_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        // Keyed on the authored weapon, not the template name (hq-2qedt.265).
        if obj.weapon_name_for_slot(0) != Some(JARMEN_KELL_RIFLE) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_BULLETS.to_string());
        let ap = has_ap_bullets_upgrade(&obj.applied_upgrades);
        let mut w = jarmen_kell_weapon(ap);
        if let Some(old) = obj.weapon.as_ref() {
            w.last_fire_time = old.last_fire_time;
        }
        obj.weapon = Some(w);
        self.jarmen_kell_residual_ap_upgrades =
            self.jarmen_kell_residual_ap_upgrades.saturating_add(1);
        true
    }

    /// Refresh Battlemaster weapon residual from current uranium / horde / nationalism flags.
    pub(crate) fn refresh_battlemaster_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::{
            battlemaster_weapon, has_uranium_shells_upgrade,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !obj.authors_battlemaster_tank_gun() || obj.selected_native_weapon_rule(0).is_some() {
            return;
        }
        let uranium = has_uranium_shells_upgrade(&obj.applied_upgrades);
        let weapon = battlemaster_weapon(uranium, false, false);
        super::tanks_and_upgrades::refresh_residual_weapon(&mut obj.weapon, weapon);
    }

    /// Apply Uranium Shells residual (PLAYER_UPGRADE DAMAGE 125%) to a Battlemaster.
    pub fn apply_battlemaster_uranium_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_battlemaster::UPGRADE_CHINA_URANIUM_SHELLS;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !obj.authors_battlemaster_tank_gun() {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
        self.battlemaster_residual_uranium_upgrades = self
            .battlemaster_residual_uranium_upgrades
            .saturating_add(1);
        self.refresh_battlemaster_weapon(object_id);
        true
    }

    pub fn honesty_usa_tank_shell_projectile_ok(&self) -> bool {
        self.usa_tank_shells_spawned > 0 || self.usa_tank_scatter_applied > 0
    }

    /// Residual honesty: USA tank ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_usa_tank_scatter_ok(&self) -> bool {
        self.usa_tank_scatter_applied > 0 || self.usa_tank_scatter_misses > 0
    }

    /// Apply USA Crusader/Paladin tank gun splash residual at impact.
    pub fn apply_usa_tank_gun_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_usa_tank_gun_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_usa_tank_gun_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_usa_tanks::{
            CRUSADER_FIRE_AUDIO, PALADIN_FIRE_AUDIO, USA_TANK_GUN_DAMAGE, USA_TANK_GUN_DAMAGE_TYPE,
            USA_TANK_GUN_DEATH_TYPE, USA_TANK_GUN_PRIMARY_RADIUS, is_crusader_template,
            is_legal_usa_tank_splash_target, is_paladin_template, usa_tank_gun_splash_damage_at,
            usa_tank_scatter_aim, usa_tank_scatter_misses_infantry,
        };

        let (source_team, damage, is_paladin) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            let is_c = is_crusader_template(&obj.template_name);
            let is_p = is_paladin_template(&obj.template_name);
            if !is_c && !is_p {
                return (0, false);
            }
            let dmg = obj
                .weapon
                .as_ref()
                .map(|w| w.damage)
                .unwrap_or(USA_TANK_GUN_DAMAGE);
            (obj.team, dmg, is_p)
        };

        // C++ Crusader/PaladinTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut intended_scatter_miss = false;
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
            let (new_impact, scattered) = usa_tank_scatter_aim(impact, true, seed);
            if scattered {
                self.usa_tank_scatter_applied = self.usa_tank_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if usa_tank_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > USA_TANK_GUN_PRIMARY_RADIUS {
                        self.usa_tank_scatter_misses =
                            self.usa_tank_scatter_misses.saturating_add(1);
                        intended_scatter_miss = true;
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Structure);
                if !is_legal_usa_tank_splash_target(
                    obj.is_alive(),
                    combat_kind,
                    obj.is_kind_of(KindOf::Projectile),
                    false,
                ) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact_xz.0;
                let dz = p.z - impact_xz.1;
                let dist = (dx * dx + dz * dz).sqrt();
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if Some(*id) == intended_target && intended_scatter_miss {
                    return None;
                }
                if dist > USA_TANK_GUN_PRIMARY_RADIUS && Some(*id) != intended_target {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = if Some(id) == intended_target {
                damage
            } else {
                usa_tank_gun_splash_damage_at(dist, damage)
            };
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
                        USA_TANK_GUN_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        USA_TANK_GUN_DEATH_TYPE,
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
        self.usa_tank_residual_units_hit = self.usa_tank_residual_units_hit.saturating_add(hits);
        let _audio = if is_paladin {
            PALADIN_FIRE_AUDIO
        } else {
            CRUSADER_FIRE_AUDIO
        };
        let _ = _audio;
        (hits, any_destroyed)
    }
}
