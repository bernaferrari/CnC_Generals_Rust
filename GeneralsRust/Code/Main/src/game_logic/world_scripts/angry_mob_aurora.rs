//! Host scripts `impl GameLogic` — `angry_mob_aurora`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! CIA / firewall / inferno / angry mob / aurora
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `SpyVisionUpdate::doActivationWork` (`SpyVisionUpdate.cpp:178-182`):
/// `playerToSetFor->getRelationship(player->getDefaultTeam()) == ENEMIES`.
/// Leftover `SpyVisionController::do_activation_work_for_owner` uses
/// `owner.is_enemy_with_player`. Faction `Team` equality is not a proxy —
/// same-faction enemy slots spy, allied same-faction do not.
fn spy_vision_target_is_enemy(
    players: &std::collections::HashMap<u32, Player>,
    spy_player_id: u32,
    obj: &Object,
) -> bool {
    use gamelogic::common::Relationship;
    if !obj.is_alive() || obj.team == Team::Neutral {
        return false;
    }
    let owner = match obj.owner_player_id {
        Some(pid) => players
            .get(&pid)
            .filter(|player| player.is_alive && player.team == obj.team)
            .map(|player| player.id),
        None => {
            let mut ids = players
                .values()
                .filter(|player| player.is_alive && player.team == obj.team)
                .map(|player| player.id);
            let first = ids.next();
            if ids.next().is_none() { first } else { None }
        }
    };
    match owner {
        Some(oid) if oid == spy_player_id => false,
        Some(oid) => {
            GameLogic::player_relationship_from_map(players, spy_player_id, oid)
                == Relationship::Enemies
        }
        None => players
            .get(&spy_player_id)
            .map(|player| obj.team != player.team)
            .unwrap_or(true),
    }
}

impl GameLogic {
    // -----------------------------------------------------------------------
    // CIA Intelligence / SpyVision residual (setUnitsVisionSpied)
    // Fail-closed: not full SpyVisionUpdate module / kindof filter / sabotage path.
    // -----------------------------------------------------------------------

    /// Host CIA Intelligence residual registry (activate + honesty).
    pub fn cia_intelligence(
        &self,
    ) -> &crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry {
        &self.cia_intelligence
    }

    /// Restore CIA / SpyVision registry from a world snapshot tail.
    pub fn restore_cia_intelligence(
        &mut self,
        registry: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    ) {
        self.cia_intelligence.restore(registry);
    }

    /// Residual honesty: CIA Intelligence activated at least once.
    pub fn honesty_cia_intelligence_activate_ok(&self) -> bool {
        self.cia_intelligence.honesty_activate_ok()
    }

    /// Residual honesty: at least one enemy unit was vision-spied.
    pub fn honesty_cia_intelligence_vision_spied_ok(&self) -> bool {
        self.cia_intelligence.honesty_vision_spied_ok()
    }

    /// Residual honesty: FOW was cleared at least once at an enemy unit.
    pub fn honesty_cia_intelligence_fow_ok(&self) -> bool {
        self.cia_intelligence.honesty_fow_reveal_ok()
    }

    /// Combined host path honesty for CIA Intelligence residual.
    pub fn honesty_cia_intelligence_ok(&self) -> bool {
        self.cia_intelligence.honesty_host_path_ok()
    }

    /// Activate CIA Intelligence residual: temporarily vision-spy all enemy units.
    ///
    /// Matches retail SuperweaponCIAIntelligence / SpyVisionSpecialPower BaseDuration
    /// (30000 ms → 900 frames). For each enemy unit: set vision-spied residual,
    /// temporary FOW reveal at unit position (shroud_clearing_range), and mark
    /// stealthed units DETECTED so they become visible/targetable.
    ///
    /// Fail-closed: not SpyVisionUpdate upgrade mux / self-powered / kindof filter /
    /// capture / sabotage-disable / full OBJECT_REGISTRY Player::setUnitsVisionSpied.
    pub fn activate_cia_intelligence(
        &mut self,
        player_id: u32,
        team: Team,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_cia_intelligence::{
            CIA_INTELLIGENCE_ACTIVATE_AUDIO, CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS,
            HostCiaIntelligence, HostCiaIntelligenceSpiedUnit, cia_intelligence_duration_frames,
        };
        use gamelogic::common::Coord3D;

        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);

        // C++ setUnitsVisionSpied byWhom is only playerToSetFor->getPlayerIndex().
        // Do not paint an extra shroud disk for every same-faction slot.
        let player_mask = 1u32 << player_id.min(31);

