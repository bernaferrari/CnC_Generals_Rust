//! Host scripts `impl GameLogic` — `saboteur_car_bomb`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! saboteur / car bomb / pilot / hive / bomb truck
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ ParachuteContain default `KillWhenLandingInWaterSlop`.
const PARACHUTE_KILL_WHEN_LANDING_IN_WATER_SLOP: f32 = 10.0;

impl GameLogic {
    // -----------------------------------------------------------------------
    // GLA Hijack / ConvertToCarBomb residual
    // Fail-closed: not full HijackerUpdate hide-in-vehicle / WeaponSet CARBOMB matrix.
    // -----------------------------------------------------------------------

    /// Host car-bomb / hijack residual registry (honesty counters).
    pub fn car_bomb_residual(&self) -> &crate::game_logic::host_car_bomb::HostCarBombRegistry {
        &self.car_bomb
    }

    /// Host GLA Saboteur residual registry (honesty counters).
    pub fn saboteur_residual(&self) -> &crate::game_logic::host_saboteur::HostSaboteurRegistry {
        &self.saboteur
    }

    /// Residual honesty: at least one structure sabotage completed.
    pub fn honesty_saboteur_ok(&self) -> bool {
        self.saboteur.honesty_any_ok()
    }

    /// Residual honesty: power-plant brownout applied.
    pub fn honesty_saboteur_power_ok(&self) -> bool {
        self.saboteur.honesty_power_ok()
    }

    /// Residual honesty: supply cash steal applied.
    pub fn honesty_saboteur_cash_ok(&self) -> bool {
        self.saboteur.honesty_cash_ok()
    }

    /// Residual honesty: military factory disable applied.
    pub fn honesty_saboteur_military_ok(&self) -> bool {
        self.saboteur.honesty_military_ok()
    }

    /// Residual honesty: at least one hijack transferred a vehicle.
    /// Host USA Pilot residual registry.
    pub fn usa_pilot_residual(&self) -> &crate::game_logic::host_usa_pilot::HostUsaPilotRegistry {
        &self.usa_pilot
    }

    /// Residual honesty: at least one pilot recrew of unmanned vehicle.
    pub fn honesty_pilot_recrew_ok(&self) -> bool {
        self.usa_pilot.honesty_recrew_ok()
    }

    /// Residual honesty: pilot recrew with veterancy transfer observed.
    pub fn honesty_pilot_veterancy_transfer_ok(&self) -> bool {
        self.usa_pilot.honesty_veterancy_transfer_ok()
    }

    /// Residual honesty: EjectPilotDie pilot spawn observed.
    pub fn honesty_pilot_eject_ok(&self) -> bool {
        self.usa_pilot.honesty_eject_ok()
    }

    /// Residual honesty: OCL InvulnerableTime residual granted on eject.
    pub fn honesty_pilot_invulnerable_ok(&self) -> bool {
        self.usa_pilot.honesty_invulnerable_ok()
    }

    /// Residual honesty: InvulnerableTime blocked at least one damage attempt.
    pub fn honesty_pilot_invulnerable_block_ok(&self) -> bool {
        self.usa_pilot.honesty_invulnerable_block_ok()
    }

    /// Residual honesty: PilotFindVehicleUpdate issued at least one Enter order.
    pub fn honesty_pilot_find_vehicle_ok(&self) -> bool {
        self.usa_pilot.honesty_find_vehicle_ok()
    }

    /// Residual honesty: VeterancyLevels REGULAR gate blocked at least one eject.
    pub fn honesty_pilot_eject_veterancy_gate_ok(&self) -> bool {
        self.usa_pilot.honesty_eject_veterancy_gate_ok()
    }

    /// Residual honesty: PilotFindVehicle base-center fallback issued at least once.
    pub fn honesty_pilot_base_center_ok(&self) -> bool {
        self.usa_pilot.honesty_base_center_ok()
    }

    /// Residual honesty: AutoFindHealingUpdate issued at least one SeekingHealing order.
    pub fn honesty_pilot_auto_heal_ok(&self) -> bool {
        self.usa_pilot.honesty_auto_heal_ok()
    }

    /// Residual honesty: EjectPilotDie air OCL parachute residual observed.
    pub fn honesty_pilot_air_eject_ok(&self) -> bool {
        self.usa_pilot.honesty_air_eject_ok()
    }

    /// Residual honesty: air-ejected pilot parachute residual landed.
    pub fn honesty_pilot_parachute_land_ok(&self) -> bool {
        self.usa_pilot.honesty_parachute_land_ok()
    }

    /// Residual honesty: non-pilot USA infantry AutoFindHealing residual issued.
    pub fn honesty_infantry_auto_heal_ok(&self) -> bool {
        self.usa_pilot.honesty_infantry_auto_heal_ok()
    }

    /// Residual honesty: EjectPilotDie DeathTypes CRUSHED/SPLATTED gate blocked.
    pub fn honesty_pilot_eject_death_type_gate_ok(&self) -> bool {
        self.usa_pilot.honesty_eject_death_type_gate_ok()
    }

    /// Residual honesty: EjectPilotDie ExemptStatus HIJACKED gate blocked.
    pub fn honesty_pilot_eject_hijacked_gate_ok(&self) -> bool {
        self.usa_pilot.honesty_eject_hijacked_gate_ok()
    }

    /// Residual honesty: DieMux residual (death type or hijacked) blocked eject.
    pub fn honesty_pilot_eject_die_mux_ok(&self) -> bool {
        self.usa_pilot.honesty_eject_die_mux_ok()
    }

    /// Residual honesty: PilotFindVehicle CollideModule residual rejected a target.
    pub fn honesty_pilot_find_vehicle_collide_ok(&self) -> bool {
        self.usa_pilot.honesty_find_vehicle_collide_ok()
    }

    /// Residual honesty: PilotFindVehicle PartitionFilterPlayer residual rejected.
    pub fn honesty_pilot_find_vehicle_player_ok(&self) -> bool {
        self.usa_pilot.honesty_find_vehicle_player_ok()
    }

    /// Residual honesty: AmericaParachute residual chute opened past OpenDist.
    pub fn honesty_pilot_parachute_open_ok(&self) -> bool {
        self.usa_pilot.honesty_parachute_open_ok()
    }

    /// Residual honesty: AmericaParachute pitch/roll sway residual stepped.
    pub fn honesty_pilot_parachute_sway_ok(&self) -> bool {
        self.usa_pilot.honesty_parachute_sway_ok()
    }

    /// Combined USA Pilot residual honesty.
    pub fn honesty_pilot_ok(&self) -> bool {
        self.usa_pilot.honesty_pilot_ok()
    }

    /// PilotFindVehicleUpdate residual: AI idle pilot auto-scan for recrewable
    /// unmanned vehicles (ScanRate 1000ms → 30 frames, ScanRange 300, MinHealth 0.5).
    ///
    /// C++ PilotFindVehicleUpdate: human players sleep forever; AI issues
    /// `aiEnter` on closest valid target. Host residual maps valid targets onto
    /// the recrewable-unmanned path + VeterancyCrateCollide wouldLikeToCollideWith
    /// gates (not above terrain / not airborne / trainable / can gain exp) +
    /// PartitionFilterPlayer residual (same team / Neutral with matching owner).
    /// When no vehicle found: one-shot base-center fallback (`m_didMoveToBase`).
    /// Fail-closed: not full same-map PartitionFilterSameMapStatus.
    pub(in super::super) fn try_pilot_find_vehicle_residual(&mut self, pilot_id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            PILOT_FIND_VEHICLE_MIN_HEALTH, PILOT_FIND_VEHICLE_SCAN_RANGE,
            is_pilot_find_vehicle_collide_target, is_recrewable_unmanned_vehicle,
            pilot_collide_would_like_to_collide_with, pilot_find_vehicle_scan_eligible,
            pilot_find_vehicle_scan_frame, pilot_levels_to_gain, should_pilot_base_center_fallback,
            vehicle_can_gain_exp_for_levels, vehicle_meets_pilot_find_min_health,
        };

