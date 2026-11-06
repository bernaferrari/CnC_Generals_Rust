//! GPS scrambler, repair, firewall, radar and satellite markers.

use super::*;

impl GameLogic {
    /// C++ OCL GPSScrambler_InvisibleMarker residual.
    pub fn spawn_gps_scrambler_marker(&mut self, team: Team, position: Vec3) -> Option<ObjectId> {
        self.spawn_gps_scrambler_marker_with_effects(
            team,
            position,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_gps_scrambler_marker_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_gps_scrambler::GPS_SCRAMBLER_INVISIBLE_MARKER;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(GPS_SCRAMBLER_INVISIBLE_MARKER) {
            let mut t = ThingTemplate::new(GPS_SCRAMBLER_INVISIBLE_MARKER);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(GPS_SCRAMBLER_INVISIBLE_MARKER.to_string(), t);
        }
        let mid = self.create_object_with_script_effects(
            GPS_SCRAMBLER_INVISIBLE_MARKER,
            team,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.gps_scrambler_marker = true;
        }
        self.gps_scramblers.record_marker_spawn();
        Some(mid)
    }

    /// C++ GrantStealthBehavior.cpp:147-150 `TheGameLogic->destroyObject(self)`
    /// after the FinalRadius scan. Invisible marker has no SlowDeath — hard-remove
    /// like Frenzy DeletionUpdate teardown.
    pub(super) fn destroy_gps_scrambler_marker(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        self.destroy_gps_scrambler_marker_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(super) fn destroy_gps_scrambler_marker_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if !self
            .objects
            .get(&id)
            .map(|o| o.gps_scrambler_marker)
            .unwrap_or(false)
        {
            return Ok(());
        }
        self.destroy_object_with_effects(id, presentation_effects)?;
        Ok(())
    }

    /// C++ GrantStealthBehavior radius grow pulse residual (Start 20 → Final 100).
    pub fn update_gps_scrambler_grow(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_gps_scrambler_grow_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_gps_scrambler_grow_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_gps_scrambler::{
            GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL, gps_scrambler_grow_is_final,
            gps_scrambler_scan_radius_after_updates, in_gps_scrambler_radius_2d,
            is_gps_scrambler_disguise_name, is_legal_gps_scrambler_target,
        };

        // Collect grow work without holding registry mut across object mut.
        let mut markers_to_destroy: Vec<ObjectId> = Vec::new();
        let work: Vec<(u32, Vec3, f32, Team, Option<ObjectId>, u32)> = {
            let mut out = Vec::new();
            for a in self.gps_scramblers.growing_missions_mut() {
                if a.grow_index >= GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL {
                    a.growing = false;
                    a.radius =
                        gps_scrambler_scan_radius_after_updates(a.grow_index.saturating_sub(1));
                    if let Some(mid) = a.marker_id.take() {
                        markers_to_destroy.push(mid);
                    }
                    continue;
                }
                let radius = gps_scrambler_scan_radius_after_updates(a.grow_index);
                a.radius = radius;
                a.grow_index = a.grow_index.saturating_add(1);
                let this_is_final = gps_scrambler_grow_is_final(a.grow_index.saturating_sub(1))
                    || a.grow_index >= GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL;
                if this_is_final {
                    a.growing = false;
                    // C++ GrantStealthBehavior.cpp:147-150 after the FinalRadius scan.
                    if let Some(mid) = a.marker_id.take() {
                        markers_to_destroy.push(mid);
                    }
                }
                let team = a
                    .caster_id
                    .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
                    .unwrap_or(Team::GLA);
                out.push((a.id, a.location, radius, team, a.caster_id, a.player_id));
            }
            out
        };

        for (_aid, location, radius, team, caster_id, player_id) in work {
            self.gps_scramblers.record_grow_pulse();
            let center = (location.x, location.z);
            let candidates: Vec<(ObjectId, bool, bool, bool, bool, bool, bool)> = self
                .objects
                .registered()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    if obj.gps_scrambler_marker {
                        return None;
                    }
                    let pos = obj.get_position();
                    if !in_gps_scrambler_radius_2d(center, (pos.x, pos.z), radius) {
                        return None;
                    }
                    let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                    let is_infantry = obj.is_kind_of(KindOf::Infantry);
                    let is_ally = self.gps_grant_is_ally(player_id, caster_id, team, obj);
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    let is_disguise = is_gps_scrambler_disguise_name(&obj.template_name);
                    let has_stealth_module = obj.has_gps_stealth_module();
                    Some((
                        *id,
                        is_vehicle,
                        is_infantry,
                        is_ally,
                        under_construction,
                        is_disguise,
                        has_stealth_module,
                    ))
                })
                .collect();

            let mut grants = 0u32;
            for (
                id,
                is_vehicle,
                is_infantry,
                is_ally,
                under_construction,
                is_disguise,
                has_stealth_module,
            ) in candidates
            {
                if !is_legal_gps_scrambler_target(
                    is_vehicle,
                    is_infantry,
                    true,
                    is_ally,
                    under_construction,
                    is_disguise,
                    has_stealth_module,
                ) {
                    continue;
                }
                let Some(target) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                let was = target.is_effectively_stealthed();
                target.apply_grant_stealth();
                // C++ grantStealthToObject: receiveGrant() then draw->flashAsSelected().
                target.flash_as_selected();
                if !was || target.is_effectively_stealthed() {
                    grants = grants.saturating_add(1);
                }
            }
            if grants > 0 {
                // Bookkeeping on registry grant_count via record if available.
                self.gps_scramblers.grant_count =
                    self.gps_scramblers.grant_count.saturating_add(grants);
            }
            let _ = caster_id;
        }