        // C++ SpyVisionSpecialPower: duration += contain->getContainCount() * bonus.
        let captured_count = caster_id
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.contained_units().len() as u32)
            .unwrap_or(0);
        let duration = cia_intelligence_duration_frames(captured_count);
        let frame = self.frame;
        let expires_frame = frame.saturating_add(duration);

        // Collect enemy unit snapshots first (avoid borrow issues while mutating).
        let enemy_snapshots: Vec<(ObjectId, Vec3, f32, bool)> = self
            .objects
            .values()
            .filter(|obj| {
                spy_vision_target_is_enemy(&self.players, player_id, obj)
                    && caster_id.map(|c| c != obj.id).unwrap_or(true)
            })
            .map(|obj| {
                let shroud = obj.shroud_clearing_range;
                let radius = if shroud > 0.0 {
                    shroud
                } else {
                    CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS
                };
                (obj.id, obj.get_position(), radius, obj.status.stealthed)
            })
            .collect();

        // Ensure shroud grid exists (tests / pre-map residual).
        {
            let shroud = get_shroud_manager();
            if let Ok(mut shroud_mgr) = shroud.lock() {
                if !shroud_mgr.has_shroud_grid() {
                    shroud_mgr.init_shroud_grid(world_w, world_h);
                }
            }
        }

        let mut spied_units = Vec::with_capacity(enemy_snapshots.len());
        let mut any_vision_spied = false;
        let mut any_fow = false;
        let mut any_detect = false;
        let mut audio_pos = caster_id
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(Vec3::ZERO);

        for (obj_id, location, radius, _was_stealthed) in enemy_snapshots {
            // C++ Player::setUnitsVisionSpied → Object::setVisionSpied
            // (Object.cpp:5222-5253) makes the enemy a moving looker via
            // handlePartitionCellMaintenance. It does **not** mark DETECTED
            // or destalth (SpyVisionSpecialPower.cpp:64-102).
            if let Some(obj) = self.objects.get_mut(&obj_id) {
                obj.set_vision_spied_by_player(player_id, true);
                any_vision_spied = true;
            }

            // Temporary FOW reveal at enemy unit (spy their vision residual).
            // ShroudManager grid axes are (x, y); host uses (x, z) ground plane.
            let center = Coord3D::new(location.x, location.z, location.y);
            let fow_reveal_ok = {
                let shroud = get_shroud_manager();
                let mut shroud_mgr = match shroud.lock() {
                    Ok(mgr) => mgr,
                    Err(_) => {
                        spied_units.push(HostCiaIntelligenceSpiedUnit {
                            object_id: obj_id,
                            location,
                            radius,
                            fow_reveal_ok: false,
                            detected_ok: false,
                        });
                        continue;
                    }
                };
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
            if fow_reveal_ok {
                any_fow = true;
            }
            audio_pos = location;
            spied_units.push(HostCiaIntelligenceSpiedUnit {
                object_id: obj_id,
                location,
                radius,
                fow_reveal_ok,
                detected_ok: false,
            });
        }

        let act_id = self.cia_intelligence.alloc_id();
        self.cia_intelligence
            .record_activation(HostCiaIntelligence {
                captured_count,
                id: act_id,
                player_id,
                player_mask,
                spying_team: team,
                activate_frame: frame,
                expires_frame,
                caster_id,
                spied_units,
                vision_spied_ok: any_vision_spied,
                fow_reveal_ok: any_fow,
                detect_ok: any_detect,
            });

        self.queue_audio_event(
            AudioEventRequest::new(CIA_INTELLIGENCE_ACTIVATE_AUDIO)
                .with_position(audio_pos)
                .with_priority(150),
        );

        // Residual success: activation recorded (even with zero enemies — honesty
        // activate_ok). Vision-spied path preferred when enemies present.
        self.cia_intelligence.activations() > 0
    }

    /// Advance CIA Intelligence residual: moving lookers + expired marks.
    pub(in super::super) fn update_cia_intelligence(&mut self) {
        // C++ SpyVisionUpdate keeps enemy units as lookers until duration
        // expires. Re-reveal FOW at each still-spied unit's current pos.
        if !self.cia_intelligence.active_scans().is_empty() {
            use crate::game_logic::host_cia_intelligence::CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS;
            use gamelogic::common::Coord3D;
            let remaining_by_id: Vec<(ObjectId, u32, u32)> = self
                .cia_intelligence
                .active_scans()
                .iter()
                // C++ SpyVisionUpdate::update (SpyVisionUpdate.cpp:142-148):
                // once m_deactivateFrame <= now the spy turns OFF
                // (doActivationWork FALSE). Expired scans must NOT re-reveal;
                // the undo queued at activation removes the looker disk this
                // same tick.
                .filter(|scan| !scan.is_expired(self.frame))
                .flat_map(|scan| {
                    let remain = scan.expires_frame.saturating_sub(self.frame).max(1);
                    scan.spied_units
                        .iter()
                        .map(move |u| (u.object_id, remain, scan.player_mask))
                })
                .collect();
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                for (obj_id, remain, player_mask) in remaining_by_id {
                    let Some(obj) = self.objects.get(&obj_id) else {
                        continue;
                    };
                    if !obj.is_alive() {
                        continue;
                    }
                    let pos = obj.get_position();
                    let shroud = obj.shroud_clearing_range;
                    let radius = if shroud > 0.0 {
                        shroud
                    } else {
                        CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS
                    };
                    let center = Coord3D::new(pos.x, pos.z, pos.y);
                    shroud_mgr.do_shroud_reveal(&center, radius, player_mask);
                    shroud_mgr.queue_undo_shroud_reveal(
                        &center,
                        radius,
                        player_mask,
                        remain,
                        self.frame,
                    );
                }
            }
        }

        let cleared = self.cia_intelligence.prune_expired(self.frame);
        // Clear vision_spied residual marks only if no other active spy still covers them.
        for obj_id in cleared {
            let still_spied = self
                .cia_intelligence
                .active_scans()
                .iter()
                .any(|a| a.is_object_spied(obj_id));
            if still_spied {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&obj_id) {
                // Clear all spy player bits that no longer have an active residual.
                // Residual simplification: clear full mask when no active spy covers unit.
                obj.vision_spied_mask = 0;
                obj.record_host_vision_camo();
            }
        }
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.process_pending_undo_shroud_reveals(self.frame);
        }
    }

    /// C++ `SpyVisionUpdate::upgradeImplementation` → `activateSpyVision`.
    pub(crate) fn activate_satellite_hack_spy_vision(
        &mut self,
        center_id: ObjectId,
        spec: crate::game_logic::host_satellite_hack::SatelliteHackSpySpec,
    ) -> bool {
        use crate::game_logic::host_cia_intelligence::{
            CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS, HostCiaIntelligence,
            HostCiaIntelligenceSpiedUnit,
        };
        use crate::game_logic::host_satellite_hack::spy_vision_deactivate_frame;
        use crate::game_logic::host_upgrades::{
            UPGRADE_CHINA_SATELLITE_HACK_ONE, UPGRADE_CHINA_SATELLITE_HACK_TWO,
        };
        use gamelogic::common::Coord3D;

        let Some(center) = self.objects.get(&center_id) else {
            return false;
        };
        if !center.is_alive() {
            return false;
        }
        if center.is_spy_vision_disabled(self.frame) {
            return false;
        }
        let team = center.team;
        let player_id = center.owner_player_id.unwrap_or_else(|| {
            self.players
                .iter()
                .find(|(_, p)| p.team == team)
                .map(|(id, _)| *id)
                .unwrap_or(0)
        });

        // C++ setUnitsVisionSpied byWhom is only the caster player index.
        let player_mask = 1u32 << player_id.min(31);

        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);
        let command_centers_only = spec.command_centers_only;
        let enemy_snapshots: Vec<(ObjectId, Vec3, f32)> = self
            .objects
            .values()
            .filter(|obj| {
                spy_vision_target_is_enemy(&self.players, player_id, obj)
                    && obj.id != center_id
                    && (!command_centers_only || obj.is_command_center())
            })
            .map(|obj| {
                let shroud = obj.shroud_clearing_range;
                let radius = if shroud > 0.0 {
                    shroud
                } else {
                    CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS
                };
                (obj.id, obj.get_position(), radius)
            })
            .collect();

        {
            let shroud = get_shroud_manager();
            if let Ok(mut shroud_mgr) = shroud.lock() {
                if !shroud_mgr.has_shroud_grid() {
                    shroud_mgr.init_shroud_grid(world_w, world_h);
                }
            }
        }

        let mut spied_units = Vec::with_capacity(enemy_snapshots.len());
        let mut any_vision_spied = false;
        let mut any_fow = false;
        for (obj_id, location, radius) in enemy_snapshots {
            if let Some(obj) = self.objects.get_mut(&obj_id) {
                obj.set_vision_spied_by_player(player_id, true);
                any_vision_spied = true;
            }
            let center_pos = Coord3D::new(location.x, location.z, location.y);
            let fow_reveal_ok = {
                let shroud = get_shroud_manager();
                let Ok(mut shroud_mgr) = shroud.lock() else {
                    spied_units.push(HostCiaIntelligenceSpiedUnit {
                        object_id: obj_id,
                        location,
                        radius,
                        fow_reveal_ok: false,
                        detected_ok: false,
                    });
                    continue;
                };
                let duration = if spec.duration_frames == 0 {
                    u32::MAX / 4
                } else {
                    spec.duration_frames
                };
                shroud_mgr.do_shroud_reveal(&center_pos, radius, player_mask);
                shroud_mgr.queue_undo_shroud_reveal(
                    &center_pos,
                    radius,
                    player_mask,
                    duration,
                    self.frame,
                );
                let mut visible = shroud_mgr.is_position_visible(player_id.min(31), &center_pos);
                if !visible {
                    for bit in 0..32u32 {
                        if (player_mask & (1u32 << bit)) != 0
                            && shroud_mgr.is_position_visible(bit, &center_pos)
                        {
                            visible = true;
                            break;
                        }
                    }
                }
                visible
            };
            if fow_reveal_ok {
                any_fow = true;
            }
            spied_units.push(HostCiaIntelligenceSpiedUnit {
                object_id: obj_id,
                location,
                radius,
                fow_reveal_ok,
                detected_ok: false,
            });
        }

        let expires_frame = spy_vision_deactivate_frame(self.frame, spec.duration_frames);
        let act_id = self.cia_intelligence.alloc_id();
        self.cia_intelligence
            .record_activation(HostCiaIntelligence {
                captured_count: 0,
                id: act_id,
                player_id,
                player_mask,
                spying_team: team,
                activate_frame: self.frame,
                expires_frame,
                caster_id: Some(center_id),
                spied_units,
                vision_spied_ok: any_vision_spied,
                fow_reveal_ok: any_fow,
                detect_ok: false,
            });

        if let Some(center) = self.objects.get_mut(&center_id) {
            if spec.command_centers_only {
                center.apply_upgrade_tag(UPGRADE_CHINA_SATELLITE_HACK_ONE);
            } else {
                center.apply_upgrade_tag(UPGRADE_CHINA_SATELLITE_HACK_TWO);
                if spec.interval_frames > 0 && expires_frame != u32::MAX {
                    center.status.spy_vision_hack_two_wake_frame =
                        expires_frame.saturating_add(spec.interval_frames);
                }
            }
        }
        true
    }

    /// C++ `SpyVisionUpdate::update` self-powered cycle + sabotage reset.
    pub(in super::super) fn update_satellite_hack_spy_vision(&mut self) {
        use crate::game_logic::host_satellite_hack::satellite_hack_spy_spec;
        use crate::game_logic::host_upgrades::{
            UPGRADE_CHINA_SATELLITE_HACK_ONE, UPGRADE_CHINA_SATELLITE_HACK_TWO,
        };

        let frame = self.frame;

        // Expire scans whose Internet Center caster died or is sabotaged.
        let mut expire_casters = std::collections::HashSet::new();
        for act in self.cia_intelligence.active_scans() {
            let Some(cid) = act.caster_id else {
                continue;
            };
            let dead_or_disabled = match self.objects.get(&cid) {
                None => true,
                Some(o) => !o.is_alive() || o.is_spy_vision_disabled(frame),
            };
            if dead_or_disabled {
                expire_casters.insert(cid);
            }
        }
        if !expire_casters.is_empty() {
            for act in &mut self.cia_intelligence.active {
                if act.caster_id.is_some_and(|c| expire_casters.contains(&c)) {
                    act.expires_frame = frame;
                }
            }
        }

        let mut activate = Vec::new();
        let center_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::FSInternetCenter)
                        || crate::game_logic::host_satellite_hack::object_authors_spy_vision_update(
                            &o.template_name,
                        ))
            })
            .map(|(id, _)| *id)
            .collect();

        for id in center_ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if obj.is_spy_vision_disabled(frame) {
                continue;
            }
            let reset = obj.status.spy_vision_reset_timers;
            let has_one = obj.has_upgrade_tag(UPGRADE_CHINA_SATELLITE_HACK_ONE);
            let has_two = obj.has_upgrade_tag(UPGRADE_CHINA_SATELLITE_HACK_TWO);
            let wake_two = obj.status.spy_vision_hack_two_wake_frame;
            if reset {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.status.spy_vision_reset_timers = false;
                }
                if has_one {
                    if let Some(spec) = satellite_hack_spy_spec(UPGRADE_CHINA_SATELLITE_HACK_ONE) {
                        activate.push((id, spec));
                    }
                }
                if has_two {
                    if let Some(spec) = satellite_hack_spy_spec(UPGRADE_CHINA_SATELLITE_HACK_TWO) {
                        if let Some(obj) = self.objects.get_mut(&id) {
                            obj.status.spy_vision_hack_two_wake_frame =
                                frame.saturating_add(spec.interval_frames);
                        }
                    }
                }
                continue;
            }
            if has_two && wake_two > 0 && frame >= wake_two {
                if let Some(spec) = satellite_hack_spy_spec(UPGRADE_CHINA_SATELLITE_HACK_TWO) {
                    let already_pulsing = self.cia_intelligence.active_scans().iter().any(|a| {
                        a.caster_id == Some(id)
                            && a.expires_frame != u32::MAX
                            && !a.is_expired(frame)
                    });
                    if !already_pulsing {
                        activate.push((id, spec));
                    }
                }
            }
        }

        for (id, spec) in activate {
            let _ = self.activate_satellite_hack_spy_vision(id, spec);
        }
    }

    // -----------------------------------------------------------------------
    // China FireWall / Firestorm residual (Dragon Tank FIRE_WEAPON secondary)
    // Fail-closed: not full OCL FireWallSegment / InchForwardLocomotor / projectile stream.
    // -----------------------------------------------------------------------

    /// Host FireWall residual registry (activate + honesty).
    pub fn fire_walls(&self) -> &crate::game_logic::host_firewall::HostFireWallRegistry {
        &self.fire_walls
    }

    /// Residual honesty: FireWall activated at least once.
    pub fn honesty_firewall_activate_ok(&self) -> bool {
        self.fire_walls.honesty_activate_ok()
    }

    /// Residual honesty: FireWall applied fire damage at least once.
    pub fn honesty_firewall_damage_ok(&self) -> bool {
        self.fire_walls.honesty_damage_ok()
    }

    /// Combined host path honesty for FireWall residual.
    pub fn honesty_firewall_ok(&self) -> bool {
        self.fire_walls.honesty_host_path_ok()
    }

    /// Residual honesty: BlackNapalm FireWall segment upgrade used at least once.
    pub fn honesty_firewall_black_napalm_ok(&self) -> bool {
        self.fire_walls.honesty_upgraded_ok() || self.dragon_tank_residual_black_napalm_upgrades > 0
    }

    /// Residual honesty: InchForward crawl applied at least once.
    pub fn honesty_firewall_inch_forward_ok(&self) -> bool {
        self.fire_walls.honesty_crawl_ok()
    }

    /// Activate China FireWall residual: line of fire damage zones from caster
    /// toward `target_position` (retail DragonTankFireWallWeapon → OCL_FireWallSegment).
    ///
    /// Fail-closed: not full projectile stream / InchForwardLocomotor crawl /
    /// BlackNapalm upgraded segments / weapon-slot AI matrix.
    pub fn activate_firewall(
        &mut self,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_dragon_tank::has_black_napalm_upgrade;
        use crate::game_logic::host_firewall::{FIREWALL_ACTIVATE_AUDIO, FIREWALL_BURN_AUDIO};

        let (caster_pos, source_team, upgraded) = {
            let obj = self.objects.get(&source_object)?;
            if !obj.is_alive() {
                return None;
            }
            (
                obj.get_position(),
                obj.team,
                has_black_napalm_upgrade(&obj.applied_upgrades),
            )
        };

        let frame = self.frame;
        let id = self.fire_walls.activate(
            source_object,
            source_team,
            caster_pos,
            target_position,
            frame,
            upgraded,
        );
        if upgraded {
            self.dragon_tank_residual_black_napalm_upgrades = self
                .dragon_tank_residual_black_napalm_upgrades
                .saturating_add(1);
        }

        self.queue_audio_event(
            AudioEventRequest::new(FIREWALL_ACTIVATE_AUDIO)
                .with_object(source_object)
                .with_position(caster_pos)
                .with_priority(160),
        );
        self.queue_audio_event(
            AudioEventRequest::new(FIREWALL_BURN_AUDIO)
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(140),
        );

        // Residual flame particle at first segment (presentation observability).
        if let Some(wall) = self.fire_walls.active_walls().iter().find(|w| w.id == id) {
            if let Some(seg) = wall.segments.first() {
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::WeaponMuzzleFlash,
                    seg.position,
                    frame,
                    Some(source_object),
                    None,
                );
            }
        }

        // C++ OCL_FireWallSegment CreateObject residual along wall line.
        let _ = self.spawn_firewall_segment_objects(id, source_object, source_team);

        Some(id)
    }

    /// Advance FireWall residual: apply periodic flame damage along active segments.
    pub(in super::super) fn update_firewalls(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .fire_walls
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

            self.fire_walls.record_tick_complete(
                plan.wall_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.fire_walls.prune_expired(frame);
    }

    // -----------------------------------------------------------------------
    // China Inferno Cannon residual (FireFieldSmall DoT on shell impact)
    // Fail-closed: not full InfernoTankShell projectile / OCL_FireFieldSmall object spawn.
    // -----------------------------------------------------------------------

    /// Host Inferno Cannon residual fire-zone registry (spawn + honesty).
    pub fn inferno_fire_zones(
        &self,
    ) -> &crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry {
        &self.inferno_fire_zones
    }

    /// Residual honesty: Inferno fire zone spawned at least once.
    pub fn honesty_inferno_fire_spawn_ok(&self) -> bool {
        self.inferno_fire_zones.honesty_spawn_ok()
    }

    /// Residual honesty: Inferno fire zone applied damage at least once.
    pub fn honesty_inferno_fire_damage_ok(&self) -> bool {
        self.inferno_fire_zones.honesty_damage_ok()
    }

    /// Combined host path honesty for Inferno Cannon fire residual.
    pub fn honesty_inferno_cannon_ok(&self) -> bool {
        self.inferno_fire_zones.honesty_host_path_ok()
            || self.inferno_shells_spawned > 0
            || self.inferno_scatter_applied > 0
    }

    /// Residual honesty: Inferno Cannon ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_inferno_scatter_ok(&self) -> bool {
        self.inferno_scatter_applied > 0 || self.inferno_scatter_misses > 0
    }

    /// Spawn residual FireFieldSmall at Inferno Cannon shell impact.
    ///
    /// Retail path: InfernoTankShell death → SmallFireFieldCreationWeapon →
    /// OCL_FireFieldSmall → FireFieldSmall with SmallFireFieldWeapon DoT.
    ///
    /// Fail-closed: not full projectile lob path / BlackNapalm upgraded particle
    /// bones / HistoricBonus Firestorm multi-shell matrix.
    /// C++ InfernoTankShell DumbProjectile residual (Bezier + FireField on detonate).
    pub fn spawn_inferno_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        upgraded: bool,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_inferno_cannon::{
            INFERNO_CANNON_PROJECTILE, INFERNO_CANNON_PROJECTILE_UPGRADED,
            INFERNO_SHELL_MAX_HEALTH, inferno_shell_flight_frames,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let proj = if upgraded {
            INFERNO_CANNON_PROJECTILE_UPGRADED
        } else {
            INFERNO_CANNON_PROJECTILE
        };
        if !self.templates.contains_key(proj) {
            let mut t = ThingTemplate::new(proj);
            t.add_kind_of(KindOf::Projectile)
                .set_health(INFERNO_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(proj.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on InfernoCannonGun vs infantry (**30**).
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_inferno_cannon::inferno_cannon_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.inferno_scatter_applied = self.inferno_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_inferno_cannon::inferno_cannon_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_inferno_cannon::INFERNO_CANNON_SHELL_RADIUS {
                        self.inferno_scatter_misses = self.inferno_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(proj, team, start)?;
        let frames = inferno_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.inferno_shell_projectile = true;
            o.inferno_shell_from = Some([start.x, start.y, start.z]);
            o.inferno_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.inferno_shell_launch_frame = Some(self.frame);
            o.inferno_shell_flight_frames = frames;
            o.inferno_shell_intended = intended.map(|id| id.0);
            o.inferno_shell_upgraded = upgraded;
            o.note_producer(source_id);
            o.health.maximum = INFERNO_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, INFERNO_SHELL_MAX_HEALTH);
        }
        self.inferno_shells_spawned = self.inferno_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_inferno_shell_projectiles(&mut self) {
        use crate::game_logic::host_inferno_cannon::inferno_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.inferno_shell_projectile && o.is_alive() {
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
            Team,
        )> = Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames, upgraded, team) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .inferno_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .inferno_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.inferno_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.inferno_shell_launch_frame.unwrap_or(frame),
                    o.inferno_shell_flight_frames.max(1),
                    o.inferno_shell_upgraded,
                    o.team,
                )
            };
            let team = source
                .and_then(|sid| self.objects.get(&sid).map(|s| s.team))
                .unwrap_or(team);
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = inferno_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim, upgraded, team));
            }
        }
        for (id, source, intended, pos, upgraded, team) in impact {
            let shell_team = self.objects.get(&id).map(|o| o.team);
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
                o.inferno_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_inferno_shell_residual_at(pos, source, intended);
            if let Some(sid) = source {
                let _ = self.spawn_inferno_fire_zone(sid, team, pos, upgraded);
            }
            self.mark_object_for_destruction(id, shell_team);
        }
    }

    pub fn honesty_inferno_shell_projectile_ok(&self) -> bool {
        self.inferno_shells_spawned > 0
    }

    /// Apply InfernoTankShell primary splash residual at impact.
    pub fn apply_inferno_shell_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_inferno_cannon::{
            INFERNO_CANNON_DAMAGE_TYPE, INFERNO_CANNON_DEATH_TYPE, INFERNO_CANNON_SHELL_DAMAGE,
            INFERNO_CANNON_SHELL_RADIUS, inferno_shell_damage_at, is_inferno_cannon_template,
        };

        let (source_team, shell_dmg) = {
            let Some(sid) = source else {
                return (0, false);
            };
            let Some(obj) = self.objects.get(&sid) else {
                return (0, false);
            };
            if !is_inferno_cannon_template(&obj.template_name) {
                return (0, false);
            }
            let dmg = obj
                .weapon
                .as_ref()
                .map(|w| w.damage)
                .unwrap_or(INFERNO_CANNON_SHELL_DAMAGE);
            (obj.team, dmg)
        };

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Structure);
                if !combat_kind || obj.is_kind_of(KindOf::Projectile) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact_xz.0;
                let dz = p.z - impact_xz.1;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist > INFERNO_CANNON_SHELL_RADIUS && Some(*id) != intended_target {
                    return None;
                }
                Some((*id, dist))
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = if Some(id) == intended_target {
                shell_dmg
            } else {
                let base = inferno_shell_damage_at(dist);
                if base <= 0.0 {
                    0.0
                } else {
                    shell_dmg * (base / INFERNO_CANNON_SHELL_DAMAGE.max(0.001))
                }
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    INFERNO_CANNON_DAMAGE_TYPE,
                    INFERNO_CANNON_DEATH_TYPE,
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
        (hits, any_destroyed)
    }

    pub fn spawn_inferno_fire_zone(
        &mut self,
        source_object: ObjectId,
        source_team: Team,
        impact: Vec3,
        upgraded: bool,
    ) -> u32 {
        use crate::game_logic::host_inferno_cannon::{
            INFERNO_CANNON_FIRE_AUDIO, INFERNO_FIRE_BURN_AUDIO,
        };

        let frame = self.frame;
        let id =
            self.inferno_fire_zones
                .spawn_zone(source_object, source_team, impact, frame, upgraded);
        if upgraded {
            self.inferno_black_napalm_residual_zones =
                self.inferno_black_napalm_residual_zones.saturating_add(1);
        }

        self.queue_audio_event(
            AudioEventRequest::new(INFERNO_CANNON_FIRE_AUDIO)
                .with_object(source_object)
                .with_position(impact)
                .with_priority(160),
        );
        self.queue_audio_event(
            AudioEventRequest::new(INFERNO_FIRE_BURN_AUDIO)
                .with_object(source_object)
                .with_position(impact)
                .with_priority(140),
        );

        // Residual flame particle at impact (presentation observability).
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponMuzzleFlash,
            impact,
            frame,
            Some(source_object),
            None,
        );

        // OCL_FireFieldSmall / FireFieldUpgradedSmall object residual.
        let _ = self.spawn_inferno_fire_field_object(id, impact, upgraded, source_team);

        id
    }

    /// C++ OCL_FireFieldSmall CreateObject FireFieldSmall residual.
    pub fn spawn_inferno_fire_field_object(
        &mut self,
        zone_id: u32,
        impact: Vec3,
        upgraded: bool,
        team: Team,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_inferno_cannon::{
            INFERNO_FIRE_DURATION_FRAMES, INFERNO_FIRE_FIELD_MAX_HEALTH,
            INFERNO_FIRE_FIELD_TEMPLATE, INFERNO_FIRE_FIELD_TEMPLATE_UPGRADED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let name = if upgraded {
            INFERNO_FIRE_FIELD_TEMPLATE_UPGRADED
        } else {
            INFERNO_FIRE_FIELD_TEMPLATE
        };
        if !self.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Immobile)
                .set_health(INFERNO_FIRE_FIELD_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(name.to_string(), t);
        }
        let mut pos = impact;
        pos.y = 0.0; // ON_GROUND_ALIGNED residual
        let pid = self.create_object(name, team, pos)?;
        let expires = self
            .frame
            .saturating_add(INFERNO_FIRE_DURATION_FRAMES.max(1));
        if let Some(o) = self.objects.get_mut(&pid) {
            o.inferno_fire_field = true;
            o.inferno_fire_field_upgraded = upgraded;
            o.inferno_fire_field_expires_frame = Some(expires);
            o.inferno_fire_field_zone_id = Some(zone_id);
            o.health.current = INFERNO_FIRE_FIELD_MAX_HEALTH;
            o.health.maximum = INFERNO_FIRE_FIELD_MAX_HEALTH;
            // C++ FireFieldSmall uses InactiveBody: no HP, effectively dead.
            o.uses_inactive_body = true;
            o.status.effectively_dead = true;
        }
        self.inferno_fire_zones.record_fire_field_object(1);
        Some(pid)
    }

    pub fn update_inferno_fire_field_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.inferno_fire_field {
                    if let Some(exp) = o.inferno_fire_field_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
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
                o.inferno_fire_field = false;
            }
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_inferno_fire_field_object_ok(&self) -> bool {
        self.inferno_fire_zones.honesty_fire_field_object_ok()
    }

    /// Advance Inferno fire zones: apply periodic flame damage in residual radius.
    pub(in super::super) fn update_inferno_fire_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .inferno_fire_zones
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

            self.inferno_fire_zones.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.inferno_fire_zones.prune_expired(frame);
    }

    // -----------------------------------------------------------------------
    // GLA Angry Mob residual (nexus damages nearby enemies / SpawnBehavior members)
    // C++ SpawnBehavior: rapid-spawn SpawnNumber, replace on delay, last-member
    // death destroys nexus, computeAggregateStates MASKED.
    // Fail-closed: not full MobMemberSlavedUpdate / wander locomotor matrix.
    // -----------------------------------------------------------------------

    /// Host GLA Angry Mob residual registry (member expand + aggregate fire).
    pub fn angry_mobs(&self) -> &crate::game_logic::host_angry_mob::HostAngryMobRegistry {
        &self.angry_mobs
    }

    /// Residual honesty: Angry Mob applied damage to nearby enemies.
    pub fn honesty_angry_mob_damage_ok(&self) -> bool {
        self.angry_mobs.honesty_damage_ok()
    }

    /// Residual honesty: Angry Mob expand residual grew member count.
    pub fn honesty_angry_mob_expand_ok(&self) -> bool {
        self.angry_mobs.honesty_expand_ok()
    }

    /// Combined host path honesty for Angry Mob residual.
    pub fn honesty_angry_mob_ok(&self) -> bool {
        self.angry_mobs.honesty_host_path_ok()
    }

    /// Advance Angry Mob residual: SpawnBehavior members + aggregate fire.
    ///
    /// Retail: SpawnBehavior members fire pistol/rock/molotov; residual collapses
    /// that into periodic AoE damage around the nexus within AttackRange 100.
    /// C++ rapid-spawns all SpawnNumber, then SpawnReplaceDelay replacements.
    ///
    /// Fail-closed: not full member wander locomotor or slave AI matrix.
    /// C++ SpawnBehavior member SpecialObject residual for AngryMob nexus.
    pub fn spawn_angry_mob_member_object(
        &mut self,
        nexus_id: ObjectId,
        team: Team,
        template_name: &str,
        slot_index: u32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_angry_mob::ANGRY_MOB_MEMBER_MAX_HEALTH;
        use crate::game_logic::{KindOf, ThingTemplate};
        use std::f32::consts::PI;
        if !self.templates.contains_key(template_name) {
            let mut t = ThingTemplate::new(template_name);
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::IgnoredInGui)
                .set_health(ANGRY_MOB_MEMBER_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(template_name.to_string(), t);
        } else if let Some(t) = self.templates.get_mut(template_name) {
            t.add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::IgnoredInGui);
        }
        let origin = self.objects.get(&nexus_id)?.get_position();
        let angle = (slot_index as f32) * (2.0 * PI / 8.0);
        let radius = 8.0 + (slot_index % 3) as f32 * 2.0;
        let place = glam::Vec3::new(
            origin.x + angle.cos() * radius,
            origin.y,
            origin.z + angle.sin() * radius,
        );
        let Some(mid) = self.create_object(template_name, team, place) else {
            self.angry_mobs
                .rollback_failed_member_spawn(nexus_id, self.frame);
            return None;
        };
        if let Some(o) = self.objects.get_mut(&mid) {
            o.angry_mob_member = true;
            o.angry_mob_nexus_id = Some(nexus_id);
            o.producer_id = Some(nexus_id);
            o.health.maximum = ANGRY_MOB_MEMBER_MAX_HEALTH;
            o.thing.template.add_kind_of(KindOf::Attackable);
            o.thing.template.add_kind_of(KindOf::Selectable);
            o.thing.template.add_kind_of(KindOf::IgnoredInGui);
            Self::write_object_health_authority_aware(o, ANGRY_MOB_MEMBER_MAX_HEALTH);
        }
        if let Some(m) = self
            .angry_mobs
            .active_mobs_mut()
            .iter_mut()
            .find(|m| m.object_id == nexus_id)
        {
            m.member_ids.push(mid);
        }
        self.angry_mobs.record_member_spawned(1);
        Some(mid)
    }

    pub fn flush_angry_mob_member_spawns(&mut self) {
        let pending = self.angry_mobs.take_pending_member_spawns();
        for spawn in pending {
            if self
                .spawn_angry_mob_member_object(
                    spawn.nexus_id,
                    spawn.team,
                    &spawn.template_name,
                    spawn.slot_index,
                )
                .is_none()
            {
                self.angry_mobs
                    .rollback_failed_member_spawn(spawn.nexus_id, self.frame);
            }
        }
    }

    /// MobMemberSlavedUpdate residual: members follow nexus position.
    pub fn update_angry_mob_member_follow(&mut self) {
        use std::f32::consts::PI;
        let pairs: Vec<(ObjectId, ObjectId, u32)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.angry_mob_member {
                    o.angry_mob_nexus_id.map(|n| (*id, n, id.0 % 8))
                } else {
                    None
                }
            })
            .collect();
        let mut destroy = Vec::new();
        let mut moves = Vec::new();
        for (mid, nid, slot) in pairs {
            let Some(nexus) = self.objects.get(&nid) else {
                destroy.push(mid);
                continue;
            };
            if !nexus.is_alive() || nexus.status.destroyed {
                destroy.push(mid);
                continue;
            }
            let origin = nexus.get_position();
            let angle = (slot as f32) * (2.0 * PI / 8.0);
            let radius = 8.0 + (slot % 3) as f32 * 2.0;
            let dest = glam::Vec3::new(
                origin.x + angle.cos() * radius,
                origin.y,
                origin.z + angle.sin() * radius,
            );
            // C++ MobMemberSlavedUpdate.cpp:198-201: WANDER when ahead of nexus,
            // PANIC when lagging (masterPathDistToGoal > myPathDistToGoal).
            let member_pos = self
                .objects
                .get(&mid)
                .map(|o| o.get_position())
                .unwrap_or(origin);
            let nexus_goal = nexus
                .movement
                .target_position
                .or(nexus.move_away_destination);
            let member_to_goal = match nexus_goal {
                Some(g) => (member_pos.x - g.x).hypot(member_pos.z - g.z),
                None => (member_pos.x - origin.x).hypot(member_pos.z - origin.z),
            };
            let nexus_to_goal = match nexus_goal {
                Some(g) => (origin.x - g.x).hypot(origin.z - g.z),
                None => 0.0,
            };
            let ahead = member_to_goal + 0.01 < nexus_to_goal;
            moves.push((mid, dest, ahead));
        }
        for (mid, pos, ahead) in moves {
            if let Some(o) = self.objects.get_mut(&mid) {
                use crate::game_logic::host_upgrade_module_residuals::{
                    HostLocomotorSetKind, apply_choose_locomotor_set,
                };
                if ahead {
                    apply_choose_locomotor_set(o, HostLocomotorSetKind::Wander, false);
                } else {
                    apply_choose_locomotor_set(o, HostLocomotorSetKind::Panic, true);
                }
                // C++ MobMemberSlavedUpdate::aiMoveToPosition — pathfind catch-up,
                // never snap-teleport (that overwrote player nexus orders).
                let retarget = match o.movement.target_position {
                    Some(cur) => {
                        let dx = cur.x - pos.x;
                        let dz = cur.z - pos.z;
                        dx * dx + dz * dz > 100.0
                    }
                    None => true,
                };
                if retarget {
                    o.set_destination(pos);
                }
            }
        }
        for mid in destroy {
            if let Some(o) = self.objects.get_mut(&mid) {
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
                o.angry_mob_member = false;
            }
            self.mark_object_for_destruction(mid, None);
        }
    }

    pub fn honesty_angry_mob_member_spawn_ok(&self) -> bool {
        self.angry_mobs.honesty_member_spawn_ok()
    }

    /// C++ `PartitionFilterLiveMapEnemies` / `findClosestEnemy`: ENEMIES only
    /// (`Object::getRelationship`). Missing owners fall back to leftover
    /// `is_angry_mob_hostile_team` (faction residual).
    fn angry_mob_relationship_enemies_from_maps(
        players: &std::collections::HashMap<u32, crate::game_logic::Player>,
        objects: &std::collections::HashMap<ObjectId, Object>,
        source_id: ObjectId,
        source_team: Team,
        target_id: ObjectId,
        target_team: Team,
    ) -> bool {
        use crate::game_logic::host_angry_mob::is_angry_mob_hostile_team;
        match (objects.get(&source_id), objects.get(&target_id)) {
            (Some(src), Some(tgt))
                if src.owner_player_id.is_some() && tgt.owner_player_id.is_some() =>
            {
                Self::object_relationship_from_owners(
                    players,
                    src.owner_player_id,
                    &src.team_instance_name,
                    tgt.owner_player_id,
                    &tgt.team_instance_name,
                ) == gamelogic::common::Relationship::Enemies
            }
            _ => is_angry_mob_hostile_team(
                source_team == Team::Neutral,
                target_team == source_team,
                target_team == Team::Neutral,
            ),
        }
    }

    pub fn update_angry_mobs(&mut self) {
        use crate::game_logic::host_angry_mob::{
            ANGRY_MOB_FIRE_AUDIO, UPGRADE_GLA_ARM_THE_MOB, is_angry_mob_nexus_template,
        };

        let frame = self.frame;

        // Living residual nexus sources.
        let living: Vec<(ObjectId, Team, Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || !is_angry_mob_nexus_template(&obj.template_name) {
                    return None;
                }
                if obj.status.under_construction || obj.construction_percent + 0.001 < 1.0 {
                    return None;
                }
                if obj.status.disabled_unmanned
                    || obj.status.disabled_hacked
                    || obj.status.disabled_emp
                    || obj.status.disabled_subdued
                {
                    return None;
                }
                Some((*id, obj.team, obj.get_position()))
            })
            .collect();

        self.angry_mobs.sync_mobs(&living, frame);

        // C++ SpawnBehavior::onSpawnDeath — shrink live count; last member kills nexus.
        let destroy_nexuses =
            self.angry_mobs
                .process_dead_members(frame, |mid| match self.objects.get(&mid) {
                    None => true,
                    Some(o) => !o.is_alive() || o.status.destroyed || o.status.effectively_dead,
                });
        for nid in destroy_nexuses {
            if let Some(o) = self.objects.get_mut(&nid) {
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
            }
            self.mark_object_for_destruction(nid, None);
        }
        self.angry_mobs.evict_pending_destroyed_nexuses();

        self.angry_mobs.apply_due_expands(frame);
        self.flush_angry_mob_member_spawns();
        // Wave 801: under coupled shadow, AngryMob member follow is owned by
        // GW tick_status_timer_expirations + destroy logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_angry_mob_member_follow();
        }

        // C++ MASKED is a weapon override. Live is_selectable treats MASKED as
        // unselectable, so keep the nexus clear and Selectable. Weapons skip it
        // via is_angry_mob_nexus_template.
        let nexus_ids: Vec<ObjectId> = self
            .angry_mobs
            .active_mobs()
            .iter()
            .filter(|m| !m.pending_nexus_destroy)
            .map(|m| m.object_id)
            .collect();
        for nid in nexus_ids {
            if let Some(obj) = self.objects.get_mut(&nid) {
                if obj.status.masked {
                    obj.set_status_masked(false);
                }
                obj.thing.template.add_kind_of(KindOf::Selectable);
                obj.thing.template.add_kind_of(KindOf::Infantry);
            }
        }

        // C++ SpawnBehavior averages spawn positions into m_healthBoxOffset
        // so the Angry Mob bar sits on the swarm, not the invisible nexus.
        {
            let plans: Vec<(ObjectId, Vec<ObjectId>)> = self
                .angry_mobs
                .active_mobs()
                .iter()
                .filter(|m| !m.pending_nexus_destroy)
                .map(|m| (m.object_id, m.member_ids.clone()))
                .collect();
            for (nid, members) in plans {
                let Some(nexus_pos) = self.objects.get(&nid).map(|o| o.get_position()) else {
                    continue;
                };
                let mut sx = 0.0;
                let mut sy = 0.0;
                let mut sz = 0.0;
                let mut n = 0u32;
                for mid in members {
                    if let Some(m) = self.objects.get(&mid) {
                        if m.is_alive() {
                            let p = m.get_position();
                            sx += p.x - nexus_pos.x;
                            sy += p.y - nexus_pos.y;
                            sz += p.z - nexus_pos.z;
                            n = n.saturating_add(1);
                        }
                    }
                }
                if n > 0 {
                    let inv = 1.0 / n as f32;
                    if let Some(obj) = self.objects.get_mut(&nid) {
                        obj.health_box_offset = [sx * inv, sy * inv, sz * inv];
                    }
                }
            }
        }

        if self.angry_mobs.active_count() == 0 {
            return;
        }

        // Candidates for residual aggregate fire.
        // C++ member weapons AntiAir=No (`Weapon.cpp:287` WEAPON_ANTI_GROUND).
        // Leftover `get_victim_anti_mask` already matches; live
        // `weapon_target_anti_mask` is the same exclusive mask. KindOf::Aircraft
        // is residual-air so jets without AIRBORNE_TARGET still miss.
        use crate::game_logic::host_angry_mob::angry_mob_possible_to_attack;
        let candidates: Vec<(ObjectId, Vec3, Team, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                let air_ok = angry_mob_possible_to_attack(
                    obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft,
                    obj.status.airborne_target,
                    obj.weapon_target_anti_mask(),
                );
                let combat_kind = air_ok
                    && (obj.is_kind_of(KindOf::Attackable)
                        || obj.is_kind_of(KindOf::Structure)
                        || obj.is_kind_of(KindOf::Infantry)
                        || obj.is_kind_of(KindOf::Vehicle)
                        || obj.object_type == ObjectType::Building
                        || obj.object_type == ObjectType::Infantry
                        || obj.object_type == ObjectType::Vehicle);
                (
                    *id,
                    obj.get_position(),
                    obj.team,
                    obj.is_alive(),
                    combat_kind,
                    obj.status.under_construction,
                    crate::game_logic::host_angry_mob::angry_mob_skips_stealthed_undetected(
                        obj.status.stealthed,
                        obj.status.detected,
                        obj.status.disguised,
                    ),
                )
            })
            .collect();

        // ArmTheMob upgrade residual per team (any player on that team).
        let armed_teams: std::collections::HashSet<Team> = self
            .players
            .values()
            .filter(|p| p.has_unlocked_upgrade(UPGRADE_GLA_ARM_THE_MOB))
            .map(|p| p.team)
            .collect();

        // C++ getNextMoodTarget → findClosestEnemy: player ENEMIES, not
        // faction Team. 2v2 GLA+China Allies skipped; FFA same-faction hits.
        let objects = self.objects.map();
        let players = &self.players;
        let plans = self.angry_mobs.plan_due_ticks_with_enemies(
            frame,
            &candidates,
            |team| armed_teams.contains(&team),
            |mob_id, mob_team, tgt_id, tgt_team| {
                Self::angry_mob_relationship_enemies_from_maps(
                    players, objects, mob_id, mob_team, tgt_id, tgt_team,
                )
            },
        );

        for mut plan in plans {
            plan.hits.retain(|hit| {
                self.objects
                    .get(&hit.target_id)
                    .map(|o| o.is_alive() && !o.is_effectively_stealthed())
                    .unwrap_or(false)
            });
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
            let mut audio_pos: Option<Vec3> = None;

            // Rock/molotov lob residual toward primary hit target.
            if let Some(first) = plan.hits.first() {
                use crate::game_logic::host_angry_mob::angry_mob_projectile_kind_for_tick;
                let kind = angry_mob_projectile_kind_for_tick(frame);
                let from = self
                    .objects
                    .get(&plan.mob_id)
                    .map(|o| o.get_position())
                    .unwrap_or(Vec3::ZERO);
                let aim = self
                    .objects
                    .get(&first.target_id)
                    .map(|o| o.get_position())
                    .unwrap_or(from);
                let _ = self.spawn_angry_mob_projectile(
                    plan.mob_id,
                    from,
                    aim,
                    Some(first.target_id),
                    kind,
                );
            }

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() || target.is_effectively_stealthed() {
                        continue;
                    }
                    if audio_pos.is_none() {
                        audio_pos = Some(target.get_position());
                    }
                    let killed = target.take_damage_from_immediate_residual(
                        hit.damage,
                        Some(plan.mob_id),
                        crate::game_logic::host_angry_mob::ANGRY_MOB_PISTOL_DAMAGE_TYPE,
                        crate::game_logic::host_angry_mob::ANGRY_MOB_PISTOL_DEATH_TYPE,
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

            let had_hits = applications > 0;
            self.angry_mobs.record_tick_complete(
                plan.mob_id,
                total_damage,
                applications,
                destroyed,
                frame,
                had_hits,
            );

            // Mark nexus as residual-attacking when it dealt damage (AI state residual).
            if had_hits {
                let first_target = plan.hits.first().map(|h| h.target_id);
                if let Some(mob) = self.objects.get_mut(&plan.mob_id) {
                    mob.set_status_attacking(true);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        if let Some(tid) = first_target {
                            crate::game_logic::host_ai_decision_log::record_attack(
                                plan.mob_id,
                                tid,
                            );
                        }
                        crate::game_logic::host_ai_decision_log::record_set_state(plan.mob_id, 2);
                    } else {
                        if let Some(tid) = first_target {
                            mob.target = Some(tid);
                        }
                        mob.set_ai_state(AIState::Attacking);
                    }
                }
                let muzzle = self
                    .objects
                    .get(&plan.mob_id)
                    .map(|m| m.get_position())
                    .unwrap_or(Vec3::ZERO);
                let impact = audio_pos.or(Some(muzzle));
                let _ = self.combat_particles.spawn_weapon_fire_fx(
                    muzzle,
                    impact,
                    frame,
                    plan.mob_id,
                    None,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(ANGRY_MOB_FIRE_AUDIO)
                        .with_object(plan.mob_id)
                        .with_position(muzzle)
                        .with_priority(150),
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // America Aurora dive bomb residual (delayed FuelAir / AuroraBomb area damage)
    // Fail-closed: not full AuroraBombLocomotor / HeightDieUpdate / gas OCL path.
    // -----------------------------------------------------------------------

    /// Host Aurora dive-bomb residual registry (queue + honesty).
    pub fn aurora_bombs(&self) -> &crate::game_logic::host_aurora_bomb::HostAuroraBombRegistry {
        &self.aurora_bombs
    }

    /// Residual honesty: at least one Aurora bomb dive activated/queued.
    pub fn honesty_aurora_bomb_activate_ok(&self) -> bool {
        self.aurora_bombs.honesty_activate_ok()
    }

    /// Residual honesty: at least one delayed Aurora detonation completed.
    pub fn honesty_aurora_bomb_complete_ok(&self) -> bool {
        self.aurora_bombs.honesty_complete_ok()
    }

    /// Residual honesty: Aurora blast damage applied.
    pub fn honesty_aurora_bomb_damage_ok(&self) -> bool {
        self.aurora_bombs.honesty_damage_ok()
    }

    /// Combined host path honesty for Aurora dive bomb residual.
    pub fn honesty_aurora_bomb_ok(&self) -> bool {
        self.aurora_bombs.honesty_host_path_ok()
    }

    /// Queue a residual Aurora dive bomb at target. Returns mission id.
    ///
    /// Retail path: AuroraBombWeapon → AuroraBomb projectile dive, or
    /// AirF/SupW FuelAir bomb → gas → detonation weapon.
    /// Host residual collapses projectile/gas into delayed area damage.
    /// C++ AuroraBomb SpecialObject residual (AuroraBombLocomotor guided drop).
    pub fn spawn_aurora_bomb_projectile(
        &mut self,
        mission_id: u32,
        source_id: ObjectId,
        source_owner_player_id: Option<u32>,
        from: glam::Vec3,
        aim: glam::Vec3,
        projectile_name: &str,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_aurora_bomb::{
            AURORA_BOMB_HEIGHT_DIE_TARGET, AURORA_BOMB_LOCO_MIN_SPEED, AURORA_BOMB_LOCO_SPEED,
            AURORA_BOMB_PROJECTILE, AURORA_BOMB_PROJECTILE_MAX_HEALTH,
        };
        use crate::game_logic::host_height_die::HostHeightDieData;
        use crate::game_logic::{KindOf, ThingTemplate};

        let name = if projectile_name.is_empty() {
            AURORA_BOMB_PROJECTILE
        } else {
            projectile_name
        };
        if !self.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Projectile)
                .set_health(AURORA_BOMB_PROJECTILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(name.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        // Drop slightly below the aircraft so freefall/guidance is visible.
        let mut start = from;
        if start.y < aim.y + 30.0 {
            start.y = aim.y + 80.0;
        } else {
            start.y -= 5.0;
        }
        let pid =
            self.create_object_for_owner_or_team(name, team, source_owner_player_id, start)?;
        let speed = AURORA_BOMB_LOCO_SPEED / 30.0;
        let min_speed = AURORA_BOMB_LOCO_MIN_SPEED / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        let mut vel = dir * speed.max(min_speed);
        // Bias downward residual (dive bomb).
        vel.y = vel.y.min(-min_speed * 0.35);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.aurora_bomb_projectile = true;
            o.aurora_bomb_aim = Some([aim.x, aim.y, aim.z]);
            o.aurora_bomb_mission_id = Some(mission_id);
            o.note_producer(source_id);
            o.health.maximum = AURORA_BOMB_PROJECTILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, AURORA_BOMB_PROJECTILE_MAX_HEALTH);
            o.movement.velocity = vel;
            o.set_orientation(dir.z.atan2(dir.x));
            o.height_die = Some(HostHeightDieData::with_target(
                AURORA_BOMB_HEIGHT_DIE_TARGET,
                true,
                self.frame.saturating_add(1),
            ));
            o.ensure_height_die(self.frame);
        }
        self.aurora_bombs.record_projectile_spawn();
        Some(pid)
    }

    pub fn update_aurora_bomb_projectiles(&mut self) {
        use crate::game_logic::host_aurora_bomb::{
            AURORA_BOMB_LOCO_MIN_SPEED, AURORA_BOMB_LOCO_SPEED,
        };
        // Drop shells whose mission already completed residual detonation.
        let stale: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.aurora_bomb_projectile || !o.is_alive() {
                    return None;
                }
                match o.aurora_bomb_mission_id {
                    Some(mid) if !self.aurora_bombs.has_mission(mid) => Some(*id),
                    _ => None,
                }
            })
            .collect();
        for id in stale {
            if let Some(o) = self.objects.get_mut(&id) {
                o.aurora_bomb_projectile = false;
                // Wave 753: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
            }
            let team = self.objects.get(&id).map(|o| o.team);
            self.mark_object_for_destruction(id, team);
        }
        let speed = AURORA_BOMB_LOCO_SPEED / 30.0;
        let min_speed = AURORA_BOMB_LOCO_MIN_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.aurora_bomb_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut arrived: Vec<ObjectId> = Vec::new();
        for id in flying {
            let (aim, pos) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .aurora_bomb_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                (aim, o.get_position())
            };
            let to_aim = aim - pos;
            let dist = to_aim.length();
            let vel = if dist > 0.001 {
                let mut v = to_aim.normalize() * speed.max(min_speed);
                // Keep dive component while high.
                if pos.y > aim.y + 10.0 {
                    v.y = v.y.min(-min_speed * 0.5);
                }
                v
            } else {
                glam::Vec3::new(0.0, -speed, 0.0)
            };
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(pos + vel);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let new_pos = pos + vel;
            let near = glam::Vec3::new(aim.x - new_pos.x, 0.0, aim.z - new_pos.z).length() < 8.0
                && new_pos.y <= aim.y + 12.0;
            let height_die = self
                .objects
                .get_mut(&id)
                .map(|o| o.tick_height_die(self.frame, 0.0))
                .unwrap_or(false);
            if near || height_die {
                arrived.push(id);
            }
        }
        for id in arrived {
            // Snap to aim residual and mark dead; detonation is mission-timer driven.
            if let Some(o) = self.objects.get_mut(&id) {
                if let Some(a) = o.aurora_bomb_aim {
                    o.set_position(glam::Vec3::new(a[0], a[1], a[2]));
                }
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
                o.aurora_bomb_projectile = false;
            }
            let team = self.objects.get(&id).map(|o| o.team);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn queue_aurora_bomb(
        &mut self,
        kind: crate::game_logic::host_aurora_bomb::HostAuroraBombKind,
        source_object: ObjectId,
        source_team: Team,
        target_position: Vec3,
    ) -> u32 {
        use crate::game_logic::host_aurora_bomb::AURORA_BOMB_LAUNCH_AUDIO;

        let frame = self.frame;
        let source_owner_player_id = self
            .objects
            .get(&source_object)
            .and_then(|object| self.player_owner_for_host_object(object));
        let id = self.aurora_bombs.queue_for_owner(
            kind,
            source_object,
            source_team,
            source_owner_player_id,
            target_position,
            frame,
        );

        // C++ AuroraBomb SpecialObject residual (guided drop under aircraft).
        let from = self
            .objects
            .get(&source_object)
            .map(|o| o.get_position())
            .unwrap_or(target_position);
        let _ = self.spawn_aurora_bomb_projectile(
            id,
            source_object,
            source_owner_player_id,
            from,
            target_position,
            kind.projectile_object_name(),
        );

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(170),
        );
        // Launch residual particle (not full FX_AuroraBombLaunch).
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponMuzzleFlash,
            self.objects
                .get(&source_object)
                .map(|o| o.get_position())
                .unwrap_or(target_position),
            frame,
            Some(source_object),
            None,
        );
        let _ = AURORA_BOMB_LAUNCH_AUDIO; // name residual documented via activate_audio
        id
    }

    /// Advance pending Aurora dive bombs to impact and apply area damage.
    /// C++ CreateObjectDie OCL_AuroraBombExplode / SupW FuelAir gas SpecialObject residual.
    pub fn spawn_aurora_fuel_air_gas_object(
        &mut self,
        kind: crate::game_logic::host_aurora_bomb::HostAuroraBombKind,
        source_object: ObjectId,
        source_team: Team,
        source_owner_player_id: Option<u32>,
        position: Vec3,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_fuel_air_gas_slow_death::FUEL_AIR_GAS_MAX_HEALTH;
        use crate::game_logic::{KindOf, ThingTemplate};

        let gas_name = kind.fuel_air_gas_object_name()?;
        if !self.templates.contains_key(gas_name) {
            let mut t = ThingTemplate::new(gas_name);
            t.add_kind_of(KindOf::Immobile)
                .set_health(FUEL_AIR_GAS_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(gas_name.to_string(), t);
        }
        let place = Vec3::new(position.x, position.y.max(0.0) + 20.0, position.z);
        let gid = self.create_object_for_owner_or_team(
            gas_name,
            source_team,
            source_owner_player_id,
            place,
        )?;
        if let Some(o) = self.objects.get_mut(&gid) {
            o.note_producer(source_object);
            o.health.maximum = FUEL_AIR_GAS_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, FUEL_AIR_GAS_MAX_HEALTH);
            o.movement.max_speed = 0.0;
            o.weapon = None;
            o.secondary_weapon = None;
            o.ensure_fuel_air_gas_slow_death(self.frame);
        }
        if self
            .objects
            .get(&gid)
            .and_then(|o| o.fuel_air_gas_slow_death.as_ref())
            .is_some()
        {
            self.fuel_air_gas_reg.record_install();
        }
        self.aurora_fuel_air_gas_spawned = self.aurora_fuel_air_gas_spawned.saturating_add(1);
        Some(gid)
    }

    pub fn honesty_aurora_fuel_air_gas_object_ok(&self) -> bool {
        self.aurora_fuel_air_gas_spawned > 0
    }

    pub(crate) fn tick_combat_field_residuals_sole(&mut self) {
        // Wave 826: post-writeback sole-tick for host combat/field residuals.
        self.update_aurora_bombs();
        self.update_supply_drop_zone_drops();
        self.update_point_defense_intercept();
        self.update_mines_and_demo_traps();
        self.update_money_crate_collides();
        self.update_firewall_segment_objects();
        self.update_wave_guides();
        self.update_tensile_formations();
    }

    pub(in super::super) fn update_aurora_bombs(&mut self) {
        self.aurora_bombs.clear_frame_events();

        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .aurora_bombs
            .plan_due_impacts(self.frame, &object_positions);

        for plan in plans {
            // FuelAir: CreateObjectDie gas SpecialObject carries SlowDeath detonation.
            if plan.kind.is_fuel_air() {
                let gas_id = self.spawn_aurora_fuel_air_gas_object(
                    plan.kind,
                    plan.source_object,
                    plan.source_team,
                    plan.source_owner_player_id,
                    plan.target_position,
                );
                // Impact cue residual (bomb shell break / ignite path).
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    plan.target_position,
                    self.frame,
                    Some(plan.source_object),
                    None,
                );
                self.queue_audio_event(
                    AudioEventRequest::new(plan.kind.impact_audio())
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(200),
                );
                self.aurora_bombs.record_impact_complete(
                    plan.mission_id,
                    0.0,
                    if gas_id.is_some() { 1 } else { 0 },
                    0,
                );
                let _ = gas_id;
                continue;
            }

            let mut total_damage = 0.0_f32;
            let mut objects_hit = 0_u32;
            let mut objects_destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    // BodyModule last_damage_source residual for cash bounty killer.
                    let destroyed =
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    objects_hit += 1;
                    if destroyed {
                        objects_destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            // Impact feedback residual: explosion particle + audio at epicenter.
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );
            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.impact_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(200),
            );

            self.aurora_bombs.record_impact_complete(
                plan.mission_id,
                total_damage,
                objects_hit,
                objects_destroyed,
            );

            log::info!(
                "Host Aurora {} bomb {} completed at {:?} (dmg={:.1}, hit={}, killed={})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                total_damage,
                objects_hit,
                objects_destroyed
            );
        }
    }
}