        if !pilot_find_vehicle_scan_frame(self.frame) {
            return;
        }

        let snapshot = match self.objects.get(&pilot_id) {
            Some(obj) if obj.is_alive() => {
                let is_pilot = obj
                    .thing
                    .template
                    .veterancy_crate_collide
                    .as_ref()
                    .is_some_and(|metadata| metadata.supports_pilot_recrew());
                let is_idle = matches!(obj.ai_state, AIState::Idle);
                // C++ PLAYER_HUMAN → no scan. Host residual: local player is human.
                // No mapped player → fail-closed treat as non-AI (no auto-scan).
                let is_ai = self
                    .player_owner_for_host_object(obj)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|p| !p.is_local)
                    .unwrap_or(false);
                if !pilot_find_vehicle_scan_eligible(is_pilot, true, is_idle, is_ai) {
                    return;
                }
                (
                    obj.get_position(),
                    obj.team,
                    obj.selection_radius,
                    obj.status.pilot_did_move_to_base,
                    obj.experience.level,
                )
            }
            _ => return,
        };
        let (pilot_pos, pilot_team, pilot_radius, did_move_to_base, pilot_level) = snapshot;
        let levels_to_gain = pilot_levels_to_gain(pilot_level);

        // Pure residual pilot-vehicle acquire (recrew choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(|(&vid, vehicle)| {
                // RequiredKindOf/ForbiddenKindOf use the authored KindOf
                // bank, not an ObjectType or basename approximation.
                let is_vehicle = vehicle.is_kind_of(KindOf::Vehicle);
                let is_air = vehicle.is_kind_of(KindOf::Aircraft) || vehicle.status.airborne_target;
                let under_construction =
                    vehicle.status.under_construction || vehicle.construction_percent + 0.001 < 1.0;
                let is_dozer = vehicle.is_kind_of(KindOf::Dozer);
                let recrewable = is_recrewable_unmanned_vehicle(
                    vehicle.is_alive(),
                    is_vehicle,
                    is_air,
                    vehicle.is_unmanned(),
                    under_construction,
                    is_dozer,
                );
                let health_ok = vehicle_meets_pilot_find_min_health(
                    vehicle.health.current,
                    vehicle.max_health.max(vehicle.health.maximum),
                    PILOT_FIND_VEHICLE_MIN_HEALTH,
                );
                let vpos = vehicle.get_position();
                let same_player_ok = self.pilot_recrew_controller_matches(pilot_id, vid);
                let terrain_y = self.terrain_height_at(vpos).unwrap_or(0.0);
                let above_terrain =
                    crate::game_logic::host_usa_pilot::is_significantly_above_terrain(
                        vpos.y - terrain_y,
                    );
                let airborne_locomotor = is_air;
                let is_trainable = is_vehicle && !is_air;
                let can_gain =
                    vehicle_can_gain_exp_for_levels(vehicle.experience.level, levels_to_gain);
                let collide_ok = pilot_collide_would_like_to_collide_with(
                    vehicle.is_alive(),
                    is_vehicle,
                    is_dozer,
                    above_terrain,
                    airborne_locomotor,
                    is_trainable,
                    can_gain,
                );
                crate::game_logic::host_residual_acquire::PilotVehicleCandidate {
                    id: vid,
                    position: vpos,
                    recrewable,
                    health_ok,
                    same_player_ok,
                    collide_ok,
                }
            })
            .collect();
        let (best, player_rejects, collide_rejects) =
            crate::game_logic::host_residual_acquire::pick_nearest_pilot_vehicle_target(
                pilot_id,
                pilot_pos,
                candidates,
                PILOT_FIND_VEHICLE_SCAN_RANGE,
            );
        if collide_rejects > 0 {
            for _ in 0..collide_rejects {
                self.usa_pilot.record_find_vehicle_collide_reject();
            }
        }
        if player_rejects > 0 {
            for _ in 0..player_rejects {
                self.usa_pilot.record_find_vehicle_player_reject();
            }
        }

        if let Some((vehicle_id, _, vehicle_pos)) = best {
            // Issue Enter residual (matches player Enter command → recrew path).
            // C++ clears m_didMoveToBase when a vehicle target is found.
            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                pilot.set_target(Some(vehicle_id));
                pilot.set_status_pilot_did_move_to_base(false);
                let _ = pilot_radius;
                let _ = pilot_team;
            }
            self.path_approach_with_state(pilot_id, vehicle_pos, AIState::Entering);
            self.usa_pilot.record_find_vehicle_order();
            return;
        }

        // No vehicle: one-shot base-center fallback residual (getAiBaseCenter).
        // Fail-closed: only a real CommandCenter residual (not any structure /
        // unit fallback — avoids stealing AutoFindHealing hospital residual).
        let base_pos = self.objects.values().find_map(|obj| {
            if obj.team == pilot_team
                && obj.is_alive()
                && (obj.is_kind_of(KindOf::CommandCenter) || obj.is_command_center())
            {
                Some(obj.get_position())
            } else {
                None
            }
        });
        if !should_pilot_base_center_fallback(false, did_move_to_base, base_pos.is_some()) {
            return;
        }
        let Some(base_pos) = base_pos else {
            return;
        };
        if let Some(pilot) = self.objects.get_mut(&pilot_id) {
            pilot.set_target(None);
            pilot.set_status_pilot_did_move_to_base(true);
        }
        self.path_approach_with_state(pilot_id, base_pos, AIState::Moving);
        self.usa_pilot.record_base_center_move();
    }

    /// C++ ParachuteContain::setOverrideDestination residual (DeliverPayload aim).
    pub fn set_parachute_override_destination(
        &mut self,
        chute_or_rider_id: ObjectId,
        dest: glam::Vec3,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&chute_or_rider_id) else {
            return false;
        };
        if !obj.is_parachuting()
            && !obj
                .template_name
                .eq_ignore_ascii_case(crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME)
        {
            // Allow arming on AmericaParachute even before parachuting flag if template matches.
            if !obj.template_name.to_ascii_lowercase().contains("parachute") {
                return false;
            }
        }
        obj.set_parachute_override_destination(dest);
        self.usa_pilot.record_landing_override();
        true
    }

    /// Residual honesty: landingOverride aim + horizontal step observed.
    pub fn honesty_parachute_landing_override_ok(&self) -> bool {
        self.usa_pilot.honesty_landing_override_ok()
    }

    /// OCL_EjectPilotViaParachute residual: freefall → OpenDist open → sink to ground.
    ///
    /// AmericaParachute residual: freefall at faster rate until fallen
    /// `ParachuteOpenDist` (100), then open chute (slower sink + open audio) and
    /// pitch/roll spring-damper sway residual while open.
    ///
    /// Also drives HijackerUpdate PutInContainer AmericaParachute residual:
    /// chute Object sinks, riders sync position, ground collide → removeAllContained
    /// + kill chute (C++ ParachuteContain::onCollide). Empty `containCount==0`
    /// kills the chute mid-air (C++ ParachuteContain::update).
    /// Fail-closed: not full bone PARA_COG / DeliverPayload matrix.
    pub(crate) fn tick_eject_parachute_residual(&mut self, pilot_id: ObjectId) {
        use crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME;
        use crate::game_logic::host_usa_pilot::{
            PILOT_PARACHUTE_LAND_AUDIO, PILOT_PARACHUTE_OPEN_AUDIO, is_pilot_template,
            should_open_parachute, tick_parachute_height_with_state, tick_parachute_sway,
        };

        let (
            pos,
            is_pilot,
            is_chute,
            is_infantry_rider,
            contained_by,
            chute_open,
            start_h,
            pitch,
            roll,
            pitch_rate,
            roll_rate,
            landing_override,
        ) = match self.objects.get(&pilot_id) {
            Some(obj) if obj.is_alive() && obj.is_parachuting() => {
                let name = obj.template_name.as_str();
                (
                    obj.get_position(),
                    is_pilot_template(name),
                    name.eq_ignore_ascii_case(HIJACKER_PARACHUTE_NAME),
                    obj.is_kind_of(KindOf::Infantry) || obj.object_type == ObjectType::Infantry,
                    obj.contained_by,
                    obj.is_parachute_open(),
                    obj.status.parachute_start_height,
                    obj.status.parachute_pitch,
                    obj.status.parachute_roll,
                    obj.status.parachute_pitch_rate,
                    obj.status.parachute_roll_rate,
                    obj.parachute_landing_override(),
                )
            }
            _ => return,
        };

        // Riders inside AmericaParachute: chute drives sink; just soft-sync.
        if let Some(cid) = contained_by {
            if self
                .objects
                .get(&cid)
                .map(|c| {
                    c.template_name
                        .eq_ignore_ascii_case(HIJACKER_PARACHUTE_NAME)
                        && c.is_alive()
                })
                .unwrap_or(false)
            {
                if let Some(chute) = self.objects.get(&cid) {
                    let cp = chute.get_position();
                    let open = chute.is_parachute_open();
                    if let Some(r) = self.objects.get_mut(&pilot_id) {
                        r.set_position(cp);
                        crate::game_logic::host_ground_height_log::record(pilot_id, cp.y, false);
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            r.record_host_movement();
                        }
                        if open && !r.is_parachute_open() {
                            r.open_eject_parachute();
                        }
                    }
                }
                return;
            }
        }

        // Drive sink for: AmericaParachute containers, ejected pilots, and
        // parachuting infantry (hijacker residual without/with container).
        if !(is_chute || is_pilot || is_infantry_rider) {
            return;
        }

        // Host residual ground height 0 (flat terrain residual; not full TerrainLogic).
        let ground = 0.0_f32;
        // OpenDist residual: freefall until fallen ≥ 100, then open chute.
        let mut just_opened = false;
        let mut open = chute_open;
        if !open && should_open_parachute(start_h, pos.y) {
            open = true;
            just_opened = true;
        }
        let (new_y, landed) = tick_parachute_height_with_state(pos.y, ground, open);
        // Pitch/roll sway residual only while chute open (C++ m_opened gate).
        let mut did_sway = false;
        let sway = if open && !just_opened && !landed {
            did_sway = true;
            Some(tick_parachute_sway(
                pitch,
                roll,
                pitch_rate,
                roll_rate,
                (new_y - ground).max(0.0),
            ))
        } else {
            None
        };

        // C++ ParachuteContain::update open: findPositionAround(0, 100) unless override.
        let mut resolved_override = landing_override;
        if just_opened && resolved_override.is_none() {
            use crate::game_logic::host_ocl_special_power::find_ocl_passable_around;
            if let Some(found) =
                find_ocl_passable_around(pos, 100.0, |p| self.parachute_open_lz_clear(p))
            {
                resolved_override = Some(found);
            }
        }

        // C++ open chute → aiMoveToPosition(landingOverride / found LZ).
        let mut nx = pos.x;
        let mut nz = pos.z;
        let mut did_override_step = false;
        if open && !landed {
            if let Some(target) = resolved_override {
                use crate::game_logic::host_usa_pilot::{
                    PARACHUTE_LANDING_OVERRIDE_SPEED, step_parachute_landing_override,
                };
                let (sx, sz, moved) = step_parachute_landing_override(
                    pos.x,
                    pos.z,
                    target.x,
                    target.z,
                    PARACHUTE_LANDING_OVERRIDE_SPEED,
                );
                if moved {
                    nx = sx;
                    nz = sz;
                    did_override_step = true;
                }
            }
        }

        let land_pos = Vec3::new(nx, if landed { ground } else { new_y }, nz);
        let riders_to_release: Vec<ObjectId> = if is_chute && landed {
            self.objects
                .get(&pilot_id)
                .map(|c| c.contained_units())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if let Some(obj) = self.objects.get_mut(&pilot_id) {
            if just_opened {
                obj.open_eject_parachute();
                if let Some(lz) = resolved_override {
                    obj.set_parachute_override_destination(lz);
                }
            }

            let mut p = obj.get_position();
            p.x = land_pos.x;
            p.z = land_pos.z;
            p.y = new_y;
            if landed {
                p.y = ground;
            }
            obj.set_position(p);
            crate::game_logic::host_ground_height_log::record(pilot_id, ground, false);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                // Freefall residual is not path-integrate; host pose stays residual,
                // but landing destination is logged for GameWorld move channel.
                if landed {
                    crate::game_logic::host_move_log::record(pilot_id, Some([p.x, p.y, p.z]));
                }
                obj.record_host_movement();
            }
            if let Some((np, nr, npr, nrr)) = sway {
                obj.status.parachute_pitch = np;
                obj.status.parachute_roll = nr;
                obj.status.parachute_pitch_rate = npr;
                obj.status.parachute_roll_rate = nrr;
            }
            if landed && !is_chute {
                obj.clear_eject_parachuting();
            }
        }
        if landed && !is_chute {
            // C++ ParachuteContain::onRemoving water/cliff/impassable/off-map.
            self.apply_parachute_landing_legality_kill(pilot_id, land_pos);
            self.apply_parachute_land_ai(pilot_id);
        }

        // Sync contained riders to chute while descending.
        if is_chute && !landed {
            let ids = self
                .objects
                .get(&pilot_id)
                .map(|c| c.contained_units())
                .unwrap_or_default();
            for rid in ids {
                if let Some(r) = self.objects.get_mut(&rid) {
                    r.set_position(land_pos);
                    crate::game_logic::host_ground_height_log::record(rid, ground, false);
                    if crate::gameworld_shadow::gameworld_movement_authority_live() {
                        r.record_host_movement();
                    }
                    if open && !r.is_parachute_open() {
                        r.open_eject_parachute();
                    }
                }
            }
        }

        // C++ ParachuteContain::onCollide(null): removeAllContained.
        if is_chute && landed {
            for rid in &riders_to_release {
                if let Some(chute) = self.objects.get_mut(&pilot_id) {
                    let _ = chute.exit_transport(*rid);
                }
                if let Some(r) = self.objects.get_mut(rid) {
                    r.set_contained_by(None);
                    r.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(*rid, 0);
                    }
                    r.set_position(land_pos);
                    crate::game_logic::host_ground_height_log::record(*rid, ground, false);
                    if crate::gameworld_shadow::gameworld_movement_authority_live() {
                        crate::game_logic::host_move_log::record(
                            *rid,
                            Some([land_pos.x, land_pos.y, land_pos.z]),
                        );
                        r.record_host_movement();
                    }
                    r.clear_eject_parachuting();
                    // Partition restore residual after chute dump.
                    r.set_status_masked(false);
                    r.set_status_unselectable(false);
                    r.set_status_no_collisions(false);
                    r.stop_moving();
                    r.target = None;
                }
            }
            for rid in &riders_to_release {
                // C++ ParachuteContain::onRemoving after removeAllContained.
                self.apply_parachute_landing_legality_kill(*rid, land_pos);
            }
            for rid in &riders_to_release {
                self.apply_parachute_land_ai(*rid);
            }
            // Hijacker airborne PutInContainer land honesty.
            self.car_bomb.record_airborne_parachute_land();
        }

        // C++ ParachuteContain::update (ParachuteContain.cpp:411-413):
        // "If we have lost our passenger for whatever reason, die early."
        // Leftover parachute_contain.rs:853-855 — altitude-independent empty kill
        // after OpenContain sweep, before water slop. HELD skips the kill
        // (C++ update returns before the empty-count check).
        if is_chute {
            let ids = self
                .objects
                .get(&pilot_id)
                .map(|c| c.contained_units())
                .unwrap_or_default();
            for rid in ids {
                let gone = self
                    .objects
                    .get(&rid)
                    .map(|o| !o.is_alive())
                    .unwrap_or(true);
                if gone {
                    if let Some(chute) = self.objects.get_mut(&pilot_id) {
                        let _ = chute.exit_transport(rid);
                    }
                    if let Some(r) = self.objects.get_mut(&rid) {
                        if r.contained_by == Some(pilot_id) {
                            r.set_contained_by(None);
                        }
                    }
                }
            }
            let held = self
                .objects
                .get(&pilot_id)
                .is_some_and(|c| c.status.disabled_held);
            let empty = self
                .objects
                .get(&pilot_id)
                .map(|c| c.contained_units().is_empty())
                .unwrap_or(true);
            if !held && empty {
                if let Some(chute) = self.objects.get_mut(&pilot_id) {
                    chute.clear_eject_parachuting();
                    let hp = chute.health.current.max(1.0);
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        crate::game_logic::host_damage_log::record(pilot_id, hp, None, true);
                    } else {
                        chute.health.current = 0.0;
                    }
                    chute.status.destroyed = true;
                }
                self.mark_object_for_destruction(pilot_id, None);
            }
        }

        if just_opened {
            self.usa_pilot.record_parachute_open();
            self.queue_audio_event(
                AudioEventRequest::new(PILOT_PARACHUTE_OPEN_AUDIO)
                    .with_position(Vec3::new(pos.x, new_y, pos.z))
                    .with_priority(145),
            );
        }
        if did_sway {
            self.usa_pilot.record_parachute_sway_tick();
        }
        if did_override_step {
            self.usa_pilot.record_landing_override_step();
        }
        if landed {
            self.usa_pilot.record_parachute_land();
            self.queue_audio_event(
                AudioEventRequest::new(PILOT_PARACHUTE_LAND_AUDIO)
                    .with_position(Vec3::new(pos.x, ground, pos.z))
                    .with_priority(140),
            );
        }
    }

    /// C++ ParachuteContain::onRemoving landing-legality kills.
    /// Water (within KillWhenLandingInWaterSlop) → DAMAGE_WATER / DEATH_FLOODED.
    /// Off-map / CELL_CLIFF / CELL_WATER / CELL_IMPASSABLE → kill().
    fn apply_parachute_landing_legality_kill(&mut self, rider_id: ObjectId, land_pos: Vec3) {
        use crate::game_logic::host_partition_collision_physics_residual::PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use gamelogic::ai::pathfind_astar::PathfindCellType;

        let Some(rider) = self.objects.get(&rider_id) else {
            return;
        };
        if !rider.is_alive() || rider.status.destroyed {
            return;
        }

        let water_z = self.terrain.as_ref().and_then(|t| t.water_plane_y);
        let underwater = self
            .terrain
            .as_ref()
            .is_some_and(|t| t.is_underwater_at_world(land_pos));
        let water_kill = water_z.is_some_and(|wz| {
            underwater && land_pos.y <= wz + PARACHUTE_KILL_WHEN_LANDING_IN_WATER_SLOP
        });

        if water_kill {
            if let Some(r) = self.objects.get_mut(&rider_id) {
                let _ = r.take_damage_from_typed_death(
                    PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                    None,
                    crate::game_logic::combat::DamageType::Water,
                    HostDeathType::Flooded,
                );
            }
        }

        let still_alive = self
            .objects
            .get(&rider_id)
            .is_some_and(|r| r.is_alive() && !r.status.destroyed && r.health.current > 0.0);
        if !still_alive {
            self.mark_object_for_destruction(rider_id, None);
            return;
        }

        let cell = self.pathfinding_system.grid.world_to_grid(land_pos);
        let bad_cell = self.pathfinding_system.grid.is_valid_pos(cell)
            && matches!(
                self.pathfinding_system.grid.cell_type(cell),
                PathfindCellType::Cliff | PathfindCellType::Water | PathfindCellType::Impassable
            );
        let cliff_terrain = self
            .terrain
            .as_ref()
            .is_some_and(|t| t.is_cliff_at_world(land_pos));
        let off_map = land_pos.x < self.world_min.x
            || land_pos.x > self.world_max.x
            || land_pos.z < self.world_min.z
            || land_pos.z > self.world_max.z;

        if !(off_map || bad_cell || cliff_terrain) {
            return;
        }

        if let Some(r) = self.objects.get_mut(&rider_id) {
            let hp = r.health.current.max(1.0);
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                crate::game_logic::host_damage_log::record(rider_id, hp, None, true);
            } else {
                r.health.current = 0.0;
            }
            r.status.destroyed = true;
        }
        self.mark_object_for_destruction(rider_id, None);
    }

    /// C++ PartitionManager::findPositionAround clear-cell residual for chute open.
    fn parachute_open_lz_clear(&self, pos: glam::Vec3) -> bool {
        use gamelogic::ai::pathfind_astar::PathfindCellType;
        if pos.x < self.world_min.x
            || pos.x > self.world_max.x
            || pos.z < self.world_min.z
            || pos.z > self.world_max.z
        {
            return false;
        }
        if self
            .terrain
            .as_ref()
            .is_some_and(|t| t.is_cliff_at_world(pos) || t.is_underwater_at_world(pos))
        {
            return false;
        }
        let cell = self.pathfinding_system.grid.world_to_grid(pos);
        if !self.pathfinding_system.grid.is_valid_pos(cell) {
            return false;
        }
        !matches!(
            self.pathfinding_system.grid.cell_type(cell),
            PathfindCellType::Cliff | PathfindCellType::Water | PathfindCellType::Impassable
        )
    }

    /// C++ ParachuteContain::onRemoving: skirmish AI → aiHunt; else
    /// rider.producer (chute) → chute.producer (building) UseSpawnRallyPoint
    /// → DefaultProductionExitUpdate::exitObjectViaDoor; else aiIdle.
    fn apply_parachute_land_ai(&mut self, rider_id: ObjectId) {
        let Some(rider) = self.objects.get(&rider_id) else {
            return;
        };
        if !rider.is_alive() || rider.status.destroyed {
            return;
        }
        let skirmish = self
            .player_owner_for_host_object(rider)
            .and_then(|pid| self.ai_manager.ai_difficulty(pid))
            .is_some();
        if skirmish {
            let _ = self.unit_command_patrol(rider_id);
            return;
        }

        // C++: transport = rider->getProducerID(); building = transport->getProducerID().
        let transport_id = rider.producer_id;
        let building_id = transport_id.and_then(|tid| self.objects.get(&tid)?.producer_id);
        let use_spawn = building_id.is_some_and(|bid| {
            self.objects.get(&bid).is_some_and(|building| {
                building
                    .thing
                    .template
                    .production_exit_metadata
                    .is_some_and(|exit| exit.use_spawn_rally_point)
            })
        });
        if let Some(building_id) = building_id.filter(|_| use_spawn) {
            self.exit_parachute_rider_via_spawn_rally(rider_id, building_id);
            return;
        }

        // C++ riderAI->aiIdle(CMD_FROM_AI).
        if let Some(r) = self.objects.get_mut(&rider_id) {
            r.set_ai_state(AIState::Idle);
            r.hunting = false;
            r.target = None;
            r.stop_moving();
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(rider_id, 0);
            }
        }
    }

    /// C++ DefaultProductionExitUpdate::exitObjectViaDoor(rider, DOOR_1).
    /// UnitCreatePoint + 2-cell natural rally + custom rally; ignore the producer.
    fn exit_parachute_rider_via_spawn_rally(&mut self, rider_id: ObjectId, building_id: ObjectId) {
        let Some(building) = self.objects.get(&building_id) else {
            return;
        };
        let Some(exit) = building.thing.template.production_exit_metadata else {
            return;
        };
        if !exit.use_spawn_rally_point {
            return;
        }
        let prod_pos = building.get_position();
        let forward = building.thing.get_direction_vector();
        let orientation = building.get_orientation();
        let custom_rally = building.building_data.as_ref().and_then(|b| b.rally_point);
        let create = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
            prod_pos,
            forward,
            (
                exit.unit_create_point[0],
                exit.unit_create_point[1],
                exit.unit_create_point[2],
            ),
        );
        let create = glam::Vec3::new(create.x, 0.0, create.z);
        let natural_pt = exit.natural_rally_point_with_path_offset(
            crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F,
        );
        let natural = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
            prod_pos,
            forward,
            (natural_pt[0], natural_pt[1], natural_pt[2]),
        );
        let natural = glam::Vec3::new(natural.x, 0.0, natural.z);

        if let Some(unit) = self.objects.get_mut(&rider_id) {
            unit.set_position(create);
            unit.set_orientation(orientation);
            crate::game_logic::host_ground_height_log::record(rider_id, 0.0, false);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    rider_id,
                    Some([create.x, create.y, create.z]),
                );
                unit.record_host_movement();
            }
        }

        let mut exit_path = vec![natural];
        if let Some(rally) = custom_rally {
            exit_path.push(rally);
        }
        self.path_approach_with_state_ignoring(
            rider_id,
            exit_path[0],
            AIState::Moving,
            Some(building_id),
        );
        for &wp in exit_path.iter().skip(1) {
            let already_at = self.objects.get(&rider_id).is_some_and(|unit| {
                unit.movement
                    .path
                    .last()
                    .is_some_and(|last| last.distance(wp) < 0.1)
                    || unit
                        .movement
                        .target_position
                        .is_some_and(|dest| dest.distance(wp) < 0.1)
            });
            if already_at {
                if let Some(unit) = self.objects.get_mut(&rider_id) {
                    unit.movement.path.push(wp);
                    unit.movement.target_position = Some(wp);
                }
            } else {
                let _ = self.append_unit_waypoint(rider_id, wp);
            }
        }
        if let Some(unit) = self.objects.get_mut(&rider_id) {
            unit.can_path_through_units = true;
        }
    }

    /// AutoFindHealingUpdate residual: AI idle injured USA infantry auto-scan for HealPad.
    ///
    /// Retail ModuleTag: ScanRate 1000ms → 30 frames, ScanRange 300, NeverHeal 0.85,
    /// AlwaysHeal 0.25. Templates: Pilot / Ranger / MissileDefender / Pathfinder /
    /// ColonelBurton residual. C++ human players skip; host residual: is_local → no
    /// auto-scan. Idle-only residual (AlwaysHeal busy-interrupt path fail-closed —
    /// C++ early-return makes busy path unreachable). Issues SeekingHealing toward
    /// nearest HealPad in range.
    pub(in super::super) fn try_auto_find_healing_residual(&mut self, unit_id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            AUTO_FIND_HEALING_NEVER_HEAL, AUTO_FIND_HEALING_SCAN_RANGE,
            auto_find_healing_scan_eligible, auto_find_healing_scan_frame,
            health_needs_auto_find_healing, is_auto_find_healing_target,
            is_auto_find_healing_template, is_pilot_template,
        };

        if !auto_find_healing_scan_frame(self.frame) {
            return;
        }

        let snapshot = match self.objects.get(&unit_id) {
            Some(obj) if obj.is_alive() => {
                // Parachuting residual: no hospital auto-scan mid-air.
                if obj.is_parachuting() {
                    return;
                }
                let has_module = is_auto_find_healing_template(&obj.template_name);
                let is_idle = matches!(obj.ai_state, AIState::Idle);
                let is_ai = self
                    .player_owner_for_host_object(obj)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|p| !p.is_local)
                    .unwrap_or(false);
                if !auto_find_healing_scan_eligible(has_module, true, is_idle, is_ai) {
                    return;
                }
                let max_hp = obj.max_health.max(obj.health.maximum);
                if !health_needs_auto_find_healing(
                    obj.health.current,
                    max_hp,
                    AUTO_FIND_HEALING_NEVER_HEAL,
                ) {
                    return;
                }
                (
                    obj.get_position(),
                    obj.team,
                    is_pilot_template(&obj.template_name),
                )
            }
            _ => return,
        };
        let (unit_pos, unit_team, is_pilot) = snapshot;

        // Nearest HealPad residual (C++ KINDOF_HEAL_PAD; host name/BuildingType residual).
        // Pure residual service-target acquire (heal pad choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(|(&hid, pad)| {
                let under_construction =
                    pad.status.under_construction || pad.construction_percent + 0.001 < 1.0;
                let is_heal_pad = pad
                    .building_data
                    .as_ref()
                    .map(|b| b.building_type == BuildingType::HealPad)
                    .unwrap_or_else(|| {
                        let lower = pad.template_name.to_ascii_lowercase();
                        lower.contains("hospital")
                            || lower.contains("heal")
                            || lower.contains("medic")
                    });
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: hid,
                    team: pad.team,
                    position: pad.get_position(),
                    is_alive: pad.is_alive(),
                    is_neutral: pad.team == Team::Neutral,
                    under_construction,
                    // Reuse combat_kind as service-match residual for heal pads.
                    combat_kind: is_heal_pad,
                    effectively_stealthed: false,
                    is_air: false,
                    eject_invulnerable: false,
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_service_target(
            unit_id,
            unit_pos,
            candidates,
            AUTO_FIND_HEALING_SCAN_RANGE,
            |c| {
                // Fail-closed: only own/ally/neutral heal pads.
                let team_ok = c.team == unit_team || c.is_neutral;
                !c.under_construction
                    && c.is_alive
                    && team_ok
                    && is_auto_find_healing_target(c.combat_kind, true, true)
            },
        );

        let Some((pad_id, _, pad_pos)) = best else {
            return;
        };

        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.set_target(Some(pad_id));
        }
        self.path_approach_with_state(unit_id, pad_pos, AIState::SeekingHealing);
        if is_pilot {
            self.usa_pilot.record_auto_heal_order();
        } else {
            self.usa_pilot.record_infantry_auto_heal_order();
        }
    }

    /// Apply residual damage to an object, honoring InvulnerableTime residual.
    /// Returns (destroyed, blocked_by_invuln).
    ///
    /// Default residual class is HitStructure (non-hive path). Use
    /// [`Self::apply_host_hive_damage`] for Stinger HiveStructureBody matrix.
    pub fn apply_host_damage(&mut self, id: ObjectId, damage: f32) -> (bool, bool) {
        self.apply_host_hive_damage(
            id,
            damage,
            crate::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
        )
    }

    /// Apply residual damage with HiveStructureBody damage-class residual.
    ///
    /// For Stinger Sites:
    /// - PropagateToSlaves / SwallowIfNoSlaves route through residual soldiers
    /// - HitStructure damages the structure body
    /// Also honors InvulnerableTime residual and CamoNetting TAKING_DAMAGE uncloak.
    pub fn apply_host_hive_damage(
        &mut self,
        id: ObjectId,
        damage: f32,
        class: crate::game_logic::host_base_defense::HostHiveDamageClass,
    ) -> (bool, bool) {
        self.apply_host_hive_damage_from(id, damage, class, None, None)
    }

    /// HiveStructureBody residual with optional shooter for getClosestSlave.
    ///
    /// Live skirmish `Object::take_damage_from_typed` now runs the same
    /// propagate/swallow matrix (hq-c5cin / hq-j6a24). This API remains for
    /// tests and residual honesty counters (closest-slave / swallow stats).
    ///
    /// `shooter_xz`: world (x, z) of damage source. When set, residual damages
    /// the closest physical slave slot (C++ `getClosestSlave(shooter->pos)`).
    pub fn apply_host_hive_damage_from(
        &mut self,
        id: ObjectId,
        damage: f32,
        class: crate::game_logic::host_base_defense::HostHiveDamageClass,
        shooter_xz: Option<(f32, f32)>,
        source_id: Option<ObjectId>,
    ) -> (bool, bool) {
        use crate::game_logic::host_base_defense::{
            STINGER_SOLDIER_DIE_AUDIO, is_stinger_site_structure, next_stinger_slave_respawn_frame,
            resolve_hive_structure_damage_roster, sync_hive_slave_mirrors,
        };

        // Snapshot flags before mutating so we can update honesty counters after
        // releasing the object borrow.
        let (is_stinger, was_stealthed_on_damage, invuln, struct_hp, pos, respawn_frame, site_xz) = {
            let Some(obj) = self.objects.get(&id) else {
                return (false, false);
            };
            let p = obj.get_position();
            (
                is_stinger_site_structure(&obj.template_name),
                obj.stealth_breaks_on_damage && obj.status.stealthed,
                obj.is_eject_invulnerable(),
                obj.health.current,
                p,
                obj.hive_slave_respawn_frame,
                (p.x, p.z),
            )
        };

        if invuln {
            self.usa_pilot.record_invulnerable_block();
            return (false, true);
        }

        if is_stinger {
            let frame = self.frame;
            let shooter_arg = shooter_xz.map(|(qx, qz)| (site_xz.0, site_xz.1, qx, qz));

            let mut camo_reveal = false;
            let mut destroyed = false;
            let (result_slave_dmg, result_killed, result_swallowed, closest_idx) = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    return (false, false);
                };
                // Roster is source of truth. Align alive count to mirror only when
                // an external residual path wrote count alone (preserve per-slot HP).
                {
                    use crate::game_logic::host_base_defense::{
                        align_hive_roster_to_count, count_alive_hive_slaves,
                    };
                    let roster_alive = count_alive_hive_slaves(&obj.hive_slaves);
                    if roster_alive != obj.hive_slave_count {
                        align_hive_roster_to_count(&mut obj.hive_slaves, obj.hive_slave_count);
                        // If mirror said one active partial HP and we just revived
                        // first slot from empty, apply mirror HP to first alive.
                        if roster_alive == 0 && obj.hive_slave_count > 0 && obj.hive_slave_hp > 0.0
                        {
                            if let Some(slot) = obj.hive_slaves.iter_mut().find(|s| s.alive) {
                                slot.hp = obj.hive_slave_hp;
                            }
                        }
                    }
                }
                let (_, new_struct_hp, result) = resolve_hive_structure_damage_roster(
                    &mut obj.hive_slaves,
                    struct_hp,
                    damage,
                    class,
                    shooter_arg,
                );
                let (new_count, new_hp) = sync_hive_slave_mirrors(&obj.hive_slaves);
                obj.hive_slave_count = new_count;
                obj.record_host_hive();
                obj.hive_slave_hp = new_hp;
                obj.record_host_hive();
                if result.slaves_killed > 0 {
                    obj.hive_slave_respawn_frame =
                        next_stinger_slave_respawn_frame(frame, respawn_frame);
                }
                // TAKING_DAMAGE residual: any damage attempt uncloaks CamoNetting structures.
                if obj.stealth_breaks_on_damage && obj.status.stealthed {
                    obj.break_stealth();
                    camo_reveal = true;
                }
                if result.structure_damage_applied > 0.0 {
                    // Wave 748: under damage authority, do not mutate host HP
                    // (dual with GW HP writeback). Damage log owns the numeric
                    // residual; lethal still stamps destroyed + idle residual.
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        crate::game_logic::host_damage_log::record(
                            obj.id,
                            result.structure_damage_applied,
                            None,
                            new_struct_hp <= 0.0,
                        );
                    } else {
                        obj.health.current = new_struct_hp;
                        crate::game_logic::host_damage_log::record(
                            obj.id,
                            result.structure_damage_applied,
                            None,
                            new_struct_hp <= 0.0,
                        );
                        if new_struct_hp <= 0.0 {
                            obj.health.current = 0.0;
                        }
                    }
                    if new_struct_hp <= 0.0 {
                        obj.status.destroyed = true;
                        let hid = obj.id;
                        obj.set_ai_state(AIState::Idle);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(hid, 0);
                        }
                        obj.target = None;
                        destroyed = true;
                    }
                }
                (
                    result.slave_damage_applied,
                    result.slaves_killed,
                    result.swallowed,
                    result.closest_slave_index,
                )
            };

            if camo_reveal || was_stealthed_on_damage {
                self.camo_netting_structure_residual_reveals = self
                    .camo_netting_structure_residual_reveals
                    .saturating_add(1);
            }
            if result_slave_dmg > 0.0 {
                self.stinger_hive_residual_slave_hits =
                    self.stinger_hive_residual_slave_hits.saturating_add(1);
                if closest_idx.is_some() && shooter_xz.is_some() {
                    self.stinger_hive_residual_closest_slave_hits = self
                        .stinger_hive_residual_closest_slave_hits
                        .saturating_add(1);
                }
            }
            if result_killed > 0 {
                self.stinger_hive_residual_slave_kills = self
                    .stinger_hive_residual_slave_kills
                    .saturating_add(result_killed);
                self.queue_audio_event(
                    AudioEventRequest::new(STINGER_SOLDIER_DIE_AUDIO)
                        .with_object(id)
                        .with_position(pos)
                        .with_priority(140),
                );
            }
            if result_swallowed {
                self.stinger_hive_residual_swallows =
                    self.stinger_hive_residual_swallows.saturating_add(1);
            }
            return (destroyed, false);
        }

        let (destroyed, camo_reveal) = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return (false, false);
            };
            let before = obj.status.stealthed && obj.stealth_breaks_on_damage;
            let destroyed = obj.take_damage_from(damage, source_id);
            let revealed = before && (!obj.status.stealthed || obj.stealth_delay_pending);
            (destroyed, revealed)
        };
        self.flush_pending_garrison_really_damaged_ejects();
        if camo_reveal {
            self.camo_netting_structure_residual_reveals = self
                .camo_netting_structure_residual_reveals
                .saturating_add(1);
        }
        (destroyed, false)
    }

    /// Advance Stinger SpawnBehavior residual slave respawns (SpawnReplaceDelay).
    pub fn update_stinger_hive_respawns(&mut self) {
        use crate::game_logic::host_base_defense::{
            STINGER_SOLDIER_MAX_HEALTH, STINGER_SPAWN_NUMBER, count_alive_hive_slaves,
            is_stinger_site_structure, next_stinger_slave_respawn_frame, respawn_one_hive_slave,
            should_respawn_stinger_slave, sync_hive_slave_mirrors,
        };

        let frame = self.frame;
        let mut respawned = 0u32;
        for obj in self.objects.values_mut() {
            if !is_stinger_site_structure(&obj.template_name) || !obj.is_alive() {
                continue;
            }
            // Roster is source of truth; align only when mirror count diverges
            // (tests / legacy paths). Does not clobber HP of already-alive slots.
            {
                use crate::game_logic::host_base_defense::align_hive_roster_to_count;
                let roster_alive = count_alive_hive_slaves(&obj.hive_slaves);
                if roster_alive != obj.hive_slave_count {
                    align_hive_roster_to_count(&mut obj.hive_slaves, obj.hive_slave_count);
                    if roster_alive == 0 && obj.hive_slave_count > 0 && obj.hive_slave_hp > 0.0 {
                        if let Some(slot) = obj.hive_slaves.iter_mut().find(|s| s.alive) {
                            slot.hp = obj.hive_slave_hp;
                        }
                    }
                }
            }
            if !should_respawn_stinger_slave(
                obj.hive_slave_count,
                frame,
                obj.hive_slave_respawn_frame,
            ) {
                // Ensure a respawn is scheduled when below capacity.
                if obj.hive_slave_count < STINGER_SPAWN_NUMBER as u8
                    && obj.hive_slave_respawn_frame == 0
                {
                    obj.hive_slave_respawn_frame = next_stinger_slave_respawn_frame(frame, 0);
                }
                continue;
            }
            // Physical roster residual: revive first dead SpawnPoint slot.
            if respawn_one_hive_slave(&mut obj.hive_slaves) {
                let (c, h) = sync_hive_slave_mirrors(&obj.hive_slaves);
                obj.hive_slave_count = c;
                obj.record_host_hive();
                obj.hive_slave_hp = h;
                obj.record_host_hive();
            } else {
                // Count-only fallback residual.
                obj.hive_slave_count = obj
                    .hive_slave_count
                    .saturating_add(1)
                    .min(STINGER_SPAWN_NUMBER as u8);
                if obj.hive_slave_count == 1 {
                    obj.hive_slave_hp = STINGER_SOLDIER_MAX_HEALTH;
                    obj.record_host_hive();
                }
            }
            respawned = respawned.saturating_add(1);
            if obj.hive_slave_count < STINGER_SPAWN_NUMBER as u8 {
                obj.hive_slave_respawn_frame = next_stinger_slave_respawn_frame(frame, 0);
            } else {
                obj.hive_slave_respawn_frame = 0;
            }
        }
        self.stinger_hive_residual_respawns = self
            .stinger_hive_residual_respawns
            .saturating_add(respawned);
        self.update_stinger_hive_world_soldiers();
    }

    /// Host GLA Worker residual registry.
    pub fn gla_worker_residual(
        &self,
    ) -> &crate::game_logic::host_gla_worker::HostGlaWorkerRegistry {
        &self.gla_worker
    }

    /// Residual honesty: WorkerShoes speed applied to workers.
    pub fn honesty_worker_shoes_apply_ok(&self) -> bool {
        self.gla_worker.honesty_shoes_apply_ok()
    }

    /// Residual honesty: WorkerShoes supply boost on drop-off.
    pub fn honesty_worker_shoes_boost_ok(&self) -> bool {
        self.gla_worker.honesty_shoes_boost_ok()
    }

    /// Combined GLA Worker residual honesty.
    pub fn honesty_worker_ok(&self) -> bool {
        self.gla_worker.honesty_worker_ok()
    }

    pub fn honesty_hijack_ok(&self) -> bool {
        self.car_bomb.honesty_hijack_ok()
    }

    /// Residual honesty: at least one ConvertToCarBomb conversion.
    pub fn honesty_carbomb_convert_ok(&self) -> bool {
        self.car_bomb.honesty_convert_ok()
    }

    /// Residual honesty: at least one car-bomb detonation with observable damage.
    pub fn honesty_carbomb_detonate_ok(&self) -> bool {
        self.car_bomb.honesty_detonate_ok()
    }

    /// Combined residual honesty: any hijack / convert / detonate path observed.
    pub fn honesty_carbomb_ok(&self) -> bool {
        self.car_bomb.honesty_any_ok()
    }

    /// Host bomb-truck disguise residual registry.
    pub fn bomb_truck_disguise(
        &self,
    ) -> &crate::game_logic::host_bomb_truck_disguise::HostBombTruckDisguiseRegistry {
        &self.bomb_truck_disguise
    }

    /// Residual honesty: at least one bomb-truck disguise applied.
    pub fn honesty_bomb_truck_disguise_ok(&self) -> bool {
        self.bomb_truck_disguise.honesty_disguise_ok()
    }

    /// Residual honesty: disguiseAsObject copied from already-disguised target.
    pub fn honesty_bomb_truck_disguise_copy_ok(&self) -> bool {
        self.bomb_truck_disguise.honesty_disguise_copy_ok()
    }

    /// Residual honesty: Internet Center sabotage disabled SpyVision residual.
    pub fn honesty_internet_center_spy_vision_ok(&self) -> bool {
        self.saboteur.honesty_internet_spy_vision_ok()
    }

    /// Residual honesty: Internet Center sabotage disabled contained hackers.
    pub fn honesty_internet_center_hackers_disabled_ok(&self) -> bool {
        self.saboteur.honesty_internet_hackers_disabled_ok()
    }

    /// Residual honesty: at least one bomb-truck disguise reveal.
    pub fn honesty_bomb_truck_reveal_ok(&self) -> bool {
        self.bomb_truck_disguise.honesty_reveal_ok()
    }

    /// Combined bomb-truck disguise residual honesty.
    pub fn honesty_bomb_truck_disguise_path_ok(&self) -> bool {
        self.bomb_truck_disguise.honesty_host_path_ok()
    }

    // -----------------------------------------------------------------------
    // GLA Bomb Truck HE/Bio FireWeaponWhenDead residual
    // Fail-closed: not full exclusive module matrix / SubObjectsUpgrade visuals.
    // -----------------------------------------------------------------------

    /// Host Bomb Truck detonation residual registry.
    pub fn bomb_truck_detonate(
        &self,
    ) -> &crate::game_logic::host_bomb_truck_detonate::HostBombTruckDetonateRegistry {
        &self.bomb_truck_detonate
    }

    pub fn honesty_bomb_truck_detonate_ok(&self) -> bool {
        self.bomb_truck_detonate.honesty_detonate_ok()
    }

    pub fn honesty_bomb_truck_he_ok(&self) -> bool {
        self.bomb_truck_detonate.honesty_he_ok()
    }

    pub fn honesty_bomb_truck_bio_ok(&self) -> bool {
        self.bomb_truck_detonate.honesty_bio_ok()
    }

    pub fn honesty_bomb_truck_detonate_path_ok(&self) -> bool {
        self.bomb_truck_detonate.honesty_host_path_ok()
    }

    /// Apply residual HE/Bio detonation at a Bomb Truck death site.
    ///
    /// Retail path: FireWeaponWhenDeadBehavior exclusive damage + effect weapons.
    /// Fail-closed: not full RequiresAllTriggers / ConflictsWith module ordering /
    /// SubObjectsUpgrade Bombload visuals / Anthrax Gamma matrix.
    ///
    /// Called from `process_destroy_list` after the truck is removed from the map;
    /// uses snapshot position/team/profile from the destroyed object.
    pub fn apply_bomb_truck_death_detonation_at(
        &mut self,
        truck_id: ObjectId,
        truck_team: Team,
        truck_pos: Vec3,
        profile: crate::game_logic::host_bomb_truck_detonate::BombTruckDetonationProfile,
    ) -> bool {
        use crate::game_logic::host_bomb_truck_detonate::{
            BOMB_TRUCK_DAMAGE_TYPE, BOMB_TRUCK_DEATH_TYPE, BOMB_TRUCK_POISON_AUDIO,
            bomb_truck_blast_damage_at,
        };

        let max_radius = profile.secondary_radius();

        let mut damage_dealt = 0.0f32;
        let mut blast_hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            if vid == truck_id {
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
                let dx = vpos.x - truck_pos.x;
                let dz = vpos.z - truck_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            if dist > max_radius {
                continue;
            }
            let dmg = bomb_truck_blast_damage_at(profile, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                damage_dealt += dmg.min(victim.health.current.max(0.0));
                blast_hits = blast_hits.saturating_add(1);
                if victim.take_damage_from_immediate_residual(
                    dmg,
                    Some(truck_id),
                    BOMB_TRUCK_DAMAGE_TYPE,
                    BOMB_TRUCK_DEATH_TYPE,
                ) {
                    destroy_ids.push((vid, truck_team));
                }
            }
        }

        self.bomb_truck_detonate
            .record_detonation(profile, blast_hits, damage_dealt);

        if profile.spawns_poison() {
            let _ = self.bomb_truck_detonate.spawn_poison_field(
                truck_id,
                truck_team,
                truck_pos,
                self.frame,
                profile.poison_upgraded(),
            );
            self.queue_audio_event(
                AudioEventRequest::new(BOMB_TRUCK_POISON_AUDIO)
                    .with_object(truck_id)
                    .with_position(truck_pos)
                    .with_priority(140),
            );
        }

        self.queue_audio_event(
            AudioEventRequest::new(profile.detonate_audio())
                .with_object(truck_id)
                .with_position(truck_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            truck_pos,
            self.frame,
            Some(truck_id),
            None,
        );

        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Advance Bomb Truck BioBomb MediumPoisonField residual zones.
    /// AutoFindRepair residual: AI idle damaged vehicles seek RepairPad / WarFactory.
    ///
    /// Fail-closed vs full AutoFindRepairUpdate INI matrix — host residual for AI only,
    /// idle damaged ground vehicles/aircraft when health < 70%.
    pub(in super::super) fn try_auto_find_repair_residual(&mut self, unit_id: ObjectId) {
        // Throttle with healing scan cadence residual (30 frames).
        use crate::game_logic::host_usa_pilot::auto_find_healing_scan_frame;
        if !auto_find_healing_scan_frame(self.frame) {
            return;
        }

        let snapshot = match self.objects.get(&unit_id) {
            Some(obj) if obj.is_alive() => {
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft)
                    || obj.object_type == ObjectType::Vehicle
                    || obj.object_type == ObjectType::Aircraft;
                if !is_vehicle || obj.is_kind_of(KindOf::Structure) {
                    return;
                }
                if !matches!(obj.ai_state, AIState::Idle) || obj.target.is_some() {
                    return;
                }
                // Human player residual: no auto-seek.
                let is_ai = self
                    .player_owner_for_host_object(obj)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|p| !p.is_local)
                    .unwrap_or(false);
                if !is_ai {
                    return;
                }
                let max_hp = obj.max_health.max(obj.health.maximum).max(1.0);
                let ratio = obj.health.current / max_hp;
                // Never seek above 70% residual (retail-ish NeverRepair).
                if ratio >= 0.70 {
                    return;
                }
                if !obj.can_move() {
                    return;
                }
                (obj.get_position(), obj.team)
            }
            _ => return,
        };
        let (unit_pos, unit_team) = snapshot;

        const SCAN_RANGE: f32 = 400.0;
        // Pure residual service-target acquire (repair pad choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(|(&pid, pad)| {
                let name = pad.template_name.to_ascii_lowercase();
                let is_pad = pad
                    .building_data
                    .as_ref()
                    .map(|b| {
                        matches!(
                            b.building_type,
                            BuildingType::RepairPad
                                | BuildingType::WarFactory
                                | BuildingType::Airfield
                        )
                    })
                    .unwrap_or(false)
                    || name.contains("repair")
                    || name.contains("warfactory")
                    || name.contains("war_factory")
                    || name.contains("airfield")
                    || name.contains("air_field");
                crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: pid,
                    team: pad.team,
                    position: pad.get_position(),
                    is_alive: pad.is_alive() && pad.is_constructed(),
                    is_neutral: pad.team == Team::Neutral,
                    under_construction: pad.status.under_construction || pad.status.sold,
                    combat_kind: is_pad,
                    effectively_stealthed: false,
                    is_air: false,
                    eject_invulnerable: false,
                }
            })
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_service_target(
            unit_id,
            unit_pos,
            candidates,
            SCAN_RANGE,
            |c| c.is_alive && c.team == unit_team && !c.under_construction && c.combat_kind,
        );
        let Some((pad_id, _, pad_pos)) = best else {
            return;
        };
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            obj.set_target(Some(pad_id));
            obj.target_location = None;
        }
        self.path_approach_with_state(unit_id, pad_pos, AIState::SeekingRepair);
    }

    /// AI dozer/worker residual: idle builders resume unfinished ally structures.
    pub(in super::super) fn try_auto_resume_construction_residual(&mut self, unit_id: ObjectId) {
        use crate::game_logic::host_usa_pilot::auto_find_healing_scan_frame;
        if !auto_find_healing_scan_frame(self.frame) {
            return;
        }
        let snapshot = match self.objects.get(&unit_id) {
            Some(obj) if obj.is_alive() => {
                let name = obj.template_name.to_ascii_lowercase();
                let is_builder = obj.is_worker()
                    || name.contains("dozer")
                    || name.contains("worker")
                    || name.contains("crane");
                if !is_builder || !matches!(obj.ai_state, AIState::Idle) || obj.target.is_some() {
                    return;
                }
                if !obj.can_move() {
                    return;
                }
                let is_ai = self
                    .player_owner_for_host_object(obj)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|p| !p.is_local)
                    .unwrap_or(false);
                if !is_ai {
                    return;
                }
                (obj.get_position(), obj.team)
            }
            _ => return,
        };
        let (unit_pos, unit_team) = snapshot;
        const SCAN: f32 = 350.0;
        // Pure residual service-target acquire (unfinished structure choice phase).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .map(
                |(&sid, st)| crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: sid,
                    team: st.team,
                    position: st.get_position(),
                    is_alive: st.is_alive(),
                    is_neutral: st.team == Team::Neutral,
                    under_construction: st.status.under_construction,
                    combat_kind: st.is_kind_of(KindOf::Structure),
                    effectively_stealthed: false,
                    is_air: false,
                    eject_invulnerable: false,
                },
            )
            .collect();
        let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_service_target(
            unit_id,
            unit_pos,
            candidates,
            SCAN,
            |c| c.is_alive && c.team == unit_team && c.under_construction && c.combat_kind,
        );
        let Some((tid, _, tpos)) = best else {
            return;
        };
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            // Construction target association stays host.
            obj.set_target(Some(tid));
            obj.target_location = None;
        }
        self.set_ai_state_decision_aware(unit_id, AIState::Constructing);
        self.path_approach_with_state(unit_id, tpos, AIState::Constructing);
    }

    pub(in super::super) fn update_bomb_truck_poison_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                (
                    *id,
                    obj.get_position(),
                    obj.team,
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                )
            })
            .collect();

        let plans = self
            .bomb_truck_detonate
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
                    let killed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        crate::game_logic::host_poisoned_behavior::poison_weapon_damage_type(),
                        plan.death_type,
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

            self.bomb_truck_detonate.record_tick_complete(
                plan.zone_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.bomb_truck_detonate.prune_expired(frame);
    }
}
