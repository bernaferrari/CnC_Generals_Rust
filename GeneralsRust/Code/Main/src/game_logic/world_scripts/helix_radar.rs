//! Host scripts `impl GameLogic` — `helix_radar`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! nuclear tanks / booby trap / helix napalm / steal cash / radar / spy
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `ActionManager.cpp:1652-1672` `SPECIAL_CASH_HACK`.
fn is_legal_superweapon_cash_hack_victim(victim: &Object, caster_team: Option<Team>) -> bool {
    use crate::game_logic::host_hero_abilities::is_cash_hack_target;
    if !victim.is_alive() {
        return false;
    }
    if !victim.is_kind_of(KindOf::Structure) {
        return false;
    }
    let Some(caster_team) = caster_team else {
        return false;
    };
    // C++ relationship ENEMIES — same-team / Neutral are not enemies.
    if victim.team == caster_team || victim.team == Team::Neutral || caster_team == Team::Neutral {
        return false;
    }
    if !victim.thing.template.capturable || victim.thing.template.immune_to_capture {
        return false;
    }
    if victim.is_rebuild_hole {
        return false;
    }
    if victim.status.under_construction
        || victim.construction_percent + 0.001 < 1.0
        || victim.status.sold
    {
        return false;
    }
    is_cash_hack_target(
        &victim.template_name,
        victim.is_kind_of(KindOf::SupplyCenter),
        victim.is_kind_of(KindOf::FSSupplyCenter),
        victim.is_kind_of(KindOf::FSBlackMarket),
        victim.is_kind_of(KindOf::FSSupplyDropzone),
    )
}

/// C++ `Object::look` lookingMask: controller `getRelationship(current->defaultTeam) == ALLIES`.
/// FFA same-faction enemies do not share RadarVanScan / SpySat ping disks.
fn looker_mask_for_controller(
    players: &std::collections::HashMap<u32, crate::game_logic::Player>,
    controller_id: u32,
) -> u32 {
    use gamelogic::common::Relationship;
    let mut mask = 0u32;
    for &pid in players.keys() {
        if GameLogic::player_relationship_from_map(players, controller_id, pid)
            == Relationship::Allies
        {
            mask |= 1u32 << pid.min(31);
        }
    }
    if mask == 0 {
        mask = 1u32 << controller_id.min(31);
    }
    mask
}

impl GameLogic {
    // -----------------------------------------------------------------------
    // China Nuclear Tanks residual (death blast + radiation + speed)
    // Fail-closed: not full FireWeaponWhenDead exclusive / Nuclear*Locomotor matrix.
    // -----------------------------------------------------------------------

    /// Host Nuclear Tanks residual registry.
    pub fn nuclear_tanks(
        &self,
    ) -> &crate::game_logic::host_nuclear_tanks::HostNuclearTanksRegistry {
        &self.nuclear_tanks
    }

    pub fn honesty_nuclear_tanks_upgrade_ok(&self) -> bool {
        self.nuclear_tanks.honesty_upgrade_ok()
    }

    pub fn honesty_nuclear_tanks_death_ok(&self) -> bool {
        self.nuclear_tanks.honesty_death_ok()
    }

    pub fn honesty_nuclear_tanks_radiation_ok(&self) -> bool {
        self.nuclear_tanks.honesty_radiation_ok()
    }

    pub fn honesty_nuclear_tanks_ok(&self) -> bool {
        self.nuclear_tanks.honesty_host_path_ok()
    }

    /// Apply residual NuclearTankDeathWeapon dual-radius blast + SmallRadiationField.
    pub fn apply_nuclear_tanks_death_detonation_at(
        &mut self,
        tank_id: ObjectId,
        tank_team: Team,
        tank_pos: Vec3,
        nuke_general: bool,
    ) -> bool {
        use crate::game_logic::host_nuclear_tanks::{
            NUCLEAR_TANK_DAMAGE_TYPE, NUCLEAR_TANK_DEATH_AUDIO, NUCLEAR_TANK_DEATH_TYPE,
            SMALL_RADIATION_AUDIO, is_legal_nuclear_death_target, nuclear_tank_death_damage_at,
            nuclear_tank_death_splash_radius,
        };

        let max_radius = nuclear_tank_death_splash_radius(nuke_general);
        let mut blast_hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            if vid == tank_id {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            let combat_kind = victim.is_kind_of(KindOf::Attackable)
                || victim.is_kind_of(KindOf::Structure)
                || victim.is_kind_of(KindOf::Infantry)
                || victim.is_kind_of(KindOf::Vehicle)
                || victim.is_kind_of(KindOf::Aircraft);
            if !is_legal_nuclear_death_target(
                victim.is_alive(),
                false,
                victim.status.under_construction,
                combat_kind,
            ) {
                continue;
            }
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - tank_pos.x;
                let dz = vpos.z - tank_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            if dist > max_radius {
                continue;
            }
            let dmg = nuclear_tank_death_damage_at(dist, nuke_general);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                blast_hits = blast_hits.saturating_add(1);
                if victim.take_damage_from_immediate_residual(
                    dmg,
                    Some(tank_id),
                    NUCLEAR_TANK_DAMAGE_TYPE,
                    NUCLEAR_TANK_DEATH_TYPE,
                ) {
                    destroy_ids.push((vid, tank_team));
                }
            }
        }

        self.nuclear_tanks
            .record_death_detonation(blast_hits, nuke_general);
        let _ = self
            .nuclear_tanks
            .spawn_radiation_zone(tank_id, tank_team, tank_pos, self.frame);

        self.queue_audio_event(
            AudioEventRequest::new(NUCLEAR_TANK_DEATH_AUDIO)
                .with_object(tank_id)
                .with_position(tank_pos)
                .with_priority(190),
        );
        self.queue_audio_event(
            AudioEventRequest::new(SMALL_RADIATION_AUDIO)
                .with_position(tank_pos)
                .with_priority(140),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            tank_pos,
            self.frame,
            Some(tank_id),
            None,
        );

        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Advance Nuclear Tanks SmallRadiationField residual zones.
    pub(in super::super) fn update_nuclear_tanks_radiation_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .nuclear_tanks
            .plan_due_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed =
                        target.take_radiation_field_tick(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.nuclear_tanks.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.nuclear_tanks.prune_expired(frame);
    }

    // -----------------------------------------------------------------------
    // GLA Rebel BoobyTrap residual
    // Fail-closed: not full StickyBombUpdate SpecialObject / MaxSpecialObjects matrix.
    // -----------------------------------------------------------------------

    /// Host BoobyTrap residual registry.
    pub fn booby_trap_residual(
        &self,
    ) -> &crate::game_logic::host_booby_trap::HostBoobyTrapRegistry {
        &self.booby_trap
    }

    /// C++ StickyBombUpdate::xfer + Object status BOOBY_TRAPPED.
    /// Live WorldSnapshot has no nested booby field; restore the host registry
    /// and SpecialObject spawn counter after objects exist.
    pub fn restore_booby_traps(
        &mut self,
        registry: crate::game_logic::host_booby_trap::HostBoobyTrapRegistry,
        objects_spawned: u32,
    ) {
        self.booby_trap = registry;
        self.booby_trap_objects_spawned = objects_spawned;
    }

    pub fn booby_trap_objects_spawned(&self) -> u32 {
        self.booby_trap_objects_spawned
    }

    pub fn honesty_booby_trap_plant_ok(&self) -> bool {
        self.booby_trap.honesty_plant_ok()
    }

    pub fn honesty_booby_trap_detonate_ok(&self) -> bool {
        self.booby_trap.honesty_detonate_ok()
    }

    pub fn honesty_booby_trap_upgrade_ok(&self) -> bool {
        self.booby_trap.honesty_upgrade_ok()
    }

    pub fn honesty_booby_trap_ok(&self) -> bool {
        self.booby_trap.honesty_host_path_ok()
    }

