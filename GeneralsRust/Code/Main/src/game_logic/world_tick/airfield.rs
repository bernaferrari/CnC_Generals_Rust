//! Host tick `impl GameLogic` — `airfield`.
#![allow(unused_imports, non_snake_case)]
use super::super::HostHeliTakeoffOrLanding;
use super::super::*;

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct HostAirfieldPPInfo {
    parking_space: glam::Vec3,
    parking_orientation: f32,
    runway_prep: glam::Vec3,
    runway_start: glam::Vec3,
    runway_end: glam::Vec3,
    runway_approach: glam::Vec3,
    hangar_internal: glam::Vec3,
    hangar_internal_orient: f32,
    runway_takeoff_dist: f32,
}

fn std_angle_diff(a: f32, b: f32) -> f32 {
    let mut d = a - b;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

/// C++ `JetAIUpdate.cpp:499-508` taxi corner (host Y-up: XZ ground).
fn taxi_intermediate_point(info: &HostAirfieldPPInfo) -> Option<glam::Vec3> {
    let heading = (info.runway_prep.z - info.parking_space.z)
        .atan2(info.runway_prep.x - info.parking_space.x);
    if std_angle_diff(heading, info.parking_orientation).abs() <= std::f32::consts::PI / 128.0 {
        return None;
    }
    let ax = info.parking_space.x;
    let az = info.parking_space.z;
    let ao = info.parking_orientation;
    let cx = info.runway_prep.x;
    let cz = info.runway_prep.z;
    let co = info.parking_orientation + std::f32::consts::FRAC_PI_2;
    let bx = ax + ao.cos();
    let bz = az + ao.sin();
    let dx = cx + co.cos();
    let dz = cz + co.sin();
    let denom = (bx - ax) * (dz - cz) - (bz - az) * (dx - cx);
    if denom.abs() < 1.0e-6 {
        return None;
    }
    let r = ((az - cz) * (dx - cx) - (ax - cx) * (dz - cz)) / denom;
    Some(glam::Vec3::new(
        ax + r * (bx - ax),
        (info.parking_space.y + info.runway_prep.y) * 0.5,
        az + r * (bz - az),
    ))
}

fn horiz_dist_sq(a: glam::Vec3, b: glam::Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    dx * dx + dz * dz
}

/// C++ `HEAL_RATE_FRAMES` (`LOGICFRAMES_PER_SECOND / 5`).
const AIRFIELD_HEAL_RATE_FRAMES: u32 = 6;
const AIRFIELD_HEAL_FOREVER: u32 = u32::MAX;

impl GameLogic {
    /// Update combat for all objects.
    ///
    /// Fail-closed residual: uses secondary when present and selected by
    /// `Object::select_combat_weapon_slot` (prefer secondary vs structures when
    /// secondary damage is better; alternate secondary when primary not ready).
    /// Not full C++ AutoChoose / PreferredAgainst matrices.
    ///
    /// `pub(crate)` so residual/unit tests can exercise the fire path directly.
    /// C++ JetAIUpdate RETURN_TO_BASE residual: rearm empty jet weapons at
    /// a source-authored ParkingPlaceBehavior airfield.  Fail-closed versus
    /// unsupported parking/taxi matrices.
    /// C++ JetOrHeliCirclingDeadAirfieldState residual for all empty RTB jets.
    pub(crate) fn tick_out_of_ammo_jet_damage(&mut self) {
        self.tick_jet_ai_update_all();
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && (o.return_to_base_requested
                        || (o.needs_return_to_base_rearm() && Self::jet_is_idle_for_rtb(o)))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.snapshot_jet_producer_location(id);
            if self.try_return_to_base_rearm(id) {
                if let Some(jet) = self.objects.get_mut(&id) {
                    jet.leave_circling_dead_airfield();
                }
                continue;
            }
            self.tick_return_or_circle_dead_airfield(id);
        }
    }

    fn jet_is_idle_for_rtb(o: &Object) -> bool {
        matches!(o.ai_state, AIState::Idle)
            && o.target.is_none()
            && !o.hunting
            && o.guard_position.is_none()
            && o.guard_target.is_none()
    }

    /// C++ `JetAIUpdate::update` live-host residual.
    pub(crate) fn tick_jet_ai_update_all(&mut self) {
        use crate::game_logic::object::JetAiTickAction;

        let now = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                (o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft))
                    || o.jet_ai.lockon_drawable_id.is_some()
            })
            .map(|(id, _)| *id)
            .collect();
        let mut transfers: Vec<ObjectId> = Vec::new();
        for id in ids {
            let (action, lockon_tick, lockon_pos) = {
                let Some(jet) = self.objects.get_mut(&id) else {
                    continue;
                };
                let action = jet.tick_jet_ai_update(now);
                let lockon_tick = jet.take_jet_lockon_tick();
                let lockon_pos = jet.jet_ai.lockon_pos;
                if jet.jet_should_transfer_runway(now) {
                    transfers.push(id);
                }
                (action, lockon_tick, lockon_pos)
            };
            self.prune_jet_targeters(id);
            if self
                .objects
                .get(&id)
                .is_some_and(|jet| jet.jet_reached_runway_head())
            {
                self.begin_pause_after_taxi_to_runway(id);
            }
            self.sync_jet_afterburner_sound(id);
            self.sync_jet_lockon_drawable(id);

            if lockon_tick {
                let pos = lockon_pos
                    .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                    .or_else(|| self.objects.get(&id).map(|j| j.get_position()))
                    .unwrap_or(glam::Vec3::ZERO);
                // C++ `TheAudio->getMiscAudio()->m_lockonTickSound` (JetAIUpdate.cpp:2143-2145).
                let event = crate::game_logic::host_economy_log::resolve_misc_audio_event(
                    crate::game_logic::object::JET_LOCKON_TICK_SOUND,
                );
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&event)
                        .with_object(id)
                        .with_position(pos),
                );
            }
            self.maybe_play_jet_wheel_screech(id);

            match action {
                JetAiTickAction::ReturnToBase => {
                    if let Some(jet) = self.objects.get_mut(&id) {
                        jet.return_to_base_requested = true;
                    }
                    if self.try_return_to_base_rearm(id) {
                        if let Some(jet) = self.objects.get_mut(&id) {
                            jet.leave_circling_dead_airfield();
                        }
                    }
                }
                JetAiTickAction::ResumePending => {
                    if let Some(jet) = self.objects.get_mut(&id) {
                        jet.apply_pending_jet_resume();
                    }
                    if self
                        .objects
                        .get(&id)
                        .is_some_and(|j| j.is_parked_at_airfield() || j.contained_by.is_some())
                    {
                        let _ = self.try_runway_takeoff_from_airfield(id);
                    }
                }
                JetAiTickAction::None => {}
            }
        }
        for id in transfers {
            self.transfer_runway_reservation_to_next_in_line(id);
        }
    }

    /// C++ `TurretAI::removeSelfAsTargeter` / `setCurrentVictim(NULL)`.
    fn prune_jet_targeters(&mut self, jet_id: ObjectId) {
        let now = self.frame;
        let targeted = self
            .objects
            .get(&jet_id)
            .map(|jet| jet.jet_ai.targeted_by.clone())
            .unwrap_or_default();
        if targeted.is_empty() {
            return;
        }
        for tid in targeted {
            let keep = self
                .objects
                .get(&tid)
                .is_some_and(|atk| atk.is_alive() && atk.target == Some(jet_id));
            if !keep {
                if let Some(jet) = self.objects.get_mut(&jet_id) {
                    jet.add_jet_targeter(tid, false, now);
                }
            }
        }
    }

    /// C++ `friend_enableAfterburners` start/stop via TheAudio.
    fn sync_jet_afterburner_sound(&mut self, jet_id: ObjectId) {
        let (on, playing, pos, template_name) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return;
            };
            (
                jet.jet_ai.afterburners_on,
                jet.jet_ai.afterburner_sound_playing,
                jet.get_position(),
                jet.template_name.clone(),
            )
        };
        if on == playing {
            return;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.jet_ai.afterburner_sound_playing = on;
        }
        self.queue_afterburner_per_unit_sound(jet_id, &template_name, pos, on);
    }

    /// C++ `buildLockonDrawableIfNecessary` + `positionLockon` (JetAIUpdate.cpp:2098-2171).
    fn sync_jet_lockon_drawable(&mut self, jet_id: ObjectId) {
        #[cfg(not(feature = "game_client"))]
        {
            let _ = jet_id;
        }
        #[cfg(feature = "game_client")]
        {
            use crate::game_logic::object::STEALTH_FIGHTER_LOCKON_CURSOR;
            let Some(jet) = self.objects.get(&jet_id) else {
                return;
            };
            let lockon_pos = jet.jet_ai.lockon_pos;
            let lockon_hidden = jet.jet_ai.lockon_hidden;
            let existing = jet.jet_ai.lockon_drawable_id.filter(|&id| id != 0);
            let owner = jet.get_position();
            let alive = jet.is_alive();

            if !alive || lockon_pos.is_none() {
                if let Some(draw_id) = existing {
                    gamelogic::helpers::TheGameClient.destroy_drawable(draw_id);
                    if let Some(jet) = self.objects.get_mut(&jet_id) {
                        jet.jet_ai.lockon_drawable_id = None;
                    }
                }
                return;
            }
            let pos = lockon_pos.unwrap();

            let draw_id = if let Some(id) = existing {
                id
            } else {
                let Some(template) = gamelogic::helpers::TheThingFactory::find_template(
                    STEALTH_FIGHTER_LOCKON_CURSOR,
                ) else {
                    return;
                };
                let id = gamelogic::helpers::TheGameClient.create_drawable(template.as_ref());
                if id == 0 {
                    return;
                }
                if let Some(jet) = self.objects.get_mut(&jet_id) {
                    jet.jet_ai.lockon_drawable_id = Some(id);
                }
                id
            };

            let cpp_pos = gamelogic::common::Coord3D {
                x: pos[0],
                y: pos[1],
                z: pos[2],
            };
            gamelogic::helpers::TheGameClient.set_drawable_position(draw_id, &cpp_pos);
            let dx = owner.x - pos[0];
            let dz = owner.z - pos[2];
            if dx != 0.0 || dz != 0.0 {
                gamelogic::helpers::TheGameClient.set_drawable_orientation(draw_id, dz.atan2(dx));
            }
            gamelogic::helpers::TheGameClient.set_drawable_hidden(draw_id, lockon_hidden);
            gamelogic::helpers::TheGameClient
                .set_drawable_shroud_status_object_id(draw_id, jet_id.0);
        }
    }

    /// C++ `getPerUnitSound("Afterburner")` then `TheAudio->addAudioEvent` /
    /// `removeAudioEvent`. Missing UnitSpecificSounds is NoSound — never the
    /// slot token.
    pub(crate) fn queue_afterburner_per_unit_sound(
        &mut self,
        object_id: ObjectId,
        template_name: &str,
        pos: glam::Vec3,
        on: bool,
    ) {
        if on {
            let Some(event) = crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
                template_name,
                crate::game_logic::object::JET_AFTERBURNER_SOUND,
            ) else {
                return;
            };
            self.queue_audio_event(
                crate::game_logic::AudioEventRequest::new(&event)
                    .with_object(object_id)
                    .with_position(pos)
                    .looping(),
            );
        } else {
            // C++ `TheAudio->removeAudioEvent(m_afterburnerSound)` uses the
            // same per-unit event that start queued — never only the slot token.
            let stop_name = crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
                template_name,
                crate::game_logic::object::JET_AFTERBURNER_SOUND,
            )
            .unwrap_or_else(|| crate::game_logic::object::JET_AFTERBURNER_SOUND_STOP.to_string());
            self.queue_audio_event(
                crate::game_logic::AudioEventRequest::new(&stop_name)
                    .with_object(object_id)
                    .with_position(pos)
                    .stopping(),
            );
        }
    }

    /// C++ `JetTakeoffOrLandingState::update` first-contact MiscAudio
    /// `m_aircraftWheelScreech` (JetAIUpdate.cpp:818-836).
    fn maybe_play_jet_wheel_screech(&mut self, jet_id: ObjectId) {
        self.play_jet_wheel_screech(jet_id, false);
    }

    fn play_jet_wheel_screech(&mut self, jet_id: ObjectId, force_touchdown: bool) {
        use crate::game_logic::object::{
            JET_RTB_PHASE_LANDING, JET_WHEEL_SCREECH_SOUND, JET_WHEEL_SCREECH_Z_SLOP,
        };
        let pos = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return;
            };
            if jet.jet_ai.landing_sound_played {
                return;
            }
            if Self::object_is_produced_at_helipad(jet) {
                return;
            }
            if !force_touchdown && jet.jet_ai.rtb_landing_phase != JET_RTB_PHASE_LANDING {
                return;
            }
            let pos = jet.get_position();
            if !force_touchdown {
                let mut ground = jet.ground_height;
                if let Some(pid) = jet.producer_id {
                    if let Some(af) = self.objects.get(&pid) {
                        if let Some(pp) = af.thing.template.parking_place.as_ref() {
                            ground += pp.landing_deck_height_offset;
                        }
                    }
                }
                // C++ `zPos - zSlop <= groundZ`
                if pos.y - JET_WHEEL_SCREECH_Z_SLOP > ground {
                    return;
                }
            }
            pos
        };
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.jet_ai.landing_sound_played = true;
        }
        let event =
            crate::game_logic::host_economy_log::resolve_misc_audio_event(JET_WHEEL_SCREECH_SOUND);
        self.queue_audio_event(
            crate::game_logic::AudioEventRequest::new(&event)
                .with_object(jet_id)
                .with_position(pos),
        );
    }

    /// C++ PauseBeforeTakeoff after TAXI_TO_TAKEOFF reaches runwayStart.
    fn begin_pause_after_taxi_to_runway(&mut self, jet_id: ObjectId) {
        let producer = self.objects.get(&jet_id).and_then(|j| j.producer_id);
        let approach_y = producer.and_then(|pid| {
            self.calc_airfield_pp_info(pid, jet_id)
                .map(|info| info.runway_approach.y)
        });
        let (waited, end, dist) = {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                return;
            };
            if !jet.jet_ai.taxi_to_takeoff {
                return;
            }
            let waited = jet.jet_ai.takeoff_waited_for_taxi;
            let end = jet
                .jet_ai
                .takeoff_runway_end
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            let dist = jet.jet_ai.takeoff_runway_dist.max(1.0);
            let Some(mut end) = end else {
                jet.jet_ai.taxi_to_takeoff = false;
                return;
            };
            // C++ JetTakeoffOrLandingState::onEnter takeoff:
            // ppinfo.runwayEnd.z = ppinfo.runwayApproach.z (JetAIUpdate.cpp:774).
            if let Some(y) = approach_y {
                end.y = y;
            }
            jet.jet_ai.taxi_to_takeoff = false;
            jet.begin_jet_runway_takeoff(self.frame, end, dist, waited);
            (waited, end, dist)
        };
        let _ = (waited, dist);
        if !self.assign_unit_path(jet_id, end, &[]) {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.movement.path = vec![end];
                jet.movement.current_path_index = 0;
                jet.movement.target_position = Some(end);
                jet.set_ai_state(AIState::Moving);
                jet.set_status_moving(true);
            }
        }
    }

    /// C++ `ParkingPlaceBehavior::transferRunwayReservationToNextInLineForTakeoff`.
    pub(crate) fn transfer_runway_reservation_to_next_in_line(&mut self, jet_id: ObjectId) {
        let airfields: Vec<ObjectId> = self.runway_reservations.keys().copied().collect();
        for af_id in airfields {
            let Some(slots) = self.runway_reservations.get(&af_id) else {
                continue;
            };
            let Some(idx) = slots.iter().position(|s| *s == Some(jet_id)) else {
                continue;
            };
            let next = self
                .airfield_runway_next_in_line
                .get(&af_id)
                .and_then(|next| next.get(idx))
                .copied()
                .flatten();
            let Some(next_id) = next else {
                continue;
            };
            if let Some(slots) = self.runway_reservations.get_mut(&af_id) {
                if idx < slots.len() {
                    slots[idx] = Some(next_id);
                }
            }
            if let Some(queue) = self.airfield_runway_next_in_line.get_mut(&af_id) {
                if idx < queue.len() {
                    queue[idx] = None;
                }
            }
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(&af_id) {
                if idx < was.len() {
                    was[idx] = true;
                }
            }
        }
    }

    /// C++ `JetAIUpdate::getProducerLocation` — snapshot while the airfield lives.
    fn snapshot_jet_producer_location(&mut self, jet_id: ObjectId) {
        let producer_id = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return;
            };
            if jet.jet_producer_location.is_some() {
                return;
            }
            jet.producer_id
        };
        let airfield_pos = producer_id.and_then(|pid| {
            self.objects
                .get(&pid)
                .filter(|airfield| airfield.is_alive())
                .map(|airfield| airfield.get_position())
        });
        if airfield_pos.is_none() && producer_id.is_some() {
            // Producer id still set but the object is gone — do not overwrite
            // with the jet's combat position; wait for a live snapshot or
            // the C++ own-pos fallback after producer is cleared.
            return;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.capture_jet_producer_location(airfield_pos);
        }
    }

    /// C++ RETURN_TO_DEAD_AIRFIELD then CIRCLING_DEAD_AIRFIELD.
    /// Damage only after arriving at the remembered wreck and entering circle.
    fn tick_return_or_circle_dead_airfield(&mut self, jet_id: ObjectId) {
        {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                return;
            };
            if jet.jet_producer_location.is_none() {
                jet.capture_jet_producer_location(None);
            }
        }
        const DEAD_AIRFIELD_ARRIVE: f32 = 80.0;
        let (needs, at_wreck, goal) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return;
            };
            (
                jet.needs_return_to_base_rearm(),
                jet.is_at_jet_producer_location(DEAD_AIRFIELD_ARRIVE),
                jet.jet_producer_location_vec(),
            )
        };
        if !needs {
            return;
        }
        if !at_wreck {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.leave_circling_dead_airfield();
                jet.target = None;
                jet.set_status_attacking(false);
                jet.set_ai_state(AIState::Moving);
            }
            if let Some(goal) = goal {
                let _ = self.assign_unit_path(jet_id, goal, &[]);
            }
            return;
        }
        let entered = self
            .objects
            .get_mut(&jet_id)
            .is_some_and(|jet| jet.enter_circling_dead_airfield(self.frame));
        if entered {
            // C++ JetOrHeliCirclingDeadAirfieldState::onEnter (JetAIUpdate.cpp:338-341):
            // play getPerUnitSound("VoiceLowFuel") at the jet object id. Missing
            // UnitSpecificSounds is silent (NoSound) — never the slot token or
            // an invented `{template}VoiceLowFuel` concat.
            if let Some(jet) = self.objects.get(&jet_id) {
                let pos = jet.get_position();
                let template_name = jet.template_name.clone();
                if let Some(event) = crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
                    &template_name,
                    "VoiceLowFuel",
                ) {
                    self.queue_audio_event(
                        crate::game_logic::AudioEventRequest::new(&event)
                            .with_object(jet_id)
                            .with_position(pos)
                            .with_priority(90),
                    );
                }
            }
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            if jet.jet_circling_dead_airfield {
                let _ = jet.apply_out_of_ammo_damage_frame();
            }
        }
    }

    #[inline]
    fn is_aircraft(object: &Object) -> bool {
        object.is_kind_of(KindOf::Aircraft) || object.object_type == ObjectType::Aircraft
    }

    /// C++ `KINDOF_PRODUCED_AT_HELIPAD`. Host KindOf bank may omit the bit, so
    /// also honor the established helicopter-template detector.
    pub(crate) fn template_is_produced_at_helipad(
        template: &crate::game_logic::ThingTemplate,
    ) -> bool {
        crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
            &template.name,
        )
    }

    pub(crate) fn object_is_produced_at_helipad(object: &Object) -> bool {
        Self::template_is_produced_at_helipad(&object.thing.template)
            || crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
                &object.template_name,
            )
    }

    /// C++ `HeliTakeoffOrLandingState` 3-unit 3D success (JetAIUpdate.cpp:1075-1084).
    const HELI_TAKEOFF_OR_LANDING_THRESH_SQ: f32 = 9.0;

    /// C++ `landingApproach.z += approachHeight + landingDeckHeightOffset`.
    fn helipad_approach_from_parking(
        parking: glam::Vec3,
        approach_height: f32,
        landing_deck_height_offset: f32,
    ) -> glam::Vec3 {
        let mut approach = parking;
        approach.y += approach_height + landing_deck_height_offset;
        approach
    }

    fn begin_heli_takeoff_or_landing(
        &mut self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
        parking: glam::Vec3,
        approach: glam::Vec3,
        landing: bool,
    ) {
        let path = if landing {
            [approach, parking]
        } else {
            [parking, approach]
        };
        self.heli_takeoff_or_landing.insert(
            jet_id,
            HostHeliTakeoffOrLanding {
                path,
                index: 0,
                landing,
                airfield_id,
            },
        );
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.status.airborne_target = true;
            // C++ HeliTakeoffOrLandingState::onEnter (JetAIUpdate.cpp:975-976).
            jet.set_precise_z_and_ultra_accurate(true);
            jet.set_ai_state(AIState::Moving);
            jet.set_status_moving(true);
            jet.movement.path.clear();
            jet.movement.current_path_index = 0;
            jet.movement.target_position = Some(path[0]);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    jet_id,
                    Some([path[0].x, path[0].y, path[0].z]),
                );
                jet.record_host_movement();
            }
        }
    }

    fn tick_heli_takeoff_or_landing(&mut self) {
        let ids: Vec<ObjectId> = self.heli_takeoff_or_landing.keys().copied().collect();
        for id in ids {
            self.step_heli_takeoff_or_landing(id);
        }
    }

    fn step_heli_takeoff_or_landing(&mut self, jet_id: ObjectId) {
        let Some(state) = self.heli_takeoff_or_landing.get(&jet_id).copied() else {
            return;
        };
        let Some((alive, pos, speed)) = self.objects.get(&jet_id).map(|jet| {
            (
                jet.is_alive(),
                jet.get_position(),
                jet.effective_max_speed()
                    .max(jet.movement.max_speed)
                    .max(1.0),
            )
        }) else {
            self.heli_takeoff_or_landing.remove(&jet_id);
            return;
        };
        if !alive {
            self.heli_takeoff_or_landing.remove(&jet_id);
            return;
        }
        let idx = state.index.min(1) as usize;
        let goal = state.path[idx];
        let delta = goal - pos;
        let dist_sq = delta.length_squared();
        if dist_sq <= Self::HELI_TAKEOFF_OR_LANDING_THRESH_SQ {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.set_position(goal);
            }
            let mut next = state;
            next.index = next.index.saturating_add(1);
            if next.index >= 2 {
                self.heli_takeoff_or_landing.remove(&jet_id);
                if next.landing {
                    self.finish_helipad_landing(jet_id, next.airfield_id, next.path[1]);
                } else if let Some(jet) = self.objects.get_mut(&jet_id) {
                    jet.set_ai_state(AIState::Idle);
                    jet.set_precise_z_and_ultra_accurate(false);
                    jet.set_allow_invalid_position(false);
                    jet.status.airborne_target = true;
                    jet.set_status_moving(false);
                    jet.movement.path.clear();
                    jet.movement.current_path_index = 0;
                    jet.movement.target_position = None;
                }
            } else {
                self.heli_takeoff_or_landing.insert(jet_id, next);
                if let Some(jet) = self.objects.get_mut(&jet_id) {
                    jet.movement.target_position = Some(next.path[next.index.min(1) as usize]);
                }
            }
            return;
        }
        let step = speed * LOGIC_FRAME_TIMESTEP;
        let dist = dist_sq.sqrt();
        let t = (step / dist).min(1.0);
        let new_pos = pos + delta * t;
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.set_position(new_pos);
            jet.movement.target_position = Some(goal);
            jet.set_status_moving(true);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    jet_id,
                    Some([new_pos.x, new_pos.y, new_pos.z]),
                );
                jet.record_host_movement();
            }
        }
    }

    fn finish_helipad_landing(&mut self, jet_id: ObjectId, airfield_id: ObjectId, pad: glam::Vec3) {
        self.heli_takeoff_or_landing.remove(&jet_id);
        {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                return;
            };
            jet.set_contained_by(Some(airfield_id));
            jet.set_ai_state(AIState::Docked);
            jet.set_status_moving(false);
            jet.set_precise_z_and_ultra_accurate(false);
            jet.set_allow_invalid_position(false);
            jet.status.airborne_target = false;
            jet.target = None;
            jet.movement.path.clear();
            jet.movement.current_path_index = 0;
            jet.movement.target_position = None;
            jet.set_position(pad);
            jet.return_to_base_requested = false;
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(jet_id, 12);
                crate::game_logic::host_move_log::record(jet_id, Some([pad.x, pad.y, pad.z]));
                jet.record_host_movement();
            }
            jet.begin_parked_airfield_rearm(self.frame);
            let _ = jet.tick_parked_airfield_rearm(self.frame);
        }
        self.release_airfield_runway_for_jet(jet_id);
        self.sync_airfield_hangar_doors(airfield_id);
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.begin_parked_airfield_rearm(self.frame);
            let _ = jet.tick_parked_airfield_rearm(self.frame);
        }
    }

    /// C++ HeliTakeoffOrLandingState landing: approach then pad, success at 3 units.
    fn continue_helipad_landing(
        &mut self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
        metadata: &crate::game_logic::ParkingPlaceMetadata,
    ) -> bool {
        if self
            .objects
            .get(&jet_id)
            .is_some_and(|jet| jet.contained_by == Some(airfield_id))
        {
            return true;
        }
        if let Some(state) = self.heli_takeoff_or_landing.get(&jet_id).copied() {
            if state.landing && state.airfield_id == airfield_id {
                return true;
            }
        }
        let parking = self.heli_park01_pose(airfield_id).unwrap_or_else(|| {
            self.objects
                .get(&airfield_id)
                .map(|airfield| {
                    let mut pad = airfield.get_position();
                    pad.y += metadata.landing_deck_height_offset;
                    pad
                })
                .unwrap_or(glam::Vec3::ZERO)
        });
        let approach = Self::helipad_approach_from_parking(
            parking,
            metadata.approach_height,
            metadata.landing_deck_height_offset,
        );
        let Some(pos) = self.objects.get(&jet_id).map(|jet| jet.get_position()) else {
            return false;
        };
        if (pos - parking).length_squared() <= Self::HELI_TAKEOFF_OR_LANDING_THRESH_SQ {
            self.finish_helipad_landing(jet_id, airfield_id, parking);
            return true;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.target = None;
            jet.set_status_attacking(false);
        }
        self.begin_heli_takeoff_or_landing(jet_id, airfield_id, parking, approach, true);
        true
    }
    /// C++ JetAIUpdate::update parked helipad auto-takeoff when health == max.
    fn tick_helipad_repaired_auto_takeoff(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && Self::object_is_produced_at_helipad(o)
                    && o.contained_by.is_some()
                    && o.target.is_none()
                    && o.health.current + 1e-3 >= o.health.maximum
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if self.heli_takeoff_or_landing.contains_key(&id) {
                continue;
            }
            let ready = self.objects.get(&id).is_some_and(|o| {
                o.contained_by.is_some()
                    && o.target.is_none()
                    && o.health.current + 1e-3 >= o.health.maximum
                    && (o.ai_state == AIState::Docked
                        || o.ai_state == AIState::Idle
                        || crate::gameworld_shadow::gameworld_ai_decision_authority_live())
            });
            if ready {
                let _ = self.try_runway_takeoff_from_airfield(id);
            }
        }
    }

    /// C++ `ParkingPlaceInfo` bone layout residual (host has no W3D logical bones).
    fn airfield_space_row_col(
        index: usize,
        metadata: &crate::game_logic::ParkingPlaceMetadata,
    ) -> Option<(usize, usize)> {
        let cols = usize::try_from(metadata.num_cols).ok().filter(|&c| c > 0)?;
        Some((index / cols, index % cols))
    }

    fn airfield_logical_bone_pose(
        origin: glam::Vec3,
        forward: glam::Vec3,
        right: glam::Vec3,
        col: usize,
        row: usize,
        num_cols: usize,
        num_rows: usize,
        deck: f32,
        along: f32,
        across_scale: f32,
    ) -> (glam::Vec3, f32) {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
        let col_center = col as f32 - (num_cols.saturating_sub(1) as f32 * 0.5);
        let row_center = row as f32 - (num_rows.saturating_sub(1) as f32 * 0.5);
        let pos = origin
            + right * (col_center * PARKING_PLACE_RUNWAY_PREP_SPACING)
            + forward * (along + row_center * PARKING_PLACE_RUNWAY_PREP_SPACING * across_scale);
        (
            glam::Vec3::new(pos.x, origin.y + deck, pos.z),
            forward.x.atan2(forward.z),
        )
    }

    /// C++ `calcPPInfo` from stall/runway bones + `wasInLine` start override.
    fn calc_airfield_pp_info(
        &self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<HostAirfieldPPInfo> {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            PARKING_PLACE_RUNWAY_APPROACH_DIST, PARKING_PLACE_RUNWAY_PREP_SPACING,
        };
        let airfield = self.objects.get(&airfield_id)?;
        let metadata = airfield.thing.template.parking_place.as_ref()?;
        let spaces = self.airfield_parking_spaces.get(&airfield_id)?;
        let index = spaces
            .iter()
            .position(|space| space.object_id == Some(jet_id))?;
        let (row, col) = Self::airfield_space_row_col(index, metadata)?;
        let num_cols = usize::try_from(metadata.num_cols).ok().filter(|&c| c > 0)?;
        let num_rows = usize::try_from(metadata.num_rows).ok().filter(|&r| r > 0)?;
        let origin = airfield.get_position();
        let mut forward = airfield.thing.get_direction_vector();
        forward.y = 0.0;
        if forward.length_squared() < 1.0e-6 {
            forward = glam::Vec3::new(0.0, 0.0, -1.0);
        } else {
            forward = forward.normalize();
        }
        let right = glam::Vec3::new(forward.z, 0.0, -forward.x);
        let deck = metadata.landing_deck_height_offset;
        let (hangar, hangar_orient) = Self::airfield_logical_bone_pose(
            origin,
            forward,
            right,
            col,
            row,
            num_cols,
            num_rows,
            deck,
            -PARKING_PLACE_RUNWAY_PREP_SPACING,
            0.25,
        );
        let (parking, parking_orient) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, row, num_cols, num_rows, deck, 0.0, 0.25,
        );
        let (prep, _) = Self::airfield_logical_bone_pose(
            origin,
            forward,
            right,
            col,
            row,
            num_cols,
            num_rows,
            deck,
            PARKING_PLACE_RUNWAY_PREP_SPACING * 0.5,
            0.0,
        );
        let (runway_start, _) = Self::airfield_logical_bone_pose(
            origin, forward, right, col, 0, num_cols, 1, deck, 0.0, 0.0,
        );
        let (runway_end, _) = Self::airfield_logical_bone_pose(
            origin,
            forward,
            right,
            col,
            0,
            num_cols,
            1,
            deck,
            PARKING_PLACE_RUNWAY_PREP_SPACING * 2.0,
            0.0,
        );
        let park_in_hangars = metadata.park_in_hangars;
        let mut info = HostAirfieldPPInfo {
            parking_space: if park_in_hangars { hangar } else { parking },
            parking_orientation: if park_in_hangars {
                hangar_orient
            } else {
                parking_orient
            },
            runway_prep: prep,
            runway_start,
            runway_end,
            runway_approach: runway_end
                + (runway_end - runway_start) * PARKING_PLACE_RUNWAY_APPROACH_DIST,
            hangar_internal: hangar,
            hangar_internal_orient: hangar_orient,
            runway_takeoff_dist: runway_start.distance(runway_end),
        };
        info.runway_approach.y =
            runway_end.y + metadata.approach_height + metadata.landing_deck_height_offset;
        if self
            .airfield_runway_was_in_line
            .get(&airfield_id)
            .and_then(|flags| flags.get(col))
            .copied()
            .unwrap_or(false)
            && self
                .runway_reservations
                .get(&airfield_id)
                .and_then(|slots| slots.get(col))
                .copied()
                .flatten()
                == Some(jet_id)
        {
            info.runway_start = info.runway_prep;
        }
        Some(info)
    }

    fn heli_park01_pose(&self, airfield_id: ObjectId) -> Option<glam::Vec3> {
        let airfield = self.objects.get(&airfield_id)?;
        let deck = airfield
            .thing
            .template
            .parking_place
            .as_ref()
            .map(|metadata| metadata.landing_deck_height_offset)
            .unwrap_or(0.0);
        let mut pos = airfield.get_position();
        pos.y += deck;
        Some(pos)
    }

    fn ensure_airfield_runway_queues(&mut self, airfield_id: ObjectId) -> Option<usize> {
        let runway_count = self
            .objects
            .get(&airfield_id)
            .filter(|object| Self::has_usable_airfield_parking_behavior(object))
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| metadata.runway_count())?;
        if runway_count == 0 {
            return None;
        }
        {
            let slots = self
                .runway_reservations
                .entry(airfield_id)
                .or_insert_with(|| vec![None; runway_count]);
            if slots.len() != runway_count {
                slots.resize(runway_count, None);
            }
        }
        {
            let next = self
                .airfield_runway_next_in_line
                .entry(airfield_id)
                .or_insert_with(|| vec![None; runway_count]);
            if next.len() != runway_count {
                next.resize(runway_count, None);
            }
        }
        {
            let was = self
                .airfield_runway_was_in_line
                .entry(airfield_id)
                .or_insert_with(|| vec![false; runway_count]);
            if was.len() != runway_count {
                was.resize(runway_count, false);
            }
        }
        Some(runway_count)
    }

    fn airfield_parking_runway_column(
        &self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        let spaces = self.airfield_parking_spaces.get(&airfield_id)?;
        let index = spaces
            .iter()
            .position(|space| space.object_id == Some(jet_id))?;
        let cols = self
            .objects
            .get(&airfield_id)
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| usize::try_from(metadata.num_cols).ok())
            .filter(|&cols| cols > 0)?;
        Some(index % cols)
    }

    fn jet_holds_airfield_runway(&self, airfield_id: ObjectId, jet_id: ObjectId) -> bool {
        self.runway_reservations
            .get(&airfield_id)
            .is_some_and(|slots| slots.iter().any(|slot| *slot == Some(jet_id)))
    }

    fn jet_is_above_terrain(jet: &Object) -> bool {
        jet.status.airborne_target || jet.get_position().y > 5.0
    }

    fn sync_airfield_hangar_doors(&mut self, airfield_id: ObjectId) {
        // C++ reserveSpace/releaseSpace/purgeDead: setHoldDoorOpen(ppi->m_door)
        // for that stall only. Stall index == ExitDoorType (DOOR_1..4).
        let holds: [bool; 4] = {
            let mut holds = [false; 4];
            if let Some(spaces) = self.airfield_parking_spaces.get(&airfield_id) {
                for (i, space) in spaces.iter().enumerate().take(4) {
                    holds[i] = space.object_id.is_some() || space.reserved_for_exit;
                }
            }
            holds
        };
        let frame = self.frame;
        if let Some(airfield) = self.objects.get_mut(&airfield_id) {
            let count = airfield.production_door_count();
            for i in 0..count {
                if airfield.production_door_is_held(i) != holds[i] {
                    airfield.set_production_door_hold_open_at(i, holds[i], frame);
                }
            }
        }
    }

    fn apply_pending_helipad_exits(&mut self) {
        let pending: Vec<(ObjectId, ObjectId)> =
            self.airfield_pending_helipad_exits.drain().collect();
        for (jet_id, airfield_id) in pending {
            let Some(heli) = self.heli_park01_pose(airfield_id) else {
                continue;
            };
            let rally = self
                .objects
                .get(&airfield_id)
                .and_then(|airfield| airfield.building_data.as_ref())
                .and_then(|building| building.rally_point);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if !jet.is_alive() || !Self::object_is_produced_at_helipad(jet) {
                    continue;
                }
                jet.set_position(heli);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(
                        jet_id,
                        Some([heli.x, heli.y, heli.z]),
                    );
                    jet.record_host_movement();
                }
            }
            let dest = rally.unwrap_or(heli);
            let _ = self.assign_unit_path(jet_id, dest, &[]);
        }
    }

    /// C++ `ParkingPlaceBehavior::killAllParkedUnits`.
    fn kill_all_parked_units(&mut self, airfield_id: ObjectId) {
        let Some(spaces) = self.airfield_parking_spaces.get(&airfield_id).cloned() else {
            return;
        };
        let mut kill = Vec::new();
        for space in &spaces {
            let Some(jet_id) = space.object_id else {
                continue;
            };
            let Some(jet) = self.objects.get(&jet_id) else {
                continue;
            };
            if !jet.is_alive() {
                continue;
            }
            let takeoff_or_landing = self.jet_holds_airfield_runway(airfield_id, jet_id);
            if Self::jet_is_above_terrain(jet) && !takeoff_or_landing {
                continue;
            }
            kill.push(jet_id);
        }
        for jet_id in kill {
            self.destroy_object(jet_id);
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            self.release_airfield_runway_for_jet(jet_id);
        }
    }

    /// C++ `ParkingPlaceBehavior::defectAllParkedUnits`.
    pub(in super::super) fn defect_all_parked_units(&mut self, airfield_id: ObjectId) {
        use crate::game_logic::host_defection_helper::DEFAULT_DEFECTION_PROTECTION_FRAMES;
        let Some(spaces) = self.airfield_parking_spaces.get(&airfield_id).cloned() else {
            return;
        };
        let Some((new_team, new_owner)) = self
            .objects
            .get(&airfield_id)
            .map(|airfield| (airfield.team, airfield.owner_player_id))
        else {
            return;
        };
        let now = self.frame;
        let mut release = Vec::new();
        for space in &spaces {
            let Some(jet_id) = space.object_id else {
                continue;
            };
            let Some(jet) = self.objects.get(&jet_id) else {
                continue;
            };
            if !jet.is_alive() {
                continue;
            }
            let takeoff_or_landing = self.jet_holds_airfield_runway(airfield_id, jet_id);
            if Self::jet_is_above_terrain(jet) && !takeoff_or_landing {
                let owner_differs = jet.team != new_team || jet.owner_player_id != new_owner;
                if owner_differs {
                    release.push(jet_id);
                }
                continue;
            }
            if jet.team == new_team && jet.owner_player_id == new_owner {
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.defect(new_team, now, DEFAULT_DEFECTION_PROTECTION_FRAMES);
                jet.owner_player_id = new_owner;
            }
        }
        for jet_id in release {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if jet.producer_id == Some(airfield_id) {
                    jet.producer_id = None;
                }
            }
        }
    }

    fn tick_airfield_parking_lifecycle(&mut self) {
        self.apply_pending_helipad_exits();
        let mut airfields: Vec<ObjectId> = self.airfield_parking_spaces.keys().copied().collect();
        for (&id, object) in self.objects.iter() {
            if object.thing.template.parking_place.is_some() {
                airfields.push(id);
            }
        }
        airfields.sort_by_key(|id| id.0);
        airfields.dedup();
        for airfield_id in airfields {
            let dead_or_sold = self
                .objects
                .get(&airfield_id)
                .map(|object| {
                    !object.is_alive()
                        || object.status.destroyed
                        || object.status.sold
                        || object.status.effectively_dead
                })
                .unwrap_or(true);
            if dead_or_sold {
                self.kill_all_parked_units(airfield_id);
            }
            // C++ `ParkingPlaceBehavior::defectAllParkedUnits` runs only from
            // `Object::defect` (airfield capture), never each tick on team mismatch.
        }
    }

    /// Exact C++ `getPP` eligibility for the compact host path.  `FSAirfield`
    /// by itself is deliberately insufficient: C++ finds a real
    /// `ParkingPlaceBehaviorInterface`, and the host must have its authored
    /// module data before it can reserve a space.
    #[inline]
    fn has_usable_airfield_parking_behavior(object: &Object) -> bool {
        object.is_alive()
            && !object.status.destroyed
            && !object.status.under_construction
            && !object.status.sold
            && object.is_kind_of(KindOf::FSAirfield)
            && object
                .thing
                .template
                .parking_place
                .as_ref()
                .and_then(|metadata| metadata.capacity())
                .is_some_and(|capacity| capacity > 0)
    }

    /// C++ `ActionManager::canEnterObject` uses a controller-exact check for
    /// a jet entering its own airfield.  JetAI's later `findSuitableAirfield`
    /// search may use an explicitly allied airfield, but never a merely
    /// same-faction or unproven-owner object.
    fn airfield_has_exact_controller_for_jet(
        &self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
    ) -> bool {
        let (Some(jet), Some(airfield)) =
            (self.objects.get(&jet_id), self.objects.get(&airfield_id))
        else {
            return false;
        };
        match (jet.owner_player_id, airfield.owner_player_id) {
            (Some(jet_owner), Some(airfield_owner)) => {
                jet_owner == airfield_owner
                    && self.player_owner_for_host_object(jet) == Some(jet_owner)
                    && self.player_owner_for_host_object(airfield) == Some(airfield_owner)
            }
            // Producer preference deliberately does not infer an exact owner
            // from a faction.  A pre-owner-save aircraft can use the explicit
            // relationship fallback below, never an arbitrary producer id.
            _ => false,
        }
    }

    /// JetAI's fallback `findSuitableAirfield` predicate.  This is separate
    /// from the producer-first controller check above: `ALLOW_ALLIES` is an
    /// explicit player relationship, not a matching USA/China/GLA faction.
    pub(crate) fn is_friendly_airfield(&self, jet_id: ObjectId, airfield_id: ObjectId) -> bool {
        let (Some(jet), Some(airfield)) =
            (self.objects.get(&jet_id), self.objects.get(&airfield_id))
        else {
            return false;
        };
        Self::has_usable_airfield_parking_behavior(airfield)
            && self.normal_enter_relationship(jet, airfield)
                == gamelogic::common::Relationship::Allies
    }

    /// Authored C++ `ParkingPlaceBehavior` capacity for exactly one object.
    /// No template-name or retail `2 × 2` fallback is permitted here.
    pub(crate) fn airfield_parking_capacity(&self, airfield_id: ObjectId) -> Option<usize> {
        self.objects
            .get(&airfield_id)
            .filter(|object| Self::has_usable_airfield_parking_behavior(object))
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| metadata.capacity())
    }

    /// Rebuild/clean the host's exact parking reservation table from its
    /// persistent per-jet slot index.  Old snapshots that predate that field
    /// can only recover a reservation when the jet is physically marked as
    /// contained by this exact airfield; arbitrary garrison rosters are never
    /// treated as ParkingPlace state.
    fn normalize_airfield_parking_spaces(&mut self, airfield_id: ObjectId) -> bool {
        let Some(capacity) = self.airfield_parking_capacity(airfield_id) else {
            self.airfield_parking_spaces.remove(&airfield_id);
            self.sync_airfield_hangar_doors(airfield_id);
            return false;
        };

        let mut spaces = self
            .airfield_parking_spaces
            .remove(&airfield_id)
            .filter(|spaces| spaces.len() == capacity)
            .unwrap_or_else(|| vec![AirfieldParkingSpace::default(); capacity]);

        // Clear an entry once its owner no longer records this exact parking
        // slot.  C++ `releaseSpace` is explicit; producer identity alone is
        // not enough because a jet keeps its producer after takeoff.
        for (index, space) in spaces.iter_mut().enumerate() {
            let keep = space.object_id.is_some_and(|jet_id| {
                self.objects.get(&jet_id).is_some_and(|jet| {
                    jet.is_alive()
                        && Self::is_aircraft(jet)
                        && jet.producer_id == Some(airfield_id)
                        && jet.airfield_parking_space_index == u32::try_from(index).ok()
                })
            });
            if !keep {
                let reserved_for_exit = space.reserved_for_exit && space.object_id.is_none();
                *space = AirfieldParkingSpace::default();
                space.reserved_for_exit = reserved_for_exit;
            }
        }

        // Restore live reservations from the persistent pair written by
        // `reserveSpace`: this covers both parked jets and en-route jets
        // after a save/load, where the in-memory table itself is rebuilt.
        // A malformed duplicate index is resolved deterministically and the
        // later claimant is left unreserved rather than silently sharing a
        // C++ ParkingPlace slot.
        let mut persisted_reservations: Vec<(ObjectId, usize)> = self
            .objects
            .iter()
            .filter_map(|(&jet_id, jet)| {
                (jet.is_alive() && Self::is_aircraft(jet) && jet.producer_id == Some(airfield_id))
                    .then(|| {
                        jet.airfield_parking_space_index
                            .and_then(|index| usize::try_from(index).ok())
                            .filter(|&index| index < capacity)
                            .map(|index| (jet_id, index))
                    })
                    .flatten()
            })
            .collect();
        persisted_reservations.sort_by_key(|(jet_id, _)| jet_id.0);
        let mut conflicting_claimants = Vec::new();
        for (jet_id, index) in persisted_reservations {
            if spaces[index].reserved_for_exit && spaces[index].object_id.is_none() {
                conflicting_claimants.push(jet_id);
            } else if spaces[index].object_id.is_none() || spaces[index].object_id == Some(jet_id) {
                spaces[index].object_id = Some(jet_id);
            } else {
                conflicting_claimants.push(jet_id);
            }
        }

        for jet_id in conflicting_claimants {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.airfield_parking_space_index = None;
            }
        }

        // Older save/runtime records did not retain the slot index.  Their
        // exact `contained_by` link is an active parked state, so recover the
        // first free source-authored space deterministically by ObjectId.
        let mut legacy_parked: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&jet_id, jet)| {
                (jet.is_alive()
                    && Self::is_aircraft(jet)
                    && jet.contained_by == Some(airfield_id)
                    && jet.airfield_parking_space_index.is_none())
                .then_some(jet_id)
            })
            .collect();
        legacy_parked.sort_by_key(|id| id.0);
        let mut recovered = Vec::new();
        for jet_id in legacy_parked {
            let Some(index) = spaces
                .iter()
                .position(|space| space.object_id.is_none() && !space.reserved_for_exit)
            else {
                break;
            };
            spaces[index].object_id = Some(jet_id);
            recovered.push((jet_id, index));
        }
        for (jet_id, index) in recovered {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.producer_id = Some(airfield_id);
                jet.airfield_parking_space_index = u32::try_from(index).ok();
            }
        }

        self.airfield_parking_spaces.insert(airfield_id, spaces);
        self.sync_airfield_hangar_doors(airfield_id);
        true
    }

    /// C++ `ParkingPlaceBehavior::hasReservedSpace` mirrored against the
    /// actual authored-space reservation, never a building garrison list.
    fn airfield_has_reserved_space(&self, airfield_id: ObjectId, jet_id: ObjectId) -> bool {
        self.airfield_parking_spaces
            .get(&airfield_id)
            .is_some_and(|spaces| spaces.iter().any(|space| space.object_id == Some(jet_id)))
    }

    /// C++ `ParkingPlaceBehavior::reserveSpace`.  The selected slot is stored
    /// on both sides of the relation: the airfield table and the aircraft's
    /// persistent `producer_id` + parking-space index.
    fn reserve_airfield_parking_space(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        if !self.normalize_airfield_parking_spaces(airfield_id) {
            return None;
        }
        let existing = self
            .airfield_parking_spaces
            .get(&airfield_id)
            .and_then(|spaces| {
                spaces
                    .iter()
                    .position(|space| space.object_id == Some(jet_id))
            });
        let index = match existing {
            Some(index) => index,
            None => {
                let spaces = self.airfield_parking_spaces.get_mut(&airfield_id)?;
                let index = spaces
                    .iter()
                    .position(|space| space.object_id.is_none() && !space.reserved_for_exit)?;
                spaces[index].object_id = Some(jet_id);
                index
            }
        };
        let slot_index = u32::try_from(index).ok();
        let airfield_pos = self.objects.get(&airfield_id).map(|a| a.get_position());
        let jet_valid = self
            .objects
            .get(&jet_id)
            .is_some_and(|jet| Self::is_aircraft(jet) && jet.is_alive());
        let (Some(slot_index), true) = (slot_index, jet_valid) else {
            // C++ reserveSpace never leaves the stall table mutated when the
            // reserving aircraft cannot take the reservation: undo the claim
            // before any early return so the slot stays available.
            if existing.is_none() {
                if let Some(spaces) = self.airfield_parking_spaces.get_mut(&airfield_id) {
                    if let Some(space) = spaces.get_mut(index) {
                        space.object_id = None;
                    }
                }
                self.sync_airfield_hangar_doors(airfield_id);
            }
            return None;
        };
        let Some(jet) = self.objects.get_mut(&jet_id) else {
            return None;
        };
        jet.producer_id = Some(airfield_id);
        jet.airfield_parking_space_index = Some(slot_index);
        jet.capture_jet_producer_location(airfield_pos);
        self.sync_airfield_hangar_doors(airfield_id);
        Some(index)
    }

    /// Authored `NumRows × NumCols` stall count (C++ ParkingPlaceInfo length).
    fn authored_parking_capacity(metadata: &crate::game_logic::ParkingPlaceMetadata) -> usize {
        let rows = usize::try_from(metadata.num_rows).unwrap_or(0);
        let cols = usize::try_from(metadata.num_cols).unwrap_or(0);
        rows.saturating_mul(cols)
    }

    /// Free stalls that C++ `reserveSpace` / `DOOR_NONE_AVAILABLE` would accept.
    pub(in super::super) fn count_free_airfield_parking_slots(
        spaces: Option<&[AirfieldParkingSpace]>,
        capacity: usize,
    ) -> usize {
        match spaces {
            Some(spaces) if !spaces.is_empty() => spaces
                .iter()
                .filter(|space| space.object_id.is_none() && !space.reserved_for_exit)
                .count(),
            _ => capacity,
        }
    }

    /// C++ `ParkingPlaceBehavior::shouldReserveDoorWhenQueued`.
    pub(crate) fn should_reserve_airfield_door_when_queued(
        &self,
        airfield_id: ObjectId,
        template: &crate::game_logic::ThingTemplate,
    ) -> bool {
        self.objects
            .get(&airfield_id)
            .is_some_and(Self::has_usable_airfield_parking_behavior)
            && template.is_kind_of(KindOf::Aircraft)
            && !Self::template_is_produced_at_helipad(template)
    }

    /// C++ `ParkingPlaceBehavior::reserveDoorForExit` — stall reserved, no occupant.
    pub(crate) fn reserve_airfield_door_for_exit(
        &mut self,
        airfield_id: ObjectId,
    ) -> Option<usize> {
        if !self.normalize_airfield_parking_spaces(airfield_id) {
            return None;
        }
        let spaces = self.airfield_parking_spaces.get_mut(&airfield_id)?;
        let index = spaces
            .iter()
            .position(|space| space.object_id.is_none() && !space.reserved_for_exit)?;
        spaces[index].reserved_for_exit = true;
        self.sync_airfield_hangar_doors(airfield_id);
        Some(index)
    }

    /// C++ `ParkingPlaceBehavior::unreserveDoorForExit`.
    pub(crate) fn unreserve_airfield_door_for_exit(&mut self, airfield_id: ObjectId, door: usize) {
        if let Some(spaces) = self.airfield_parking_spaces.get_mut(&airfield_id) {
            if let Some(space) = spaces.get_mut(door) {
                if space.object_id.is_none() {
                    space.reserved_for_exit = false;
                }
            }
        }
        self.sync_airfield_hangar_doors(airfield_id);
    }

    fn unreserve_nth_airfield_exit_door(&mut self, airfield_id: ObjectId, n: usize) {
        let door = self
            .airfield_parking_spaces
            .get(&airfield_id)
            .and_then(|spaces| {
                spaces
                    .iter()
                    .enumerate()
                    .filter(|(_, space)| space.reserved_for_exit && space.object_id.is_none())
                    .nth(n)
                    .map(|(index, _)| index)
            });
        if let Some(door) = door {
            self.unreserve_airfield_door_for_exit(airfield_id, door);
        }
    }

    pub(crate) fn unreserve_all_airfield_exit_doors(&mut self, airfield_id: ObjectId) {
        if let Some(spaces) = self.airfield_parking_spaces.get_mut(&airfield_id) {
            for space in spaces.iter_mut() {
                if space.object_id.is_none() {
                    space.reserved_for_exit = false;
                }
            }
        }
        self.sync_airfield_hangar_doors(airfield_id);
    }

    /// Queue-order index of a reserving production item (non-helipad aircraft).
    pub(crate) fn airfield_reserving_queue_index(
        &self,
        airfield_id: ObjectId,
        queue_pos: usize,
    ) -> Option<usize> {
        let building = self.objects.get(&airfield_id)?.building_data.as_ref()?;
        let mut n = 0usize;
        for (i, item) in building.production_queue.iter().enumerate() {
            if item.is_upgrade() {
                continue;
            }
            let Some(template) = self.templates.get(&item.template_name) else {
                continue;
            };
            if !self.should_reserve_airfield_door_when_queued(airfield_id, template) {
                continue;
            }
            if i == queue_pos {
                return Some(n);
            }
            n = n.saturating_add(1);
        }
        None
    }

    pub(crate) fn unreserve_airfield_door_for_cancelled_queue_item(
        &mut self,
        airfield_id: ObjectId,
        queue_pos: usize,
        template_name: &str,
    ) {
        let Some(template) = self.templates.get(template_name).cloned() else {
            return;
        };
        if !self.should_reserve_airfield_door_when_queued(airfield_id, &template) {
            return;
        }
        let Some(n) = self.airfield_reserving_queue_index(airfield_id, queue_pos) else {
            return;
        };
        self.unreserve_nth_airfield_exit_door(airfield_id, n);
    }

    /// C++ `exitObjectViaDoor` — claim a reserved-for-exit stall.
    fn claim_reserved_exit_parking_space(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        if !self.normalize_airfield_parking_spaces(airfield_id) {
            return None;
        }
        let existing = self
            .airfield_parking_spaces
            .get(&airfield_id)
            .and_then(|spaces| {
                spaces
                    .iter()
                    .position(|space| space.object_id == Some(jet_id))
            });
        let index = match existing {
            Some(index) => index,
            None => {
                let spaces = self.airfield_parking_spaces.get_mut(&airfield_id)?;
                let index = spaces
                    .iter()
                    .position(|space| space.object_id.is_none() && space.reserved_for_exit)?;
                spaces[index].object_id = Some(jet_id);
                spaces[index].reserved_for_exit = false;
                index
            }
        };
        let slot_index = u32::try_from(index).ok()?;
        let airfield_pos = self.objects.get(&airfield_id).map(|a| a.get_position());
        let jet = self.objects.get_mut(&jet_id)?;
        if !Self::is_aircraft(jet) || !jet.is_alive() {
            return None;
        }
        jet.producer_id = Some(airfield_id);
        jet.airfield_parking_space_index = Some(slot_index);
        jet.capture_jet_producer_location(airfield_pos);
        self.sync_airfield_hangar_doors(airfield_id);
        Some(index)
    }

    /// C++ `JetAIUpdateModuleData::m_keepsParkingSpaceWhenAirborne` default true.
    fn object_keeps_parking_space_when_airborne(_object: &Object) -> bool {
        true
    }

    /// C++ `ParkingPlaceBehavior::setHealee`.
    fn set_airfield_healee(&mut self, airfield_id: ObjectId, jet_id: ObjectId, add: bool) {
        if add {
            let list = self.airfield_healing.entry(airfield_id).or_default();
            if list.iter().any(|info| info.getting_healed_id == jet_id) {
                return;
            }
            list.push(AirfieldHealingInfo {
                getting_healed_id: jet_id,
                heal_start_frame: self.frame,
            });
            self.reset_airfield_heal_wake(airfield_id);
        } else {
            let Some(list) = self.airfield_healing.get_mut(&airfield_id) else {
                return;
            };
            let before = list.len();
            list.retain(|info| info.getting_healed_id != jet_id);
            if list.len() != before {
                if list.is_empty() {
                    self.airfield_healing.remove(&airfield_id);
                }
                self.reset_airfield_heal_wake(airfield_id);
            }
        }
    }

    fn reset_airfield_heal_wake(&mut self, airfield_id: ObjectId) {
        if self
            .airfield_healing
            .get(&airfield_id)
            .is_none_or(|list| list.is_empty())
        {
            self.airfield_next_heal_frame
                .insert(airfield_id, AIRFIELD_HEAL_FOREVER);
        } else {
            self.airfield_next_heal_frame.insert(
                airfield_id,
                self.frame.saturating_add(AIRFIELD_HEAL_RATE_FRAMES),
            );
        }
    }

    fn set_airfield_healee_for_jet(&mut self, jet_id: ObjectId, add: bool) {
        let airfields: Vec<ObjectId> = self
            .objects
            .get(&jet_id)
            .and_then(|jet| jet.contained_by.or(jet.producer_id))
            .into_iter()
            .chain(
                self.airfield_parking_spaces
                    .iter()
                    .filter_map(|(&airfield_id, spaces)| {
                        spaces
                            .iter()
                            .any(|space| space.object_id == Some(jet_id))
                            .then_some(airfield_id)
                    }),
            )
            .collect();
        for airfield_id in airfields {
            self.set_airfield_healee(airfield_id, jet_id, add);
        }
    }

    /// C++ `ChinookAIUpdate.cpp:1055` `setHealee(obj, flightStatus == CHINOOK_LANDED)`.
    fn chinook_is_landed_at_airfield(jet: &Object, airfield_id: ObjectId) -> bool {
        jet.chinook_ai.as_ref().is_some_and(|ai| {
            ai.flight_status
                == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
                && ai.airfield_id == Some(airfield_id.0)
        })
    }

    /// C++ `setHealee(jet, !ALLOW_AIR_LOCO)` — parked, taxiing, or helipad-landed.
    fn jet_should_be_airfield_healee(&self, jet: &Object, airfield_id: ObjectId) -> bool {
        if !jet.is_alive() || !Self::is_aircraft(jet) {
            return false;
        }
        if Self::chinook_is_landed_at_airfield(jet, airfield_id) {
            return true;
        }
        if Self::object_is_produced_at_helipad(jet) {
            return jet.contained_by == Some(airfield_id);
        }
        if !self.airfield_has_reserved_space(airfield_id, jet.id) {
            return false;
        }
        jet.contained_by == Some(airfield_id)
            || jet.jet_ai.takeoff_in_progress
            || jet.jet_ai.landing_in_progress
            || !jet.status.airborne_target
    }

    fn sync_airfield_healees(&mut self) {
        let mut desired: Vec<(ObjectId, ObjectId)> = Vec::new();
        let airfields: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, object)| Self::has_usable_airfield_parking_behavior(object))
            .map(|(&id, _)| id)
            .collect();
        for airfield_id in airfields {
            let jet_ids: Vec<ObjectId> = self
                .objects
                .iter()
                .filter(|(_, jet)| Self::is_aircraft(jet) && jet.is_alive())
                .map(|(&id, _)| id)
                .collect();
            for jet_id in jet_ids {
                let should = self
                    .objects
                    .get(&jet_id)
                    .is_some_and(|jet| self.jet_should_be_airfield_healee(jet, airfield_id));
                if should {
                    desired.push((airfield_id, jet_id));
                }
            }
        }
        let current: Vec<(ObjectId, ObjectId)> = self
            .airfield_healing
            .iter()
            .flat_map(|(&airfield_id, list)| {
                list.iter()
                    .map(move |info| (airfield_id, info.getting_healed_id))
            })
            .collect();
        for (airfield_id, jet_id) in &current {
            if !desired.contains(&(*airfield_id, *jet_id)) {
                self.set_airfield_healee(*airfield_id, *jet_id, false);
            }
        }
        for (airfield_id, jet_id) in desired {
            self.set_airfield_healee(airfield_id, jet_id, true);
        }
    }

    /// C++ `ParkingPlaceBehavior::exitObjectViaDoor` hangar/parking bone pose.
    pub(in super::super) fn place_produced_jet_at_parking_pose(
        &mut self,
        producer_id: ObjectId,
        jet_id: ObjectId,
    ) -> bool {
        let Some(info) = self.calc_airfield_pp_info(producer_id, jet_id) else {
            return false;
        };
        let Some(jet) = self.objects.get_mut(&jet_id) else {
            return false;
        };
        // C++ exitObjectViaDoor creation/hangar bone; FROM_HANGAR taxis to parking.
        jet.set_position(info.hangar_internal);
        jet.set_orientation(info.hangar_internal_orient);
        jet.set_contained_by(Some(producer_id));
        jet.set_ai_state(AIState::Docked);
        jet.set_status_moving(false);
        jet.status.airborne_target = false;
        jet.apply_taxiing_locomotor_set();
        jet.movement.path.clear();
        if crate::gameworld_shadow::gameworld_movement_authority_live() {
            crate::game_logic::host_move_log::record(
                jet_id,
                Some([
                    info.hangar_internal.x,
                    info.hangar_internal.y,
                    info.hangar_internal.z,
                ]),
            );
            jet.record_host_movement();
        }
        self.sync_airfield_hangar_doors(producer_id);
        true
    }

    /// C++ `ParkingPlaceBehavior::exitObjectViaDoor` for a completed factory
    /// aircraft: its producer link is only a landing authority when the
    /// output was assigned one real authored parking space under the exact
    /// same controller.  The new jet remains outside generic containment and
    /// follows the ordinary production exit path while retaining the slot.
    pub(in super::super) fn reserve_produced_aircraft_parking_space(
        &mut self,
        producer_id: ObjectId,
        jet_id: ObjectId,
    ) -> bool {
        if !self
            .objects
            .get(&producer_id)
            .is_some_and(Self::has_usable_airfield_parking_behavior)
        {
            return false;
        }
        let produced_at_helipad = self
            .objects
            .get(&jet_id)
            .is_some_and(Self::object_is_produced_at_helipad);
        if produced_at_helipad {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.producer_id = Some(producer_id);
                jet.airfield_parking_space_index = None;
            }
            self.airfield_pending_helipad_exits
                .insert(jet_id, producer_id);
            return true;
        }
        self.airfield_has_exact_controller_for_jet(jet_id, producer_id)
            && self
                .claim_reserved_exit_parking_space(producer_id, jet_id)
                .or_else(|| self.reserve_airfield_parking_space(producer_id, jet_id))
                .is_some()
    }

    /// C++ `ParkingPlaceBehavior::releaseSpace` for every reservation held by
    /// this aircraft.  Producer identity intentionally survives the release,
    /// matching JetAI's producer-first future landing preference.
    fn release_airfield_parking_space_for_jet(&mut self, jet_id: ObjectId) -> Option<ObjectId> {
        let airfields: Vec<ObjectId> = self.airfield_parking_spaces.keys().copied().collect();
        let mut released_from = None;
        for airfield_id in airfields {
            let Some(spaces) = self.airfield_parking_spaces.get_mut(&airfield_id) else {
                continue;
            };
            let mut released_here = false;
            for space in spaces.iter_mut() {
                if space.object_id == Some(jet_id) {
                    *space = AirfieldParkingSpace::default();
                    released_here = true;
                }
            }
            if released_here {
                released_from = Some(airfield_id);
            }
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.airfield_parking_space_index = None;
        }
        if let Some(airfield_id) = released_from {
            self.sync_airfield_hangar_doors(airfield_id);
        }
        released_from
    }

    /// Count currently parked jets from the actual ParkingPlace reservation
    /// records.  En-route reservations remain occupied but are intentionally
    /// excluded from this parked-only view.
    pub(crate) fn airfield_parked_count(&self, airfield_id: ObjectId) -> usize {
        self.airfield_parking_spaces
            .get(&airfield_id)
            .map(|spaces| {
                spaces
                    .iter()
                    .filter_map(|space| space.object_id)
                    .filter(|jet_id| {
                        self.objects.get(jet_id).is_some_and(|jet| {
                            jet.is_alive()
                                && Self::is_aircraft(jet)
                                && jet.contained_by == Some(airfield_id)
                        })
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Ensure runway slots are sized from the same authored ParkingPlace
    /// metadata.  `HasRunways = No` yields no synthetic runway.
    pub(crate) fn airfield_runway_slots_mut(
        &mut self,
        airfield_id: ObjectId,
    ) -> Option<&mut Vec<Option<ObjectId>>> {
        let runway_count = self
            .objects
            .get(&airfield_id)
            .filter(|object| Self::has_usable_airfield_parking_behavior(object))
            .and_then(|object| object.thing.template.parking_place.as_ref())
            .and_then(|metadata| metadata.runway_count())?;
        if runway_count == 0 {
            return None;
        }
        let slots = self
            .runway_reservations
            .entry(airfield_id)
            .or_insert_with(|| vec![None; runway_count]);
        if slots.len() != runway_count {
            slots.resize(runway_count, None);
            slots.truncate(runway_count);
        }
        Some(slots)
    }

    /// C++ ParkingPlaceBehavior::transferRunwayReservationToNext / reserve residual.
    ///
    /// Returns runway index when a free runway is reserved for `jet_id`.
    pub(crate) fn reserve_airfield_runway(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        self.reserve_airfield_runway_ex(airfield_id, jet_id, false)
    }

    /// C++ `ParkingPlaceBehavior::reserveRunway(id, forLanding)`.
    fn reserve_airfield_runway_ex(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
        for_landing: bool,
    ) -> Option<usize> {
        let _ = self.ensure_airfield_runway_queues(airfield_id)?;
        // C++ ParkingPlaceBehavior keeps m_spaces live for every docked
        // aircraft (buildInfo/purgeDead run before reserveRunway), so a
        // docked jet always has a stall→runway mapping. Rebuild the host
        // table the same way before the column lookup.
        self.normalize_airfield_parking_spaces(airfield_id);
        if let Some(slots) = self.runway_reservations.get(&airfield_id) {
            if let Some(idx) = slots.iter().position(|s| *s == Some(jet_id)) {
                return Some(idx);
            }
        }
        // C++: no stall → DEBUG_CRASH and false. Never first-free across columns.
        let column = self.airfield_parking_runway_column(airfield_id, jet_id)?;
        let in_use = self
            .runway_reservations
            .get(&airfield_id)
            .and_then(|slots| slots.get(column))
            .copied()
            .flatten();
        if in_use == Some(jet_id) {
            return Some(column);
        }
        if in_use.is_none() {
            if let Some(slots) = self.runway_reservations.get_mut(&airfield_id) {
                if let Some(slot) = slots.get_mut(column) {
                    *slot = Some(jet_id);
                }
            }
            let was_in_line = self
                .airfield_runway_next_in_line
                .get(&airfield_id)
                .and_then(|next| next.get(column))
                .copied()
                .flatten()
                == Some(jet_id);
            if let Some(next) = self.airfield_runway_next_in_line.get_mut(&airfield_id) {
                if let Some(slot) = next.get_mut(column) {
                    if *slot == Some(jet_id) {
                        *slot = None;
                    }
                }
            }
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(&airfield_id) {
                if let Some(flag) = was.get_mut(column) {
                    *flag = was_in_line;
                }
            }
            return Some(column);
        }
        if !for_landing {
            let next_free = self
                .airfield_runway_next_in_line
                .get(&airfield_id)
                .and_then(|next| next.get(column))
                .copied()
                .flatten()
                .is_none();
            if next_free {
                if let Some(next) = self.airfield_runway_next_in_line.get_mut(&airfield_id) {
                    if let Some(slot) = next.get_mut(column) {
                        *slot = Some(jet_id);
                    }
                }
            }
        }
        None
    }

    /// Release any runway held by this jet (all airfields).
    pub(crate) fn release_airfield_runway_for_jet(&mut self, jet_id: ObjectId) {
        for slots in self.runway_reservations.values_mut() {
            for s in slots.iter_mut() {
                if *s == Some(jet_id) {
                    *s = None;
                }
            }
        }
        for next in self.airfield_runway_next_in_line.values_mut() {
            for s in next.iter_mut() {
                if *s == Some(jet_id) {
                    *s = None;
                }
            }
        }
        for (airfield_id, slots) in self.runway_reservations.iter() {
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(airfield_id) {
                for (idx, holder) in slots.iter().enumerate() {
                    if holder.is_none() {
                        if let Some(flag) = was.get_mut(idx) {
                            *flag = false;
                        }
                    }
                }
            }
        }
    }

    /// Count reserved runways at airfield.
    pub(crate) fn airfield_runway_reserved_count(&self, airfield_id: ObjectId) -> usize {
        self.runway_reservations
            .get(&airfield_id)
            .map(|s| s.iter().filter(|x| x.is_some()).count())
            .unwrap_or(0)
    }

    /// C++ JetAIUpdate runway takeoff residual: reserve runway, taxi to prep, then climb.
    ///
    /// Returns false when HasRunways and all runways are busy (jet stays docked).
    pub(crate) fn try_runway_takeoff_from_airfield(&mut self, jet_id: ObjectId) -> bool {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
        let (af_id, metadata, airfield_position) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !Self::is_aircraft(jet) {
                return false;
            }
            let parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            if !parked {
                // Already free — still clear any stale reservation.
                self.release_airfield_runway_for_jet(jet_id);
                return true;
            }
            let Some(af_id) = jet.contained_by else {
                return false;
            };
            let Some(airfield) = self
                .objects
                .get(&af_id)
                .filter(|airfield| Self::has_usable_airfield_parking_behavior(airfield))
            else {
                return false;
            };
            let Some(metadata) = airfield.thing.template.parking_place.clone() else {
                return false;
            };
            (af_id, metadata, airfield.get_position())
        };
        if !metadata.has_runways || metadata.runway_count().unwrap_or(0) == 0 {
            let parking = self.objects.get(&jet_id).map(|jet| jet.get_position());
            let is_heli = self
                .objects
                .get(&jet_id)
                .is_some_and(Self::object_is_produced_at_helipad);
            let released = self.launch_jet_from_airfield_parking(jet_id);
            if released {
                if is_heli {
                    if let Some(parking) = parking {
                        if let Some(jet) = self.objects.get_mut(&jet_id) {
                            jet.set_position(parking);
                        }
                        let approach = Self::helipad_approach_from_parking(
                            parking,
                            metadata.approach_height,
                            metadata.landing_deck_height_offset,
                        );
                        self.begin_heli_takeoff_or_landing(jet_id, af_id, parking, approach, false);
                    }
                } else if let Some(jet) = self.objects.get_mut(&jet_id) {
                    let mut position = jet.get_position();
                    position.y = position.y.max(
                        airfield_position.y
                            + metadata.landing_deck_height_offset
                            + metadata.approach_height,
                    );
                    jet.set_position(position);
                }
            }
            return released;
        }

        let Some(runway_idx) = self.reserve_airfield_runway(af_id, jet_id) else {
            // All runways busy — remain docked this frame.
            return false;
        };
        let ppinfo = self.calc_airfield_pp_info(af_id, jet_id);
        let jet_pos = self
            .objects
            .get(&jet_id)
            .map(|jet| jet.get_position())
            .unwrap_or(airfield_position);
        let (taxi, runway_end, runway_dist) = if let Some(info) = ppinfo {
            let mut path = Vec::new();
            let hangar_to_park = horiz_dist_sq(info.hangar_internal, info.parking_space) > 1.0;
            let at_parking = horiz_dist_sq(jet_pos, info.parking_space) <= 12.0 * 12.0;
            // C++ FROM_HANGAR then FROM_PARKING (intermediate + prep + start).
            if hangar_to_park && !at_parking {
                path.push(info.parking_space);
            }
            if let Some(inter) = taxi_intermediate_point(&info) {
                path.push(inter);
            }
            path.push(info.runway_prep);
            path.push(info.runway_start);
            (path, info.runway_end, info.runway_takeoff_dist.max(1.0))
        } else {
            let runway_count = metadata.runway_count().unwrap_or(1);
            let mut prep = airfield_position;
            prep.x += (runway_idx as f32 - (runway_count.saturating_sub(1) as f32 * 0.5))
                * PARKING_PLACE_RUNWAY_PREP_SPACING;
            prep.y += metadata.landing_deck_height_offset;
            (vec![prep], prep, 80.0)
        };
        let dest = *taxi.last().unwrap_or(&airfield_position);
        let via: Vec<glam::Vec3> = taxi
            .iter()
            .copied()
            .take(taxi.len().saturating_sub(1))
            .collect();
        let waited = self
            .airfield_runway_was_in_line
            .get(&af_id)
            .and_then(|flags| flags.get(runway_idx))
            .copied()
            .unwrap_or(false);
        // C++ taxi is ground SET_TAXIING — do not lift to ApproachHeight yet.
        if !self.uncontain_jet_for_ground_taxi(jet_id) {
            self.release_airfield_runway_for_jet(jet_id);
            return false;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            let mut pos = jet.get_position();
            pos.y = dest.y;
            jet.set_position(pos);
            jet.arm_jet_taxi_to_takeoff(dest, runway_end, runway_dist, waited);
        }
        if horiz_dist_sq(jet_pos, dest) <= 12.0 * 12.0 {
            self.begin_pause_after_taxi_to_runway(jet_id);
            return true;
        }
        if !self.assign_unit_path(jet_id, dest, &via) {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.movement.path = taxi.clone();
                jet.movement.current_path_index = 0;
                jet.movement.target_position = Some(dest);
                jet.set_ai_state(AIState::Moving);
                jet.set_status_moving(true);
            }
        }
        true
    }

    /// Release runway reservations once jets are clear of the airfield.
    pub(crate) fn tick_airfield_runway_clear(&mut self) {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let clear_sq = PARKING_PLACE_RUNWAY_CLEAR_DIST * PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let mut to_clear: Vec<(ObjectId, usize, bool)> = Vec::new();
        let airfields: Vec<ObjectId> = self.runway_reservations.keys().copied().collect();
        for af_id in airfields {
            let af_pos = match self.objects.get(&af_id) {
                Some(o) if Self::has_usable_airfield_parking_behavior(o) => o.get_position(),
                _ => {
                    self.runway_reservations.remove(&af_id);
                    self.airfield_runway_next_in_line.remove(&af_id);
                    self.airfield_runway_was_in_line.remove(&af_id);
                    continue;
                }
            };
            let Some(slots) = self.runway_reservations.get(&af_id) else {
                continue;
            };
            for (idx, holder) in slots.iter().enumerate() {
                let Some(jet_id) = *holder else {
                    continue;
                };
                let (clear, dead) = match self.objects.get(&jet_id) {
                    None => (true, true),
                    Some(jet) if !jet.is_alive() => (true, true),
                    Some(jet) if jet.contained_by.is_some() => (true, false),
                    Some(jet) => {
                        let p = jet.get_position();
                        let dx = p.x - af_pos.x;
                        let dz = p.z - af_pos.z;
                        let d2 = dx * dx + dz * dz;
                        let far = jet.status.airborne_target && d2 >= clear_sq;
                        // C++ transfers during pause + every takeoff-roll frame.
                        (far || jet.jet_should_transfer_runway(self.frame), false)
                    }
                };
                if clear {
                    to_clear.push((af_id, idx, dead));
                }
            }
        }
        for (af_id, idx, dead) in to_clear {
            let next = self
                .airfield_runway_next_in_line
                .get(&af_id)
                .and_then(|next| next.get(idx))
                .copied()
                .flatten()
                .filter(|_| !dead);
            if let Some(slots) = self.runway_reservations.get_mut(&af_id) {
                if idx < slots.len() {
                    slots[idx] = next;
                }
            }
            if let Some(queue) = self.airfield_runway_next_in_line.get_mut(&af_id) {
                if idx < queue.len() {
                    queue[idx] = None;
                }
            }
            if let Some(was) = self.airfield_runway_was_in_line.get_mut(&af_id) {
                if idx < was.len() {
                    was[idx] = next.is_some();
                }
            }
        }
    }

    /// C++ `JetAIUpdate::doLandingCommand` — reserve, re-home producer, RTB.
    pub(crate) fn do_jet_landing_command(
        &mut self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
    ) -> bool {
        let Some(jet) = self.objects.get(&jet_id) else {
            return false;
        };
        if !jet.is_alive() || !Self::is_aircraft(jet) {
            return false;
        }
        if jet.jet_ai.landing_in_progress {
            return true;
        }
        if Self::object_is_produced_at_helipad(jet) {
            return false;
        }
        let airfield_ok = self.objects.get(&airfield_id).is_some_and(|af| {
            af.is_alive()
                && Self::has_usable_airfield_parking_behavior(af)
                && af.is_kind_of(KindOf::FSAirfield)
        });
        if !airfield_ok {
            return false;
        }
        if jet.contained_by == Some(airfield_id) && jet.ai_state == AIState::Docked {
            return true;
        }
        let old_producer = jet.producer_id;
        if old_producer.is_some() && old_producer != Some(airfield_id) {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
        }
        if self
            .reserve_airfield_parking_space(airfield_id, jet_id)
            .is_none()
        {
            return false;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.producer_id = Some(airfield_id);
            jet.return_to_base_requested = true;
            jet.jet_ai.landing_in_progress = true;
            jet.jet_ai.allow_interrupt_for_reload = false;
            jet.target = None;
            jet.set_status_attacking(false);
        }
        self.try_return_to_base_rearm(jet_id)
    }

    pub(crate) fn try_jet_enter_or_repair_airfield(
        &mut self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
    ) -> bool {
        self.do_jet_landing_command(jet_id, airfield_id)
    }

    /// Undock pairing for the dock-time container listing: drop the jet from
    /// the airfield's parked-aircraft list (contained_units mirror).
    fn remove_jet_from_airfield_occupants(&mut self, jet_id: ObjectId, af_hint: Option<ObjectId>) {
        let Some(af_id) = af_hint else {
            return;
        };
        let Some(airfield) = self.objects.get_mut(&af_id) else {
            return;
        };
        if let Some(building) = airfield.building_data.as_mut() {
            if let Some(pos) = building.garrisoned_units.iter().position(|&id| id == jet_id) {
                building.garrisoned_units.remove(pos);
                crate::game_logic::host_contain_log::record_garrison(
                    airfield.id,
                    &building.garrisoned_units,
                    building.max_garrison.min(u16::MAX as usize) as u16,
                );
            }
        } else if let Some(pos) = airfield.occupants.iter().position(|&id| id == jet_id) {
            airfield.occupants.remove(pos);
            crate::game_logic::host_contain_log::record_garrison(airfield.id, &airfield.occupants, 0);
        }
    }

    /// C++ JetOrHeliTaxiState::onEnter — uncontain, SET_TAXIING, stay on deck.
    fn uncontain_jet_for_ground_taxi(&mut self, jet_id: ObjectId) -> bool {
        self.set_airfield_healee_for_jet(jet_id, false);
        let af_hint = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            jet.contained_by.or(jet.producer_id)
        };
        let was_parked = self
            .objects
            .get(&jet_id)
            .is_some_and(|jet| jet.is_parked_at_airfield() || jet.contained_by.is_some());
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.set_contained_by(None);
            jet.status.airborne_target = false;
            // C++ JetOrHeliTaxiState::onEnter: chooseLocomotorSet then
            // setUsePreciseZPos + setUltraAccurate (JetAIUpdate.cpp:615-616).
            // Bind first — apply_host_locomotor_binding clears both flags.
            jet.apply_taxiing_locomotor_set();
            jet.set_precise_z_and_ultra_accurate(true);
        }
        self.remove_jet_from_airfield_occupants(jet_id, af_hint);
        if let Some(af_id) = af_hint {
            self.sync_airfield_hangar_doors(af_id);
        }
        was_parked || af_hint.is_some()
    }

    /// C++ takeoff onExit: uncontain, keep stall when `keepsParkingSpaceWhenAirborne`.
    fn launch_jet_from_airfield_parking(&mut self, jet_id: ObjectId) -> bool {
        self.set_airfield_healee_for_jet(jet_id, false);
        let af_hint = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            jet.contained_by.or(jet.producer_id)
        };
        let af_hint = af_hint.or_else(|| {
            self.airfield_parking_spaces
                .iter()
                .find_map(|(&airfield_id, spaces)| {
                    spaces
                        .iter()
                        .any(|space| space.object_id == Some(jet_id))
                        .then_some(airfield_id)
                })
        });
        let keep_stall = self
            .objects
            .get(&jet_id)
            .is_some_and(Self::object_keeps_parking_space_when_airborne);
        let took_off = {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                return false;
            };
            let was_parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            let af = jet.takeoff_from_airfield_parking();
            jet.set_contained_by(None);
            was_parked || af.is_some()
        };
        self.remove_jet_from_airfield_occupants(jet_id, af_hint);
        let released_from = if keep_stall {
            None
        } else {
            self.release_airfield_parking_space_for_jet(jet_id)
        };
        if let Some(af_id) = released_from.or(af_hint) {
            self.sync_airfield_hangar_doors(af_id);
        }
        took_off || released_from.is_some() || (keep_stall && af_hint.is_some())
    }

    /// C++ takeoff onExit (`JetTakeoffOrLandingState` 897-900): uncontain,
    /// keep stall when `keepsParkingSpaceWhenAirborne` (default true).
    pub(crate) fn release_jet_from_airfield_parking(&mut self, jet_id: ObjectId) -> bool {
        self.launch_jet_from_airfield_parking(jet_id)
    }

    /// C++ `JetOrHeliReturnForLandingState::onEnter`: first use the producer
    /// parking place only when it still has the jet's exact controller.  If
    /// that concrete producer is absent/stale, scan only explicitly allied
    /// ParkingPlaceBehavior airfields and reserve a real authored space.
    fn select_and_reserve_airfield_for_return(&mut self, jet_id: ObjectId) -> Option<ObjectId> {
        let (producer_id, jet_position) = {
            let jet = self.objects.get(&jet_id)?;
            (jet.producer_id, jet.get_position())
        };

        if let Some(producer_id) = producer_id {
            let producer_is_usable = self
                .objects
                .get(&producer_id)
                .is_some_and(Self::has_usable_airfield_parking_behavior);
            // C++ JetAIUpdate asks getPP(producerID) first: the producer's
            // exact controller is authoritative even when its authored
            // ParkingPlace cannot currently reserve a space (full hangar).
            // Falling through to findSuitableAirfield there would route the
            // jet into a nearer same-faction other-player airfield.
            if producer_is_usable && self.airfield_has_exact_controller_for_jet(jet_id, producer_id)
            {
                if self.reserve_airfield_parking_space(producer_id, jet_id).is_some()
                    || self.airfield_has_reserved_space(producer_id, jet_id)
                {
                    return Some(producer_id);
                }
            }

            // C++ getPP(producerID) (JetOrHeliReturnForLandingState::onEnter,
            // JetAIUpdate.cpp:1509-1511) keeps the producer by liveness alone,
            // and doLandingCommand (JetAIUpdate.cpp:2277-2312) accepts the
            // commanded airfield with no ownership/relationship gate. An
            // ownerless legacy airfield bound to the jet's live landing/RTB
            // leg therefore stays the target; owner-stamped airfields keep
            // the strict exact-controller/allied checks above.
            if producer_is_usable
                && self.jet_rtb_leg_bound_to_ownerless_airfield(jet_id, producer_id)
                && (self.reserve_airfield_parking_space(producer_id, jet_id).is_some()
                    || self.airfield_has_reserved_space(producer_id, jet_id))
            {
                return Some(producer_id);
            }

            // C++ clears an unusable producer before `findSuitableAirfield`.
            // Release the old reservation first so a captured/dead airfield
            // cannot retain an unavailable aircraft slot.
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                if jet.producer_id == Some(producer_id) {
                    jet.producer_id = None;
                }
            }
        }

        let mut candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(&airfield_id, airfield)| {
                (airfield_id != jet_id
                    && Self::has_usable_airfield_parking_behavior(airfield)
                    && self.is_friendly_airfield(jet_id, airfield_id))
                .then(|| {
                    (
                        airfield_id,
                        airfield.get_position().distance_squared(jet_position),
                    )
                })
            })
            .collect();
        candidates.sort_by(|(left_id, left_distance), (right_id, right_distance)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left_id.0.cmp(&right_id.0))
        });
        candidates.into_iter().find_map(|(airfield_id, _)| {
            self.reserve_airfield_parking_space(airfield_id, jet_id)
                .map(|_| airfield_id)
        })
    }

    /// Ownerless legacy airfield bound to a jet's live landing/RTB leg.
    /// C++ keeps the producer airfield by liveness alone
    /// (JetOrHeliReturnForLandingState::onEnter getPP(producerID),
    /// JetAIUpdate.cpp:1509-1511) and `doLandingCommand`
    /// (JetAIUpdate.cpp:2277-2312) accepts the commanded airfield without an
    /// ownership/relationship check; a C++ team-owned airfield always has a
    /// controlling player, so the host's ownerless legacy objects never hit
    /// the one-sided-owner veto while their jet flies an explicit leg.
    fn jet_rtb_leg_bound_to_ownerless_airfield(
        &self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
    ) -> bool {
        let Some(airfield) = self.objects.get(&airfield_id) else {
            return false;
        };
        if airfield.owner_player_id.is_some() || !Self::has_usable_airfield_parking_behavior(airfield)
        {
            return false;
        }
        self.objects.get(&jet_id).is_some_and(|jet| {
            jet.is_alive()
                && jet.producer_id == Some(airfield_id)
                && (jet.jet_ai.landing_in_progress
                    || jet.return_to_base_requested
                    || jet.contained_by == Some(airfield_id))
        })
    }

    /// Physical ReturnToBase command authority.  The command executor passes
    /// a frozen player id, then this method revalidates that exact ownership
    /// before it writes a request or reserves an airfield slot.
    pub(crate) fn request_return_to_base(&mut self, jet_id: ObjectId, player_id: u32) -> bool {
        let can_request = self.objects.get(&jet_id).is_some_and(|jet| {
            jet.is_alive()
                && Self::is_aircraft(jet)
                && jet.owner_player_id == Some(player_id)
                && self.player_owner_for_host_object(jet) == Some(player_id)
        });
        if !can_request {
            return false;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.return_to_base_requested = true;
        }
        if self.try_return_to_base_rearm(jet_id) {
            true
        } else {
            // A physical command that cannot reserve a C++-valid parking
            // place is not accepted optimistically.
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.return_to_base_requested = false;
            }
            false
        }
    }

    /// C++ JetAIUpdate return/land/rearm residual.  Every tick revalidates
    /// the selected producer or allied airfield before it can keep a parking
    /// reservation, so capture/destruction cannot route a jet into an enemy
    /// or same-faction other-player hangar.
    pub(crate) fn try_return_to_base_rearm(&mut self, jet_id: ObjectId) -> bool {
        let (needs_rearm, requested, jet_position) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !jet.is_alive() || !Self::is_aircraft(jet) {
                return false;
            }
            (
                jet.needs_return_to_base_rearm(),
                jet.return_to_base_requested,
                jet.get_position(),
            )
        };
        // A docked jet mid-clip-reload no longer reads needs_rearm (clip is
        // partially full), but C++ RELOAD_AMMO keeps running to its
        // m_reloadDoneFrame (JetAIUpdate.cpp:1464-1470) — the parked rearm
        // tick must still progress and complete through this entry point.
        let docked_rearm_pending = self.objects.get(&jet_id).is_some_and(|jet| {
            jet.contained_by.is_some() && jet.airfield_rearm_ready_frame.is_some()
        });
        // A jet flying an in-progress RTB landing leg (APPROACH/LANDING/TAXI)
        // keeps progressing through the leg regardless of its current ammo
        // state: C++ JetAIUpdate drives RETURNING_FOR_LANDING → LANDING →
        // DOING_LANDING/TAXI as sequential states once entered
        // (JetAIUpdate.cpp:1509-1541, 2277-2312); isOutOfSpecialReloadAmmo
        // only gates entering the return, not continuing an open leg.
        let rtb_leg_in_progress = self
            .objects
            .get(&jet_id)
            .is_some_and(|jet| jet.jet_ai.landing_in_progress || jet.jet_ai.rtb_landing_phase != 0);
        if !needs_rearm && !requested && !docked_rearm_pending && !rtb_leg_in_progress {
            return false;
        }

        let Some(airfield_id) = self.select_and_reserve_airfield_for_return(jet_id) else {
            return false;
        };
        let Some((metadata, airfield_position)) =
            self.objects.get(&airfield_id).and_then(|airfield| {
                Self::has_usable_airfield_parking_behavior(airfield)
                    .then(|| {
                        airfield
                            .thing
                            .template
                            .parking_place
                            .clone()
                            .map(|metadata| (metadata, airfield.get_position()))
                    })
                    .flatten()
            })
        else {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            return false;
        };
        if !(self.airfield_has_exact_controller_for_jet(jet_id, airfield_id)
            || self.is_friendly_airfield(jet_id, airfield_id)
            || self.jet_rtb_leg_bound_to_ownerless_airfield(jet_id, airfield_id))
        {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            return false;
        }
        if self
            .objects
            .get(&jet_id)
            .is_some_and(Self::object_is_produced_at_helipad)
        {
            return self.continue_helipad_landing(jet_id, airfield_id, &metadata);
        }
        if !self.airfield_has_reserved_space(airfield_id, jet_id) {
            let _ = self.release_airfield_parking_space_for_jet(jet_id);
            return false;
        }

        let already_docked = self
            .objects
            .get(&jet_id)
            .is_some_and(|jet| jet.contained_by == Some(airfield_id));
        let runway_index = if already_docked || !metadata.has_runways {
            None
        } else {
            match self.reserve_airfield_runway_ex(airfield_id, jet_id, true) {
                Some(index) => Some(index),
                None => {
                    // C++ LANDING_AWAIT_CLEARANCE — keep the stall, wait for the strip.
                    return self.airfield_has_reserved_space(airfield_id, jet_id);
                }
            }
        };

        let ppinfo = self.calc_airfield_pp_info(airfield_id, jet_id);
        let pad = if let Some(info) = ppinfo {
            info.parking_space
        } else {
            use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
            let mut pad = airfield_position;
            if let Some(index) = runway_index {
                let runway_count = metadata.runway_count().unwrap_or(1);
                pad.x += (index as f32 - (runway_count.saturating_sub(1) as f32 * 0.5))
                    * PARKING_PLACE_RUNWAY_PREP_SPACING;
            }
            pad.y += metadata.landing_deck_height_offset;
            pad
        };
        let prep = ppinfo.map(|info| info.runway_prep).unwrap_or(pad);
        let approach = ppinfo.map(|info| info.runway_approach).unwrap_or(pad);
        let runway_end = ppinfo.map(|info| info.runway_end).unwrap_or(approach);
        let runway_start = ppinfo.map(|info| info.runway_start).unwrap_or(pad);
        let intermediate = ppinfo.and_then(|info| taxi_intermediate_point(&info));
        let (phase, airborne) = self
            .objects
            .get(&jet_id)
            .map(|jet| (jet.jet_ai.rtb_landing_phase, jet.status.airborne_target))
            .unwrap_or((0, true));
        const WAYPOINT: f32 = 12.0;
        const WAYPOINT_SQ: f32 = WAYPOINT * WAYPOINT;
        const APPROACH_RANGE_SQ: f32 = 120.0 * 120.0;

        if already_docked
            || (horiz_dist_sq(jet_position, pad) <= WAYPOINT_SQ
                && (phase >= crate::game_logic::object::JET_RTB_PHASE_TAXI || !airborne))
        {
            return self.dock_jet_at_airfield_pad(jet_id, airfield_id, pad);
        }

        // C++ JetAIUpdate re-evaluates its state machine every update. When a
        // previously assigned RTB approach/taxi path has already been consumed
        // (empty path, idle, no target) the next call must progress to the
        // dock instead of re-issuing an identical approach leg forever.
        // C++ JetOrHeliTaxiState::onEnter destroys the obsolete move and
        // rebuilds the taxi leg, and arrival hands off to
        // JetOrHeliParkOrientState whose update() snaps the jet onto
        // parkingSpace (JetAIUpdate.cpp:1188-1195 setPosition(hoverloc)). A
        // taxi-phase jet inside the parking apron (within one taxi spacing of
        // its reserved pad) whose current move is NOT a fresh taxi leg toward
        // the pad — idle, or still carrying a stale approach leg — is parked
        // and docks immediately.
        let rtb_path_complete = self.objects.get(&jet_id).is_some_and(|jet| {
            jet.movement.path.is_empty()
                && !jet.status.moving
                && jet.movement.target_position.is_none()
                && jet.ai_state == AIState::Moving
        });
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
        let apron_reach_sq = {
            let pad_reach =
                horiz_dist_sq(airfield_position, pad).sqrt() + PARKING_PLACE_RUNWAY_PREP_SPACING;
            pad_reach * pad_reach
        };
        let apron_arrived = self.objects.get(&jet_id).is_some_and(|jet| {
            jet.jet_ai.rtb_landing_phase >= crate::game_logic::object::JET_RTB_PHASE_TAXI
                && jet.movement.target_position != Some(pad)
                && horiz_dist_sq(jet.get_position(), pad) <= apron_reach_sq
        });
        if rtb_path_complete || apron_arrived {
            return self.dock_jet_at_airfield_pad(jet_id, airfield_id, pad);
        }
        if phase >= crate::game_logic::object::JET_RTB_PHASE_TAXI
            || (phase >= crate::game_logic::object::JET_RTB_PHASE_LANDING
                && horiz_dist_sq(jet_position, runway_start) <= WAYPOINT_SQ)
        {
            if phase < crate::game_logic::object::JET_RTB_PHASE_TAXI {
                // Live-host wheels-down: taxi start is first ground contact
                // when the landing path never lowered Y to the deck.
                self.play_jet_wheel_screech(jet_id, true);
            }
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
                jet.jet_ai.landing_in_progress = false;
                jet.status.airborne_target = false;
                jet.apply_taxiing_locomotor_set();
                // C++ JetOrHeliTaxiState::onEnter (JetAIUpdate.cpp:615-616).
                jet.set_precise_z_and_ultra_accurate(true);
                jet.target = None;
                jet.set_status_attacking(false);
            }
            self.release_airfield_runway_for_jet(jet_id);
            let mut taxi = vec![prep];
            if let Some(inter) = intermediate {
                taxi.push(inter);
            }
            taxi.push(pad);
            return self.assign_rtb_path(jet_id, &taxi);
        }

        if horiz_dist_sq(jet_position, approach) > APPROACH_RANGE_SQ {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_APPROACH;
                jet.jet_ai.landing_in_progress = true;
                jet.apply_airborne_locomotor_set();
                jet.target = None;
                jet.set_status_attacking(false);
                jet.set_ai_state(AIState::Moving);
            }
            // Host-immediate Moving + decision log (dock pattern): GameWorld
            // stays last-writer for the RTB AI state under decision authority.
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                let ordinal = crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                    &AIState::Moving,
                );
                crate::game_logic::host_ai_decision_log::record_set_state(jet_id, ordinal);
            }
            // C++ aircraft approach legs never fail closed on the ground
            // grid: RETURNING_FOR_LANDING's onEnter issues the move
            // (JetAIUpdate.cpp:1536-1541) and AIInternalMoveToState flies the
            // raw goal. Install the approach leg directly when the leftover
            // pathfinder refuses an off-grid air goal so an accepted landing
            // command is not rejected after its reservation was made.
            return self.assign_rtb_path(jet_id, &[approach]);
        }

        if let Some(jet) = self.objects.get_mut(&jet_id) {
            if jet.jet_ai.rtb_landing_phase != crate::game_logic::object::JET_RTB_PHASE_LANDING {
                // C++ JetTakeoffOrLandingState::onEnter m_landingSoundPlayed = FALSE
                jet.jet_ai.landing_sound_played = false;
            }
            jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_LANDING;
            jet.jet_ai.landing_in_progress = true;
            jet.apply_airborne_locomotor_set();
            // C++ JetTakeoffOrLandingState::onEnter (JetAIUpdate.cpp:725-726).
            jet.set_precise_z_and_ultra_accurate(true);
            // C++ JetTakeoffOrLandingState::onEnter landing: setMaxSpeed(getMinSpeed()).
            if jet.min_speed > 0.0 {
                jet.movement.max_speed = jet.min_speed;
            }
            jet.target = None;
            jet.set_status_attacking(false);
        }
        let ok = self.assign_rtb_path(jet_id, &[approach, runway_end, runway_start]);
        self.maybe_play_jet_wheel_screech(jet_id);
        ok
    }

    fn assign_rtb_path(&mut self, jet_id: ObjectId, points: &[glam::Vec3]) -> bool {
        if points.is_empty() {
            return false;
        }
        let dest = *points.last().unwrap();
        let via: Vec<glam::Vec3> = points[..points.len() - 1].to_vec();
        if self.assign_unit_path(jet_id, dest, &via) {
            return true;
        }
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.target = None;
            jet.set_status_attacking(false);
            jet.set_ai_state(AIState::Moving);
            jet.movement.path = points.to_vec();
            jet.movement.current_path_index = 0;
            jet.movement.target_position = Some(dest);
            jet.set_status_moving(true);
        }
        true
    }

    fn dock_jet_at_airfield_pad(
        &mut self,
        jet_id: ObjectId,
        airfield_id: ObjectId,
        pad: glam::Vec3,
    ) -> bool {
        let parking_orientation = self
            .calc_airfield_pp_info(airfield_id, jet_id)
            .map(|info| info.parking_orientation);
        {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                self.release_airfield_runway_for_jet(jet_id);
                return false;
            };
            jet.set_contained_by(Some(airfield_id));
            jet.set_ai_state(AIState::Docked);
            jet.set_status_moving(false);
            jet.status.airborne_target = false;
            jet.jet_ai.landing_in_progress = false;
            jet.jet_ai.rtb_landing_phase = 0;
            jet.apply_taxiing_locomotor_set();
            // C++ JetOrHeliTaxiState / JetTakeoffOrLandingState::onExit.
            jet.set_precise_z_and_ultra_accurate(false);
            jet.set_allow_invalid_position(false);
            jet.target = None;
            jet.movement.path.clear();
            jet.movement.current_path_index = 0;
            jet.movement.target_position = None;
            jet.set_position(pad);
            // C++ JetOrHeliParkOrientState: setLocomotorGoalOrientation(parkingOrientation).
            // Live dock snaps pose; leftover already matches C++.
            if let Some(orient) = parking_orientation {
                jet.set_orientation(orient);
            }
            jet.return_to_base_requested = false;
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(jet_id, 12);
                crate::game_logic::host_move_log::record(jet_id, Some([pad.x, pad.y, pad.z]));
                jet.record_host_movement();
            }
            jet.begin_parked_airfield_rearm(self.frame);
            let _ = jet.tick_parked_airfield_rearm(self.frame);
        }
        // C++ ParkingPlaceBehavior holds the stall (m_objectID) so the airfield
        // knows its parked aircraft; live pairs the docked contained_by marker
        // with the container listing (contained_units mirror). Direct list
        // push: the parking reservation is the capacity authority, not the
        // garrison max.
        if let Some(airfield) = self.objects.get_mut(&airfield_id) {
            if let Some(building) = airfield.building_data.as_mut() {
                if !building.garrisoned_units.contains(&jet_id) {
                    building.garrisoned_units.push(jet_id);
                    crate::game_logic::host_contain_log::record_garrison(
                        airfield.id,
                        &building.garrisoned_units,
                        building.max_garrison.min(u16::MAX as usize) as u16,
                    );
                }
            } else if !airfield.occupants.contains(&jet_id) {
                airfield.occupants.push(jet_id);
                crate::game_logic::host_contain_log::record_garrison(
                    airfield.id,
                    &airfield.occupants,
                    0,
                );
            }
        }
        self.release_airfield_runway_for_jet(jet_id);
        self.sync_airfield_hangar_doors(airfield_id);
        if let Some(jet) = self.objects.get_mut(&jet_id) {
            jet.begin_parked_airfield_rearm(self.frame);
            let _ = jet.tick_parked_airfield_rearm(self.frame);
        }
        true
    }

    /// C++ ParkingPlaceBehavior heal residual for docked aircraft at airfields.
    pub(crate) fn tick_airfield_parking_heal(&mut self) {
        self.tick_airfield_parking_lifecycle();
        self.tick_heli_takeoff_or_landing();
        self.tick_flight_decks();
        self.sync_airfield_healees();
        self.pulse_airfield_healees();

        use crate::game_logic::host_countermeasures::aircraft_has_countermeasures_upgrade;
        // Rearm / CM reload for docked aircraft. Helos skip hangar stalls.
        let jet_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && o.contained_by.is_some()
                    && (o.ai_state == AIState::Docked
                        || crate::gameworld_shadow::gameworld_ai_decision_authority_live())
                    && (o.needs_return_to_base_rearm()
                        || o.airfield_rearm_ready_frame.is_some()
                        || aircraft_has_countermeasures_upgrade(&o.applied_upgrades))
            })
            .map(|(id, _)| *id)
            .collect();
        for jid in jet_ids {
            let (airfield_id, has_cm, helipad) = {
                let Some(jet) = self.objects.get(&jid) else {
                    continue;
                };
                let Some(af_id) = jet.contained_by else {
                    continue;
                };
                (
                    af_id,
                    aircraft_has_countermeasures_upgrade(&jet.applied_upgrades),
                    Self::object_is_produced_at_helipad(jet),
                )
            };
            if !self.normalize_airfield_parking_spaces(airfield_id) {
                continue;
            }
            if !helipad && !self.airfield_has_reserved_space(airfield_id, jid) {
                continue;
            }
            if has_cm {
                self.countermeasures.reload_at_airfield(jid);
            }
            self.snapshot_jet_producer_location(jid);
            if let Some(jet) = self.objects.get_mut(&jid) {
                jet.begin_parked_airfield_rearm(self.frame);
                let _ = jet.tick_parked_airfield_rearm(self.frame);
            }
        }
        self.tick_helipad_repaired_auto_takeoff();
    }

    /// C++ `ParkingPlaceBehavior::update` 5 Hz `attemptHealing` pulse.
    fn pulse_airfield_healees(&mut self) {
        let now = self.frame;
        let airfields: Vec<ObjectId> = self.airfield_healing.keys().copied().collect();
        for airfield_id in airfields {
            let next = self
                .airfield_next_heal_frame
                .get(&airfield_id)
                .copied()
                .unwrap_or(AIRFIELD_HEAL_FOREVER);
            if now < next {
                continue;
            }
            self.airfield_next_heal_frame
                .insert(airfield_id, now.saturating_add(AIRFIELD_HEAL_RATE_FRAMES));
            let heal_per_sec = self
                .objects
                .get(&airfield_id)
                .and_then(|airfield| airfield.thing.template.parking_place.as_ref())
                .map(|metadata| metadata.heal_amount_per_second)
                .unwrap_or(0.0);
            let amount = AIRFIELD_HEAL_RATE_FRAMES as f32 * heal_per_sec * LOGIC_FRAME_TIMESTEP;
            let healees: Vec<ObjectId> = self
                .airfield_healing
                .get(&airfield_id)
                .map(|list| list.iter().map(|info| info.getting_healed_id).collect())
                .unwrap_or_default();
            let mut dead = Vec::new();
            for jet_id in healees {
                let Some(jet) = self.objects.get(&jet_id) else {
                    dead.push(jet_id);
                    continue;
                };
                if !jet.is_alive() {
                    dead.push(jet_id);
                    continue;
                }
                if amount > 0.0 && jet.health.current + 1e-3 < jet.health.maximum {
                    if let Some(jet) = self.objects.get_mut(&jet_id) {
                        let _ = jet.take_damage_from_typed(
                            amount,
                            Some(airfield_id),
                            crate::game_logic::combat::DamageType::Healing,
                        );
                    }
                }
            }
            for jet_id in dead {
                self.set_airfield_healee(airfield_id, jet_id, false);
            }
        }
    }

    /// C++ ParkingPlaceBehavior stall occupancy for WorldSnapshot.
    pub fn snapshot_airfield_parking_spaces(
        &self,
    ) -> Vec<(ObjectId, Vec<(Option<ObjectId>, bool)>)> {
        let mut rows: Vec<_> = self
            .airfield_parking_spaces
            .iter()
            .map(|(&id, spaces)| {
                (
                    id,
                    spaces
                        .iter()
                        .map(|space| (space.object_id, space.reserved_for_exit))
                        .collect(),
                )
            })
            .collect();
        rows.sort_by_key(|(id, _)| id.0);
        rows
    }

    pub fn restore_airfield_parking_spaces(
        &mut self,
        rows: Vec<(ObjectId, Vec<(Option<ObjectId>, bool)>)>,
    ) {
        self.airfield_parking_spaces.clear();
        for (id, spaces) in rows {
            self.airfield_parking_spaces.insert(
                id,
                spaces
                    .into_iter()
                    .map(|(object_id, reserved_for_exit)| AirfieldParkingSpace {
                        object_id,
                        reserved_for_exit,
                    })
                    .collect(),
            );
        }
    }

    pub fn snapshot_runway_reservations(&self) -> Vec<(ObjectId, Vec<Option<ObjectId>>)> {
        let mut rows: Vec<_> = self
            .runway_reservations
            .iter()
            .map(|(&id, slots)| (id, slots.clone()))
            .collect();
        rows.sort_by_key(|(id, _)| id.0);
        rows
    }

    pub fn restore_runway_reservations(&mut self, rows: Vec<(ObjectId, Vec<Option<ObjectId>>)>) {
        self.runway_reservations = rows.into_iter().collect();
    }

    pub fn snapshot_airfield_runway_next_in_line(&self) -> Vec<(ObjectId, Vec<Option<ObjectId>>)> {
        let mut rows: Vec<_> = self
            .airfield_runway_next_in_line
            .iter()
            .map(|(&id, slots)| (id, slots.clone()))
            .collect();
        rows.sort_by_key(|(id, _)| id.0);
        rows
    }

    pub fn restore_airfield_runway_next_in_line(
        &mut self,
        rows: Vec<(ObjectId, Vec<Option<ObjectId>>)>,
    ) {
        self.airfield_runway_next_in_line = rows.into_iter().collect();
    }

    pub fn snapshot_airfield_runway_was_in_line(&self) -> Vec<(ObjectId, Vec<bool>)> {
        let mut rows: Vec<_> = self
            .airfield_runway_was_in_line
            .iter()
            .map(|(&id, slots)| (id, slots.clone()))
            .collect();
        rows.sort_by_key(|(id, _)| id.0);
        rows
    }

    pub fn restore_airfield_runway_was_in_line(&mut self, rows: Vec<(ObjectId, Vec<bool>)>) {
        self.airfield_runway_was_in_line = rows.into_iter().collect();
    }
}
