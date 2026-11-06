//! Host combat `impl GameLogic` — `tanks_and_upgrades`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `AIUpdateInterface::evaluateMoraleBonus` (AIUpdate.cpp:4668-4693).
/// Nationalism comes from the player upgrade regardless of horde membership.
/// `AllowedNationalism` vetoes only while the unit is in a horde
/// (`HordeUpdate.cpp` default `m_allowedNationalism = TRUE`).
pub(super) fn nationalism_bonus_from_upgrade(
    has_upgrade: bool,
    in_horde: bool,
    allowed_nationalism: bool,
) -> bool {
    if in_horde && !allowed_nationalism {
        false
    } else {
        has_upgrade
    }
}

/// Host HordeUpdate residual has no per-object AllowedNationalism field;
/// match C++ module-data default (TRUE).
pub(super) const HORDE_DEFAULT_ALLOWED_NATIONALISM: bool = true;

/// The controlling Player's completed Nationalism / Fanaticism upgrades
/// (AIUpdate.cpp `evaluateMoraleBonus`: `player->hasUpgradeComplete`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MoraleUpgrades {
    pub nationalism: bool,
    pub fanaticism: bool,
}

/// C++ `AIUpdateInterface::evaluateMoraleBonus` flag write for live host objects.
pub(crate) fn apply_evaluate_morale_bonus(obj: &mut Object, upgrades: MoraleUpgrades) {
    use crate::game_logic::host_battlemaster::leftover_horde_fanaticism_bonus;
    let nationalism_active = nationalism_bonus_from_upgrade(
        upgrades.nationalism,
        obj.weapon_bonus_horde,
        HORDE_DEFAULT_ALLOWED_NATIONALISM,
    );
    obj.weapon_bonus_nationalism = nationalism_active;
    obj.weapon_bonus_fanaticism =
        leftover_horde_fanaticism_bonus(nationalism_active, upgrades.fanaticism);
}

impl GameLogic {
    /// `player->hasUpgradeComplete(Upgrade_Nationalism / Upgrade_Fanaticism)`
    /// for the Object's controlling Player. Not a per-object tag: units built
    /// after the research see it too.
    pub(crate) fn owner_morale_upgrades(&self, owner: Option<u32>) -> MoraleUpgrades {
        use crate::game_logic::host_battlemaster::{UPGRADE_FANATICISM, UPGRADE_NATIONALISM};
        owner
            .and_then(|id| self.players.get(&id))
            .map(|player| MoraleUpgrades {
                nationalism: player.has_unlocked_upgrade(UPGRADE_NATIONALISM),
                fanaticism: player.has_unlocked_upgrade(UPGRADE_FANATICISM),
            })
            .unwrap_or_default()
    }

