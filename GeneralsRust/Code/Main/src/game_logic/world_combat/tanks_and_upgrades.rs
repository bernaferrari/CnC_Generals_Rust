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

/// C++ `AIUpdateInterface::evaluateMoraleBonus` flag write for live host objects.
pub(crate) fn apply_evaluate_morale_bonus(obj: &mut Object) {
    use crate::game_logic::host_battlemaster::{
        has_fanaticism_upgrade, has_nationalism_upgrade, leftover_horde_fanaticism_bonus,
    };
    let nationalism_active = nationalism_bonus_from_upgrade(
        has_nationalism_upgrade(&obj.applied_upgrades),
        obj.weapon_bonus_horde,
        HORDE_DEFAULT_ALLOWED_NATIONALISM,
    );
    obj.weapon_bonus_nationalism = nationalism_active;
    obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
        nationalism_active,
        has_fanaticism_upgrade(&obj.applied_upgrades),
    );
    obj.record_host_weapon_bonus();
}

impl GameLogic {
    /// C++ `evaluateMoraleBonus` on a live host HordeUpdate object.
    pub(crate) fn evaluate_horde_morale_bonus(&mut self, object_id: ObjectId) {
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        apply_evaluate_morale_bonus(obj);
    }

    /// C++ BattleMasterTankShell DumbProjectile residual.
    pub fn spawn_battlemaster_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_battlemaster::{
            BATTLE_MASTER_PROJECTILE, BM_SHELL_MAX_HEALTH, battlemaster_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(BATTLE_MASTER_PROJECTILE) {
            let mut t = ThingTemplate::new(BATTLE_MASTER_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(BM_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(BATTLE_MASTER_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on BattleMasterTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_battlemaster::battlemaster_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.battlemaster_scatter_applied = self.battlemaster_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_battlemaster::battlemaster_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_battlemaster::BATTLE_MASTER_SPLASH_RADIUS {
                        self.battlemaster_scatter_misses =
                            self.battlemaster_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(BATTLE_MASTER_PROJECTILE, team, start)?;
        let frames = battlemaster_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.battlemaster_shell_projectile = true;
            o.battlemaster_shell_from = Some([start.x, start.y, start.z]);
            o.battlemaster_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.battlemaster_shell_launch_frame = Some(self.frame);
            o.battlemaster_shell_flight_frames = frames;
            o.battlemaster_shell_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
            o.health.maximum = BM_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, BM_SHELL_MAX_HEALTH);
        }
        self.battlemaster_shells_spawned = self.battlemaster_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_battlemaster_shell_projectiles(&mut self) {
        use crate::game_logic::host_battlemaster::battlemaster_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.battlemaster_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .battlemaster_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .battlemaster_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.battlemaster_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.battlemaster_shell_launch_frame.unwrap_or(frame),
                    o.battlemaster_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = battlemaster_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.battlemaster_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_battlemaster_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
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
            .iter()
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
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    BATTLE_MASTER_DAMAGE_TYPE,
                    BATTLE_MASTER_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.battlemaster_residual_fires = self.battlemaster_residual_fires.saturating_add(1);
        self.battlemaster_residual_units_hit =
            self.battlemaster_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(BATTLE_MASTER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_battlemaster_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Refresh Red Guard weapon residual from current horde / nationalism flags.
    pub(crate) fn refresh_red_guard_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::{
            has_fanaticism_upgrade, has_nationalism_upgrade, leftover_horde_fanaticism_bonus,
        };
        use crate::game_logic::host_red_guard::{is_red_guard_template, red_guard_weapon};
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !is_red_guard_template(&obj.template_name) {
            return;
        }
        let nationalism = has_nationalism_upgrade(&obj.applied_upgrades);
        let in_horde = obj.weapon_bonus_horde;
        let nationalism_active = nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            has_fanaticism_upgrade(&obj.applied_upgrades),
        );
        obj.record_host_weapon_bonus();
        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = red_guard_weapon(in_horde, nationalism_active);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
    }

    /// Refresh Tank Hunter RPG residual from current horde / nationalism flags.
    pub(crate) fn refresh_tank_hunter_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::{
            has_fanaticism_upgrade, has_nationalism_upgrade, leftover_horde_fanaticism_bonus,
        };

        use crate::game_logic::host_tank_hunter::{is_tank_hunter_template, tank_hunter_weapon};
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !is_tank_hunter_template(&obj.template_name) {
            return;
        }
        let nationalism = has_nationalism_upgrade(&obj.applied_upgrades);
        let in_horde = obj.weapon_bonus_horde;
        let nationalism_active = nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            has_fanaticism_upgrade(&obj.applied_upgrades),
        );
        obj.record_host_weapon_bonus();

        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut w = tank_hunter_weapon(in_horde, nationalism_active);
        w.last_fire_time = last_fire;
        obj.weapon = Some(w);
    }

    /// Refresh MiniGunner dual-gun residual from continuous fire + horde / nationalism.
    pub(crate) fn refresh_minigunner_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_battlemaster::{
            has_fanaticism_upgrade, has_nationalism_upgrade, leftover_horde_fanaticism_bonus,
        };

        use crate::game_logic::host_gattling_tank::GattlingFireLevel;
        use crate::game_logic::host_minigunner::{
            has_chain_guns_upgrade, is_minigunner_template, minigunner_air_weapon,
            minigunner_ground_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !is_minigunner_template(&obj.template_name) {
            return;
        }
        let nationalism = has_nationalism_upgrade(&obj.applied_upgrades);
        let in_horde = obj.weapon_bonus_horde;
        let nationalism_active = nationalism_bonus_from_upgrade(
            nationalism,
            in_horde,
            HORDE_DEFAULT_ALLOWED_NATIONALISM,
        );
        obj.weapon_bonus_nationalism = nationalism_active;
        obj.weapon_bonus_fanaticism = leftover_horde_fanaticism_bonus(
            nationalism_active,
            has_fanaticism_upgrade(&obj.applied_upgrades),
        );
        obj.record_host_weapon_bonus();

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

    /// Apply Nationalism residual tag to a Red Guard (ROF stacks with horde when active).
    pub fn apply_red_guard_nationalism_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
        use crate::game_logic::host_red_guard::is_red_guard_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_red_guard_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades.insert(UPGRADE_NATIONALISM.to_string());
        self.red_guard_residual_nationalism_upgrades = self
            .red_guard_residual_nationalism_upgrades
            .saturating_add(1);
        self.refresh_red_guard_weapon(object_id);
        true
    }

    /// Apply Nationalism residual tag to a Tank Hunter (ROF stacks with horde when active).
    pub fn apply_tank_hunter_nationalism_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
        use crate::game_logic::host_tank_hunter::is_tank_hunter_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_tank_hunter_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades.insert(UPGRADE_NATIONALISM.to_string());
        self.tank_hunter_residual_nationalism_upgrades = self
            .tank_hunter_residual_nationalism_upgrades
            .saturating_add(1);
        self.refresh_tank_hunter_weapon(object_id);
        true
    }

    /// Apply Nationalism residual tag to a MiniGunner (ROF stacks with horde when active).
    pub fn apply_minigunner_nationalism_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
        use crate::game_logic::host_minigunner::is_minigunner_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_minigunner_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades.insert(UPGRADE_NATIONALISM.to_string());
        self.minigunner_residual_nationalism_upgrades = self
            .minigunner_residual_nationalism_upgrades
            .saturating_add(1);
        self.refresh_minigunner_weapon(object_id);
        true
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
            leftover_horde_bounding_sphere_radius, leftover_horde_draw_icon_ui,
            leftover_horde_take_wake,
        };
        use crate::game_logic::host_minigunner::is_minigunner_template;
        use crate::game_logic::host_red_guard::{
            INFANTRY_HORDE_COUNT, INFANTRY_HORDE_RADIUS, INFANTRY_HORDE_RUB_OFF_RADIUS,
            INFANTRY_HORDE_UPDATE_FRAMES, counts_toward_infantry_horde,
            is_china_infantry_horde_unit, is_red_guard_template,
            leftover_infantry_is_horde_neighbor,
        };
        use crate::game_logic::host_tank_hunter::is_tank_hunter_template;