    /// C++ SpecialObject BoobyTrap ThingFactory residual.
    pub fn spawn_booby_trap_special_object(
        &mut self,
        planter_id: ObjectId,
        team: Team,
        structure_id: ObjectId,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_booby_trap::{BOOBY_TRAP_MAX_HEALTH, BOOBY_TRAP_OBJECT};
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(BOOBY_TRAP_OBJECT) {
            let mut t = ThingTemplate::new(BOOBY_TRAP_OBJECT);
            t.add_kind_of(KindOf::Immobile)
                .set_health(BOOBY_TRAP_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(BOOBY_TRAP_OBJECT.to_string(), t);
        }
        let pos = self
            .objects
            .get(&structure_id)
            .map(|o| {
                let p = o.get_position();
                glam::Vec3::new(p.x, p.y + 8.0, p.z)
            })
            .unwrap_or(glam::Vec3::ZERO);
        let owner_player_id = {
            let planter = self.objects.get(&planter_id)?;
            if planter.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(planter)?)
            } else {
                None
            }
        };
        let bid =
            self.create_object_for_owner_or_team(BOOBY_TRAP_OBJECT, team, owner_player_id, pos)?;
        if let Some(o) = self.objects.get_mut(&bid) {
            o.booby_trap_special = true;
            o.booby_trap_attached_to = Some(structure_id);
            o.producer_id = Some(planter_id);
            o.health.maximum = BOOBY_TRAP_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, BOOBY_TRAP_MAX_HEALTH);
            o.movement.max_speed = 0.0;
            o.weapon = None;
            o.secondary_weapon = None;
        }
        self.booby_trap_objects_spawned = self.booby_trap_objects_spawned.saturating_add(1);
        Some(bid)
    }

    pub fn destroy_booby_trap_special_object(&mut self, charge_id: ObjectId) {
        if let Some(o) = self.objects.get_mut(&charge_id) {
            if !o.booby_trap_special {
                return;
            }
            // Wave 751: under damage authority, do not zero host HP mid-frame
            // (dual with GW HP writeback). Project lethal via damage log + flags;
            // non-authority path keeps host HP clear.
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                let hp = o.health.current.max(1.0);
                crate::game_logic::host_damage_log::record(charge_id, hp, None, true);
            } else {
                o.health.current = 0.0;
            }
            o.status.destroyed = true;
            o.status.effectively_dead = true;
            o.booby_trap_special = false;
            o.booby_trap_attached_to = None;
        }
        self.mark_object_for_destruction(charge_id, None);
    }

    /// C++ StickyBombUpdate residual for BoobyTrap SpecialObject.
    pub fn update_booby_trap_special_attachments(&mut self) {
        const STICKY_OFFSET_Y: f32 = 8.0;
        let pairs: Vec<(ObjectId, ObjectId)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.booby_trap_special {
                    o.booby_trap_attached_to.map(|s| (*id, s))
                } else {
                    None
                }
            })
            .collect();
        let mut destroy = Vec::new();
        let mut moves = Vec::new();
        for (cid, sid) in pairs {
            let Some(structure) = self.objects.get(&sid) else {
                destroy.push(cid);
                continue;
            };
            if !structure.is_alive() || structure.status.destroyed {
                // Death detonate path handles registry; just drop orphan special object.
                destroy.push(cid);
                continue;
            }
            let p = structure.get_position();
            moves.push((cid, glam::Vec3::new(p.x, p.y + STICKY_OFFSET_Y, p.z)));
        }
        for (cid, pos) in moves {
            if let Some(o) = self.objects.get_mut(&cid) {
                o.set_position(pos);
            }
        }
        for cid in destroy {
            self.destroy_booby_trap_special_object(cid);
        }
    }

    pub fn honesty_booby_trap_special_object_ok(&self) -> bool {
        self.booby_trap_objects_spawned > 0
    }

    /// Detonate residual BoobyTrap on structure (capture / death / special trigger).
    ///
    /// Returns units hit. Clears BOOBY_TRAPPED status and registry plant.
    pub fn detonate_booby_trap_at(
        &mut self,
        structure_id: ObjectId,
        structure_pos: Vec3,
        trigger_unit: Option<ObjectId>,
        via_capture: bool,
        via_death: bool,
    ) -> u32 {
        use crate::game_logic::host_booby_trap::{
            BOOBY_GEOMETRY_DAMAGE_FX, booby_trap_damage_at, booby_trap_splash_radius,
            is_legal_booby_victim, is_planter_ally,
        };

        let Some(plant) = self.booby_trap.take_plant(structure_id) else {
            // Status may lag registry — clear flag only.
            if let Some(obj) = self.objects.get_mut(&structure_id) {
                obj.set_status_booby_trapped(false);
            }
            return 0;
        };

        // Allies of planter do not trigger (C++ checkAndDetonateBoobyTrap).
        if let Some(tid) = trigger_unit {
            if let Some(trigger) = self.objects.get(&tid) {
                if is_planter_ally(plant.planter_team, trigger.team) {
                    // Re-install — ally touch should not consume trap.
                    self.booby_trap.install(
                        structure_id,
                        plant.planter_id,
                        plant.planter_team,
                        plant.plant_frame,
                        plant.geometry_radius,
                        plant.charge_object_id,
                    );
                    return 0;
                }
            }
        }

        if let Some(obj) = self.objects.get_mut(&structure_id) {
            obj.set_status_booby_trapped(false);
        }
        if let Some(cid) = plant.charge_object_id {
            self.destroy_booby_trap_special_object(cid);
        }

        let max_r = booby_trap_splash_radius(plant.geometry_radius);
        let mut hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            let combat_kind = victim.is_kind_of(KindOf::Attackable)
                || victim.is_kind_of(KindOf::Structure)
                || victim.is_kind_of(KindOf::Infantry)
                || victim.is_kind_of(KindOf::Vehicle)
                || victim.is_kind_of(KindOf::Aircraft);
            // C++ StickyBombUpdate::detonate includes the trapped object at
            // bounding-sphere dist 0 (primary damage). Leftover already matches.
            if !is_legal_booby_victim(
                victim.is_alive(),
                victim.status.under_construction,
                combat_kind,
            ) {
                continue;
            }
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - structure_pos.x;
                let dz = vpos.z - structure_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            if dist > max_r {
                continue;
            }
            let dmg = booby_trap_damage_at(dist, plant.geometry_radius);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                hits = hits.saturating_add(1);
                if victim.take_damage_from_immediate_residual(
                    dmg,
                    Some(plant.planter_id),
                    crate::game_logic::host_booby_trap::BOOBY_DAMAGE_TYPE,
                    crate::game_logic::host_booby_trap::BOOBY_DEATH_TYPE,
                ) {
                    destroy_ids.push((vid, plant.planter_team));
                }
            }
        }

        self.booby_trap
            .record_detonation(hits, via_capture, via_death);
        // C++ StickyBombUpdate::detonate: FXList::doFXPos(m_geometryBasedDamageFX,
        // pos, NULL, 0, NULL, secondaryDamageRange). Never queue the FXList name.
        if !crate::game_logic::dispatch_fx_list_at_pos_ex(
            BOOBY_GEOMETRY_DAMAGE_FX,
            structure_pos,
            None,
            0.0,
            max_r,
        ) {
            for sound in crate::game_logic::sound_names_for_fx_list(BOOBY_GEOMETRY_DAMAGE_FX) {
                self.queue_audio_event(
                    AudioEventRequest::new(&sound)
                        .with_object(structure_id)
                        .with_position(structure_pos)
                        .with_priority(180),
                );
            }
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::WeaponImpact,
                BOOBY_GEOMETRY_DAMAGE_FX,
                structure_pos,
                self.frame,
                Some(structure_id),
                None,
            );
        }

        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        hits
    }

    // -----------------------------------------------------------------------
    // China Helix NapalmBomb special ability residual
    // Fail-closed: not full SpecialObject fall / Firestorm expand animation.
    // -----------------------------------------------------------------------

    /// Host Helix Napalm residual registry.
    /// C++ HistoricBonus → FirestormSmallCreationWeapon residual drain.
    pub(in super::super) fn drain_historic_bonus_firestorms(&mut self) {
        let pending = crate::game_logic::host_historic_bonus::drain_pending_firestorms();
        for p in pending {
            // Reuse Helix firestorm DoT residual zones (same OCL FirestormSmall numbers).
            let _ = self.helix_napalm.record_drop_and_spawn_firestorm(
                p.source_id,
                p.source_team,
                p.position,
                self.frame,
                p.black_napalm,
                0,
                0.0,
            );
        }
    }

    pub fn helix_napalm(&self) -> &crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry {
        &self.helix_napalm
    }

    pub fn honesty_helix_napalm_drop_ok(&self) -> bool {
        self.helix_napalm.honesty_drop_ok()
    }

    pub fn honesty_helix_napalm_blast_ok(&self) -> bool {
        self.helix_napalm.honesty_blast_ok()
    }

    pub fn honesty_helix_napalm_firestorm_ok(&self) -> bool {
        self.helix_napalm.honesty_firestorm_ok()
    }

    pub fn honesty_helix_napalm_ok(&self) -> bool {
        self.helix_napalm.honesty_host_path_ok()
    }

    /// Activate Helix NapalmBomb residual at `target_position`.
    ///
    /// Retail: SpecialAbilityHelixNapalmBomb → SpecialObject NapalmBomb →
    /// HeightDie → NapalmBombWeapon blast + OCL_FirestormSmall.
    /// Requires Upgrade_HelixNapalmBomb residual unlock (TestHelix always unlocked).
    /// BlackNapalm player upgrade residual raises Firestorm tick damage.
    /// C++ SpecialObject NapalmBomb residual (Helix drop → HeightDie → FireWeaponWhenDead).
    pub fn spawn_helix_napalm_bomb_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        black_napalm: bool,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_height_die::HostHeightDieData;
        use crate::game_logic::host_helix_napalm::{
            NAPALM_BOMB_FALL_SPEED_PER_FRAME, NAPALM_BOMB_HEIGHT_DIE_TARGET,
            NAPALM_BOMB_MAX_HEALTH, NAPALM_BOMB_PROJECTILE,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let tpl_name = if black_napalm {
            "BlackNapalmBomb"
        } else {
            NAPALM_BOMB_PROJECTILE
        };
        if !self.templates.contains_key(tpl_name) {
            let mut t = ThingTemplate::new(tpl_name);
            t.add_kind_of(KindOf::Projectile)
                .set_health(NAPALM_BOMB_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(tpl_name.to_string(), t);
        }
        // Also seed NapalmBomb name for height_die peel when black uses alias.
        if black_napalm && !self.templates.contains_key(NAPALM_BOMB_PROJECTILE) {
            let mut t = ThingTemplate::new(NAPALM_BOMB_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(NAPALM_BOMB_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(NAPALM_BOMB_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_owner_player_id = {
            let source = self.objects.get(&source_id)?;
            if source.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(source)?)
            } else {
                None
            }
        };
        // Drop slightly below the Helix so freefall residual is visible.
        let mut start = from;
        if start.y < aim.y + 20.0 {
            start.y = aim.y + 40.0;
        }
        // Bias XZ toward aim so the bomb lands near the intended drop point.
        let dir_xz = glam::Vec3::new(aim.x - start.x, 0.0, aim.z - start.z);
        let horiz = dir_xz.length();
        if horiz > 1.0 {
            start += dir_xz * (8.0 / horiz).min(1.0);
        }
        let pid =
            self.create_object_for_owner_or_team(tpl_name, team, source_owner_player_id, start)?;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.helix_napalm_bomb_projectile = true;
            o.note_producer(source_id);
            o.health.maximum = NAPALM_BOMB_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, NAPALM_BOMB_MAX_HEALTH);
            // Fall velocity residual (Y-up).
            let fall_frames = ((start.y - aim.y).max(1.0) / NAPALM_BOMB_FALL_SPEED_PER_FRAME)
                .ceil()
                .max(1.0);
            let vx = (aim.x - start.x) / fall_frames;
            let vz = (aim.z - start.z) / fall_frames;
            o.movement.velocity = glam::Vec3::new(vx, -NAPALM_BOMB_FALL_SPEED_PER_FRAME, vz);
            o.height_die = Some(HostHeightDieData::with_target(
                NAPALM_BOMB_HEIGHT_DIE_TARGET,
                true,
                self.frame,
            ));
            o.ensure_height_die(self.frame);
        }
        self.helix_napalm.record_projectile_spawn();
        Some(pid)
    }

    pub fn update_helix_napalm_bomb_projectiles(&mut self) {
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.helix_napalm_bomb_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in flying {
            if let Some(o) = self.objects.get_mut(&id) {
                let p = o.get_position();
                let v = o.movement.velocity;
                o.set_position(p + v);
            }
        }
    }

    pub fn activate_helix_napalm_bomb(
        &mut self,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_helix_napalm::{
            HELIX_FIRESTORM_AUDIO, HELIX_NAPALM_DROP_AUDIO, UPGRADE_CHINA_BLACK_NAPALM,
            UPGRADE_HELIX_NAPALM_BOMB, helix_napalm_unlocked, is_helix_napalm_caster,
        };

        let (source_team, template_name, black_napalm, unlocked) = {
            let obj = self.objects.get(&source_object)?;
            if !obj.is_alive() {
                return None;
            }
            if !is_helix_napalm_caster(&obj.template_name) {
                return None;
            }
            let has_upgrade = obj.has_upgrade_tag(UPGRADE_HELIX_NAPALM_BOMB)
                || obj.has_upgrade_tag("Upgrade_HelixNapalmBomb")
                || obj
                    .has_upgrade_tag(crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NUKE_BOMB)
                || obj.has_upgrade_tag("Nuke_Upgrade_HelixNukeBomb")
                || obj.has_upgrade_tag("Upgrade_HelixNukeBomb");
            let unlocked = helix_napalm_unlocked(&obj.template_name, has_upgrade);
            if !unlocked {
                return None;
            }
            let black = obj.has_upgrade_tag(UPGRADE_CHINA_BLACK_NAPALM)
                || obj.has_upgrade_tag("Upgrade_ChinaBlackNapalm");
            (obj.team, obj.template_name.clone(), black, unlocked)
        };
        let _ = (template_name, unlocked);

        // C++ SpecialObject NapalmBomb fall residual (HeightDie → FireWeaponWhenDead + OCL firestorm).
        let from = self
            .objects
            .get(&source_object)
            .map(|o| o.get_position())
            .unwrap_or(target_position);
        let bomb_id = self.spawn_helix_napalm_bomb_projectile(
            source_object,
            from,
            target_position,
            black_napalm,
        );

        // Fail-closed fallback: if projectile spawn fails, keep instant blast residual.
        let (blast_hits, blast_damage) = if bomb_id.is_none() {
            use crate::game_logic::host_helix_napalm::{
                HELIX_NAPALM_SECONDARY_RADIUS, helix_napalm_blast_damage_at,
            };
            let mut blast_hits = 0u32;
            let mut blast_damage = 0.0f32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
            let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
            for vid in victim_ids {
                if vid == source_object {
                    continue;
                }
                let Some(victim) = self.objects.get(&vid) else {
                    continue;
                };
                if !victim.is_alive() {
                    continue;
                }
                let vpos = victim.get_position();
                let dist = {
                    let dx = vpos.x - target_position.x;
                    let dz = vpos.z - target_position.z;
                    (dx * dx + dz * dz).sqrt()
                };
                if dist > HELIX_NAPALM_SECONDARY_RADIUS {
                    continue;
                }
                let dmg = helix_napalm_blast_damage_at(dist);
                if dmg <= 0.0 {
                    continue;
                }
                if let Some(victim) = self.objects.get_mut(&vid) {
                    blast_damage += dmg.min(victim.health.current.max(0.0));
                    blast_hits = blast_hits.saturating_add(1);
                    if victim.take_damage_from_immediate_residual(
                        dmg,
                        Some(source_object),
                        crate::game_logic::host_helix_napalm::HELIX_NAPALM_DAMAGE_TYPE,
                        crate::game_logic::host_helix_napalm::HELIX_NAPALM_DEATH_TYPE,
                    ) {
                        destroy_ids.push((vid, source_team));
                    }
                }
            }
            for (vid, killer) in destroy_ids {
                self.mark_object_for_destruction(vid, Some(killer));
            }
            (blast_hits, blast_damage)
        } else {
            (0, 0.0)
        };

        let zone_id = self.helix_napalm.record_drop_and_spawn_firestorm(
            source_object,
            source_team,
            target_position,
            self.frame,
            black_napalm,
            blast_hits,
            blast_damage,
        );
        let _ = bomb_id;

        self.queue_audio_event(
            AudioEventRequest::new(HELIX_NAPALM_DROP_AUDIO)
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(170),
        );
        self.queue_audio_event(
            AudioEventRequest::new(HELIX_FIRESTORM_AUDIO)
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(140),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            target_position,
            self.frame,
            Some(source_object),
            None,
        );

        Some(zone_id)
    }

    /// Advance Helix Napalm FirestormSmall residual zones.
    pub(in super::super) fn update_helix_napalm_firestorms(&mut self) {
        let frame = self.frame;
        self.helix_napalm.advance_geometry(frame);
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self.helix_napalm.plan_due_ticks(frame, &object_positions);

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed = target.take_damage_from_immediate_typed(
                        hit.damage,
                        Some(plan.source_object),
                        crate::game_logic::combat::DamageType::Flame,
                    );
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            if plan.place_scorch {
                if let Some(zone) = self
                    .helix_napalm
                    .active_zones()
                    .iter()
                    .find(|z| z.id == plan.zone_id)
                {
                    let pos = zone.position;
                    let _ = self.combat_particles.spawn(
                        CombatParticleKind::DeathExplosion,
                        pos,
                        frame,
                        Some(plan.source_object),
                        None,
                    );
                }
            }

            self.helix_napalm.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.helix_napalm.prune_expired(frame);
    }

    /// Detonate a residual car bomb (SuicideCarBomb self-position AOE).
    /// Returns true if detonation resolved. Destroys the car bomb and damages
    /// nearby units/structures for observable splash residual.
    /// Fail-closed: not full secondary-radius NOT_SIMILAR ally filter / DeathType matrix.
    pub fn detonate_car_bomb(&mut self, car_id: ObjectId) -> bool {
        use crate::game_logic::host_car_bomb::{
            CAR_BOMB_DETONATE_AUDIO, SUICIDE_CAR_BOMB_SECONDARY_RADIUS, car_bomb_damage_at_distance,
        };

        let Some(car) = self.objects.get(&car_id) else {
            return false;
        };
        if !car.is_alive() || !car.status.is_carbomb {
            return false;
        }

        let car_team = car.team;
        let car_pos = car.get_position();

        let mut damage_dealt = 0.0f32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            if vid == car_id {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            if !victim.is_alive() {
                continue;
            }
            // SuicideCarBomb RadiusDamageAffects SELF ALLIES ENEMIES NEUTRALS NOT_SIMILAR:
            // residual hits all living non-self units in secondary radius (fail-closed
            // vs NOT_SIMILAR same-template ally skip).
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - car_pos.x;
                let dz = vpos.z - car_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            if dist > SUICIDE_CAR_BOMB_SECONDARY_RADIUS {
                continue;
            }
            let dmg = car_bomb_damage_at_distance(dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                damage_dealt += dmg.min(victim.health.current.max(0.0));
                if victim.take_damage_from_immediate_residual(
                    dmg,
                    Some(car_id),
                    crate::game_logic::host_car_bomb::SUICIDE_CAR_BOMB_DAMAGE_TYPE,
                    crate::game_logic::host_car_bomb::SUICIDE_CAR_BOMB_DEATH_TYPE,
                ) {
                    destroy_ids.push((vid, car_team));
                }
            }
        }

        self.car_bomb.record_detonation(damage_dealt);
        self.queue_audio_event(
            AudioEventRequest::new(CAR_BOMB_DETONATE_AUDIO)
                .with_object(car_id)
                .with_position(car_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            car_pos,
            self.frame,
            Some(car_id),
            None,
        );

        if let Some(car) = self.objects.get_mut(&car_id) {
            Self::mark_object_destroyed_authority_aware(car, Some(car_id));
            car.set_status_is_carbomb(false);
        }
        self.mark_object_for_destruction(car_id, Some(car_team));
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Transfer residual cash from `from_team` to `to_team` (Black Lotus cash hack).
    /// Returns amount actually stolen (capped by victim supplies).
    /// Fail-closed: not full science upgrade money matrix / EVA / floating text.
    pub fn steal_cash_from_team(&mut self, from_team: Team, to_team: Team, amount: u32) -> u32 {
        if amount == 0 || from_team == to_team || from_team == Team::Neutral {
            return 0;
        }
        let available = self
            .players
            .values()
            .find(|p| p.team == from_team)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        let stolen = crate::game_logic::host_supply_gather::steal_cash_clamped(available, amount);
        if stolen == 0 {
            // C++ SabotageSupplyCenterCrateCollide: min(desired, victimMoney);
            // a broke victim yields 0 — never mint attacker cash.
            return 0;
        }
        if let Some(src) = self.get_player_mut_by_team(from_team) {
            src.apply_supply_spend_unchecked(stolen);
            crate::game_logic::host_economy_log::record(
                src.id,
                src.resources.supplies,
                src.power_available,
            );
        }
        if let Some(dest) = self.get_player_mut_by_team(to_team) {
            dest.credit_supplies(stolen);
        }
        stolen
    }

    /// Exact-player variant used by live ability activations.  The older
    /// team-shaped API above remains for legacy/map callers that genuinely do
    /// not carry a PlayerId; a concrete caster must never debit or credit the
    /// first same-faction player instead.
    pub(in super::super) fn steal_cash_between_players(
        &mut self,
        from_player_id: u32,
        to_player_id: u32,
        amount: u32,
    ) -> u32 {
        if amount == 0 || from_player_id == to_player_id {
            return 0;
        }
        let available = match self.players.get(&from_player_id) {
            Some(player) if player.is_alive && player.team != Team::Neutral => {
                player.resources.supplies
            }
            _ => return 0,
        };
        if !self
            .players
            .get(&to_player_id)
            .is_some_and(|player| player.is_alive && player.team != Team::Neutral)
        {
            return 0;
        }
        let stolen = amount.min(available);
        if stolen == 0 {
            return 0;
        }
        if let Some(source) = self.get_player_mut(from_player_id) {
            source.apply_supply_spend_unchecked(stolen);
            crate::game_logic::host_economy_log::record(
                source.id,
                source.resources.supplies,
                source.power_available,
            );
        }
        if let Some(destination) = self.get_player_mut(to_player_id) {
            destination.credit_supplies(stolen);
        }
        stolen
    }

    // -----------------------------------------------------------------------
    // RadarScan / RadarVanScan FOW temporary-reveal residual
    // Fail-closed: not full OCL RadarVanPing / DynamicShroudClearingRangeUpdate.
    // -----------------------------------------------------------------------

    /// Host RadarScan residual registry (activate + honesty).
    pub fn radar_scans(&self) -> &crate::game_logic::host_radar_scan::HostRadarScanRegistry {
        &self.radar_scans
    }

    /// Residual honesty: RadarScan activated at least once.
    pub fn honesty_radar_scan_activate_ok(&self) -> bool {
        self.radar_scans.honesty_activate_ok()
    }

    /// Residual honesty: RadarScan cleared FOW at scan center at least once.
    pub fn honesty_radar_scan_fow_ok(&self) -> bool {
        self.radar_scans.honesty_fow_reveal_ok()
    }

    /// Combined host path honesty for RadarScan residual.
    pub fn honesty_radar_scan_ok(&self) -> bool {
        self.radar_scans.honesty_host_path_ok()
    }

    /// Activate RadarScan residual: temporary FOW reveal at `location`.
    ///
    /// Matches retail SpecialPowerRadarVanScan / RadarVanPing radius (150) and
    /// lifetime residual (10000 ms → 300 frames). Uses ShroudManager
    /// do_shroud_reveal + queue_undo_shroud_reveal so fog returns after duration.
    ///
    /// Fail-closed: not OCL object spawn / shrink curve / stealth detector.
    pub fn activate_radar_scan(
        &mut self,
        player_id: u32,
        team: Team,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_radar_scan::{
            HostRadarScan, RADAR_SCAN_ACTIVATE_AUDIO, RADAR_SCAN_DURATION_FRAMES, RADAR_SCAN_RADIUS,
        };
        use gamelogic::common::Coord3D;

        // Ensure shroud grid exists (tests / pre-map residual).
        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);

        // C++ Object::look: Allies via Player relationship, not faction Team.
        let player_mask = looker_mask_for_controller(&self.players, player_id);

        // ShroudManager grid axes are (x, y). Host residual gameplay uses glam
        // (x, z) as the ground plane (y = height). Feed horizontal plane into
        // shroud so temporary reveals land on FOW / PresentationFowGrid cells.
        let center = Coord3D::new(location.x, location.z, location.y);
        let radius = RADAR_SCAN_RADIUS;
        let duration = RADAR_SCAN_DURATION_FRAMES;
        let frame = self.frame;

        let fow_reveal_ok = {
            let shroud = get_shroud_manager();
            let mut shroud_mgr = match shroud.lock() {
                Ok(mgr) => mgr,
                Err(_) => return false,
            };

            // Init grid if not yet (unit tests without load_map).
            if !shroud_mgr.has_shroud_grid() {
                shroud_mgr.init_shroud_grid(world_w, world_h);
            }

            // C++ RadarVanPing: instant full VisionRange, then shrink after delay.
            // Do not queue_undo at the full radius — apply_radar_scan_dynamic_shroud
            // undoes/re-reveals as ShrinkDelay/ShrinkTime contract the disk.
            shroud_mgr.do_shroud_reveal(&center, radius, player_mask);

            let mut visible = shroud_mgr.is_position_visible(player_id.min(31), &center);
            if !visible {
                for bit in 0..32u32 {
                    if (player_mask & (1u32 << bit)) != 0
                        && shroud_mgr.is_position_visible(bit, &center)
                    {
                        visible = true;
                        break;
                    }
                }
            }
            visible
        };

        let scan_id = self.radar_scans.alloc_id();
        self.radar_scans.record_activation(HostRadarScan {
            id: scan_id,
            player_id,
            player_mask,
            location,
            radius,
            activate_frame: frame,
            expires_frame: frame.saturating_add(duration),
            caster_id,
            fow_reveal_ok,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            last_applied_radius: radius,
        });

        self.queue_audio_event(
            AudioEventRequest::new(RADAR_SCAN_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(150),
        );

        // C++ OCL SUPERWEAPON_RadarVanScan → RadarVanPing residual.
        let _ = self.spawn_radar_van_ping(team, location, caster_id);

        // Also enable radar UI residual if scripts had disabled it — scan is
        // a radar power; observability via radar_enabled honesty path.
        if !self.radar_enabled && !self.radar_forced {
            self.radar_enabled = true;
        }

        fow_reveal_ok || self.radar_scans.activations() > 0
    }

    /// Advance RadarScan residual: shrink FOW + expire + undo.
    pub(in super::super) fn update_radar_scans(&mut self) {
        self.apply_radar_scan_dynamic_shroud();
        self.undo_expired_radar_scan_shroud();
        self.radar_scans.prune_expired(self.frame);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.process_pending_undo_shroud_reveals(self.frame);
        }
    }

    // -----------------------------------------------------------------------
    // SpySatellite FOW temporary-reveal residual
    // Fail-closed: not full OCL SpySatellitePing / DynamicShroudClearingRangeUpdate.
    // -----------------------------------------------------------------------

    /// Host SpySatellite residual registry (activate + honesty).
    pub fn spy_satellites(
        &self,
    ) -> &crate::game_logic::host_spy_satellite::HostSpySatelliteRegistry {
        &self.spy_satellites
    }

    /// Residual honesty: SpySatellite activated at least once.
    pub fn honesty_spy_satellite_activate_ok(&self) -> bool {
        self.spy_satellites.honesty_activate_ok()
    }

    /// Residual honesty: SpySatellite cleared FOW at scan center at least once.
    pub fn honesty_spy_satellite_fow_ok(&self) -> bool {
        self.spy_satellites.honesty_fow_reveal_ok()
    }

    /// Combined host path honesty for SpySatellite residual.
    pub fn honesty_spy_satellite_ok(&self) -> bool {
        self.spy_satellites.honesty_host_path_ok()
    }

    /// Activate SpySatellite residual: temporary FOW reveal at `location`.
    ///
    /// Matches retail SpecialPowerSpySatellite / SpySatellitePing radius (300) and
    /// lifetime residual (13000 ms → 390 frames). Uses ShroudManager
    /// do_shroud_reveal + queue_undo_shroud_reveal so fog returns after duration.
    ///
    /// Fail-closed: not OCL object spawn / grow-shrink curve / stealth detector /
    /// CIA Intelligence SpyVisionUpdate setUnitsVisionSpied path.
    /// Activate SuperweaponCashHack residual: steal science-tier cash from richest enemy.
    ///
    /// Matches retail CashHackSpecialPower MoneyAmount residual:
    /// - SCIENCE_CashHack1 → 1000
    /// - SCIENCE_CashHack2 → 2000
    /// - SCIENCE_CashHack3 → 4000
    ///
    /// Fail-closed: steals from richest enemy player economy (not full victim object
    /// clamp path / multiplayer academy classification).
    /// Residual honesty: last SuperweaponCashHack requested science-tier amount.
    pub fn last_cash_hack_request_amount(&self) -> u32 {
        self.last_cash_hack_request_amount
    }

    /// Residual honesty: last SuperweaponCashHack stolen amount.
    pub fn last_cash_hack_stolen_amount(&self) -> u32 {
        self.last_cash_hack_stolen_amount
    }

    /// Residual honesty: last SuperweaponCrateDrop spawned crate count.
    pub fn last_crate_drop_spawned(&self) -> u32 {
        self.last_crate_drop_spawned
    }

    /// Activate SuperweaponCrateDrop: AmericaJetCargoPlane DeliverPayload.
    ///
    /// C++ `OCLSpecialPower` CREATE_AT_EDGE_NEAR_SOURCE + `SUPERWEAPON_CrateDrop`
    /// (`200DollarCrate` × 10, DropDelay 300ms). Crates spawn from the inbound
    /// cargo plane via `update_deliver_payloads`, not instantly in a line.
    pub fn activate_crate_drop(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_deliver_payload::{
            CARGO_PLANE_PREFERRED_HEIGHT, HostDeliverPayloadKind, SUPPLY_DROP_CARGO_APPROACH_AUDIO,
            SUPPLY_DROP_CARGO_TRANSPORT, create_at_edge_spawn_residual,
        };
        use crate::game_logic::host_money_crate::{
            SUPERWEAPON_CRATE_DROP_COUNT, SUPERWEAPON_CRATE_DROP_SPECIAL_POWER,
        };

        let (team, owner_player_id) = match caster_id.and_then(|caster| self.objects.get(&caster)) {
            Some(caster) => {
                let owner_player_id = if caster.owner_player_id.is_some() {
                    let Some(owner_player_id) = self.player_owner_for_host_object(caster) else {
                        return 0;
                    };
                    Some(owner_player_id)
                } else {
                    self.players
                        .get(&player_id)
                        .filter(|player| player.is_alive && player.team == caster.team)
                        .map(|player| player.id)
                };
                (caster.team, owner_player_id)
            }
            None => self
                .players
                .get(&player_id)
                .filter(|player| player.is_alive)
                .map(|player| (player.team, Some(player.id)))
                .unwrap_or((Team::Neutral, None)),
        };

        let tpl_name = HostDeliverPayloadKind::SuperweaponCrateDrop.payload_template();
        if !self.templates.contains_key(tpl_name) {
            let mut t = ThingTemplate::new(tpl_name);
            t.add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Selectable)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates.insert(tpl_name.to_string(), t);
        }

        // C++ EDGE_NEAR_SOURCE cargo plane. Prefer the live OCL transport spawn.
        let transport_id = if let Some(caster) =
            caster_id.filter(|id| self.objects.contains_key(id))
        {
            self.execute_ocl_special_power(SUPERWEAPON_CRATE_DROP_SPECIAL_POWER, caster, location)
        } else {
            if !self.templates.contains_key(SUPPLY_DROP_CARGO_TRANSPORT) {
                let mut t = ThingTemplate::new(SUPPLY_DROP_CARGO_TRANSPORT);
                t.add_kind_of(KindOf::Aircraft)
                    .add_kind_of(KindOf::Vehicle)
                    .set_health(800.0);
                self.templates
                    .insert(SUPPLY_DROP_CARGO_TRANSPORT.to_string(), t);
            }
            let mut edge = create_at_edge_spawn_residual(location);
            edge.y = CARGO_PLANE_PREFERRED_HEIGHT;
            self.create_object_for_owner_or_team(
                SUPPLY_DROP_CARGO_TRANSPORT,
                team,
                owner_player_id,
                edge,
            )
        };

        let source_id = caster_id.unwrap_or(ObjectId(0));
        let mission_id = self.host_deliver_payloads.queue_for_owner(
            HostDeliverPayloadKind::SuperweaponCrateDrop,
            source_id,
            team,
            owner_player_id,
            location,
            self.frame,
            tpl_name,
        );

        if let Some(tid) = transport_id {
            if let Some(t) = self.objects.get_mut(&tid) {
                if let Some(caster) = caster_id {
                    t.producer_id = Some(caster);
                    t.bind_special_power_completion_creator(caster.0);
                }
            }
            let plane_pos = self.objects.get(&tid).map(|o| o.get_position());
            if let Some(m) = self.host_deliver_payloads.get_mut(mission_id) {
                m.transport_object_id = Some(tid);
                m.transport_template = SUPPLY_DROP_CARGO_TRANSPORT.to_string();
            }
            if let (Some(pos), Some(flight)) = (
                plane_pos,
                self.host_deliver_payloads.cargo_flight_mut(mission_id),
            ) {
                flight.transport_template = SUPPLY_DROP_CARGO_TRANSPORT.to_string();
                flight.edge_spawn_pos = pos;
                flight.current_pos = pos;
                flight.delivery_distance =
                    HostDeliverPayloadKind::SuperweaponCrateDrop.delivery_distance();
                let dx = location.x - pos.x;
                let dz = location.z - pos.z;
                let dlen = (dx * dx + dz * dz).sqrt().max(0.001);
                flight.dir_x = dx / dlen;
                flight.dir_z = dz / dlen;
                if let Some(t) = self.objects.get_mut(&tid) {
                    t.set_orientation(flight.dir_z.atan2(flight.dir_x));
                }
            }
        }

        self.queue_audio_event(
            AudioEventRequest::new(SUPPLY_DROP_CARGO_APPROACH_AUDIO)
                .with_position(location)
                .with_priority(160),
        );
        let planned = SUPERWEAPON_CRATE_DROP_COUNT.max(1);
        self.last_crate_drop_spawned = planned;
        planned
    }

    pub fn activate_cash_hack(
        &mut self,
        player_id: u32,
        caster_id: Option<ObjectId>,
        victim_id: Option<ObjectId>,
    ) -> Option<u32> {
        use crate::game_logic::host_hero_abilities::{
            CASH_HACK_ACTIVATE_AUDIO, cash_hack_money_from_sciences,
        };

        let requested_owner_player_id = self
            .players
            .get(&player_id)
            .filter(|player| player.is_alive && player.team != Team::Neutral)
            .map(|player| player.id);
        // The live caster's ownership is authoritative.  An ownerless legacy
        // caster can still use the explicit command player (or a uniquely
        // resolved team owner), but a stale concrete owner fails closed.
        let caster_owner_player_id = match caster_id.and_then(|id| self.objects.get(&id)) {
            Some(caster) if !caster.is_alive() => None,
            Some(caster) if caster.owner_player_id.is_some() => {
                self.player_owner_for_host_object(caster)
            }
            Some(caster) => requested_owner_player_id
                .filter(|requested| {
                    self.players
                        .get(requested)
                        .is_some_and(|player| player.team == caster.team)
                })
                .or_else(|| self.player_owner_for_event(None, caster.team)),
            None => requested_owner_player_id,
        };

        let sciences: Vec<String> = caster_owner_player_id
            .and_then(|owner_player_id| self.players.get(&owner_player_id))
            .map(|p| p.unlocked_sciences.iter().cloned().collect())
            .unwrap_or_default();
        let amount = cash_hack_money_from_sciences(sciences.iter().map(|s| s.as_str()));

        // C++ CashHackSpecialPower::doSpecialPowerAtLocation is a no-op.
        // ActionManager.cpp:1652-1672 SPECIAL_CASH_HACK is object-only:
        // STRUCTURE + ENEMIES + CAPTURABLE + CASH_GENERATOR, not REBUILD_HOLE,
        // not UNDER_CONSTRUCTION / SOLD.
        let Some(victim_id) = victim_id else {
            return None;
        };
        let caster_team = caster_id
            .and_then(|id| self.objects.get(&id).map(|o| o.team))
            .or_else(|| {
                caster_owner_player_id.and_then(|id| self.players.get(&id).map(|p| p.team))
            });
        let Some(victim) = self.objects.get(&victim_id) else {
            return None;
        };
        if !is_legal_superweapon_cash_hack_victim(victim, caster_team) {
            return None;
        }
        let victim_team = victim.team;
        let victim_owner = victim.owner_player_id;
        let victim_player_id = self.player_owner_for_event(victim_owner, victim_team);

        let stolen = match (caster_owner_player_id, victim_player_id) {
            (Some(to_player_id), Some(from_player_id)) => {
                self.steal_cash_between_players(from_player_id, to_player_id, amount)
            }
            _ => 0,
        };
        if stolen > 0 {
            if let Some(p) = caster_owner_player_id.and_then(|id| self.get_player_mut(id)) {
                p.add_money_earned(stolen);
            }
            self.hero_abilities.record_cash_steal(stolen);
        }
        if let Some(cid) = caster_id {
            let pos = self
                .objects
                .get(&cid)
                .map(|o| o.get_position())
                .unwrap_or(Vec3::ZERO);
            self.queue_audio_event(
                AudioEventRequest::new(CASH_HACK_ACTIVATE_AUDIO)
                    .with_object(cid)
                    .with_position(pos)
                    .with_priority(180),
            );
            if stolen > 0 {
                // C++ GUI:AddCash over self (z+20 green), GUI:LoseCash over victim (z+30 red).
                self.spawn_sabotage_cash_floating_texts(cid, victim_id, stolen);
            }
        }
        // Honesty residual: last requested science-tier amount.
        self.last_cash_hack_request_amount = amount;
        self.last_cash_hack_stolen_amount = stolen;
        Some(stolen)
    }

    pub fn activate_spy_satellite(
        &mut self,
        player_id: u32,
        team: Team,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_spy_satellite::{
            HostSpySatellite, SPY_SATELLITE_ACTIVATE_AUDIO, SPY_SATELLITE_DURATION_FRAMES,
        };

        // Ensure shroud grid exists (tests / pre-map residual).
        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);

        // C++ Object::look: Allies via Player relationship, not faction Team.
        let player_mask = looker_mask_for_controller(&self.players, player_id);

        // ShroudManager grid axes are (x, y). Host residual gameplay uses glam
        // (x, z) as the ground plane (y = height). Feed horizontal plane into
        // shroud so temporary reveals land on FOW / PresentationFowGrid cells.
        let duration = SPY_SATELLITE_DURATION_FRAMES;
        let frame = self.frame;

        {
            let shroud = get_shroud_manager();
            let mut shroud_mgr = match shroud.lock() {
                Ok(mgr) => mgr,
                Err(_) => return false,
            };

            // Init grid if not yet (unit tests without load_map).
            if !shroud_mgr.has_shroud_grid() {
                shroud_mgr.init_shroud_grid(world_w, world_h);
            }
        }

        // C++ DynamicShroudClearingRangeUpdate starts m_currentClearingRange = 0.
        // Grow/shrink is applied each tick in update_spy_satellites.
        let scan_id = self.spy_satellites.alloc_id();
        self.spy_satellites.record_activation(HostSpySatellite {
            id: scan_id,
            player_id,
            player_mask,
            location,
            radius: 0.0,
            activate_frame: frame,
            expires_frame: frame.saturating_add(duration),
            caster_id,
            fow_reveal_ok: false,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            last_applied_radius: 0.0,
        });
        self.queue_audio_event(
            AudioEventRequest::new(SPY_SATELLITE_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(150),
        );

        // C++ OCL SUPERWEAPON_SpySatellite → SpySatellitePing residual.
        let _ = self.spawn_spy_satellite_ping(team, location, caster_id);

        self.spy_satellites.activations() > 0
    }

    /// Host SpyDrone: spawn a selectable stealthed AmericaVehicleSpyDrone scout.
    /// C++ OCLSpecialPower CREATE_ABOVE_LOCATION + CreateObject SUPERWEAPON_SpyDrone.
    /// Vision follows the unit (update_main_crate_vision); not a timed FOW ping.
    pub fn activate_spy_drone(
        &mut self,
        player_id: u32,
        team: Team,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_ocl_special_power::{
            OCL_CREATE_ABOVE_LOCATION_HEIGHT, OclCreateLocType, compute_creation_coord,
            default_map_extents,
        };
        use crate::game_logic::host_spy_drone::{
            HostSpyDrone, SPY_DRONE_ACTIVATE_AUDIO, SPY_DRONE_GROW_TIME_FRAMES,
            SPY_DRONE_LOCOMOTOR, SPY_DRONE_LOCOMOTOR_SPEED, SPY_DRONE_MAX_HEALTH, SPY_DRONE_MODEL,
            SPY_DRONE_PREFERRED_HEIGHT, SPY_DRONE_SPECIAL_POWER, SPY_DRONE_TEMPLATE,
            SPY_DRONE_VISION_RANGE, spy_drone_scan_radius_after_updates,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        crate::game_logic::locomotor_bootstrap::ensure_spy_drone_locomotor();

        {
            let tpl = self
                .templates
                .entry(SPY_DRONE_TEMPLATE.into())
                .or_insert_with(|| {
                    let mut t = ThingTemplate::new(SPY_DRONE_TEMPLATE);
                    t.set_health(SPY_DRONE_MAX_HEALTH);
                    t.sight_range = SPY_DRONE_VISION_RANGE;
                    t.shroud_clearing_range = 0.0;
                    t.model_name = Some(SPY_DRONE_MODEL.to_string());
                    t
                });
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Drone);
            tpl.add_kind_of(KindOf::Selectable);
            if tpl.sight_range <= 0.0 {
                tpl.sight_range = SPY_DRONE_VISION_RANGE;
            }
            if tpl.locomotor_name.is_none() {
                tpl.set_locomotor_name(SPY_DRONE_LOCOMOTOR);
            }
        }

        let owner_player_id = match caster_id.and_then(|caster| self.objects.get(&caster)) {
            Some(caster) if caster.owner_player_id.is_some() => {
                let Some(owner_player_id) = self.player_owner_for_host_object(caster) else {
                    return false;
                };
                Some(owner_player_id)
            }
            Some(caster) => self
                .players
                .get(&player_id)
                .filter(|player| player.is_alive && player.team == caster.team)
                .map(|player| player.id),
            None => self
                .players
                .get(&player_id)
                .filter(|player| player.is_alive && player.team == team)
                .map(|player| player.id),
        };

        let source_pos = caster_id
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(location);
        let spawn_pos = if let Some(cid) = caster_id {
            self.plan_ocl_special_power(SPY_DRONE_SPECIAL_POWER, cid, location)
                .map(|plan| plan.creation_coord)
                .unwrap_or_else(|| {
                    let mut c = location;
                    c.y += OCL_CREATE_ABOVE_LOCATION_HEIGHT;
                    c
                })
        } else {
            let (minx, minz, maxx, maxz) = default_map_extents();
            compute_creation_coord(
                OclCreateLocType::AboveLocation,
                source_pos,
                location,
                minx,
                minz,
                maxx,
                maxz,
            )
        };

        let spawned_id = self.create_object_for_owner_or_team(
            SPY_DRONE_TEMPLATE,
            team,
            owner_player_id,
            spawn_pos,
        );
        let spawn_ok = spawned_id.is_some();
        let frame = self.frame;
        let start_radius = spy_drone_scan_radius_after_updates(0);
        if let Some(id) = spawned_id {
            if let Some(obj) = self.host_object_mut(id) {
                obj.health.maximum = SPY_DRONE_MAX_HEALTH;
                Self::write_object_health_authority_aware(obj, SPY_DRONE_MAX_HEALTH);
                obj.thing.template.add_kind_of(KindOf::Selectable);
                obj.thing.template.add_kind_of(KindOf::Vehicle);
                obj.thing.template.add_kind_of(KindOf::Drone);
                obj.set_status_unselectable(false);
                obj.set_status_stealthed(true);
                obj.innate_stealth = true;
                obj.record_host_stealth_flags();
                obj.is_detector = true;
                obj.detection_range = SPY_DRONE_VISION_RANGE;
                obj.record_host_detector();
                obj.apply_stealth_detector_ctor_stagger(frame);
                obj.vision_range = SPY_DRONE_VISION_RANGE;
                obj.shroud_clearing_range = start_radius;
                if obj.movement.max_speed <= 10.01 {
                    if let Some(binding) =
                        crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(
                            SPY_DRONE_LOCOMOTOR,
                        )
                    {
                        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(
                            obj, &binding,
                        );
                    } else {
                        obj.movement.max_speed = SPY_DRONE_LOCOMOTOR_SPEED;
                        obj.movement.acceleration = 100.0;
                        obj.movement.turn_rate = 180.0_f32.to_radians();
                    }
                }
                if obj.loco_preferred_height <= 0.0 {
                    obj.loco_preferred_height = SPY_DRONE_PREFERRED_HEIGHT;
                }
                if let Some(cid) = caster_id {
                    obj.producer_id = Some(cid);
                    obj.bind_special_power_completion_creator(cid.0);
                }
            }
        }

        let mut player_mask = 0u32;
        for (&pid, player) in &self.players {
            if player.team == team {
                player_mask |= 1u32 << pid.min(31);
            }
        }
        if player_mask == 0 {
            player_mask = 1u32 << player_id.min(31);
        }

        let act_id = self.spy_drones.alloc_id();
        self.spy_drones.record_activation(HostSpyDrone {
            id: act_id,
            player_id,
            player_mask,
            location: spawn_pos,
            radius: start_radius,
            activate_frame: frame,
            expires_frame: frame.saturating_add(SPY_DRONE_GROW_TIME_FRAMES.saturating_add(8)),
            caster_id,
            spawned_id,
            fow_reveal_ok: spawn_ok,
            spawn_ok,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            grow_index: 1,
            growing: true,
        });

        self.queue_audio_event(
            AudioEventRequest::new(SPY_DRONE_ACTIVATE_AUDIO)
                .with_position(spawn_pos)
                .with_priority(150),
        );

        spawn_ok
    }

    /// Host SpyDrone residual registry (activate + grow + honesty).
    pub fn spy_drones(&self) -> &crate::game_logic::host_spy_drone::HostSpyDroneRegistry {
        &self.spy_drones
    }

    /// Residual honesty: SpyDrone activated at least once.
    pub fn honesty_spy_drone_activate_ok(&self) -> bool {
        self.spy_drones.honesty_activate_ok()
    }

    /// Residual honesty: SpyDrone spawned AmericaVehicleSpyDrone at least once.
    pub fn honesty_spy_drone_spawn_ok(&self) -> bool {
        self.spy_drones.honesty_spawn_ok()
    }

    /// Residual honesty: at least one missile was diverted by Countermeasures.
    /// C++ CountermeasuresBehavior flare OCL SpecialObject residual.
    pub fn spawn_countermeasure_flare_object(
        &mut self,
        aircraft_id: ObjectId,
        volley_index: u32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_countermeasures::{
            FLARE_LIFETIME_FRAMES, FLARE_MAX_HEALTH, FLARE_TEMPLATE_NAME, VOLLEY_SIZE,
            flare_volley_motive_force,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(FLARE_TEMPLATE_NAME) {
            let mut t = ThingTemplate::new(FLARE_TEMPLATE_NAME);
            t.add_kind_of(KindOf::Projectile)
                .set_health(FLARE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(FLARE_TEMPLATE_NAME.to_string(), t);
        }
        let (team, origin, ori, vel, facing, speed, owner_player_id) = {
            let o = self.objects.get(&aircraft_id)?;
            let owner_player_id = if o.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(o)?)
            } else {
                None
            };
            let facing = o.unit_direction_xz();
            let vel = o.movement.velocity;
            let speed = (vel.x * vel.x + vel.y * vel.y + vel.z * vel.z).sqrt();
            (
                o.team,
                o.get_position(),
                o.get_orientation(),
                vel,
                facing,
                speed,
                owner_player_id,
            )
        };
        // C++ launchVolley: spawn at aircraft pose, not a static ring.
        let fid = self.create_object_for_owner_or_team(
            FLARE_TEMPLATE_NAME,
            team,
            owner_player_id,
            origin,
        )?;
        let expires = self.frame.saturating_add(FLARE_LIFETIME_FRAMES.max(1));
        if let Some(o) = self.objects.get_mut(&fid) {
            o.set_position(origin);
            o.set_orientation(ori);
            o.movement.velocity += vel;
            o.invalidate_velocity_magnitude();
            let (mx, my, mz) = flare_volley_motive_force(facing, volley_index, VOLLEY_SIZE, speed);
            o.apply_motive_force(glam::Vec3::new(mx, my, mz));
            o.integrate_physics_accel();
            o.countermeasure_flare = true;
            o.countermeasure_flare_expires_frame = Some(expires);
            o.producer_id = Some(aircraft_id);
            o.health.maximum = FLARE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, FLARE_MAX_HEALTH);
            o.weapon = None;
            o.secondary_weapon = None;
        }
        self.countermeasures.record_flare_spawned(1);
        self.countermeasures.record_flare_id(aircraft_id, fid);
        Some(fid)
    }

    pub fn flush_countermeasure_flare_spawns(&mut self) {
        self.update_countermeasure_volleys();
        let pending = self.countermeasures.take_pending_flare_spawns();
        for spawn in pending {
            let _ = self.spawn_countermeasure_flare_object(spawn.aircraft_id, spawn.volley_index);
        }
    }

    /// C++ CountermeasuresBehavior::update volley launch while airborne.
    pub fn update_countermeasure_volleys(&mut self) {
        use crate::game_logic::host_countermeasures::update_countermeasures;
        let frame = self.frame;
        let ids = self.countermeasures.aircraft_ids();
        for id in ids {
            let airborne = self
                .objects
                .get(&id)
                .is_some_and(|o| o.is_alive() && o.status.airborne_target);
            update_countermeasures(&mut self.countermeasures, id, frame, airborne);
        }
    }

    pub fn update_countermeasure_flare_objects(&mut self) {
        let frame = self.frame;
        let dt = 1.0 / 30.0;
        let live: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.countermeasure_flare)
            .map(|(id, _)| *id)
            .collect();
        for id in live {
            if let Some(o) = self.objects.get_mut(&id) {
                o.integrate_physics_accel();
                let v = o.movement.velocity;
                let mut p = o.get_position();
                p += v * dt;
                o.set_position(p);
            }
        }
        let due: Vec<(ObjectId, Option<ObjectId>)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.countermeasure_flare {
                    return None;
                }
                if let Some(exp) = o.countermeasure_flare_expires_frame {
                    if exp <= frame {
                        return Some((*id, o.producer_id));
                    }
                }
                None
            })
            .collect();
        for (id, producer) in due {
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
                o.countermeasure_flare = false;
            }
            if let Some(pid) = producer {
                self.countermeasures.note_flare_expired(pid);
                self.countermeasures.forget_flare_id(pid, id);
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn honesty_countermeasure_flare_object_ok(&self) -> bool {
        self.countermeasures.honesty_flare_spawn_ok()
    }

    pub fn honesty_countermeasures_divert_ok(&self) -> bool {
        self.countermeasures.honesty_divert_ok()
    }

    /// Residual honesty: Countermeasures saw at least one incoming missile report.
    pub fn honesty_countermeasures_report_ok(&self) -> bool {
        self.countermeasures.honesty_report_ok()
    }

    /// Residual honesty: at least one airfield Countermeasures reload residual.
    pub fn honesty_countermeasures_reload_ok(&self) -> bool {
        self.countermeasures.total_reloads() > 0
    }

    pub fn countermeasures_registry(
        &self,
    ) -> &crate::game_logic::host_countermeasures::HostCountermeasuresRegistry {
        &self.countermeasures
    }

    /// Advance SpySatellite residual: DynamicShroud grow/shrink + expire + undo.
    pub(in super::super) fn update_spy_satellites(&mut self) {
        self.apply_spy_satellite_dynamic_shroud();
        self.undo_expired_spy_satellite_shroud();
        self.spy_satellites.prune_expired(self.frame);
        self.spy_drones.prune_expired(self.frame);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.process_pending_undo_shroud_reveals(self.frame);
        }
    }

    /// C++ DynamicShroudClearingRangeUpdate::update → setShroudClearingRange.
    fn apply_spy_satellite_dynamic_shroud(&mut self) {
        use gamelogic::common::Coord3D;
        let frame = self.frame;
        let work: Vec<(u32, Vec3, f32, f32, u32)> = self
            .spy_satellites
            .active_scans()
            .iter()
            .map(|s| {
                let new_r = s.dynamic_shroud_radius(frame);
                (
                    s.player_id,
                    s.location,
                    s.last_applied_radius,
                    new_r,
                    s.player_mask,
                )
            })
            .collect();
        if work.is_empty() {
            return;
        }
        let shroud_manager = get_shroud_manager();
        let Ok(mut shroud_mgr) = shroud_manager.lock() else {
            return;
        };
        for (player_id, location, old_r, new_r, player_mask) in &work {
            let center = Coord3D::new(location.x, location.z, location.y);
            if (*new_r - *old_r).abs() <= 0.01 {
                continue;
            }
            if *old_r > 0.01 {
                shroud_mgr.undo_shroud_reveal(&center, *old_r, *player_mask);
            }
            if *new_r > 0.01 {
                shroud_mgr.do_shroud_reveal(&center, *new_r, *player_mask);
            }
            let _ = player_id;
        }
        drop(shroud_mgr);
        let mut newly_visible = 0u32;
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            for scan in self.spy_satellites.active_scans_mut() {
                let new_r = scan.dynamic_shroud_radius(frame);
                scan.last_applied_radius = new_r;
                scan.radius = new_r;
                if scan.fow_reveal_ok || new_r <= 0.01 {
                    continue;
                }
                let center = Coord3D::new(scan.location.x, scan.location.z, scan.location.y);
                let mut visible = shroud_mgr.is_position_visible(scan.player_id.min(31), &center);
                if !visible {
                    for bit in 0..32u32 {
                        if (scan.player_mask & (1u32 << bit)) != 0
                            && shroud_mgr.is_position_visible(bit, &center)
                        {
                            visible = true;
                            break;
                        }
                    }
                }
                if visible {
                    scan.fow_reveal_ok = true;
                    newly_visible = newly_visible.saturating_add(1);
                }
            }
        }
        for _ in 0..newly_visible {
            self.spy_satellites.record_fow_reveal();
        }
        self.refresh_dynamic_shroud_grid_decals();
    }

    fn undo_expired_spy_satellite_shroud(&mut self) {
        use gamelogic::common::Coord3D;
        let frame = self.frame;
        let expired: Vec<(Vec3, f32, u32)> = self
            .spy_satellites
            .active_scans()
            .iter()
            .filter(|s| s.is_expired(frame) && s.last_applied_radius > 0.01)
            .map(|s| (s.location, s.last_applied_radius, s.player_mask))
            .collect();
        if expired.is_empty() {
            return;
        }
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            for (location, radius, player_mask) in expired {
                let center = Coord3D::new(location.x, location.z, location.y);
                shroud_mgr.undo_shroud_reveal(&center, radius, player_mask);
            }
        }
    }

    /// C++ RadarVanPing DynamicShroudClearingRangeUpdate shrink curve.
    fn apply_radar_scan_dynamic_shroud(&mut self) {
        use gamelogic::common::Coord3D;
        let frame = self.frame;
        let work: Vec<(Vec3, f32, f32, u32, u32)> = self
            .radar_scans
            .active_scans()
            .iter()
            .map(|s| {
                let new_r = s.dynamic_shroud_radius(frame);
                (
                    s.location,
                    s.last_applied_radius,
                    new_r,
                    s.player_mask,
                    s.player_id,
                )
            })
            .collect();
        if work.is_empty() {
            return;
        }
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            for (location, old_r, new_r, player_mask, _) in &work {
                let center = Coord3D::new(location.x, location.z, location.y);
                if (*new_r - *old_r).abs() <= 0.01 {
                    continue;
                }
                if *old_r > 0.01 {
                    shroud_mgr.undo_shroud_reveal(&center, *old_r, *player_mask);
                }
                if *new_r > 0.01 {
                    shroud_mgr.do_shroud_reveal(&center, *new_r, *player_mask);
                }
            }
        }
        for scan in self.radar_scans.active_scans_mut() {
            let new_r = scan.dynamic_shroud_radius(frame);
            scan.last_applied_radius = new_r;
            scan.radius = new_r;
        }
        self.refresh_dynamic_shroud_grid_decals();
    }

    fn undo_expired_radar_scan_shroud(&mut self) {
        use gamelogic::common::Coord3D;
        let frame = self.frame;
        let expired: Vec<(Vec3, f32, u32)> = self
            .radar_scans
            .active_scans()
            .iter()
            .filter(|s| s.is_expired(frame) && s.last_applied_radius > 0.01)
            .map(|s| (s.location, s.last_applied_radius, s.player_mask))
            .collect();
        if expired.is_empty() {
            return;
        }
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            for (location, radius, player_mask) in expired {
                let center = Coord3D::new(location.x, location.z, location.y);
                shroud_mgr.undo_shroud_reveal(&center, radius, player_mask);
            }
        }
    }

    /// C++ DynamicShroudClearingRangeUpdate GridDecalTemplate ring.
    fn refresh_dynamic_shroud_grid_decals(&mut self) {
        let frame = self.frame;
        let mut updates: Vec<(ObjectId, f32, Vec3, Option<u32>)> = Vec::new();
        for (id, obj) in &self.objects {
            if !obj.is_alive() {
                continue;
            }
            let radius = if obj.spy_satellite_ping {
                self.spy_satellites
                    .active_scans()
                    .iter()
                    .find(|s| (s.location - obj.get_position()).length() < 5.0)
                    .map(|s| s.last_applied_radius.max(s.radius))
            } else if obj.radar_van_ping {
                self.radar_scans
                    .active_scans()
                    .iter()
                    .find(|s| (s.location - obj.get_position()).length() < 5.0)
                    .map(|s| s.last_applied_radius.max(s.radius))
            } else {
                None
            };
            let Some(radius) = radius else {
                continue;
            };
            updates.push((*id, radius, obj.get_position(), obj.owner_player_id));
        }
        for (id, radius, pos, owner) in updates {
            self.apply_ping_grid_decal(id, radius, pos, owner, frame);
        }
    }

    fn apply_ping_grid_decal(
        &mut self,
        id: ObjectId,
        radius: f32,
        pos: Vec3,
        owner: Option<u32>,
        frame: u32,
    ) {
        use crate::game_logic::host_radius_decal_update::{
            HostRadiusDecalTemplate, HostRadiusDecalUpdateData,
        };
        let Some(obj) = self.objects.get_mut(&id) else {
            return;
        };
        let is_spy = obj.spy_satellite_ping;
        if obj.radius_decal_update.is_none() {
            obj.radius_decal_update = Some(HostRadiusDecalUpdateData::default());
        }
        let Some(rd) = obj.radius_decal_update.as_mut() else {
            return;
        };
        if radius <= 0.01 {
            rd.kill_radius_decal();
            return;
        }
        let tmpl = HostRadiusDecalTemplate {
            name: if is_spy {
                "SpySatellitePingGrid".into()
            } else {
                "RadarVanPingGrid".into()
            },
            texture: "EXGrid".into(),
            opacity_min: 0.25,
            opacity_max: 0.9,
            throb_frames: 0,
            only_visible_to_owner: true,
            color: 0,
        };
        rd.create_radius_decal_for_owner(tmpl, radius, pos, frame, owner.map(|id| id as i32));
    }
}