        // C++ GrantStealthBehavior.cpp:147-150 destroyObject after the final scan.
        for mid in markers_to_destroy {
            self.destroy_gps_scrambler_marker_with_effects(mid, presentation_effects)?;
        }
        let owned: std::collections::HashSet<ObjectId> = self
            .gps_scramblers
            .growing_missions_mut()
            .filter_map(|a| a.marker_id)
            .collect();
        let orphans: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                (obj.gps_scrambler_marker && obj.is_alive() && !owned.contains(id)).then_some(*id)
            })
            .collect();
        for mid in orphans {
            self.destroy_gps_scrambler_marker_with_effects(mid, presentation_effects)?;
        }
        Ok(())
    }

    /// C++ OCL RepairVehiclesInArea_InvisibleMarker residual (DeletionUpdate 0 = same-frame die).
    pub fn spawn_emergency_repair_marker(
        &mut self,
        team: Team,
        position: Vec3,
        level: crate::game_logic::host_emergency_repair::HostEmergencyRepairLevel,
    ) -> Option<ObjectId> {
        self.spawn_emergency_repair_marker_with_effects(
            team,
            position,
            level,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_emergency_repair_marker_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        level: crate::game_logic::host_emergency_repair::HostEmergencyRepairLevel,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::{KindOf, ThingTemplate};

        let tmpl = level.marker_template();
        if !self.templates.contains_key(tmpl) {
            let mut t = ThingTemplate::new(tmpl);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates.insert(tmpl.to_string(), t);
        }
        let mid = self.create_object_with_script_effects(
            tmpl,
            team,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.emergency_repair_marker = true;
            // DeletionUpdate Lifetime 0 residual: die immediately after pulse.
            // Wave 752: under damage authority, do not zero host HP mid-frame
            // (dual with GW HP writeback). Project lethal via damage log + flags.
            {
                o.internal_change_health(-o.health.current);
            }
            o.status.destroyed = true;
            o.status.effectively_dead = true;
        }
        self.emergency_repairs.record_marker_spawn();
        // Avoid full destroy pipeline on the one-pulse marker residual.
        Some(mid)
    }

    /// C++ AmericaVehicleSpyDrone DynamicShroudClearingRangeUpdate grow residual.
    /// Expands the spawned scout's shroud-clearing range 0→250; look follows the unit.
    pub fn update_spy_drone_grow(&mut self) {
        use crate::game_logic::host_spy_drone::{
            SPY_DRONE_GROW_UPDATES_TO_FINAL, SPY_DRONE_VISION_RANGE, spy_drone_grow_is_final,
            spy_drone_scan_radius_after_updates,
        };

        let work: Vec<(usize, Option<crate::game_logic::ObjectId>, f32)> = {
            let acts = self.spy_drones.activations_mut();
            let mut out = Vec::new();
            for (i, a) in acts.iter_mut().enumerate() {
                if !a.growing {
                    continue;
                }
                if a.grow_index >= SPY_DRONE_GROW_UPDATES_TO_FINAL {
                    a.growing = false;
                    a.radius = SPY_DRONE_VISION_RANGE;
                    continue;
                }
                let radius = spy_drone_scan_radius_after_updates(a.grow_index);
                a.radius = radius;
                a.grow_index = a.grow_index.saturating_add(1);
                if spy_drone_grow_is_final(a.grow_index.saturating_sub(1)) {
                    a.growing = false;
                    a.radius = SPY_DRONE_VISION_RANGE;
                }
                out.push((i, a.spawned_id, a.radius));
            }
            out
        };

        if work.is_empty() {
            return;
        }

        for (i, spawned_id, radius) in work {
            if let Some(id) = spawned_id {
                if let Some(obj) = self.objects.get_mut(&id) {
                    if obj.is_alive() {
                        obj.set_shroud_clearing_range_with_partition_owner(
                            radius,
                            &mut self.partition_manager,
                        );
                        obj.vision_range = SPY_DRONE_VISION_RANGE;
                        let pos = obj.get_position();
                        if let Some(act) = self.spy_drones.activations_mut().get_mut(i) {
                            act.location = pos;
                            act.radius = radius;
                        }
                    }
                }
            }
            self.spy_drones.record_grow_pulse();
        }
    }

    /// C++ OCL_FireWallSegment CreateObject FireWallSegment residual.
    pub fn spawn_firewall_segment_objects(
        &mut self,
        wall_id: u32,
        source_object: ObjectId,
        source_team: Team,
    ) -> u32 {
        self.spawn_firewall_segment_objects_with_effects(
            wall_id,
            source_object,
            source_team,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_firewall_segment_objects_with_effects(
        &mut self,
        wall_id: u32,
        source_object: ObjectId,
        source_team: Team,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        use crate::game_logic::host_firewall::{
            FIREWALL_DURATION_FRAMES, FIREWALL_SEGMENT_MAX_HEALTH, HostFireWallRegistry,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (positions, upgraded): (Vec<Vec3>, bool) = self
            .fire_walls
            .active_walls()
            .iter()
            .find(|w| w.id == wall_id)
            .map(|w| (w.segments.iter().map(|s| s.position).collect(), w.upgraded))
            .unwrap_or_default();
        if positions.is_empty() {
            return 0;
        }
        let template = HostFireWallRegistry::wall_segment_template(upgraded);
        if !self.templates.contains_key(template) {
            let mut t = ThingTemplate::new(template);
            t.add_kind_of(KindOf::Immobile)
                .set_health(FIREWALL_SEGMENT_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(template.to_string(), t);
        }
        let (dir_x, dir_z) = self
            .fire_walls
            .active_walls()
            .iter()
            .find(|w| w.id == wall_id)
            .map(|w| (w.dir_x, w.dir_z))
            .unwrap_or((1.0, 0.0));
        let expires = self.frame.saturating_add(FIREWALL_DURATION_FRAMES);
        let mut spawned = 0u32;
        for pos in positions {
            if let Some(sid) = self.create_object_with_script_effects(
                template,
                source_team,
                pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&sid) {
                    o.firewall_segment = true;
                    o.note_producer(source_object);
                    o.firewall_segment_expires_frame = Some(expires);
                    o.firewall_segment_wall_id = Some(wall_id);
                    o.firewall_segment_dir = Some([dir_x, dir_z]);
                    o.set_body_max_health_with_mode(
                        FIREWALL_SEGMENT_MAX_HEALTH,
                        crate::game_logic::object::MaxHealthChange::FullyHeal,
                    );
                    self.health_events
                        .record_applied_heal(o.id, o.health.current);
                }
                spawned = spawned.saturating_add(1);
            }
        }
        self.fire_walls.record_segment_spawns(spawned);
        spawned
    }

    pub fn update_firewall_segment_objects(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_firewall_segment_objects_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_firewall_segment_objects_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_firewall::FIREWALL_INCH_PER_FRAME;

        let frame = self.frame;

        // C++ InchForwardLocomotor residual: segments crawl along wall direction.
        // Keep registry damage zones and live segment objects in lockstep.
        self.fire_walls.crawl_segments();
        // Wave 809: under coupled shadow, object crawl/expire owned by GW.
        {}
        let crawlers: Vec<(ObjectId, f32, f32)> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if !o.firewall_segment || !o.is_alive() {
                    return None;
                }
                let dir = o.firewall_segment_dir.unwrap_or([1.0, 0.0]);
                Some((
                    *id,
                    dir[0] * FIREWALL_INCH_PER_FRAME,
                    dir[1] * FIREWALL_INCH_PER_FRAME,
                ))
            })
            .collect();
        for (id, dx, dz) in crawlers {
            if let Some(o) = self.objects.get_mut(&id) {
                let mut p = o.get_position();
                p.x += dx;
                p.z += dz;
                o.set_position_with_effects_and_pose_owners(
                    p,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
            }
        }

        let due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.firewall_segment {
                    if let Some(exp) = o.firewall_segment_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.firewall_segment = false;
                o.firewall_segment_wall_id = None;
                o.firewall_segment_dir = None;
            }
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        Ok(())
    }

    /// C++ OCL SUPERWEAPON_RadarVanScan CreateObject RadarVanPing residual.
    pub fn spawn_radar_van_ping(
        &mut self,
        team: Team,
        position: Vec3,
        caster_id: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.spawn_radar_van_ping_with_effects(
            team,
            position,
            caster_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_radar_van_ping_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        caster_id: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_radar_scan::{
            RADAR_SCAN_DURATION_FRAMES, RADAR_SCAN_STEALTH_DETECTION_RANGE,
            RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES, RADAR_VAN_PING_TEMPLATE,
        };
        use crate::game_logic::host_strategy_center::stealth_detector_hold_frames;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(RADAR_VAN_PING_TEMPLATE) {
            let mut t = ThingTemplate::new(RADAR_VAN_PING_TEMPLATE);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(RADAR_VAN_PING_TEMPLATE.to_string(), t);
        }
        let pid = self.create_object_with_script_effects(
            RADAR_VAN_PING_TEMPLATE,
            team,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.radar_van_ping = true;
            o.producer_id = caster_id;
            // C++ StealthDetectorUpdate.cpp:167-282 on RadarVanPing
            // (DetectionRate 500ms, DetectionRange 0 → VisionRange 150).
            o.set_detector_state(
                true,
                RADAR_SCAN_STEALTH_DETECTION_RANGE,
                RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES,
            );
            o.radar_van_ping_expires_frame =
                Some(self.frame.saturating_add(RADAR_SCAN_DURATION_FRAMES));
        }
        let (extra_required, extra_forbidden) = self
            .objects
            .get(&pid)
            .map(|o| (o.extra_detect_kindof, o.extra_detect_kindof_not))
            .unwrap_or((0, 0));
        // First DetectionRate scan is immediate (C++ UPDATE_SLEEP_NONE).
        let hold = stealth_detector_hold_frames(RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES);
        let expires = self.frame.saturating_add(hold);
        let range_sq = RADAR_SCAN_STEALTH_DETECTION_RANGE * RADAR_SCAN_STEALTH_DETECTION_RANGE;
        let stealthed: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(id, o)| {
                *id != &pid
                    && o.is_alive()
                    && o.team != team
                    && (o.status.stealthed || o.status.disguised)
                    && crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        o.kind_of_cpp_mask(),
                        extra_required,
                        extra_forbidden,
                    )
            })
            .filter(|(_, o)| {
                let p = o.get_position();
                let dx = p.x - position.x;
                let dz = p.z - position.z;
                dx * dx + dz * dz <= range_sq
            })
            .map(|(id, _)| *id)
            .collect();
        let mut garrison_detect_recalc: Vec<ObjectId> = Vec::new();
        for sid in stealthed {
            let (contained_by, already_detected) = self
                .objects
                .get(&sid)
                .map(|o| (o.contained_by, o.status.detected))
                .unwrap_or((None, false));
            if let Some(o) = self.objects.get_mut(&sid) {
                o.mark_detected(expires);
            }
            self.order_idle_enemies_to_attack_on_reveal(sid);
            // C++ StealthDetectorUpdate.cpp:198-282 first reveal (UPDATE_SLEEP_NONE).
            if !already_detected {
                self.fire_stealth_discover_feedback(sid, &[pid]);
            }
            if let Some(cid) = contained_by {
                garrison_detect_recalc.push(cid);
            }
        }
        self.recalc_garrisons_after_occupant_detect_change(&garrison_detect_recalc);
        self.radar_scans.record_ping_spawn();
        Some(pid)
    }

    pub fn update_radar_van_pings(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_radar_van_pings_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_radar_van_pings_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.radar_van_ping {
                    if let Some(exp) = o.radar_van_ping_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.radar_van_ping = false;
            }
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        Ok(())
    }

    /// C++ OCL SUPERWEAPON_SpySatellite CreateObject SpySatellitePing residual.
    /// SpySatellitePing carries StealthDetectorUpdate (DetectionRate 500ms,
    /// DetectionRange 0 → VisionRange 300) so stealthed units in the scan
    /// are DETECTED while the ping lives.
    pub fn spawn_spy_satellite_ping(
        &mut self,
        team: Team,
        position: Vec3,
        caster_id: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.spawn_spy_satellite_ping_with_effects(
            team,
            position,
            caster_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_spy_satellite_ping_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        caster_id: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_spy_satellite::{
            SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_PING_TEMPLATE,
            SPY_SATELLITE_STEALTH_DETECTION_RANGE, SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES,
        };
        use crate::game_logic::host_strategy_center::stealth_detector_hold_frames;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SPY_SATELLITE_PING_TEMPLATE) {
            let mut t = ThingTemplate::new(SPY_SATELLITE_PING_TEMPLATE);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(SPY_SATELLITE_PING_TEMPLATE.to_string(), t);
        }
        let owner_player_id = match caster_id.and_then(|caster| self.objects.get(&caster)) {
            Some(caster) if caster.owner_player_id.is_some() => {
                Some(self.player_owner_for_host_object(caster)?)
            }
            Some(_) => None,
            None => None,
        };
        let pid = self.create_object_for_owner_or_team_with_effects(
            SPY_SATELLITE_PING_TEMPLATE,
            team,
            owner_player_id,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.spy_satellite_ping = true;
            o.producer_id = caster_id;
            o.set_detector_state(
                true,
                SPY_SATELLITE_STEALTH_DETECTION_RANGE,
                SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES,
            );
            // DeletionUpdate lifetime residual tracked via destroy at duration.
            o.spy_satellite_ping_expires_frame =
                Some(self.frame.saturating_add(SPY_SATELLITE_DURATION_FRAMES));
        }
        let (extra_required, extra_forbidden) = self
            .objects
            .get(&pid)
            .map(|o| (o.extra_detect_kindof, o.extra_detect_kindof_not))
            .unwrap_or((0, 0));
        // First DetectionRate scan is immediate (C++ UPDATE_SLEEP_NONE).
        let hold = stealth_detector_hold_frames(SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES);
        let expires = self.frame.saturating_add(hold);
        let range_sq =
            SPY_SATELLITE_STEALTH_DETECTION_RANGE * SPY_SATELLITE_STEALTH_DETECTION_RANGE;
        let stealthed: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(id, o)| {
                *id != &pid
                    && o.is_alive()
                    && o.team != team
                    && (o.status.stealthed || o.status.disguised)
                    && crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        o.kind_of_cpp_mask(),
                        extra_required,
                        extra_forbidden,
                    )
            })
            .filter(|(_, o)| {
                let p = o.get_position();
                let dx = p.x - position.x;
                let dz = p.z - position.z;
                dx * dx + dz * dz <= range_sq
            })
            .map(|(id, _)| *id)
            .collect();
        let mut garrison_detect_recalc: Vec<ObjectId> = Vec::new();
        for sid in stealthed {
            let contained_by = self.objects.get(&sid).and_then(|o| o.contained_by);
            if let Some(obj) = self.objects.get_mut(&sid) {
                obj.mark_detected(expires);
            }
            self.order_idle_enemies_to_attack_on_reveal(sid);
            if let Some(cid) = contained_by {
                garrison_detect_recalc.push(cid);
            }
        }
        self.recalc_garrisons_after_occupant_detect_change(&garrison_detect_recalc);
        self.spy_satellites.record_ping_spawn();
        Some(pid)
    }

    pub fn update_spy_satellite_pings(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_spy_satellite_pings_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_spy_satellite_pings_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.spy_satellite_ping {
                    if let Some(exp) = o.spy_satellite_ping_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.spy_satellite_ping = false;
            }
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        Ok(())
    }

    pub fn apply_ocl_random_force(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_ocl_apply_random_force::{
            apply_random_force_plan_for, calc_random_force, spin_nudge_rad,
        };
        let Some(o) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let plan = match apply_random_force_plan_for(&o.template_name) {
            Some(p) => p,
            None => return false,
        };
        let salt = object_id.0.wrapping_add(self.frame);
        let force = calc_random_force(&plan, salt);
        o.movement.velocity += force * 0.05;
        let spin = spin_nudge_rad(&plan, salt);
        o.set_orientation_with_partition_owner(
            o.get_orientation() + spin,
            &mut self.partition_manager,
        );
        self.ocl_apply_random_force_reg.record(force);
        true
    }
}