        let horde_units: Vec<(ObjectId, Team, Option<u32>, LeftoverHordeScanUnit, String)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.is_alive() || !is_china_infantry_horde_unit(&o.template_name) {
                    return None;
                }
                let p = o.get_position();
                let geom = &o.thing.template.geometry_info;
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
        let draw_icon = leftover_horde_draw_icon_ui();
        let frame = self.frame;

        for (idx, (id, _team, _owner, _scan, name)) in horde_units.iter().enumerate() {
            let scanned = membership[idx].in_horde;
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
                apply_evaluate_morale_bonus(obj);
                obj.apply_horde_terrain_decal(was, now_horde, draw_icon);
                if now_horde && !was {
                    if is_red_guard_template(name) {
                        red_grants = red_grants.saturating_add(1);
                    } else if is_tank_hunter_template(name) {
                        th_grants = th_grants.saturating_add(1);
                    } else if is_minigunner_template(name) {
                        mg_grants = mg_grants.saturating_add(1);
                    }
                }
                if now_horde != was || now_horde {
                    if is_red_guard_template(name) {
                        to_refresh_red.push(*id);
                    } else if is_tank_hunter_template(name) {
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

    /// Apply Red Guard residual fire: bayonet one-shot vs close infantry, else gun damage.
    pub(in super::super) fn apply_red_guard_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_red_guard::{
            BAYONET_DAMAGE, BAYONET_DAMAGE_TYPE, BAYONET_FIRE_AUDIO, REDGUARD_DAMAGE,
            REDGUARD_DAMAGE_TYPE, REDGUARD_DEATH_TYPE, REDGUARD_FIRE_AUDIO, distance_2d,
            should_apply_bayonet_residual,
        };

        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);
        let source_pos = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.get_position()))
            .unwrap_or(impact);