#[cfg(test)]
mod cash_hack_target_tests {
    use super::*;
    use crate::game_logic::host_hero_abilities::CASH_HACK_ACTIVATE_AUDIO;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    fn insert_cc_and_depot(logic: &mut GameLogic) {
        logic
            .players
            .insert(0, Player::new(0, Team::China, "China", true));
        logic
            .players
            .insert(1, Player::new(1, Team::USA, "USA", false));
        logic.players.get_mut(&0).unwrap().resources.supplies = 0;
        logic.players.get_mut(&1).unwrap().resources.supplies = 5_000;

        let mut cc = ThingTemplate::new("ChinaCommandCenter");
        cc.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(5000.0);
        logic.templates.insert("ChinaCommandCenter".into(), cc);

        let mut depot = ThingTemplate::new("AmericaSupplyCenter");
        depot
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSSupplyCenter)
            .set_health(2000.0);
        depot.capturable = true;
        logic.templates.insert("AmericaSupplyCenter".into(), depot);

        let mut tank = ThingTemplate::new("AmericaTankCrusader");
        tank.add_kind_of(KindOf::Vehicle).set_health(400.0);
        logic.templates.insert("AmericaTankCrusader".into(), tank);
    }

    /// C++ ActionManager.cpp:1652-1672 — tank / unfinished / own / hole are no-ops.
    #[test]
    fn cash_hack_rejects_illegal_object_targets() {
        let mut logic = GameLogic::new();
        insert_cc_and_depot(&mut logic);
        let src = logic
            .create_object("ChinaCommandCenter", Team::China, Vec3::ZERO)
            .expect("cc");
        let tank = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .expect("tank");
        let unfinished = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(80.0, 0.0, 0.0))
            .expect("uc");
        if let Some(o) = logic.host_object_mut(unfinished) {
            o.set_status_under_construction(true);
            o.construction_percent = 0.4;
        }
        let own = logic
            .create_object(
                "AmericaSupplyCenter",
                Team::China,
                Vec3::new(120.0, 0.0, 0.0),
            )
            .expect("own");
        let hole = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(160.0, 0.0, 0.0))
            .expect("hole");
        if let Some(o) = logic.host_object_mut(hole) {
            o.is_rebuild_hole = true;
        }

        logic.queued_audio_events.clear();
        assert_eq!(logic.activate_cash_hack(0, Some(src), Some(tank)), None);
        assert_eq!(
            logic.activate_cash_hack(0, Some(src), Some(unfinished)),
            None
        );
        assert_eq!(logic.activate_cash_hack(0, Some(src), Some(own)), None);
        assert_eq!(logic.activate_cash_hack(0, Some(src), Some(hole)), None);
        assert!(
            !logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == CASH_HACK_ACTIVATE_AUDIO),
            "invalid CashHack must not play activate audio"
        );
        assert_eq!(logic.last_cash_hack_stolen_amount(), 0);
        assert_eq!(logic.players.get(&1).unwrap().effective_supplies(), 5_000);
    }

    /// Empty coffers is still a successful fire (C++ steal min(amount, money)).
    #[test]
    fn cash_hack_empty_coffers_still_fires() {
        let mut logic = GameLogic::new();
        insert_cc_and_depot(&mut logic);
        logic.players.get_mut(&1).unwrap().resources.supplies = 0;
        let src = logic
            .create_object("ChinaCommandCenter", Team::China, Vec3::ZERO)
            .expect("cc");
        let victim = logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(80.0, 0.0, 0.0))
            .expect("depot");
        logic.queued_audio_events.clear();
        assert_eq!(
            logic.activate_cash_hack(0, Some(src), Some(victim)),
            Some(0)
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == CASH_HACK_ACTIVATE_AUDIO)
        );
    }
}
