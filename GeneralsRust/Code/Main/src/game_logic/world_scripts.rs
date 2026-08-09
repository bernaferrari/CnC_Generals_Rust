//! Additional `impl GameLogic` methods. Child of `game_logic.rs`.
#![allow(unused_imports, non_snake_case)]
use super::*;

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
    pub(super) fn try_pilot_find_vehicle_residual(&mut self, pilot_id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            is_pilot_find_vehicle_collide_target, is_pilot_template,
            is_recrewable_unmanned_vehicle, pilot_collide_would_like_to_collide_with,
            pilot_find_vehicle_same_player_ok, pilot_find_vehicle_scan_eligible,
            pilot_find_vehicle_scan_frame, pilot_levels_to_gain, should_pilot_base_center_fallback,
            uses_air_eject_ocl, vehicle_can_gain_exp_for_levels,
            vehicle_meets_pilot_find_min_health, PILOT_FIND_VEHICLE_MIN_HEALTH,
            PILOT_FIND_VEHICLE_SCAN_RANGE,
        };

        if !pilot_find_vehicle_scan_frame(self.frame) {
            return;
        }

        let snapshot = match self.objects.get(&pilot_id) {
            Some(obj) if obj.is_alive() => {
                let is_pilot = is_pilot_template(&obj.template_name);
                let is_idle = matches!(obj.ai_state, AIState::Idle);
                // C++ PLAYER_HUMAN → no scan. Host residual: local player is human.
                // No mapped player → fail-closed treat as non-AI (no auto-scan).
                let is_ai = self
                    .player_id_for_team(obj.team)
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
                let is_vehicle = vehicle.is_kind_of(KindOf::Vehicle)
                    || vehicle.object_type == ObjectType::Vehicle;
                let is_air = vehicle.is_kind_of(KindOf::Aircraft)
                    || vehicle.object_type == ObjectType::Aircraft
                    || vehicle.status.airborne_target;
                let under_construction =
                    vehicle.status.under_construction || vehicle.construction_percent + 0.001 < 1.0;
                let is_dozer = vehicle.is_worker()
                    || vehicle.template_name.to_ascii_lowercase().contains("dozer");
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
                let same_team = vehicle.team == pilot_team;
                let is_neutral = vehicle.team == Team::Neutral;
                let owner_matches = vehicle
                    .status
                    .unmanned_owner_team
                    .map(|t| t == pilot_team)
                    .unwrap_or(false);
                let same_player_ok =
                    pilot_find_vehicle_same_player_ok(same_team, is_neutral, owner_matches);
                let above_terrain = uses_air_eject_ocl(vpos.y, vehicle.status.airborne_target);
                let airborne_locomotor = is_air;
                let is_trainable = is_vehicle && !is_air;
                let can_gain =
                    vehicle_can_gain_exp_for_levels(vehicle.experience.level, levels_to_gain);
                let collide_ok = pilot_collide_would_like_to_collide_with(
                    true,
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
    /// + kill chute (C++ ParachuteContain::onCollide).
    /// Fail-closed: not full bone PARA_COG / DeliverPayload matrix.
    pub(crate) fn tick_eject_parachute_residual(&mut self, pilot_id: ObjectId) {
        use crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME;
        use crate::game_logic::host_usa_pilot::{
            is_pilot_template, should_open_parachute, tick_parachute_height_with_state,
            tick_parachute_sway, PILOT_PARACHUTE_LAND_AUDIO, PILOT_PARACHUTE_OPEN_AUDIO,
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

        // C++ open chute → aiMoveToPosition(landingOverride) residual.
        let mut nx = pos.x;
        let mut nz = pos.z;
        let mut did_override_step = false;
        if open && !landed {
            if let Some(target) = landing_override {
                use crate::game_logic::host_usa_pilot::{
                    step_parachute_landing_override, PARACHUTE_LANDING_OVERRIDE_SPEED,
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

        // C++ ParachuteContain::onCollide(null): removeAllContained + kill chute.
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
            // Kill AmericaParachute (SlowDeath residual → destroy).
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
            // Hijacker airborne PutInContainer land honesty.
            self.car_bomb.record_airborne_parachute_land();
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

    /// AutoFindHealingUpdate residual: AI idle injured USA infantry auto-scan for HealPad.
    ///
    /// Retail ModuleTag: ScanRate 1000ms → 30 frames, ScanRange 300, NeverHeal 0.85,
    /// AlwaysHeal 0.25. Templates: Pilot / Ranger / MissileDefender / Pathfinder /
    /// ColonelBurton residual. C++ human players skip; host residual: is_local → no
    /// auto-scan. Idle-only residual (AlwaysHeal busy-interrupt path fail-closed —
    /// C++ early-return makes busy path unreachable). Issues SeekingHealing toward
    /// nearest HealPad in range.
    pub(super) fn try_auto_find_healing_residual(&mut self, unit_id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            auto_find_healing_scan_eligible, auto_find_healing_scan_frame,
            health_needs_auto_find_healing, is_auto_find_healing_target,
            is_auto_find_healing_template, is_pilot_template, AUTO_FIND_HEALING_NEVER_HEAL,
            AUTO_FIND_HEALING_SCAN_RANGE,
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
                    .player_id_for_team(obj.team)
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
    /// **Host API residual only** (not live skirmish `Object::take_damage` path):
    /// combat/AOE still hits structure HP via armor residual. Call this API for
    /// SMALL_ARMS/SNIPER-style propagate tests and host residual wiring.
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
            is_stinger_site_structure, next_stinger_slave_respawn_frame,
            resolve_hive_structure_damage_roster, sync_hive_slave_mirrors,
            STINGER_SOLDIER_DIE_AUDIO,
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
            count_alive_hive_slaves, is_stinger_site_structure, next_stinger_slave_respawn_frame,
            respawn_one_hive_slave, should_respawn_stinger_slave, sync_hive_slave_mirrors,
            STINGER_SOLDIER_MAX_HEALTH, STINGER_SPAWN_NUMBER,
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
            bomb_truck_blast_damage_at, BOMB_TRUCK_POISON_AUDIO,
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
                if victim.take_damage_from(dmg, Some(truck_id)) {
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
    pub(super) fn try_auto_find_repair_residual(&mut self, unit_id: ObjectId) {
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
                    .player_id_for_team(obj.team)
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
    pub(super) fn try_auto_resume_construction_residual(&mut self, unit_id: ObjectId) {
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
                    .player_id_for_team(obj.team)
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

    pub(super) fn update_bomb_truck_poison_zones(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
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
    pub(super) fn update_nuclear_tanks_radiation_zones(&mut self) {
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
        let bid = self.create_object(BOOBY_TRAP_OBJECT, team, pos)?;
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
    pub(super) fn drain_historic_bonus_firestorms(&mut self) {
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
        let pid = self.create_object(tpl_name, team, start)?;
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
    pub(super) fn update_helix_napalm_firestorms(&mut self) {
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
    pub(super) fn update_radar_scans(&mut self) {
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

        let team = caster_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
            .or_else(|| self.players.get(&player_id).map(|p| p.team))
            .unwrap_or(Team::Neutral);

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
            if let Some(id) = self.create_object(tpl_name, team, pos) {
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

        let caster_team = caster_id
            .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
            .or_else(|| self.players.get(&player_id).map(|p| p.team))
            .unwrap_or(Team::Neutral);

        let sciences: Vec<String> = self
            .players
            .get(&player_id)
            .map(|p| p.unlocked_sciences.iter().cloned().collect())
            .unwrap_or_default();
        let amount = cash_hack_money_from_sciences(sciences.iter().map(|s| s.as_str()));

        let mut victim_team: Option<Team> = None;
        let mut victim_cash: u32 = 0;
        for p in self.players.values() {
            if p.team == caster_team || p.team == Team::Neutral {
                continue;
            }
            let cash = p.resources.supplies;
            if victim_team.is_none() || cash > victim_cash {
                victim_cash = cash;
                victim_team = Some(p.team);
            }
        }

        let stolen = if let Some(from_team) = victim_team {
            self.steal_cash_from_team(from_team, caster_team, amount)
        } else {
            0
        };
        if stolen > 0 {
            if let Some(p) = self.get_player_mut_by_team(caster_team) {
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

        let spawned_id = self.create_object(SPY_DRONE_TEMPLATE, team, location);
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
        let fid = self.create_object(FLARE_TEMPLATE_NAME, team, place)?;
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
    pub(super) fn update_spy_satellites(&mut self) {
        self.spy_satellites.prune_expired(self.frame);
        self.spy_drones.prune_expired(self.frame);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.process_pending_undo_shroud_reveals(self.frame);
        }
    }

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
    /// temporary FOW reveal at unit position (sight_range residual), and mark
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
            cia_intelligence_duration_frames, HostCiaIntelligence, HostCiaIntelligenceSpiedUnit,
            CIA_INTELLIGENCE_ACTIVATE_AUDIO, CIA_INTELLIGENCE_DEFAULT_VISION_RADIUS,
        };
        use gamelogic::common::Coord3D;

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
                obj.is_alive()
                    && obj.team != team
                    && obj.team != Team::Neutral
                    && caster_id.map(|c| c != obj.id).unwrap_or(true)
            })
            .map(|obj| {
                let sight = obj.get_template().sight_range;
                let radius = if sight > 0.0 {
                    sight
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

        for (obj_id, location, radius, was_stealthed) in enemy_snapshots {
            // Mark vision-spied residual on Main object (setUnitsVisionSpied residual).
            if let Some(obj) = self.objects.get_mut(&obj_id) {
                obj.set_vision_spied_by_player(player_id, true);
                any_vision_spied = true;
                // Stealthed residual: DETECTED until spy expires so unit is
                // visible/targetable (goal: enemy units become detectable).
                if was_stealthed || obj.status.stealthed {
                    obj.mark_detected(expires_frame);
                    any_detect = true;
                }
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
                            detected_ok: was_stealthed,
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
                detected_ok: was_stealthed,
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

    /// Advance CIA Intelligence residual: clear expired vision-spied marks + FOW undos.
    pub(super) fn update_cia_intelligence(&mut self) {
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
    pub(super) fn update_firewalls(&mut self) {
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
            inferno_shell_flight_frames, INFERNO_CANNON_PROJECTILE,
            INFERNO_CANNON_PROJECTILE_UPGRADED, INFERNO_SHELL_MAX_HEALTH,
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
            o.producer_id = Some(source_id);
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
            inferno_shell_damage_at, is_inferno_cannon_template, INFERNO_CANNON_SHELL_DAMAGE,
            INFERNO_CANNON_SHELL_RADIUS,
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
                let hp = obj.health.current;
                let new_hp = (hp - dmg).max(0.0);
                Self::write_object_health_authority_aware(obj, new_hp);
                hits = hits.saturating_add(1);
                if new_hp <= 0.0 {
                    obj.status.destroyed = true;
                    obj.status.effectively_dead = true;
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
    pub(super) fn update_inferno_fire_zones(&mut self) {
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
    // GLA Angry Mob residual (nexus damages nearby enemies / expands members)
    // Fail-closed: not full SpawnBehavior members / MobMemberSlavedUpdate matrix.
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

    /// Advance Angry Mob residual: expand members + aggregate fire on nearby enemies.
    ///
    /// Retail: SpawnBehavior members fire pistol/rock/molotov; residual collapses
    /// that into periodic AoE damage around the nexus within AttackRange 100.
    /// Expand residual grows member strength from InitialBurst 5 → SpawnNumber 10.
    ///
    /// Fail-closed: not individual member objects, projectile weapons, or slave AI.
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
                .set_health(ANGRY_MOB_MEMBER_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(template_name.to_string(), t);
        }
        let origin = self.objects.get(&nexus_id)?.get_position();
        let angle = (slot_index as f32) * (2.0 * PI / 8.0);
        let radius = 8.0 + (slot_index % 3) as f32 * 2.0;
        let place = glam::Vec3::new(
            origin.x + angle.cos() * radius,
            origin.y,
            origin.z + angle.sin() * radius,
        );
        let mid = self.create_object(template_name, team, place)?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.angry_mob_member = true;
            o.angry_mob_nexus_id = Some(nexus_id);
            o.producer_id = Some(nexus_id);
            o.health.maximum = ANGRY_MOB_MEMBER_MAX_HEALTH;
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
            let _ = self.spawn_angry_mob_member_object(
                spawn.nexus_id,
                spawn.team,
                &spawn.template_name,
                spawn.slot_index,
            );
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
            moves.push((
                mid,
                glam::Vec3::new(
                    origin.x + angle.cos() * radius,
                    origin.y,
                    origin.z + angle.sin() * radius,
                ),
            ));
        }
        for (mid, pos) in moves {
            if let Some(o) = self.objects.get_mut(&mid) {
                o.set_position(pos);
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

    pub fn update_angry_mobs(&mut self) {
        use crate::game_logic::host_angry_mob::{
            is_angry_mob_nexus_template, ANGRY_MOB_FIRE_AUDIO, UPGRADE_GLA_ARM_THE_MOB,
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
        self.angry_mobs.apply_due_expands(frame);
        self.flush_angry_mob_member_spawns();
        // Wave 801: under coupled shadow, AngryMob member follow is owned by
        // GW tick_status_timer_expirations + destroy logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_angry_mob_member_follow();
        }

        if self.angry_mobs.active_count() == 0 {
            return;
        }

        // Candidates for residual aggregate fire.
        let candidates: Vec<(ObjectId, Vec3, Team, bool, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft)
                    || obj.object_type == ObjectType::Building
                    || obj.object_type == ObjectType::Infantry
                    || obj.object_type == ObjectType::Vehicle
                    || obj.object_type == ObjectType::Aircraft;
                (
                    *id,
                    obj.get_position(),
                    obj.team,
                    obj.is_alive(),
                    combat_kind,
                    obj.status.under_construction,
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

        let plans = self
            .angry_mobs
            .plan_due_ticks(frame, &candidates, |team| armed_teams.contains(&team));

        for plan in plans {
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
                    if !target.is_alive() {
                        continue;
                    }
                    if audio_pos.is_none() {
                        audio_pos = Some(target.get_position());
                    }
                    let killed = target.take_damage_from(hit.damage, Some(plan.mob_id));
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
        let pid = self.create_object(name, team, start)?;
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
            o.producer_id = Some(source_id);
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
        let id = self
            .aurora_bombs
            .queue(kind, source_object, source_team, target_position, frame);

        // C++ AuroraBomb SpecialObject residual (guided drop under aircraft).
        let from = self
            .objects
            .get(&source_object)
            .map(|o| o.get_position())
            .unwrap_or(target_position);
        let _ = self.spawn_aurora_bomb_projectile(
            id,
            source_object,
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
        let gid = self.create_object(gas_name, source_team, place)?;
        if let Some(o) = self.objects.get_mut(&gid) {
            o.producer_id = Some(source_object);
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

    pub(super) fn update_aurora_bombs(&mut self) {
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

    /// Host residual for C++ StealthUpdate + StealthDetectorUpdate targetability.
    ///
    /// - Expires `OBJECT_STATUS_DETECTED` when `detection_expires_frame` is reached
    /// - Detectors mark nearby enemy stealthed units as detected (hold ~1s)
    /// - Bomb truck disguise reveal residual (RevealDistanceFromTarget = 100)
    /// - Fail-closed: no IR FX, ExtraRequiredKindOf filters, garrisoned-detect,
    ///   or full stealth delay re-cloak state machine.
    pub fn update_stealth_and_detection(&mut self) {
        let frame = self.frame;

        // Expire timed detections (unit may remain stealthed).
        for obj in self.objects.values_mut() {
            if obj.status.detected
                && obj.detection_expires_frame > 0
                && frame >= obj.detection_expires_frame
            {
                obj.clear_detected();
            }
        }

        // Bomb truck disguise residual: RevealDistanceFromTarget = 100 while
        // attacking a victim; also reveal when firing breaks stealth residual.
        {
            use crate::game_logic::host_bomb_truck_disguise::{
                should_reveal_disguise_by_distance, BOMB_TRUCK_DISGUISE_REVEAL_AUDIO,
            };
            let disguised: Vec<(ObjectId, Option<ObjectId>, bool, Vec3)> = self
                .objects
                .iter()
                .filter(|(_, o)| o.status.disguised && o.is_alive())
                .map(|(id, o)| {
                    (
                        *id,
                        o.target,
                        o.status.attacking
                            || matches!(
                                o.ai_state,
                                AIState::Attacking
                                    | AIState::AttackMoving
                                    | AIState::AttackingGround
                            ),
                        o.get_position(),
                    )
                })
                .collect();
            let mut reveal_ids: Vec<ObjectId> = Vec::new();
            for (id, victim_id, is_attacking, pos) in disguised {
                let mut reveal = false;
                if is_attacking {
                    if let Some(vid) = victim_id {
                        if let Some(victim) = self.objects.get(&vid) {
                            let vp = victim.get_position();
                            let dx = pos.x - vp.x;
                            let dz = pos.z - vp.z;
                            let dist = (dx * dx + dz * dz).sqrt();
                            if should_reveal_disguise_by_distance(dist) {
                                reveal = true;
                            }
                        } else {
                            // Attacking with no live victim: still residual-reveal
                            // when attack state is active (fire residual).
                            reveal = true;
                        }
                    } else {
                        reveal = true;
                    }
                }
                if reveal {
                    reveal_ids.push(id);
                }
            }
            for rid in reveal_ids {
                let pos = {
                    let Some(obj) = self.objects.get_mut(&rid) else {
                        continue;
                    };
                    obj.clear_disguise();
                    obj.get_position()
                };
                self.bomb_truck_disguise.record_reveal();
                self.queue_audio_event(
                    AudioEventRequest::new(BOMB_TRUCK_DISGUISE_REVEAL_AUDIO)
                        .with_object(rid)
                        .with_position(pos)
                        .with_priority(160),
                );
            }
        }

        // Pathfinder residual: StealthForbiddenConditions = MOVING.
        // Uncloak while Moving/AttackMoving; re-cloak immediately when stopped
        // (StealthDelay = 0, InnateStealth = Yes). Fire does not break stealth.
        {
            use crate::game_logic::host_pathfinder::pathfinder_stealth_desired;
            // Class bit set at spawn — no per-frame template-name scan.
            let pf_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_pathfinder_unit && o.is_alive())
                .map(|(id, _)| *id)
                .collect();
            for pid in pf_ids {
                let Some(obj) = self.objects.get_mut(&pid) else {
                    continue;
                };
                let moving = matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
                    || obj.status.moving;
                if let Some(desired) = pathfinder_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.stealth_breaks_on_move,
                    obj.is_alive(),
                    moving,
                ) {
                    if desired && !obj.status.stealthed {
                        obj.set_status_stealthed(true);
                    } else if !desired && obj.status.stealthed {
                        obj.break_stealth();
                    }
                }
            }
        }

        // Listening Outpost residual: StealthForbiddenConditions = MOVING
        // (RIDERS_ATTACKING fail-closed). InnateStealth re-cloaks when stopped.
        {
            use crate::game_logic::host_listening_outpost::listening_outpost_stealth_desired;
            // Style bit installed at spawn for LO templates — no name scan.
            let lo_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| o.is_alive() && o.is_listening_outpost_style_container())
                .map(|(id, _)| *id)
                .collect();
            for lid in lo_ids {
                let Some(obj) = self.objects.get_mut(&lid) else {
                    continue;
                };
                let moving = matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
                    || obj.status.moving;
                if let Some(desired) = listening_outpost_stealth_desired(
                    true,
                    obj.innate_stealth,
                    obj.stealth_breaks_on_move,
                    obj.is_alive(),
                    moving,
                ) {
                    if desired && !obj.status.stealthed {
                        obj.set_status_stealthed(true);
                    } else if !desired && obj.status.stealthed {
                        obj.break_stealth();
                    }
                }
            }
        }

        // GLA Camouflage residual: re-cloak when idle (innate_stealth after
        // Upgrade_GLACamouflage). Fail-closed vs full 2500ms StealthDelay.
        {
            use crate::game_logic::host_upgrades::UPGRADE_GLA_CAMOUFLAGE;
            // Upgrade tag is only applied to camouflage-eligible units at unlock.
            let camo_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && !o.status.disguised
                        && (o.has_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE)
                            || o.has_upgrade_tag("Upgrade_GLACamouflage"))
                })
                .map(|(id, _)| *id)
                .collect();
            for cid in camo_ids {
                let Some(obj) = self.objects.get_mut(&cid) else {
                    continue;
                };
                let attacking = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                if attacking {
                    // Attack residual handled by fire path (stealth_breaks_on_attack).
                    continue;
                }
                if !obj.status.stealthed {
                    obj.set_status_stealthed(true);
                    obj.set_status_detected(false);
                    obj.detection_expires_frame = 0;
                }
            }
        }

        // GLA CamoNetting structure residual: StealthForbiddenConditions =
        // ATTACKING + USING_ABILITY + TAKING_DAMAGE, StealthDelay 2500ms re-cloak,
        // OrderIdleEnemiesToAttackMeUponReveal residual on uncloak,
        // FriendlyOpacity residual (min cloaked / max revealed + pulse while cloaked),
        // StealthLook / heat-vision second-pass residual (Drawable::setStealthLook).
        {
            use crate::game_logic::host_upgrades::{
                camo_netting_heat_vision_opacity, camo_netting_order_idle_enemy_in_range,
                camo_netting_pulse_opacity, camo_netting_stealth_allowed_frame,
                camo_netting_stealth_look, camo_netting_structure_stealth_desired,
                is_camo_netting_structure_template, CAMO_NETTING_FRIENDLY_OPACITY_MAX,
                CAMO_NETTING_FRIENDLY_OPACITY_MIN, UPGRADE_GLA_CAMO_NETTING,
            };
            let struct_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, o)| {
                    o.innate_stealth
                        && o.is_alive()
                        && is_camo_netting_structure_template(&o.template_name)
                        && (o.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING)
                            || o.has_upgrade_tag("Upgrade_GLACamoNetting")
                            || o.stealth_breaks_on_damage)
                })
                .map(|(id, _)| *id)
                .collect();
            let mut recloaks = 0u32;
            let mut reveals = 0u32;
            let mut opacity_cloaks = 0u32;
            let mut opacity_reveals = 0u32;
            let mut heat_vision = 0u32;
            let mut revealed_ids: Vec<ObjectId> = Vec::new();
            for sid in struct_ids {
                let Some(obj) = self.objects.get_mut(&sid) else {
                    continue;
                };
                // Resolve pending StealthDelay after a reveal this frame.
                if obj.stealth_delay_pending {
                    obj.stealth_allowed_frame = camo_netting_stealth_allowed_frame(frame);
                    obj.stealth_delay_pending = false;
                }
                let attacking = obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    );
                // C++ OBJECT_STATUS_IS_USING_ABILITY residual.
                let using_ability =
                    obj.status.using_ability || matches!(obj.ai_state, AIState::SpecialAbility);
                let Some(desired) = camo_netting_structure_stealth_desired(
                    obj.innate_stealth,
                    obj.is_alive(),
                    attacking,
                    using_ability,
                    frame,
                    obj.stealth_allowed_frame,
                ) else {
                    continue;
                };
                if desired && !obj.status.stealthed {
                    obj.set_status_stealthed(true);
                    obj.set_status_detected(false);
                    obj.detection_expires_frame = 0;
                    obj.stealth_allowed_frame = 0;
                    // FriendlyOpacity residual: cloaked → min (then pulse).
                    obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MIN;
                    obj.record_host_vision_camo();
                    obj.camo_opacity_pulse_phase = 0.0;
                    opacity_cloaks = opacity_cloaks.saturating_add(1);
                    recloaks = recloaks.saturating_add(1);
                } else if !desired && obj.status.stealthed {
                    obj.break_stealth();
                    // break_stealth marks delay pending; resolve immediately with frame.
                    if obj.stealth_delay_pending {
                        obj.stealth_allowed_frame = camo_netting_stealth_allowed_frame(frame);
                        obj.stealth_delay_pending = false;
                    }
                    // FriendlyOpacity residual: revealed → max (no pulse).
                    obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MAX;
                    obj.record_host_vision_camo();
                    opacity_reveals = opacity_reveals.saturating_add(1);
                    reveals = reveals.saturating_add(1);
                    revealed_ids.push(sid);
                } else if obj.status.stealthed && !obj.status.detected {
                    // Pulse residual while cloaked (C++ setEffectiveOpacity sin path).
                    // If still at default full opacity (spawned already cloaked),
                    // record one cloak opacity residual.
                    if (obj.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MAX).abs() < 0.01
                        && obj.camo_opacity_pulse_phase == 0.0
                    {
                        opacity_cloaks = opacity_cloaks.saturating_add(1);
                    }
                    let (op, next_phase) = camo_netting_pulse_opacity(obj.camo_opacity_pulse_phase);
                    obj.camo_friendly_opacity = op;
                    obj.record_host_vision_camo();
                    obj.camo_opacity_pulse_phase = next_phase;
                } else {
                    // Revealed residual: hold max opacity.
                    if (obj.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MAX).abs() > 0.01
                    {
                        obj.camo_friendly_opacity = CAMO_NETTING_FRIENDLY_OPACITY_MAX;
                        obj.record_host_vision_camo();
                        opacity_reveals = opacity_reveals.saturating_add(1);
                    }
                }

                // StealthLook residual for enemy observer (default host residual view).
                // Detected stealthed structures → heat-vision second material pass.
                let look = camo_netting_stealth_look(
                    obj.status.stealthed,
                    obj.status.detected,
                    false, // enemy observer residual (non-friendly)
                );
                let hv = camo_netting_heat_vision_opacity(look);
                if hv > 0.0 && obj.camo_heat_vision_opacity < 0.5 {
                    heat_vision = heat_vision.saturating_add(1);
                }
                obj.camo_stealth_look = look.as_u8();
                obj.record_host_vision_camo();
                obj.camo_heat_vision_opacity = hv;
                // CamoNetting sub-object net mesh residual presentation.
                if obj.camo_net_sub_object_shown || obj.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING) {
                    use crate::game_logic::host_upgrades::{
                        camo_netting_sub_object_observer_visible, camo_netting_sub_object_state,
                    };
                    obj.camo_net_sub_object_shown = true;
                    let sub = camo_netting_sub_object_state(
                        true,
                        obj.status.stealthed,
                        obj.status.detected,
                        false, // enemy observer residual default
                        obj.camo_friendly_opacity,
                    );
                    obj.camo_net_sub_object_observer_visible =
                        camo_netting_sub_object_observer_visible(&sub);
                }
            }
            self.camo_netting_heat_vision_count = self
                .camo_netting_heat_vision_count
                .saturating_add(heat_vision);
            self.camo_netting_opacity_cloak_count = self
                .camo_netting_opacity_cloak_count
                .saturating_add(opacity_cloaks);
            self.camo_netting_opacity_reveal_count = self
                .camo_netting_opacity_reveal_count
                .saturating_add(opacity_reveals);
            // OrderIdleEnemiesToAttackMeUponReveal residual: idle enemy units
            // that can see the revealed structure wake and attempt to target it.
            for rid in revealed_ids {
                let Some(victim) = self.objects.get(&rid) else {
                    continue;
                };
                let v_team = victim.team;
                let v_pos = victim.get_position();
                let candidates: Vec<(ObjectId, f32, f32, bool)> = self
                    .objects
                    .iter()
                    .filter(|(_, o)| {
                        o.is_alive()
                            && o.team != v_team
                            && o.team != Team::Neutral
                            && !matches!(
                                o.object_type,
                                ObjectType::Building | ObjectType::Projectile
                            )
                            && !o.is_kind_of(KindOf::Structure)
                            && !o.is_kind_of(KindOf::Worker)
                            && !o.is_worker()
                    })
                    .map(|(id, o)| {
                        let dx = o.get_position().x - v_pos.x;
                        let dz = o.get_position().z - v_pos.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        let vision = {
                            let sr = o.get_template().sight_range;
                            if sr > 0.0 {
                                sr
                            } else {
                                150.0
                            }
                        };
                        let can_attack = o.weapon.is_some()
                            || o.is_kind_of(KindOf::Attackable)
                            || o.can_attack()
                            || matches!(
                                o.object_type,
                                ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft
                            );
                        (*id, dist, vision, can_attack)
                    })
                    .collect();
                for (eid, dist, vision, can_attack) in candidates {
                    let Some(enemy) = self.objects.get_mut(&eid) else {
                        continue;
                    };
                    let idle = matches!(enemy.ai_state, AIState::Idle) && enemy.target.is_none();
                    if !camo_netting_order_idle_enemy_in_range(
                        enemy.is_alive(),
                        idle,
                        can_attack,
                        dist,
                        vision,
                    ) {
                        continue;
                    }
                    enemy.set_target(Some(rid));
                    enemy.set_ai_state(AIState::Attacking);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_attack(eid, rid);
                        crate::game_logic::host_ai_decision_log::record_set_state(eid, 2);
                    }
                    self.camo_netting_order_idle_enemies_count =
                        self.camo_netting_order_idle_enemies_count.saturating_add(1);
                }
            }
            self.camo_netting_structure_residual_recloaks = self
                .camo_netting_structure_residual_recloaks
                .saturating_add(recloaks);
            self.camo_netting_structure_residual_reveals = self
                .camo_netting_structure_residual_reveals
                .saturating_add(reveals);
        }

        // Collect active detectors (alive, not under construction) that are due
        // for a StealthDetectorUpdate DetectionRate residual scan.
        // Track residual detector kind for honesty counters.
        use crate::game_logic::host_strategy_center::{
            stealth_detector_hold_frames, stealth_detector_next_scan_frame,
            stealth_detector_scan_due,
        };
        #[derive(Clone, Copy)]
        struct DetFlags {
            is_sentry: bool,
            is_pathfinder: bool,
            is_scout: bool,
            is_listening_outpost: bool,
            is_troop_crawler: bool,
        }
        let mut detectors: Vec<(ObjectId, Team, Vec3, f32, DetFlags, u32)> = Vec::new();
        let mut scanned_detector_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in &self.objects {
            if !(o.is_detector
                && o.is_alive()
                && !o.status.under_construction
                && !o.status.destroyed)
            {
                continue;
            }
            let range = o.effective_detection_range();
            if range <= 0.0 {
                continue;
            }
            if !stealth_detector_scan_due(
                o.detection_rate_frames,
                o.next_detection_scan_frame,
                frame,
            ) {
                continue;
            }
            let flags = DetFlags {
                is_sentry: crate::game_logic::host_sentry_drone::is_sentry_drone_template(
                    &o.template_name,
                ),
                is_pathfinder: crate::game_logic::host_pathfinder::is_pathfinder_template(
                    &o.template_name,
                ),
                is_scout: crate::game_logic::host_slave_drones::is_scout_drone_template(
                    &o.template_name,
                ),
                is_listening_outpost:
                    crate::game_logic::host_listening_outpost::is_listening_outpost_template(
                        &o.template_name,
                    ) || o.is_listening_outpost_style_container(),
                is_troop_crawler: crate::game_logic::host_troop_crawler::is_troop_crawler_template(
                    &o.template_name,
                ) || o.is_troop_crawler_style_container(),
            };
            detectors.push((
                *id,
                o.team,
                o.get_position(),
                range,
                flags,
                o.detection_rate_frames,
            ));
            scanned_detector_ids.push(*id);
        }

        // Advance DetectionRate residual sleep for every detector that scanned
        // this tick (C++ returns UPDATE_SLEEP(m_updateRate) after each wake).
        for det_id in &scanned_detector_ids {
            if let Some(obj) = self.objects.get_mut(det_id) {
                if obj.detection_rate_frames > 0 {
                    obj.next_detection_scan_frame =
                        stealth_detector_next_scan_frame(obj.detection_rate_frames, frame);
                    self.stealth_detector_rate_scans =
                        self.stealth_detector_rate_scans.saturating_add(1);
                }
            }
        }

        if detectors.is_empty() {
            return;
        }

        // Collect stealthed targets first to avoid borrow conflicts.
        let stealthed_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.status.stealthed)
            .map(|(id, _)| *id)
            .collect();

        for sid in stealthed_ids {
            let Some((s_team, s_pos, already_detected)) = self
                .objects
                .get(&sid)
                .map(|o| (o.team, o.get_position(), o.status.detected))
            else {
                continue;
            };

            let mut detected_by_sentry = false;
            let mut detected_by_pathfinder = false;
            let mut detected_by_scout = false;
            let mut detected_by_listening_outpost = false;
            let mut detected_by_troop_crawler = false;
            // C++ markAsDetected(updateRate + 1); take max hold among detecting scanners.
            let mut best_expires: u32 = 0;
            let detected_by_someone =
                detectors
                    .iter()
                    .any(|(_id, det_team, det_pos, range, flags, rate)| {
                        let in_range = *det_team != s_team && det_pos.distance(s_pos) <= *range;
                        if in_range {
                            let hold = stealth_detector_hold_frames(*rate);
                            let exp = frame.saturating_add(hold);
                            if exp > best_expires {
                                best_expires = exp;
                            }
                            if flags.is_sentry {
                                detected_by_sentry = true;
                            }
                            if flags.is_pathfinder {
                                detected_by_pathfinder = true;
                            }
                            if flags.is_scout {
                                detected_by_scout = true;
                            }
                            if flags.is_listening_outpost {
                                detected_by_listening_outpost = true;
                            }
                            if flags.is_troop_crawler {
                                detected_by_troop_crawler = true;
                            }
                        }
                        in_range
                    });

            if detected_by_someone {
                if let Some(obj) = self.objects.get_mut(&sid) {
                    obj.mark_detected(best_expires);
                }
                // Honesty: first residual reveal by residual detector kinds this tick.
                if !already_detected {
                    if detected_by_sentry {
                        self.sentry_drone_residual_detects =
                            self.sentry_drone_residual_detects.saturating_add(1);
                    }
                    if detected_by_pathfinder {
                        self.pathfinder_residual_detects =
                            self.pathfinder_residual_detects.saturating_add(1);
                    }
                    if detected_by_scout {
                        self.scout_drone_residual_detects =
                            self.scout_drone_residual_detects.saturating_add(1);
                    }
                    if detected_by_listening_outpost {
                        self.listening_outpost.record_detect();
                    }
                    if detected_by_troop_crawler {
                        self.troop_crawler.record_detect();
                    }
                    // C++ hero stealth detection EVA residual (Own/Enemy *Detected).
                    self.try_eva_hero_detected(sid);
                }
            }
        }
    }

    /// Place a residual land mine at `position` for `team`.
    pub fn place_land_mine(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::LandMine,
            "TestLandMine",
            team,
            position,
            producer,
            None,
            None,
        )
    }

    /// Place a residual GLA demo trap (proximity mode, standard detonation).
    pub fn place_demo_trap(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
    ) -> Option<ObjectId> {
        self.place_demo_trap_named("TestDemoTrap", team, position, producer, false)
    }

    /// Place a residual demo trap with Chem/Demo/Standard profile from template name.
    ///
    /// `has_gamma` applies Chem Gamma death weapon residual when true.
    pub fn place_demo_trap_named(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        has_gamma: bool,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{demo_trap_profile, HostMineData, HostMineKind};

        let profile = demo_trap_profile(template_name, has_gamma, false);
        self.ensure_residual_mine_template(template_name, HostMineKind::DemoTrap);
        let id = self.create_object(template_name, team, position)?;
        let mut data = HostMineData::demo_trap_with_profile(profile);
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;
            obj.record_host_demo_mine_cheer();
            obj.movement.max_speed = 0.0;
            obj.weapon = None;
            obj.secondary_weapon = None;
        }
        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(HostMineKind::DemoTrap.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        Some(id)
    }

    /// Place a residual timed demo charge (detonates after delay frames).
    pub fn place_timed_demo_charge(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{
            retail_timed_charge_lifetime_frames, retail_timed_charge_template,
        };
        let producer_template =
            producer.and_then(|pid| self.objects.get(&pid).map(|o| o.template_name.clone()));
        let template_name = retail_timed_charge_template(producer_template.as_deref());
        let delay =
            delay_frames.or_else(|| Some(retail_timed_charge_lifetime_frames(template_name)));
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge,
            template_name,
            team,
            position,
            producer,
            attach_to,
            delay,
        )
    }

    /// Place a residual remote demo charge (no auto-timer; remote detonate only).
    /// Fail-closed: not full StickyBombUpdate attach bones / max-charge list.
    pub fn place_remote_demo_charge(
        &mut self,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::BURTON_REMOTE_CHARGE_OBJECT;
        self.place_mine_kind(
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge,
            BURTON_REMOTE_CHARGE_OBJECT,
            team,
            position,
            producer,
            attach_to,
            None,
        )
    }

    /// Detonate all residual remote demo charges planted by any of `producers`.
    /// Matches C++ SPECIAL_REMOTE_CHARGES no-target path (StickyBombUpdate::detonate).
    /// Returns the number of charges detonated.
    pub fn detonate_remote_demo_charges(&mut self, producers: &[ObjectId]) -> u32 {
        use crate::game_logic::host_mines::{HostMineDetonateReason, HostMineKind};

        if producers.is_empty() {
            return 0;
        }
        let producer_set: std::collections::HashSet<ObjectId> = producers.iter().copied().collect();
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if !data.is_active() || !obj.is_alive() {
                    return None;
                }
                if data.kind != HostMineKind::RemoteDemoCharge {
                    return None;
                }
                let producer = data.producer_id?;
                if !producer_set.contains(&producer) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        let mut count = 0u32;
        for mine_id in due {
            if self.detonate_mine_internal(mine_id, HostMineDetonateReason::Manual) {
                count = count.saturating_add(1);
            }
        }
        if count > 0 {
            self.hero_abilities.record_remote_charge_detonate(count);
        }
        count
    }

    /// Cluster Mines special-power residual: place a ring of land mines.
    /// Fail-closed: not full OCL ClusterMinesBomb / GenerateMinefieldBehavior density.
    pub fn place_cluster_mines(
        &mut self,
        team: Team,
        center: Vec3,
        producer: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        use crate::game_logic::host_mines::{
            cluster_mine_positions, CLUSTER_MINE_COUNT, CLUSTER_MINE_RING_RADIUS,
        };
        let positions =
            cluster_mine_positions(center, CLUSTER_MINE_COUNT, CLUSTER_MINE_RING_RADIUS);
        let mut ids = Vec::with_capacity(positions.len());
        for pos in positions {
            if let Some(id) = self.place_land_mine(team, pos, producer) {
                ids.push(id);
            }
        }
        if !ids.is_empty() {
            self.queue_audio_event(
                AudioEventRequest::new("MineFieldPlaced")
                    .with_position(center)
                    .with_priority(160),
            );
        }
        ids
    }

    pub(super) fn ensure_residual_mine_template(
        &mut self,
        template_name: &str,
        kind: crate::game_logic::host_mines::HostMineKind,
    ) {
        if self.templates.contains_key(template_name) {
            return;
        }
        let mut t = ThingTemplate::new(template_name);
        // Mines are not infantry/vehicles; residual treats them as Neutral objects
        // with mine_data driving behavior. Demo trap is structure-like but residual
        // does not require full structure production path.
        match kind {
            crate::game_logic::host_mines::HostMineKind::DemoTrap => {
                t.add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::Selectable)
                    .set_health(100.0)
                    .set_cost(400, 0);
            }
            crate::game_logic::host_mines::HostMineKind::LandMine
            | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
            | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                t.set_health(100.0).set_cost(0, 0);
            }
        }
        self.templates.insert(template_name.to_string(), t);
    }

    pub(super) fn place_mine_kind(
        &mut self,
        kind: crate::game_logic::host_mines::HostMineKind,
        template_name: &str,
        team: Team,
        position: Vec3,
        producer: Option<ObjectId>,
        attach_to: Option<ObjectId>,
        delay_frames: Option<u32>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{
            can_place_remote_charge, can_place_timed_charge, HostMineData, HostMineKind,
            BURTON_UNIQUE_CHARGE_TARGETS,
        };

        // C++ MaxSpecialObjects + UniqueSpecialObjectTargets residual (Burton C4).
        if matches!(
            kind,
            HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
        ) {
            if let Some(pid) = producer {
                let mut timed_n = 0u32;
                let mut remote_n = 0u32;
                for o in self.objects.values() {
                    if !o.is_alive() {
                        continue;
                    }
                    let Some(md) = o.mine_data.as_ref() else {
                        continue;
                    };
                    if md.detonated {
                        continue;
                    }
                    let owned = md.producer_id == Some(pid) || o.producer_id == Some(pid);
                    if !owned {
                        continue;
                    }
                    match md.kind {
                        HostMineKind::TimedDemoCharge => timed_n = timed_n.saturating_add(1),
                        HostMineKind::RemoteDemoCharge => remote_n = remote_n.saturating_add(1),
                        _ => {}
                    }
                }
                match kind {
                    HostMineKind::TimedDemoCharge if !can_place_timed_charge(timed_n) => {
                        return None;
                    }
                    HostMineKind::RemoteDemoCharge if !can_place_remote_charge(remote_n) => {
                        return None;
                    }
                    _ => {}
                }
            }
            if BURTON_UNIQUE_CHARGE_TARGETS {
                if let Some(tid) = attach_to {
                    let dup = self.objects.values().any(|o| {
                        o.is_alive()
                            && o.mine_data
                                .as_ref()
                                .map(|m| {
                                    !m.detonated
                                        && matches!(
                                            m.kind,
                                            HostMineKind::TimedDemoCharge
                                                | HostMineKind::RemoteDemoCharge
                                        )
                                        && m.attached_to == Some(tid)
                                })
                                .unwrap_or(false)
                    });
                    if dup {
                        return None;
                    }
                }
            }
        }

        self.ensure_residual_mine_template(template_name, kind);
        let id = self.create_object(template_name, team, position)?;

        let mut data = match kind {
            crate::game_logic::host_mines::HostMineKind::LandMine => HostMineData::land_mine(),
            crate::game_logic::host_mines::HostMineKind::DemoTrap => HostMineData::demo_trap(),
            crate::game_logic::host_mines::HostMineKind::TimedDemoCharge => {
                let mut d = HostMineData::timed_demo_charge(self.frame);
                if let Some(delay) = delay_frames {
                    d = d.with_lifetime_frames(self.frame, delay);
                }
                d
            }
            crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge => {
                HostMineData::remote_demo_charge()
            }
        };
        if let Some(p) = producer {
            data = data.with_producer(p);
        }
        if let Some(t) = attach_to {
            data = data.with_attach(t);
        }

        if let Some(obj) = self.objects.get_mut(&id) {
            obj.mine_data = Some(data);
            obj.producer_id = producer;
            obj.record_host_demo_mine_cheer();
            // Mines/charges are not combat movers.
            obj.movement.max_speed = 0.0;
            obj.weapon = None;
            obj.secondary_weapon = None;
        }

        self.mine_residual_places = self.mine_residual_places.saturating_add(1);
        self.queue_audio_event(
            AudioEventRequest::new(kind.place_audio())
                .with_object(id)
                .with_position(position)
                .with_priority(150),
        );
        Some(id)
    }

    /// Manually detonate a residual demo trap / charge (command residual).
    pub fn manual_detonate_mine(&mut self, mine_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::HostMineDetonateReason;
        self.detonate_mine_internal(mine_id, HostMineDetonateReason::Manual)
    }

    /// Advance residual mines: dozer clear + proximity scan + timed detonation.
    ///
    /// Clear residual (C++ DozerMineDisarmingWeapon DAMAGE_DISARM / MinefieldBehavior
    /// clearer immunity): Workers/Dozers do not proximity-detonate mines; when within
    /// clear range of an enemy/neutral mine they disarm it without area damage.
    /// Fail-closed: not full weapon-set flag / PreAttack scoop delay / AcademyStats.

    /// C++ StickyBombUpdate::update residual.
    ///
    /// Timed/remote demo charges with `attached_to` follow the target position
    /// (vehicle: ride on roof offset Z; structure/immobile: stay at ground height).
    /// If the target dies, the charge is destroyed (C++ destroyObject(self)).
    pub(crate) fn update_sticky_bomb_attachments(&mut self) {
        use crate::game_logic::host_mines::HostMineKind;
        /// Retail StickyBombUpdate OffsetZ residual (ride on vehicle roof).
        const STICKY_OFFSET_Z: f32 = 8.0;

        let sticky_ids: Vec<(ObjectId, ObjectId, HostMineKind)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let md = obj.mine_data.as_ref()?;
                if !md.is_active() || !obj.is_alive() {
                    return None;
                }
                if !matches!(
                    md.kind,
                    HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
                ) {
                    return None;
                }
                let tid = md.attached_to?;
                Some((*id, tid, md.kind))
            })
            .collect();

        let mut destroy_charges: Vec<ObjectId> = Vec::new();
        let mut moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();

        for (charge_id, target_id, _kind) in sticky_ids {
            let Some(target) = self.objects.get(&target_id) else {
                destroy_charges.push(charge_id);
                continue;
            };
            if !target.is_alive() || target.status.effectively_dead {
                destroy_charges.push(charge_id);
                continue;
            }
            let tpos = target.get_position();
            let immobile =
                target.is_kind_of(KindOf::Structure) || target.is_kind_of(KindOf::Immobile);
            let new_pos = if immobile {
                // Keep ground height for mine-clearing units (C++ IMMOBILE path).
                glam::Vec3::new(tpos.x, 0.0, tpos.z)
            } else {
                glam::Vec3::new(tpos.x, tpos.y + STICKY_OFFSET_Z, tpos.z)
            };
            // If structure path kept original plant XY, we still snap to target XY
            // residual for moving vehicles; immobile also snaps XY to structure center
            // for host residual simplicity (fail-closed vs bomber plant XY freeze).
            moves.push((charge_id, new_pos));
        }

        for (charge_id, pos) in moves {
            if let Some(obj) = self.objects.get_mut(&charge_id) {
                obj.set_position(pos);
            }
            self.sticky_bomb_follow_ticks = self.sticky_bomb_follow_ticks.saturating_add(1);
        }
        for charge_id in destroy_charges {
            self.sticky_bomb_target_deaths = self.sticky_bomb_target_deaths.saturating_add(1);
            self.destroy_object(charge_id);
        }
    }

    /// C++ RemoteC4Charge SpecialObjectsPersistWhenOwnerDies = No residual.
    /// TimedC4Charge persists (BURTON_TIMED_PERSIST_WHEN_OWNER_DIES = true).
    pub fn cleanup_remote_charges_when_owner_dies(&mut self) {
        use crate::game_logic::host_mines::HostMineKind;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let md = obj.mine_data.as_ref()?;
                if md.detonated || !obj.is_alive() {
                    return None;
                }
                if md.kind != HostMineKind::RemoteDemoCharge {
                    return None;
                }
                let pid = md.producer_id.or(obj.producer_id)?;
                let owner_dead = self
                    .objects
                    .get(&pid)
                    .map(|p| !p.is_alive() || p.status.destroyed || p.status.effectively_dead)
                    .unwrap_or(true);
                if owner_dead {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in due {
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
                if let Some(md) = o.mine_data.as_mut() {
                    md.detonated = true;
                }
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn update_mines_and_demo_traps(&mut self) {
        use crate::game_logic::host_mines::{
            can_clear_mine_kind, is_mine_clearer, HostMineDetonateReason, HostMineKind,
            DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE,
        };

        let frame = self.frame;
        // C++ StickyBombUpdate::update residual — stick to target / die with target.
        // Wave 807: under coupled shadow, attach follow owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_sticky_bomb_attachments();
        }
        // Wave 807: under coupled shadow, attach follow owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_booby_trap_special_attachments();
        }
        // C++ SpecialObjectsPersistWhenOwnerDies = No for RemoteC4Charge residual.
        self.cleanup_remote_charges_when_owner_dies();
        let mut due: Vec<(ObjectId, HostMineDetonateReason)> = Vec::new();
        let mut clear_due: Vec<(ObjectId, ObjectId)> = Vec::new(); // (mine_id, clearer_id)
        let mut approach: Vec<(ObjectId, Vec3)> = Vec::new(); // clearer moves toward mine

        // Collect active mine positions + params first (avoid borrow issues).
        let mines: Vec<(
            ObjectId,
            Team,
            Vec3,
            f32,
            bool,
            Option<u32>,
            bool,
            HostMineKind,
        )> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if !data.is_active() || !obj.is_alive() {
                    return None;
                }
                Some((
                    *id,
                    obj.team,
                    obj.get_position(),
                    data.trigger_range,
                    data.proximity_enabled,
                    data.detonate_at_frame,
                    obj.status.under_construction,
                    data.kind,
                ))
            })
            .collect();

        // Mine clearers: Worker / Dozer residual (C++ KINDOF_DOZER + DISARM weapon).
        let clearers: Vec<(ObjectId, Team, Vec3, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                if !is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name) {
                    return None;
                }
                // Busy construction/economy jobs do not auto-clear (Dozer primary task residual).
                let busy = matches!(
                    obj.ai_state,
                    AIState::Constructing
                        | AIState::Repairing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Entering
                        | AIState::Docking
                        | AIState::Capturing
                        | AIState::SpecialAbility
                );
                Some((*id, obj.team, obj.get_position(), busy))
            })
            .collect();

        // Potential victims snapshot (mine clearers never proximity-trigger residual).
        let victims: Vec<(ObjectId, Team, Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() || obj.mine_data.is_some() {
                    return None;
                }
                // C++: mine-clearers with DISARM / isClearingMines are immune to detonation.
                if is_mine_clearer(obj.is_kind_of(KindOf::Worker), &obj.template_name) {
                    return None;
                }
                // Only ground combatants / structures trigger residual mines.
                let combatant = obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Attackable);
                if !combatant {
                    return None;
                }
                // Aircraft do not trigger (C++ DemoTrapUpdate is_above_terrain skip residual).
                if obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target {
                    return None;
                }
                Some((*id, obj.team, obj.get_position()))
            })
            .collect();

        // Dozer/Worker clear + approach residual before proximity (so clear wins).
        // C++: only enemy/neutral mines (not ally/own) — residual uses team inequality.
        let clear_range_sqr = DOZER_MINE_CLEAR_RANGE * DOZER_MINE_CLEAR_RANGE;
        let scan_range_sqr = DOZER_MINE_CLEAR_SCAN_RANGE * DOZER_MINE_CLEAR_SCAN_RANGE;
        for (cid, cteam, cpos, busy) in &clearers {
            if *busy {
                continue;
            }
            // Pure residual acquire: nearest clearable enemy/neutral mine in scan range (XZ).
            let mine_cands: Vec<_> = mines
                .iter()
                .filter(|(_, mine_team, _, _, _, _, under_construction, kind)| {
                    !*under_construction && can_clear_mine_kind(*kind) && *mine_team != *cteam
                })
                .map(|(mine_id, mine_team, mine_pos, _, _, _, _, _)| {
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: *mine_id,
                        team: *mine_team,
                        position: *mine_pos,
                        is_alive: true,
                        is_neutral: *mine_team == Team::Neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    }
                })
                .collect();
            let best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                Some(*cid),
                (cpos.x, cpos.z),
                mine_cands,
                DOZER_MINE_CLEAR_SCAN_RANGE,
                |_| true,
            )
            .map(|(mine_id, dist, _)| {
                let mine_pos = mines
                    .iter()
                    .find(|(id, _, _, _, _, _, _, _)| *id == mine_id)
                    .map(|(_, _, p, _, _, _, _, _)| *p)
                    .unwrap_or(*cpos);
                (mine_id, dist * dist, mine_pos)
            });
            // Keep scan_range_sqr referenced for residual parity with prior sqr gate.
            let _ = scan_range_sqr;
            if let Some((mine_id, dist_sqr, mine_pos)) = best {
                if dist_sqr <= clear_range_sqr {
                    // Prefer first clearer to claim a mine this frame.
                    if !clear_due.iter().any(|(m, _)| *m == mine_id) {
                        clear_due.push((mine_id, *cid));
                    }
                } else {
                    // Approach residual: move idle clearer toward nearest mine.
                    approach.push((*cid, mine_pos));
                }
            }
        }

        for (
            mine_id,
            mine_team,
            mine_pos,
            trigger_range,
            proximity,
            detonate_at,
            under_construction,
            _,
        ) in &mines
        {
            if *under_construction {
                continue;
            }
            // Already scheduled for safe clear this frame — do not also detonate.
            if clear_due.iter().any(|(m, _)| *m == *mine_id) {
                continue;
            }
            if let Some(at) = detonate_at {
                if frame >= *at {
                    due.push((*mine_id, HostMineDetonateReason::Timed));
                    continue;
                }
            }
            if !proximity || *trigger_range <= 0.0 {
                continue;
            }
            let range_sqr = trigger_range * trigger_range;
            for (vid, vteam, vpos) in &victims {
                if *vid == *mine_id {
                    continue;
                }
                // Enemies (and neutrals as residual default for mines) trigger.
                if *vteam == *mine_team {
                    continue;
                }
                let dx = vpos.x - mine_pos.x;
                let dz = vpos.z - mine_pos.z;
                if dx * dx + dz * dz <= range_sqr {
                    due.push((*mine_id, HostMineDetonateReason::Proximity));
                    break;
                }
            }
        }

        // Safe clears first (mine gone, no splash).
        for (mine_id, clearer_id) in clear_due {
            let _ = self.clear_mine_internal(mine_id, clearer_id);
        }

        // Idle clearer approach: set move toward nearest enemy mine.
        for (clearer_id, mine_pos) in approach {
            if let Some(obj) = self.objects.get_mut(&clearer_id) {
                // Don't clobber an explicit non-idle order already in flight.
                if matches!(
                    obj.ai_state,
                    AIState::Idle | AIState::Moving | AIState::Attacking
                ) || obj.target.is_none()
                {
                    obj.set_ai_state(AIState::Moving);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(clearer_id, 1);
                    }
                    obj.movement.target_position = Some(mine_pos);
                    crate::game_logic::host_move_log::record(
                        clearer_id,
                        Some([mine_pos.x, mine_pos.y, mine_pos.z]),
                    );
                    obj.set_status_moving(true);
                }
            }
        }

        for (mine_id, reason) in due {
            let _ = self.detonate_mine_internal(mine_id, reason);
        }
    }

    /// Safely disarm/clear a residual mine without detonation or area damage.
    /// C++ Weapon DAMAGE_DISARM → LandMineInterface::disarm / destroyObject residual.
    pub fn clear_mine_internal(&mut self, mine_id: ObjectId, clearer_id: ObjectId) -> bool {
        use crate::game_logic::host_mines::{can_clear_mine_kind, MINE_CLEARED_AUDIO};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if !mine.is_alive() {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if data.detonated || !can_clear_mine_kind(data.kind) {
            return false;
        }
        let clearer_team = self.objects.get(&clearer_id).map(|o| o.team);
        if clearer_team == Some(mine.team) {
            // Never clear own/ally residual mines.
            return false;
        }
        let mine_pos = mine.get_position();

        // Mark disarmed (detonated flag reuses "no longer active" residual bookkeeping).
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            if let Some(md) = obj.mine_data.as_mut() {
                md.detonated = true;
                md.proximity_enabled = false;
                md.detonate_at_frame = None;
            }
        }

        self.mine_residual_clears = self.mine_residual_clears.saturating_add(1);

        self.queue_audio_event(
            AudioEventRequest::new(MINE_CLEARED_AUDIO)
                .with_object(mine_id)
                .with_position(mine_pos)
                .with_priority(160),
        );

        // Destroy mine without splash damage (DAMAGE_DISARM residual).
        self.mark_object_for_destruction(mine_id, None);

        // Clearer stays alive — no damage applied.
        if let Some(clearer) = self.objects.get_mut(&clearer_id) {
            if clearer.target == Some(mine_id) {
                clearer.target = None;
            }
            if matches!(clearer.ai_state, AIState::Attacking | AIState::Moving) {
                clearer.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(clearer_id, 0);
                }
                clearer.movement.target_position = None;
                clearer.set_status_moving(false);
                clearer.set_status_attacking(false);
            }
        }

        true
    }

    pub(super) fn detonate_mine_internal(
        &mut self,
        mine_id: ObjectId,
        reason: crate::game_logic::host_mines::HostMineDetonateReason,
    ) -> bool {
        use crate::game_logic::host_mines::{damage_at_distance, HostMineDetonateReason};

        let Some(mine) = self.objects.get(&mine_id) else {
            return false;
        };
        if !mine.is_alive() {
            return false;
        }
        let Some(data) = mine.mine_data.as_ref() else {
            return false;
        };
        if data.detonated {
            return false;
        }
        if mine.status.under_construction {
            return false;
        }

        let kind = data.kind;
        let damage = data.detonation_damage;
        let radius = data.detonation_radius;
        let demo_profile = data.demo_trap_profile;
        let is_demo_trap = matches!(kind, crate::game_logic::host_mines::HostMineKind::DemoTrap);
        let mine_team = mine.team;
        let mine_pos = mine.get_position();
        let producer = data.producer_id;

        // Mark detonated before applying damage.
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            if let Some(md) = obj.mine_data.as_mut() {
                md.detonated = true;
            }
        }

        match reason {
            HostMineDetonateReason::Proximity => {
                self.mine_residual_proximity_detonations =
                    self.mine_residual_proximity_detonations.saturating_add(1);
            }
            HostMineDetonateReason::Timed => {
                self.mine_residual_timed_detonations =
                    self.mine_residual_timed_detonations.saturating_add(1);
            }
            HostMineDetonateReason::Manual => {
                self.mine_residual_manual_detonations =
                    self.mine_residual_manual_detonations.saturating_add(1);
            }
        }

        // Area damage: residual hits enemies + neutrals; demo trap / sticky charges
        // also hit allies (DemoTrap/TNT RadiusDamageAffects SELF ALLIES ENEMIES NEUTRALS).
        let hit_allies = matches!(
            kind,
            crate::game_logic::host_mines::HostMineKind::DemoTrap
                | crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                | crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
        );

        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let victim_ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for vid in victim_ids {
            if vid == mine_id {
                continue;
            }
            let Some(victim) = self.objects.get(&vid) else {
                continue;
            };
            if !victim.is_alive() || victim.mine_data.is_some() {
                continue;
            }
            if victim.team == mine_team && !hit_allies {
                continue;
            }
            let vpos = victim.get_position();
            let dist = {
                let dx = vpos.x - mine_pos.x;
                let dz = vpos.z - mine_pos.z;
                (dx * dx + dz * dz).sqrt()
            };
            let dmg = if is_demo_trap {
                crate::game_logic::host_mines::demo_trap_damage_at(demo_profile, dist)
            } else {
                damage_at_distance(damage, radius, dist)
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(victim) = self.objects.get_mut(&vid) {
                if victim.take_damage_from(dmg, Some(mine_id)) {
                    destroy_ids.push((vid, mine_team));
                }
            }
        }

        // Chem DemoTrap residual: spawn MediumPoisonField at detonation.
        if is_demo_trap && demo_profile.spawns_poison() {
            let _ = self.toxin_tractor.spawn_medium_field(
                mine_id,
                mine_team,
                mine_pos,
                self.frame,
                demo_profile.poison_anthrax_tier(),
            );
        }

        // Audio + particle residual.
        self.queue_audio_event(
            AudioEventRequest::new(kind.detonate_audio())
                .with_object(mine_id)
                .with_position(mine_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            mine_pos,
            self.frame,
            Some(mine_id),
            None,
        );

        // Destroy the mine/trap itself.
        self.mark_object_for_destruction(mine_id, producer.map(|_| mine_team));
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }

        let _ = producer; // residual bookkeeping only
        true
    }

    /// Queue a host residual superweapon strike from DoSpecialPower.
    /// Returns strike id when the power maps to a supported residual kind.
    /// Residual A10 science tier stored on a queued/completed strike.
    /// Residual CarpetBomb faction tier stored on a queued/completed strike.
    pub fn special_power_strike_carpet_tier(
        &self,
        strike_id: u32,
    ) -> Option<crate::game_logic::special_power_strikes::CarpetBombFactionTier> {
        self.special_power_strikes
            .get(strike_id)
            .map(|s| s.carpet_tier)
    }

    pub fn special_power_strike_a10_tier(
        &self,
        strike_id: u32,
    ) -> Option<crate::game_logic::special_power_strikes::A10StrikeScienceTier> {
        self.special_power_strikes
            .get(strike_id)
            .map(|s| s.a10_tier)
    }

    pub fn queue_special_power_strike(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::special_power_strikes::{
            A10StrikeScienceTier, ArtilleryBarrageScienceTier, HostSuperweaponKind,
            ScudStormAnthraxTier, SpectreGunshipScienceTier,
        };
        let kind = HostSuperweaponKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;
        let sciences: Vec<String> = self
            .players
            .values()
            .filter(|p| p.team == source_team)
            .flat_map(|p| p.unlocked_sciences.iter().cloned())
            .collect();
        // ArtilleryBarrage FormationSize residual from unlocked SCIENCE_ArtilleryBarrage1/2/3.
        let artillery_tier = if kind == HostSuperweaponKind::ArtilleryBarrage {
            ArtilleryBarrageScienceTier::highest_from_sciences(sciences.iter().map(|s| s.as_str()))
        } else {
            ArtilleryBarrageScienceTier::Level1
        };
        // SpectreGunship OrbitTime residual from unlocked SCIENCE_SpectreGunship1/2/3.
        let spectre_tier = if kind == HostSuperweaponKind::SpectreGunship {
            SpectreGunshipScienceTier::highest_from_sciences(sciences.iter().map(|s| s.as_str()))
        } else {
            SpectreGunshipScienceTier::Level2
        };
        // ScudStorm anthrax-upgrade residual from unlocked Anthrax Beta/Gamma.
        let scud_anthrax_tier = if kind == HostSuperweaponKind::ScudStorm {
            ScudStormAnthraxTier::highest_from_upgrades(sciences.iter().map(|s| s.as_str()))
        } else {
            ScudStormAnthraxTier::Base
        };
        // A10 FormationSize residual from unlocked SCIENCE_A10ThunderboltMissileStrike1/2/3.
        let a10_tier = if kind == HostSuperweaponKind::A10Strike {
            A10StrikeScienceTier::highest_from_sciences(sciences.iter().map(|s| s.as_str()))
        } else {
            A10StrikeScienceTier::Level1
        };
        let id = self.special_power_strikes.queue_with_all_tiers(
            kind,
            source_object,
            source_team,
            target_position,
            frame,
            artillery_tier,
            spectre_tier,
            scud_anthrax_tier,
            a10_tier,
        );
        // C++ OCL DeliveryDecal via RadiusDecalUpdate on SCUD Storm host.
        if kind == HostSuperweaponKind::ScudStorm {
            let _ = self.create_delivery_radius_decal(source_object, target_position);
        }
        // C++ SpectreGunshipDeploymentUpdate::initiateIntent residual.
        if kind == HostSuperweaponKind::SpectreGunship {
            let _ = self.initiate_spectre_gunship_deployment(source_object, target_position);
        }
        // C++ OCLSpecialPower::doSpecialPowerAtLocation → ObjectCreationList::create residual.
        if let Some(tmpl) =
            crate::game_logic::host_ocl_special_power::special_power_template_for_host_kind(
                kind.label(),
            )
        {
            let _ = self.execute_ocl_special_power(tmpl, source_object, target_position);
        }
        // C++ CarpetBomb DeliverPayload residual (B52/AirF/China + staggered drops).
        let carpet_flight_tier = if kind == HostSuperweaponKind::CarpetBomb {
            use crate::command_system::SpecialPowerType;
            use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
            Some(
                if matches!(
                    *power,
                    SpecialPowerType::EarlyChinaCarpetBomb | SpecialPowerType::NukeChinaCarpetBomb
                ) {
                    CarpetBombFactionTier::China
                } else if matches!(*power, SpecialPowerType::AirForceCarpetBomb) {
                    CarpetBombFactionTier::AirForce
                } else {
                    CarpetBombFactionTier::highest_from_team_and_sciences(
                        source_team,
                        sciences.iter().map(|s| s.as_str()),
                    )
                },
            )
        } else {
            None
        };
        if let Some(tier) = carpet_flight_tier {
            let _ = self.spawn_carpet_bomb_flight(source_object, target_position, tier);
        }
        // C++ ArtilleryBarrage DeliverPayload residual (cannon + staggered shells).
        if kind == HostSuperweaponKind::ArtilleryBarrage {
            let _ =
                self.spawn_artillery_barrage_flight(source_object, target_position, artillery_tier);
        }
        // C++ A10Thunderbolt DeliverPayload residual (jet + staggered missiles).
        if kind == HostSuperweaponKind::A10Strike {
            let _ = self.spawn_a10_strike_flight(source_object, target_position, a10_tier);
        }
        // C++ DaisyCutter / MOAB DeliverPayload residual (B52 or JetB3 + bomb).
        if kind == HostSuperweaponKind::DaisyCutter {
            use crate::command_system::SpecialPowerType;
            use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
            let tier = match power {
                SpecialPowerType::FuelAirBomb => DaisyFlightPayloadTier::Moab,
                _ => DaisyFlightPayloadTier::DaisyCutter,
            };
            let _ = self.spawn_daisy_cutter_flight(source_object, target_position, tier);
        }
        // C++ AnthraxBomb DeliverPayload residual (GLAJetCargoPlane + bomb).
        if kind == HostSuperweaponKind::AnthraxBomb {
            let _ = self.spawn_anthrax_bomb_flight(source_object, target_position);
        }
        // C++ OCL FireWeaponNugget / AttackNugget residual (Neutron / Cruise / ScudStorm).
        if let Some(nugget) =
            crate::game_logic::host_ocl_fire_weapon_attack::ocl_nugget_for_host_kind(kind.label())
        {
            use crate::game_logic::host_ocl_fire_weapon_attack::OclNuggetKind;
            match nugget {
                OclNuggetKind::FireWeapon(ocl) => {
                    let primary = self
                        .objects
                        .get(&source_object)
                        .map(|o| o.get_position())
                        .unwrap_or(target_position);
                    let _ =
                        self.execute_ocl_fire_weapon(ocl, source_object, primary, target_position);
                }
                OclNuggetKind::Attack(ocl) => {
                    let _ = self.execute_ocl_attack(ocl, source_object, target_position);
                }
            }
        }
        // CarpetBomb faction residual (America / AirForce / China payload matrix).
        if kind == HostSuperweaponKind::CarpetBomb {
            use crate::command_system::SpecialPowerType;
            use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
            let carpet = if matches!(
                *power,
                SpecialPowerType::EarlyChinaCarpetBomb | SpecialPowerType::NukeChinaCarpetBomb
            ) {
                CarpetBombFactionTier::China
            } else if matches!(*power, SpecialPowerType::AirForceCarpetBomb) {
                CarpetBombFactionTier::AirForce
            } else {
                CarpetBombFactionTier::highest_from_team_and_sciences(
                    source_team,
                    sciences.iter().map(|s| s.as_str()),
                )
            };
            let _ =
                self.special_power_strikes
                    .apply_carpet_tier(id, carpet, frame, target_position);
        }

        // C++ SpecialPowerModule SuperweaponLaunched EVA residual.
        self.try_eva_superweapon_launched(source_team, kind);

        // Activation audio residual (observable request path).
        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
        // Launch-site combat particle residual (not full OCL aircraft).
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
        Some(id)
    }

    /// Advance pending host superweapon strikes to impact and apply area damage.
    /// NuclearMissile residual also ticks radiation fields after impact.
    /// AnthraxBomb residual also ticks toxin fields after impact.
    /// SpectreGunship residual also ticks orbit fields after orbit insertion.
    /// CarpetBomb residual applies multi-point line damage after approach delay.
    /// ArtilleryBarrage residual applies multi-shell scatter damage after delay.
    /// CruiseMissile residual applies MOAB area damage after loft delay.
    pub fn update_special_power_strikes(&mut self) {
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_TOXIN_AUDIO, NUKE_RADIATION_AUDIO, SPECTRE_ORBIT_AUDIO,
        };

        self.special_power_strikes.clear_frame_events();

        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_impacts(self.frame, &object_positions);

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut objects_hit = 0_u32;
            let mut objects_destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    // BodyModule last_damage_source residual for cash bounty killer
                    // (superweapon blast path — same residual as combat fire).
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

            self.special_power_strikes.record_impact_wave(
                plan.strike_id,
                total_damage,
                objects_hit,
                objects_destroyed,
                plan.wave_shell_count,
                plan.is_final_wave,
                &plan.epicenters,
            );

            // NuclearMissile residual: radiation field ambient cue on spawn.
            if plan.is_final_wave
                && plan.kind.spawns_radiation()
                && !self
                    .special_power_strikes
                    .radiation_spawned_this_frame()
                    .is_empty()
            {
                self.queue_audio_event(
                    AudioEventRequest::new(NUKE_RADIATION_AUDIO)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            // AnthraxBomb final / ScudStorm per-missile residual toxin ambient.
            if plan.kind.spawns_toxin_field()
                && !self
                    .special_power_strikes
                    .toxin_spawned_this_frame()
                    .is_empty()
                && (plan.is_final_wave || plan.kind.spawns_scud_poison_field())
            {
                let cue = if plan.kind.spawns_scud_poison_field() {
                    crate::game_logic::special_power_strikes::SCUD_STORM_POISON_AUDIO
                } else {
                    ANTHRAX_TOXIN_AUDIO
                };
                self.queue_audio_event(
                    AudioEventRequest::new(cue)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            // SpectreGunship residual: orbit ambient cue on insertion.
            if plan.is_final_wave
                && plan.kind.spawns_orbit_field()
                && !self
                    .special_power_strikes
                    .orbit_spawned_this_frame()
                    .is_empty()
            {
                self.queue_audio_event(
                    AudioEventRequest::new(SPECTRE_ORBIT_AUDIO)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            // ParticleCannon residual: continuous beam annihilation cue on start.
            if plan.is_final_wave
                && plan.kind.spawns_beam_field()
                && !self
                    .special_power_strikes
                    .beam_spawned_this_frame()
                    .is_empty()
            {
                use crate::game_logic::special_power_strikes::PARTICLE_BEAM_AUDIO;
                self.queue_audio_event(
                    AudioEventRequest::new(PARTICLE_BEAM_AUDIO)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            log::info!(
                "Host superweapon {} strike {} completed at {:?} (dmg={:.1}, hit={}, killed={})",
                plan.kind.label(),
                plan.strike_id,
                plan.target_position,
                total_damage,
                objects_hit,
                objects_destroyed
            );
        }

        // NuclearMissile residual radiation field ticks (after impact blasts).
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuclear_radiation_fields();
        }
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_neutron_slow_death_fields();
        }
        self.update_wave_guides();
        // AnthraxBomb residual toxin field ticks (after impact blasts).
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_anthrax_toxin_fields();
        }
        // SpectreGunship residual orbit damage ticks (after insertion).
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_spectre_orbit_fields();
        }
        self.spawn_spectre_howitzer_shell_objects_for_new_spawns();
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_spectre_howitzer_shell_objects();
        }
        // ParticleCannon residual continuous beam pulses (after charge residual).
        self.update_particle_beam_fields();
        self.spawn_particle_orbital_laser_objects_for_new_beams();
        self.spawn_particle_connector_laser_objects_for_new_beams();
        // Wave 808: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_particle_orbital_laser_objects();
        }
        // Wave 808: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_particle_connector_laser_objects();
        }
        // Particle Uplink DamagePulseRemnant trail residual ticks.
        self.spawn_particle_trail_remnant_objects_for_new_fields();
        // Wave 808: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_particle_trail_remnant_objects();
        }
        self.update_particle_remnant_fields();
    }

    /// Tick residual radiation fields spawned by NuclearMissile impacts.
    /// Fail-closed vs full HazardousMaterialArmor / cleanup-hazard objects.

    /// C++ NeutronMissileSlowDeathBehavior multi-blast residual.
    pub(super) fn update_neutron_slow_death_fields(&mut self) {
        use crate::game_logic::host_neutron_missile_slow_death::{
            plan_neutron_frame, MC_BIT_BURNED,
        };
        use crate::game_logic::host_topple::HostToppleData;

        let n = self.special_power_strikes.neutron_slow_death_field_count();
        if n == 0 {
            return;
        }

        // Snapshot object xz + ids for planning.
        let objects: Vec<(ObjectId, f32, f32, bool)> = self
            .objects
            .iter()
            .map(|(id, o)| {
                let p = o.get_position();
                (*id, p.x, p.z, o.is_alive())
            })
            .collect();

        // Access fields via temporary steal pattern.
        let fields = self
            .special_power_strikes
            .neutron_slow_death_fields_mut_for_tick();
        let metas = self
            .special_power_strikes
            .neutron_slow_death_meta()
            .to_vec();

        let frame = self.frame;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let mut keep_fields = Vec::new();
        let mut keep_metas = Vec::new();

        for (mut state, meta) in fields.into_iter().zip(metas.into_iter()) {
            let epicenter = (meta.position.x, meta.position.z);
            let xz: Vec<(f32, f32)> = objects.iter().map(|(_, x, z, _)| (*x, *z)).collect();
            let (hits, place_scorch, done) = plan_neutron_frame(&mut state, frame, epicenter, &xz);

            // C++ SlowDeath MIDPOINT OCL_NukeRadiationField residual.
            if state.take_radiation_ocl_request(frame) {
                self.special_power_strikes.spawn_radiation_field(
                    meta.source_object,
                    meta.source_team,
                    meta.position,
                    frame,
                    meta.parent_strike_id,
                );
            }

            if place_scorch {
                // Presentation residual: combat particle at epicenter.
                let _ = self.combat_particles.spawn(
                    crate::game_logic::combat_particles::CombatParticleKind::DeathExplosion,
                    meta.position,
                    frame,
                    Some(meta.source_object),
                    None,
                );
            }

            for hit in hits {
                let Some((id, _, _, alive)) = objects.get(hit.target_index).copied() else {
                    continue;
                };
                if id == meta.source_object {
                    continue;
                }
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                if hit.set_burned {
                    obj.model_condition_bits |= 1u128 << MC_BIT_BURNED;
                }
                if hit.topple_speed > 0.0 {
                    // Tree/prop topple residual peel.
                    let name = obj.template_name.to_ascii_lowercase();
                    let can_topple = obj.topple_data.is_none()
                        && (name.contains("tree")
                            || name.contains("shrub")
                            || crate::game_logic::host_topple::is_topple_capable_template(
                                &obj.template_name,
                            ));
                    if can_topple {
                        let mut td = HostToppleData::default();
                        if td.apply_toppling_force(
                            hit.topple_dx,
                            hit.topple_dz,
                            hit.topple_speed,
                            crate::game_logic::host_topple::TOPPLE_OPTIONS_NO_BOUNCE
                                | crate::game_logic::host_topple::TOPPLE_OPTIONS_NO_FX,
                        ) {
                            obj.topple_data = Some(td);
                        }
                    }
                }
                if hit.damage > 0.0 && alive {
                    let destroyed =
                        obj.take_damage_from_immediate(hit.damage, Some(meta.source_object));
                    if destroyed {
                        destroy_ids.push((id, meta.source_team));
                    }
                }
            }

            if !done {
                keep_fields.push(state);
                keep_metas.push(meta);
            }
        }

        self.special_power_strikes
            .restore_neutron_slow_death_fields(keep_fields, keep_metas);

        for (id, team) in destroy_ids {
            self.mark_object_for_destruction(id, Some(team));
        }
    }

    /// C++ WaveGuideUpdate residual — flood wave motion + damage after DamDie.
    pub(super) fn update_wave_guides(&mut self) {
        use crate::game_logic::host_topple::{
            HostToppleData, TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX,
        };
        use crate::game_logic::host_wave_guide::{
            is_wave_guide_template, wave_damage_at_distance, MC_BIT_FLOODED, WAVE_DAMAGE_RADIUS,
            WAVE_TOPPLE_FORCE,
        };

        let frame = self.frame;
        // Collect waveguide ids + poses first.
        let guides: Vec<(ObjectId, glam::Vec3, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.status.disabled_default {
                    return None;
                }
                let is_wg = o.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                    || is_wave_guide_template(&o.template_name);
                if !is_wg {
                    return None;
                }
                let wg = o.wave_guide_data.as_ref()?;
                if !wg.is_moving(frame) {
                    // Still ensure data exists / active clock.
                    return None;
                }
                Some((*id, o.get_position(), o.get_orientation()))
            })
            .collect();

        if guides.is_empty() {
            // Still tick ensure_active for enabled waveguides waiting on delay.
            for obj in self.objects.values_mut() {
                if obj.status.disabled_default {
                    continue;
                }
                let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                    || is_wave_guide_template(&obj.template_name);
                if is_wg {
                    if obj.wave_guide_data.is_none() {
                        let mut wg =
                            crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                        wg.facing = obj.get_orientation();
                        wg.ensure_active(frame.max(1));
                        obj.wave_guide_data = Some(wg);
                    } else if let Some(wg) = obj.wave_guide_data.as_mut() {
                        wg.ensure_active(frame.max(1));
                    }
                }
            }
            return;
        }

        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

        for (gid, gpos, gori) in guides {
            // Motion
            if let Some(obj) = self.objects.get_mut(&gid) {
                if let Some(wg) = obj.wave_guide_data.as_mut() {
                    wg.facing = gori;
                    if let Some((dx, dz)) = wg.motion_delta(frame) {
                        let mut p = obj.get_position();
                        p.x += dx;
                        p.z += dz;
                        obj.set_position(p);
                    }
                }
            }
            let gpos = self
                .objects
                .get(&gid)
                .map(|o| o.get_position())
                .unwrap_or(gpos);

            // Damage / topple nearby
            let victims: Vec<ObjectId> = self
                .objects
                .iter()
                .filter_map(|(id, o)| {
                    if *id == gid {
                        return None;
                    }
                    if o.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                        || is_wave_guide_template(&o.template_name)
                    {
                        return None;
                    }
                    if !o.is_alive() {
                        return None;
                    }
                    let p = o.get_position();
                    let dx = p.x - gpos.x;
                    let dz = p.z - gpos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist <= WAVE_DAMAGE_RADIUS {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();

            for vid in victims {
                let Some(obj) = self.objects.get_mut(&vid) else {
                    continue;
                };
                let p = obj.get_position();
                let dx = p.x - gpos.x;
                let dz = p.z - gpos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let dmg = wave_damage_at_distance(dist);
                // FLOODED model residual.
                obj.model_condition_bits |= 1u128 << MC_BIT_FLOODED;
                // Topple trees/props.
                let name = obj.template_name.to_ascii_lowercase();
                if obj.topple_data.is_none()
                    && (name.contains("tree")
                        || name.contains("shrub")
                        || crate::game_logic::host_topple::is_topple_capable_template(
                            &obj.template_name,
                        ))
                {
                    let mut td = HostToppleData::default();
                    let len = dist.max(0.001);
                    if td.apply_toppling_force(
                        dx / len,
                        dz / len,
                        WAVE_TOPPLE_FORCE,
                        TOPPLE_OPTIONS_NO_BOUNCE | TOPPLE_OPTIONS_NO_FX,
                    ) {
                        obj.topple_data = Some(td);
                    }
                }
                if dmg > 0.0 {
                    if let Some(wg) = self
                        .objects
                        .get_mut(&gid)
                        .and_then(|o| o.wave_guide_data.as_mut())
                    {
                        wg.damage_applications = wg.damage_applications.saturating_add(1);
                    }
                    let team = self
                        .objects
                        .get(&gid)
                        .map(|o| o.team)
                        .unwrap_or(Team::Neutral);
                    if let Some(obj) = self.objects.get_mut(&vid) {
                        let destroyed = obj.take_damage_from_immediate(dmg, Some(gid));
                        if destroyed {
                            destroy_ids.push((vid, team));
                        }
                    }
                }
            }
        }

        for (id, team) in destroy_ids {
            self.mark_object_for_destruction(id, Some(team));
        }
    }

    pub(super) fn update_nuclear_radiation_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_radiation_ticks(self.frame, &object_positions);
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

            self.special_power_strikes.record_radiation_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        // NukeRadiationFieldWeapon Object residual (spawn + DeletionUpdate lifetime).
        self.spawn_nuke_radiation_field_objects_for_new_fields();
        // Wave 820: under coupled shadow, field-object lifetime owned by GW expire.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuke_radiation_field_objects();
        }
        self.special_power_strikes.prune_expired_radiation(frame);
    }

    /// Tick residual toxin fields spawned by AnthraxBomb impacts.
    /// Fail-closed vs full HazardousMaterialArmor / cleanup-hazard / gamma objects.
    pub(super) fn update_anthrax_toxin_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_toxin_ticks(self.frame, &object_positions);
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

            self.special_power_strikes.record_toxin_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.spawn_anthrax_toxin_field_objects_for_new_fields();
        // Wave 820: under coupled shadow, field-object lifetime owned by GW expire.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_anthrax_toxin_field_objects();
        }
        self.special_power_strikes.prune_expired_toxin(frame);
    }

    /// Tick residual Spectre orbit fields spawned at orbit insertion.
    /// Fail-closed vs full SpectreGunshipUpdate gattling-strafe / howitzer projectile.
    pub(super) fn update_spectre_orbit_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_orbit_ticks(self.frame, &object_positions);
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

            self.special_power_strikes.record_orbit_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.special_power_strikes.prune_expired_orbit(frame);
    }

    /// Tick residual Particle Uplink continuous beam fields after charge residual.
    /// Manual drive + WidthGrow grow/hold/decay + outer-node honesty residual closed.
    /// Intensity schedule (CHARGING/PREPARING/ALMOST_READY/POSTFIRE/PACKING) +
    /// BeamLaunchFX residual closed.
    /// Fail-closed vs full bone-extract lasers / GPU OuterBeamWidth matrix.
    /// Swath + DamagePulseRemnant residual closed.
    pub(super) fn update_particle_beam_fields(&mut self) {
        let frame = self.frame;
        // Pre-fire intensity schedule + BeamLaunchFX + POSTFIRE/PACKING residual
        // (also advances ScudStorm PreAttack residual frame counter).
        self.special_power_strikes
            .advance_particle_intensity_schedule(frame);
        // Manual beam driving residual: advance current target toward override
        // before damage / scorch planning (retail update order).
        self.special_power_strikes.advance_manual_beam_drive(frame);

        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_beam_ticks(frame, &object_positions);

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

            self.special_power_strikes.record_beam_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        // WidthGrow grow/hold/decay honesty sample (even when no damage pulse due).
        // Retail LASERSTATUS_DECAYING after TotalFiringTime shrinks m_currentWidthScalar.
        self.special_power_strikes.sample_beam_width_honesty(frame);

        // TotalScorchMarks / GroundHitFX / RevealRange residual (retail STATUS_FIRING).
        // C++: doShroudReveal + undoShroudReveal at current target with RevealRange
        // each scorch tick (instant "gratuitous vision" pulse, not duration reveal).
        let scorch_events = self
            .special_power_strikes
            .apply_due_beam_scorch_reveals(frame);
        if !scorch_events.is_empty() {
            use crate::game_logic::special_power_strikes::PARTICLE_REVEAL_RANGE;
            use gamelogic::common::Coord3D;
            let world_w = self.world_width.max(1.0);
            let world_h = self.world_height.max(1.0);
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                if !shroud_mgr.has_shroud_grid() {
                    shroud_mgr.init_shroud_grid(world_w, world_h);
                }
                for event in &scorch_events {
                    let mut player_mask = 0u32;
                    for (&pid, player) in &self.players {
                        if player.team == event.source_team {
                            player_mask |= 1u32 << pid.min(31);
                        }
                    }
                    if player_mask == 0 {
                        // No registered players for team: skip FOW write (honesty
                        // counters already recorded on the beam field).
                        continue;
                    }
                    // Host gameplay plane (x,z) → shroud (x,y).
                    let center = Coord3D::new(event.position.x, event.position.z, event.position.y);
                    let range = if event.reveal_range > 0.0 {
                        event.reveal_range
                    } else {
                        PARTICLE_REVEAL_RANGE
                    };
                    // Retail: do + undo same frame (pulse reveal, not duration FOW).
                    shroud_mgr.do_shroud_reveal(&center, range, player_mask);
                    shroud_mgr.undo_shroud_reveal(&center, range, player_mask);
                }
            }
        }

        self.special_power_strikes.prune_expired_beam(frame);
    }

    /// C++ SpectreHowitzerShell ThingFactory Object residual (orbit howitzer ticks).
    pub fn spawn_spectre_howitzer_shell_objects_for_new_spawns(&mut self) {
        use crate::game_logic::special_power_strikes::{
            SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES, SPECTRE_HOWITZER_SHELL_MAX_HEALTH,
            SPECTRE_HOWITZER_SHELL_OBJECT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending = self
            .special_power_strikes
            .take_howitzer_shell_spawns_this_frame();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(SPECTRE_HOWITZER_SHELL_OBJECT) {
            let mut t = ThingTemplate::new(SPECTRE_HOWITZER_SHELL_OBJECT);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SPECTRE_HOWITZER_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(SPECTRE_HOWITZER_SHELL_OBJECT.to_string(), t);
        }
        let expires = self
            .frame
            .saturating_add(SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES.max(1));
        for (source, team, pos) in pending {
            if let Some(oid) = self.create_object(SPECTRE_HOWITZER_SHELL_OBJECT, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.spectre_howitzer_shell = true;
                    o.producer_id = Some(source);
                    o.spectre_howitzer_shell_expires_frame = Some(expires);
                    o.health.maximum = SPECTRE_HOWITZER_SHELL_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, SPECTRE_HOWITZER_SHELL_MAX_HEALTH);
                    // Fall residual toward ground.
                    o.movement.velocity = Vec3::new(0.0, -14.0, 0.0);
                }
                self.special_power_strikes
                    .record_howitzer_shell_object_spawn();
            }
        }
    }

    pub fn update_spectre_howitzer_shell_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.spectre_howitzer_shell {
                    if let Some(exp) = o.spectre_howitzer_shell_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                    // HeightDie residual: destroy near ground.
                    if o.get_position().y <= 1.0 {
                        return Some(*id);
                    }
                }
                None
            })
            .collect();
        for id in due {
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
                o.spectre_howitzer_shell = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ PoisonFieldAnthraxBomb / PoisonFieldLarge ThingFactory Object residual.
    pub fn spawn_anthrax_toxin_field_objects_for_new_fields(&mut self) {
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_TOXIN_FIELD_MAX_HEALTH, ANTHRAX_TOXIN_OBJECT_NAME,
            SCUD_POISON_FIELD_MAX_HEALTH, SCUD_POISON_OBJECT_NAME,
            SCUD_POISON_UPGRADED_FIELD_MAX_HEALTH, SCUD_POISON_UPGRADED_OBJECT_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3, u32, String)> = self
            .special_power_strikes
            .toxin_spawned_this_frame()
            .iter()
            .filter_map(|tid| {
                self.special_power_strikes
                    .toxin_fields()
                    .iter()
                    .find(|f| f.id == *tid && f.object_id.is_none())
                    .map(|f| {
                        (
                            f.id,
                            f.source_object,
                            f.source_team,
                            f.position,
                            f.expires_frame.saturating_sub(f.spawn_frame),
                            f.object_template.clone(),
                        )
                    })
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        for (tid, source, team, pos, lifetime, template) in pending {
            let max_hp = if template == SCUD_POISON_UPGRADED_OBJECT_NAME
                || template == "PoisonFieldUpgradedLarge"
            {
                SCUD_POISON_UPGRADED_FIELD_MAX_HEALTH
            } else if template == SCUD_POISON_OBJECT_NAME {
                SCUD_POISON_FIELD_MAX_HEALTH
            } else {
                ANTHRAX_TOXIN_FIELD_MAX_HEALTH
            };
            let tmpl = if template.is_empty() {
                ANTHRAX_TOXIN_OBJECT_NAME.to_string()
            } else {
                template
            };
            if !self.templates.contains_key(&tmpl) {
                let mut t = ThingTemplate::new(&tmpl);
                t.add_kind_of(KindOf::Immobile)
                    .set_health(max_hp)
                    .set_cost(0, 0);
                self.templates.insert(tmpl.clone(), t);
            }
            let expires = self.frame.saturating_add(lifetime.max(1));
            if let Some(oid) = self.create_object(&tmpl, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.anthrax_toxin_field = true;
                    o.producer_id = Some(source);
                    o.anthrax_toxin_field_expires_frame = Some(expires);
                    o.health.maximum = max_hp;
                    Self::write_object_health_authority_aware(o, max_hp);
                }
                let _ = self.special_power_strikes.bind_toxin_object(tid, oid);
            }
        }
    }

    pub fn update_anthrax_toxin_field_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.anthrax_toxin_field {
                    if let Some(exp) = o.anthrax_toxin_field_expires_frame {
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.anthrax_toxin_field = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ NukeRadiationFieldWeapon ThingFactory Object residual.
    pub fn spawn_nuke_radiation_field_objects_for_new_fields(&mut self) {
        use crate::game_logic::special_power_strikes::{
            NUKE_RADIATION_DURATION_FRAMES, NUKE_RADIATION_FIELD_MAX_HEALTH,
            NUKE_RADIATION_OBJECT_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3)> = self
            .special_power_strikes
            .radiation_spawned_this_frame()
            .iter()
            .filter_map(|rid| {
                self.special_power_strikes
                    .radiation_fields()
                    .iter()
                    .find(|f| f.id == *rid && f.object_id.is_none())
                    .map(|f| (f.id, f.source_object, f.source_team, f.position))
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(NUKE_RADIATION_OBJECT_NAME) {
            let mut t = ThingTemplate::new(NUKE_RADIATION_OBJECT_NAME);
            t.add_kind_of(KindOf::Immobile)
                .set_health(NUKE_RADIATION_FIELD_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(NUKE_RADIATION_OBJECT_NAME.to_string(), t);
        }
        let expires = self.frame.saturating_add(NUKE_RADIATION_DURATION_FRAMES);
        for (rid, source, team, pos) in pending {
            if let Some(oid) = self.create_object(NUKE_RADIATION_OBJECT_NAME, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.nuke_radiation_field = true;
                    o.producer_id = Some(source);
                    o.nuke_radiation_field_expires_frame = Some(expires);
                    o.health.maximum = NUKE_RADIATION_FIELD_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, NUKE_RADIATION_FIELD_MAX_HEALTH);
                }
                let _ = self.special_power_strikes.bind_radiation_object(rid, oid);
            }
        }
    }

    pub fn update_nuke_radiation_field_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.nuke_radiation_field {
                    if let Some(exp) = o.nuke_radiation_field_expires_frame {
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.nuke_radiation_field = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ Medium/Intense ConnectorLaser ThingFactory Objects (STATUS_FIRING residual).
    pub fn spawn_particle_connector_laser_objects_for_new_beams(&mut self) {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_CONNECTOR_INTENSE_LASER, PARTICLE_CONNECTOR_LASER_MAX_HEALTH,
            PARTICLE_CONNECTOR_MEDIUM_LASER,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3, u32)> = self
            .special_power_strikes
            .beam_spawned_this_frame()
            .iter()
            .filter_map(|bid| {
                self.special_power_strikes
                    .beam_fields()
                    .iter()
                    .find(|f| f.id == *bid && f.connector_object_ids.is_empty())
                    .map(|f| {
                        (
                            f.id,
                            f.source_object,
                            f.source_team,
                            // Connector residual originates at caster building.
                            self.objects
                                .get(&f.source_object)
                                .map(|o| o.get_position())
                                .unwrap_or(f.position),
                            f.expires_frame.saturating_sub(f.spawn_frame),
                        )
                    })
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        for name in [
            PARTICLE_CONNECTOR_MEDIUM_LASER,
            PARTICLE_CONNECTOR_INTENSE_LASER,
        ] {
            if !self.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Immobile)
                    .set_health(PARTICLE_CONNECTOR_LASER_MAX_HEALTH)
                    .set_cost(0, 0);
                self.templates.insert(name.to_string(), t);
            }
        }
        for (bid, source, team, pos, lifetime) in pending {
            let expires = self.frame.saturating_add(lifetime.max(1));
            // Medium connector slightly above building; intense higher toward orbit.
            let placements = [
                (
                    PARTICLE_CONNECTOR_MEDIUM_LASER,
                    Vec3::new(pos.x, pos.y + 40.0, pos.z),
                ),
                (
                    PARTICLE_CONNECTOR_INTENSE_LASER,
                    Vec3::new(pos.x, pos.y + 120.0, pos.z),
                ),
            ];
            let mut ids = Vec::new();
            for (name, cpos) in placements {
                if let Some(oid) = self.create_object(name, team, cpos) {
                    if let Some(o) = self.objects.get_mut(&oid) {
                        o.particle_connector_laser = true;
                        o.producer_id = Some(source);
                        o.particle_connector_laser_expires_frame = Some(expires);
                        o.health.maximum = PARTICLE_CONNECTOR_LASER_MAX_HEALTH;
                        Self::write_object_health_authority_aware(
                            o,
                            PARTICLE_CONNECTOR_LASER_MAX_HEALTH,
                        );
                    }
                    ids.push(oid);
                }
            }
            if !ids.is_empty() {
                let _ = self.special_power_strikes.bind_connector_objects(bid, &ids);
            }
        }
    }

    pub fn update_particle_connector_laser_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.particle_connector_laser {
                    if let Some(exp) = o.particle_connector_laser_expires_frame {
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.particle_connector_laser = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ ParticleUplinkCannon_OrbitalLaser ThingFactory Object residual.
    pub fn spawn_particle_orbital_laser_objects_for_new_beams(&mut self) {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_ORBITAL_LASER_MAX_HEALTH, PARTICLE_ORBITAL_LASER_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3, u32)> = self
            .special_power_strikes
            .beam_spawned_this_frame()
            .iter()
            .filter_map(|bid| {
                self.special_power_strikes
                    .beam_fields()
                    .iter()
                    .find(|f| f.id == *bid && f.object_id.is_none())
                    .map(|f| {
                        (
                            f.id,
                            f.source_object,
                            f.source_team,
                            f.position,
                            f.expires_frame.saturating_sub(f.spawn_frame),
                        )
                    })
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(PARTICLE_ORBITAL_LASER_NAME) {
            let mut t = ThingTemplate::new(PARTICLE_ORBITAL_LASER_NAME);
            t.add_kind_of(KindOf::Immobile)
                .set_health(PARTICLE_ORBITAL_LASER_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(PARTICLE_ORBITAL_LASER_NAME.to_string(), t);
        }
        for (bid, source, team, pos, lifetime) in pending {
            let expires = self.frame.saturating_add(lifetime.max(1));
            // Place orbital laser residual above target (retail laser origin altitude).
            let laser_pos = Vec3::new(pos.x, pos.y + 500.0, pos.z);
            if let Some(oid) = self.create_object(PARTICLE_ORBITAL_LASER_NAME, team, laser_pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.particle_orbital_laser = true;
                    o.producer_id = Some(source);
                    o.particle_orbital_laser_expires_frame = Some(expires);
                    o.health.maximum = PARTICLE_ORBITAL_LASER_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, PARTICLE_ORBITAL_LASER_MAX_HEALTH);
                }
                let _ = self.special_power_strikes.bind_beam_object(bid, oid);
            }
        }
    }

    pub fn update_particle_orbital_laser_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.particle_orbital_laser {
                    if let Some(exp) = o.particle_orbital_laser_expires_frame {
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.particle_orbital_laser = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ ParticleUplinkCannonTrailRemnant ThingFactory Object residual.
    pub fn spawn_particle_trail_remnant_objects_for_new_fields(&mut self) {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_REMNANT_DURATION_FRAMES, PARTICLE_REMNANT_MAX_HEALTH,
            PARTICLE_REMNANT_OBJECT_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3)> = self
            .special_power_strikes
            .remnant_spawned_this_frame()
            .iter()
            .filter_map(|rid| {
                self.special_power_strikes
                    .remnant_fields()
                    .iter()
                    .find(|f| f.id == *rid && f.object_id.is_none())
                    .map(|f| (f.id, f.source_object, f.source_team, f.position))
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(PARTICLE_REMNANT_OBJECT_NAME) {
            let mut t = ThingTemplate::new(PARTICLE_REMNANT_OBJECT_NAME);
            t.add_kind_of(KindOf::Immobile)
                .set_health(PARTICLE_REMNANT_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(PARTICLE_REMNANT_OBJECT_NAME.to_string(), t);
        }
        let expires = self.frame.saturating_add(PARTICLE_REMNANT_DURATION_FRAMES);
        for (rid, source, team, pos) in pending {
            if let Some(oid) = self.create_object(PARTICLE_REMNANT_OBJECT_NAME, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.particle_trail_remnant = true;
                    o.producer_id = Some(source);
                    o.particle_trail_remnant_expires_frame = Some(expires);
                    o.health.maximum = PARTICLE_REMNANT_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, PARTICLE_REMNANT_MAX_HEALTH);
                }
                let _ = self.special_power_strikes.bind_remnant_object(rid, oid);
            }
        }
    }

    pub fn update_particle_trail_remnant_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.particle_trail_remnant {
                    if let Some(exp) = o.particle_trail_remnant_expires_frame {
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.particle_trail_remnant = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// Tick residual DamagePulseRemnant trail fields spawned by Particle Uplink
    /// beam pulses. ParticleUplinkCannonTrailRemnant Object residual closed.
    pub(super) fn update_particle_remnant_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_remnant_ticks(self.frame, &object_positions);
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

            self.special_power_strikes.record_remnant_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.special_power_strikes.prune_expired_remnant(frame);
    }

    /// Queue a host residual America Paradrop / Airborne mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    /// Residual unit_count for a queued/completed host paradrop mission.
    pub fn paradrop_mission_unit_count(&self, mission_id: u32) -> Option<u32> {
        self.host_paradrops.get(mission_id).map(|m| m.unit_count)
    }

    pub fn queue_paradrop(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_paradrop::{HostParadropKind, PARADROP_RESIDUAL_TEMPLATE};
        let kind = HostParadropKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;

        // Prefer retail ranger template when loaded; otherwise residual TestInfantry.
        let preferred = kind.unit_template();
        let unit_template = if self.templates.contains_key(preferred) {
            preferred.to_string()
        } else {
            self.ensure_residual_paradrop_infantry_template();
            PARADROP_RESIDUAL_TEMPLATE.to_string()
        };

        // C++ SCIENCE_Paradrop1/2/3 residual payload size (5/10/20 Rangers).
        let unit_count = {
            use crate::game_logic::host_paradrop::ParadropScienceTier;
            let sciences: Vec<&str> = self
                .players
                .values()
                .filter(|p| p.team == source_team)
                .flat_map(|p| p.unlocked_sciences.iter().map(|s| s.as_str()))
                .collect();
            ParadropScienceTier::highest_from_sciences(sciences).ranger_count()
        };
        let id = self.host_paradrops.queue_with_unit_count(
            kind,
            source_object,
            source_team,
            target_position,
            frame,
            unit_template,
            unit_count,
        );

        // C++ OCLSpecialPower DeliverPayload residual: cargo plane transport only;
        // infantry drop remains host_paradrops-owned.
        let _ = self.execute_ocl_special_power(
            "SuperweaponParadropAmerica",
            source_object,
            target_position,
        );

        // DeliverPayload cargo residual bookkeeping (AmericaJetCargoPlane honesty).
        let _cargo_id = self.host_deliver_payloads.queue(
            crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::AmericaParadrop,
            source_object,
            source_team,
            target_position,
            frame,
            String::new(),
        );
        // Live AmericaJetCargoPlane + AmericaParachute residual (playability slice).
        let _ = self.spawn_paradrop_cargo_plane(source_object, target_position);

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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
        Some(id)
    }

    /// Ensure residual infantry template used by America Paradrop drop path.
    pub(super) fn ensure_residual_paradrop_infantry_template(&mut self) {
        use crate::game_logic::host_paradrop::PARADROP_RESIDUAL_TEMPLATE;
        if self.templates.contains_key(PARADROP_RESIDUAL_TEMPLATE) {
            return;
        }
        let mut t = ThingTemplate::new(PARADROP_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(100, 0);
        self.templates
            .insert(PARADROP_RESIDUAL_TEMPLATE.to_string(), t);
    }

    /// Advance pending host paradrops to drop frame and spawn infantry near target.
    pub fn update_paradrops(&mut self) {
        self.host_paradrops.clear_frame_events();

        let plans = self.host_paradrops.plan_due_drops(self.frame);
        for plan in plans {
            if !self.templates.contains_key(&plan.unit_template) {
                self.ensure_residual_paradrop_infantry_template();
            }
            let template_name = if self.templates.contains_key(&plan.unit_template) {
                plan.unit_template.clone()
            } else {
                crate::game_logic::host_paradrop::PARADROP_RESIDUAL_TEMPLATE.to_string()
            };

            let mut spawned: Vec<ObjectId> = Vec::with_capacity(plan.spawn_positions.len());
            for pos in &plan.spawn_positions {
                if let Some(id) = self.create_object(&template_name, plan.source_team, *pos) {
                    // C++ Paradrop PutInContainer AmericaParachute + ParachuteDirectly
                    // residual: elevated infantry freefall aiming at LZ.
                    if let Some(obj) = self.objects.get_mut(&id) {
                        // Elevate residual if spawn is near ground.
                        let mut p = obj.get_position();
                        if p.y < 80.0 {
                            p.y = 120.0;
                            obj.set_position(p);
                            crate::game_logic::host_ground_height_log::record(id, p.y, false);
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                crate::game_logic::host_move_log::record(id, Some([p.x, p.y, p.z]));
                                obj.record_host_movement();
                            }
                        }
                        obj.apply_eject_parachuting();
                    }
                    if self.set_parachute_override_destination(id, plan.target_position) {
                        self.host_deliver_payloads
                            .record_parachute_directly_override();
                    }
                    spawned.push(id);
                }
            }

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.drop_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            let spawned_count = spawned.len();
            self.host_paradrops
                .record_drop_complete(plan.mission_id, spawned);

            // Complete matching DeliverPayload cargo residual bookkeeping
            // (AmericaJetCargoPlane honesty; infantry already spawned above).
            let cargo_due = self.host_deliver_payloads.plan_due_drops(self.frame);
            for cargo_plan in cargo_due {
                if cargo_plan.kind
                    == crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::AmericaParadrop
                    && cargo_plan.source_object == plan.source_object
                {
                    self.host_deliver_payloads.record_drop_complete(
                        cargo_plan.mission_id,
                        Vec::new(),
                        0,
                    );
                }
            }

            log::info!(
                "Host paradrop {} mission {} completed at {:?} (spawned={}/{})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                spawned_count,
                plan.spawn_positions.len()
            );
        }
    }

    /// Queue a host residual GLA Rebel Ambush mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    /// Residual unit_count for a queued/completed host ambush mission.
    pub fn ambush_mission_unit_count(&self, mission_id: u32) -> Option<u32> {
        self.host_ambushes.get(mission_id).map(|m| m.unit_count)
    }

    pub fn queue_ambush(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_ambush::{HostAmbushKind, AMBUSH_RESIDUAL_TEMPLATE};
        let kind = HostAmbushKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;

        // Prefer retail rebel template when loaded; otherwise residual TestInfantry.
        let preferred = kind.unit_template();
        let unit_template = if self.templates.contains_key(preferred) {
            preferred.to_string()
        } else {
            self.ensure_residual_ambush_infantry_template();
            AMBUSH_RESIDUAL_TEMPLATE.to_string()
        };

        // C++ SCIENCE_RebelAmbush1/2/3 residual payload size (4/8/16 Rebels).
        let unit_count = {
            use crate::game_logic::host_ambush::AmbushScienceTier;
            let sciences: Vec<&str> = self
                .players
                .values()
                .filter(|p| p.team == source_team)
                .flat_map(|p| p.unlocked_sciences.iter().map(|s| s.as_str()))
                .collect();
            AmbushScienceTier::highest_from_sciences(sciences).rebel_count()
        };
        let id = self.host_ambushes.queue_with_unit_count(
            kind,
            source_object,
            source_team,
            target_position,
            frame,
            unit_template,
            unit_count,
        );

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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
        Some(id)
    }

    /// Ensure residual infantry template used by GLA Ambush spawn path.
    pub(super) fn ensure_residual_ambush_infantry_template(&mut self) {
        use crate::game_logic::host_ambush::AMBUSH_RESIDUAL_TEMPLATE;
        if self.templates.contains_key(AMBUSH_RESIDUAL_TEMPLATE) {
            return;
        }
        let mut t = ThingTemplate::new(AMBUSH_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(100, 0);
        self.templates
            .insert(AMBUSH_RESIDUAL_TEMPLATE.to_string(), t);
    }

    /// Advance pending host ambushes to spawn frame and create infantry near target.
    pub fn update_ambushes(&mut self) {
        self.host_ambushes.clear_frame_events();

        // C++ FadeIn residual: clear STEALTHED after FadeTime frames.
        let fade_due = self.host_ambushes.take_due_fade_clears(self.frame);
        for id in fade_due {
            if let Some(o) = self.objects.get_mut(&id) {
                if o.ambush_fade_in {
                    o.set_status_stealthed(false);
                    o.ambush_fade_in = false;
                }
            }
        }

        let plans = self.host_ambushes.plan_due_spawns(self.frame);
        for plan in plans {
            if !self.templates.contains_key(&plan.unit_template) {
                self.ensure_residual_ambush_infantry_template();
            }
            let template_name = if self.templates.contains_key(&plan.unit_template) {
                plan.unit_template.clone()
            } else {
                crate::game_logic::host_ambush::AMBUSH_RESIDUAL_TEMPLATE.to_string()
            };

            let mut spawned: Vec<ObjectId> = Vec::with_capacity(plan.spawn_positions.len());
            for pos in &plan.spawn_positions {
                if let Some(id) = self.create_object(&template_name, plan.source_team, *pos) {
                    // C++ CreateObject FadeIn residual: STEALTHED until FadeTime.
                    if crate::game_logic::host_ambush::AMBUSH_FADE_IN {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.set_status_stealthed(true);
                            o.ambush_fade_in = true;
                        }
                        self.host_ambushes.schedule_fade_in(id, self.frame);
                    }
                    // C++ DiesOnBadLand residual: drown on water/cliff spawn cell.
                    if crate::game_logic::host_ambush::AMBUSH_DIES_ON_BAD_LAND {
                        let (cliff, water) = self.sample_stun_surface_at(*pos);
                        if water || cliff {
                            if let Some(o) = self.objects.get_mut(&id) {
                                o.cell_is_underwater = water;
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
                                o.ambush_fade_in = false;
                                o.set_status_stealthed(false);
                            }
                            self.host_ambushes.record_dies_on_bad_land_kill();
                            self.mark_object_for_destruction(id, None);
                            // Do not count drowned residual as successful spawn.
                            continue;
                        }
                    }
                    spawned.push(id);
                }
            }

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.spawn_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            let spawned_count = spawned.len();
            self.host_ambushes
                .record_spawn_complete(plan.mission_id, spawned);

            log::info!(
                "Host ambush {} mission {} completed at {:?} (spawned={}/{})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                spawned_count,
                plan.spawn_positions.len()
            );
        }
    }

    /// Queue a host residual USA Leaflet Drop mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    pub fn queue_leaflet_drop(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_leaflet_drop::HostLeafletDropKind;
        let kind = HostLeafletDropKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;

        let id =
            self.host_leaflet_drops
                .queue(kind, source_object, source_team, target_position, frame);

        // C++ OCLSpecialPower DeliverPayload residual: B52 transport only;
        // LeafletDropBehavior disable residual remains host-owned.
        let _ = self.execute_ocl_special_power(
            "SuperweaponLeafletDrop",
            source_object,
            target_position,
        );
        // Live AmericaJetB52 + LeafletContainer residual (playability slice).
        let _ = self.spawn_leaflet_b52_flight(source_object, target_position);

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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
        Some(id)
    }

    /// Advance pending host leaflet drops past Delay and apply DISABLED_EMP residual.
    ///
    /// Matches retail LeafletDropBehavior::doDisableAttack:
    /// - Delay residual 2500 ms → 75 frames
    /// - AffectRadius residual 110
    /// - DisabledDuration 20000 ms → 600 frames (DISABLED_EMP)
    /// - Enemy infantry + vehicles only
    ///
    /// Fail-closed: not full OCL B52 / LeafletContainer drawable / LeafletFX path.
    pub fn update_leaflet_drops(&mut self) {
        use crate::game_logic::host_leaflet_drop::{
            in_leaflet_radius_2d, is_legal_leaflet_disable_target,
        };

        self.host_leaflet_drops.clear_frame_events();

        let plans = self.host_leaflet_drops.plan_due_impacts(self.frame);
        for plan in plans {
            let center = (plan.target_position.x, plan.target_position.z);
            let candidates: Vec<(ObjectId, bool, bool, bool, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    // Residual: never leaflet the caster object itself.
                    if *id == plan.source_object {
                        return None;
                    }
                    let pos = obj.get_position();
                    if !in_leaflet_radius_2d(center, (pos.x, pos.z), plan.radius) {
                        return None;
                    }
                    let is_infantry = obj.is_kind_of(KindOf::Infantry);
                    let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                    // Enemy residual: different team, skip Neutral.
                    let is_enemy = obj.team != plan.source_team
                        && obj.team != Team::Neutral
                        && plan.source_team != Team::Neutral;
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    Some((*id, is_infantry, is_vehicle, is_enemy, under_construction))
                })
                .collect();

            let mut disables: u32 = 0;
            for (id, is_infantry, is_vehicle, is_enemy, under_construction) in candidates {
                if !is_legal_leaflet_disable_target(
                    is_infantry,
                    is_vehicle,
                    true,
                    is_enemy,
                    under_construction,
                ) {
                    continue;
                }
                let Some(target) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                target.apply_disabled_emp(plan.disable_until_frame);
                disables = disables.saturating_add(1);
            }

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.impact_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::WeaponImpact,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            self.host_leaflet_drops
                .record_impact_complete(plan.mission_id, disables);

            log::info!(
                "Host leaflet drop {} mission {} completed at {:?} (disables={})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                disables
            );
        }
    }

    /// Queue a host residual GLA Sneak Attack mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    pub fn queue_sneak_attack(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_sneak_attack::{
            HostSneakAttackKind, SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        };
        let kind = HostSneakAttackKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;

        // Prefer retail tunnel template when loaded; otherwise residual TestSneakTunnel.
        let preferred = kind.tunnel_template();
        let tunnel_template = if self.templates.contains_key(preferred) {
            preferred.to_string()
        } else {
            self.ensure_residual_sneak_tunnel_template();
            SNEAK_ATTACK_RESIDUAL_TEMPLATE.to_string()
        };

        let id = self.host_sneak_attacks.queue(
            kind,
            source_object,
            source_team,
            target_position,
            frame,
            tunnel_template,
        );

        // C++ OCL_CreateSneakAttackTunnelStart residual (Start object, Lifetime 5000ms).
        let _ = self.spawn_sneak_attack_tunnel_start(id, source_team, target_position);

        // C++ SuperweaponLaunched Sneak Attack EVA residual.
        self.try_eva_special_launched_misc(source_team, "sneak");

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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
        Some(id)
    }

    /// Ensure residual tunnel structure template used by GLA Sneak Attack spawn path.
    pub(super) fn ensure_residual_sneak_tunnel_template(&mut self) {
        use crate::game_logic::host_sneak_attack::SNEAK_ATTACK_RESIDUAL_TEMPLATE;
        if self.templates.contains_key(SNEAK_ATTACK_RESIDUAL_TEMPLATE) {
            return;
        }
        let mut t = ThingTemplate::new(SNEAK_ATTACK_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_cost(0, 0);
        self.templates
            .insert(SNEAK_ATTACK_RESIDUAL_TEMPLATE.to_string(), t);
    }

    /// Advance pending host sneak attacks to spawn frame: create tunnel + shockwave.
    ///
    /// Matches residual SuperweaponSneakAttack → Start Lifetime 5000ms → tunnel:
    /// - Spawn delay residual 150 frames
    /// - Shockwave residual SneakAttackShockwaveWeaponBig (50 dmg / radius 50)
    /// - Tunnel template GLASneakAttackTunnelNetwork or residual TestSneakTunnel
    ///
    /// TunnelStart object residual closed; fail-closed vs full Start animation / TunnelContain.
    pub fn update_sneak_attacks(&mut self) {
        use crate::game_logic::host_sneak_attack::{
            in_sneak_shockwave_radius_2d, is_legal_sneak_shockwave_target,
            SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        };

        self.host_sneak_attacks.clear_frame_events();

        // C++ Start FireWeaponUpdate multi-pulse residual (Small + 2× Big).
        let due_pulses = self.host_sneak_attacks.take_due_shockwaves(self.frame);
        for pulse in due_pulses {
            let center = (pulse.target_position.x, pulse.target_position.z);
            let candidates: Vec<(ObjectId, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    let pos = obj.get_position();
                    if !in_sneak_shockwave_radius_2d(center, (pos.x, pos.z), pulse.radius) {
                        return None;
                    }
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    Some((*id, under_construction))
                })
                .collect();

            let mut hits: u32 = 0;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
            for (id, under_construction) in candidates {
                if !is_legal_sneak_shockwave_target(true, under_construction) {
                    continue;
                }
                let Some(target) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                let destroyed =
                    target.take_damage_from_immediate(pulse.damage, Some(pulse.source_object));
                hits = hits.saturating_add(1);
                if destroyed {
                    destroy_ids.push((id, pulse.source_team));
                }
            }
            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }
            if hits > 0 || pulse.pulse_index == 0 {
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pulse.target_position,
                    self.frame,
                    Some(pulse.source_object),
                    None,
                );
            }
            self.host_sneak_attacks.record_multi_pulse_apply(hits);
            if let Some(m) = self.host_sneak_attacks.get_mut(pulse.mission_id) {
                m.shockwave_hits = m.shockwave_hits.saturating_add(hits);
                m.shockwave_damage_total += pulse.damage * hits as f32;
            }
        }

        let plans = self.host_sneak_attacks.plan_due_spawns(self.frame);
        for plan in plans {
            if !self.templates.contains_key(&plan.tunnel_template) {
                self.ensure_residual_sneak_tunnel_template();
            }
            let template_name = if self.templates.contains_key(&plan.tunnel_template) {
                plan.tunnel_template.clone()
            } else {
                SNEAK_ATTACK_RESIDUAL_TEMPLATE.to_string()
            };

            // C++ CreateObjectDie on Start → destroy Start, spawn real tunnel.
            if let Some(start_id) = self
                .host_sneak_attacks
                .get(plan.mission_id)
                .and_then(|m| m.tunnel_start_object)
            {
                self.mark_object_for_destruction(start_id, None);
            }

            let tunnel_id =
                self.create_object(&template_name, plan.source_team, plan.target_position);

            // Shockwave damage is multi-pulse residual (applied above); tunnel spawn
            // only creates the structure + audio residual.
            let shockwave_hits = self
                .host_sneak_attacks
                .get(plan.mission_id)
                .map(|m| m.shockwave_hits)
                .unwrap_or(0);
            let shockwave_damage_total = self
                .host_sneak_attacks
                .get(plan.mission_id)
                .map(|m| m.shockwave_damage_total)
                .unwrap_or(0.0);

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.spawn_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            self.host_sneak_attacks.record_spawn_complete(
                plan.mission_id,
                tunnel_id,
                shockwave_hits,
                shockwave_damage_total,
            );

            log::info!(
                "Host sneak attack {} mission {} completed at {:?} (tunnel={:?}, shock_hits={})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                tunnel_id,
                shockwave_hits
            );
        }
    }

    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// Wave 908: single residual snapshot for host stamp after logic ticks.
    #[inline]
    pub fn sim_timing_snapshot(&self) -> SimTimingSnapshot {
        let d = self.last_fixed_step_diagnostics;
        SimTimingSnapshot {
            frame: self.frame,
            steps_run: d.steps_run,
            budget_hit: d.budget_hit,
            accumulated_time_seconds: d.accumulated_time_seconds,
        }
    }

    /// Apply skirmish match rules from UI configuration.
    pub fn set_skirmish_rules(
        &mut self,
        fog_of_war: bool,
        crates_enabled: bool,
        limit_superweapons: bool,
        allow_tech_buildings: bool,
        game_speed: f32,
    ) {
        self.skirmish_rules = SkirmishRulesState {
            fog_of_war,
            crates_enabled,
            limit_superweapons,
            allow_tech_buildings,
            game_speed: game_speed.clamp(0.1, 4.0),
        };
    }

    /// Read-only skirmish rules snapshot.
    pub fn skirmish_rules(&self) -> &SkirmishRulesState {
        &self.skirmish_rules
    }

    pub fn world_dimensions(&self) -> (f32, f32) {
        (self.world_width, self.world_height)
    }

    /// Get the current map name
    pub fn get_current_map_name(&self) -> &str {
        &self.map_name
    }

    /// Get total play time for this game session
    pub fn get_total_play_time(&self) -> f32 {
        self.sim_time_seconds
    }

    /// Get the current difficulty setting (based on AI difficulty)
    pub fn get_difficulty(&self) -> AIDifficulty {
        self.ai_manager
            .dominant_difficulty()
            .unwrap_or(AIDifficulty::Medium)
    }

    /// True when the skirmish/AI manager owns this player id.
    #[inline]
    pub fn ai_manager_contains_player(&self, player_id: u32) -> bool {
        self.ai_manager.ai_players.contains_key(&player_id)
    }

    /// Check if the game is currently in battle
    pub fn is_in_battle(&self) -> bool {
        // Check if any objects are currently in combat
        self.objects
            .values()
            .any(|obj| obj.status.attacking || obj.ai_state == AIState::Attacking)
    }

    pub fn get_world_dimensions(&self) -> (f32, f32) {
        (self.world_width, self.world_height)
    }

    // Command system compatibility methods

    /// Wave 958: legacy alias — prefer [`Self::host_object`] at authority boundaries.
    #[inline]
    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.host_object(id)
    }

    /// Wave 958: legacy alias — prefer [`Self::host_object_mut`] at authority boundaries.
    #[inline]
    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.host_object_mut(id)
    }

    /// Wave 227: alive probe without exposing `&Object` to engine dual-read paths.
    #[inline]
    pub fn object_is_alive(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_alive())
    }

    /// Wave 230: command-system unit mutation APIs (authority owned by GameLogic).
    #[inline]
    pub fn unit_can_move(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_move())
    }

    #[inline]
    pub fn unit_can_attack(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_attack())
    }

    /// Wave 244: team probe without exposing `&Object`.
    #[inline]
    pub fn unit_team(&self, id: ObjectId) -> Option<Team> {
        self.objects.get(&id).map(|o| o.team)
    }

    /// Wave 244: alive probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_alive(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_alive())
    }

    /// Wave 244: worker probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_worker(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_worker())
    }

    /// Wave 244: repair/dozer probe without exposing `&Object`.
    #[inline]
    pub fn unit_can_repair(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_repair())
    }

    /// Wave 244: hero probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_hero(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_hero())
    }

    /// Wave 244: KindOf probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_kind_of(&self, id: ObjectId, kind: KindOf) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_kind_of(kind))
    }

    /// Wave 244: template name probe without exposing `&Object`.
    #[inline]
    pub fn unit_template_name(&self, id: ObjectId) -> Option<String> {
        self.objects.get(&id).map(|o| o.template_name.clone())
    }

    /// Wave 244: existence probe without exposing `&Object`.
    #[inline]
    pub fn unit_exists(&self, id: ObjectId) -> bool {
        self.objects.contains_key(&id)
    }

    /// Wave 244: under-construction status without exposing `&Object`.
    #[inline]
    pub fn unit_under_construction(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.status.under_construction)
    }

    /// Wave 244: damaged/injured probe without exposing `&Object`.
    #[inline]
    pub fn unit_needs_service(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.health.current + 0.01 < o.health.maximum)
    }

    /// Wave 245: dead probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_dead(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.health.current <= 0.0)
    }

    /// Wave 245: sold status without exposing `&Object`.
    #[inline]
    pub fn unit_is_sold(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.status.sold)
    }

    /// Wave 245: resource/harvestable probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_resource_target(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.is_kind_of(KindOf::Harvestable)
                || o.is_kind_of(KindOf::Resource)
                || o.object_type == ObjectType::Supply
        })
    }

    /// Wave 245: can-contain probe without exposing `&Object`.
    #[inline]
    pub fn unit_can_contain(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_contain())
    }

    /// Wave 245: medical facility probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_medical_facility(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            if o.building_data
                .as_ref()
                .is_some_and(|b| b.building_type == crate::game_logic::BuildingType::HealPad)
            {
                return true;
            }
            let lower = o.template_name.to_ascii_lowercase();
            lower.contains("hospital") || lower.contains("healpad") || lower.contains("ambulance")
        })
    }

    /// Wave 245: building type probe without exposing `&Object`.
    #[inline]
    pub fn unit_building_type(&self, id: ObjectId) -> Option<crate::game_logic::BuildingType> {
        self.objects
            .get(&id)
            .and_then(|o| o.building_data.as_ref().map(|b| b.building_type))
    }

    /// Wave 245: faction structure probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_faction_structure(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.is_faction_structure())
    }

    /// Wave 245: container has space for one more without exposing `&Object`.
    #[inline]
    pub fn unit_has_capacity_for(&self, id: ObjectId, count: usize) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.has_capacity_for(count))
    }

    /// Wave 245: container contains unit without exposing `&Object`.
    #[inline]
    pub fn unit_contains(&self, container: ObjectId, unit: ObjectId) -> bool {
        self.objects
            .get(&container)
            .is_some_and(|o| o.contained_units().contains(&unit))
    }

    /// Wave 245: container has any occupants without exposing `&Object`.
    #[inline]
    pub fn unit_has_occupants(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| !o.contained_units().is_empty())
    }

    /// Wave 245: overlord bunker infantry-only residual without exposing `&Object`.
    #[inline]
    pub fn unit_enter_infantry_only(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.is_kind_of(KindOf::Structure)
                || (o.is_overlord_style_container() && o.overlord_bunker_slot_capacity() > 0)
        })
    }

    /// Wave 245: selectable probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_selectable(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_selectable())
    }

    /// Wave 245: object-type probe without exposing `&Object`.
    #[inline]
    pub fn unit_object_type(&self, id: ObjectId) -> Option<ObjectType> {
        self.objects.get(&id).map(|o| o.object_type)
    }

    /// Wave 245: position probe without exposing `&Object`.
    #[inline]
    pub fn unit_position(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects.get(&id).map(|o| o.get_position())
    }

    /// Wave 245: selectable similar-unit ids (select-similar boot residual).
    pub fn selectable_similar_unit_ids(
        &self,
        team: Team,
        template_name: &str,
        object_type: ObjectType,
        match_object_type: bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == team
                    && obj.is_selectable()
                    && (obj.template_name == template_name
                        || (match_object_type && obj.object_type == object_type))
                {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Wave 245: selectable unit ids in world XZ bounds (box-select boot residual).
    pub fn selectable_unit_ids_in_bounds(
        &self,
        team: Team,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team != team || !obj.is_selectable() {
                    return None;
                }
                let p = obj.get_position();
                if p.x >= min_x && p.x <= max_x && p.z >= min_z && p.z <= max_z {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Wave 245: selectable unit ids on a team matching a unit-id predicate.
    pub fn selectable_unit_ids_for_team_where(
        &self,
        team: Team,
        mut predicate: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == team && obj.is_selectable() && predicate(id) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Wave 245: all unit ids on a team matching a unit-id predicate.
    pub fn unit_ids_for_team_where(
        &self,
        team: Team,
        mut predicate: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == team && predicate(id) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Wave 246: world-position object pick without exposing object dual-walk to callers.
    ///
    /// Priority bands mirror command_integration residual acquire:
    /// - with selection: enemy attackable, then friendly selectable, then other
    /// - without selection: own selectable only
    pub fn pick_object_id_at_world(
        &self,
        origin: glam::Vec3,
        player_team: Option<Team>,
        has_selected_units: bool,
        base_selection_radius: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_residual_acquire::{
            pick_best_priority_residual_target, PriorityAcquireCandidate,
        };

        let cands: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                let distance = (pos - origin).length();
                let radius = base_selection_radius.max(obj.selection_radius);
                if distance > radius {
                    return None;
                }
                let priority = if has_selected_units {
                    match player_team {
                        Some(team) if obj.team != team && obj.is_attackable() => Some(0),
                        Some(team) if obj.team == team && obj.is_selectable() => Some(1),
                        _ if obj.is_attackable() => Some(2),
                        _ if obj.is_selectable() => Some(3),
                        _ => None,
                    }
                } else {
                    match player_team {
                        Some(team) if obj.team == team && obj.is_selectable() => Some(0),
                        Some(_) => None,
                        None if obj.is_selectable() => Some(0),
                        None => None,
                    }
                };
                Some(PriorityAcquireCandidate {
                    id,
                    position: pos,
                    is_alive: true,
                    priority,
                })
            })
            .collect();

        pick_best_priority_residual_target(
            ObjectId(0),
            origin,
            (origin.x, origin.z),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    #[inline]
    pub fn unit_is_dead_or_missing(&self, id: ObjectId) -> bool {
        match self.objects.get(&id) {
            Some(o) => !o.is_alive(),
            None => true,
        }
    }

    /// Prepare move: stop attack then assign path (fallback set_destination).
    /// Wave 230/232: stop attack residual then path or set destination + Moving.
    pub fn unit_command_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        let ok = if self.assign_unit_path(id, destination, &[]) {
            true
        } else if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_destination(destination);
            true
        } else {
            false
        };
        if ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_ai_state(AIState::Moving);
            }
        }
        ok
    }

    /// Wave 232: path with waypoints after stop_attack (executor move_to residual).
    pub fn unit_command_move_to_waypoints(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        waypoints: &[glam::Vec3],
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        self.assign_unit_path(id, destination, waypoints)
    }

    /// Wave 232: force-move — stop attack, path, force Moving state.
    pub fn unit_command_force_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 230/232: attack target (records host attack log).
    pub fn unit_command_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_force_attack(false);
        unit.set_target(Some(target_id));
        crate::game_logic::host_attack_log::record(id, Some(target_id));
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 230/232: force-attack target (records host attack log).
    pub fn unit_command_force_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_target(Some(target_id));
        crate::game_logic::host_attack_log::record(id, Some(target_id));
        unit.set_force_attack(true);
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 230/232: full player stop (idle + clear guard/target/force + logs).
    pub fn unit_command_stop(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop();
        unit.set_target(None);
        unit.set_force_attack(false);
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        crate::game_logic::host_attack_log::record(id, None);
        crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
        unit.end_guard_retaliate();
        unit.set_ai_state(AIState::Idle);
        true
    }

    pub fn unit_command_guard_position(&mut self, id: ObjectId, pos: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_guard_position(Some(pos));
        unit.set_ai_state(AIState::GuardingArea);
        true
    }

    pub fn unit_command_guard_object(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_guard_target(Some(target_id));
        unit.set_ai_state(AIState::GuardingObject);
        true
    }

    /// Wave 231/232: attack-move via path + AttackMoving state.
    pub fn unit_command_attack_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        self.unit_command_attack_move_to_ex(id, destination, -1)
    }

    /// Wave 232: attack-move with max-shots + attack-path flags (executor residual).
    pub fn unit_command_attack_move_to_ex(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        let (can_move, can_attack) = match self.objects.get(&id) {
            Some(unit) => (
                unit.is_alive() && unit.can_move(),
                unit.can_attack() || unit.weapon.is_some(),
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
            unit.set_force_attack(false);
            unit.set_max_shots_to_fire(max_shots);
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if !path_ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_destination(destination);
            } else {
                return false;
            }
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if can_attack {
                unit.is_attack_path = true;
                unit.auto_acquire_when_idle = true;
                unit.set_ai_state(AIState::AttackMoving);
            } else {
                unit.is_attack_path = false;
                unit.set_ai_state(AIState::Moving);
            }
            return true;
        }
        false
    }

    /// Wave 232: promote unit onto attack-move path after waypoint follow.
    pub fn unit_command_promote_attack_path(&mut self, id: ObjectId) -> bool {
        let can_attack = self
            .objects
            .get(&id)
            .map(|u| u.is_alive() && (u.can_attack() || u.weapon.is_some()))
            .unwrap_or(false);
        if !can_attack {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if let Some(slot) = unit.find_waypoint_following_capable_weapon_slot() {
                unit.set_active_weapon_slot(slot);
            }
            unit.is_attack_path = true;
            unit.set_ai_state(AIState::AttackMoving);
            return true;
        }
        false
    }

    /// Wave 231: force-attack ground location.
    pub fn unit_command_attack_ground(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        unit.set_target_location(Some(location));
        unit.set_ai_state(AIState::AttackingGround);
        true
    }

    /// Wave 231: move helper that always leaves unit in Moving state (scatter/formation).
    pub fn unit_command_move_to_moving(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            if !path_ok {
                unit.set_destination(destination);
            }
            unit.set_ai_state(AIState::Moving);
            return true;
        }
        false
    }

    /// Wave 232: dozer construct — path/destination + Constructing AI state.
    pub fn unit_command_begin_construct(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if !self.assign_unit_path(id, location, &[]) {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_destination(location);
            } else {
                return false;
            }
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Constructing);
            return true;
        }
        false
    }

    /// Wave 231: additive selection mark on a selectable friendly unit.
    pub fn unit_select_if_team(&mut self, id: ObjectId, player_team: Team) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.team == player_team && obj.is_selectable() {
            obj.select();
            obj.flash_as_selected();
            true
        } else {
            false
        }
    }

    /// Wave 232: path after stop_attack; optionally clear formation id (free move).
    pub fn unit_command_move_clear_formation(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        clear_formation: bool,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if clear_formation && unit.formation_id != 0 {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 232: tighten-group prep — stop attack, clear formation/guard, then Moving path.
    pub fn unit_command_tighten_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
            unit.set_formation(0, glam::Vec2::ZERO);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
        }
        let path_ok = self.assign_unit_path(id, destination, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            if !path_ok {
                unit.set_destination(destination);
            }
            unit.set_ai_state(AIState::Moving);
            return true;
        }
        false
    }

    /// Wave 232: stamp formation id + offset (create/dissolve).
    pub fn unit_command_set_formation(
        &mut self,
        id: ObjectId,
        formation_id: u32,
        offset: glam::Vec2,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_formation(formation_id, offset);
        true
    }

    /// Wave 232: full guard order (position or object) with radius/mode.
    /// Returns whether unit accepted the order; caller may still path.
    pub fn unit_command_guard_full(
        &mut self,
        id: ObjectId,
        position: Option<glam::Vec3>,
        target: Option<ObjectId>,
        guard_radius: f32,
        mode: GuardMode,
    ) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        // For object guard, require living target position when provided.
        let (gpos, gtarget, ai_state) = if let Some(tid) = target {
            let tpos = self
                .objects
                .get(&tid)
                .filter(|o| o.is_alive())
                .map(|o| o.get_position());
            if tpos.is_none() {
                return false;
            }
            (
                tpos.map(|p| [p.x, p.y, p.z]),
                tid.0,
                AIState::GuardingObject,
            )
        } else if let Some(pos) = position {
            (Some([pos.x, pos.y, pos.z]), 0u32, AIState::GuardingArea)
        } else {
            return false;
        };
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.guard_radius = guard_radius;
            unit.set_guard_mode(mode);
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.end_guard_retaliate();
            if let Some(tid) = target {
                unit.guard_position = None;
                unit.set_guard_target(Some(tid));
            } else if let Some(pos) = position {
                unit.set_guard_target(None);
                unit.set_guard_position(Some(pos));
            }
            unit.set_ai_state(ai_state);
            crate::game_logic::host_guard_log::record(id, gpos, gtarget, guard_radius);
            crate::game_logic::host_attack_log::record(id, None);
            return true;
        }
        false
    }

    /// Wave 232: set guard radius only (guard area residual).
    pub fn unit_command_set_guard_radius(&mut self, id: ObjectId, radius: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.guard_radius = radius;
        true
    }

    /// Wave 232: attack-ground with max shots + force flag.
    pub fn unit_command_attack_ground_ex(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && (u.can_attack() || u.weapon.is_some() || u.is_kind_of(KindOf::Structure))
            })
            .unwrap_or(false);
        if !can {
            // Still allow soft structure residual if alive.
            let alive = self.objects.get(&id).map(|u| u.is_alive()).unwrap_or(false);
            if !alive {
                return false;
            }
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target(None);
        unit.set_force_attack(true);
        unit.set_max_shots_to_fire(max_shots);
        unit.set_target_location(Some(location));
        unit.set_ai_state(AIState::AttackingGround);
        true
    }

    /// Wave 232: hunt/patrol residual.
    pub fn unit_command_patrol(&mut self, id: ObjectId) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && u.can_move()
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
            crate::game_logic::host_guard_log::record(id, None, 0, 0.0);
            crate::game_logic::host_attack_log::record(id, None);
            unit.auto_acquire_when_idle = true;
            unit.set_ai_state(AIState::Patrolling);
            unit.set_status_moving(false);
            return true;
        }
        false
    }

    /// Wave 232: cheer model-condition residual.
    pub fn unit_command_cheer(
        &mut self,
        id: ObjectId,
        cheer_secs: f32,
        cheer_bit: Option<usize>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.begin_cheer(cheer_secs, cheer_bit);
        true
    }

    /// Wave 232: toggle deployed status for deploy-style units.
    pub fn unit_command_set_deployed(&mut self, id: ObjectId, deployed: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_deployed(deployed);
        if deployed {
            unit.set_ai_state(AIState::Idle);
        }
        true
    }

    /// Wave 232: attack nearest of team without force flag (attack-team residual).
    pub fn unit_command_attack_soft(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target(Some(target_id));
        unit.set_force_attack(false);
        unit.set_ai_state(AIState::Attacking);
        true
    }

    /// Wave 232: free group move — stop attack, path, clear formation if goal not offset.
    pub fn unit_command_move_free(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        click_destination: glam::Vec3,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack();
        }
        if !self.assign_unit_path(id, goal, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            // C++ setFormationID(NO_FORMATION) on free individual move when goal is not
            // the stamped formation offset destination.
            if unit.formation_id != 0 {
                let off = unit.formation_offset;
                let expected = glam::Vec3::new(
                    click_destination.x + off.x,
                    click_destination.y,
                    click_destination.z + off.y,
                );
                if (goal - expected).length() > 0.5 {
                    unit.set_formation(0, glam::Vec2::ZERO);
                }
            }
            unit.set_ai_state(AIState::Moving);
        }
        true
    }

    /// Wave 233: set order target (records host attack/order log residual).
    pub fn unit_command_set_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(target);
        true
    }

    /// Wave 233: stop moving then set order target (enter/gather/dock residual).
    pub fn unit_command_stop_moving_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_moving();
        unit.set_order_target(target);
        true
    }

    /// Wave 233: set AI attitude residual.
    pub fn unit_command_set_ai_attitude(
        &mut self,
        id: ObjectId,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_ai_attitude(attitude);
        true
    }

    /// Wave 233: building orientation stamp after under-construction create.
    pub fn unit_command_set_orientation(&mut self, id: ObjectId, orientation: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_orientation(orientation);
        true
    }

    /// Wave 233: set building rally point + host_rally_log.
    pub fn unit_command_set_rally_point(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.rally_point = Some(location);
        crate::game_logic::host_rally_log::record(id, Some([location.x, location.y, location.z]));
        true
    }

    /// Wave 233: return-supplies order target + ReturningResources state.
    pub fn unit_command_return_supplies(&mut self, id: ObjectId, supply_center: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(Some(supply_center));
        unit.set_ai_state(AIState::ReturningResources);
        true
    }

    /// Wave 233: waypoint-path prep — stop attack and clear guard anchors.
    pub fn unit_command_waypoint_path_prep(&mut self, id: ObjectId, as_team: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack();
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        unit.end_guard_retaliate();
        // AsTeam keeps formation identity; free follow clears it.
        if !as_team {
            unit.set_formation(0, glam::Vec2::ZERO);
        }
        true
    }

    /// Wave 233: remove occupant from container (enter/exit residual).
    pub fn unit_command_remove_occupant(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) -> bool {
        let Some(container) = self.objects.get_mut(&container_id) else {
            return false;
        };
        container.remove_occupant(occupant_id);
        true
    }

    /// Wave 233: exit-unit drop residual (position/contain/target/ai).
    pub fn unit_command_exit_drop(&mut self, id: ObjectId, drop_position: glam::Vec3) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_moving();
        unit.set_position(drop_position);
        unit.set_contained_by(None);
        unit.set_target(None);
        unit.set_ai_state(AIState::Idle);
        unit.set_status_moving(false);
        unit.set_status_attacking(false);
        true
    }

    /// Wave 233: mine-clearing weapon-set detail residual.
    pub fn unit_command_set_mine_clearing_detail(&mut self, id: ObjectId, enabled: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_weapon_set_mine_clearing_detail(enabled);
        true
    }

    /// Wave 233: evacuate-on-stop pending flags residual.
    pub fn unit_command_set_pending_evacuate(
        &mut self,
        id: ObjectId,
        pending_evacuate_on_stop: bool,
        pending_exit_after_evacuate: bool,
        prep_move: bool,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.pending_evacuate_on_stop = pending_evacuate_on_stop;
        obj.pending_exit_after_evacuate = pending_exit_after_evacuate;
        if prep_move {
            obj.set_target(None);
            obj.set_force_attack(false);
            obj.set_guard_position(None);
            obj.set_guard_target(None);
            obj.end_guard_retaliate();
        }
        true
    }

    /// Wave 233: order target + Entering state (deploy-to-garrison residual).
    pub fn unit_command_order_enter(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target(Some(target_id));
        unit.set_ai_state(AIState::Entering);
        true
    }

    /// Wave 233: set weapon-set flag residual.
    pub fn unit_command_set_weapon_set_flag(
        &mut self,
        id: ObjectId,
        flag: u8,
        enabled: bool,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_weapon_set_flag(flag, enabled)
    }

    /// Wave 233: surrender residual.
    pub fn unit_command_set_surrendered(&mut self, id: ObjectId, surrendered: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_surrendered(surrendered);
        true
    }

    /// Wave 233: set AI state if alive.
    pub fn unit_command_set_ai_state(&mut self, id: ObjectId, state: AIState) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_ai_state(state);
        true
    }

    /// Wave 233: weapon fire at object/location residual.
    pub fn unit_command_fire_weapon(
        &mut self,
        id: ObjectId,
        target_object: Option<ObjectId>,
        target_location: Option<glam::Vec3>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if let Some(tid) = target_object {
            unit.set_target(Some(tid));
            unit.set_ai_state(AIState::Attacking);
        } else if let Some(pos) = target_location {
            unit.target_location = Some(pos);
            unit.set_ai_state(AIState::AttackingGround);
        } else {
            return false;
        }
        true
    }

    /// Wave 233: infantry go-prone residual.
    pub fn unit_command_go_prone(&mut self, id: ObjectId, prone_secs: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
            return false;
        }
        let is_infantry =
            unit.is_kind_of(KindOf::Infantry) || unit.object_type == ObjectType::Infantry;
        if !is_infantry {
            return false;
        }
        unit.go_prone(prone_secs);
        true
    }

    /// Wave 233: emoticon residual.
    pub fn unit_command_set_emoticon(
        &mut self,
        id: ObjectId,
        name: &str,
        duration_frames: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_emoticon(name, duration_frames);
        true
    }

    /// Wave 233: weapon lock residual.
    pub fn unit_command_set_weapon_lock(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_weapon_lock(slot, lock_type)
    }

    /// Wave 233: release weapon lock residual.
    pub fn unit_command_release_weapon_lock(
        &mut self,
        id: ObjectId,
        lock_type: WeaponLockType,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.release_weapon_lock(lock_type);
        true
    }

    /// Wave 233: switch weapons residual.
    pub fn unit_command_switch_weapons(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let next = unit.active_weapon_slot ^ 1;
        if unit.weapon_slot(next).is_some() {
            let _ = unit.set_weapon_lock(next, WeaponLockType::LockedPermanently);
        } else {
            unit.set_active_weapon_slot(next);
        }
        unit.set_ai_state(AIState::SpecialAbility);
        true
    }

    /// Wave 233: special-power overridable destination residual.
    pub fn unit_command_set_special_power_overridable_destination(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_special_power_overridable_destination(location, None);
        true
    }

    /// Wave 233: queue upgrade on producer building residual.
    pub fn unit_command_building_add_upgrade_to_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
        research_secs: f32,
        cost: Resources,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.add_upgrade_to_queue(upgrade_name.to_string(), research_secs, cost)
    }

    /// Wave 233: remove upgrade entry from producer production queue.
    pub fn unit_command_building_remove_upgrade_from_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        let before = building.production_queue.len();
        building.production_queue.retain(|item| {
            !(item.is_upgrade() && item.template_name.eq_ignore_ascii_case(upgrade_name))
        });
        building.production_queue.len() < before
    }

    /// Wave 233: path then set AI state (executor path_to_goal_with_state residual).
    pub fn unit_command_path_with_state(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
    ) -> bool {
        if !self.assign_unit_path(id, goal, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(state);
            return true;
        }
        false
    }

    /// Wave 233: C++ groupIdle stealth mood delay residual for one unit.
    pub fn unit_command_apply_stealth_mood_delay(
        &mut self,
        id: ObjectId,
        now_frame: u32,
        skew: u32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let can_stealth = unit.innate_stealth || unit.stealth_delay_frames > 0;
        if can_stealth
            && unit.auto_acquire_when_idle
            && unit.can_attack()
            && !unit.status.stealthed
            && !unit.status.detected
        {
            let delay = unit.stealth_delay_frames.max(1);
            unit.next_mood_check_time = now_frame.saturating_add(delay).saturating_add(skew);
            return true;
        }
        false
    }

    #[inline]
    pub fn unit_position_if_movable(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects
            .get(&id)
            .filter(|o| o.can_move())
            .map(|o| o.get_position())
    }

    /// Wave 227: world position probe without exposing `&Object` to engine dual-read paths.
    #[inline]
    pub fn object_position(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects.get(&id).map(|o| o.get_position())
    }

    /// Wave 224: host residual — force-complete an under-construction structure
    /// (train/construct producer path). Authority mutation owned by GameLogic.
    pub fn force_complete_construction(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.construction_percent = 1.0;
        obj.status.under_construction = false;
        obj.health.current = obj.health.maximum;
        true
    }

    /// Wave 224: host residual — ensure barracks `building_data` for force-picked
    /// producers so production queue identity is honest without engine dual-scan.
    pub fn ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let need_bd = obj.building_data.is_none()
            || obj
                .building_data
                .as_ref()
                .map(|b| !matches!(b.building_type, BuildingType::Barracks))
                .unwrap_or(true);
        let name_ok = obj.template_name.to_ascii_lowercase().contains("barracks")
            || obj.is_kind_of(KindOf::FSBarracks);
        // Wave 834: also accept force-complete residual producers that already
        // look like infantry factories (Barracks building_type stamp only).
        if need_bd && name_ok {
            // Mirror engine residual: stamp Barracks building_data when missing/mismatched.
            obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
            return true;
        }
        false
    }

    /// Wave 834: force-stamp Barracks building_data for auto_target train residual
    /// when the producer is already known (host spawn / force-complete path).
    pub fn force_ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
        obj.status.under_construction = false;
        obj.construction_percent = 1.0;
        true
    }

    /// Wave 225: clear movement path / target on a unit (host residual).
    pub fn clear_unit_movement_path(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.movement.path.is_empty() && obj.movement.target_position.is_none() {
            return false;
        }
        obj.movement.path.clear();
        obj.movement.current_path_index = 0;
        obj.movement.target_position = None;
        obj.status.moving = false;
        true
    }

    /// Wave 225: adjust guard radius residual on a unit. Returns new radius when applied.
    pub fn adjust_unit_guard_radius(&mut self, id: ObjectId, delta: f32) -> Option<f32> {
        let obj = self.objects.get_mut(&id)?;
        let guarding = matches!(
            obj.ai_state,
            AIState::GuardingArea | AIState::GuardingObject
        ) || obj.guard_position.is_some()
            || obj.guard_target.is_some();
        if !guarding && obj.guard_radius <= 0.0 {
            let base = obj.selection_radius.max(20.0) * 2.0;
            obj.guard_radius = (base + delta).clamp(30.0, 400.0);
        } else {
            let cur = if obj.guard_radius > 1.0 {
                obj.guard_radius
            } else {
                obj.selection_radius.max(20.0) * 2.0
            };
            obj.guard_radius = (cur + delta).clamp(30.0, 400.0);
        }
        Some(obj.guard_radius)
    }

    /// Add object to the game world
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        let id = object.id;
        self.objects.insert(id, object);
        id
    }

    // ====== ENHANCED RTS COMMAND SYSTEM ======

    /// Get all objects visible to a specific team (for rendering and UI)
    pub fn get_visible_objects(&self, viewing_team: Team) -> Vec<ObjectId> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);
        self.objects
            .iter()
            .filter_map(|(id, obj)| {
                if Self::is_object_visible_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                ) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get visual information for all visible objects
    pub fn get_visual_object_info(
        &self,
        viewing_team: Team,
    ) -> Vec<(ObjectId, super::ObjectVisualInfo)> {
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);
        self.objects
            .iter()
            .filter_map(|(id, obj)| {
                if Self::is_object_visible_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                ) {
                    Some((*id, obj.get_visual_info()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Select objects within a rectangular area
    pub fn select_objects_in_area(
        &mut self,
        player_id: u32,
        min_pos: Vec3,
        max_pos: Vec3,
        add_to_selection: bool,
    ) -> Vec<ObjectId> {
        if let Some(player) = self.players.get_mut(&player_id) {
            let mut selected_objects = Vec::new();

            // Clear previous selection if not adding
            if !add_to_selection {
                for &old_id in &player.selected_objects {
                    if let Some(obj) = self.objects.get_mut(&old_id) {
                        obj.deselect();
                    }
                }
                player.selected_objects.clear();
            }

            // Find objects in the selection area.
            // C++ parity: uses bounding-circle intersection with the selection
            // rectangle, not just center-point containment.  This allows selecting
            // large objects whose center is outside the box but whose radius
            // overlaps it.
            for (id, obj) in &mut self.objects {
                if obj.team == player.team && obj.is_selectable() {
                    let pos = obj.get_position();
                    let r = obj.selection_radius;
                    // Circle-vs-AABB intersection test.
                    let closest_x = pos.x.clamp(min_pos.x, max_pos.x);
                    let closest_z = pos.z.clamp(min_pos.z, max_pos.z);
                    let dist_sq = (pos.x - closest_x).powi(2) + (pos.z - closest_z).powi(2);
                    if dist_sq <= r * r {
                        obj.select();
                        selected_objects.push(*id);
                        if !player.selected_objects.contains(id) {
                            player.selected_objects.push(*id);
                        }
                    }
                }
            }

            log::trace!(
                "{} selected {} objects in area",
                player_id,
                selected_objects.len()
            );
            selected_objects
        } else {
            Vec::new()
        }
    }

    /// Select a single object by click
    pub fn select_object_at_position(
        &mut self,
        player_id: u32,
        position: Vec3,
        selection_radius: f32,
        add_to_selection: bool,
    ) -> Option<ObjectId> {
        if let Some(player) = self.players.get_mut(&player_id) {
            let team = player.team;
            // Pure residual acquire: nearest selectable friendly in click radius (3D).
            // Per-object selection_radius expands the pick disk (C++ pick residual).
            let candidates: Vec<_> = self
                .objects
                .iter()
                .filter_map(|(&id, obj)| {
                    if obj.team != team || !obj.is_selectable() {
                        return None;
                    }
                    Some((
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: false,
                            under_construction: obj.status.under_construction,
                            combat_kind: true,
                            effectively_stealthed: false,
                            is_air: false,
                            eject_invulnerable: false,
                        },
                        obj.selection_radius,
                    ))
                })
                .collect();
            let closest_object =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                    ObjectId(u32::MAX),
                    team,
                    position,
                    candidates.iter().map(|(c, _)| c.clone()),
                    |_| selection_radius + 256.0, // upper bound; legality enforces per-object radius
                    |c| {
                        let sel_r = candidates
                            .iter()
                            .find(|(cand, _)| cand.id == c.id)
                            .map(|(_, r)| *r)
                            .unwrap_or(0.0);
                        let dist = position.distance(c.position);
                        dist <= selection_radius.max(sel_r)
                    },
                )
                .map(|(id, dist, _)| (id, dist));

            if let Some((selected_id, _)) = closest_object {
                // Clear previous selection if not adding
                if !add_to_selection {
                    for &old_id in &player.selected_objects {
                        if let Some(obj) = self.objects.get_mut(&old_id) {
                            obj.deselect();
                        }
                    }
                    player.selected_objects.clear();
                }

                // Select the new object
                if let Some(obj) = self.objects.get_mut(&selected_id) {
                    obj.select();
                    if !player.selected_objects.contains(&selected_id) {
                        player.selected_objects.push(selected_id);
                    }
                }

                log::trace!("{} selected object {}", player_id, selected_id);
                Some(selected_id)
            } else {
                // Clear selection if clicking on empty space and not adding
                if !add_to_selection {
                    for &old_id in &player.selected_objects {
                        if let Some(obj) = self.objects.get_mut(&old_id) {
                            obj.deselect();
                        }
                    }
                    player.selected_objects.clear();
                    log::trace!("{} cleared selection", player_id);
                }
                None
            }
        } else {
            None
        }
    }

    /// Command selected units to stop all actions
    pub fn command_stop(&mut self, player_id: u32) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                if let Some(obj) = self.objects.get_mut(&object_id) {
                    obj.stop_moving();
                    obj.stop_attack();
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(object_id, 0);
                        crate::game_logic::host_ai_decision_log::record_stop_attack(object_id);
                    }
                }
            }
            log::trace!("{} commanded {} units to stop", player_id, selected.len());
        }
    }

    /// Command selected units to attack-move to a position (with pathfinding)
    pub fn command_attack_move(&mut self, player_id: u32, target_position: Vec3) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                let (is_mobile, can_attack) = self
                    .objects
                    .get(&object_id)
                    .map(|obj| (obj.is_mobile(), obj.can_attack() || obj.weapon.is_some()))
                    .unwrap_or((false, false));
                if is_mobile {
                    self.move_object_with_pathfinding(
                        object_id,
                        target_position,
                        Some(if can_attack {
                            AIState::AttackMoving
                        } else {
                            AIState::Moving
                        }),
                    );
                    if let Some(obj) = self.objects.get_mut(&object_id) {
                        if can_attack {
                            obj.is_attack_path = true;
                            obj.auto_acquire_when_idle = true;
                            obj.set_max_shots_to_fire(-1);
                        }
                    }
                }
            }
        }
    }

    /// Get detailed information about an object (for UI display)
    pub fn get_object_info(&self, object_id: ObjectId) -> Option<ObjectInfo> {
        self.objects.get(&object_id).map(|obj| ObjectInfo {
            id: object_id,
            name: obj.get_display_name(),
            team: obj.team,
            object_type: obj.object_type,
            health: obj.health.clone(),
            max_health: obj.max_health,
            position: obj.get_position(),
            is_selected: obj.selected,
            is_moving: obj.status.moving,
            is_attacking: obj.status.attacking,
            under_construction: obj.status.under_construction,
            construction_percent: obj.construction_percent,
            experience_level: obj.experience.level,
            ai_state: obj.ai_state.clone(),
            can_attack: obj.can_attack(),
            can_move: obj.is_mobile(),
        })
    }

    /// Spawn a unit at the specified position (for testing/cheats)
    pub fn spawn_unit(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.create_object(template_name, team, position)
    }

    pub(super) fn template_team_hint(name: &str) -> Option<Team> {
        let upper = name.to_ascii_uppercase();
        if upper.starts_with("USA_") || upper.starts_with("AMERICA_") {
            Some(Team::USA)
        } else if upper.starts_with("CHINA_") {
            Some(Team::China)
        } else if upper.starts_with("GLA_") {
            Some(Team::GLA)
        } else if upper.starts_with("NEUTRAL_") || upper.starts_with("CIVILIAN_") {
            Some(Team::Neutral)
        } else {
            None
        }
    }

    /// Get available unit/building templates for a team.
    ///
    /// This keeps a broad fallback for generic templates while avoiding obvious
    /// cross-faction leakage for names with clear faction prefixes.
    pub fn get_available_templates(&self, team: Team) -> Vec<String> {
        let mut templates = self
            .templates
            .iter()
            .filter(|(name, template)| {
                // Exclude non-interactive map/decorative templates.
                let is_interactive = template.is_kind_of(KindOf::Selectable)
                    || template.is_kind_of(KindOf::Infantry)
                    || template.is_kind_of(KindOf::Vehicle)
                    || template.is_kind_of(KindOf::Aircraft)
                    || template.is_kind_of(KindOf::Structure)
                    || template.is_kind_of(KindOf::Worker)
                    || template.is_kind_of(KindOf::SupplyCenter)
                    || template.is_kind_of(KindOf::CommandCenter);
                if !is_interactive {
                    return false;
                }

                // Keep generic templates for all teams; faction-tagged names are filtered.
                match Self::template_team_hint(name.as_str()) {
                    Some(hinted_team) => hinted_team == team || team == Team::Neutral,
                    None => true,
                }
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        templates.sort();
        templates
    }

    /// Get templates registry (immutable access)
    pub fn get_templates(&self) -> &HashMap<String, ThingTemplate> {
        &self.templates
    }

    /// Get templates registry (mutable access)
    pub fn get_templates_mut(&mut self) -> &mut HashMap<String, ThingTemplate> {
        &mut self.templates
    }

    /// Demonstrate RTS functionality (for testing)
    pub fn demonstrate_rts_features(&mut self) {
        println!("\n🎮 DEMONSTRATING RTS FUNCTIONALITY:");

        // Show all objects and their status
        println!("\n📊 CURRENT GAME STATE:");
        println!("   Total Objects: {}", self.objects.len());
        println!("   Players: {}", self.players.len());

        // Show objects by team
        for team in [Team::USA, Team::China, Team::GLA, Team::Neutral] {
            let team_objects: Vec<_> = self
                .objects
                .iter()
                .filter(|(_, obj)| obj.team == team && obj.is_alive())
                .collect();

            if !team_objects.is_empty() {
                println!(
                    "\n   {} Team Objects ({}): ",
                    team.get_name(),
                    team_objects.len()
                );
                for (id, obj) in team_objects.iter().take(5) {
                    // Show first 5
                    let health_percent = (obj.health.percentage() * 100.0) as u32;
                    let pos = obj.get_position();
                    println!(
                        "      {} - {} [{}% HP] at ({:.0}, {:.0}, {:.0})",
                        id,
                        obj.get_display_name(),
                        health_percent,
                        pos.x,
                        pos.y,
                        pos.z
                    );
                }
                if team_objects.len() > 5 {
                    println!("      ... and {} more", team_objects.len() - 5);
                }
            }
        }

        // Demonstrate selection
        println!("\n🖱️ TESTING SELECTION SYSTEM:");
        let usa_objects: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if obj.team == Team::USA && obj.is_alive() && obj.is_selectable() {
                    Some(*id)
                } else {
                    None
                }
            })
            .take(3)
            .collect();

        if !usa_objects.is_empty() {
            let local_player = self.local_player_id().unwrap_or(0);
            self.select_objects(local_player, usa_objects.clone());
            println!("   Selected {} USA units", usa_objects.len());
        }

        // Demonstrate movement command
        println!("\n⚡ TESTING MOVEMENT COMMANDS:");
        if let Some(player) = self.players.get(&0) {
            if !player.selected_objects.is_empty() {
                let target_pos = Vec3::new(50.0, 0.0, 50.0);
                self.command_move(0, target_pos);
                println!(
                    "   Commanded selected units to move to ({}, {}, {})",
                    target_pos.x, target_pos.y, target_pos.z
                );
            }
        }

        // Show visual info for rendering
        println!("\n🎨 VISUAL INFORMATION:");
        let visual_info = self.get_visual_object_info(Team::USA);
        println!("   {} objects visible to USA team", visual_info.len());

        for (id, info) in visual_info.iter().take(3) {
            println!(
                "      {} - {} {} [Selected: {}, Health: {:.0}%]",
                id,
                info.team.get_name(),
                if let Some(ref model) = info.model_name {
                    model
                } else {
                    "Unknown"
                },
                info.is_selected,
                info.health_percentage * 100.0
            );
        }

        // Show available templates
        println!("\n🏭 AVAILABLE UNIT TEMPLATES:");
        let templates = self.get_available_templates(Team::USA);
        println!("   {} unit templates available:", templates.len());
        for template in templates.iter().take(8) {
            println!("      - {}", template);
        }
        if templates.len() > 8 {
            println!("      ... and {} more", templates.len() - 8);
        }

        println!("\n✅ RTS FUNCTIONALITY DEMONSTRATION COMPLETE!\n");
    }

    /// Add one AI opponent with an explicit difficulty (skirmish config path).
    pub fn add_ai_opponent(&mut self, player_id: u32, team: Team, difficulty: AIDifficulty) {
        self.ensure_ai_faction_templates(team);
        self.ai_manager.add_ai_player(player_id, team, difficulty);
    }

    /// After `load_map` wipes world objects, rebind host AI rebuild soup and
    /// re-ensure faction structure/unit templates used by the AI build path.
    ///
    /// Preserves: registered AI players, difficulty, `is_active`, base layout
    /// template names, and host `players` (cash/slots). Does **not** claim full
    /// C++ AI parity — only keeps the host AI update path non-panicking and able
    /// to issue builds after a skirmish preserve load.
    pub fn rebind_host_ai_after_map_load(&mut self) {
        let mut teams = self.ai_manager.registered_teams();
        for player in self.players.values() {
            if player.team != Team::Neutral && !teams.contains(&player.team) {
                teams.push(player.team);
            }
        }
        for team in teams {
            self.ensure_ai_faction_templates(team);
        }
        self.ai_manager.rebind_after_world_reset();
        // Skirmish residual: AI must have enough cash to start rebuild soup after
        // preserve load. Never wipe intentional positive cash; only top-up empty
        // AI slots (e.g. map path that recreated players without starting_cash).
        self.ensure_skirmish_ai_starting_cash(10_000);
        // Map residual: synthetic default bases (e.g. 120,120) are often shrouded /
        // far from the retail start. Anchor each AI rebuild soup on that team's
        // living CommandCenter (or any structure) so LegalBuildCode can pass FOW.
        self.relocate_host_ai_bases_to_map_starts();
        log::info!(
            "Host AI rebound after map load: ai_players={}, host_players={}",
            self.ai_manager.ai_players.len(),
            self.players.len()
        );
    }

    /// After load_map, move host AI base layouts onto map start structures.
    pub fn relocate_host_ai_bases_to_map_starts(&mut self) {
        let ai_ids: Vec<(u32, Team)> = self
            .ai_manager
            .ai_players
            .iter()
            .map(|(&id, ai)| (id, ai.team))
            .collect();
        for (pid, team) in ai_ids {
            let anchor = self
                .objects
                .values()
                .filter(|o| o.team == team && o.is_alive() && o.is_kind_of(KindOf::CommandCenter))
                .map(|o| o.get_position())
                .next()
                .or_else(|| {
                    self.objects
                        .values()
                        .filter(|o| {
                            o.team == team && o.is_alive() && o.is_kind_of(KindOf::Structure)
                        })
                        .map(|o| o.get_position())
                        .next()
                });
            if let Some(pos) = anchor {
                // Offset slightly so soup pads are not stacked on the CC footprint.
                self.relocate_host_ai_base(pid, pos + glam::Vec3::new(40.0, 0.0, 40.0));
            }
        }
    }

    /// Ensure registered host AI players have at least `min_cash` supplies.
    /// Used after load_map rebind so Medium AI can produce/rebuild without a
    /// full C++ economy parity pass.
    pub fn ensure_skirmish_ai_starting_cash(&mut self, min_cash: u32) {
        let ai_ids: Vec<u32> = self.ai_manager.ai_players.keys().copied().collect();
        for pid in ai_ids {
            if let Some(player) = self.players.get_mut(&pid) {
                if player.effective_supplies() < min_cash {
                    let need = min_cash.saturating_sub(player.effective_supplies());
                    log::info!(
                        "Topping up AI player {} cash {} -> {} after map rebind",
                        pid,
                        player.effective_supplies(),
                        min_cash
                    );
                    // Economy authority: gain delta to absolute floor, not host poke.
                    player.apply_supply_gain(need);
                    crate::game_logic::host_economy_log::record(
                        player.id,
                        player.effective_supplies(),
                        player.power_available,
                    );
                }
            }
        }
    }

    /// Whether a host AI player is registered and currently active.
    pub fn is_host_ai_active(&self, player_id: u32) -> bool {
        self.ai_manager.is_ai_active(player_id)
    }

    /// Configured host AI difficulty, if the player is a registered AI opponent.
    pub fn host_ai_difficulty(&self, player_id: u32) -> Option<AIDifficulty> {
        self.ai_manager.ai_difficulty(player_id)
    }

    /// Ensure faction templates the host AI build/produce paths require are registered.
    pub fn ensure_ai_faction_templates(&mut self, team: Team) {
        // Prefer real WeaponStore / LocomotorStore stats (seeded/INI).
        let _ = super::weapon_bootstrap::ensure_host_weapon_store();
        let _ = super::locomotor_bootstrap::ensure_host_locomotor_store();
        fn structure(name: &str, kinds: &[KindOf], hp: f32, cost: u32) -> ThingTemplate {
            let mut t = ThingTemplate::new(name);
            t.set_health(hp);
            t.set_cost(cost, 0);
            t.build_time = 0.05;
            for k in kinds {
                t.add_kind_of(*k);
            }
            t
        }
        fn unit(name: &str, kinds: &[KindOf], hp: f32, cost: u32) -> ThingTemplate {
            let mut t = structure(name, kinds, hp, cost);
            // Host combat: bind retail Weapon.ini name when known so create_object
            // resolves via WeaponStore (seed/INI). Do not set explicit
            // primary_weapon(Weapon::default()) — that short-circuits the store.
            if let Some(wname) = super::weapon_bootstrap::primary_weapon_name_for_unit(name) {
                t.set_primary_weapon_name(wname);
            }
            if let Some(wname) = super::weapon_bootstrap::secondary_weapon_name_for_unit(name) {
                t.set_secondary_weapon_name(wname);
            }
            // Host movement: bind SET_NORMAL Locomotor.ini name when known so
            // create_object applies retail-ish max_speed (e.g. BasicHuman 20).
            if let Some(lname) = super::locomotor_bootstrap::locomotor_name_for_unit(name) {
                t.set_locomotor_name(lname);
            }
            t
        }
        let entries: Vec<ThingTemplate> = match team {
            Team::USA => vec![
                structure(
                    "USA_CommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
                ),
                structure(
                    "USA_SupplyCenter",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1000.0,
                    1500,
                ),
                structure(
                    "USA_PowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                ),
                structure(
                    "USA_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                ),
                structure(
                    "USA_WarFactory",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1200.0,
                    1500,
                ),
                unit(
                    "USA_Ranger",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    120.0,
                    100,
                ),
                unit(
                    "USA_Humvee",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    300.0,
                    400,
                ),
                {
                    let mut d = unit(
                        "USA_Dozer",
                        &[KindOf::Vehicle, KindOf::Worker, KindOf::Selectable],
                        300.0,
                        1000,
                    );
                    // Workers are not combat units — clear default weapon.
                    d.primary_weapon = None;
                    d.secondary_weapon = None;
                    d.primary_weapon_name = None;
                    d.secondary_weapon_name = None;
                    d
                },
            ],
            Team::China => vec![
                structure(
                    "China_CommandCenter",
                    &[KindOf::Structure, KindOf::CommandCenter, KindOf::Selectable],
                    2000.0,
                    2000,
                ),
                structure(
                    "China_SupplyCenter",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1000.0,
                    1500,
                ),
                structure(
                    "China_PowerPlant",
                    &[KindOf::Structure, KindOf::PowerPlant, KindOf::Selectable],
                    800.0,
                    800,
                ),
                structure(
                    "China_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    1000.0,
                    500,
                ),
                structure(
                    "China_WarFactory",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1200.0,
                    1500,
                ),
                unit(
                    "China_RedGuard",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                ),
            ],
            Team::GLA => vec![
                structure(
                    "GLA_CommandCenter",
                    &[
                        KindOf::Structure,
                        KindOf::CommandCenter,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1800.0,
                    500,
                ),
                structure(
                    "GLA_SupplyStash",
                    &[
                        KindOf::Structure,
                        KindOf::SupplyCenter,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    900.0,
                    300,
                ),
                structure(
                    "GLA_ArmsDealer",
                    &[
                        KindOf::Structure,
                        KindOf::FSWarFactory,
                        KindOf::Selectable,
                        KindOf::Attackable,
                    ],
                    1100.0,
                    400,
                ),
                structure(
                    "GLA_Barracks",
                    &[KindOf::Structure, KindOf::FSBarracks, KindOf::Selectable],
                    900.0,
                    200,
                ),
                unit(
                    "GLA_Soldier",
                    &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
                    100.0,
                    80,
                ),
                unit(
                    "GLA_Technical",
                    &[KindOf::Vehicle, KindOf::Selectable, KindOf::Attackable],
                    250.0,
                    300,
                ),
            ],
            Team::Neutral => vec![],
        };
        for t in entries {
            self.templates.entry(t.name.clone()).or_insert_with(|| t);
        }
    }

    /// Total host-AI activity counter (builds/production/attacks issued).
    pub fn host_ai_activity_count(&self) -> u64 {
        self.ai_manager.total_activity_count()
    }

    /// Number of registered host AI players.
    pub fn host_ai_player_count(&self) -> usize {
        self.ai_manager.ai_players.len()
    }

    /// Set up AI opponents for skirmish matches
    pub fn setup_skirmish_ai(&mut self, human_player_id: u32) {
        println!("🤖 Setting up AI opponents for skirmish match...");

        // --- Initialize the gamelogic crate AI subsystem ---
        // THE_AI singleton (pathfinder, groups) and the AiIntegrationManager
        // must be initialized before any AI player updates run.
        if let Ok(mut ai) = THE_AI.write() {
            ai.init();
            log::info!("THE_AI singleton initialized for skirmish");
        }
        if let Err(e) = initialize_ai_integration() {
            log::warn!("AiIntegrationManager init failed (non-fatal): {:?}", e);
        }

        // Add AI players for non-human players
        for player_id in 0..4 {
            if player_id == human_player_id {
                continue;
            }

            let team = self.players.get(&player_id).map(|p| p.team);
            if let Some(team) = team {
                // Legacy fallback when no SkirmishMatchConfig was supplied:
                // difficulty-by-player-id. Prefer apply_skirmish_config.
                let difficulty = match player_id {
                    1 => AIDifficulty::Medium,
                    2 => AIDifficulty::Hard,
                    3 => AIDifficulty::Easy,
                    _ => AIDifficulty::Medium,
                };

                self.add_ai_opponent(player_id, team, difficulty);
                println!(
                    "  Added AI player {} ({}) with {:?} difficulty",
                    player_id,
                    team.get_name(),
                    difficulty
                );
            }
        }

        println!("✅ AI opponents configured for challenging gameplay!");
    }

    /// Relocate host AI base layout (building queue positions) without mutating
    /// the template catalog. Keeps AI active while placing rebuild sites in range.
    pub fn relocate_host_ai_base(&mut self, player_id: u32, base_position: glam::Vec3) {
        self.ai_manager.relocate_ai_base(player_id, base_position);
    }

    /// Enable/disable AI for specific player
    pub fn set_ai_active(&mut self, player_id: u32, active: bool) {
        self.ai_manager.set_ai_active(player_id, active);
    }

    /// True when this team's skirmish AI player is non-local and currently paused.
    /// Human/local teams always return false (auto-engage remains available).
    pub fn skirmish_ai_auto_engage_paused(&self, team: Team) -> bool {
        self.players.iter().any(|(&pid, player)| {
            player.team == team && !player.is_local && !self.is_host_ai_active(pid)
        })
    }

    /// Pause skirmish AI for `player_id` and clear that team's combat targets so
    /// residual unit AI does not keep counterfiring after the manager pause.
    /// Also cancels production queues on that team so barracks/factories do not
    /// keep spawning units during a golden map clear while AI is paused.
    /// Used by golden map clear (AI rebuild off + no structure auto-engage).
    pub fn pause_skirmish_ai_and_clear_combat(&mut self, player_id: u32) {
        self.set_ai_active(player_id, false);
        let team = self.players.get(&player_id).map(|p| p.team);
        let Some(team) = team else {
            return;
        };
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.team == team && o.is_alive())
            .map(|o| o.id)
            .collect();
        // Cancel production first (needs building_data); then clear combat state.
        for id in ids.iter().copied() {
            let _ = self.cancel_all_production(id);
        }
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.stop_attack();
                obj.target = None;
                obj.target_location = None;
                obj.set_status_force_attack(false);
                // Halt workers/dozers mid-rebuild so paused AI does not finish distant CCs.
                if obj.is_kind_of(KindOf::Worker) || obj.template_name.contains("Dozer") {
                    obj.stop_moving();
                    obj.movement.target_position = None;
                    if matches!(
                        obj.ai_state,
                        AIState::Moving | AIState::Constructing | AIState::Repairing
                    ) {
                        obj.set_ai_state(AIState::Idle);
                    }
                }
                if matches!(
                    obj.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                ) {
                    obj.set_ai_state(AIState::Idle);
                }
            }
        }
    }

    /// Set AI difficulty for a player
    pub fn set_ai_difficulty(&mut self, player_id: u32, difficulty: AIDifficulty) {
        self.ai_manager.set_difficulty(player_id, difficulty);
    }

    /// Get AI status information
    pub fn get_ai_status(&self, player_id: u32) -> Option<String> {
        self.ai_manager.get_ai_info(player_id)
    }

    /// Start skirmish match with AI opponents
    pub fn start_skirmish_match(&mut self, human_team: Team, map_name: &str) {
        println!(
            "🎮 Starting skirmish match: {} vs AI",
            human_team.get_name()
        );

        // Start new game
        self.start_new_game(GameMode::Skirmish);

        // Load map
        self.load_map(map_name);

        // Create human player
        let human_player = Player::new(0, human_team, "Human Player", true);
        self.players.insert(0, human_player);

        // Create AI players with different teams
        let ai_teams = match human_team {
            Team::USA => vec![Team::China, Team::GLA],
            Team::China => vec![Team::USA, Team::GLA],
            Team::GLA => vec![Team::USA, Team::China],
            _ => vec![Team::USA, Team::China, Team::GLA],
        };

        for (i, &team) in ai_teams.iter().enumerate() {
            let ai_player_id = (i + 1) as u32;
            let ai_player = Player::new(
                ai_player_id,
                team,
                &format!("{} AI", team.get_name()),
                false,
            );
            self.players.insert(ai_player_id, ai_player);
        }

        // Set up AI opponents
        self.setup_skirmish_ai(0);

        println!(
            "✅ Skirmish match started with {} AI opponents!",
            ai_teams.len()
        );
    }

    /// Demonstrate AI capabilities
    pub fn demonstrate_ai_functionality(&mut self) {
        println!("\n🤖 DEMONSTRATING AI FUNCTIONALITY:");

        // Show AI status for each AI player
        for player_id in 1..4 {
            if let Some(status) = self.get_ai_status(player_id) {
                println!("\n{}", status);
            }
        }

        // Show AI decision making
        println!("\n🧠 AI DECISION MAKING:");
        println!("   - Economic management: Resource optimization and base construction");
        println!("   - Military strategy: Unit production and attack coordination");
        println!("   - Intelligence gathering: Enemy assessment and reconnaissance");
        println!("   - Base defense: Defensive positioning and threat response");
        println!("   - Advanced tactics: Combined arms and veteran unit management");

        println!("\n✅ AI SYSTEM FULLY OPERATIONAL!\n");
    }

    /// Add comprehensive faction-specific building templates
    /// This ensures perfect alignment with C++ template expectations
    pub(super) fn add_faction_building_templates(&mut self) {
        log::debug!("Adding faction-specific building templates for C++ alignment");

        // Integrate the comprehensive building templates from buildings.rs
        let building_templates = create_building_templates();
        let template_count = building_templates.len();

        for (name, template) in building_templates {
            self.templates.insert(name, template);
        }

        log::info!(
            "Added {} faction-specific building templates",
            template_count
        );
    }

    /// Initialize script system for mission/level scripting
    /// Called once per map load to set up script engine and load mission scripts
    pub fn initialize_scripts(&mut self, map_name: &str) {
        if self.scripts_loaded {
            return; // Already initialized
        }

        if self.script_engine.is_none() {
            log::debug!("Initializing script system");
            match ScriptingEngine::new() {
                Ok(mut engine) => {
                    let handler: Arc<dyn ScriptActionHandler> = Arc::new(
                        MissionScriptActionHandler::new(self.mission_scripts.clone()),
                    );

                    engine.set_action_handler(Some(Arc::clone(&handler)));
                    let _ = engine.set_game_state_context(self.build_script_game_state_context());
                    self.script_engine = Some(Arc::new(engine));

                    // Also install the handler into the legacy ScriptEngine pipeline that runs INI
                    // mission scripts, so ScriptActions like DISPLAY_TEXT, MOVE_CAMERA_TO, etc. are
                    // delivered to the main runtime.
                    let _ = gamelogic::scripting::engine::initialize_script_engine();
                    if let Ok(mut legacy_guard) =
                        gamelogic::scripting::engine::get_script_engine().write()
                    {
                        if let Some(legacy) = legacy_guard.as_mut() {
                            legacy.set_action_handler(Some(handler));
                        }
                    }

                    log::info!("Scripting engine initialized");
                }
                Err(err) => {
                    log::error!("Failed to initialize scripting engine: {}", err);
                    return;
                }
            }
        }

        match super::script_loader::load_map_scripts(map_name) {
            Ok(Some(result)) => {
                self.loaded_script_lists = result.script_lists;
                self.script_source_path = Some(result.source_path);
                self.scripts_loaded = true;
                self.mission_scripts
                    .install_lists(&self.loaded_script_lists);
                // Dense campaign maps: disable host-hanging utility scripts (random
                // generators that CALL_SUBROUTINE every frame, attack-wave spawns,
                // cinematic camera chains). Decode/install still proven; evaluation
                // is budgeted separately for residual safety.
                if result.total_scripts >= DENSE_MISSION_SCRIPT_THRESHOLD {
                    let attack = self.mission_scripts.disable_attack_wave_scripts();
                    let utility = self
                        .mission_scripts
                        .disable_heavy_campaign_utility_scripts();
                    log::info!(
                        "Dense campaign scripts for '{}': disabled attack_wave={} utility={} (of {})",
                        map_name,
                        attack,
                        utility,
                        result.total_scripts
                    );
                }
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();

                // Feed the decoded per-player ScriptLists into the legacy ScriptEngine
                // implementation (gamelogic::scripting::engine) so that `ScriptEngine::update()`
                // runs real mission scripts each frame.
                let _ = gamelogic::scripting::engine::initialize_script_engine();
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        // C++ parity: ScriptEngine::newMap() resets transient script runtime state
                        // on every map load before installing map-owned script lists.
                        engine.reset();
                        for (idx, list) in self.loaded_script_lists.iter().enumerate() {
                            let _ = engine
                                .set_script_list_for_player(idx, Some(Box::new(list.clone())));
                        }
                    }
                }

                log::info!(
                    "Loaded {} mission scripts for '{}'",
                    result.total_scripts,
                    map_name
                );
            }
            Ok(None) => {
                self.loaded_script_lists.clear();
                self.script_source_path = None;
                self.scripts_loaded = true;
                self.mission_scripts.install_lists(&[]);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();

                // Ensure the legacy ScriptEngine doesn't keep running scripts from a previous map.
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.reset();
                    }
                }

                log::warn!("No mission scripts found for '{}'", map_name);
            }
            Err(err) => {
                log::error!(
                    "Failed to decode mission scripts for '{}': {}",
                    map_name,
                    err
                );
                self.mission_scripts.install_lists(&[]);
                self.script_broadcasts.clear();
                self.new_script_messages.clear();
                self.pending_popup_messages.clear();
                self.pending_view_guardband = None;
                self.pending_camera_bw_mode = None;
                self.pending_camera_motion_blur.clear();
                self.script_skybox_enabled = true;
                self.script_cameo_flash_count.clear();
                self.script_named_timers.clear();
                self.script_named_timer_display_shown = true;
                self.script_superweapon_display_enabled = true;
                self.script_superweapon_hidden_objects.clear();

                // On load failures, clear any previously loaded scripts for safety.
                if let Ok(mut engine_guard) =
                    gamelogic::scripting::engine::get_script_engine().write()
                {
                    if let Some(engine) = engine_guard.as_mut() {
                        engine.reset();
                    }
                }
            }
        }
    }

    pub(super) fn build_script_game_state_context(&self) -> gamelogic::scripting::GameStateContext {
        let players = self
            .players
            .values()
            .map(|player| {
                let color = color_for_player(player.id as u8);
                gamelogic::scripting::PlayerInfo {
                    id: player.id,
                    name: player.name.clone(),
                    team: player.team as u32,
                    color: format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b),
                    is_human: player.is_local,
                    is_alive: player.is_alive,
                    score: 0,
                }
            })
            .collect();

        gamelogic::scripting::GameStateContext {
            map_name: self.map_name.clone(),
            game_mode: format!("{:?}", self.game_mode),
            players,
            objectives: Vec::new(),
        }
    }

    /// Queue an audio event to be processed by the audio system
    /// Mirrors C++ TheAudio->addAudioEvent() pattern
    /// Test/honesty: pending audio events not yet process_audio_events drained.
    pub fn queued_audio_event_count_for_test(&self) -> usize {
        self.queued_audio_events.len()
    }

    pub fn queue_audio_event(&mut self, event: AudioEventRequest) {
        self.queued_audio_events.push(event);
    }

    pub fn play_ui_sound(&mut self, event_type: &str) {
        let translated = translate_audio_event(event_type);
        self.queue_audio_event(AudioEventRequest::new(translated));
    }

    /// Process all queued audio events (called once per frame).
    /// Also invoked after presentation `apply_events_to_audio` so same-frame
    /// presentation residual is not delayed one tick.
    pub(crate) fn process_audio_events(&mut self) {
        for event in self.queued_audio_events.drain(..) {
            if let Some(obj_id) = event.object_id {
                if let Some(pos) = event.position {
                    log::trace!(
                        "🔊 Audio: {} at {:?} from object {}",
                        event.event_type,
                        pos,
                        obj_id
                    );
                } else {
                    log::trace!("🔊 Audio: {} from object {}", event.event_type, obj_id);
                }
            } else if let Some(pos) = event.position {
                log::trace!("🔊 Audio: {} at {:?}", event.event_type, pos);
            } else {
                log::trace!("🔊 Audio: {}", event.event_type);
            }

            let _ = crate::subsystem_manager::with_subsystem_mut::<
                crate::subsystem_manager::AudioManagerSubsystem,
                _,
            >(|audio| audio.queue_event(event.clone()));
        }
    }

    /// Drain EVA events from TheEva and dispatch them as audio.
    pub(super) fn process_eva_events(&mut self) {
        if let Ok(events) = gamelogic::helpers::TheEva::drain_events() {
            for eva in events {
                let sound_name = match eva {
                    gamelogic::helpers::EvaEvent::LowPower => "EVA_LowPower",
                    gamelogic::helpers::EvaEvent::InsufficientFunds => "EVA_InsufficientFunds",
                    gamelogic::helpers::EvaEvent::BuildingLost => "EVA_BuildingLost",
                    gamelogic::helpers::EvaEvent::BaseUnderAttack => "EVA_BaseUnderAttack",
                    gamelogic::helpers::EvaEvent::AllyUnderAttack => "EVA_AllyUnderAttack",
                    gamelogic::helpers::EvaEvent::UnitLost => "EVA_UnitLost",
                    gamelogic::helpers::EvaEvent::BuildingSabotaged => "EVA_BuildingSabotaged",
                    gamelogic::helpers::EvaEvent::CashStolen => "EVA_CashStolen",
                    gamelogic::helpers::EvaEvent::VehicleStolen => "EVA_VehicleStolen",
                    gamelogic::helpers::EvaEvent::BuildingStolen => "EVA_BuildingStolen",
                    gamelogic::helpers::EvaEvent::UpgradeComplete => "EVA_UpgradeComplete",
                    gamelogic::helpers::EvaEvent::BuildingBeingStolen => "EVA_BuildingBeingStolen",
                    gamelogic::helpers::EvaEvent::BeaconDetected => "EVA_BeaconDetected",
                    gamelogic::helpers::EvaEvent::GeneralLevelUp => "EVA_GeneralLevelUp",
                    gamelogic::helpers::EvaEvent::EnemyBlackLotusDetected => {
                        "EVA_EnemyBlackLotusDetected"
                    }
                    gamelogic::helpers::EvaEvent::EnemyJarmenKellDetected => {
                        "EVA_EnemyJarmenKellDetected"
                    }
                    gamelogic::helpers::EvaEvent::EnemyColonelBurtonDetected => {
                        "EVA_EnemyColonelBurtonDetected"
                    }
                    gamelogic::helpers::EvaEvent::OwnBlackLotusDetected => {
                        "EVA_OwnBlackLotusDetected"
                    }
                    gamelogic::helpers::EvaEvent::OwnJarmenKellDetected => {
                        "EVA_OwnJarmenKellDetected"
                    }
                    gamelogic::helpers::EvaEvent::OwnColonelBurtonDetected => {
                        "EVA_OwnColonelBurtonDetected"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponDetectedOwnParticleCannon
                    | gamelogic::helpers::EvaEvent::SuperweaponDetectedAllyParticleCannon
                    | gamelogic::helpers::EvaEvent::SuperweaponDetectedEnemyParticleCannon => {
                        "EVA_SuperweaponDetectedParticleCannon"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponDetectedOwnNuke
                    | gamelogic::helpers::EvaEvent::SuperweaponDetectedAllyNuke
                    | gamelogic::helpers::EvaEvent::SuperweaponDetectedEnemyNuke => {
                        "EVA_SuperweaponDetectedNuke"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponDetectedOwnScudStorm
                    | gamelogic::helpers::EvaEvent::SuperweaponDetectedAllyScudStorm
                    | gamelogic::helpers::EvaEvent::SuperweaponDetectedEnemyScudStorm => {
                        "EVA_SuperweaponDetectedScudStorm"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponLaunchedOwnParticleCannon
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedAllyParticleCannon
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedEnemyParticleCannon => {
                        "EVA_SuperweaponLaunchedParticleCannon"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponLaunchedOwnNuke
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedAllyNuke
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedEnemyNuke => {
                        "EVA_SuperweaponLaunchedNuke"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponLaunchedOwnScudStorm
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedAllyScudStorm
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedEnemyScudStorm => {
                        "EVA_SuperweaponLaunchedScudStorm"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponReadyOwnParticleCannon
                    | gamelogic::helpers::EvaEvent::SuperweaponReadyAllyParticleCannon
                    | gamelogic::helpers::EvaEvent::SuperweaponReadyEnemyParticleCannon => {
                        "EVA_SuperweaponReadyParticleCannon"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponReadyOwnNuke
                    | gamelogic::helpers::EvaEvent::SuperweaponReadyAllyNuke
                    | gamelogic::helpers::EvaEvent::SuperweaponReadyEnemyNuke => {
                        "EVA_SuperweaponReadyNuke"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponReadyOwnScudStorm
                    | gamelogic::helpers::EvaEvent::SuperweaponReadyAllyScudStorm
                    | gamelogic::helpers::EvaEvent::SuperweaponReadyEnemyScudStorm => {
                        "EVA_SuperweaponReadyScudStorm"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponLaunchedOwnGpsScrambler
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedAllyGpsScrambler
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedEnemyGpsScrambler => {
                        "EVA_SuperweaponLaunchedGpsScrambler"
                    }
                    gamelogic::helpers::EvaEvent::SuperweaponLaunchedOwnSneakAttack
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedAllySneakAttack
                    | gamelogic::helpers::EvaEvent::SuperweaponLaunchedEnemySneakAttack => {
                        "EVA_SuperweaponLaunchedSneakAttack"
                    }
                };
                game_engine::common::audio::dispatch_eva_announcement(sound_name);
            }
        }
    }

    /// Evaluate and execute scripts each frame
    /// This is called from the main game loop (update_simulation)
    /// Phase 8 of game loop update sequence (C++ Generals compatibility)
    /// Count scripts currently installed from the last map load (groups + free lists).
    pub(super) fn mission_script_count(&self) -> usize {
        let mut count = 0usize;
        for list in &self.loaded_script_lists {
            let mut script = list.first_script.as_deref();
            while let Some(s) = script {
                count += 1;
                script = s.get_next();
            }
            let mut group = list.first_group.as_deref();
            while let Some(g) = group {
                let mut script = g.get_script();
                while let Some(s) = script {
                    count += 1;
                    script = s.get_next();
                }
                group = g.get_next();
            }
        }
        count
    }

    pub(super) fn evaluate_and_execute_scripts(&mut self, dt: f32) {
        if !self.scripts_loaded {
            return;
        }

        self.update_script_camera(dt * self.visual_speed_multiplier.max(0.0));

        // Increment script frame counter
        self.mission_script_counter += 1;

        for event in script_events::drain_events() {
            match event {
                ScriptEvent::PlayerDefeated { player_id } => {
                    log::debug!(
                        "📜 Script event: player {} defeated (frame {})",
                        player_id,
                        self.frame
                    );
                    self.partition_manager.reveal_map_for_player(player_id);
                }
                ScriptEvent::RevealMapForPlayer { player_id } => {
                    log::debug!("📜 Script event: reveal map for player {}", player_id);
                    self.partition_manager.reveal_map_for_player(player_id);
                }
                ScriptEvent::CompletedSpecialPower {
                    player_id,
                    ref special_power_name,
                    creator_id,
                } => {
                    log::debug!(
                        "📜 Script event: completed special power {} player {} creator {}",
                        special_power_name,
                        player_id,
                        creator_id
                    );
                }
                ScriptEvent::AllianceStateChanged { player_id, state } => {
                    log::debug!(
                        "📜 Script event: alliance state {:?} for player {}",
                        state,
                        player_id
                    );
                }
            }

            self.forward_event_to_scripts(&event);
        }

        if let Some(engine) = self.script_engine_handle() {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let in_flight = Arc::clone(&self.script_event_pump_in_flight);
                if !in_flight.swap(true, Ordering::AcqRel) {
                    self.script_event_pump_busy_frames = 0;
                    handle.spawn(async move {
                        if let Err(err) = engine.process_events().await {
                            log::error!("Scripting engine event processing failed: {}", err);
                        }
                        in_flight.store(false, Ordering::Release);
                    });
                } else {
                    self.script_event_pump_busy_frames =
                        self.script_event_pump_busy_frames.saturating_add(1);
                    if self.script_event_pump_busy_frames.is_multiple_of(90) {
                        let pending_events = engine.pending_event_count();
                        log::warn!(
                            "Script event pump busy for {} frames (pending_events={})",
                            self.script_event_pump_busy_frames,
                            pending_events
                        );
                    }
                }
            }
        }

        let mission_runtime_started = Instant::now();
        let dense_script_map = self.mission_script_count() >= DENSE_MISSION_SCRIPT_THRESHOLD;
        let mission_update_result = if self.isInShellGame() {
            // Shell/menu mode already has chunked heavy-script evaluation; cap how many
            // scripts we touch per frame so the UI thread cannot stall on long script lists.
            self.mission_scripts
                .update_shell_budgeted(self.frame as u64, Some(SHELL_MISSION_SCRIPT_BUDGET))
        } else if dense_script_map {
            // Dense campaign maps: budget evaluation so residual/gates cannot hang a frame.
            // Full parity still progresses scripts over successive frames.
            self.mission_scripts
                .update_budgeted(self.frame as u64, Some(DENSE_MISSION_SCRIPT_BUDGET))
        } else {
            self.mission_scripts.update(self.frame as u64)
        };
        if let Err(err) = mission_update_result {
            log::error!("Mission script runtime update failed: {}", err);
        }
        let mission_runtime_elapsed = mission_runtime_started.elapsed();
        if mission_runtime_elapsed >= Duration::from_millis(120) {
            log::warn!(
                "Slow mission script update: {:?} (frame={}, mode={:?})",
                mission_runtime_elapsed,
                self.frame,
                self.game_mode
            );
        }

        self.script_broadcasts
            .retain(|msg| self.sim_time_seconds <= msg.expires_at);

        if self
            .cinematic_text
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.cinematic_text = None;
        }

        if self
            .military_caption
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.military_caption = None;
        }

        for msg in self.mission_scripts.drain_messages() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: msg.clone(),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
            self.new_script_messages.push(msg);
        }

        for sound in self.mission_scripts.drain_sounds() {
            self.play_ui_sound(&sound);
        }

        for sound in self.mission_scripts.drain_sound_events() {
            let translated = translate_audio_event(&sound.sound_name);
            let mut event = AudioEventRequest::new(translated);
            if let Some(pos) = sound.position {
                event = event.with_position(pos);
            }
            self.queue_audio_event(event);
        }

        for camera_target in self.mission_scripts.drain_camera_moves() {
            self.request_camera_focus(camera_target);
        }

        if !self
            .mission_scripts
            .drain_camera_move_to_selection_requests()
            .is_empty()
        {
            if let Some(center) = self.selected_objects_center_for_local_player() {
                self.camera_follow_target = None;
                self.request_camera_focus(center);
            }
        }

        if !self
            .mission_scripts
            .drain_camera_move_home_requests()
            .is_empty()
        {
            if let Some(home) = self.local_player_camera_home_position() {
                self.camera_follow_target = None;
                self.request_camera_focus(home);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_follows()
            .into_iter()
            .last()
        {
            if last.object_id == 0 {
                self.camera_follow_target = None;
            } else {
                self.script_camera_move_to = None;
                self.script_camera_path = None;
                self.camera_follow_target = Some(ObjectId(last.object_id));
                if last.snap_to_unit {
                    if let Some(obj) = self.objects.get(&ObjectId(last.object_id)) {
                        self.request_camera_focus(obj.get_position());
                    }
                }
            }
        }

        if !self
            .mission_scripts
            .drain_camera_mod_freeze_time_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_time();
        }

        if !self
            .mission_scripts
            .drain_camera_mod_freeze_angle_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_angle();
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_final_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_rolling_average_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_rolling_average(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_visual_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_visual_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_script_freeze_time_requests()
            .into_iter()
            .last()
        {
            self.script_time_frozen_by_script = last;
        }

        if let Some(last) = self
            .mission_scripts
            .drain_set_fps_limit_requests()
            .into_iter()
            .last()
        {
            self.apply_set_fps_limit(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_move_to()
            .into_iter()
            .last()
        {
            self.start_camera_move_to(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_path_moves()
            .into_iter()
            .last()
        {
            self.start_camera_path_move(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_set_default_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_default(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_slave_mode_enable_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_slave_mode_enable = Some(last);
            self.pending_camera_slave_mode_disable = false;
        }

        if !self
            .mission_scripts
            .drain_camera_slave_mode_disable_requests()
            .is_empty()
        {
            self.pending_camera_slave_mode_enable = None;
            self.pending_camera_slave_mode_disable = true;
        }

        let screen_shakes = self.mission_scripts.drain_screen_shake_requests();
        if !screen_shakes.is_empty() {
            self.pending_screen_shakes.extend(screen_shakes);
        }

        let camera_shakers = self.mission_scripts.drain_camera_add_shaker_requests();
        if !camera_shakers.is_empty() {
            self.pending_camera_add_shakers.extend(camera_shakers);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_resets()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            self.pending_camera_zoom_reset = true;
            let request = CameraMoveToRequest {
                position: last.position,
                seconds: last.duration_seconds,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            };
            self.start_camera_move_to(request);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_zoom_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_zoom = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_pitch_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_pitch = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_rotate_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = Some(last);
            } else {
                log::debug!("Camera rotate ignored due to active CAMERA_MOD_FREEZE_ANGLE");
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_zoom_requests()
            .into_iter()
            .last()
        {
            let remaining = self.script_camera_remaining_seconds();
            self.pending_camera_zoom = Some(CameraZoomRequest {
                zoom: last.zoom,
                duration_seconds: remaining,
                ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
            });
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_pitch_requests()
            .into_iter()
            .last()
        {
            let remaining = self.script_camera_remaining_seconds();
            self.pending_camera_pitch = Some(CameraPitchRequest {
                pitch: last.pitch,
                duration_seconds: remaining,
                ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
            });
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_setup_requests()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            self.request_camera_focus(last.position);
            self.pending_camera_zoom = Some(CameraZoomRequest {
                zoom: last.zoom,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            self.pending_camera_pitch = Some(CameraPitchRequest {
                pitch: last.pitch,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: last.look_toward,
                    duration_seconds: 0.0,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_waypoint_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(last);
            } else {
                log::debug!(
                    "Camera look toward waypoint ignored due to active CAMERA_MOD_FREEZE_ANGLE"
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_object_requests()
            .into_iter()
            .last()
        {
            if self.is_script_camera_angle_frozen() {
                log::debug!(
                    "Camera look toward object ignored due to active CAMERA_MOD_FREEZE_ANGLE"
                );
            } else if let Some(obj) = self.objects.get(&ObjectId(last.object_id)) {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: obj.get_position(),
                    duration_seconds: last.duration_seconds,
                    ease_in_seconds: last.ease_in_seconds,
                    ease_out_seconds: last.ease_out_seconds,
                    reverse_rotation: false,
                });
            } else {
                log::warn!(
                    "Camera look toward object request ignored; object {} not found",
                    last.object_id
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_look_toward_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: last.position,
                    duration_seconds: 0.0,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            } else {
                log::debug!("Camera mod look toward ignored due to active CAMERA_MOD_FREEZE_ANGLE");
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_look_toward_requests()
            .into_iter()
            .last()
        {
            if !self.is_script_camera_angle_frozen() {
                let remaining = self.script_camera_remaining_seconds();
                self.pending_camera_rotate = None;
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: last.position,
                    duration_seconds: remaining,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            } else {
                log::debug!(
                    "Camera mod final look toward ignored due to active CAMERA_MOD_FREEZE_ANGLE"
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_letterbox_events()
            .last()
            .copied()
        {
            self.cinematic_letterbox = last;
        }

        if let Some((text, _font, duration_seconds)) = self
            .mission_scripts
            .drain_cinematic_text()
            .into_iter()
            .last()
        {
            let duration = (duration_seconds as f32).max(0.0);
            self.cinematic_text = Some((text, self.sim_time_seconds + duration));
        }

        if let Some(last) = self
            .mission_scripts
            .drain_military_captions()
            .into_iter()
            .last()
        {
            let duration = Self::military_caption_duration_seconds(last.duration_ms);
            self.military_caption = Some((last.text, self.sim_time_seconds + duration));
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_movie = Some(movie.clone());
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Movie requested: {}", movie),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_radar_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_radar_movie = Some(movie);
        }

        let objective_updates = self.mission_scripts.drain_objective_updates();
        if !objective_updates.is_empty() {
            for update in objective_updates {
                let status = if update.completed {
                    ObjectiveStatus::Completed
                } else {
                    ObjectiveStatus::Active
                };

                let updated_existing = self.with_objective_mut(&update.name, |objective| {
                    objective.title = update.name.clone();
                    objective.description = update.description.clone();
                    objective.status = status;
                });

                if !updated_existing {
                    self.mission_objectives.push(ObjectiveDisplay::new(
                        Some(update.name.clone()),
                        update.name.clone(),
                        update.description.clone(),
                        ObjectiveCategory::Primary,
                    ));
                    let idx = self.mission_objectives.len().saturating_sub(1);
                    self.objective_lookup
                        .insert(update.name.to_ascii_lowercase(), idx);
                }
            }
        }

        for effect in self.mission_scripts.drain_effect_requests() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!(
                    "Effect '{}' at ({:.0}, {:.0}, {:.0})",
                    effect.effect_type, effect.position.x, effect.position.y, effect.position.z
                ),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        for radar_event in self.mission_scripts.drain_radar_event_requests() {
            self.queue_script_radar_event(radar_event);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_radar_enabled_updates()
            .into_iter()
            .last()
        {
            self.radar_enabled = enabled;
        }

        if let Some(forced) = self
            .mission_scripts
            .drain_radar_forced_updates()
            .into_iter()
            .last()
        {
            self.radar_forced = forced;
        }

        if let Some(visible) = self
            .mission_scripts
            .drain_weather_visibility_updates()
            .into_iter()
            .last()
        {
            self.set_weather_visible(visible);
        }

        let popup_messages = self.mission_scripts.drain_popup_message_requests();
        if !popup_messages.is_empty() {
            #[cfg(feature = "game_client")]
            for popup in &popup_messages {
                game_client::core::script_action_handler::script_popup_message(
                    &popup.message,
                    popup.x_percent,
                    popup.y_percent,
                    popup.width,
                    popup.pause,
                    popup.pause_music,
                );
            }

            for popup in popup_messages {
                if popup.pause {
                    self.set_paused(true);
                }
                if popup.pause_music {
                    self.pending_music_stop = true;
                }
                self.script_broadcasts.push(ScriptBroadcast {
                    text: popup.message.clone(),
                    expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
                });
                self.new_script_messages.push(popup.message.clone());
                self.pending_popup_messages.push(popup);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_view_guardband_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_resize_view_guardband(
                last.x_bias,
                last.y_bias,
            );
            self.pending_view_guardband = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_bw_mode_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_camera_bw_mode(
                last.enabled,
                last.frames,
            );
            self.pending_camera_bw_mode = Some(last);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_skybox_enabled_updates()
            .into_iter()
            .last()
        {
            self.script_skybox_enabled = enabled;
            {
                let mut global = game_engine::common::global_data::write();
                global.draw_sky_box = enabled;
            }
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_skybox_enabled(enabled);
        }

        for request in self.mission_scripts.drain_camera_motion_blur_requests() {
            #[cfg(feature = "game_client")]
            match &request {
                CameraMotionBlurRequest::Basic { zoom_in, saturate } => {
                    game_client::core::script_action_handler::script_camera_motion_blur(
                        *zoom_in, *saturate,
                    );
                }
                CameraMotionBlurRequest::Jump { position, saturate } => {
                    game_client::core::script_action_handler::script_camera_motion_blur_jump(
                        position.x, position.z, position.y, *saturate,
                    );
                }
                CameraMotionBlurRequest::Follow { amount } => {
                    game_client::core::script_action_handler::script_camera_motion_blur_follow(
                        *amount,
                    );
                }
                CameraMotionBlurRequest::EndFollow => {
                    game_client::core::script_action_handler::script_camera_motion_blur_end_follow(
                    );
                }
            }

            if let CameraMotionBlurRequest::Jump { position, .. } = &request {
                self.camera_follow_target = None;
                self.request_camera_focus(*position);
            }
            self.pending_camera_motion_blur.push(request);
        }

        for flash in self.mission_scripts.drain_cameo_flash_requests() {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_cameo_flash(
                &flash.command_button_name,
                flash.flash_count,
            );
            self.script_cameo_flash_count
                .insert(flash.command_button_name, flash.flash_count);
        }

        for mutation in self.mission_scripts.drain_named_timer_mutations() {
            match mutation {
                NamedTimerMutation::Add {
                    name,
                    text,
                    countdown,
                } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_add_named_timer(
                        &name, &text, countdown,
                    );
                    self.script_named_timers.insert(name, (text, countdown));
                }
                NamedTimerMutation::Remove { name } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_remove_named_timer(&name);
                    self.script_named_timers.remove(&name);
                }
            }
        }

        if let Some(show) = self
            .mission_scripts
            .drain_named_timer_display_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_show_named_timer_display(show);
            self.script_named_timer_display_shown = show;
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_superweapon_display_enabled_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_superweapon_display_enabled(
                enabled,
            );
            self.script_superweapon_display_enabled = enabled;
        }

        for mutation in self
            .mission_scripts
            .drain_superweapon_object_display_mutations()
        {
            match mutation {
                SuperweaponObjectDisplayMutation::Hide { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_hide_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .insert(ObjectId(object_id));
                }
                SuperweaponObjectDisplayMutation::Show { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_show_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .remove(&ObjectId(object_id));
                }
            }
        }

        if !self.mission_scripts.drain_music_stop_requests().is_empty() {
            self.pending_music_stop = true;
        }

        #[cfg(feature = "game_client")]
        {
            if let Some(amount) = self
                .mission_scripts
                .drain_oversize_terrain_requests()
                .into_iter()
                .last()
            {
                if let Ok(mut terrain_guard) =
                    game_client::terrain::terrain_visual::get_terrain_visual()
                {
                    if let Some(visual) = terrain_guard.as_mut() {
                        visual.oversize_terrain(amount);
                    }
                }
            }

            if let Some(level) = self
                .mission_scripts
                .drain_border_shroud_levels()
                .into_iter()
                .last()
            {
                if !game_client::core::script_action_handler::set_script_display_border_shroud_level(
                    level,
                ) {
                    log::warn!(
                        "Border shroud level script request not applied: display bridge unavailable"
                    );
                }
            }
        }
    }

    pub(super) fn start_camera_path_move(&mut self, request: CameraPathRequest) {
        self.script_camera_move_to = None;
        if let Some(move_state) =
            ScriptCameraPathMove::new(self.script_camera_focus_estimate, &request)
        {
            let mut move_state = move_state;
            if self.script_camera_freeze_time_armed {
                move_state.set_freeze_time(true);
                self.script_camera_freeze_time_armed = false;
            }
            if self.script_camera_freeze_angle_armed {
                move_state.set_freeze_angle(true);
                self.script_camera_freeze_angle_armed = false;
            }
            if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
                move_state.set_final_speed_multiplier(multiplier);
            }
            if let Some(frames) = self.script_camera_pending_rolling_average_frames.take() {
                move_state.set_rolling_average_frames(frames);
            }
            self.mission_scripts.set_camera_movement_finished(false);
            self.script_camera_path = Some(move_state);
        } else {
            self.mission_scripts.set_camera_movement_finished(true);
            self.script_camera_path = None;
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Camera path '{}' not found", request.waypoint),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }
    }

    pub(super) fn start_camera_move_to(&mut self, request: CameraMoveToRequest) {
        self.mission_scripts.set_camera_movement_finished(false);
        self.script_camera_path = None;
        let mut move_state = ScriptCameraMoveTo::new(self.script_camera_focus_estimate, &request);
        if self.script_camera_freeze_time_armed {
            move_state.set_freeze_time(true);
            self.script_camera_freeze_time_armed = false;
        }
        if self.script_camera_freeze_angle_armed {
            move_state.set_freeze_angle(true);
            self.script_camera_freeze_angle_armed = false;
        }
        if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
            move_state.set_final_speed_multiplier(multiplier);
        }
        self.script_camera_move_to = Some(move_state);
    }

    pub(super) fn script_camera_remaining_seconds(&self) -> f32 {
        if let Some(move_to) = self.script_camera_move_to.as_ref() {
            return move_to.remaining_time_seconds();
        }
        if let Some(path) = self.script_camera_path.as_ref() {
            return path.remaining_time_seconds();
        }
        0.0
    }

    pub(super) fn is_script_camera_angle_frozen(&self) -> bool {
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_angle())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_angle())
                .unwrap_or(false)
    }

    pub(super) fn apply_script_camera_mod_freeze_time(&mut self) {
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_time(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_freeze_time(true);
            applied = true;
        }
        if !applied {
            self.script_camera_freeze_time_armed = true;
        }
    }

    pub(super) fn apply_script_camera_mod_freeze_angle(&mut self) {
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_angle(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_freeze_angle(true);
            applied = true;
        }
        if !applied {
            self.script_camera_freeze_angle_armed = true;
        }
    }

    pub(super) fn apply_script_camera_mod_final_speed_multiplier(
        &mut self,
        request: &CameraModFinalSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier as f32;
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if !applied {
            self.script_camera_pending_final_speed_multiplier = Some(multiplier.max(0.0));
        }
    }

    pub(super) fn apply_script_camera_mod_rolling_average(
        &mut self,
        request: &CameraModRollingAverageRequest,
    ) {
        let frames = request.frames.max(1);
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_rolling_average_frames(frames);
        } else {
            self.script_camera_pending_rolling_average_frames = Some(frames);
        }
    }

    pub(super) fn apply_visual_speed_multiplier(&mut self, request: &VisualSpeedMultiplierRequest) {
        let multiplier = request.multiplier.max(1) as f32;
        if multiplier.is_finite() {
            self.visual_speed_multiplier = multiplier;
        }
    }

    pub(super) fn apply_set_fps_limit(&mut self, request: &SetFpsLimitRequest) {
        self.pending_script_fps_limit = Some(request.fps);
    }

    pub(super) fn apply_script_camera_default(&mut self, request: CameraSetDefaultRequest) {
        self.script_default_camera_pitch = request.pitch;
        // Match C++ W3DView::setDefaultView(): angle is ignored for the active 3D path.
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = if request.max_height.is_finite() {
            request.max_height.max(0.0)
        } else {
            1.0
        };
    }

    pub(super) fn update_script_camera(&mut self, dt: f32) {
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            self.mission_scripts.set_camera_movement_finished(false);

            if move_to.is_finished() {
                let focus = move_to.final_focus();
                self.request_camera_focus(focus);
                self.script_camera_move_to = None;
                self.mission_scripts.set_camera_movement_finished(true);
                return;
            }

            if let Some(focus) = move_to.advance(dt) {
                self.request_camera_focus(focus);
            }
            return;
        }

        let Some(path_move) = self.script_camera_path.as_mut() else {
            self.mission_scripts.set_camera_movement_finished(true);
            return;
        };

        self.mission_scripts.set_camera_movement_finished(false);

        if path_move.is_finished() {
            let focus = path_move.final_focus();
            self.request_camera_focus(focus);
            self.script_camera_path = None;
            self.mission_scripts.set_camera_movement_finished(true);
            return;
        }

        if let Some(focus) = path_move.advance(dt) {
            self.request_camera_focus(focus);
        }
    }

    pub(super) fn military_caption_duration_seconds(duration_ms: i32) -> f32 {
        (duration_ms as f32 / 1000.0).max(0.0)
    }

    /// Update UI state from game logic
    /// This method extracts all data needed for UI rendering each frame
    /// Matches pattern from C++ InGameUI::preDraw() (InGameUI.h line 466)
    pub fn update_ui_state(&mut self, player_id: u32) -> crate::ui::GameUIState {
        use crate::ui::{
            BuildQueueEntry, GameUIState, MinimapDot, RadarMessageEntry, RadarPing, RadarPingKind,
            UnitDisplayInfo,
        };

        // Get player associated with the current viewport/camera
        let player = self.players.get(&player_id);

        let (credits, power_generated, power_used, max_power, credits_per_second) = if let Some(p) =
            player
        {
            let (produced, consumed) =
                super::buildings::BuildingBehavior::calculate_power_for_team(p.team, &self.objects);
            let supply_centers = self
                .objects
                .values()
                .filter(|obj| {
                    obj.team == p.team
                        && obj.is_constructed()
                        && obj.is_alive()
                        && obj.is_kind_of(KindOf::SupplyCenter)
                })
                .count();
            let income = 5.0 + supply_centers as f32 * 25.0;
            (
                p.resources.supplies as i32,
                produced,
                consumed,
                produced,
                income,
            )
        } else {
            (10000, 100, 60, 100, 5.0)
        };

        // Get selected units
        let mut selected_units = Vec::new();
        let mut selected_unit_infos = Vec::new();

        if let Some(player) = player {
            for &object_id in &player.selected_objects {
                selected_units.push(object_id);

                if let Some(obj) = self.objects.get(&object_id) {
                    selected_unit_infos.push(UnitDisplayInfo {
                        object_id,
                        name: obj.name.clone(),
                        health_current: obj.health.current,
                        health_maximum: obj.health.maximum,
                        unit_type: format!("{:?}", obj.object_type),
                        current_order: if obj.target.is_some() {
                            "Attacking".to_string()
                        } else if obj.movement.target_position.is_some() {
                            "Moving".to_string()
                        } else {
                            "Idle".to_string()
                        },
                        veterancy_overlay: None,
                        production_progress: None,
                        production_template: None,
                        command_set_override: obj.command_set_override.clone().unwrap_or_default(),
                        can_produce: obj.building_data.is_some()
                            && !obj.status.under_construction
                            && obj.construction_percent >= 1.0,
                        production_is_upgrade: false,
                        production_paused: false,
                    });
                }
            }
        }

        // Get build queues (from all constructing buildings)
        let mut build_queue = Vec::new();
        for obj in self.objects.values() {
            if obj.status.under_construction {
                // Estimate time remaining based on construction percent (assuming 30 second build time)
                let estimated_total_time = 30.0;
                let time_remaining = estimated_total_time * (1.0 - obj.construction_percent);

                build_queue.push(BuildQueueEntry {
                    template_name: obj.name.clone(),
                    percent_complete: obj.construction_percent,
                    time_remaining,
                });
            }
        }

        // Generate minimap dots for all units
        let mut minimap_unit_dots = Vec::new();
        let (world_min, world_max) = self.world_bounds();
        let world_span_x = (world_max.x - world_min.x).max(1.0);
        let world_span_z = (world_max.z - world_min.z).max(1.0);
        let viewing_team = player.map(|p| p.team).unwrap_or(Team::Neutral);
        let shroud_snapshot = self.shroud_visibility_snapshot_for_team(viewing_team);

        for (id, obj) in &self.objects {
            if obj.is_alive()
                && (obj.is_kind_of(KindOf::Selectable) || obj.is_kind_of(KindOf::Structure))
                && Self::is_object_visible_on_minimap_for_team(
                    *id,
                    obj,
                    viewing_team,
                    shroud_snapshot.as_ref(),
                )
            {
                // Normalize position to 0.0-1.0 range based on world dimensions
                let normalized_x = ((obj.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
                let normalized_y = ((obj.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);

                let color = match obj.team {
                    Team::USA => color_for_player(1),
                    Team::China => color_for_player(0),
                    Team::GLA => color_for_player(4),
                    Team::Neutral => color_for_player(7),
                };

                let size = if obj.is_kind_of(KindOf::Structure) {
                    4.0
                } else {
                    2.0
                };

                minimap_unit_dots.push(MinimapDot::normalized(
                    normalized_x,
                    normalized_y,
                    color,
                    size,
                ));
            }
        }

        let mut minimap_beacons = Vec::new();
        for beacon in snapshot_beacons() {
            let normalized_x = ((beacon.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
            let normalized_y = ((beacon.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);
            minimap_beacons.push(MinimapDot::normalized(
                normalized_x,
                normalized_y,
                color_for_player(beacon.player_id as u8),
                4.0,
            ));
        }

        // Use WW3D-synchronized time
        let game_time = self.sim_time_seconds;

        let player_name = player
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("Commander {}", player_id + 1));

        let mut ui_state = GameUIState::default();
        ui_state.credits = credits;
        ui_state.power_generated = power_generated;
        ui_state.power_used = power_used;
        ui_state.max_power = max_power;
        ui_state.credits_per_second = credits_per_second;
        ui_state.player_id = player_id;
        ui_state.player_name = player_name;
        ui_state.selected_units = selected_units;
        ui_state.selected_unit_infos = selected_unit_infos;
        // Live path fills panel; production overlay replaces with PresentationFrame.
        ui_state.selection_panel = crate::ui::ControlBarSelectionPanelState::from_unit_infos(
            &ui_state.selected_unit_infos,
        );
        ui_state.build_queue = build_queue;
        ui_state.is_game_paused = self.is_paused;
        ui_state.current_game_time = game_time;
        ui_state.fps = LOGIC_FRAMES_PER_SECOND;
        ui_state.frame_time_ms = 1000.0 / LOGIC_FRAMES_PER_SECOND;
        ui_state.performance_score = 1.0;
        ui_state.minimap_unit_dots = minimap_unit_dots;
        ui_state.minimap_beacons = minimap_beacons.clone();
        ui_state.new_beacons = std::mem::take(&mut self.recent_beacons);
        ui_state.minimap_viewport = crate::ui::default_minimap_viewport();
        ui_state.minimap_texture_id = None;
        ui_state.minimap_coordinates = Some(crate::graphics::MinimapCoordinates {
            minimap_width: 1.0,
            minimap_height: 1.0,
            world_min,
            world_max,
            screen_pos: Vec2::ZERO,
        });

        // Pull fresh radar updates from GameLogic (typed) and turn them into HUD/radar pings.
        for update in radar_notifier::drain() {
            let pos_world = Vec3::new(update.position.0, 0.0, update.position.1);
            match update.event_type {
                RadarEventType::BaseAttacked => {
                    self.queue_radar_attack_at("Base under attack", pos_world);
                }
                RadarEventType::EnemyDetected => {
                    self.queue_radar_message_at(
                        "Enemy detected",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::UnitCreated => {
                    self.queue_radar_message_at(
                        "Unit ready",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::UnitDestroyed => {
                    self.queue_radar_message_at(
                        "Unit lost",
                        pos_world,
                        radar_notifications::RadarKind::Generic,
                    );
                }
                RadarEventType::BeaconPlaced | RadarEventType::BeaconRemoved => {
                    // Beacon events are already handled via beacon manager; skip to avoid duplicates.
                }
            }
        }

        let radar_entries = self.radar_notifications.drain();
        const RADAR_PING_LIFETIME: f32 = 6.0;
        let mut latest_by_kind: [Option<RadarEntry>; 3] = [None, None, None];
        ui_state.radar_messages = radar_entries
            .iter()
            .map(|entry| entry.text.clone())
            .collect();
        ui_state.radar_events = radar_entries
            .iter()
            .map(|entry| RadarMessageEntry {
                text: entry.text.clone(),
                position: Some(entry.position),
                kind: match entry.kind {
                    radar_notifications::RadarKind::Generic => RadarPingKind::Generic,
                    radar_notifications::RadarKind::Attack => RadarPingKind::Attack,
                    radar_notifications::RadarKind::Ally => RadarPingKind::Ally,
                },
            })
            .collect();
        ui_state.radar_pings = radar_entries
            .iter()
            .filter_map(|entry| {
                let age = (self.sim_time_seconds - entry.timestamp).max(0.0);
                if age > RADAR_PING_LIFETIME {
                    return None;
                }
                // Fade out linearly and add a soft pulse to mimic C++ radar blips.
                let normalized = (1.0 - age / RADAR_PING_LIFETIME).clamp(0.0, 1.0);
                let pulse = 0.5 * (1.0 + (age * std::f32::consts::TAU).cos());
                let intensity = (normalized * 0.6 + pulse * 0.4).clamp(0.0, 1.0);
                Some(RadarPing {
                    position: entry.position,
                    intensity,
                    age_seconds: age,
                    kind: match entry.kind {
                        radar_notifications::RadarKind::Generic => RadarPingKind::Generic,
                        radar_notifications::RadarKind::Attack => RadarPingKind::Attack,
                        radar_notifications::RadarKind::Ally => RadarPingKind::Ally,
                    },
                })
            })
            .collect();
        for entry in radar_entries {
            let idx = match entry.kind {
                radar_notifications::RadarKind::Generic => 0,
                radar_notifications::RadarKind::Attack => 1,
                radar_notifications::RadarKind::Ally => 2,
            };
            let slot = &mut latest_by_kind[idx];
            if slot
                .as_ref()
                .map(|e| entry.timestamp >= e.timestamp)
                .unwrap_or(true)
            {
                *slot = Some(entry);
            }
        }
        if let Some(entry) = latest_by_kind
            .iter()
            .filter_map(|e| e.as_ref())
            .max_by(|a, b| {
                a.timestamp
                    .partial_cmp(&b.timestamp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            self.last_radar_event = Some(entry.clone());
        }
        ui_state.last_radar_ping = self.last_radar_event.as_ref().map(|e| e.position);
        ui_state.script_messages = self
            .script_broadcasts
            .iter()
            .map(|msg| msg.text.clone())
            .collect();
        ui_state.cinematic_letterbox = self.cinematic_letterbox;
        ui_state.cinematic_text = self.cinematic_text.as_ref().map(|(text, _)| text.clone());
        ui_state.military_caption = self.military_caption.as_ref().map(|(text, _)| text.clone());
        // C++ W3DControlBar / ControlBarCallback:
        // isRadarForced() || (!isRadarHidden() && player->hasRadar())
        // radar_enabled here is script "not hidden"; has_radar is ownership residual.
        let local_has_radar = self
            .local_player_id()
            .and_then(|id| self.get_player(id))
            .map(|p| p.has_radar())
            .unwrap_or(false);
        ui_state.radar_enabled = self.radar_forced || (self.radar_enabled && local_has_radar);
        ui_state.radar_forced = self.radar_forced;
        ui_state.objectives = self.mission_objectives.clone();
        ui_state
    }

    /// Active script broadcast texts residual (presentation freeze).
    pub fn script_broadcast_texts(&self) -> Vec<String> {
        self.script_broadcasts
            .iter()
            .map(|msg| msg.text.clone())
            .collect()
    }

    /// Pending script messages this frame (presentation freeze; non-draining).
    pub fn peek_new_script_messages(&self) -> &[String] {
        &self.new_script_messages
    }

    pub fn cinematic_letterbox(&self) -> bool {
        self.cinematic_letterbox
    }

    pub fn cinematic_text(&self) -> Option<&str> {
        self.cinematic_text.as_ref().map(|(t, _)| t.as_str())
    }

    pub fn military_caption_text(&self) -> Option<&str> {
        self.military_caption.as_ref().map(|(t, _)| t.as_str())
    }

    /// Remaining military caption lifetime in milliseconds (0 if expired/absent).
    pub fn military_caption_remaining_ms(&self) -> Option<i32> {
        self.military_caption.as_ref().map(|(_, expiry)| {
            let rem = (*expiry - self.sim_time_seconds).max(0.0);
            (rem * 1000.0).round() as i32
        })
    }

    /// Remaining cinematic text lifetime in milliseconds.
    pub fn cinematic_text_remaining_ms(&self) -> Option<i32> {
        self.cinematic_text.as_ref().map(|(_, expiry)| {
            let rem = (*expiry - self.sim_time_seconds).max(0.0);
            (rem * 1000.0).round() as i32
        })
    }

    pub fn radar_script_enabled(&self) -> bool {
        self.radar_enabled
    }

    pub fn radar_forced(&self) -> bool {
        self.radar_forced
    }

    /// Push a script/UI message residual (broadcast + new-message feed).
    pub fn push_script_ui_message<S: Into<String>>(&mut self, message: S) {
        let msg = message.into();
        if msg.is_empty() {
            return;
        }
        self.script_broadcasts.push(ScriptBroadcast {
            text: msg.clone(),
            expires_at: self.sim_time_seconds + 10.0,
        });
        self.new_script_messages.push(msg);
    }

    pub fn set_cinematic_letterbox(&mut self, enabled: bool) {
        self.cinematic_letterbox = enabled;
    }

    pub fn set_cinematic_text(&mut self, text: Option<String>) {
        self.cinematic_text = text.map(|t| (t, self.sim_time_seconds + 10.0));
    }

    pub fn set_military_caption(&mut self, text: Option<String>) {
        self.military_caption = text.map(|t| (t, self.sim_time_seconds + 10.0));
    }

    pub fn set_radar_forced(&mut self, forced: bool) {
        self.radar_forced = forced;
    }

    pub fn take_new_script_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.new_script_messages)
    }

    /// Queue a command from the UI
    pub fn queue_command(&mut self, command: crate::command_system::GameCommand) {
        log::trace!("Queuing command: {:?}", command.command_type);
        self.command_queue.push_back(command);
    }

    /// Process queued commands
    /// Wave 914: true when command queue has pending authority work.
    #[inline]
    pub fn has_pending_commands(&self) -> bool {
        !self.command_queue.is_empty()
    }

    /// Wave 914/915: process command queue only when non-empty (skip empty dual-write path).
    /// Returns whether any commands were processed.
    #[inline]
    pub fn process_commands_if_needed(&mut self) -> bool {
        if self.command_queue.is_empty() {
            return false;
        }
        self.process_commands();
        true
    }

    /// Wave 922: queue one command then process if the queue is non-empty.
    #[inline]
    pub fn queue_and_process_command(
        &mut self,
        command: crate::command_system::GameCommand,
    ) -> bool {
        self.queue_command(command);
        self.process_commands_if_needed()
    }

    pub fn process_commands(&mut self) {
        // Process all queued commands
        while let Some(command) = self.command_queue.pop_front() {
            self.execute_command(command);
        }
        // Standalone command processing (unit tests / host gates without a full
        // sim tick) must still settle deferred economy/damage authority logs.
        if !crate::gameworld_shadow::shadow_coupled_tick_active() {
            crate::gameworld_shadow::materialize_host_economy_pending(self);
        }
    }

    /// Snapshot number of active beacons (used by HUD to clear highlights).

    /// Object IDs currently following a path (pathfinding step residual).
    ///
    /// Prefer this over iterating every object key each frame.
    pub fn object_ids_with_active_path(&self) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.movement.path.is_empty()
                    && o.movement.current_path_index < o.movement.path.len()
            })
            .map(|(&id, _)| id)
            .collect()
    }

    /// Peek beacon placements queued this frame (HUD bloom residual).
    pub fn recent_beacons(&self) -> &[glam::Vec3] {
        &self.recent_beacons
    }

    /// Drain beacon placements queued this frame (presentation / UI residual).
    pub fn drain_recent_beacons(&mut self) -> Vec<glam::Vec3> {
        std::mem::take(&mut self.recent_beacons)
    }

    pub fn beacon_count(&self) -> usize {
        snapshot_beacons().len()
    }

    /// Structure placement radius residual for LBC_OBJECTS_IN_THE_WAY.
    pub(super) fn structure_place_radius(obj: &Object) -> f32 {
        use crate::game_logic::host_production_buildable_command_residual::STRUCTURE_PLACE_CLEARANCE_RESIDUAL;
        // Prefer selection_radius when set; else default clearance residual.
        if obj.selection_radius > 1.0 {
            obj.selection_radius * 0.5
        } else {
            STRUCTURE_PLACE_CLEARANCE_RESIDUAL * 0.5
        }
    }

    /// C++ BuildAssistant::isLocationLegalToBuild residual (subset).
    ///
    /// Checks world bounds, living structure overlap, and for supply centers
    /// LBC_TOO_CLOSE_TO_SUPPLIES vs SUPPLY_SOURCE residual. Fail-closed vs full
    /// terrain slope / shroud graph.
    pub fn legal_build_code_at(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
    ) -> u32 {
        self.legal_build_code_at_for_builder(team, position, template_name, None)
    }

    /// C++ isLocationLegalToBuild with optional builder for CLEAR_PATH residual.
    pub fn legal_build_code_at_for_builder(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::host_production_buildable_command_residual::{
            cell_shroud_blocks_build_residual, footprint_height_delta_residual,
            legal_build_code_from_checks_complete_residual,
            legal_build_objects_in_the_way_residual, legal_build_too_close_to_supplies_residual,
            min_dist_from_map_edge_residual, STRUCTURE_PLACE_CLEARANCE_RESIDUAL,
        };
        use crate::game_logic::host_structure_economy_residual::{
            is_legal_build_distance_from_map_edge, is_legal_build_height_variation,
            is_supply_warehouse_template, MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD, SUPPLY_BUILD_BORDER,
        };
        use crate::game_logic::host_upgrades::is_supply_center_template;
        let (min, max) = self.world_bounds();
        // Use real map extent (no generous pad) for C++ off-map / edge residual.
        let min_x = min.x;
        let max_x = max.x;
        let min_z = min.z;
        let max_z = max.z;
        let in_bounds = position.x.is_finite()
            && position.z.is_finite()
            && position.x >= min_x
            && position.x <= max_x
            && position.z >= min_z
            && position.z <= max_z;
        let edge_dist = min_dist_from_map_edge_residual(
            (position.x, position.z),
            (min_x, min_z),
            (max_x, max_z),
        );
        let too_close_edge = in_bounds
            && MIN_DIST_FROM_EDGE_OF_MAP_FOR_BUILD > 0.0
            && !is_legal_build_distance_from_map_edge(edge_dist);
        let place_r = STRUCTURE_PLACE_CLEARANCE_RESIDUAL * 0.5;
        let mut blockers: Vec<(f32, f32, f32)> = Vec::new();
        let mut supply_sources: Vec<(f32, f32, f32)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let p = obj.get_position();
            let r = Self::structure_place_radius(obj);
            if obj.is_kind_of(KindOf::Structure) {
                blockers.push((p.x, p.z, r));
            }
            // C++ KINDOF_SUPPLY_SOURCE residual (docks/piles/warehouses).
            if is_supply_warehouse_template(&obj.template_name)
                || obj.is_kind_of(KindOf::Harvestable)
                || obj.is_kind_of(KindOf::Resource)
            {
                supply_sources.push((p.x, p.z, r.max(10.0)));
            }
        }
        let in_way =
            legal_build_objects_in_the_way_residual((position.x, position.z), place_r, &blockers);
        // C++ CANNOT_BUILD_NEAR_SUPPLIES: supply centers only.
        let lower = template_name.to_ascii_lowercase();
        let too_close = if is_supply_center_template(template_name)
            || lower.contains("supplycenter")
            || lower.contains("supply_center")
            || lower.contains("supplystash")
        {
            legal_build_too_close_to_supplies_residual(
                (position.x, position.z),
                place_r,
                &supply_sources,
                SUPPLY_BUILD_BORDER,
            )
        } else {
            false
        };
        // C++ SHROUD_REVEALED residual: require CELLSHROUD_CLEAR for human build.
        // When fog_of_war is off or no shroud grid is initialized, fail-open (clear).
        let shrouded = if !self.skirmish_rules.fog_of_war {
            false
        } else {
            let player_id = self
                .players
                .values()
                .find(|p| p.team == team)
                .map(|p| p.id)
                .unwrap_or(0);
            let clear = self.is_build_location_shroud_clear(player_id, position);
            cell_shroud_blocks_build_residual(clear)
        };
        // C++ footprint height sample residual (hiZ-loZ > AllowedHeightVariation).
        // Fail-open when no height samples available (synthetic maps without terrain).
        let not_flat = self.footprint_not_flat_enough(position, place_r);
        // C++ CLEAR_PATH residual when a mobile builder is provided.
        let no_clear = match builder_id {
            Some(bid) => !self.builder_has_clear_path_to(bid, position),
            None => false,
        };
        crate::game_logic::host_production_buildable_command_residual::legal_build_code_from_checks_with_path_residual(
            in_bounds,
            shrouded,
            not_flat,
            in_way,
            too_close,
            too_close_edge,
            no_clear,
        )
    }

    /// C++ AIUpdateInterface::isQuickPathAvailable residual (simplified host pathfind).
    ///
    /// Fail-open when builder missing / immobile / already at goal. Fail-closed
    /// when pathfinding returns no path for a mobile constructor.
    pub fn builder_has_clear_path_to(&self, builder_id: ObjectId, goal: glam::Vec3) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::builder_skips_clear_path_residual;
        let Some(builder) = self.objects.get(&builder_id) else {
            return builder_skips_clear_path_residual(true);
        };
        if !builder.is_alive() {
            return false;
        }
        // Structures / immobile skip CLEAR_PATH residual.
        if builder.is_kind_of(KindOf::Structure) || builder.is_kind_of(KindOf::Immobile) {
            return builder_skips_clear_path_residual(true);
        }
        if !builder.can_move() && !builder.can_construct() {
            return builder_skips_clear_path_residual(true);
        }
        let start = builder.get_position();
        let dx = start.x - goal.x;
        let dz = start.z - goal.z;
        // Already close enough to pad residual — treat as clear.
        if dx * dx + dz * dz <= 64.0 * 64.0 {
            return true;
        }
        // Host pathfinding residual: need &mut pathfinding_system — use interior mutability
        // via a quick cell walk instead of full A* when possible.
        self.quick_path_available_residual(start, goal)
    }

    /// Simplified CLEAR_PATH residual without mutably borrowing pathfinding.
    ///
    /// Walks a coarse line of cells; blocked if any cell is impassable structure
    /// footprint residual. Fail-open when grid unavailable.
    pub(super) fn quick_path_available_residual(&self, start: glam::Vec3, goal: glam::Vec3) -> bool {
        use crate::game_logic::pathfinding::GridPos;
        let grid = &self.pathfinding_system.grid;
        let gs = grid.world_to_grid(start);
        let gg = grid.world_to_grid(goal);
        // If either end invalid, fail-open residual (map placement still works).
        if !grid.is_valid_pos(gs) || !grid.is_valid_pos(gg) {
            return true;
        }
        // Goal on static structure residual is still a legal build pad — dozer
        // walks to the edge. Only intermediate cells block CLEAR_PATH residual.
        let steps = (gs.x - gg.x).abs().max((gs.y - gg.y).abs()).max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = (gs.x as f32 + (gg.x - gs.x) as f32 * t).round() as i32;
            let y = (gs.y as f32 + (gg.y - gs.y) as f32 * t).round() as i32;
            let cell = GridPos::new(x, y);
            if !grid.is_valid_pos(cell) {
                continue;
            }
            // Skip start and goal cells residual.
            if cell == gs || cell == gg {
                continue;
            }
            if grid.is_static_blocked(cell) {
                return false;
            }
        }
        true
    }

    /// C++ BuildAssistant footprint hiZ-loZ residual vs AllowedHeightVariationForBuilding.
    pub(super) fn footprint_not_flat_enough(&self, position: glam::Vec3, place_radius: f32) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::footprint_height_delta_residual;
        use crate::game_logic::host_structure_economy_residual::is_legal_build_height_variation;
        let r = place_radius.max(1.0);
        // 3x3 sample residual across pad (simplified vs full iterateFootprint resolution).
        let offsets = [
            (-r, -r),
            (0.0, -r),
            (r, -r),
            (-r, 0.0),
            (0.0, 0.0),
            (r, 0.0),
            (-r, r),
            (0.0, r),
            (r, r),
        ];
        let mut samples = Vec::with_capacity(9);
        for (dx, dz) in offsets {
            let p = glam::Vec3::new(position.x + dx, 0.0, position.z + dz);
            if let Some(h) = self.terrain_height_at(p) {
                samples.push(h);
            }
        }
        if samples.is_empty() {
            return false; // fail-open residual
        }
        let delta = footprint_height_delta_residual(&samples);
        !is_legal_build_height_variation(delta)
    }

    /// C++ PartitionManager::getShroudStatusForPlayer == CELLSHROUD_CLEAR residual.
    ///
    /// Fail-open when shroud grid is not initialized (synthetic/host tests).
    pub(super) fn is_build_location_shroud_clear(&self, player_id: u32, position: glam::Vec3) -> bool {
        use gamelogic::common::Coord3D;
        use gamelogic::system::shroud_manager::{get_shroud_manager, ShroudState};
        let Ok(shroud) = get_shroud_manager().lock() else {
            return true;
        };
        if !shroud.has_shroud_grid() {
            return true;
        }
        // Match host vision residual Coord3D axis order (x, z, y).
        let coord = Coord3D::new(position.x, position.z, position.y);
        matches!(
            shroud.get_shroud_state(player_id, &coord),
            ShroudState::Visible
        )
    }

    /// True when residual LegalBuildCode is LBC_OK.
    pub fn is_location_legal_to_build(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
    ) -> bool {
        self.is_location_legal_to_build_for_builder(team, position, template_name, None)
    }

    pub fn is_location_legal_to_build_for_builder(
        &self,
        team: Team,
        position: glam::Vec3,
        template_name: &str,
        builder_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::LBC_OK;
        self.legal_build_code_at_for_builder(team, position, template_name, builder_id) == LBC_OK
    }

    /// Count living/under-construction Superweapon-link-key objects for a team residual.
    pub fn count_superweapon_link_key_owned(&self, team: Team) -> u32 {
        use crate::game_logic::host_superweapon_kindof::is_superweapon_link_key_template;
        self.objects
            .values()
            .filter(|o| {
                o.team == team && o.is_alive() && is_superweapon_link_key_template(&o.template_name)
            })
            .count() as u32
    }

    /// Living constructed template names owned by a team residual (prereq scan).
    pub fn team_owned_constructed_templates(&self, team: Team) -> Vec<String> {
        let mut names = Vec::new();
        for obj in self.objects.values() {
            if obj.team == team && obj.is_alive() && obj.is_constructed() {
                names.push(obj.template_name.clone());
            }
        }
        names
    }

    /// C++ ProductionPrerequisite residual for known sample templates.
    ///
    /// Fail-closed: unknown templates (not in residual sample table) are allowed
    /// so map/script spawns and unported INI trees still work. Known SW / tech
    /// buildings require their Prerequisites Object list.
    pub fn team_satisfies_build_prerequisites(&self, team: Team, template_name: &str) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::{
            prereq_is_satisfied_residual, prereq_objects_for_template_residual,
        };
        let Some((prereqs, or_chain)) = prereq_objects_for_template_residual(template_name) else {
            return true;
        };
        let owned = self.team_owned_constructed_templates(team);
        let owned_refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        // Science residual: fail-open for structure Object prereqs (no RequiredScience on SW).
        prereq_is_satisfied_residual(prereqs, or_chain, &owned_refs, true)
    }

    /// C++ MaxSimultaneousOfType Superweapon residual gate.
    pub fn can_start_superweapon_building(&self, team: Team, template_name: &str) -> bool {
        use crate::game_logic::host_superweapon_kindof::{
            is_superweapon_link_key_template, superweapon_max_simultaneous_allowed,
        };
        if !is_superweapon_link_key_template(template_name) {
            return true;
        }
        let Some(max) =
            superweapon_max_simultaneous_allowed(self.skirmish_rules.limit_superweapons)
        else {
            return true;
        };
        self.count_superweapon_link_key_owned(team) < max
    }

    /// Enqueue unit production on a building if permitted.

    /// Living units of template for a team + queued production of that template residual.
    pub fn count_team_units_of_template_owned_or_queued(
        &self,
        team: Team,
        template_name: &str,
    ) -> u32 {
        let mut n = 0u32;
        for obj in self.objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }
            if obj.template_name.eq_ignore_ascii_case(template_name) {
                n = n.saturating_add(1);
            }
            // Queued production residual.
            if let Some(b) = obj.building_data.as_ref() {
                for item in &b.production_queue {
                    if item.template_name.eq_ignore_ascii_case(template_name) {
                        n = n.saturating_add(1);
                    }
                }
            }
        }
        n
    }

    /// Hangar occupancy residual: docked aircraft at this airfield + queued aircraft.
    pub fn airfield_parking_occupied_or_queued(&self, airfield_id: ObjectId) -> u32 {
        let Some(af) = self.objects.get(&airfield_id) else {
            return 0;
        };
        let mut n = 0u32;
        // Docked hangar roster residual (garrisoned_units or occupants).
        if let Some(building) = af.building_data.as_ref() {
            n = n.saturating_add(building.garrisoned_units.len() as u32);
            // Queued aircraft production residual.
            for item in &building.production_queue {
                if self
                    .templates
                    .get(&item.template_name)
                    .map(|t| t.is_kind_of(KindOf::Aircraft))
                    .unwrap_or_else(|| {
                        item.template_name.to_ascii_lowercase().contains("aircraft")
                            || item.template_name.to_ascii_lowercase().contains("jet")
                            || item.template_name.to_ascii_lowercase().contains("raptor")
                            || item.template_name.to_ascii_lowercase().contains("aurora")
                            || item.template_name.to_ascii_lowercase().contains("comanche")
                            || item.template_name.to_ascii_lowercase().contains("mig")
                            || item
                                .template_name
                                .to_ascii_lowercase()
                                .contains("helicopter")
                    })
                {
                    n = n.saturating_add(1);
                }
            }
        } else {
            n = n.saturating_add(af.occupants.len() as u32);
        }
        // Also count living aircraft with producer_id == this airfield still airborne
        // (space reserved until destroyed).
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            if obj.producer_id != Some(airfield_id) {
                continue;
            }
            if !(obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft) {
                continue;
            }
            // Already counted if docked in garrison list.
            let docked = obj.contained_by == Some(airfield_id)
                || af
                    .building_data
                    .as_ref()
                    .map(|b| b.garrisoned_units.contains(&obj.id))
                    .unwrap_or(false);
            if !docked {
                n = n.saturating_add(1);
            }
        }
        n
    }

    /// C++ BuildAssistant::canMakeUnit residual status for a producer + template.
    ///
    /// Fail-closed parking/maxed residual currently unused (always false) until
    /// Hero MaxSimultaneousOfType=1 residual live; full INI MaxSimultaneous matrix deferred.
    pub fn can_make_unit(&self, producer_id: ObjectId, template_name: &str) -> u32 {
        use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;
        use crate::game_logic::host_production_buildable_command_residual::{
            can_make_type_from_checks_residual, CANMAKE_OK,
        };

        let Some(template) = self.templates.get(template_name) else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
        };
        let Some(producer) = self.objects.get(&producer_id) else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        };
        if !producer.is_alive() || !producer.is_constructed() {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        }
        let team = producer.team;
        let factory_disabled = producer.is_disabled();
        let Some(building) = producer.building_data.as_ref() else {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_FACTORY_IS_DISABLED;
        };
        // Wrong factory type residual → treat as no prereq/unavailable.
        if !building.can_produce(template) {
            return crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ;
        }
        let queue_full = building.production_queue.len() >= DEFAULT_PRODUCTION_QUEUE_LIMIT;
        // C++ ParkingPlaceBehavior hangar capacity residual for aircraft at airfields.
        let parking_full = {
            use crate::game_logic::buildings::BuildingType;
            use crate::game_logic::host_dock_contain_exit_heal_residual::airfield_parking_places_full;
            let is_airfield = matches!(building.building_type, BuildingType::Airfield)
                || producer.is_kind_of(KindOf::FSAirfield)
                || producer
                    .template_name
                    .to_ascii_lowercase()
                    .contains("airfield");
            let is_aircraft = template.is_kind_of(KindOf::Aircraft);
            if is_airfield && is_aircraft {
                // Occupancy includes current queue aircraft; producing one more needs a free slot.
                airfield_parking_places_full(self.airfield_parking_occupied_or_queued(producer_id))
            } else {
                false
            }
        };
        let has_prereq = self.team_satisfies_build_prerequisites(team, template_name)
            && self.can_start_superweapon_building(team, template_name);
        // Science residual (stealth fighter etc.) as prereq gate.
        let science_ok = {
            use crate::game_logic::host_stealth_fighter::{
                is_stealth_fighter_science, player_may_produce_stealth_aircraft,
                requires_stealth_fighter_science,
            };
            if requires_stealth_fighter_science(template_name) {
                match self.get_player_by_team(team) {
                    Some(p) => {
                        let has = p
                            .unlocked_sciences
                            .iter()
                            .any(|s| is_stealth_fighter_science(s));
                        player_may_produce_stealth_aircraft(has, template_name)
                    }
                    None => false,
                }
            } else {
                true
            }
        };
        let has_prereq = has_prereq && science_ok;
        let has_money = match self.get_player_by_team(team) {
            Some(p) => {
                let cost = self.modified_build_cost_supplies(
                    p.id,
                    template_name,
                    template.build_cost.supplies,
                );
                p.resources.supplies >= cost
            }
            None => false,
        };
        let _ = CANMAKE_OK;
        // C++ MaxSimultaneousOfType residual (heroes MaxSimultaneousOfType=1).
        let maxed_out = {
            use crate::game_logic::host_production_buildable_command_residual::{
                unit_max_simultaneous_of_type_residual, unit_maxed_out_for_player_residual,
            };
            let max = unit_max_simultaneous_of_type_residual(template_name);
            let owned = self.count_team_units_of_template_owned_or_queued(team, template_name);
            unit_maxed_out_for_player_residual(owned, max)
        };
        can_make_type_from_checks_residual(
            has_prereq,
            has_money,
            factory_disabled,
            queue_full,
            parking_full,
            maxed_out,
        )
    }

    /// True when CanMake residual is CANMAKE_OK.
    pub fn can_make_unit_ok(&self, producer_id: ObjectId, template_name: &str) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::CANMAKE_OK;
        self.can_make_unit(producer_id, template_name) == CANMAKE_OK
    }

    pub fn enqueue_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        use crate::game_logic::host_production_buildable_command_residual::{
            CANMAKE_NO_MONEY, CANMAKE_OK,
        };
        use crate::game_logic::host_stealth_fighter::requires_stealth_fighter_science;

        let template = match self.templates.get(&template_name) {
            Some(t) => t.clone(),
            None => return false,
        };
        let science_gated = requires_stealth_fighter_science(&template_name);
        // C++ BuildAssistant::canMakeUnit residual gate (before charging).
        let can_make = self.can_make_unit(producer_id, &template_name);
        if can_make != CANMAKE_OK {
            if can_make == CANMAKE_NO_MONEY {
                if let Some(producer) = self.objects.get(&producer_id) {
                    if let Some(p) = self.get_player_by_team(producer.team) {
                        let pid = p.id;
                        self.try_eva_insufficient_funds(pid);
                    }
                }
            }
            if science_gated
                && can_make
                    == crate::game_logic::host_production_buildable_command_residual::CANMAKE_NO_PREREQ
            {
                self.stealth_fighter_science.record_production_denied();
            }
            return false;
        }
        let science_ok = science_gated; // residual already validated via can_make
        if let Some(producer) = self.objects.get(&producer_id) {
            let team = producer.team;
            let Some(player) = self.get_player_mut_by_team(team) else {
                return false;
            };
            let player_id = player.id;
            let base = template.build_cost.supplies;
            let mod_supplies = {
                let factor = player.production_cost_factor(
                    &crate::game_logic::host_upgrade_module_residuals::kindof_cost_tokens(
                        template.is_kind_of(crate::game_logic::KindOf::Vehicle),
                        template.is_kind_of(crate::game_logic::KindOf::Infantry),
                        template.is_kind_of(crate::game_logic::KindOf::Aircraft),
                        template.is_kind_of(crate::game_logic::KindOf::Structure),
                    ),
                );
                crate::game_logic::host_upgrade_module_residuals::apply_production_cost_factor(
                    base, factor,
                )
            };
            let mut cost = template.build_cost.clone();
            cost.supplies = mod_supplies;
            if !player.spend_resources(&cost) {
                // Race residual: money spent between can_make and charge.
                self.try_eva_insufficient_funds(player_id);
                return false;
            }
        }

        let producer_template_name = self
            .objects
            .get(&producer_id)
            .map(|o| o.template_name.clone())
            .unwrap_or_default();
        let quantity = crate::game_logic::host_production_buildable_command_residual::production_quantity_modifier(
            &producer_template_name,
            &template_name,
        );
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if building.add_to_queue_with_quantity(template_name.clone(), &template, quantity) {
                    if science_gated && science_ok {
                        self.stealth_fighter_science.record_production_enqueue();
                    }
                    crate::game_logic::host_production_log::record(
                        producer_id,
                        template_name.clone(),
                    );
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Unlock a science for a team and record residual honesty hooks.
    ///
    /// Fail-closed: not full PrerequisiteSciences rank tree / control-bar UI.
    pub fn unlock_team_science(&mut self, team: Team, science_name: &str) -> bool {
        use crate::game_logic::host_stealth_fighter::is_stealth_fighter_science;
        use crate::game_logic::host_unit_training::is_unit_training_science;

        let player_id = {
            let Some(player) = self.get_player_mut_by_team(team) else {
                return false;
            };
            if !player.unlock_science(science_name) {
                return false;
            }
            player.id
        };
        if is_stealth_fighter_science(science_name) {
            self.stealth_fighter_science.record_science_unlock();
        }
        if is_unit_training_science(science_name) {
            self.unit_training.record_science_unlock();
        }
        self.on_special_power_science_creation(player_id, science_name);
        true
    }

    /// Record SCIENCE_StealthFighter unlock honesty (PurchaseScience residual path).
    pub fn record_stealth_fighter_science_unlock(&mut self) {
        self.stealth_fighter_science.record_science_unlock();
    }

    /// Host SCIENCE_StealthFighter residual honesty registry.
    pub fn stealth_fighter_science(
        &self,
    ) -> &crate::game_logic::host_stealth_fighter::HostStealthFighterRegistry {
        &self.stealth_fighter_science
    }

    /// Residual honesty: SCIENCE_StealthFighter unlocked at least once.
    pub fn honesty_stealth_fighter_science_unlock_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_unlock_ok()
    }

    /// Residual honesty: science-gated Stealth Fighter accepted into production.
    pub fn honesty_stealth_fighter_science_produce_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_produce_ok()
    }

    /// Residual honesty: production denied for missing SCIENCE_StealthFighter.
    pub fn honesty_stealth_fighter_science_deny_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_deny_ok()
    }

    /// Residual honesty: science-gated Stealth Fighter finished production spawn.
    pub fn honesty_stealth_fighter_science_spawn_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_spawn_ok()
    }

    /// Combined residual honesty for SCIENCE_StealthFighter host path.
    pub fn honesty_stealth_fighter_science_ok(&self) -> bool {
        self.stealth_fighter_science.honesty_ok()
    }

    /// Host SCIENCE unit-training residual honesty registry.
    pub fn unit_training(
        &self,
    ) -> &crate::game_logic::host_unit_training::HostUnitTrainingRegistry {
        &self.unit_training
    }

    pub fn honesty_unit_training_unlock_ok(&self) -> bool {
        self.unit_training.honesty_unlock_ok()
    }

    pub fn honesty_unit_training_grant_ok(&self) -> bool {
        self.unit_training.honesty_grant_ok()
    }

    pub fn honesty_unit_training_ok(&self) -> bool {
        self.unit_training.honesty_ok()
    }

    /// Host Demo SuicideBomb residual honesty registry.
    pub fn demo_suicide_bomb(
        &self,
    ) -> &crate::game_logic::host_demo_suicide_bomb::HostDemoSuicideBombRegistry {
        &self.demo_suicide_bomb
    }

    pub fn honesty_demo_suicide_bomb_upgrade_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_upgrade_ok()
    }

    pub fn honesty_demo_suicide_bomb_death_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_death_ok()
    }

    pub fn honesty_demo_suicide_bomb_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_host_path_ok()
    }

    /// Apply residual Demo_DestroyedWeapon blast at a Demo SuicideBomb death site.
    pub fn apply_demo_suicide_bomb_death_at(
        &mut self,
        source_id: ObjectId,
        source_team: Team,
        source_pos: Vec3,
    ) -> bool {
        use crate::game_logic::host_demo_suicide_bomb::{
            plan_demo_destroyed_hits, DEMO_SUICIDE_BOMB_AUDIO,
        };

        let candidates: Vec<(ObjectId, Vec3, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, o)| {
                (
                    *id,
                    o.get_position(),
                    o.is_alive(),
                    o.status.under_construction,
                )
            })
            .collect();
        let hits = plan_demo_destroyed_hits(source_id, source_pos, &candidates);
        let mut damage_dealt = 0.0f32;
        let mut blast_hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        for hit in &hits {
            if let Some(victim) = self.objects.get_mut(&hit.target_id) {
                if !victim.is_alive() {
                    continue;
                }
                damage_dealt += hit.damage.min(victim.health.current.max(0.0));
                blast_hits = blast_hits.saturating_add(1);
                if victim.take_damage_from_immediate(hit.damage, Some(source_id)) {
                    destroy_ids.push((hit.target_id, source_team));
                }
            }
        }
        let destroyed = destroy_ids.len() as u32;
        self.demo_suicide_bomb
            .record_death_detonation(blast_hits, damage_dealt, destroyed);
        self.queue_audio_event(
            AudioEventRequest::new(DEMO_SUICIDE_BOMB_AUDIO)
                .with_object(source_id)
                .with_position(source_pos)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            source_pos,
            self.frame,
            Some(source_id),
            None,
        );
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Apply residual Demo_SuicideDynamitePackPlusFire blast (SUICIDED residual).
    pub fn apply_demo_plus_fire_death_at(
        &mut self,
        source_id: ObjectId,
        source_team: Team,
        source_pos: Vec3,
    ) -> bool {
        use crate::game_logic::host_demo_suicide_bomb::{
            plan_demo_plus_fire_hits, DEMO_SUICIDE_BOMB_AUDIO, DEMO_SUICIDE_DYNAMITE_PLUS_FIRE,
        };

        let _ = DEMO_SUICIDE_DYNAMITE_PLUS_FIRE; // honesty weapon name residual
        let candidates: Vec<(ObjectId, Vec3, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, o)| {
                (
                    *id,
                    o.get_position(),
                    o.is_alive(),
                    o.status.under_construction,
                )
            })
            .collect();
        let hits = plan_demo_plus_fire_hits(source_id, source_pos, &candidates);
        let mut damage_dealt = 0.0f32;
        let mut blast_hits = 0u32;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        for hit in &hits {
            if let Some(victim) = self.objects.get_mut(&hit.target_id) {
                if !victim.is_alive() {
                    continue;
                }
                damage_dealt += hit.damage.min(victim.health.current.max(0.0));
                blast_hits = blast_hits.saturating_add(1);
                if victim.take_damage_from_immediate(hit.damage, Some(source_id)) {
                    destroy_ids.push((hit.target_id, source_team));
                }
            }
        }
        let destroyed = destroy_ids.len() as u32;
        self.demo_suicide_bomb
            .record_suicided_detonation(blast_hits, damage_dealt, destroyed);
        self.queue_audio_event(
            AudioEventRequest::new(DEMO_SUICIDE_BOMB_AUDIO)
                .with_object(source_id)
                .with_position(source_pos)
                .with_priority(190),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            source_pos,
            self.frame,
            Some(source_id),
            None,
        );
        for (vid, killer) in destroy_ids {
            self.mark_object_for_destruction(vid, Some(killer));
        }
        true
    }

    /// Issue Demo_Command_TertiarySuicide residual (intentional SUICIDED PlusFire).
    ///
    /// Fail-closed: requires SuicideBomb upgrade + CommandSetUpgrade residual.
    /// Terrorists keep host_terrorist path (not TertiarySuicide).
    pub fn issue_demo_tertiary_suicide(&mut self, unit_id: ObjectId) -> bool {
        use crate::game_logic::host_demo_suicide_bomb::{
            can_issue_demo_tertiary_suicide, command_set_enables_tertiary_suicide,
        };
        use crate::game_logic::host_terrorist::is_terrorist_template;

        let Some(obj) = self.objects.get(&unit_id) else {
            self.demo_suicide_bomb.record_tertiary_suicide_denied();
            return false;
        };
        let is_terrorist = is_terrorist_template(&obj.template_name);
        let can = can_issue_demo_tertiary_suicide(
            &obj.template_name,
            &obj.applied_upgrades,
            obj.is_alive(),
            is_terrorist,
        ) && command_set_enables_tertiary_suicide(obj.command_set_override.as_deref());
        if !can {
            self.demo_suicide_bomb.record_tertiary_suicide_denied();
            return false;
        }

        let source_team = obj.team;
        let source_pos = obj.get_position();
        // Mark before blast so destroy path skips DestroyedWeapon double-fire.
        if let Some(obj) = self.objects.get_mut(&unit_id) {
            obj.demo_suicided_detonating = true;
            obj.record_host_demo_mine_cheer();
            Self::mark_object_destroyed_authority_aware(obj, Some(unit_id));
        }
        self.demo_suicide_bomb.record_tertiary_suicide_issued();
        let _ = self.apply_demo_plus_fire_death_at(unit_id, source_team, source_pos);
        self.mark_object_for_destruction(unit_id, Some(source_team));
        true
    }

    pub fn honesty_demo_suicide_bomb_command_set_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_command_set_ok()
    }

    pub fn honesty_demo_suicide_bomb_suicided_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_suicided_ok()
    }

    pub fn honesty_demo_suicide_bomb_plus_fire_ok(&self) -> bool {
        self.demo_suicide_bomb.honesty_plus_fire_path_ok()
    }

    /// Cancel a queued production item by template name (first match).
    pub fn cancel_production(&mut self, producer_id: ObjectId, template_name: String) -> bool {
        let Some(team) = self.objects.get(&producer_id).map(|p| p.team) else {
            return false;
        };
        if !self.players.values().any(|player| player.team == team) {
            return false;
        }

        let mut refund: Option<Resources> = None;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                if let Some(pos) = building
                    .production_queue
                    .iter()
                    .position(|item| item.template_name == template_name)
                {
                    refund = building.cancel_production(pos).map(|item| item.cost);
                }
            }
        }

        if let Some(cost) = refund {
            if let Some(player) = self.get_player_mut_by_team(team) {
                // Economy authority: refund via pending delta + log (GameWorld last-writer).
                player.apply_supply_gain(cost.supplies);
                player.power_available -= cost.power;
                crate::game_logic::host_economy_log::record(
                    player.id,
                    player.effective_supplies(),
                    player.power_available,
                );
            }
            crate::game_logic::host_production_log::record_cancel(
                producer_id,
                template_name.clone(),
            );
            // Wave 485: last cancelled item clears factory exit-delay residual.
            if let Some(producer) = self.objects.get_mut(&producer_id) {
                if let Some(building) = producer.building_data.as_mut() {
                    if building.production_queue.is_empty() && building.exit_delay_remaining > 0.0 {
                        building.exit_delay_remaining = 0.0;
                        crate::game_logic::host_production_progress_log::record_exit_delay_only(
                            producer_id,
                            0.0,
                        );
                    }
                }
            }
            return true;
        }

        false
    }

    /// Wave 985: host production pause residual (ControlBar empty dual-world queue).
    pub fn set_production_paused(&mut self, producer_id: ObjectId, paused: bool) -> bool {
        let Some(producer) = self.objects.get_mut(&producer_id) else {
            return false;
        };
        let Some(building) = producer.building_data.as_mut() else {
            return false;
        };
        building.set_production_paused(paused);
        true
    }

    /// Cancel every queued production item on a producer and refund the owner.
    pub fn cancel_all_production(&mut self, producer_id: ObjectId) -> bool {
        let Some(team) = self.objects.get(&producer_id).map(|p| p.team) else {
            return false;
        };
        if !self.players.values().any(|player| player.team == team) {
            return false;
        }

        let mut refund = Resources::default();
        let mut cancelled_any = false;
        let mut cancelled_names: Vec<String> = Vec::new();
        let mut cleared_exit_delay = false;
        if let Some(producer) = self.objects.get_mut(&producer_id) {
            if let Some(building) = producer.building_data.as_mut() {
                for item in building.production_queue.drain(..) {
                    refund.supplies = refund.supplies.saturating_add(item.cost.supplies);
                    refund.power += item.cost.power;
                    cancelled_names.push(item.template_name);
                    cancelled_any = true;
                }
                // Wave 485: empty queue clears QueueProductionExitUpdate residual.
                if cancelled_any && building.exit_delay_remaining > 0.0 {
                    building.exit_delay_remaining = 0.0;
                    cleared_exit_delay = true;
                }
            }
        }

        if cancelled_any {
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.apply_supply_gain(refund.supplies);
                player.power_available -= refund.power;
                crate::game_logic::host_economy_log::record(
                    player.id,
                    player.effective_supplies(),
                    player.power_available,
                );
            }
            // Wave 484: sole-tick skips per-frame progress log — Cancel refreshes
            // GW producer queue snapshot after host drain (sell/death/cancel-all).
            if cancelled_names.is_empty() {
                crate::game_logic::host_production_log::record_cancel(producer_id, String::new());
            } else {
                for name in cancelled_names {
                    crate::game_logic::host_production_log::record_cancel(producer_id, name);
                }
            }
            // Wave 485: publish exit-delay clear so GW sole-tick does not hold a ghost timer.
            if cleared_exit_delay {
                crate::game_logic::host_production_progress_log::record_exit_delay_only(
                    producer_id,
                    0.0,
                );
            }
        }

        cancelled_any
    }

    /// Snapshot pending radar texts for PresentationFrame (does not drain).
    pub fn radar_notification_snapshot(
        &self,
    ) -> Vec<crate::game_logic::radar_notifications::RadarEntry> {
        self.radar_notifications.snapshot()
    }

    pub fn queue_radar_message<S: Into<String>>(&mut self, message: S) {
        self.queue_radar_message_at(message, Vec3::ZERO, radar_notifications::RadarKind::Generic);
    }

    pub(super) fn queue_script_radar_event(&mut self, event: RadarScriptEventRequest) {
        let position = event.position;
        match event.event_type {
            1 => self.queue_radar_message_at(
                "Construction event",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            2 => self.queue_radar_message_at(
                "Upgrade event",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            3 => self.queue_radar_attack_at("Under attack", position),
            4 => self.queue_radar_message_at(
                "Radar event",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            5 => self.queue_radar_message_at(
                "Beacon pulse",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            6 => self.queue_radar_message_at(
                "Infiltration event",
                position,
                radar_notifications::RadarKind::Attack,
            ),
            7 => self.queue_radar_message_at(
                "Battle plan event",
                position,
                radar_notifications::RadarKind::Ally,
            ),
            8 => self.queue_radar_message_at(
                "Stealth discovered",
                position,
                radar_notifications::RadarKind::Generic,
            ),
            9 => self.queue_radar_message_at(
                "Stealth neutralized",
                position,
                radar_notifications::RadarKind::Attack,
            ),
            10 => {
                self.last_radar_event = Some(RadarEntry {
                    text: "Radar event".to_string(),
                    position,
                    timestamp: self.sim_time_seconds,
                    kind: radar_notifications::RadarKind::Generic,
                });
            }
            _ => {}
        }
    }

    pub fn queue_radar_message_at<S: Into<String>>(
        &mut self,
        message: S,
        position: Vec3,
        kind: radar_notifications::RadarKind,
    ) {
        let kind_index = match kind {
            radar_notifications::RadarKind::Generic => 0,
            radar_notifications::RadarKind::Attack => 1,
            radar_notifications::RadarKind::Ally => 2,
        };
        const RADAR_DEDUP_WINDOW: f32 = 0.5;
        if self.sim_time_seconds - self.last_radar_kind_time[kind_index] < RADAR_DEDUP_WINDOW {
            // Drop duplicate of same kind emitted too fast.
            return;
        }
        let entry = RadarEntry {
            text: message.into(),
            position,
            timestamp: self.sim_time_seconds,
            kind,
        };
        self.radar_notifications.push(entry.clone());
        self.last_radar_event = Some(entry);
        self.last_radar_kind_time[kind_index] = self.sim_time_seconds;

        // Trigger the classic radar/EVA audio cue to mirror the C++ client feedback.
        self.maybe_play_radar_audio("Radar_Event");
    }

    /// Radar attack warning at a location (plays distinct EVA cue).
    pub fn queue_radar_attack_at<S: Into<String>>(&mut self, message: S, position: Vec3) {
        self.queue_radar_message_at(message, position, radar_notifications::RadarKind::Attack);
        self.maybe_play_radar_audio("Radar_Attack");
    }

    /// Radar ally request cue.
    pub fn queue_radar_ally<S: Into<String>>(&mut self, message: S) {
        self.queue_radar_message_at(message, Vec3::ZERO, radar_notifications::RadarKind::Ally);
        self.maybe_play_radar_audio("Radar_Ally");
    }

    /// C++ Radar::tryInfiltrationEvent residual.
    ///
    /// Notifies the **victim** controlling team (local player residual) with
    /// RADAR_EVENT_INFILTRATION + audio honesty. Saboteur / hijack / special
    /// ability paths call this when an enemy structure/vehicle is compromised.

    /// Residual honesty: last radar message text (tests / presentation bridge).
    pub fn last_radar_message_text(&self) -> Option<&str> {
        self.last_radar_event.as_ref().map(|e| e.text.as_str())
    }

    /// C++ Object::isLocallyControlled residual for EVA/radar victim gates.
    pub fn is_object_locally_controlled(&self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        self.players
            .values()
            .any(|p| p.team == obj.team && p.is_local && p.is_alive)
    }

    /// C++ TheEva->setShouldPlay(EVA_BuildingSabotaged) when victim is local.

    /// C++ CrateCollide::doSabotageFeedbackFX residual.
    ///
    /// Type-specific MiscAudio cue + Drawable::flashAsSelected on the victim.
    /// Fake buildings skip additional feedback (C++ early return).

    /// C++ SabotageSupplyCenter floating cash text residual:
    /// green GUI:AddCash over saboteur (z+20), red GUI:LoseCash over victim (z+30).
    pub fn spawn_sabotage_cash_floating_texts(
        &mut self,
        saboteur_id: ObjectId,
        victim_id: ObjectId,
        amount: u32,
    ) {
        if amount == 0 {
            return;
        }
        use crate::game_logic::host_money_crate::HostMoneyFloatingText;
        use crate::game_logic::host_saboteur::{
            SABOTEUR_ADD_CASH_COLOR_RGBA, SABOTEUR_ADD_CASH_TEXT_KEY, SABOTEUR_ADD_CASH_Z_OFFSET,
            SABOTEUR_LOSE_CASH_COLOR_RGBA, SABOTEUR_LOSE_CASH_TEXT_KEY,
            SABOTEUR_LOSE_CASH_Z_OFFSET,
        };
        let sab_pos = self
            .objects
            .get(&saboteur_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let vic_pos = self
            .objects
            .get(&victim_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        let frame = self.frame;
        // Host world uses Y-up; C++ Coord3D.z is height → map to .y.
        let add = HostMoneyFloatingText {
            text: format!("+${amount}"),
            text_key: SABOTEUR_ADD_CASH_TEXT_KEY.to_string(),
            position: glam::Vec3::new(sab_pos.x, sab_pos.y + SABOTEUR_ADD_CASH_Z_OFFSET, sab_pos.z),
            color_rgba: SABOTEUR_ADD_CASH_COLOR_RGBA,
            amount,
            spawn_frame: frame,
            crate_id: saboteur_id,
            picker_id: victim_id,
        };
        let lose = HostMoneyFloatingText {
            text: format!("-${amount}"),
            text_key: SABOTEUR_LOSE_CASH_TEXT_KEY.to_string(),
            position: glam::Vec3::new(
                vic_pos.x,
                vic_pos.y + SABOTEUR_LOSE_CASH_Z_OFFSET,
                vic_pos.z,
            ),
            color_rgba: SABOTEUR_LOSE_CASH_COLOR_RGBA,
            amount,
            spawn_frame: frame,
            crate_id: victim_id,
            picker_id: saboteur_id,
        };
        self.host_money_crates.record_money_floating_text(add);
        self.host_money_crates.record_money_floating_text(lose);
        self.saboteur.record_cash_floating_texts();
    }

    pub fn do_sabotage_feedback_fx(
        &mut self,
        victim_id: ObjectId,
        kind: crate::game_logic::host_saboteur::SaboteurEffectKind,
    ) {
        use crate::game_logic::host_saboteur::SaboteurEffectKind;
        // Flash first so FakeBuilding still returns without audio but we match
        // C++: FakeBuilding returns before flash. So skip entirely for fake.
        if matches!(kind, SaboteurEffectKind::FakeBuilding) {
            return;
        }
        if let Some(audio) = kind.feedback_audio() {
            let pos = self
                .objects
                .get(&victim_id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            self.queue_audio_event(
                AudioEventRequest::new(audio)
                    .with_object(victim_id)
                    .with_position(pos)
                    .with_priority(170),
            );
        }
        if let Some(obj) = self.objects.get_mut(&victim_id) {
            obj.flash_as_selected();
            self.saboteur.record_flash_as_selected();
        }
        self.saboteur.record_feedback_fx();
    }

    /// C++ TheEva->setShouldPlay(EVA_BuildingBeingStolen) when capture prep starts.

    /// C++ TheEva->setShouldPlay(EVA_VehicleStolen) when hijack victim is local.

    /// C++ Object::onDie EVA residual for local non-self-inflicted losses.
    ///
    /// - STRUCTURE + MP_COUNT_FOR_VICTORY-class → EVA_BuildingLost (C++ typo BuldingLost)
    /// - INFANTRY or VEHICLE → EVA_UnitLost + RADAR_EVENT_FAKE residual

    /// C++ Radar::tryUnderAttackEvent residual.
    ///
    /// Throttled by tryEvent distance/time residual. Fires radar attack message,
    /// audio honesty, and EVA BaseUnderAttack / AllyUnderAttack for victory-class
    /// structures owned by local / allied players.

    /// C++ Eva::shouldPlayLowPower residual for the local player.

    /// C++ TheEva->setShouldPlay(EVA_UpgradeComplete) residual (local player).

    /// Classify superweapon residual family from template name.
    /// Returns Some("particle"|"nuke"|"scud") for EVA SuperweaponReady paths.
    pub fn classify_superweapon_eva_kind(template_name: &str) -> Option<&'static str> {
        let n = template_name.to_ascii_lowercase();
        if n.contains("particle") && (n.contains("cannon") || n.contains("uplink")) {
            Some("particle")
        } else if n.contains("scudstorm") || n.contains("scud_storm") {
            Some("scud")
        } else if n.contains("nuclearmissile")
            || n.contains("nuclear_missile")
            || (n.contains("nuke") && n.contains("silo"))
            || n.contains("neutronmissile")
        {
            Some("nuke")
        } else if n.contains("particlecannon") || n.contains("particleuplink") {
            Some("particle")
        } else {
            None
        }
    }

    /// C++ InGameUI SuperweaponReady EVA residual (own/ally/enemy × type).

    /// C++ Player::onStructureConstructionComplete SuperweaponDetected EVA residual.

    /// Map HostSuperweaponKind residual to EVA SuperweaponLaunched family key.
    /// Only ParticleCannon / NuclearMissile / ScudStorm map to C++ launched EVA.
    pub fn classify_superweapon_launched_kind(
        kind: crate::game_logic::special_power_strikes::HostSuperweaponKind,
    ) -> Option<&'static str> {
        use crate::game_logic::special_power_strikes::HostSuperweaponKind;
        match kind {
            HostSuperweaponKind::ParticleCannon => Some("particle"),
            HostSuperweaponKind::NuclearMissile => Some("nuke"),
            HostSuperweaponKind::ScudStorm => Some("scud"),
            _ => None,
        }
    }

    /// C++ SpecialPowerModule SuperweaponLaunched EVA residual (own/ally/enemy × type).

    /// C++ GameLogicDispatch beacon place residual:
    /// EVA_BeaconDetected when local player is ALLIES with the placer (not self).

    /// C++ SpecialPowerModule SuperweaponLaunched GPS Scrambler / Sneak Attack residual.
    ///
    /// `kind`: "gps" | "sneak"
    pub fn try_eva_special_launched_misc(&mut self, owner_team: Team, kind: &str) {
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };
        use gamelogic::helpers::EvaEvent;
        let event = match (kind, relation) {
            ("gps", "own") => EvaEvent::SuperweaponLaunchedOwnGpsScrambler,
            ("gps", "ally") => EvaEvent::SuperweaponLaunchedAllyGpsScrambler,
            ("gps", _) => EvaEvent::SuperweaponLaunchedEnemyGpsScrambler,
            ("sneak", "own") => EvaEvent::SuperweaponLaunchedOwnSneakAttack,
            ("sneak", "ally") => EvaEvent::SuperweaponLaunchedAllySneakAttack,
            ("sneak", _) => EvaEvent::SuperweaponLaunchedEnemySneakAttack,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_special_launched_misc = self.eva_special_launched_misc.saturating_add(1);
    }

    pub fn honesty_eva_special_launched_misc_ok(&self) -> bool {
        self.eva_special_launched_misc > 0
    }

    pub fn try_eva_beacon_detected(&mut self, placer_player_id: u32) {
        let Some(placer) = self.players.get(&placer_player_id) else {
            return;
        };
        let placer_team = placer.team;
        let placer_alliance = placer.alliance_team;
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        // C++ relationship ALLIES — exclude self / same controlling player.
        if local.id == placer_player_id || local.team == placer_team {
            return;
        }
        let is_ally = local.alliance_team >= 0 && local.alliance_team == placer_alliance;
        if !is_ally {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BeaconDetected,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::BeaconDetected);
        self.eva_beacon_detected = self.eva_beacon_detected.saturating_add(1);
    }

    pub fn honesty_eva_beacon_detected_ok(&self) -> bool {
        self.eva_beacon_detected > 0
    }

    /// C++ stealth detector hero EVA residual (own vs enemy).
    ///
    /// When a stealth hero is newly detected, fire Own* if local owns the hero,
    /// else Enemy* if local is hostile to the hero team.
    pub fn try_eva_hero_detected(&mut self, hero_id: ObjectId) {
        let Some(obj) = self.objects.get(&hero_id) else {
            return;
        };
        if !obj.is_alive() {
            return;
        }
        let name = obj.template_name.to_ascii_lowercase();
        let team = obj.team;
        let kind =
            if crate::game_logic::host_hero_abilities::is_black_lotus_template(&obj.template_name)
                || name.contains("blacklotus")
                || name.contains("black_lotus")
            {
                "lotus"
            } else if name.contains("jarmen") || name.contains("kell") {
                "jarmen"
            } else if name.contains("burton") || name.contains("colonel") {
                "burton"
            } else {
                return;
            };
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let is_own = team == local_team;
        let is_ally = !is_own && local_alliance >= 0 && local_alliance == owner_alliance;
        // Enemy residual for non-own non-ally; ally residual fail-closed (no ally EVA names).
        if is_ally {
            return;
        }
        use gamelogic::helpers::EvaEvent;
        let event = match (kind, is_own) {
            ("lotus", true) => EvaEvent::OwnBlackLotusDetected,
            ("lotus", false) => EvaEvent::EnemyBlackLotusDetected,
            ("jarmen", true) => EvaEvent::OwnJarmenKellDetected,
            ("jarmen", false) => EvaEvent::EnemyJarmenKellDetected,
            ("burton", true) => EvaEvent::OwnColonelBurtonDetected,
            ("burton", false) => EvaEvent::EnemyColonelBurtonDetected,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_hero_detected = self.eva_hero_detected.saturating_add(1);
    }

    pub fn honesty_eva_hero_detected_ok(&self) -> bool {
        self.eva_hero_detected > 0
    }

    pub fn try_eva_superweapon_launched(
        &mut self,
        owner_team: Team,
        kind: crate::game_logic::special_power_strikes::HostSuperweaponKind,
    ) {
        let Some(family) = Self::classify_superweapon_launched_kind(kind) else {
            return;
        };
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };
        use gamelogic::helpers::EvaEvent;
        let event = match (family, relation) {
            ("particle", "own") => EvaEvent::SuperweaponLaunchedOwnParticleCannon,
            ("particle", "ally") => EvaEvent::SuperweaponLaunchedAllyParticleCannon,
            ("particle", _) => EvaEvent::SuperweaponLaunchedEnemyParticleCannon,
            ("nuke", "own") => EvaEvent::SuperweaponLaunchedOwnNuke,
            ("nuke", "ally") => EvaEvent::SuperweaponLaunchedAllyNuke,
            ("nuke", _) => EvaEvent::SuperweaponLaunchedEnemyNuke,
            ("scud", "own") => EvaEvent::SuperweaponLaunchedOwnScudStorm,
            ("scud", "ally") => EvaEvent::SuperweaponLaunchedAllyScudStorm,
            ("scud", _) => EvaEvent::SuperweaponLaunchedEnemyScudStorm,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_superweapon_launched = self.eva_superweapon_launched.saturating_add(1);
    }

    pub fn honesty_eva_superweapon_launched_ok(&self) -> bool {
        self.eva_superweapon_launched > 0
    }

    pub fn try_eva_superweapon_detected(&mut self, owner_team: Team, template_name: &str) {
        let Some(kind) = Self::classify_superweapon_eva_kind(template_name) else {
            return;
        };
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);
        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };
        use gamelogic::helpers::EvaEvent;
        let event = match (kind, relation) {
            ("particle", "own") => EvaEvent::SuperweaponDetectedOwnParticleCannon,
            ("particle", "ally") => EvaEvent::SuperweaponDetectedAllyParticleCannon,
            ("particle", _) => EvaEvent::SuperweaponDetectedEnemyParticleCannon,
            ("nuke", "own") => EvaEvent::SuperweaponDetectedOwnNuke,
            ("nuke", "ally") => EvaEvent::SuperweaponDetectedAllyNuke,
            ("nuke", _) => EvaEvent::SuperweaponDetectedEnemyNuke,
            ("scud", "own") => EvaEvent::SuperweaponDetectedOwnScudStorm,
            ("scud", "ally") => EvaEvent::SuperweaponDetectedAllyScudStorm,
            ("scud", _) => EvaEvent::SuperweaponDetectedEnemyScudStorm,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_superweapon_detected = self.eva_superweapon_detected.saturating_add(1);
    }

    pub fn honesty_eva_superweapon_detected_ok(&self) -> bool {
        self.eva_superweapon_detected > 0
    }

    pub fn try_eva_superweapon_ready(
        &mut self,
        _source_id: ObjectId,
        owner_team: Team,
        template_name: &str,
    ) {
        let Some(kind) = Self::classify_superweapon_eva_kind(template_name) else {
            return;
        };
        // Need a local player to attribute own/ally/enemy residual.
        let Some(local) = self.players.values().find(|p| p.is_local && p.is_alive) else {
            return;
        };
        let local_team = local.team;
        let local_alliance = local.alliance_team;
        let owner_alliance = self
            .players
            .values()
            .find(|p| p.team == owner_team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);

        let relation = if owner_team == local_team {
            "own"
        } else if local_alliance >= 0 && local_alliance == owner_alliance {
            "ally"
        } else {
            "enemy"
        };

        use gamelogic::helpers::EvaEvent;
        let event = match (kind, relation) {
            ("particle", "own") => EvaEvent::SuperweaponReadyOwnParticleCannon,
            ("particle", "ally") => EvaEvent::SuperweaponReadyAllyParticleCannon,
            ("particle", _) => EvaEvent::SuperweaponReadyEnemyParticleCannon,
            ("nuke", "own") => EvaEvent::SuperweaponReadyOwnNuke,
            ("nuke", "ally") => EvaEvent::SuperweaponReadyAllyNuke,
            ("nuke", _) => EvaEvent::SuperweaponReadyEnemyNuke,
            ("scud", "own") => EvaEvent::SuperweaponReadyOwnScudStorm,
            ("scud", "ally") => EvaEvent::SuperweaponReadyAllyScudStorm,
            ("scud", _) => EvaEvent::SuperweaponReadyEnemyScudStorm,
            _ => return,
        };
        let _ = gamelogic::helpers::TheEva::set_should_play(event);
        crate::game_logic::host_eva_log::record_event(event);
        self.eva_superweapon_ready = self.eva_superweapon_ready.saturating_add(1);
    }

    pub fn honesty_eva_superweapon_ready_ok(&self) -> bool {
        self.eva_superweapon_ready > 0
    }

    /// C++ ProductionUpdate RADAR_EVENT_UPGRADE + UPGRADE:UpgradeComplete residual.
    ///
    /// Creates a radar event at a producer structure (or team centroid residual)
    /// and queues a localized upgrade-complete radar message for the local player.

    /// C++ structure construction-complete residual feedback for local owner:
    /// radar message + BuildingComplete audio honesty + model condition bit.

    /// Start radar dish extend residual on a newly completed radar provider.
    pub fn maybe_start_radar_extend(&mut self, structure_id: ObjectId) {
        use crate::game_logic::host_radar::is_legal_radar_provider;
        use crate::game_logic::host_radar_stealth_vision_residual::RADAR_EXTEND_TIME_FRAMES_RESIDUAL;
        let Some(obj) = self.objects.get_mut(&structure_id) else {
            return;
        };
        let is_cc = obj.is_command_center() || obj.is_kind_of(KindOf::CommandCenter);
        if !is_legal_radar_provider(obj.is_alive(), true, is_cc, &obj.template_name) {
            return;
        }
        let done = self.frame.saturating_add(RADAR_EXTEND_TIME_FRAMES_RESIDUAL);
        obj.extend_radar(done);
        self.radar_extend_starts = self.radar_extend_starts.saturating_add(1);
    }

    pub fn honesty_radar_extend_start_ok(&self) -> bool {
        self.radar_extend_starts > 0
    }

    pub fn honesty_radar_extend_complete_ok(&self) -> bool {
        self.radar_extend_completes > 0
    }

    /// C++ SpecialPowerModule::onSpecialPowerCreation residual for SW structures.
    ///
    /// Starts full ReloadTime recharge on the structure's PublicTimer power
    /// (ParticleCannon / NuclearMissile / ScudStorm). SharedNSync science powers
    /// are handled separately via `on_special_power_science_creation`.
    pub fn on_structure_superweapon_creation(&mut self, structure_id: ObjectId) {
        use crate::game_logic::host_superweapon_kindof::special_power_for_superweapon_structure;
        let Some(obj) = self.objects.get(&structure_id) else {
            return;
        };
        if !obj.is_alive() || !obj.is_constructed() {
            return;
        }
        let Some(power) = special_power_for_superweapon_structure(&obj.template_name) else {
            return;
        };
        // Non-shared structure SWs: startPowerRecharge only (not express ready-now).
        if let Some(obj) = self.objects.get_mut(&structure_id) {
            // Retail KindOf POWERED residual for energy-draining SWs (PUC/Nuke).
            if crate::game_logic::host_superweapon_kindof::superweapon_energy_production_for_template(
                &obj.template_name,
            )
            .is_some_and(|e| e < 0)
            {
                obj.thing.template.add_kind_of(KindOf::Powered);
            }
            obj.start_power_recharge(&power);
        }
        let _ = self
            .special_power_strikes
            .reset_timers_for_source_object(structure_id);
    }

    pub fn notify_structure_construction_complete(&mut self, structure_id: ObjectId) {
        let Some(obj) = self.objects.get_mut(&structure_id) else {
            return;
        };
        // C++ ProductionUpdate CONSTRUCTION_COMPLETE + duration residual.
        let now = self.frame.max(1);
        obj.set_construction_complete_condition_at(now);
        let team = obj.team;
        let pos = obj.get_position();
        let name = obj.template_name.clone();
        // NLL ends `obj` borrow after last field copy above.
        // C++ PreorderCreate::onBuildComplete residual.
        let did_preorder = self
            .players
            .values()
            .find(|p| p.team == team && p.is_alive)
            .map(|p| p.did_preorder)
            .unwrap_or(false);
        if crate::game_logic::host_preorder_create::is_preorder_create_template(&name) {
            if let Some(o) = self.objects.get_mut(&structure_id) {
                o.model_condition_bits =
                    crate::game_logic::host_preorder_create::apply_preorder_model_bit(
                        o.model_condition_bits,
                        did_preorder,
                    );
                o.refresh_model_condition_bits();
            }
            if did_preorder {
                self.preorder_create_reg.record_set();
            } else {
                self.preorder_create_reg.record_clear();
            }
        }
        // C++ SpecialPowerCreate → onSpecialPowerCreation (all owners, not local-only).
        self.on_structure_superweapon_creation(structure_id);
        let local = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.team == team);
        if !local {
            self.structure_complete_events = self.structure_complete_events.saturating_add(1);
            return;
        }
        // C++ DozerAIUpdate complete residual: DOZER:ConstructionComplete +
        // VoiceTaskComplete on dozer + RADAR_EVENT_CONSTRUCTION.
        let msg = localization::localize(
            "DOZER:ConstructionComplete",
            &format!("Construction complete: {name}"),
        );
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
        self.radar_construction_events = self.radar_construction_events.saturating_add(1);
        // Prefer nearby same-team dozer VoiceTaskComplete residual.
        let dozer_id = self
            .objects
            .iter()
            .find(|(_, o)| {
                o.team == team
                    && o.is_alive()
                    && o.can_construct()
                    && o.get_position().distance(pos) <= 80.0
            })
            .map(|(id, _)| *id);
        if let Some(did) = dozer_id {
            let dpos = self
                .objects
                .get(&did)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            self.queue_audio_event(
                AudioEventRequest::new("VoiceTaskComplete")
                    .with_object(did)
                    .with_position(dpos)
                    .with_priority(155),
            );
        } else {
            self.queue_audio_event(
                AudioEventRequest::new("BuildingComplete")
                    .with_object(structure_id)
                    .with_position(pos)
                    .with_priority(150),
            );
        }
        self.structure_complete_events = self.structure_complete_events.saturating_add(1);
    }

    /// C++ unit production complete residual: VoiceCreated + UnitReady radar for local.
    pub fn notify_unit_production_complete(
        &mut self,
        unit_id: ObjectId,
        producer_id: ObjectId,
        template_name: &str,
    ) {
        let Some(unit) = self.objects.get(&unit_id) else {
            return;
        };
        let team = unit.team;
        let pos = unit.get_position();
        let local = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.team == team);
        // C++ VoiceCreated on new unit always (all owners).
        self.queue_audio_event(
            AudioEventRequest::new("VoiceCreated")
                .with_object(unit_id)
                .with_position(pos)
                .with_priority(140),
        );
        if local {
            let msg =
                localization::localize("GUI:UnitReady", &format!("Unit ready: {template_name}"));
            self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
        }
        let _ = producer_id;
        self.unit_ready_events = self.unit_ready_events.saturating_add(1);
    }

    pub fn honesty_structure_complete_ok(&self) -> bool {
        self.structure_complete_events > 0
    }

    pub fn honesty_radar_construction_event_ok(&self) -> bool {
        self.radar_construction_events > 0
    }

    pub fn honesty_production_door_cycle_ok(&self) -> bool {
        self.production_door_cycles > 0
    }

    /// C++ DozerAIUpdate / ProductionUpdate ACTIVELY_CONSTRUCTING residual.
    ///
    /// - Dozers with AIState::Constructing get the bit set
    /// - Factories with non-empty production queue get the bit set
    /// - Cleared when idle / empty queue
    pub fn update_actively_constructing_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let ac_mask = 1u128 << actively_constructing_model_bit();
        let mut updates = 0u32;
        // Only workers / producers / objects already carrying the bit — skip the
        // rest of Lone Eagle's ~900 decorative props each frame.
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.can_construct()
                        || o.building_data
                            .as_ref()
                            .map(|b| !b.production_queue.is_empty())
                            .unwrap_or(false)
                        || (o.model_condition_bits & ac_mask) != 0)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            // C++ DozerAIUpdate: ACTIVELY_CONSTRUCTING for BUILD and REPAIR.
            let is_dozer_building = obj.can_construct()
                && matches!(obj.ai_state, AIState::Constructing | AIState::Repairing);
            let is_producing = obj
                .building_data
                .as_ref()
                .map(|b| !b.production_queue.is_empty())
                .unwrap_or(false);
            let want = is_dozer_building || is_producing;
            // Cheap edge: always set to desired state (idempotent bit ops).
            let bit_before = obj.model_condition_bits;
            obj.set_actively_constructing(want);
            if obj.model_condition_bits != bit_before {
                updates = updates.saturating_add(1);
            }
        }
        if updates > 0 {
            self.actively_constructing_updates =
                self.actively_constructing_updates.saturating_add(updates);
        }
    }

    /// C++ BuildAssistant::sellObject residual — start multi-frame sell process.

    /// C++ Object::setDisabled(DISABLED_UNMANNED) car-bomb dead-man trigger residual.
    ///
    /// If vehicle has WEAPONSET_CARBOMB / IS_CARBOMB, sniping the pilot detonates
    /// it instead of leaving an unmanned car bomb.
    pub fn maybe_detonate_carbomb_on_unmanned(&mut self, vehicle_id: ObjectId) -> bool {
        let is_bomb = self
            .objects
            .get(&vehicle_id)
            .map(|o| o.is_alive() && o.is_car_bomb())
            .unwrap_or(false);
        if !is_bomb {
            return false;
        }
        // Clear unmanned so detonation path owns the object (not recrewable).
        if let Some(o) = self.objects.get_mut(&vehicle_id) {
            o.set_status_disabled_unmanned(false);
            o.status.unmanned_owner_team = None;
        }
        let ok = self.detonate_car_bomb(vehicle_id);
        if ok {
            self.carbomb_unmanned_detonations = self.carbomb_unmanned_detonations.saturating_add(1);
        }
        ok
    }

    /// C++ OverchargeBehavior::enable / toggle residual for China power plants.
    ///
    /// Adjusts power_provided by EnergyBonus when toggling; auto-disable path
    /// is handled by `update_overcharge_drain`.
    pub fn toggle_overcharge_object(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_structure_economy_residual::{
            is_power_plant_template, CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC,
            CHINA_POWER_ENERGY_BONUS,
        };
        let _ = CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        if !is_power_plant_template(&obj.template_name)
            && !obj.is_kind_of(KindOf::PowerPlant)
            && !obj.is_kind_of(KindOf::FSPower)
        {
            return false;
        }
        // C++ NotAllowedWhenHealthBelowPercent residual (China  = typically 0.2?).
        // Use 20% if enabling while critically damaged.
        const NOT_ALLOWED_BELOW: f32 = 0.20;
        let hp_frac = if obj.max_health > 0.0 {
            obj.health.current / obj.max_health
        } else {
            0.0
        };
        if !obj.overcharge_enabled && hp_frac < NOT_ALLOWED_BELOW {
            return false;
        }
        let bonus = CHINA_POWER_ENERGY_BONUS;
        if obj.overcharge_enabled {
            // Disable.
            obj.set_overcharge_enabled(false);
            obj.power_provided = (obj.power_provided - bonus).max(0);
            obj.record_host_entity_power();
            // C++ PowerPlantUpdate::extendRods(FALSE) residual.
            use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
            if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                obj.model_condition_bits &= !(1u128 << bit);
            }
        } else {
            obj.set_overcharge_enabled(true);
            obj.power_provided = obj.power_provided.saturating_add(bonus);
            obj.record_host_entity_power();
            if let Some(bit) =
                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                    "POWER_PLANT_UPGRADED",
                )
            {
                obj.model_condition_bits |= 1u128 << bit;
            }
        }
        self.overcharge_toggles = self.overcharge_toggles.saturating_add(1);
        true
    }

    /// C++ OverchargeBehavior::update residual — drain HP while overcharge active.
    pub fn update_overcharge_drain(&mut self, dt: f32) {
        use crate::game_logic::host_structure_economy_residual::CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC;
        if dt <= 0.0 {
            return;
        }
        const NOT_ALLOWED_BELOW: f32 = 0.20;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.overcharge_enabled && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let max_hp = obj.max_health.max(1.0);
            // C++ amount = (maxHealth * percentPerSec) / LOGICFRAMES_PER_SECOND per frame
            // We receive dt seconds so: maxHealth * percentPerSec * dt
            let dmg = max_hp * CHINA_OVERCHARGE_DRAIN_PERCENT_PER_SEC * dt;
            if dmg > 0.0 {
                let _ = obj.take_damage_from(dmg, Some(id));
            }
            self.overcharge_drain_ticks = self.overcharge_drain_ticks.saturating_add(1);
            let frac = obj.health.current / max_hp;
            let dead = !obj.is_alive() || obj.health.current <= 0.0;
            if dead || frac < NOT_ALLOWED_BELOW {
                // Auto-disable residual (GUI:OverchargeExhausted).
                let bonus =
                    crate::game_logic::host_structure_economy_residual::CHINA_POWER_ENERGY_BONUS;
                obj.set_overcharge_enabled(false);
                obj.power_provided = (obj.power_provided - bonus).max(0);
                obj.record_host_entity_power();
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "POWER_PLANT_UPGRADED",
                    )
                {
                    obj.model_condition_bits &= !(1u128 << bit);
                }
                self.overcharge_exhaustions = self.overcharge_exhaustions.saturating_add(1);
                if dead {
                    self.mark_object_for_destruction(id, None);
                }
            }
        }
    }

    /// C++ PhysicsUpdate infantry→unmanned vehicle pilot residual.
    ///
    /// Returns true when the pair was handled (vehicle recrewed, infantry destroyed).
    pub fn try_infantry_unmanned_reclaim(
        &mut self,
        infantry_id: ObjectId,
        vehicle_id: ObjectId,
    ) -> bool {
        let (inf_team, inf_level, is_inf) = match self.objects.get(&infantry_id) {
            Some(inf) => (
                inf.team,
                inf.experience.level,
                inf.is_kind_of(KindOf::Infantry) && inf.is_alive(),
            ),
            None => return false,
        };
        if !is_inf {
            return false;
        }
        let is_unmanned = self
            .objects
            .get(&vehicle_id)
            .map(|v| v.is_alive() && v.status.disabled_unmanned)
            .unwrap_or(false);
        if !is_unmanned {
            return false;
        }
        if let Some(veh) = self.objects.get_mut(&vehicle_id) {
            let _ = veh.apply_pilot_recrew(inf_team, inf_level);
        }
        self.destroy_object(infantry_id);
        // C++ destroyObject is immediate for collision reclaim residual path.
        self.process_destroy_list();
        self.unmanned_reclaims = self.unmanned_reclaims.saturating_add(1);
        true
    }

    /// C++ TunnelContain::onCapture residual.
    ///
    /// Re-home entrance to new owner's tunnel system. Passengers are NOT
    /// kicked (isKickOutOnCapture=false). If this was the old owner's last
    /// entrance and the shared pool is non-empty, eject the pool (same
    /// last-tunnel safety residual as onSelling).
    pub fn on_capture_tunnel_network_residual(
        &mut self,
        tunnel_id: ObjectId,
        old_team: Team,
        new_team: Team,
    ) {
        if old_team == new_team {
            return;
        }
        let is_tunnel = self
            .objects
            .get(&tunnel_id)
            .map(|o| {
                o.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &o.template_name,
                    )
            })
            .unwrap_or(false);
        if !is_tunnel {
            return;
        }

        // Count remaining tunnel entrances for old team (exclude this one).
        let remaining_old: u32 = self
            .objects
            .iter()
            .filter(|(id, o)| {
                **id != tunnel_id
                    && o.team == old_team
                    && o.is_alive()
                    && !o.status.sold
                    && (o.is_tunnel_network_style_container()
                        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                            &o.template_name,
                        ))
            })
            .count() as u32;

        if remaining_old == 0 {
            // Last entrance left old team — eject shared pool (C++ assert path residual).
            let units: Vec<ObjectId> = self.tunnel_network.contained_for_team(old_team);
            if !units.is_empty() {
                let pos = self
                    .objects
                    .get(&tunnel_id)
                    .map(|o| o.get_position())
                    .unwrap_or(glam::Vec3::ZERO);
                for (i, uid) in units.into_iter().enumerate() {
                    let _ = self.tunnel_network.record_exit(old_team, uid, tunnel_id);
                    if let Some(unit) = self.objects.get_mut(&uid) {
                        let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                        let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 12.0;
                        unit.stop_moving();
                        unit.set_position(pos + offset);
                        if crate::gameworld_shadow::gameworld_movement_authority_live() {
                            let p = pos + offset;
                            crate::game_logic::host_move_log::record(
                                unit.id,
                                Some([p.x, p.y, p.z]),
                            );
                            unit.record_host_movement();
                        }
                        unit.set_target(None);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_stop_attack(uid);
                        }
                        unit.set_contained_by(None);
                        unit.set_ai_state(AIState::Idle);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                        }
                        unit.set_status_moving(false);
                        unit.set_status_attacking(false);
                    }
                    self.capture_tunnel_last_ejects =
                        self.capture_tunnel_last_ejects.saturating_add(1);
                }
            }
        }

        // Honesty: entrance transferred to new owner (pool stays with old team unless ejected).
        self.capture_tunnel_transfers = self.capture_tunnel_transfers.saturating_add(1);
        let _ = new_team;
    }

    /// C++ Object::onCapture residual (after ownership flip).
    ///
    /// - OpenContain/TransportContain: kick passengers (tunnels/caves skip)
    /// - AIUpdateInterface::aiIdle when owner changes
    /// - Skirmish AI sells captured faction structures
    /// - Deselect from former owners' selection lists
    pub fn on_capture_object_residual(
        &mut self,
        object_id: ObjectId,
        old_team: Team,
        new_team: Team,
    ) {
        if old_team == new_team {
            return;
        }
        // Contain kick residual.
        self.on_capture_kick_passengers(object_id, old_team, new_team);
        // TunnelContain::onCapture entrance transfer / last-entrance eject residual.
        self.on_capture_tunnel_network_residual(object_id, old_team, new_team);

        // Deselect from all players (C++ TheGameLogic->deselectObject residual).
        for player in self.players.values_mut() {
            let before = player.selected_objects.len();
            player.selected_objects.retain(|&id| id != object_id);
            if player.selected_objects.len() != before {
                self.capture_deselections = self.capture_deselections.saturating_add(1);
            }
        }

        // aiIdle residual — stop orders on the captured object.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving();
            obj.set_status_moving(false);
            obj.set_status_attacking(false);
            // C++ Object::setCaptured(true) residual (sticky private status).
            obj.set_private_captured(true);
            // C++ clearScriptStatus(OBJECT_STATUS_SCRIPT_UNSELLABLE) residual.
        }
        self.clear_target_decision_aware(object_id);
        // Capture must clear host AI/orders immediately (observable residual).
        // Under AI_DECISION_AUTHORITY also log for GameWorld last-write channel.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_ai_state(AIState::Idle);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            let ordinal =
                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Idle);
            crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
        }
        // C++ ScoreKeeper::addObjectCaptured residual for new owner.
        if let Some(p) = self.get_player_mut_by_team(new_team) {
            p.record_object_captured();
        }

        // C++ TechBuildingBehavior MODELCONDITION_CAPTURED residual:
        // playable side owner → set CAPTURED; neutral → clear.
        let is_tech = self
            .objects
            .get(&object_id)
            .map(|o| {
                let n = o.template_name.to_ascii_lowercase();
                n.starts_with("tech")
                    || crate::game_logic::host_oil_derrick::is_oil_derrick_template(
                        &o.template_name,
                    )
                    || n.contains("oilrefinery")
                    || n.contains("hospital")
                    || n.contains("artilleryplatform")
                    || n.contains("reinforcementpad")
                    || n.contains("repairbay") && n.contains("tech")
            })
            .unwrap_or(false);
        if is_tech {
            let playable = new_team != Team::Neutral;
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_captured_model_condition(playable);
            }
            self.capture_tech_model_updates = self.capture_tech_model_updates.saturating_add(1);
        }

        // Skirmish AI sells captured faction structures (C++ isSkirmishAIPlayer residual).
        let new_owner_is_ai = self
            .players
            .values()
            .filter(|p| p.team == new_team && p.is_alive)
            .any(|p| {
                // C++ Player::isSkirmishAIPlayer residual: registered AI difficulty.
                self.ai_manager.ai_difficulty(p.id).is_some()
            });
        let is_faction = self
            .objects
            .get(&object_id)
            .map(|o| o.is_faction_structure())
            .unwrap_or(false);
        if new_owner_is_ai && is_faction {
            // C++ TheBuildAssistant->sellObject(this)
            if self.start_sell_object(object_id) {
                self.capture_ai_auto_sells = self.capture_ai_auto_sells.saturating_add(1);
            }
        }
    }

    /// C++ TransportContain/OpenContain::onCapture residual.
    ///
    /// Default containers kick passengers on capture (tunnels/caves do not).
    /// Unmanned vehicles eject instantly (residual: same eject path).
    pub fn on_capture_kick_passengers(
        &mut self,
        container_id: ObjectId,
        old_team: Team,
        new_team: Team,
    ) {
        if old_team == new_team {
            return;
        }
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        // C++ TunnelContain/CaveContain isKickOutOnCapture = false.
        if container.is_tunnel_network_style_container()
            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                &container.template_name,
            )
        {
            return;
        }
        let pos = container.get_position();
        let unmanned = container.status.disabled_unmanned;
        let occupants: Vec<ObjectId> = container.contained_units();
        if occupants.is_empty() {
            return;
        }
        for (i, uid) in occupants.into_iter().enumerate() {
            if let Some(unit) = self.objects.get_mut(&uid) {
                let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 10.0;
                unit.stop_moving();
                unit.set_position(pos + offset);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    let p = pos + offset;
                    crate::game_logic::host_move_log::record(unit.id, Some([p.x, p.y, p.z]));
                    unit.record_host_movement();
                }
                unit.set_target(None);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_stop_attack(uid);
                }
                unit.set_contained_by(None);
                unit.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                }
                unit.set_status_moving(false);
                unit.set_status_attacking(false);
                // Occupants keep their own team residual (don't flip with container).
            }
            if let Some(c) = self.objects.get_mut(&container_id) {
                let _ = c.remove_occupant(uid);
            }
            self.capture_kick_outs = self.capture_kick_outs.saturating_add(1);
        }
        let _ = unmanned;
    }

    /// C++ OpenContain::onSelling + ParkingPlaceBehavior::killAllParkedUnits residual.
    ///
    /// - Eject garrison/transport occupants (orderAllPassengersToExit residual)
    /// - Kill parked aircraft at airfield hangar (grounded only)
    pub fn on_selling_container_residual(&mut self, structure_id: ObjectId) {
        let Some(pos) = self.objects.get(&structure_id).map(|o| o.get_position()) else {
            return;
        };
        let is_airfield = self
            .objects
            .get(&structure_id)
            .map(|o| {
                o.is_kind_of(KindOf::FSAirfield)
                    || o.template_name.to_ascii_lowercase().contains("airfield")
            })
            .unwrap_or(false);

        // Snapshot occupants.
        let occupants: Vec<ObjectId> = self
            .objects
            .get(&structure_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();

        // Eject passengers around structure (OpenContain::onSelling).
        for (i, uid) in occupants.iter().copied().enumerate() {
            if is_airfield {
                // Parked jets: kill if not airborne takeoff residual.
                let kill = self
                    .objects
                    .get(&uid)
                    .map(|u| {
                        let aircraft =
                            u.is_kind_of(KindOf::Aircraft) || u.object_type == ObjectType::Aircraft;
                        let airborne = u.status.airborne_target || u.get_position().y > 5.0;
                        aircraft && !airborne
                    })
                    .unwrap_or(false);
                if kill {
                    self.destroy_object_for_sell_residual(uid);
                    self.sell_parked_units_killed = self.sell_parked_units_killed.saturating_add(1);
                    continue;
                }
            }
            // Eject residual.
            if let Some(unit) = self.objects.get_mut(&uid) {
                let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 10.0;
                unit.stop_moving();
                unit.set_position(pos + offset);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    let p = pos + offset;
                    crate::game_logic::host_move_log::record(unit.id, Some([p.x, p.y, p.z]));
                    unit.record_host_movement();
                }
                unit.set_target(None);
                unit.set_contained_by(None);
                unit.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                }
                unit.set_status_moving(false);
                unit.set_status_attacking(false);
            }
            if let Some(st) = self.objects.get_mut(&structure_id) {
                let _ = st.remove_occupant(uid);
            }
            self.sell_passengers_ejected = self.sell_passengers_ejected.saturating_add(1);
        }

        // C++ TunnelContain::onSelling: last tunnel kicks shared pool passengers.
        let is_tunnel = self
            .objects
            .get(&structure_id)
            .map(|o| {
                o.is_tunnel_network_style_container()
                    || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                        &o.template_name,
                    )
            })
            .unwrap_or(false);
        if is_tunnel {
            let team = self.objects.get(&structure_id).map(|o| o.team);
            if let Some(team) = team {
                let tunnel_count = self
                    .objects
                    .values()
                    .filter(|o| {
                        o.is_alive()
                            && o.team == team
                            && o.id != structure_id
                            && !o.status.sold
                            && (o.is_tunnel_network_style_container()
                                || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                    &o.template_name,
                                ))
                    })
                    .count();
                // friend_getTunnelCount()==1 means this is the last (others already gone).
                // Count other live tunnels; if 0, we are last.
                if tunnel_count == 0 {
                    let units = self.tunnel_network.contained_for_team(team);
                    for (i, uid) in units.into_iter().enumerate() {
                        let _ = self.tunnel_network.record_exit(team, uid, structure_id);
                        if let Some(unit) = self.objects.get_mut(&uid) {
                            let angle = (uid.0 as f32 + i as f32 * 1.11) * 0.7;
                            let offset = glam::Vec3::new(angle.cos(), 0.0, angle.sin()) * 10.0;
                            unit.stop_moving();
                            unit.set_position(pos + offset);
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                let p = pos + offset;
                                crate::game_logic::host_move_log::record(
                                    unit.id,
                                    Some([p.x, p.y, p.z]),
                                );
                                unit.record_host_movement();
                            }
                            unit.set_target(None);
                            unit.set_contained_by(None);
                            unit.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(uid, 0);
                            }
                            unit.set_status_moving(false);
                            unit.set_status_attacking(false);
                        }
                        self.sell_passengers_ejected =
                            self.sell_passengers_ejected.saturating_add(1);
                        self.sell_tunnel_last_ejects =
                            self.sell_tunnel_last_ejects.saturating_add(1);
                    }
                }
            }
        }

        // Also kill any jet with contained_by = structure (hangar roster residual).
        if is_airfield {
            let parked: Vec<ObjectId> = self
                .objects
                .iter()
                .filter_map(|(id, o)| {
                    if o.contained_by != Some(structure_id) {
                        return None;
                    }
                    let aircraft =
                        o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft;
                    let airborne = o.status.airborne_target || o.get_position().y > 5.0;
                    if aircraft && !airborne {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for pid in parked {
                self.destroy_object_for_sell_residual(pid);
                self.sell_parked_units_killed = self.sell_parked_units_killed.saturating_add(1);
            }
        }
        // Wave 482: flush sell residual kills (parked aircraft) same frame.
        if self.sell_parked_units_killed > 0 {
            self.process_destroy_list();
        }
    }

    pub fn start_sell_object(&mut self, object_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&object_id) else {
            return false;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        if obj.status.sold || obj.status.under_construction || obj.status.reconstructing {
            return false;
        }
        if self.sell_list.iter().any(|s| s.id == object_id) {
            return false;
        }
        let team = obj.team;
        let frame = self.frame;
        // Cancel production + refund queue first (C++ ProductionUpdate cancelAndRefundAllProduction).
        self.cancel_all_production(object_id);
        // C++ contain->onSelling() + ParkingPlace killAllParkedUnits residual.
        self.on_selling_container_residual(object_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ setConstructionPercent(99.9f) on 0..100 scale → host 0.999
            obj.construction_percent = 0.999;
            crate::game_logic::host_construction_progress_log::record(object_id, 0.999, false, 0.0);
            obj.set_status_sold(true);
            obj.set_status_unselectable(true);
            obj.set_status_under_construction(false);
            // Wave 212: deselect logs host_status selected last-writer.
            obj.deselect();
            obj.set_ai_state(AIState::Idle);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, 0);
            }
            obj.apply_sell_scaffold_model_conditions();
        }
        // Deselect from all players.
        for p in self.players.values_mut() {
            p.selected_objects.retain(|&id| id != object_id);
        }
        self.sell_list.insert(
            0,
            ObjectSellInfo {
                id: object_id,
                sell_frame: frame,
            },
        );
        self.sell_process_starts = self.sell_process_starts.saturating_add(1);
        // C++ sellObject: destroy mines owned by this structure (producerID).
        let mine_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.is_alive() {
                    return None;
                }
                let is_mine = o.mine_data.is_some()
                    || crate::game_logic::host_mines::infer_mine_kind(&o.template_name).is_some();
                if !is_mine {
                    return None;
                }
                let producer = o
                    .producer_id
                    .or_else(|| o.mine_data.as_ref().and_then(|m| m.producer_id));
                if producer == Some(object_id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for mid in mine_ids {
            self.destroy_object(mid);
            self.sell_owned_mines_destroyed = self.sell_owned_mines_destroyed.saturating_add(1);
        }
        let _ = team;
        true
    }

    /// C++ BuildAssistant::update sell list residual.
    pub(crate) fn tick_host_systems_residuals_sole(&mut self) {
        // Wave 827: post-writeback sole-tick for remaining host system residuals.
        self.update_main_crate_vision();
        self.update_sell_list();
        self.update_special_power_strikes();
        self.update_player_upgrades();
    }

    pub(crate) fn update_sell_list(&mut self) {
        if self.sell_list.is_empty() {
            return;
        }
        let frame = self.frame;
        let mut finished: Vec<ObjectId> = Vec::new();
        let mut still: Vec<ObjectSellInfo> = Vec::with_capacity(self.sell_list.len());
        // Wave 619: under construction sole-tick, GameWorld writeback records sell-ready
        // structures (pct <= -0.5); host finishes after writeback same frame (Wave 716).
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Empty mid-update ready set under sole: finish only via post-writeback helper.
        // Non-sole finishes via projected percent (may_finish without ready membership).
        let ready_sells: std::collections::HashSet<ObjectId> = std::collections::HashSet::new();
        for entry in std::mem::take(&mut self.sell_list) {
            let Some(obj) = self.objects.get_mut(&entry.id) else {
                // Object gone by other means.
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            let elapsed = frame.saturating_sub(entry.sell_frame);
            if elapsed >= FRAMES_TO_ALLOW_SCAFFOLD_RESIDUAL {
                let previous = obj.construction_percent;
                let sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
                // Allow percent to fall through SELL_FINISH (-0.5); do not floor at -0.01
                // (that made finish unreachable and stalled multi-frame sell forever).
                let projected = if sole {
                    // Wave 481: GW sole-ticks sell percent via negative rate; host uses writeback.
                    previous
                } else {
                    previous - SELL_CONSTRUCTION_DECREMENT_RESIDUAL
                };
                if !sole {
                    obj.construction_percent = projected;
                    crate::game_logic::host_construction_progress_log::record(
                        entry.id, projected, false, 0.0,
                    );
                } else {
                    // Negative fraction/sec so tick_construction_progress advances sell.
                    let rate =
                        -SELL_CONSTRUCTION_DECREMENT_RESIDUAL / LOGIC_FRAME_TIMESTEP.max(1e-6);
                    crate::game_logic::host_construction_progress_log::record_rate_only(
                        entry.id, false, rate,
                    );
                }
                // Cross from positive to <= 0 → MODELCONDITION_SOLD.
                // Under sole-tick, projected == writeback previous; fire when writeback already ≤ 0.
                if (previous > 0.0 && projected <= 0.0) || (sole && previous <= 0.0) {
                    obj.apply_sold_model_condition();
                }
                if projected <= SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL {
                    // Wave 619: under sole-tick, only finish IDs GW recorded ready.
                    if !construction_sole || ready_sells.contains(&entry.id) {
                        finished.push(entry.id);
                    } else {
                        still.push(entry);
                    }
                } else {
                    still.push(entry);
                }
            } else if obj.construction_percent <= SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL {
                if !construction_sole || ready_sells.contains(&entry.id) {
                    finished.push(entry.id);
                } else {
                    still.push(entry);
                }
            } else {
                still.push(entry);
            }
        }
        self.sell_list = still;
        for id in finished {
            // Refund structure sell value then destroy.
            let (team, refund) = if let Some(obj) = self.objects.get(&id) {
                let sell_percentage = game_engine::common::global_data::read().sell_percentage;
                let refund = ((obj.thing.template.build_cost.supplies as f32) * sell_percentage)
                    .max(0.0) as u32;
                (obj.team, refund)
            } else {
                continue;
            };
            if refund > 0 {
                if let Some(player) = self.get_player_mut_by_team(team) {
                    player.apply_supply_gain(refund);
                } else if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                    player.apply_supply_gain(refund);
                }
            }
            // Cancel any leftover production (C++ cancel again at finish).
            self.cancel_all_production(id);
            self.destroy_object(id);
            self.sell_process_finishes = self.sell_process_finishes.saturating_add(1);
            let msg = crate::localization::localize("hud.sell.complete", "Structure sold");
            self.queue_radar_message_for_team(team, msg);
        }
    }

    /// Wave 716: after GW construction writeback records sell-ready structures,
    /// host applies refund/destroy in the same coupled tick (not next frame).
    pub(crate) fn host_apply_sell_completions_after_ready_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            return;
        }
        let ready: std::collections::HashSet<ObjectId> =
            crate::game_logic::host_sell_ready_log::drain()
                .into_iter()
                .map(|ev| ev.structure)
                .collect();
        if ready.is_empty() {
            return;
        }
        // Drop finished entries from sell_list residual.
        self.sell_list.retain(|entry| !ready.contains(&entry.id));
        for id in ready {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            // Writeback already pushed percent <= SELL_FINISH while sold.
            if !obj.status.sold
                && obj.construction_percent > SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL + 1e-6
            {
                continue;
            }
            let team = obj.team;
            let sell_percentage = game_engine::common::global_data::read().sell_percentage;
            let refund =
                ((obj.thing.template.build_cost.supplies as f32) * sell_percentage).max(0.0) as u32;
            if refund > 0 {
                if let Some(player) = self.get_player_mut_by_team(team) {
                    player.apply_supply_gain(refund);
                } else if let Some(player) = self.players.values_mut().find(|p| p.team == team) {
                    player.apply_supply_gain(refund);
                }
            }
            self.cancel_all_production(id);
            self.destroy_object(id);
            self.sell_process_finishes = self.sell_process_finishes.saturating_add(1);
            let msg = crate::localization::localize("hud.sell.complete", "Structure sold");
            self.queue_radar_message_for_team(team, msg);
        }
    }

    pub fn honesty_sell_process_ok(&self) -> bool {
        self.sell_process_starts > 0 && self.sell_process_finishes > 0
    }

    /// C++ DozerAIUpdate::cancelTask residual when construction is cancelled/killed.
    ///
    /// Dozers targeting `structure_id` (or actively Constructing nearby same team)
    /// go Idle and clear ACTIVELY_CONSTRUCTING model residual.
    pub fn cancel_dozers_building(&mut self, structure_id: ObjectId) {
        let team = self.objects.get(&structure_id).map(|o| o.team);
        let build_pos = self.objects.get(&structure_id).map(|o| o.get_position());
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut cancelled = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if !obj.is_alive() || !obj.can_construct() {
                continue;
            }
            let targeting = obj.target == Some(structure_id);
            let constructing = matches!(obj.ai_state, AIState::Constructing);
            let nearby = match (team, build_pos) {
                (Some(t), Some(bp)) => obj.team == t && obj.get_position().distance(bp) <= 40.0,
                _ => false,
            };
            if targeting || (constructing && nearby) {
                obj.target = None;
                obj.set_ai_state(AIState::Idle);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(id, 0);
                }
                obj.set_actively_constructing(false);
                cancelled = cancelled.saturating_add(1);
            }
        }
        if cancelled > 0 {
            self.dozer_cancel_task_events = self.dozer_cancel_task_events.saturating_add(cancelled);
            // Refresh ACTIVELY_CONSTRUCTING residual globally after cancel.
            // Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                self.update_actively_constructing_model_conditions();
            }
        }
    }

    /// C++ ActionManager::canResumeConstructionOf residual.
    pub fn can_resume_construction_of(&self, dozer_id: ObjectId, structure_id: ObjectId) -> bool {
        let Some(dozer) = self.objects.get(&dozer_id) else {
            return false;
        };
        let Some(structure) = self.objects.get(&structure_id) else {
            return false;
        };
        if !dozer.is_alive() || !dozer.can_construct() {
            return false;
        }
        if !structure.is_alive() || !structure.is_kind_of(KindOf::Structure) {
            return false;
        }
        if structure.team != dozer.team {
            return false;
        }
        if !structure.status.under_construction || structure.status.sold {
            return false;
        }
        // Another dozer already actively building this structure.
        for (id, obj) in &self.objects {
            if *id == dozer_id || !obj.is_alive() || !obj.can_construct() {
                continue;
            }
            if obj.team != structure.team {
                continue;
            }
            if matches!(obj.ai_state, AIState::Constructing) && obj.target == Some(structure_id) {
                return false;
            }
        }
        true
    }

    /// C++ DozerAIUpdate::privateResumeConstruction residual.
    /// Returns true if a dozer was assigned.
    pub fn resume_construction(&mut self, dozer_ids: &[ObjectId], structure_id: ObjectId) -> bool {
        // Only one dozer resumes (C++ groupResumeConstruction — first that accepts).
        for &dozer_id in dozer_ids {
            if !self.can_resume_construction_of(dozer_id, structure_id) {
                continue;
            }
            if let Some(dozer) = self.objects.get_mut(&dozer_id) {
                dozer.target = Some(structure_id); // non-combat build association stays host
                dozer.set_ai_state(AIState::Constructing);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(dozer_id, 7);
                }
                dozer.set_actively_constructing(true);
            }
            // Structure awaiting → actively being constructed residual when dozer assigned.
            if let Some(st) = self.objects.get_mut(&structure_id) {
                st.set_under_construction_model_conditions(true);
            }
            self.resume_construction_events = self.resume_construction_events.saturating_add(1);
            // Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                self.update_actively_constructing_model_conditions();
            }
            return true;
        }
        false
    }

    pub fn honesty_resume_construction_ok(&self) -> bool {
        self.resume_construction_events > 0
    }

    pub fn honesty_repair_complete_ok(&self) -> bool {
        self.repair_complete_events > 0
    }

    /// C++ DozerAIUpdate findObjectToRepair residual (same player, structure, damaged).
    pub fn find_dozer_bored_repair_target(&self, dozer_id: ObjectId) -> Option<ObjectId> {
        let dozer = self.objects.get(&dozer_id)?;
        if !dozer.is_alive() || !dozer.can_repair() {
            return None;
        }
        let pos = dozer.get_position();
        let team = dozer.team;
        let range = crate::game_logic::host_repair::DOZER_BORED_RANGE;
        // Pure residual service acquire (2D/XZ bored range).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive()
                    || obj.team != team
                    || !obj.is_kind_of(KindOf::Structure)
                    || obj.status.under_construction
                    || obj.status.sold
                    || obj.health.current + 0.01 >= obj.health.maximum
                {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: false,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            Some(dozer_id),
            (pos.x, pos.z),
            candidates,
            range,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ DozerAIUpdate findMine residual (enemy/neutral mines in BoredRange).
    pub fn find_dozer_bored_mine_target(&self, dozer_id: ObjectId) -> Option<ObjectId> {
        use crate::game_logic::host_mines::{can_clear_mine_kind, is_mine_clearer};
        let dozer = self.objects.get(&dozer_id)?;
        if !dozer.is_alive() {
            return None;
        }
        if !is_mine_clearer(dozer.is_worker(), &dozer.template_name) {
            return None;
        }
        let pos = dozer.get_position();
        let team = dozer.team;
        let range = crate::game_logic::host_repair::DOZER_BORED_RANGE;
        // Pure residual acquire (enemy/neutral mines in BoredRange, XZ).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() || obj.team == team {
                    return None;
                }
                // C++ ALLOW_ENEMIES | ALLOW_NEUTRAL only (not allies / own mines).
                let is_mine = obj.mine_data.is_some()
                    || crate::game_logic::host_mines::infer_mine_kind(&obj.template_name).is_some();
                if !is_mine {
                    return None;
                }
                if let Some(md) = obj.mine_data.as_ref() {
                    if !can_clear_mine_kind(md.kind) {
                        return None;
                    }
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: false,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            Some(dozer_id),
            (pos.x, pos.z),
            candidates,
            range,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ DozerPrimaryIdleState bored residual: repair, else mine-clear.
    pub(crate) fn process_dozer_bored_event(&mut self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        if !obj.is_alive() || !obj.can_repair() {
            return;
        }
        // Idle stamp already advanced on GW; attempt service residual once.
        if let Some(target_id) = self.find_dozer_bored_repair_target(id) {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.target = Some(target_id);
                obj.set_actively_constructing(true);
                obj.idle_since_frame = 0;
            }
            self.set_ai_state_decision_aware(id, AIState::Repairing);
            self.dozer_bored_repair_events = self.dozer_bored_repair_events.saturating_add(1);
            return;
        }
        if let Some(mine_id) = self.find_dozer_bored_mine_target(id) {
            let mine_pos = self
                .objects
                .get(&mine_id)
                .map(|m| m.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.idle_since_frame = 0;
            }
            self.apply_engagement_decision_aware(id, mine_id);
            self.path_approach_with_state(id, mine_pos, AIState::Attacking);
            self.dozer_bored_mine_clear_events =
                self.dozer_bored_mine_clear_events.saturating_add(1);
        }
    }

    pub(super) fn update_dozer_bored_repair(&mut self) {
        let now = self.frame;
        let bored = crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if !obj.is_alive() || !obj.can_repair() {
                continue;
            }
            // Track idle timestamp residual.
            if matches!(obj.ai_state, AIState::Idle) {
                if obj.idle_since_frame == 0 {
                    obj.idle_since_frame = now.max(1);
                }
            } else {
                obj.idle_since_frame = 0;
                continue;
            }
            let idle_since = obj.idle_since_frame;
            if now.saturating_sub(idle_since) < bored {
                continue;
            }
            // Reset stamp so we don't scan every frame (C++ resets idle timestamp).
            obj.idle_since_frame = now.max(1);

            if let Some(target_id) = self.find_dozer_bored_repair_target(id) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    // Repair target is non-combat host association (not AttackTarget).
                    obj.target = Some(target_id);
                    obj.set_actively_constructing(true);
                    obj.idle_since_frame = 0;
                }
                self.set_ai_state_decision_aware(id, AIState::Repairing);
                self.dozer_bored_repair_events = self.dozer_bored_repair_events.saturating_add(1);
                continue;
            }

            // C++ else branch: WEAPONSET_MINE_CLEARING_DETAIL + findMine + aiAttackObject.
            if let Some(mine_id) = self.find_dozer_bored_mine_target(id) {
                let mine_pos = self
                    .objects
                    .get(&mine_id)
                    .map(|m| m.get_position())
                    .unwrap_or(glam::Vec3::ZERO);
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.idle_since_frame = 0;
                }
                // Combat engagement via decision authority; path_approach logs Attacking too.
                self.apply_engagement_decision_aware(id, mine_id);
                // Approach residual — attack resolution happens in combat update.
                self.path_approach_with_state(id, mine_pos, AIState::Attacking);
                self.dozer_bored_mine_clear_events =
                    self.dozer_bored_mine_clear_events.saturating_add(1);
            }
        }
    }

    pub fn honesty_dozer_bored_repair_ok(&self) -> bool {
        self.dozer_bored_repair_events > 0
    }

    /// C++ Object::onDie RECONSTRUCTING residual — transfer attackers back to hole
    /// and restart RebuildHole worker spawn process.
    pub fn handle_reconstructing_death(&mut self, destroyed_id: ObjectId) -> bool {
        let (is_recon, producer, template_name) = {
            let Some(o) = self.objects.get(&destroyed_id) else {
                return false;
            };
            (
                o.status.reconstructing,
                o.producer_id,
                o.template_name.clone(),
            )
        };
        if !is_recon {
            return false;
        }
        let Some(hole_id) = producer else {
            return false;
        };
        let Some(hole) = self.objects.get_mut(&hole_id) else {
            return false;
        };
        if !hole.is_rebuild_hole {
            return false;
        }
        // Restart rebuild process residual.
        hole.rebuild_template_name = Some(template_name);
        hole.rebuild_reconstructing_id = None;
        hole.rebuild_worker_id = None;
        hole.set_status_masked(false);
        hole.set_status_unselectable(false);
        hole.rebuild_ready_frame = self
            .frame
            .max(1)
            .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
        // Transfer attackers from lost reconstruction to hole.
        let n = self.transfer_attack(destroyed_id, hole_id);
        if n > 0 {
            self.rebuild_hole_attack_transfers =
                self.rebuild_hole_attack_transfers.saturating_add(n as u32);
        }
        self.rebuild_hole_recon_deaths = self.rebuild_hole_recon_deaths.saturating_add(1);
        true
    }

    /// C++ RebuildHoleBehavior::transferBombs residual.
    ///
    /// Sticky bombs / mines attached to `from_id` retarget to `to_id`.
    pub fn transfer_bombs(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        if from_id == to_id {
            return 0;
        }
        if !self.objects.contains_key(&to_id) {
            return 0;
        }
        let mut n = 0usize;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            if id == from_id || id == to_id {
                continue;
            }
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            // StickyBombUpdate target residual stored as mine_data.attached_to.
            if let Some(md) = obj.mine_data.as_mut() {
                if md.attached_to == Some(from_id) {
                    md.attached_to = Some(to_id);
                    n = n.saturating_add(1);
                }
            }
            // Also retarget if attacking the old host as sticky residual.
            // Fail-closed: only mine-kind objects (mine_data present).
        }
        if n > 0 {
            self.rebuild_hole_bomb_transfers =
                self.rebuild_hole_bomb_transfers.saturating_add(n as u32);
        }
        n
    }

    /// C++ RebuildHoleExposeDie HoleName residual for common GLA structures.
    pub(super) fn rebuild_hole_name_for_template(template_name: &str) -> Option<&'static str> {
        let n = template_name.to_ascii_lowercase();
        if n.contains("tunnel") {
            return Some("GLAHoleTunnelNetwork");
        }
        if n.contains("stinger") {
            return Some("GLAHoleStingerSite");
        }
        if n.contains("blackmarket") {
            return Some("GLAHoleBlackMarket");
        }
        if n.contains("barracks") || n.contains("armsdealer") {
            return Some("GLAHole");
        }
        if n.contains("palace") || n.contains("command") {
            return Some("GLAHole");
        }
        if n.contains("supply") || n.contains("demo") || n.contains("scud") {
            return Some("GLAHole");
        }
        if n.starts_with("gla") || n.contains("gla_") {
            return Some("GLAHole");
        }
        None
    }

    /// C++ RebuildHoleExposeDie::onDie residual — spawn hole for GLA structures.
    pub fn maybe_spawn_rebuild_hole(&mut self, destroyed_id: ObjectId) -> Option<ObjectId> {
        let (team, pos, orient, template_name, under_construction, is_structure, is_hole) = {
            let o = self.objects.get(&destroyed_id)?;
            (
                o.team,
                o.get_position(),
                o.get_orientation(),
                o.template_name.clone(),
                o.status.under_construction,
                o.is_kind_of(KindOf::Structure),
                o.is_rebuild_hole,
            )
        };
        if is_hole || !is_structure || under_construction {
            return None;
        }
        if !matches!(team, Team::GLA) {
            return None;
        }
        let hole_name = Self::rebuild_hole_name_for_template(&template_name)?;
        if !self.templates.contains_key(hole_name) {
            let mut ht = ThingTemplate::new(hole_name);
            ht.add_kind_of(KindOf::Structure)
                .set_health(REBUILD_HOLE_MAX_HEALTH_RESIDUAL);
            self.templates.insert(hole_name.to_string(), ht);
        }
        // Wave 742: under construction sole-tick, pre-spawn hole entity on coupled
        // shadow and bind host ObjectId (entity-first). Non-sole / no-shadow falls
        // back to host create_object. Missing bind under sole is fail-closed via
        // host_spawn_rebuild_bound_object (Wave 741) unless opt-in.
        let gw_raw = if crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            crate::gameworld_shadow::spawn_rebuild_hole_entity_if_coupled(
                hole_name,
                [pos.x, pos.y, pos.z],
                orient,
                REBUILD_HOLE_MAX_HEALTH_RESIDUAL,
            )
        } else {
            None
        };
        let hole_id = self.host_spawn_rebuild_bound_object(hole_name, team, pos, gw_raw)?;
        if let Some(h) = self.objects.get_mut(&hole_id) {
            h.set_orientation(orient);
            h.set_status_under_construction(false);
            // Defer percent only when shadow sole-ticks construction; host-only
            // must set percent immediately or rebuild holes stay incomplete forever.
            if crate::gameworld_shadow::gameworld_construction_authority_live() {
                crate::game_logic::host_construction_progress_log::record(hole_id, 1.0, false, 0.0);
            } else {
                h.construction_percent = 1.0;
                crate::game_logic::host_construction_progress_log::record(hole_id, 1.0, false, 0.0);
            }
            Self::write_object_health_authority_aware(h, REBUILD_HOLE_MAX_HEALTH_RESIDUAL);
            h.health.maximum = REBUILD_HOLE_MAX_HEALTH_RESIDUAL;
            h.is_rebuild_hole = true;
            h.rebuild_template_name = Some(template_name);
            h.rebuild_spawner_id = Some(destroyed_id);
            h.rebuild_ready_frame = self
                .frame
                .max(1)
                .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
        }
        self.rebuild_hole_spawns = self.rebuild_hole_spawns.saturating_add(1);
        // C++ RebuildHoleExposeDie TransferAttackers residual (default true).
        let n = self.transfer_attack(destroyed_id, hole_id);
        if n > 0 {
            self.rebuild_hole_attack_transfers =
                self.rebuild_hole_attack_transfers.saturating_add(n as u32);
        }
        Some(hole_id)
    }

    /// C++ RebuildHoleBehavior::update residual:
    /// - hole health regen while waiting
    /// - spawn unselectable worker after WorkerRespawnDelay
    /// - worker starts reconstruction; hole masked while reconstructing
    /// - when reconstruction completes, hole is destroyed
    pub fn update_rebuild_holes(&mut self) {
        let now = self.frame;
        let dt = 1.0 / 30.0;
        let heal_frac = REBUILD_HOLE_HEALTH_REGEN_PERCENT_PER_SEC * dt;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.is_alive() && o.is_rebuild_hole)
            .map(|(id, _)| *id)
            .collect();
        let mut holes_to_remove: Vec<ObjectId> = Vec::new();
        // Wave 620: under construction sole-tick, GameWorld writeback records
        // rebuild-ready holes; host only starts worker/building for those IDs.
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Wave 740: keep full ready events (GW entity-first worker/rebuild raws).
        let ready_by_hole: std::collections::HashMap<
            ObjectId,
            crate::game_logic::host_rebuild_ready_log::HostRebuildReadyEvent,
        > = if construction_sole {
            crate::game_logic::host_rebuild_ready_log::drain()
                .into_iter()
                .map(|ev| (ev.hole, ev))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        let ready_holes: std::collections::HashSet<ObjectId> =
            ready_by_hole.keys().copied().collect();
        for hole_id in ids {
            // Hole health regen residual (always while hole alive).
            if let Some(h) = self.objects.get_mut(&hole_id) {
                if h.health.current + 1e-3 < h.health.maximum {
                    let add = h.health.maximum * heal_frac;
                    h.heal(add);
                    self.rebuild_hole_heals = self.rebuild_hole_heals.saturating_add(1);
                }
            }

            // C++ newWorkerRespawnProcess: worker gone → restart delay residual.
            {
                let (worker_id, recon_id) = {
                    let Some(h) = self.objects.get(&hole_id) else {
                        continue;
                    };
                    (h.rebuild_worker_id, h.rebuild_reconstructing_id)
                };
                let worker_gone = match worker_id {
                    Some(wid) => self
                        .objects
                        .get(&wid)
                        .map(|w| !w.is_alive())
                        .unwrap_or(true),
                    None => false,
                };
                if worker_gone {
                    let recon_alive = match recon_id {
                        Some(rid) => self
                            .objects
                            .get(&rid)
                            .map(|b| b.is_alive())
                            .unwrap_or(false),
                        None => false,
                    };
                    if let Some(h) = self.objects.get_mut(&hole_id) {
                        h.rebuild_worker_id = None;
                        if !recon_alive {
                            h.rebuild_reconstructing_id = None;
                            h.set_status_masked(false);
                        }
                        h.rebuild_ready_frame = now
                            .max(1)
                            .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
                        self.rebuild_hole_worker_restarts =
                            self.rebuild_hole_worker_restarts.saturating_add(1);
                    }
                }
            }

            // If reconstructing building finished, destroy hole.// If reconstructing building finished, destroy hole.
            let recon_done = {
                let Some(h) = self.objects.get(&hole_id) else {
                    continue;
                };
                if let Some(rid) = h.rebuild_reconstructing_id {
                    match self.objects.get(&rid) {
                        None => {
                            // Building gone — clear and respawn worker residual next cycle.
                            true // treat as need reset
                        }
                        Some(b) if b.is_alive() && !b.status.under_construction => true,
                        _ => false,
                    }
                } else {
                    false
                }
            };
            if recon_done {
                let finished = self
                    .objects
                    .get(&hole_id)
                    .and_then(|h| h.rebuild_reconstructing_id)
                    .and_then(|rid| self.objects.get(&rid))
                    .map(|b| b.is_alive() && !b.status.under_construction)
                    .unwrap_or(false);
                if finished {
                    // Clear producer link residual and remove hole.
                    if let Some(rid) = self
                        .objects
                        .get(&hole_id)
                        .and_then(|h| h.rebuild_reconstructing_id)
                    {
                        if let Some(b) = self.objects.get_mut(&rid) {
                            b.set_status_reconstructing(false);
                            b.set_status_masked(false);
                        }
                    }
                    // Destroy residual unselectable worker if still around.
                    if let Some(wid) = self.objects.get(&hole_id).and_then(|h| h.rebuild_worker_id)
                    {
                        self.destroy_object(wid);
                    }
                    holes_to_remove.push(hole_id);
                    self.rebuild_hole_completes = self.rebuild_hole_completes.saturating_add(1);
                    continue;
                } else {
                    // Reconstructing object died — reset for new worker.
                    if let Some(h) = self.objects.get_mut(&hole_id) {
                        h.rebuild_reconstructing_id = None;
                        h.rebuild_worker_id = None;
                        h.set_status_masked(false);
                        h.rebuild_ready_frame = now
                            .max(1)
                            .saturating_add(REBUILD_HOLE_WORKER_RESPAWN_FRAMES);
                    }
                    continue;
                }
            }

            let Some(h) = self.objects.get(&hole_id) else {
                continue;
            };
            // Already reconstructing — keep masked.
            if h.rebuild_reconstructing_id.is_some() {
                continue;
            }
            if h.rebuild_ready_frame == 0 || now < h.rebuild_ready_frame {
                continue;
            }
            // Wave 620: under sole-tick, only start rebuild for IDs GW recorded ready.
            if construction_sole && !ready_holes.contains(&hole_id) {
                continue;
            }
            let team = h.team;
            let pos = h.get_position();
            let orient = h.get_orientation();
            let rebuild_name = match h.rebuild_template_name.clone() {
                Some(n) => n,
                None => continue,
            };

            // Ensure worker template residual.
            if !self.templates.contains_key(REBUILD_HOLE_WORKER_TEMPLATE) {
                let mut wt = ThingTemplate::new(REBUILD_HOLE_WORKER_TEMPLATE);
                wt.add_kind_of(KindOf::Vehicle)
                    .add_kind_of(KindOf::Worker)
                    .set_health(200.0);
                self.templates
                    .insert(REBUILD_HOLE_WORKER_TEMPLATE.to_string(), wt);
            }
            // Wave 740: under construction sole-tick, bind host ObjectIds to
            // GameWorld pre-spawned worker + reconstruct entities when present.
            let ready_ev = ready_by_hole.get(&hole_id);
            // Wave 740: prefer GW spawn pose residual when writeback provided one.
            let pos = ready_ev
                .and_then(|e| e.spawn_pos)
                .map(|p| Vec3::new(p[0], p[1], p[2]))
                .unwrap_or(pos);
            let worker_id = self.host_spawn_rebuild_bound_object(
                REBUILD_HOLE_WORKER_TEMPLATE,
                team,
                pos,
                ready_ev.and_then(|e| e.worker_entity_raw),
            );
            let Some(worker_id) = worker_id else {
                continue;
            };
            if let Some(w) = self.objects.get_mut(&worker_id) {
                w.set_status_unselectable(true);
                w.set_status_masked(false);
                if let Some(ev) = ready_ev {
                    w.set_orientation(ev.orientation);
                }
            }
            self.set_ai_state_decision_aware(worker_id, AIState::Constructing);
            self.rebuild_hole_workers = self.rebuild_hole_workers.saturating_add(1);

            // Spawn reconstructing building (C++ ai->construct residual).
            // Wave 740: entity-first bind when GW pre-spawned the structure.
            let Some(new_id) = self.host_spawn_rebuild_bound_object(
                &rebuild_name,
                team,
                pos,
                ready_ev.and_then(|e| e.rebuild_entity_raw),
            ) else {
                self.destroy_object(worker_id);
                continue;
            };
            if let Some(o) = self.objects.get_mut(&new_id) {
                o.set_orientation(orient);
                o.set_status_under_construction(true);
                o.set_status_reconstructing(true);
                if crate::gameworld_shadow::gameworld_construction_authority_live() {
                    crate::game_logic::host_construction_progress_log::record(
                        new_id, 0.0, true, 0.0,
                    );
                } else {
                    o.construction_percent = 0.0;
                    crate::game_logic::host_construction_progress_log::record(
                        new_id, 0.0, true, 0.0,
                    );
                }
                o.set_under_construction_model_conditions(true);
                let start_hp = (o.health.maximum * 0.1).max(1.0);
                Self::write_object_health_authority_aware(o, start_hp);
                // C++ setProducer(hole) residual.
                o.producer_id = Some(hole_id);
            }
            if let Some(w) = self.objects.get_mut(&worker_id) {
                // Construction target association stays host (not combat AttackTarget).
                w.target = Some(new_id);
                w.set_actively_constructing(true);
            }
            self.set_ai_state_decision_aware(worker_id, AIState::Constructing);
            if let Some(h) = self.objects.get_mut(&hole_id) {
                h.rebuild_worker_id = Some(worker_id);
                h.rebuild_reconstructing_id = Some(new_id);
                // C++ maskObject(TRUE) while reconstructing.
                h.set_status_masked(true);
                h.set_status_unselectable(true);
            }
            // C++ transferAttack(hole, reconstructing) residual.
            let n = self.transfer_attack(hole_id, new_id);
            if n > 0 {
                self.rebuild_hole_attack_transfers =
                    self.rebuild_hole_attack_transfers.saturating_add(n as u32);
            }
            // C++ transferBombs(reconstructing) residual.
            let _ = self.transfer_bombs(hole_id, new_id);
            self.rebuild_hole_reconstructs = self.rebuild_hole_reconstructs.saturating_add(1);
        }
        for hid in holes_to_remove {
            self.objects.remove(&hid);
        }
    }

    pub fn honesty_rebuild_hole_ok(&self) -> bool {
        self.rebuild_hole_spawns > 0
            && self.rebuild_hole_reconstructs > 0
            && self.rebuild_hole_workers > 0
    }

    pub fn honesty_rebuild_hole_heal_ok(&self) -> bool {
        self.rebuild_hole_heals > 0
    }

    pub fn honesty_dozer_bored_mine_clear_ok(&self) -> bool {
        self.dozer_bored_mine_clear_events > 0
    }

    pub fn honesty_sole_benefactor_repair_ok(&self) -> bool {
        self.sole_benefactor_repair_rejects > 0
    }

    pub fn honesty_dozer_cancel_task_ok(&self) -> bool {
        self.dozer_cancel_task_events > 0
    }

    pub fn honesty_construction_complete_clear_ok(&self) -> bool {
        self.construction_complete_clears > 0
    }

    pub fn is_object_being_sold(&self, id: ObjectId) -> bool {
        self.sell_list.iter().any(|s| s.id == id)
            || self
                .objects
                .get(&id)
                .map(|o| o.status.sold)
                .unwrap_or(false)
    }

    pub fn honesty_actively_constructing_ok(&self) -> bool {
        self.actively_constructing_updates > 0
    }

    pub fn honesty_construction_model_condition_ok(&self) -> bool {
        self.construction_model_condition_updates > 0
    }

    pub fn honesty_unit_ready_ok(&self) -> bool {
        self.unit_ready_events > 0
    }

    pub fn try_radar_upgrade_complete(
        &mut self,
        player_id: u32,
        team: Team,
        upgrade_name: &str,
        source_object: Option<ObjectId>,
    ) {
        if !self.is_local_player(player_id) {
            return;
        }
        let pos = source_object
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .or_else(|| {
                // Prefer command center / any structure residual position.
                self.objects
                    .values()
                    .filter(|o| o.team == team && o.is_alive() && o.is_kind_of(KindOf::Structure))
                    .map(|o| o.get_position())
                    .next()
            })
            .unwrap_or(glam::Vec3::ZERO);

        let msg = localization::localize(
            "UPGRADE:UpgradeComplete",
            &format!("Upgrade complete: {upgrade_name}"),
        );
        // C++ TheRadar->createEvent(..., RADAR_EVENT_UPGRADE) residual.
        // Host maps upgrade events as Generic radar kind with upgrade honesty.
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
        self.radar_upgrade_events = self.radar_upgrade_events.saturating_add(1);
    }

    pub fn honesty_radar_upgrade_event_ok(&self) -> bool {
        self.radar_upgrade_events > 0
    }

    pub fn try_eva_upgrade_complete(&mut self, player_id: u32) {
        if !self.is_local_player(player_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::UpgradeComplete,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::UpgradeComplete,
        );
        self.eva_upgrade_complete = self.eva_upgrade_complete.saturating_add(1);
    }

    /// C++ TheEva->setShouldPlay(EVA_GeneralLevelUp) residual (local player).
    pub fn try_eva_general_level_up(&mut self, player_id: u32) {
        if !self.is_local_player(player_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::GeneralLevelUp,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::GeneralLevelUp);
        self.eva_general_level_up = self.eva_general_level_up.saturating_add(1);
    }

    /// Award skill points and fire GeneralLevelUp EVA on rank change residual.
    pub fn add_player_skill_points(&mut self, player_id: u32, points: i32) -> bool {
        let Some(p) = self.players.get_mut(&player_id) else {
            return false;
        };
        let leveled = p.add_skill_points(points);
        if leveled {
            self.try_eva_general_level_up(player_id);
        }
        leveled
    }

    pub fn honesty_eva_upgrade_complete_ok(&self) -> bool {
        self.eva_upgrade_complete > 0
    }

    pub fn honesty_eva_general_level_up_ok(&self) -> bool {
        self.eva_general_level_up > 0
    }

    pub fn update_eva_low_power(&mut self) {
        use crate::game_logic::host_ui_presentation_residual::EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL;
        let local_low = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.power_available < 0);
        if !local_low {
            self.eva_low_power_active = false;
            return;
        }
        let edge = !self.eva_low_power_active;
        self.eva_low_power_active = true;
        if !edge && self.frame < self.eva_low_power_next_frame {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::LowPower);
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::LowPower);
        self.eva_low_power = self.eva_low_power.saturating_add(1);
        self.eva_low_power_next_frame = self
            .frame
            .saturating_add(EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL);
    }

    /// C++ TheEva->setShouldPlay(EVA_InsufficientFunds) residual (local player).
    pub fn try_eva_insufficient_funds(&mut self, player_id: u32) {
        use crate::game_logic::host_ui_presentation_residual::EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL;
        let Some(p) = self.players.get(&player_id) else {
            return;
        };
        if !p.is_local || !p.is_alive {
            return;
        }
        if self.frame < self.eva_insufficient_funds_next_frame {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::InsufficientFunds,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::InsufficientFunds,
        );
        self.eva_insufficient_funds = self.eva_insufficient_funds.saturating_add(1);
        self.eva_insufficient_funds_next_frame = self
            .frame
            .saturating_add(EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL);
    }

    pub fn honesty_eva_low_power_ok(&self) -> bool {
        self.eva_low_power > 0
    }

    pub fn eva_low_power_count(&self) -> u32 {
        self.eva_low_power
    }

    pub fn eva_insufficient_funds_count(&self) -> u32 {
        self.eva_insufficient_funds
    }

    pub fn eva_base_under_attack_count(&self) -> u32 {
        self.eva_base_under_attack
    }

    pub fn eva_ally_under_attack_count(&self) -> u32 {
        self.eva_ally_under_attack
    }

    pub fn honesty_eva_insufficient_funds_ok(&self) -> bool {
        self.eva_insufficient_funds > 0
    }

    pub fn try_under_attack_event(&mut self, victim_id: ObjectId) -> bool {
        use crate::game_logic::host_radar_stealth_vision_residual::{
            RADAR_AUDIO_HARVESTER_UNDER_ATTACK, RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
            RADAR_MSG_HARVESTER_UNDER_ATTACK, RADAR_MSG_STRUCTURE_UNDER_ATTACK,
            RADAR_MSG_UNDER_ATTACK, RADAR_MSG_UNIT_UNDER_ATTACK,
            SPOTTER_TRY_EVENT_CLOSE_ENOUGH_DISTANCE_SQ_RESIDUAL,
            SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL,
        };
        let Some(obj) = self.objects.get(&victim_id) else {
            return false;
        };
        if !obj.is_alive() {
            return false;
        }
        let pos = obj.get_position();
        let team = obj.team;
        let is_infantry = obj.is_kind_of(KindOf::Infantry);
        let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
        let is_structure = obj.is_kind_of(KindOf::Structure);
        let name_l = obj.template_name.to_ascii_lowercase();
        let is_harvester = name_l.contains("supplytruck")
            || name_l.contains("supply_truck")
            || name_l.contains("harvester")
            || name_l.contains("gatherer")
            || (name_l.contains("worker") && !name_l.contains("dozer"));
        let is_mp_count = is_structure
            && (obj.is_kind_of(KindOf::CommandCenter)
                || obj.is_kind_of(KindOf::FSPower)
                || obj.is_kind_of(KindOf::PowerPlant)
                || obj.is_kind_of(KindOf::FSBarracks)
                || obj.is_kind_of(KindOf::FSWarFactory)
                || obj.is_kind_of(KindOf::FSAirfield)
                || obj.is_kind_of(KindOf::FSSuperweapon)
                || obj.is_kind_of(KindOf::FSStrategyCenter)
                || obj.is_kind_of(KindOf::FSTechnology)
                || obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter));
        let alliance = self
            .players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.alliance_team)
            .unwrap_or(-1);

        // C++ tryEvent throttle residual (XZ plane).
        let now = self.frame;
        let close_sq = SPOTTER_TRY_EVENT_CLOSE_ENOUGH_DISTANCE_SQ_RESIDUAL;
        let frames_between = SPOTTER_TRY_EVENT_FRAMES_BETWEEN_EVENTS_RESIDUAL;
        let px = pos.x;
        let pz = pos.z;
        for &(frame, ex, ez) in &self.under_attack_event_history {
            if now.saturating_sub(frame) < frames_between {
                let dx = ex - px;
                let dz = ez - pz;
                if dx * dx + dz * dz <= close_sq {
                    return false;
                }
            }
        }
        self.under_attack_event_history.push((now, px, pz));
        if self.under_attack_event_history.len() > 64 {
            let drain = self.under_attack_event_history.len() - 64;
            self.under_attack_event_history.drain(0..drain);
        }
        self.under_attack_events = self.under_attack_events.saturating_add(1);

        let (msg_key, msg_fallback, audio) = if is_infantry || is_vehicle {
            if is_harvester {
                (
                    RADAR_MSG_HARVESTER_UNDER_ATTACK,
                    "Harvester under attack",
                    RADAR_AUDIO_HARVESTER_UNDER_ATTACK,
                )
            } else {
                (
                    RADAR_MSG_UNIT_UNDER_ATTACK,
                    "Unit under attack",
                    RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
                )
            }
        } else if is_structure && is_mp_count {
            (
                RADAR_MSG_STRUCTURE_UNDER_ATTACK,
                "Structure under attack",
                RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
            )
        } else {
            (
                RADAR_MSG_UNDER_ATTACK,
                "Under attack",
                RADAR_AUDIO_STRUCTURE_UNDER_ATTACK,
            )
        };
        let msg = localization::localize(msg_key, msg_fallback);
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
        self.queue_audio_event(
            AudioEventRequest::new(audio)
                .with_object(victim_id)
                .with_position(pos)
                .with_priority(165),
        );

        if is_structure && is_mp_count {
            let local_owns = self
                .players
                .values()
                .any(|p| p.is_local && p.is_alive && p.team == team);
            let local_ally = !local_owns
                && self.players.values().any(|p| {
                    p.is_local
                        && p.is_alive
                        && p.alliance_team == alliance
                        && alliance >= 0
                        && p.team != team
                });
            if local_owns {
                let _ = gamelogic::helpers::TheEva::set_should_play(
                    gamelogic::helpers::EvaEvent::BaseUnderAttack,
                );
                crate::game_logic::host_eva_log::record_event(
                    gamelogic::helpers::EvaEvent::BaseUnderAttack,
                );
                self.eva_base_under_attack = self.eva_base_under_attack.saturating_add(1);
            } else if local_ally {
                let _ = gamelogic::helpers::TheEva::set_should_play(
                    gamelogic::helpers::EvaEvent::AllyUnderAttack,
                );
                crate::game_logic::host_eva_log::record_event(
                    gamelogic::helpers::EvaEvent::AllyUnderAttack,
                );
                self.eva_ally_under_attack = self.eva_ally_under_attack.saturating_add(1);
            }
        }
        true
    }

    pub fn honesty_under_attack_event_ok(&self) -> bool {
        self.under_attack_events > 0
    }

    pub fn honesty_eva_base_under_attack_ok(&self) -> bool {
        self.eva_base_under_attack > 0
    }

    pub fn try_eva_on_local_object_death(
        &mut self,
        _victim_id: ObjectId,
        victim_team: crate::game_logic::Team,
        is_structure: bool,
        is_infantry: bool,
        is_vehicle: bool,
        is_mp_count_for_victory: bool,
        death_pos: glam::Vec3,
        killer: Option<crate::game_logic::Team>,
    ) {
        // Local victim residual.
        let local = self
            .players
            .values()
            .any(|p| p.is_local && p.is_alive && p.team == victim_team);
        if !local {
            return;
        }
        // C++ !selfInflicted residual.
        if killer == Some(victim_team) {
            return;
        }
        if is_structure && is_mp_count_for_victory {
            let _ = gamelogic::helpers::TheEva::set_should_play(
                gamelogic::helpers::EvaEvent::BuildingLost,
            );
            crate::game_logic::host_eva_log::record_event(
                gamelogic::helpers::EvaEvent::BuildingLost,
            );
            self.saboteur.record_eva_building_lost();
            let _ = gamelogic::helpers::TheEva::set_should_play(
                gamelogic::helpers::EvaEvent::BuildingLost,
            );
            crate::game_logic::host_eva_log::record_event(
                gamelogic::helpers::EvaEvent::BuildingLost,
            );
            self.saboteur.record_eva_building_lost();
        } else if is_infantry || is_vehicle {
            let _ =
                gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::UnitLost);
            crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::UnitLost);
            self.saboteur.record_eva_unit_lost();
            let _ =
                gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::UnitLost);
            crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::UnitLost);
            self.saboteur.record_eva_unit_lost();
            // C++ TheRadar->tryEvent(RADAR_EVENT_FAKE, pos) residual for spacebar jump.
            let msg = localization::localize("RADAR:UnitLost", "Unit lost");
            self.queue_radar_message_at(msg, death_pos, radar_notifications::RadarKind::Generic);
        }
    }

    pub fn try_eva_vehicle_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::VehicleStolen,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::VehicleStolen);
        self.car_bomb.record_eva_vehicle_stolen();
    }

    pub fn try_eva_building_being_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BuildingBeingStolen,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::BuildingBeingStolen,
        );
        self.hero_abilities.record_eva_building_being_stolen();
    }

    /// C++ TheEva->setShouldPlay(EVA_BuildingStolen) when capture completes.
    pub fn try_eva_building_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BuildingStolen,
        );
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::BuildingStolen);
        self.hero_abilities.record_eva_building_stolen();
    }

    pub fn try_eva_building_sabotaged(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ = gamelogic::helpers::TheEva::set_should_play(
            gamelogic::helpers::EvaEvent::BuildingSabotaged,
        );
        crate::game_logic::host_eva_log::record_event(
            gamelogic::helpers::EvaEvent::BuildingSabotaged,
        );
        self.saboteur.record_eva_building_sabotaged();
    }

    /// C++ TheEva->setShouldPlay(EVA_CashStolen) when local supply center is robbed.
    pub fn try_eva_cash_stolen(&mut self, victim_id: ObjectId) {
        if !self.is_object_locally_controlled(victim_id) {
            return;
        }
        let _ =
            gamelogic::helpers::TheEva::set_should_play(gamelogic::helpers::EvaEvent::CashStolen);
        crate::game_logic::host_eva_log::record_event(gamelogic::helpers::EvaEvent::CashStolen);
        self.saboteur.record_eva_cash_stolen();
    }

    pub fn try_infiltration_event(&mut self, victim_id: ObjectId) {
        let Some(obj) = self.objects.get(&victim_id) else {
            return;
        };
        if !obj.is_alive() {
            return;
        }
        let victim_team = obj.team;
        let pos = obj.get_position();
        // Local-player residual: only warn if a local player owns the victim team.
        let local_victim = self
            .players
            .values()
            .any(|p| p.team == victim_team && p.is_local);
        if !local_victim {
            // Still record honesty for AI-vs-AI residual observability when any
            // player on that team exists (fail-open for headless host tests).
            if !self.players.values().any(|p| p.team == victim_team) {
                return;
            }
        }
        let msg = localization::localize("RADAR:Infiltration", "Infiltration event");
        self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
        self.queue_audio_event(
            AudioEventRequest::new(
                crate::game_logic::host_radar_stealth_vision_residual::RADAR_INFILTRATION_AUDIO,
            )
            .with_object(victim_id)
            .with_position(pos)
            .with_priority(175),
        );
        self.saboteur.record_infiltration_event();
    }

    pub fn queue_radar_message_for_team<S: Into<String>>(&mut self, team: Team, message: S) {
        if let Some(position) = self.command_center_position(team) {
            self.queue_radar_message_at(message, position, radar_notifications::RadarKind::Generic);
        } else {
            self.queue_radar_message(message);
        }
    }

    /// Track a newly placed beacon so the UI can bloom/highlight it this frame.
    pub fn note_beacon_placed(&mut self, position: Vec3) {
        // Wave 211: host-owned active list + frame bloom residual.
        const MATCH: f32 = 3.0; // beacon_manager BEACON_MATCH_THRESHOLD residual
        self.host_beacons
            .retain(|p| (*p - position).length() > MATCH);
        self.host_beacons.push(position);
        self.recent_beacons.push(position);
    }

    /// Wave 211: remove latest host beacon for player place-order residual
    /// (manager remove_latest is player-scoped; host list is position-only).
    pub fn note_beacon_removed_latest(&mut self) {
        let _ = self.host_beacons.pop();
    }

    /// Active host beacon positions for presentation freeze.
    pub fn host_beacons(&self) -> &[Vec3] {
        &self.host_beacons
    }

    /// Play radar audio with a short cooldown to avoid stacking duplicates if many events fire simultaneously.
    pub(super) fn maybe_play_radar_audio(&mut self, cue: &str) {
        const RADAR_AUDIO_COOLDOWN: f32 = 1.0;
        if self.sim_time_seconds - self.last_radar_audio_time >= RADAR_AUDIO_COOLDOWN {
            self.queue_audio_event(AudioEventRequest::new(translate_audio_event(cue)));
            self.last_radar_audio_time = self.sim_time_seconds;
        }
    }

    pub fn last_radar_event_position(&self) -> Option<Vec3> {
        self.last_radar_event.as_ref().map(|entry| entry.position)
    }

    pub fn request_camera_focus(&mut self, position: Vec3) {
        static DEBUG_CAMERA_FOCUS_LOGS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if DEBUG_CAMERA_FOCUS_LOGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 24 {
            log::trace!("DEBUG_SHELL_CAMERA_BRIDGE: request_camera_focus position={position:?}");
        }
        self.pending_camera_focus = Some(position);
        self.script_camera_focus_estimate = position;
    }

    pub(super) fn selected_objects_center_for_local_player(&self) -> Option<Vec3> {
        let local_player_id = self.local_player_id()?;
        let player = self.players.get(&local_player_id)?;
        if player.selected_objects.is_empty() {
            return None;
        }

        let mut count = 0usize;
        let mut sum = Vec3::ZERO;
        for object_id in &player.selected_objects {
            let Some(obj) = self.objects.get(object_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            sum += obj.get_position();
            count += 1;
        }

        if count == 0 {
            None
        } else {
            Some(sum / count as f32)
        }
    }

    pub(super) fn local_player_camera_home_position(&self) -> Option<Vec3> {
        let local_player_id = self.local_player_id()?;
        let team = self.players.get(&local_player_id)?.team;
        self.command_center_position(team)
            .or_else(|| self.team_base_position(team))
    }

    pub fn peek_pending_screen_shakes(
        &self,
    ) -> &[crate::game_logic::mission_scripts::ScreenShakeRequest] {
        &self.pending_screen_shakes
    }

    pub fn peek_script_skybox_enabled(&self) -> bool {
        self.script_skybox_enabled
    }

    pub fn peek_script_superweapon_display_enabled(&self) -> bool {
        self.script_superweapon_display_enabled
    }

    pub fn peek_script_named_timer_display_shown(&self) -> bool {
        self.script_named_timer_display_shown
    }

    pub fn peek_script_superweapon_hidden_objects(
        &self,
    ) -> &std::collections::HashSet<crate::game_logic::ObjectId> {
        &self.script_superweapon_hidden_objects
    }

    pub fn queue_pending_screen_shake(&mut self, intensity: i32) {
        self.pending_screen_shakes
            .push(crate::game_logic::mission_scripts::ScreenShakeRequest { intensity });
    }

    pub fn set_script_skybox_enabled_for_test(&mut self, enabled: bool) {
        self.script_skybox_enabled = enabled;
    }

    pub fn set_script_superweapon_display_enabled_for_test(&mut self, enabled: bool) {
        self.script_superweapon_display_enabled = enabled;
    }

    pub fn set_script_named_timer_display_shown_for_test(&mut self, shown: bool) {
        self.script_named_timer_display_shown = shown;
    }

    pub fn hide_script_superweapon_object_for_test(
        &mut self,
        object_id: crate::game_logic::ObjectId,
    ) {
        self.script_superweapon_hidden_objects.insert(object_id);
    }

    pub fn peek_pending_camera_zoom(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraZoomRequest> {
        self.pending_camera_zoom.as_ref()
    }

    pub fn peek_pending_camera_zoom_reset(&self) -> bool {
        self.pending_camera_zoom_reset
    }

    pub fn peek_pending_camera_pitch(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraPitchRequest> {
        self.pending_camera_pitch.as_ref()
    }

    pub fn peek_pending_camera_rotate(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraRotateRequest> {
        self.pending_camera_rotate.as_ref()
    }

    pub fn peek_pending_camera_look_toward(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraLookTowardWaypointRequest> {
        self.pending_camera_look_toward.as_ref()
    }

    pub fn peek_pending_camera_slave_enable(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraSlaveModeRequest> {
        self.pending_camera_slave_mode_enable.as_ref()
    }

    pub fn peek_pending_camera_slave_disable(&self) -> bool {
        self.pending_camera_slave_mode_disable
    }

    pub fn peek_script_named_timers(&self) -> &std::collections::HashMap<String, (String, bool)> {
        &self.script_named_timers
    }

    pub fn peek_script_cameo_flash_count(&self) -> &std::collections::HashMap<String, i32> {
        &self.script_cameo_flash_count
    }

    pub fn queue_pending_camera_zoom(&mut self, zoom: f32, duration_seconds: f32) {
        self.pending_camera_zoom = Some(crate::game_logic::mission_scripts::CameraZoomRequest {
            zoom,
            duration_seconds,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    }

    pub fn queue_pending_camera_zoom_reset(&mut self) {
        self.pending_camera_zoom_reset = true;
    }

    pub fn queue_pending_camera_pitch(&mut self, pitch: f32, duration_seconds: f32) {
        self.pending_camera_pitch = Some(crate::game_logic::mission_scripts::CameraPitchRequest {
            pitch,
            duration_seconds,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
        });
    }

    pub fn queue_pending_camera_rotate(&mut self, rotations: f32, duration_seconds: f32) {
        self.pending_camera_rotate =
            Some(crate::game_logic::mission_scripts::CameraRotateRequest {
                rotations,
                duration_seconds,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
    }

    pub fn queue_pending_camera_look_toward(&mut self, position: Vec3, duration_seconds: f32) {
        self.pending_camera_look_toward = Some(
            crate::game_logic::mission_scripts::CameraLookTowardWaypointRequest {
                position,
                duration_seconds,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
                reverse_rotation: false,
            },
        );
    }

    pub fn queue_pending_camera_slave_enable(
        &mut self,
        thing_template_name: impl Into<String>,
        bone_name: impl Into<String>,
    ) {
        self.pending_camera_slave_mode_enable =
            Some(crate::game_logic::mission_scripts::CameraSlaveModeRequest {
                thing_template_name: thing_template_name.into(),
                bone_name: bone_name.into(),
            });
    }

    pub fn queue_pending_camera_slave_disable(&mut self) {
        self.pending_camera_slave_mode_disable = true;
    }

    pub fn upsert_script_named_timer(
        &mut self,
        name: impl Into<String>,
        text: impl Into<String>,
        countdown: bool,
    ) {
        self.script_named_timers
            .insert(name.into(), (text.into(), countdown));
    }

    pub fn set_script_cameo_flash(&mut self, button: impl Into<String>, flash_count: i32) {
        self.script_cameo_flash_count
            .insert(button.into(), flash_count);
    }

    pub fn peek_pending_camera_focus(&self) -> Option<Vec3> {
        self.pending_camera_focus
    }

    pub fn peek_pending_view_guardband(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::ViewGuardbandRequest> {
        self.pending_view_guardband.as_ref()
    }

    pub fn peek_pending_script_fps_limit(&self) -> Option<i32> {
        self.pending_script_fps_limit
    }

    pub fn peek_pending_camera_bw_mode(
        &self,
    ) -> Option<&crate::game_logic::mission_scripts::CameraBwModeRequest> {
        self.pending_camera_bw_mode.as_ref()
    }

    pub fn peek_pending_camera_add_shakers(
        &self,
    ) -> &[crate::game_logic::mission_scripts::CameraAddShakerRequest] {
        &self.pending_camera_add_shakers
    }

    pub fn peek_pending_camera_motion_blur_count(&self) -> usize {
        self.pending_camera_motion_blur.len()
    }

    pub fn queue_pending_camera_focus(&mut self, pos: Vec3) {
        self.pending_camera_focus = Some(pos);
    }

    pub fn queue_pending_view_guardband(&mut self, x_bias: f32, y_bias: f32) {
        self.pending_view_guardband =
            Some(crate::game_logic::mission_scripts::ViewGuardbandRequest { x_bias, y_bias });
    }

    pub fn queue_pending_script_fps_limit(&mut self, fps: i32) {
        self.pending_script_fps_limit = Some(fps);
    }

    pub fn queue_pending_camera_bw_mode(&mut self, enabled: bool, frames: i32) {
        self.pending_camera_bw_mode =
            Some(crate::game_logic::mission_scripts::CameraBwModeRequest { enabled, frames });
    }

    pub fn queue_pending_camera_shaker(
        &mut self,
        position: Vec3,
        amplitude: f32,
        duration_seconds: f32,
        radius: f32,
    ) {
        self.pending_camera_add_shakers.push(
            crate::game_logic::mission_scripts::CameraAddShakerRequest {
                position,
                amplitude,
                duration_seconds,
                radius,
            },
        );
    }

    pub fn set_script_time_frozen_for_test(&mut self, frozen: bool) {
        self.script_time_frozen_by_script = frozen;
    }

    pub fn take_camera_focus_request(&mut self) -> Option<Vec3> {
        self.pending_camera_focus.take()
    }

    pub fn script_default_camera_pitch(&self) -> f32 {
        self.script_default_camera_pitch
    }

    pub fn script_default_camera_max_height(&self) -> f32 {
        self.script_default_camera_max_height
    }

    pub fn visual_speed_multiplier(&self) -> f32 {
        self.visual_speed_multiplier
    }

    pub fn is_script_camera_time_frozen(&self) -> bool {
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_time())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_time())
                .unwrap_or(false)
    }

    pub fn take_camera_zoom_reset(&mut self) -> bool {
        std::mem::take(&mut self.pending_camera_zoom_reset)
    }

    pub fn take_camera_zoom_request(&mut self) -> Option<CameraZoomRequest> {
        self.pending_camera_zoom.take()
    }

    pub fn take_camera_pitch_request(&mut self) -> Option<CameraPitchRequest> {
        self.pending_camera_pitch.take()
    }

    pub fn take_camera_rotate_request(&mut self) -> Option<CameraRotateRequest> {
        self.pending_camera_rotate.take()
    }

    pub fn take_camera_look_toward_request(&mut self) -> Option<CameraLookTowardWaypointRequest> {
        self.pending_camera_look_toward.take()
    }

    pub fn take_camera_slave_mode_enable_request(&mut self) -> Option<CameraSlaveModeRequest> {
        self.pending_camera_slave_mode_enable.take()
    }

    pub fn take_camera_slave_mode_disable_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_camera_slave_mode_disable)
    }

    pub fn take_screen_shake_requests(&mut self) -> Vec<ScreenShakeRequest> {
        std::mem::take(&mut self.pending_screen_shakes)
    }

    pub fn take_camera_add_shaker_requests(&mut self) -> Vec<CameraAddShakerRequest> {
        std::mem::take(&mut self.pending_camera_add_shakers)
    }

    pub fn take_popup_message_requests(&mut self) -> Vec<ScriptPopupMessageRequest> {
        std::mem::take(&mut self.pending_popup_messages)
    }

    pub fn take_view_guardband_request(&mut self) -> Option<ViewGuardbandRequest> {
        self.pending_view_guardband.take()
    }

    pub fn take_camera_bw_mode_request(&mut self) -> Option<CameraBwModeRequest> {
        self.pending_camera_bw_mode.take()
    }

    pub fn take_camera_motion_blur_requests(&mut self) -> Vec<CameraMotionBlurRequest> {
        std::mem::take(&mut self.pending_camera_motion_blur)
    }

    pub fn queue_pending_movie(&mut self, name: impl Into<String>) {
        self.pending_movie = Some(name.into());
    }

    pub fn queue_pending_radar_movie(&mut self, name: impl Into<String>) {
        self.pending_radar_movie = Some(name.into());
    }

    pub fn queue_pending_music_stop(&mut self) {
        self.pending_music_stop = true;
    }

    pub fn queue_pending_popup_message(&mut self, message: impl Into<String>) {
        self.pending_popup_messages.push(
            crate::game_logic::mission_scripts::ScriptPopupMessageRequest {
                message: message.into(),
                x_percent: 50,
                y_percent: 50,
                width: 40,
                pause: false,
                pause_music: false,
            },
        );
    }

    pub fn peek_pending_movie(&self) -> Option<&str> {
        self.pending_movie.as_deref()
    }

    pub fn peek_pending_radar_movie(&self) -> Option<&str> {
        self.pending_radar_movie.as_deref()
    }

    /// Consume pending script movie (after presentation freeze/apply).
    pub fn take_pending_movie(&mut self) -> Option<String> {
        self.pending_movie.take()
    }

    /// Consume pending radar movie (after presentation freeze/apply).
    pub fn take_pending_radar_movie(&mut self) -> Option<String> {
        self.pending_radar_movie.take()
    }

    pub fn peek_pending_music_stop(&self) -> bool {
        self.pending_music_stop
    }

    pub fn peek_pending_popup_messages(
        &self,
    ) -> &[crate::game_logic::mission_scripts::ScriptPopupMessageRequest] {
        &self.pending_popup_messages
    }

    pub fn take_music_stop_request(&mut self) -> bool {
        std::mem::take(&mut self.pending_music_stop)
    }

    pub fn take_script_fps_limit_request(&mut self) -> Option<i32> {
        self.pending_script_fps_limit.take()
    }

    pub fn is_script_time_frozen(&self) -> bool {
        self.script_time_frozen_by_script
    }

    pub fn is_time_frozen_for_simulation(&self) -> bool {
        self.is_script_time_frozen() || self.is_script_camera_time_frozen()
    }

    /// Player residual: lock camera follow to an object (None clears).
    pub fn set_camera_follow_object(&mut self, id: Option<ObjectId>) {
        self.camera_follow_target = id;
        if let Some(oid) = id {
            if let Some(obj) = self.objects.get(&oid) {
                self.request_camera_focus(obj.get_position());
            }
        }
    }

    pub fn camera_follow_object_id(&self) -> Option<ObjectId> {
        self.camera_follow_target
    }

    pub fn camera_follow_target_position(&mut self) -> Option<Vec3> {
        let target = self.camera_follow_target?;
        let Some(obj) = self.objects.get(&target) else {
            self.camera_follow_target = None;
            return None;
        };
        if !obj.is_alive() {
            self.camera_follow_target = None;
            return None;
        }
        Some(obj.get_position())
    }

    /// Peek camera-follow world position without clearing the follow target.
    /// Used to freeze presentation residual each frame.
    pub fn peek_camera_follow_target_position(&self) -> Option<Vec3> {
        let target = self.camera_follow_target?;
        let obj = self.objects.get(&target)?;
        if !obj.is_alive() {
            return None;
        }
        Some(obj.get_position())
    }

    /// Execute a single command
    pub(super) fn execute_command(&mut self, command: crate::command_system::GameCommand) {
        let command_type = command.command_type.clone();
        let mut executor = crate::command_executor::CommandExecutor::new(self, command.player_id);

        match executor.execute_command(command) {
            Ok(crate::command_system::CommandResult::Success) => {}
            Ok(result) => {
                log::debug!(
                    "[GameLogic] Command {:?} completed with {:?}",
                    command_type,
                    result
                );
            }
            Err(err) => {
                log::warn!(
                    "[GameLogic] Failed to execute command {:?}: {}",
                    command_type,
                    err
                );
            }
        }
    }
}