        let Some(target_id) = intended_target else {
            return (0, false);
        };
        let Some(target) = self.objects.get(&target_id) else {
            return (0, false);
        };
        if !target.is_alive() {
            return (0, false);
        }
        let target_pos = target.get_position();
        let dist = distance_2d(source_pos.x, source_pos.z, target_pos.x, target_pos.z);
        let target_is_infantry = target.is_kind_of(KindOf::Infantry);
        let bayonet = should_apply_bayonet_residual(true, target_is_infantry, true, dist);
        let damage = if bayonet {
            BAYONET_DAMAGE
        } else {
            source
                .and_then(|sid| self.objects.get(&sid))
                .and_then(|o| o.weapon.as_ref().map(|w| w.damage))
                .unwrap_or(REDGUARD_DAMAGE)
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        if let Some(obj) = self.objects.get_mut(&target_id) {
            let (dt_name, death_name) = if bayonet {
                (BAYONET_DAMAGE_TYPE, REDGUARD_DEATH_TYPE)
            } else {
                (REDGUARD_DAMAGE_TYPE, REDGUARD_DEATH_TYPE)
            };
            let destroyed =
                obj.take_damage_from_immediate_residual(damage, source, dt_name, death_name);
            hits = 1;
            if destroyed {
                any_destroyed = true;
                self.mark_object_for_destruction(target_id, Some(source_team));
            }
        }

        self.red_guard_residual_fires = self.red_guard_residual_fires.saturating_add(1);
        if bayonet {
            self.red_guard_residual_bayonet_kills =
                self.red_guard_residual_bayonet_kills.saturating_add(1);
        }

        let audio = if bayonet {
            BAYONET_FIRE_AUDIO
        } else {
            REDGUARD_FIRE_AUDIO
        };
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_position(target_pos)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(target_pos),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Tank Hunter residual RPG fire (primary on intended + small splash radius).
    /// C++ TankHunterMissile ProjectileObject residual.
    pub fn spawn_tank_hunter_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_tank_hunter::{
            TANK_HUNTER_MISSILE_FUEL_FRAMES, TANK_HUNTER_MISSILE_INITIAL_VELOCITY,
            TANK_HUNTER_MISSILE_MAX_HEALTH, TANK_HUNTER_PROJECTILE, TANK_HUNTER_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(TANK_HUNTER_PROJECTILE) {
            let mut t = ThingTemplate::new(TANK_HUNTER_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(TANK_HUNTER_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(TANK_HUNTER_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on TankHunter missile vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_tank_hunter::tank_hunter_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.tank_hunter_scatter_applied = self.tank_hunter_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_tank_hunter::tank_hunter_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_tank_hunter::TANK_HUNTER_SPLASH_RADIUS {
                        self.tank_hunter_scatter_misses =
                            self.tank_hunter_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 6.0;
        let pid = self.create_object(TANK_HUNTER_PROJECTILE, team, start)?;
        let launch = TANK_HUNTER_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.tank_hunter_missile_projectile = true;
            o.tank_hunter_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.tank_hunter_missile_intended = intended.map(|id| id.0);
            o.tank_hunter_missile_travelled = 0.0;
            o.tank_hunter_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(TANK_HUNTER_MISSILE_FUEL_FRAMES));
            o.note_producer(source_id);
            o.health.maximum = TANK_HUNTER_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, TANK_HUNTER_MISSILE_MAX_HEALTH);
            o.movement.velocity = dir * launch;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        self.tank_hunter_missiles_spawned = self.tank_hunter_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_tank_hunter_missile_projectiles(&mut self) {
        use crate::game_logic::host_tank_hunter::{
            TANK_HUNTER_MISSILE_INITIAL_VELOCITY, TANK_HUNTER_MISSILE_TURN_DISTANCE,
            TANK_HUNTER_PROJECTILE_SPEED,
        };
        let frame = self.frame;
        let launch = TANK_HUNTER_MISSILE_INITIAL_VELOCITY / 30.0;
        let cruise = TANK_HUNTER_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.tank_hunter_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, aim, pos, travelled, fuel_done) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .tank_hunter_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.tank_hunter_missile_intended.map(ObjectId);
                let fuel_done = o
                    .tank_hunter_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    o.tank_hunter_missile_travelled,
                    fuel_done,
                )
            };
            let aim = intended
                .and_then(|tid| {
                    self.objects
                        .get(&tid)
                        .filter(|t| t.is_alive())
                        .map(|t| t.get_position())
                })
                .unwrap_or(aim);
            let speed = if travelled < TANK_HUNTER_MISSILE_TURN_DISTANCE {
                launch
            } else {
                cruise
            };
            let to_aim = aim - pos;
            let vel = if to_aim.length() > 0.001 {
                to_aim.normalize() * speed
            } else {
                glam::Vec3::new(0.0, -speed, 0.0)
            };
            let step = vel.length().max(speed);
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(pos + vel);
                o.tank_hunter_missile_travelled += step;
                o.tank_hunter_missile_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let new_pos = pos + vel;
            let near = (aim - new_pos).length() < 6.0;
            if fuel_done || near {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.tank_hunter_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_tank_hunter_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
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
            .iter()
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
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    TANK_HUNTER_DAMAGE_TYPE,
                    TANK_HUNTER_DEATH_TYPE,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.tank_hunter_residual_fires = self.tank_hunter_residual_fires.saturating_add(1);
        self.tank_hunter_residual_units_hit =
            self.tank_hunter_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TANK_HUNTER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Refresh GLA Rebel gun residual from current AP Bullets upgrade tag.
    pub(in super::super) fn refresh_rebel_weapon(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_gla_rebel::{
            has_ap_bullets_upgrade, is_gla_rebel_template, rebel_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        if !is_gla_rebel_template(&obj.template_name) {
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
        use crate::game_logic::host_gla_rebel::{UPGRADE_GLA_AP_BULLETS, is_gla_rebel_template};
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_gla_rebel_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_BULLETS.to_string());
        self.rebel_residual_ap_upgrades = self.rebel_residual_ap_upgrades.saturating_add(1);
        self.refresh_rebel_weapon(object_id);
        true
    }

    /// Apply GLA Rebel residual fire: intended-only gun damage residual.
    /// Apply USA Ranger residual fire: rifle intended-only or FlashBang dual-radius splash.
    /// C++ RangerFlashBangGrenade DumbProjectile residual.
    pub fn spawn_flashbang_grenade_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_ranger::{
            FLASHBANG_GRENADE_MAX_HEALTH, FLASHBANG_GRENADE_PROJECTILE,
            flashbang_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(FLASHBANG_GRENADE_PROJECTILE) {
            let mut t = ThingTemplate::new(FLASHBANG_GRENADE_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(FLASHBANG_GRENADE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(FLASHBANG_GRENADE_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadius residual on RangerFlashBangGrenadeWeapon (**4**, all targets).
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_ranger::ranger_flashbang_scatter_aim(aim, seed);
        if scattered {
            self.flashbang_scatter_applied = self.flashbang_scatter_applied.saturating_add(1);
        }
        // ScatterRadius (**4**) residual: miss peels when aim lands outside secondary splash.
        if intended.is_some() {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_ranger::ranger_flashbang_scatter_misses(seed, hit_r) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_ranger::FLASHBANG_SECONDARY_RADIUS {
                        self.flashbang_scatter_misses =
                            self.flashbang_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(FLASHBANG_GRENADE_PROJECTILE, team, start)?;
        let frames = flashbang_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.flashbang_grenade_projectile = true;
            o.flashbang_grenade_from = Some([start.x, start.y, start.z]);
            o.flashbang_grenade_aim = Some([aim.x, aim.y, aim.z]);
            o.flashbang_grenade_launch_frame = Some(self.frame);
            o.flashbang_grenade_flight_frames = frames;
            o.flashbang_grenade_intended = intended.map(|id| id.0);
            o.note_producer(source_id);
            o.health.maximum = FLASHBANG_GRENADE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, FLASHBANG_GRENADE_MAX_HEALTH);
        }
        self.flashbang_grenades_spawned = self.flashbang_grenades_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_flashbang_grenade_projectiles(&mut self) {
        use crate::game_logic::host_ranger::flashbang_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.flashbang_grenade_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .flashbang_grenade_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .flashbang_grenade_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.flashbang_grenade_intended.map(ObjectId),
                    from,
                    aim,
                    o.flashbang_grenade_launch_frame.unwrap_or(frame),
                    o.flashbang_grenade_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = flashbang_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.flashbang_grenade_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_ranger_residual_at(pos, source, intended, true);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// Spawn Humvee TOW projectile residual (HumveeMissile ground or PatriotMissile air).
    pub fn spawn_humvee_tow_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        air: bool,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_humvee::{
            HUMVEE_AIR_TOW_MISSILE_FUEL_FRAMES, HUMVEE_AIR_TOW_MISSILE_IGNITION_DELAY_FRAMES,
            HUMVEE_AIR_TOW_MISSILE_MAX_HEALTH, HUMVEE_AIR_TOW_PROJECTILE,
            HUMVEE_GROUND_TOW_MISSILE_FUEL_FRAMES, HUMVEE_GROUND_TOW_MISSILE_IGNITION_DELAY_FRAMES,
            HUMVEE_GROUND_TOW_MISSILE_MAX_HEALTH, HUMVEE_MISSILE_PROJECTILE,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (name, max_hp, fuel_frames, ign_delay) = if air {
            (
                HUMVEE_AIR_TOW_PROJECTILE,
                HUMVEE_AIR_TOW_MISSILE_MAX_HEALTH,
                HUMVEE_AIR_TOW_MISSILE_FUEL_FRAMES,
                HUMVEE_AIR_TOW_MISSILE_IGNITION_DELAY_FRAMES,
            )
        } else {
            (
                HUMVEE_MISSILE_PROJECTILE,
                HUMVEE_GROUND_TOW_MISSILE_MAX_HEALTH,
                HUMVEE_GROUND_TOW_MISSILE_FUEL_FRAMES,
                HUMVEE_GROUND_TOW_MISSILE_IGNITION_DELAY_FRAMES,
            )
        };

        if !self.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Projectile)
                .set_health(max_hp)
                .set_cost(0, 0);
            self.templates.insert(name.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on HumveeMissileWeapon (ground TOW) vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_humvee::humvee_tow_scatter_aim(
            aim,
            target_is_infantry,
            air,
            seed,
        );
        if scattered {
            self.humvee_tow_scatter_applied = self.humvee_tow_scatter_applied.saturating_add(1);
        }
        if target_is_infantry && !air {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_humvee::humvee_tow_scatter_misses_infantry(
                true, false, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_humvee::HUMVEE_GROUND_TOW_RADIUS {
                        self.humvee_tow_scatter_misses =
                            self.humvee_tow_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        if air {
            start.y = start.y.max(aim.y + 10.0);
        } else {
            start.y = start.y.max(2.0);
        }
        let pid = self.create_object(name, team, start)?;
        let expires = self.frame.saturating_add(fuel_frames.max(1));
        let ignites = self.frame.saturating_add(ign_delay);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.humvee_tow_projectile = true;
            o.humvee_tow_air = air;
            o.humvee_tow_aim = Some([aim.x, aim.y, aim.z]);
            o.humvee_tow_intended = intended.map(|id| id.0);
            o.humvee_tow_travelled = 0.0;
            o.humvee_tow_fuel_expires_frame = Some(expires);
            o.humvee_tow_ignition_frame = Some(ignites);
            o.note_producer(source_id);
            o.health.current = max_hp;
            o.health.maximum = max_hp;
        }
        self.humvee_tow_missiles_spawned = self.humvee_tow_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_humvee_tow_missile_projectiles(&mut self) {
        use crate::game_logic::host_humvee::{
            HUMVEE_AIR_TOW_MISSILE_LOCK_DISTANCE, HUMVEE_AIR_TOW_MISSILE_TURN_DISTANCE,
            HUMVEE_GROUND_TOW_MISSILE_TURN_DISTANCE, humvee_air_tow_missile_step_speed,
            humvee_ground_tow_missile_step_speed,
        };
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.humvee_tow_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(
            ObjectId,
            Option<ObjectId>,
            Option<ObjectId>,
            glam::Vec3,
            bool,
        )> = Vec::new();
        for id in flying {
            let (source, intended, aim, pos, fuel_done, ignited, air, travelled) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .humvee_tow_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.humvee_tow_intended.map(ObjectId);
                let fuel_done = o
                    .humvee_tow_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .humvee_tow_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
                    o.humvee_tow_air,
                    o.humvee_tow_travelled,
                )
            };
            // Air TOW seeks live intended target (PatriotMissile TryToFollowTarget=Yes).
            // Ground TOW locks aim at fire (TryToFollowTarget=No).
            let aim = if air {
                intended
                    .and_then(|tid| {
                        self.objects
                            .get(&tid)
                            .filter(|t| t.is_alive())
                            .map(|t| t.get_position())
                    })
                    .unwrap_or(aim)
            } else {
                aim
            };
            let turn_dist = if air {
                HUMVEE_AIR_TOW_MISSILE_TURN_DISTANCE
            } else {
                HUMVEE_GROUND_TOW_MISSILE_TURN_DISTANCE
            };
            // Before DistanceToTravelBeforeTurning: coast straight at initial velocity.
            let can_steer = travelled >= turn_dist;
            let speed = if air {
                humvee_air_tow_missile_step_speed(ignited && can_steer)
            } else {
                humvee_ground_tow_missile_step_speed(ignited && can_steer)
            };
            // Pre-turn residual: keep launch direction toward original aim snapshot.
            let to_aim = aim - pos;
            let dist = to_aim.length();
            let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
            let vel = if dist > 0.001 {
                to_aim.normalize() * step_speed
            } else {
                glam::Vec3::new(0.0, -step_speed, 0.0)
            };
            let step = vel.length().max(step_speed);
            let new_pos = pos + vel;
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(new_pos);
                o.humvee_tow_travelled += step;
                o.humvee_tow_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let lock = if air {
                dist <= HUMVEE_AIR_TOW_MISSILE_LOCK_DISTANCE
            } else {
                false
            };
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 8.0 || lock;
            if fuel_done || near {
                impact.push((id, source, intended, aim, air));
            }
        }
        for (id, source, intended, pos, air) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.humvee_tow_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_humvee_tow_residual_at(pos, source, intended, air);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// Spawn DragonTankFlameProjectile residual (MissileAI non-seek, DetonateOnNoFuel).
    pub fn spawn_dragon_flame_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_FLAME_MISSILE_FUEL_FRAMES, DRAGON_FLAME_MISSILE_IGNITION_DELAY_FRAMES,
            DRAGON_FLAME_MISSILE_MAX_HEALTH, DRAGON_FLAME_PROJECTILE, DRAGON_FLAME_STREAM,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(DRAGON_FLAME_PROJECTILE) {
            let mut t = ThingTemplate::new(DRAGON_FLAME_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(DRAGON_FLAME_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(DRAGON_FLAME_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mut start = from;
        start.y = start.y.max(2.0);
        let pid = self.create_object(DRAGON_FLAME_PROJECTILE, team, start)?;
        let expires = self
            .frame
            .saturating_add(DRAGON_FLAME_MISSILE_FUEL_FRAMES.max(1));
        let ignites = self
            .frame
            .saturating_add(DRAGON_FLAME_MISSILE_IGNITION_DELAY_FRAMES);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.dragon_flame_projectile = true;
            o.dragon_flame_aim = Some([aim.x, aim.y, aim.z]);
            o.dragon_flame_intended = intended.map(|id| id.0);
            o.dragon_flame_travelled = 0.0;
            o.dragon_flame_fuel_expires_frame = Some(expires);
            o.dragon_flame_ignition_frame = Some(ignites);
            o.dragon_flame_shooter = Some(source_id.0);
            o.note_producer(source_id);
            o.health.current = DRAGON_FLAME_MISSILE_MAX_HEALTH;
            o.health.maximum = DRAGON_FLAME_MISSILE_MAX_HEALTH;
        }
        // Seed stream with launch point residual.
        self.projectile_streams.add_projectile(
            source_id,
            DRAGON_FLAME_STREAM,
            start,
            intended,
            Some(aim),
            self.frame,
        );
        self.dragon_flame_missiles_spawned = self.dragon_flame_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_dragon_flame_projectiles(&mut self) {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_FLAME_MISSILE_DETONATE_ON_NO_FUEL, DRAGON_FLAME_MISSILE_TURN_DISTANCE,
            DRAGON_FLAME_STREAM, dragon_flame_missile_step_speed,
        };
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.dragon_flame_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        let mut stream_pts: Vec<(ObjectId, glam::Vec3, Option<ObjectId>, glam::Vec3)> = Vec::new();
        for id in flying {
            let (source, intended, aim, pos, fuel_done, ignited, travelled, shooter) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .dragon_flame_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.dragon_flame_intended.map(ObjectId);
                let fuel_done = o
                    .dragon_flame_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .dragon_flame_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                let shooter = o.dragon_flame_shooter.map(ObjectId).or(o.producer_id);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
                    o.dragon_flame_travelled,
                    shooter,
                )
            };
            // Non-seek: keep fire-time aim (TryToFollowTarget = No).
            let can_steer = travelled >= DRAGON_FLAME_MISSILE_TURN_DISTANCE;
            let speed = dragon_flame_missile_step_speed(ignited && can_steer);
            let to_aim = aim - pos;
            let dist = to_aim.length();
            let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
            let vel = if dist > 0.001 {
                to_aim.normalize() * step_speed
            } else {
                glam::Vec3::new(0.0, -step_speed, 0.0)
            };
            let step = vel.length().max(step_speed);
            let new_pos = pos + vel;
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(new_pos);
                o.dragon_flame_travelled += step;
                o.dragon_flame_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if let Some(sid) = shooter {
                stream_pts.push((sid, new_pos, intended, aim));
            }
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 6.0;
            let fuel_detonate = fuel_done && DRAGON_FLAME_MISSILE_DETONATE_ON_NO_FUEL;
            if fuel_detonate || near {
                impact.push((id, source, intended, if near { aim } else { new_pos }));
            }
        }
        for (sid, pos, intended, aim) in stream_pts {
            self.projectile_streams.add_projectile(
                sid,
                DRAGON_FLAME_STREAM,
                pos,
                intended,
                Some(aim),
                frame,
            );
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.dragon_flame_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_dragon_flame_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// Spawn ToxinTruckStreamProjectile residual (MissileAI non-seek poison stream).
    pub fn spawn_toxin_stream_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_toxin_tractor::{
            TOXIN_STREAM_MISSILE_FUEL_FRAMES, TOXIN_STREAM_MISSILE_IGNITION_DELAY_FRAMES,
            TOXIN_STREAM_MISSILE_MAX_HEALTH, TOXIN_STREAM_NAME, UPGRADE_GLA_ANTHRAX_BETA,
            UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT, anthrax_tier_from_flags,
            is_chem_general_template, toxin_stream_projectile_name,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (name, max_hp) = {
            let src = self.objects.get(&source_id)?;
            let has_gamma = src.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
                || src.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT)
                || src.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                || src.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
            let has_beta = src.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA)
                || src.has_upgrade_tag("Upgrade_GLAAnthraxBeta");
            let tier = anthrax_tier_from_flags(
                has_gamma,
                has_beta,
                is_chem_general_template(&src.template_name),
            );
            (
                toxin_stream_projectile_name(tier),
                TOXIN_STREAM_MISSILE_MAX_HEALTH,
            )
        };

        if !self.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Projectile)
                .set_health(max_hp)
                .set_cost(0, 0);
            self.templates.insert(name.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mut start = from;
        start.y = start.y.max(2.0);
        let pid = self.create_object(name, team, start)?;
        let expires = self
            .frame
            .saturating_add(TOXIN_STREAM_MISSILE_FUEL_FRAMES.max(1));
        let ignites = self
            .frame
            .saturating_add(TOXIN_STREAM_MISSILE_IGNITION_DELAY_FRAMES);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.toxin_stream_projectile = true;
            o.toxin_stream_aim = Some([aim.x, aim.y, aim.z]);
            o.toxin_stream_intended = intended.map(|id| id.0);
            o.toxin_stream_travelled = 0.0;
            o.toxin_stream_fuel_expires_frame = Some(expires);
            o.toxin_stream_ignition_frame = Some(ignites);
            o.toxin_stream_shooter = Some(source_id.0);
            o.note_producer(source_id);
            o.health.current = max_hp;
            o.health.maximum = max_hp;
        }
        self.projectile_streams.add_projectile(
            source_id,
            TOXIN_STREAM_NAME,
            start,
            intended,
            Some(aim),
            self.frame,
        );
        self.toxin_stream_missiles_spawned = self.toxin_stream_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_toxin_stream_projectiles(&mut self) {
        use crate::game_logic::host_toxin_tractor::{
            TOXIN_STREAM_MISSILE_TURN_DISTANCE, TOXIN_STREAM_NAME, toxin_stream_missile_step_speed,
        };
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.toxin_stream_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(
            ObjectId,
            Option<ObjectId>,
            Option<ObjectId>,
            glam::Vec3,
            Team,
        )> = Vec::new();
        let mut stream_pts: Vec<(ObjectId, glam::Vec3, Option<ObjectId>, glam::Vec3)> = Vec::new();
        for id in flying {
            let (source, intended, aim, pos, fuel_done, ignited, travelled, shooter, team) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .toxin_stream_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.toxin_stream_intended.map(ObjectId);
                let fuel_done = o
                    .toxin_stream_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                let ignited = o
                    .toxin_stream_ignition_frame
                    .map(|f| f <= frame)
                    .unwrap_or(true);
                let shooter = o.toxin_stream_shooter.map(ObjectId).or(o.producer_id);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    fuel_done,
                    ignited,
                    o.toxin_stream_travelled,
                    shooter,
                    o.team,
                )
            };
            let can_steer = travelled >= TOXIN_STREAM_MISSILE_TURN_DISTANCE;
            let speed = toxin_stream_missile_step_speed(ignited && can_steer);
            let to_aim = aim - pos;
            let dist = to_aim.length();
            let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
            let vel = if dist > 0.001 {
                to_aim.normalize() * step_speed
            } else {
                glam::Vec3::new(0.0, -step_speed, 0.0)
            };
            let step = vel.length().max(step_speed);
            let new_pos = pos + vel;
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(new_pos);
                o.toxin_stream_travelled += step;
                o.toxin_stream_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            if let Some(sid) = shooter {
                stream_pts.push((sid, new_pos, intended, aim));
            }
            let near = dist <= speed + 0.001 || (aim - new_pos).length() < 6.0;
            // Fuel expiry detonates residual splash at current position (stream continuity).
            if fuel_done || near {
                impact.push((id, source, intended, if near { aim } else { new_pos }, team));
            }
        }
        for (sid, pos, intended, aim) in stream_pts {
            self.projectile_streams.add_projectile(
                sid,
                TOXIN_STREAM_NAME,
                pos,
                intended,
                Some(aim),
                frame,
            );
        }
        for (id, source, intended, pos, team) in impact {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.toxin_stream_projectile = false;
                o.set_position(pos);
            }
            let source_team = source
                .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
                .unwrap_or(team);
            let _ = self.apply_toxin_tractor_stream_at(pos, source, intended, source_team);
            self.mark_object_for_destruction(id, Some(team));
        }
    }

    /// Spawn TechnicalRPGMissile residual (MissileAI seek, Fuel 1000ms).
    pub fn spawn_technical_rpg_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_technical::{
            TECH_RPG_MISSILE_FUEL_FRAMES, TECH_RPG_MISSILE_IGNITION_DELAY_FRAMES,
            TECH_RPG_MISSILE_MAX_HEALTH, TECHNICAL_RPG_MISSILE,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(TECHNICAL_RPG_MISSILE) {
            let mut t = ThingTemplate::new(TECHNICAL_RPG_MISSILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(TECH_RPG_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(TECHNICAL_RPG_MISSILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let mut start = from;
        start.y = start.y.max(aim.y) + 6.0;
        let pid = self.create_object(TECHNICAL_RPG_MISSILE, team, start)?;
        let expires = self
            .frame
            .saturating_add(TECH_RPG_MISSILE_FUEL_FRAMES.max(1));
        let ignites = self
            .frame
            .saturating_add(TECH_RPG_MISSILE_IGNITION_DELAY_FRAMES);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.technical_rpg_missile_projectile = true;
            o.technical_rpg_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.technical_rpg_missile_intended = intended.map(|id| id.0);
            o.technical_rpg_missile_travelled = 0.0;
            o.technical_rpg_missile_fuel_expires_frame = Some(expires);
            o.technical_rpg_missile_ignition_frame = Some(ignites);
            o.note_producer(source_id);
            o.health.current = TECH_RPG_MISSILE_MAX_HEALTH;
            o.health.maximum = TECH_RPG_MISSILE_MAX_HEALTH;
        }
        self.technical_rpg_missiles_spawned = self.technical_rpg_missiles_spawned.saturating_add(1);
        Some(pid)
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
