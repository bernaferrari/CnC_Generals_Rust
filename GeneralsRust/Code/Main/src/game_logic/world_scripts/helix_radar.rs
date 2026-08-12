//! Host scripts `impl GameLogic` — `helix_radar`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! nuclear tanks / booby trap / helix napalm / steal cash / radar / spy
#![allow(unused_imports, non_snake_case)]
use super::super::*;

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
            is_legal_nuclear_death_target, nuclear_tank_death_damage_at,
            nuclear_tank_death_splash_radius, NUCLEAR_TANK_DEATH_AUDIO, SMALL_RADIATION_AUDIO,
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
                if victim.take_damage_from(dmg, Some(tank_id)) {
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
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
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
        let bid = self.create_object_for_owner_or_team(BOOBY_TRAP_OBJECT, team, owner_player_id, pos)?;
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
            booby_trap_damage_at, booby_trap_splash_radius, is_legal_booby_victim, is_planter_ally,
            BOOBY_TRAP_DETONATE_AUDIO,
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
            let is_self = vid == structure_id;
            let combat_kind = victim.is_kind_of(KindOf::Attackable)
                || victim.is_kind_of(KindOf::Structure)
                || victim.is_kind_of(KindOf::Infantry)
                || victim.is_kind_of(KindOf::Vehicle)
                || victim.is_kind_of(KindOf::Aircraft);
            // On death path, structure itself may already be removed — still hit nearby.
            // Geometry-based residual damages units near structure, not the structure host
            // when dying (structure already dead). Fail-closed: skip structure self.
            if !is_legal_booby_victim(
                victim.is_alive(),
                is_self,
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
                if victim.take_damage_from(dmg, Some(plant.planter_id)) {
                    destroy_ids.push((vid, plant.planter_team));
                }
            }
        }

        self.booby_trap
            .record_detonation(hits, via_capture, via_death);
        self.queue_audio_event(
            AudioEventRequest::new(BOOBY_TRAP_DETONATE_AUDIO)
                .with_object(structure_id)
                .with_position(structure_pos)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            structure_pos,
            self.frame,
            Some(structure_id),
            None,
        );

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
        let pid = self.create_object_for_owner_or_team(tpl_name, team, source_owner_player_id, start)?;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.helix_napalm_bomb_projectile = true;
            o.producer_id = Some(source_id);
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
            helix_napalm_unlocked, is_helix_napalm_caster, HELIX_FIRESTORM_AUDIO,
            HELIX_NAPALM_DROP_AUDIO, UPGRADE_CHINA_BLACK_NAPALM, UPGRADE_HELIX_NAPALM_BOMB,
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
                helix_napalm_blast_damage_at, HELIX_NAPALM_SECONDARY_RADIUS,
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
                    if victim.take_damage_from(dmg, Some(source_object)) {
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
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .helix_napalm
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
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
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
            car_bomb_damage_at_distance, CAR_BOMB_DETONATE_AUDIO, SUICIDE_CAR_BOMB_SECONDARY_RADIUS,
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
                if victim.take_damage_from(dmg, Some(car_id)) {
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
        let stolen = amount.min(available);
        if stolen == 0 {
            // No registered victim player cash — still grant residual steal for
            // host tests / maps without economy slots (observable attacker gain).
            if let Some(dest) = self.get_player_mut_by_team(to_team) {
                dest.credit_supplies(amount);
                return amount;
            }
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
    fn steal_cash_between_players(
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

        let mut player_mask = 0u32;
        for (&pid, player) in &self.players {
            if player.team == team {
                player_mask |= 1u32 << pid.min(31);
            }
        }
        if player_mask == 0 {
            // No registered players for team: fall back to commanding player bit.
            player_mask = 1u32 << player_id.min(31);
        }

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

            shroud_mgr.do_shroud_reveal(&center, radius, player_mask);
            shroud_mgr.queue_undo_shroud_reveal(&center, radius, player_mask, duration, frame);

            // Observe FOW: center must be visible for the commanding player.
            let mut visible = shroud_mgr.is_position_visible(player_id.min(31), &center);
            if !visible {
                // Team-shared mask may use a different bit; check any teammate bit.
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
            // Wave 48: RadarVanPing DynamicShroud + StealthDetector residual on activate.
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
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

    /// Advance RadarScan residual: expire bookkeeping + process shroud undos.
    pub(in super::super) fn update_radar_scans(&mut self) {
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

    /// Activate SuperweaponCrateDrop residual: spawn 200DollarCrate × 10 near target.
    ///
    /// Matches retail SUPERWEAPON_CrateDrop payload residual (MoneyProvided 200 × 10).
    /// Fail-closed: scatter spawn + MoneyCrateCollide registration —
    /// not full AmericaJetCargoPlane DeliverPayload flight Object / parachute container.
    pub fn activate_crate_drop(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_money_crate::{
            SUPERWEAPON_CRATE_DROP_ACTIVATE_AUDIO, SUPERWEAPON_CRATE_DROP_COUNT,
            SUPERWEAPON_CRATE_DROP_MONEY, SUPERWEAPON_CRATE_DROP_SPACING,
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

        let tpl_name = "200DollarCrate";
        if !self.templates.contains_key(tpl_name) {
            let mut t = ThingTemplate::new(tpl_name);
            t.add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Selectable)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates.insert(tpl_name.to_string(), t);
        }

        let n = SUPERWEAPON_CRATE_DROP_COUNT.max(1) as usize;
        let mut spawned: u32 = 0;
        for i in 0..n {
            let offset = (i as f32 - (n as f32 - 1.0) * 0.5) * SUPERWEAPON_CRATE_DROP_SPACING;
            let pos = Vec3::new(location.x + offset, location.y + 40.0, location.z);
            if let Some(id) = self.create_object_for_owner_or_team(
                tpl_name,
                team,
                owner_player_id,
                pos,
            ) {
                self.host_money_crates
                    .register(id, SUPERWEAPON_CRATE_DROP_MONEY, false, 0);
                self.host_money_crates.arm_default_deletion(
                    id,
                    self.frame,
                    id.0.wrapping_add(self.frame),
                );
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.apply_crate_parachuting();
                }
                spawned = spawned.saturating_add(1);
            }
        }

        self.queue_audio_event(
            AudioEventRequest::new(SUPERWEAPON_CRATE_DROP_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(160),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::DeathExplosion,
            location,
            self.frame,
            caster_id,
            None,
        );
        self.last_crate_drop_spawned = spawned;
        spawned
    }

    pub fn activate_cash_hack(&mut self, player_id: u32, caster_id: Option<ObjectId>) -> u32 {
        use crate::game_logic::host_hero_abilities::{
            cash_hack_money_from_sciences, CASH_HACK_ACTIVATE_AUDIO,
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

        let mut victim_player_id: Option<u32> = None;
        let mut victim_cash: u32 = 0;
        for (&candidate_player_id, candidate) in &self.players {
            let Some(caster_owner_player_id) = caster_owner_player_id else {
                break;
            };
            if candidate.team == Team::Neutral
                || !candidate.is_alive
                || !matches!(
                    self.player_relationship(caster_owner_player_id, candidate_player_id),
                    gamelogic::common::Relationship::Enemies
                )
            {
                continue;
            }
            let cash = candidate.resources.supplies;
            if victim_player_id.is_none() || cash > victim_cash {
                victim_cash = cash;
                victim_player_id = Some(candidate_player_id);
            }
        }

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
                self.spawn_sabotage_cash_floating_texts(cid, cid, stolen);
            }
        }
        // Honesty residual: last requested science-tier amount.
        self.last_cash_hack_request_amount = amount;
        self.last_cash_hack_stolen_amount = stolen;
        stolen
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
            SPY_SATELLITE_RADIUS,
        };
        use gamelogic::common::Coord3D;

        // Ensure shroud grid exists (tests / pre-map residual).
        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);

        let mut player_mask = 0u32;
        for (&pid, player) in &self.players {
            if player.team == team {
                player_mask |= 1u32 << pid.min(31);
            }
        }
        if player_mask == 0 {
            // No registered players for team: fall back to commanding player bit.
            player_mask = 1u32 << player_id.min(31);
        }

        // ShroudManager grid axes are (x, y). Host residual gameplay uses glam
        // (x, z) as the ground plane (y = height). Feed horizontal plane into
        // shroud so temporary reveals land on FOW / PresentationFowGrid cells.
        let center = Coord3D::new(location.x, location.z, location.y);
        let radius = SPY_SATELLITE_RADIUS;
        let duration = SPY_SATELLITE_DURATION_FRAMES;
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

            shroud_mgr.do_shroud_reveal(&center, radius, player_mask);
            shroud_mgr.queue_undo_shroud_reveal(&center, radius, player_mask, duration, frame);

            // Observe FOW: center must be visible for the commanding player.
            let mut visible = shroud_mgr.is_position_visible(player_id.min(31), &center);
            if !visible {
                // Team-shared mask may use a different bit; check any teammate bit.
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

        let scan_id = self.spy_satellites.alloc_id();
        self.spy_satellites.record_activation(HostSpySatellite {
            id: scan_id,
            player_id,
            player_mask,
            location,
            radius,
            activate_frame: frame,
            expires_frame: frame.saturating_add(duration),
            caster_id,
            fow_reveal_ok,
            // Wave 48: SpySatellitePing DynamicShroud + StealthDetector residual on activate.
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
        });

        self.queue_audio_event(
            AudioEventRequest::new(SPY_SATELLITE_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(150),
        );

        // C++ OCL SUPERWEAPON_SpySatellite → SpySatellitePing residual.
        let _ = self.spawn_spy_satellite_ping(team, location, caster_id);

        fow_reveal_ok || self.spy_satellites.activations() > 0
    }

    /// Host SpyDrone residual: spawn AmericaVehicleSpyDrone + temporary FOW reveal.
    /// Fail-closed: not full DynamicShroud grow/shrink / stealth module matrix.
    pub fn activate_spy_drone(
        &mut self,
        player_id: u32,
        team: Team,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_spy_drone::{
            HostSpyDrone, SPY_DRONE_ACTIVATE_AUDIO, SPY_DRONE_FOW_DURATION_FRAMES,
            SPY_DRONE_MAX_HEALTH, SPY_DRONE_RADIUS, SPY_DRONE_TEMPLATE, SPY_DRONE_VISION_RANGE,
        };
        use crate::game_logic::{KindOf, ThingTemplate};
        use gamelogic::common::Coord3D;

        // Ensure template residual exists for spawn.
        if !self.templates.contains_key(SPY_DRONE_TEMPLATE) {
            let mut tpl = ThingTemplate::new(SPY_DRONE_TEMPLATE);
            tpl.set_health(SPY_DRONE_MAX_HEALTH);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Drone);
            // Vision residual for FOW / presentation.
            tpl.sight_range = SPY_DRONE_VISION_RANGE;
            tpl.model_name = Some(crate::game_logic::host_spy_drone::SPY_DRONE_MODEL.to_string());
            self.templates.insert(SPY_DRONE_TEMPLATE.into(), tpl);
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
        let spawned_id = self.create_object_for_owner_or_team(
            SPY_DRONE_TEMPLATE,
            team,
            owner_player_id,
            location,
        );
        let spawn_ok = spawned_id.is_some();
        if let Some(id) = spawned_id {
            if let Some(obj) = self.host_object_mut(id) {
                obj.health.maximum = SPY_DRONE_MAX_HEALTH;
                Self::write_object_health_authority_aware(obj, SPY_DRONE_MAX_HEALTH);
                // Innate stealth residual (StealthUpdate InnateStealth=Yes).
                obj.set_status_stealthed(true);
                obj.innate_stealth = true;
                obj.record_host_stealth_flags();
                obj.is_detector = true;
                obj.record_host_detector();
                obj.detection_range = SPY_DRONE_VISION_RANGE;
                obj.record_host_detector();
            }
        }

        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);
        let mut player_mask = 0u32;
        for (&pid, player) in &self.players {
            if player.team == team {
                player_mask |= 1u32 << pid.min(31);
            }
        }
        if player_mask == 0 {
            player_mask = 1u32 << player_id.min(31);
        }

        let center = Coord3D::new(location.x, location.z, location.y);
        // DynamicShroud grow residual: start at first pulse radius (not full VisionRange).
        let radius = crate::game_logic::host_spy_drone::spy_drone_scan_radius_after_updates(0);
        let duration = SPY_DRONE_FOW_DURATION_FRAMES;
        let frame = self.frame;

        let fow_reveal_ok = {
            let shroud = get_shroud_manager();
            let mut shroud_mgr = match shroud.lock() {
                Ok(mgr) => mgr,
                Err(_) => {
                    // Still record spawn residual even if shroud lock fails.
                    let act_id = self.spy_drones.alloc_id();
                    self.spy_drones.record_activation(HostSpyDrone {
                        id: act_id,
                        player_id,
                        player_mask,
                        location,
                        radius:
                            crate::game_logic::host_spy_drone::spy_drone_scan_radius_after_updates(
                                0,
                            ),
                        activate_frame: frame,
                        expires_frame: frame.saturating_add(duration),
                        caster_id,
                        spawned_id,
                        fow_reveal_ok: false,
                        spawn_ok,
                        dynamic_shroud_applied: true,
                        stealth_detector_applied: true,
                        grow_index: 0,
                        growing: true,
                    });
                    self.queue_audio_event(
                        AudioEventRequest::new(SPY_DRONE_ACTIVATE_AUDIO)
                            .with_position(location)
                            .with_priority(150),
                    );
                    return spawn_ok;
                }
            };
            if !shroud_mgr.has_shroud_grid() {
                shroud_mgr.init_shroud_grid(world_w, world_h);
            }
            shroud_mgr.do_shroud_reveal(&center, radius, player_mask);
            shroud_mgr.queue_undo_shroud_reveal(&center, radius, player_mask, duration, frame);
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

        let act_id = self.spy_drones.alloc_id();
        self.spy_drones.record_activation(HostSpyDrone {
            id: act_id,
            player_id,
            player_mask,
            location,
            radius: crate::game_logic::host_spy_drone::spy_drone_scan_radius_after_updates(0),
            activate_frame: frame,
            expires_frame: frame.saturating_add(duration),
            caster_id,
            spawned_id,
            fow_reveal_ok,
            spawn_ok,
            dynamic_shroud_applied: true,
            stealth_detector_applied: true,
            grow_index: 1, // initial FOW already applied at first grow step radius
            growing: true,
        });

        self.queue_audio_event(
            AudioEventRequest::new(SPY_DRONE_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(150),
        );

        spawn_ok || fow_reveal_ok
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
            FLARE_LIFETIME_FRAMES, FLARE_MAX_HEALTH, FLARE_TEMPLATE_NAME, VOLLEY_ARC_ANGLE_DEG,
        };
        use crate::game_logic::{KindOf, ThingTemplate};
        use std::f32::consts::PI;

        if !self.templates.contains_key(FLARE_TEMPLATE_NAME) {
            let mut t = ThingTemplate::new(FLARE_TEMPLATE_NAME);
            t.add_kind_of(KindOf::Projectile)
                .set_health(FLARE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(FLARE_TEMPLATE_NAME.to_string(), t);
        }
        let (team, origin) = {
            let o = self.objects.get(&aircraft_id)?;
            (o.team, o.get_position())
        };
        let owner_player_id = {
            let aircraft = self.objects.get(&aircraft_id)?;
            if aircraft.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(aircraft)?)
            } else {
                None
            }
        };
        // Volley arc residual: spread flares ± half VolleyArcAngle around aircraft.
        use crate::game_logic::host_countermeasures::VOLLEY_SIZE;
        let t = if VOLLEY_SIZE > 1 {
            (volley_index as f32) / ((VOLLEY_SIZE - 1) as f32)
        } else {
            0.5
        };
        let angle_deg = (t - 0.5) * VOLLEY_ARC_ANGLE_DEG;
        let angle = angle_deg * PI / 180.0;
        let dist = 12.0 + volley_index as f32 * 2.0;
        let place = glam::Vec3::new(
            origin.x + angle.cos() * dist,
            origin.y.max(0.0) + 8.0,
            origin.z + angle.sin() * dist,
        );
        let fid = self.create_object_for_owner_or_team(
            FLARE_TEMPLATE_NAME,
            team,
            owner_player_id,
            place,
        )?;
        let expires = self.frame.saturating_add(FLARE_LIFETIME_FRAMES.max(1));
        if let Some(o) = self.objects.get_mut(&fid) {
            o.countermeasure_flare = true;
            o.countermeasure_flare_expires_frame = Some(expires);
            o.producer_id = Some(aircraft_id);
            o.health.maximum = FLARE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, FLARE_MAX_HEALTH);
            o.weapon = None;
            o.secondary_weapon = None;
        }
        self.countermeasures.record_flare_spawned(1);
        Some(fid)
    }

    pub fn flush_countermeasure_flare_spawns(&mut self) {
        let pending = self.countermeasures.take_pending_flare_spawns();
        for spawn in pending {
            let _ = self.spawn_countermeasure_flare_object(spawn.aircraft_id, spawn.volley_index);
        }
    }

    pub fn update_countermeasure_flare_objects(&mut self) {
        let frame = self.frame;
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

    /// Advance SpySatellite residual: expire bookkeeping + process shroud undos.
    pub(in super::super) fn update_spy_satellites(&mut self) {
        self.spy_satellites.prune_expired(self.frame);
        self.spy_drones.prune_expired(self.frame);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.process_pending_undo_shroud_reveals(self.frame);
        }
    }
}