    /// C++ `evaluateMoraleBonus` on a live host HordeUpdate object.
    pub(crate) fn evaluate_horde_morale_bonus(&mut self, object_id: ObjectId) {
        let owner = self
            .objects
            .get(&object_id)
            .and_then(|obj| obj.owner_player_id);
        let upgrades = self.owner_morale_upgrades(owner);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            apply_evaluate_morale_bonus(obj, upgrades);
        }
    }

    pub fn honesty_battlemaster_shell_projectile_ok(&self) -> bool {
        self.battlemaster_shells_spawned > 0
    }

    pub fn apply_battlemaster_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_battlemaster_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_battlemaster_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_battlemaster::{
            BATTLE_MASTER_DAMAGE, BATTLE_MASTER_DAMAGE_TYPE, BATTLE_MASTER_DEATH_TYPE,
            BATTLE_MASTER_FIRE_AUDIO, BATTLE_MASTER_SPLASH_RADIUS, battlemaster_scatter_aim,
            battlemaster_scatter_misses_infantry, battlemaster_splash_damage_at,
            is_battlemaster_template, is_legal_battlemaster_splash_target,
        };

        let damage = source
            .and_then(|sid| self.objects.get(&sid))
            .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
            .unwrap_or(BATTLE_MASTER_DAMAGE);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ BattleMasterTankGun ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = battlemaster_scatter_aim(impact, true, seed);
            if scattered {
                self.battlemaster_scatter_applied =
                    self.battlemaster_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if battlemaster_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > BATTLE_MASTER_SPLASH_RADIUS {
                        self.battlemaster_scatter_misses =
                            self.battlemaster_scatter_misses.saturating_add(1);
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
                if !is_legal_battlemaster_splash_target(
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
                if is_intended && intended_is_infantry && dist > BATTLE_MASTER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= BATTLE_MASTER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = battlemaster_splash_damage_at(is_intended, dist, damage);
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
                        BATTLE_MASTER_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        BATTLE_MASTER_DEATH_TYPE,
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

        self.battlemaster_residual_fires = self.battlemaster_residual_fires.saturating_add(1);
        self.battlemaster_residual_units_hit =
            self.battlemaster_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(BATTLE_MASTER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Refresh Red Guard weapon residual from current horde / nationalism flags.
    pub(crate) fn refresh_red_guard_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::leftover_horde_fanaticism_bonus;
        use crate::game_logic::host_red_guard::red_guard_weapon;
        let morale = self.owner_morale_upgrades(
            self.objects
                .get(&object_id)
                .and_then(|obj| obj.owner_player_id),
        );
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !obj.authors_red_guard_machine_gun() {
            return;
        }
        let nationalism = morale.nationalism;
        let in_horde = obj.weapon_bonus_horde;
        let nationalism_active = nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            morale.fanaticism,
        );
        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = red_guard_weapon(in_horde, nationalism_active);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
    }

    /// Refresh Tank Hunter RPG residual from current horde / nationalism flags.
    pub(crate) fn refresh_tank_hunter_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::leftover_horde_fanaticism_bonus;

        use crate::game_logic::host_tank_hunter::tank_hunter_weapon;
        let morale = self.owner_morale_upgrades(
            self.objects
                .get(&object_id)
                .and_then(|obj| obj.owner_player_id),
        );
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !obj.authors_tank_hunter_missile() {
            return;
        }
        let nationalism = morale.nationalism;
        let in_horde = obj.weapon_bonus_horde;
        let nationalism_active = nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            morale.fanaticism,
        );

        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = tank_hunter_weapon(in_horde, nationalism_active);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
    }

    /// Refresh MiniGunner dual-gun residual from continuous fire + horde / nationalism.
    pub(crate) fn refresh_minigunner_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::leftover_horde_fanaticism_bonus;

        use crate::game_logic::host_gattling_tank::GattlingFireLevel;
        use crate::game_logic::host_minigunner::{
            has_chain_guns_upgrade, is_minigunner_template, minigunner_air_weapon,
            minigunner_ground_weapon,
        };
        let morale = self.owner_morale_upgrades(
            self.objects
                .get(&object_id)
                .and_then(|obj| obj.owner_player_id),
        );
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !is_minigunner_template(&obj.template_name) {
            return;
        }
        let nationalism = morale.nationalism;
        let in_horde = obj.weapon_bonus_horde;
        let nationalism_active = nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            morale.fanaticism,
        );

        let chain = has_chain_guns_upgrade(&obj.applied_upgrades);
        let level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let last_sec = obj
            .secondary_weapon
            .as_ref()
            .map(|w| w.last_fire_time)
            .unwrap_or(0.0);
        let mut w = minigunner_ground_weapon(level, chain, in_horde, nationalism_active);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
        let mut s = minigunner_air_weapon(level, chain, in_horde, nationalism_active);
        s.last_fire_time = last_sec;
        obj.secondary_weapon = Some(s);
    }

    /// Apply Chain Guns residual to a MiniGunner (PLAYER_UPGRADE damage residual × 1.25).
    pub fn apply_minigunner_chain_guns_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_minigunner::{
            UPGRADE_CHINA_CHAIN_GUNS, is_minigunner_template,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_minigunner_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        self.minigunner_residual_chain_gun_upgrades = self
            .minigunner_residual_chain_gun_upgrades
            .saturating_add(1);
        self.refresh_minigunner_weapon(object_id);
        true
    }

    /// Recompute China infantry HordeUpdate residual (Red Guard + Tank Hunter + MiniGunner).
    ///
    /// Only HordeUpdate infantry count (C++ getHUI). Radius 30 / Count 5 / RubOff 20.
    pub fn update_china_infantry_horde_status(&mut self) {
        use crate::game_logic::host_battlemaster::{
            LeftoverHordeScanUnit, evaluate_leftover_horde_blob_scan,
            leftover_horde_bounding_sphere_radius, leftover_horde_take_wake,
        };
        use crate::game_logic::host_minigunner::is_minigunner_template;
        use crate::game_logic::host_red_guard::{
            INFANTRY_HORDE_COUNT, INFANTRY_HORDE_RADIUS, INFANTRY_HORDE_RUB_OFF_RADIUS,
            INFANTRY_HORDE_UPDATE_FRAMES, counts_toward_infantry_horde,
            is_china_infantry_horde_unit, leftover_infantry_is_horde_neighbor,
        };

        // .5: authored nationalism weapon kind (1 Red Guard MG, 2 Tank Hunter launcher, 0 other) — hq-2qedt.262.
        #[allow(clippy::type_complexity)]
        let horde_units: Vec<(ObjectId, Team, Option<u32>, LeftoverHordeScanUnit, String, u8)> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if !o.is_alive() || !is_china_infantry_horde_unit(&o.template_name) {
                    return None;
                }
                let p = o.get_position();
                let geom = &(*o.geometry_info());
                Some((
                    *id,
                    o.team,
                    o.owner_player_id,
                    LeftoverHordeScanUnit {
                        x: p.x,
                        y: p.y,
                        z: p.z,
                        sphere_radius: leftover_horde_bounding_sphere_radius(
                            geom.authored,
                            geom.bounding_sphere_radius(),
                            o.selection_radius,
                        ),
                        alive: o.is_alive(),
                    },
                    o.template_name.clone(),
                    if o.authors_red_guard_machine_gun() {
                        1u8
                    } else if o.authors_tank_hunter_missile() {
                        2
                    } else {
                        0
                    },
                ))
            })
            .collect();

        let units: Vec<LeftoverHordeScanUnit> = horde_units.iter().map(|u| u.3).collect();
        let membership = evaluate_leftover_horde_blob_scan(
            &units,
            INFANTRY_HORDE_COUNT,
            INFANTRY_HORDE_RADIUS,
            INFANTRY_HORDE_RUB_OFF_RADIUS,
            |i, j, dist| {
                counts_toward_infantry_horde(
                    horde_units[i].3.alive,
                    horde_units[j].3.alive,
                    self.horde_allies_only(
                        horde_units[i].2,
                        horde_units[i].1,
                        horde_units[j].2,
                        horde_units[j].1,
                    ),
                    leftover_infantry_is_horde_neighbor(&horde_units[j].4),
                    dist,
                    INFANTRY_HORDE_RADIUS,
                )
            },
        );

        let mut red_grants = 0u32;
        let mut th_grants = 0u32;
        let mut mg_grants = 0u32;
        let mut to_refresh_red: Vec<ObjectId> = Vec::new();
        let mut to_refresh_th: Vec<ObjectId> = Vec::new();
        let mut to_refresh_mg: Vec<ObjectId> = Vec::new();
        let draw_icon = self.get_draw_icon_ui();
        let frame = self.frame;

        for (idx, (id, _team, owner, _scan, name, kind)) in horde_units.iter().enumerate() {
            let scanned = membership[idx].in_horde;
            let morale = self.owner_morale_upgrades(*owner);
            if let Some(obj) = self.objects.get_mut(id) {
                let (due, init, last, next) = leftover_horde_take_wake(
                    obj.horde_wake_initialized,
                    true,
                    frame,
                    obj.last_horde_refresh_frame,
                    obj.horde_next_wake_frame,
                    INFANTRY_HORDE_UPDATE_FRAMES,
                );
                obj.horde_wake_initialized = init;
                obj.last_horde_refresh_frame = last;
                obj.horde_next_wake_frame = next;
                if !due {
                    continue;
                }
                let was = obj.weapon_bonus_horde;
                let now_horde = scanned;
                obj.weapon_bonus_horde = now_horde;
                let morale_before = (obj.weapon_bonus_nationalism, obj.weapon_bonus_fanaticism);
                apply_evaluate_morale_bonus(obj, morale);
                let morale_changed =
                    morale_before != (obj.weapon_bonus_nationalism, obj.weapon_bonus_fanaticism);
                obj.apply_horde_terrain_decal(was, now_horde, draw_icon);
                if now_horde && !was {
                    if *kind == 1 {
                        red_grants = red_grants.saturating_add(1);
                    } else if *kind == 2 {
                        th_grants = th_grants.saturating_add(1);
                    } else if is_minigunner_template(name) {
                        mg_grants = mg_grants.saturating_add(1);
                    }
                }
                if now_horde != was || now_horde || morale_changed {
                    if *kind == 1 {
                        to_refresh_red.push(*id);
                    } else if *kind == 2 {
                        to_refresh_th.push(*id);
                    } else if is_minigunner_template(name) {
                        to_refresh_mg.push(*id);
                    }
                }
            }
        }

        self.red_guard_residual_horde_grants = self
            .red_guard_residual_horde_grants
            .saturating_add(red_grants);
        self.tank_hunter_residual_horde_grants = self
            .tank_hunter_residual_horde_grants
            .saturating_add(th_grants);
        self.minigunner_residual_horde_grants = self
            .minigunner_residual_horde_grants
            .saturating_add(mg_grants);

        for id in to_refresh_red {
            self.refresh_red_guard_weapon(id);
        }
        for id in to_refresh_th {
            self.refresh_tank_hunter_weapon(id);
        }
        for id in to_refresh_mg {
            self.refresh_minigunner_weapon(id);
        }
    }

    pub fn honesty_tank_hunter_missile_projectile_ok(&self) -> bool {
        self.tank_hunter_missiles_spawned > 0
    }

    pub fn apply_tank_hunter_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_tank_hunter_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_tank_hunter_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_tank_hunter::{
            TANK_HUNTER_DAMAGE, TANK_HUNTER_DAMAGE_TYPE, TANK_HUNTER_DEATH_TYPE,
            TANK_HUNTER_FIRE_AUDIO, TANK_HUNTER_SPLASH_RADIUS, is_legal_tank_hunter_splash_target,
            tank_hunter_scatter_aim, tank_hunter_scatter_misses_infantry,
            tank_hunter_splash_damage_at,
        };

        let damage = source
            .and_then(|sid| self.objects.get(&sid))
            .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
            .unwrap_or(TANK_HUNTER_DAMAGE);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ TankHunter missile ScatterRadiusVsInfantry residual on instant apply.
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
            let (new_impact, scattered) = tank_hunter_scatter_aim(impact, true, seed);
            if scattered {
                self.tank_hunter_scatter_applied =
                    self.tank_hunter_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if tank_hunter_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > TANK_HUNTER_SPLASH_RADIUS {
                        self.tank_hunter_scatter_misses =
                            self.tank_hunter_scatter_misses.saturating_add(1);
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
                if !is_legal_tank_hunter_splash_target(
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
                if is_intended && intended_is_infantry && dist > TANK_HUNTER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= TANK_HUNTER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = tank_hunter_splash_damage_at(is_intended, dist, damage);
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
                        TANK_HUNTER_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        TANK_HUNTER_DEATH_TYPE,
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

        self.tank_hunter_residual_fires = self.tank_hunter_residual_fires.saturating_add(1);
        self.tank_hunter_residual_units_hit =
            self.tank_hunter_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TANK_HUNTER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// True when the authored primary weapon is `GLARebelMachineGun` (hq-2qedt.243).
    pub(in super::super) fn has_authored_rebel_machine_gun(obj: &Object) -> bool {
        obj.authored_weapon_name_for_slot(0).is_some_and(|w| {
            w.eq_ignore_ascii_case(crate::game_logic::host_gla_rebel::REBEL_MACHINE_GUN)
        })
    }

    /// Refresh GLA Rebel gun residual from current AP Bullets upgrade tag.
    pub(in super::super) fn refresh_rebel_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_gla_rebel::{has_ap_bullets_upgrade, rebel_weapon};
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        // AP Bullets is a WeaponBonusUpgrade on the authored Rebel gun, not a chassis name.
        if !Self::has_authored_rebel_machine_gun(obj) {
            return;
        }
        let ap = has_ap_bullets_upgrade(&obj.applied_upgrades);
        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = rebel_weapon(ap);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
    }

    /// Apply AP Bullets residual tag to a GLA Rebel (damage × 1.25).
    pub fn apply_rebel_ap_bullets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_gla_rebel::UPGRADE_GLA_AP_BULLETS;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !Self::has_authored_rebel_machine_gun(obj) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_BULLETS.to_string());
        self.rebel_residual_ap_upgrades = self.rebel_residual_ap_upgrades.saturating_add(1);
        self.refresh_rebel_weapon(object_id);
        true
    }

}