#[cfg(test)]
mod spawn_behavior_parity {
    use super::*;
    use crate::game_logic::host_angry_mob::{
        ANGRY_MOB_EXPAND_INTERVAL_FRAMES, ANGRY_MOB_MAX_MEMBERS,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    fn make_nexus_logic() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut nexus = ThingTemplate::new("GLAInfantryAngryMobNexus");
        nexus
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(99_999.0);
        logic
            .templates
            .insert("GLAInfantryAngryMobNexus".into(), nexus);
        let nid = logic
            .create_object(
                "GLAInfantryAngryMobNexus",
                Team::GLA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("nexus");
        if let Some(o) = logic.objects.get_mut(&nid) {
            o.construction_percent = 1.0;
            o.status.under_construction = false;
        }
        (logic, nid)
    }

    fn live_members(logic: &GameLogic, nid: ObjectId) -> Vec<ObjectId> {
        logic
            .host_objects()
            .values()
            .filter(|o| {
                o.angry_mob_member
                    && o.angry_mob_nexus_id == Some(nid)
                    && o.is_alive()
                    && !o.status.destroyed
            })
            .map(|o| o.id)
            .collect()
    }

    fn kill_member(logic: &mut GameLogic, mid: ObjectId) {
        if let Some(o) = logic.objects.get_mut(&mid) {
            o.health.current = 0.0;
            o.status.destroyed = true;
            o.status.effectively_dead = true;
        }
    }

    #[test]
    fn rapid_spawn_all_spawn_number_then_replace_delay() {
        // C++ SpawnBehavior::update (SpawnBehavior.cpp:194-208, 221-243).
        let (mut logic, nid) = make_nexus_logic();
        logic.update_angry_mobs();
        assert_eq!(
            logic.angry_mobs().member_count_of(nid),
            Some(ANGRY_MOB_MAX_MEMBERS)
        );
        let members = live_members(&logic, nid);
        assert_eq!(members.len() as u32, ANGRY_MOB_MAX_MEMBERS);

        kill_member(&mut logic, members[0]);
        logic.update_angry_mobs();
        assert_eq!(
            logic.angry_mobs().member_count_of(nid),
            Some(ANGRY_MOB_MAX_MEMBERS - 1)
        );

        logic.frame = logic.frame.saturating_add(ANGRY_MOB_EXPAND_INTERVAL_FRAMES);
        logic.update_angry_mobs();
        assert_eq!(
            logic.angry_mobs().member_count_of(nid),
            Some(ANGRY_MOB_MAX_MEMBERS)
        );
        assert!(logic.honesty_angry_mob_expand_ok());
    }

    #[test]
    fn last_member_death_destroys_nexus() {
        // C++ SpawnBehavior::onSpawnDeath (SpawnBehavior.cpp:749-757).
        let (mut logic, nid) = make_nexus_logic();
        logic.update_angry_mobs();
        let members = live_members(&logic, nid);
        assert_eq!(members.len() as u32, ANGRY_MOB_MAX_MEMBERS);
        for mid in members {
            kill_member(&mut logic, mid);
        }
        logic.update_angry_mobs();
        assert_eq!(logic.angry_mobs().member_count_of(nid), None);
        let nexus_gone = logic
            .host_object(nid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true);
        assert!(
            nexus_gone,
            "last member death must destroy AggregateHealth nexus"
        );
    }

    #[test]
    fn nexus_is_selectable_and_not_a_weapon_target() {
        // Live MASKED made the playable mob unselectable. C++ isSelectable
        // does not check MASKED; weapons skip the nexus via template.
        let (mut logic, nid) = make_nexus_logic();
        logic.update_angry_mobs();
        let nexus = logic.host_object(nid).expect("nexus");
        assert!(!nexus.status.masked, "nexus must stay selectable");
        assert!(
            nexus.is_selectable(),
            "player can select the Angry Mob nexus"
        );
        assert!(nexus.can_move(), "nexus locomotor/AI accepts move orders");
        drop(nexus);
        if let Some(n) = logic.objects.get_mut(&nid) {
            n.set_destination(glam::Vec3::new(80.0, 0.0, 0.0));
        }
        assert_eq!(
            logic
                .host_object(nid)
                .and_then(|o| o.movement.target_position),
            Some(glam::Vec3::new(80.0, 0.0, 0.0)),
            "move order sticks on the nexus"
        );
        {
            let nexus = logic.host_object(nid).expect("nexus");
            assert!(
                !nexus.is_targetable_by_enemy_of(Team::USA),
                "weapons must not acquire the 99999-HP nexus"
            );
        }
        let members = live_members(&logic, nid);
        assert!(!members.is_empty());
        let member = logic.host_object(members[0]).expect("member");
        assert!(!member.status.masked);
        assert!(
            member.is_targetable_by_enemy_of(Team::USA),
            "weapons must target live members"
        );
        assert_eq!(
            crate::game_logic::host_angry_mob::remap_angry_mob_selection_id(
                member.angry_mob_member,
                member.angry_mob_nexus_id,
                member.id,
            ),
            nid
        );
    }
}