#[cfg(test)]
mod horde_allies_tests {
    use super::*;
    use crate::game_logic::{Player, ThingTemplate};
    use glam::Vec3;

    /// C++ HordeUpdate.cpp:77-79 AlliesOnly uses getRelationship == ALLIES.
    /// Same-faction enemy China must not share a horde blob.
    #[test]
    fn china_vs_china_infantry_does_not_form_one_horde() {
        let mut logic = GameLogic::new();
        let mut a = Player::new(1, Team::China, "ChinaA", true);
        a.alliance_team = 0;
        let mut b = Player::new(2, Team::China, "ChinaB", false);
        b.alliance_team = 1;
        logic.add_player(a);
        logic.add_player(b);

        let mut tpl = ThingTemplate::new("ChinaInfantryRedguard");
        tpl.add_kind_of(KindOf::Infantry)
            .set_health(120.0)
            .set_cost(0, 0);
        logic.templates.insert("ChinaInfantryRedguard".into(), tpl);

        let mut a_ids = Vec::new();
        let mut b_ids = Vec::new();
        for i in 0..3 {
            let x = i as f32 * 8.0;
            a_ids.push(
                logic
                    .create_object_for_player("ChinaInfantryRedguard", 1, Vec3::new(x, 0.0, 0.0))
                    .expect("a"),
            );
            b_ids.push(
                logic
                    .create_object_for_player(
                        "ChinaInfantryRedguard",
                        2,
                        Vec3::new(x + 4.0, 0.0, 4.0),
                    )
                    .expect("b"),
            );
        }
        logic.update_china_infantry_horde_status();
        for id in a_ids.iter().chain(b_ids.iter()) {
            let obj = logic.host_object(*id).expect("unit");
            assert!(
                !obj.weapon_bonus_horde,
                "3+3 same-faction enemies must not count as one AlliesOnly horde"
            );
        }

        for i in 3..5 {
            let x = i as f32 * 8.0;
            a_ids.push(
                logic
                    .create_object_for_player("ChinaInfantryRedguard", 1, Vec3::new(x, 0.0, 0.0))
                    .expect("a extra"),
            );
        }
        // C++ HordeUpdate.cpp:146-147 initial wake sleeps UpdateRate, then
        // :253 infantry re-evaluates membership when the module wakes. The
        // wake gate is frame-based, so re-scan at the wake frame.
        logic.frame = crate::game_logic::host_red_guard::INFANTRY_HORDE_UPDATE_FRAMES;
        logic.update_china_infantry_horde_status();
        assert!(
            logic.host_object(a_ids[0]).expect("a0").weapon_bonus_horde,
            "five owned allies must form a horde"
        );
        assert!(
            !logic.host_object(b_ids[0]).expect("b0").weapon_bonus_horde,
            "enemy China blob must stay out of the other player's horde"
        );
    }
}
