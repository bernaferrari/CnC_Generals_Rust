//! Stealth and detector updates.

use super::*;

impl GameLogic {
    /// C++ StealthUpdate + StealthDetectorUpdate for one logic frame.
    ///
    /// - Runs every awake authored StealthUpdate module, including detection expiry
    /// - Mines keep their innate cloak; detectors mark nearby enemy stealth as detected
    pub fn update_stealth_and_detection(&mut self) {
        self.update_stealth_and_detection_with_effects(&mut Default::default())
    }

    pub(crate) fn update_stealth_and_detection_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let frame = self.frame;
        // C++ StealthUpdate.cpp:725-780 cloak/destalth/detect SoundStealthOn/Off.
        let stealth_snap: Vec<(ObjectId, bool, bool)> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, o)| (*id, o.status.stealthed, o.status.detected))
            .collect();


        // C++ StealthUpdate::update for every authored module.
        self.update_stealth_update_modules(frame);

        // Mines / demo traps / charges: InnateStealth + 0 opacity
        // (C++ StealthUpdate.cpp mine setEffectiveOpacity(0,0)).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::{
                MINE_STEALTH_OPACITY_RESIDUAL, is_mine_stealth_kind_residual,
            };
            let mine_ids: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| {
                    o.is_alive()
                        && is_mine_stealth_kind_residual(
                            o.is_kind_of(KindOf::Mine),
                            o.is_kind_of(KindOf::DemoTrap),
                            o.mine_data.is_some(),
                        )
                })
                .map(|(id, _)| *id)
                .collect();
            for mid in mine_ids {
                let Some(obj) = self.objects.get_mut(&mid) else {
                    continue;
                };
                if !obj.status.stealthed {
                    obj.apply_mine_innate_stealth();
                } else {
                    obj.apply_mine_no_attack_from_ai();
                    if (obj.camo_friendly_opacity - MINE_STEALTH_OPACITY_RESIDUAL).abs() > 1e-4 {
                        obj.camo_friendly_opacity = MINE_STEALTH_OPACITY_RESIDUAL;
                    }
                }
            }
        }
        // The batched StealthUpdate work precedes detector scans. Its
        // setStealthLook material command must also precede the detector's
        // setSecondMaterialPassOpacity(1), just as in those native updates.
        self.apply_local_stealth_look_and_heat_vision();


        // Collect active detectors (alive, not under construction) that are due
        // for a StealthDetectorUpdate DetectionRate scan.
        use crate::game_logic::host_strategy_center::{
            stealth_detector_hold_frames, stealth_detector_next_scan_frame,
            stealth_detector_scan_due,
        };
        #[derive(Clone, Copy)]
        struct DetFlags {
            extra_required: u128,
            extra_forbidden: u128,
        }
        let mut detectors: Vec<(ObjectId, Team, Option<u32>, Vec3, f32, DetFlags, u32)> =
            Vec::new();
        let mut scanned_detector_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in self.objects.registered() {
            if !(o.is_detector
                && o.is_alive()
                && !o.status.under_construction
                && !o.status.destroyed
                && !o.status.sold)
            {
                continue;
            }
            if !o.stealth_or_detector_update_processes() {
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
            // C++ StealthDetectorUpdate.cpp:139-166 — contained detectors
            // sleep unless CanDetectWhileGarrisoned / CanDetectWhileTransported.
            if o.is_contained() {
                use crate::game_logic::host_radar_stealth_vision_residual::{
                    DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL,
                    DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL,
                    detector_can_scan_while_contained_residual,
                };
                let garrisonable = o
                    .contained_by
                    .and_then(|cid| self.objects.get(&cid))
                    .map(|c| {
                        c.thing().template.garrison_contain_max.is_some()
                            || (c.is_kind_of(KindOf::Structure) && c.building_data.is_some())
                    })
                    .unwrap_or(false);
                if !detector_can_scan_while_contained_residual(
                    true,
                    garrisonable,
                    DETECTOR_CAN_DETECT_WHILE_GARRISONED_CTOR_DEFAULT_RESIDUAL,
                    DETECTOR_CAN_DETECT_WHILE_TRANSPORTED_CTOR_DEFAULT_RESIDUAL,
                ) {
                    scanned_detector_ids.push(*id);
                    continue;
                }
            }
            let flags = DetFlags {
                extra_required: o.extra_detect_kindof,
                extra_forbidden: o.extra_detect_kindof_not,
            };
            detectors.push((
                *id,
                o.team,
                o.owner_player_id,
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
            self.play_stealth_transition_sounds(&stealth_snap);
            return;
        }

        // Leftover StealthDetectorUpdate::update clears IR grids at each scan
        // wake, then re-creates them for every in-range detected target.
        {
            let scanning: Vec<ObjectId> = detectors.iter().map(|d| d.0).collect();
            self.clear_detector_ir_grids_for(&scanning, effects);
        }

        // Collect stealthed targets first to avoid borrow conflicts.
        let stealthed_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive() && (o.status.stealthed || o.status.disguised))
            .map(|(id, _)| *id)
            .collect();
        let mut spotted_detectors: std::collections::HashSet<ObjectId> =
            std::collections::HashSet::new();

        for sid in stealthed_ids {
            let Some((s_team, s_owner, s_pos, already_detected, s_kind)) =
                self.objects.get(&sid).map(|o| {
                    (
                        o.team,
                        o.owner_player_id,
                        o.get_position(),
                        o.status.detected,
                        o.kind_of_cpp_mask(),
                    )
                })
            else {
                continue;
            };

            // C++ markAsDetected(updateRate + 1); take max hold among detecting scanners.
            let mut best_expires: u32 = 0;
            let mut spotting_detectors: Vec<ObjectId> = Vec::new();
            let detected_by_someone =
                detectors
                    .iter()
                    .fold(false, |found, (id, det_team, det_owner, det_pos, range, flags, rate)| {
                        // C++ PartitionFilterRelationship ALLOW_ENEMIES|ALLOW_NEUTRAL.
                        let rel_ok = match (*det_owner, s_owner) {
                            (Some(a), Some(b)) => {
                                use gamelogic::common::Relationship;
                                matches!(
                                    self.player_relationship(a, b),
                                    Relationship::Enemies | Relationship::Neutral
                                )
                            }
                            _ => *det_team != s_team,
                        };
                        // C++ PartitionFilterAcceptByKindOf ExtraRequired/ForbiddenKindOf.
                        let kind_ok = crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                            s_kind,
                            flags.extra_required,
                            flags.extra_forbidden,
                        );
                        // C++ StealthDetectorUpdate.cpp:179-180 FROM_CENTER_2D
                        // (host Y-up → XZ; altitude must not shrink radius).
                        let in_range = rel_ok
                            && kind_ok
                            && Object::stealth_detector_distance_2d(*det_pos, s_pos) <= *range;
                        if in_range {
                            spotting_detectors.push(*id);
                            let hold = stealth_detector_hold_frames(*rate);
                            let exp = frame.saturating_add(hold);
                            if exp > best_expires {
                                best_expires = exp;
                            }
                        }
                        found || in_range
                    });

            if detected_by_someone {
                for id in &spotting_detectors {
                    spotted_detectors.insert(*id);
                }
                self.mark_stealth_detected(sid, best_expires.wrapping_sub(frame));
                if let Some(obj) = self.objects.get_mut(&sid) {
                    obj.apply_detected_heat_vision_second_pass();
                }
                // `mark_stealth_detected` performs the idle-enemy wake walk.
                if !already_detected {
                    // C++ StealthDetectorUpdate.cpp:199-260 radar/audio/message.
                    // Hero Enemy*/Own* EVA is inside doFeedback (tryEvent 10s).
                    self.fire_stealth_discover_feedback(sid, &spotting_detectors);
                }
                // C++ StealthDetectorUpdate.cpp:292-308 / leftover: IR grid is
                // outside the !was_detected feedback gate — every scan refresh.
                for det_id in &spotting_detectors {
                    let Some(det_y) = detectors.iter().find(|d| d.0 == *det_id).map(|d| d.3.y)
                    else {
                        continue;
                    };
                    self.spawn_detector_ir_grid_at(s_pos, det_y, *det_id, effects);
                }
            }
        }


        // C++ PartitionFilterStealthedOrStealthGarrisoned + occupant
        // markAsDetected(updateRate+2) (StealthDetectorUpdate.cpp:105-117, :311-333).
        {
            use crate::game_logic::host_radar_stealth_vision_residual::spotter_mark_detected_garrison_rider_frames_residual;
            let buildings: Vec<ObjectId> = self
                .objects
                .registered()
                .filter(|(_, o)| o.is_alive() && o.is_garrison_contain())
                .map(|(id, _)| *id)
                .collect();
            for bid in buildings {
                let Some((b_pos, occupants, b_kind)) = self
                    .objects
                    .get(&bid)
                    .map(|b| (b.get_position(), b.contained_units(), b.kind_of_cpp_mask()))
                else {
                    continue;
                };
                let stealth_riders: Vec<ObjectId> = occupants
                    .into_iter()
                    .filter(|id| {
                        self.objects
                            .get(id)
                            .is_some_and(|o| o.is_alive() && o.status.stealthed)
                    })
                    .collect();
                if stealth_riders.is_empty() {
                    continue;
                }
                for (_id, det_team, det_owner, det_pos, range, flags, rate) in &detectors {
                    if Object::stealth_detector_distance_2d(*det_pos, b_pos) > *range {
                        continue;
                    }
                    // C++ applies PartitionFilterAcceptByKindOf to the container `them`.
                    if !crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        b_kind,
                        flags.extra_required,
                        flags.extra_forbidden,
                    ) {
                        continue;
                    }

                    let hold = spotter_mark_detected_garrison_rider_frames_residual(*rate);
                    for rid in &stealth_riders {
                        let Some((r_team, r_owner)) =
                            self.objects.get(rid).map(|o| (o.team, o.owner_player_id))
                        else {
                            continue;
                        };
                        let rel_ok = match (*det_owner, r_owner) {
                            (Some(a), Some(b)) => {
                                use gamelogic::common::Relationship;
                                matches!(
                                    self.player_relationship(a, b),
                                    Relationship::Enemies | Relationship::Neutral
                                )
                            }
                            _ => *det_team != r_team,
                        };
                        if !rel_ok {
                            continue;
                        }
                        self.mark_stealth_detected(*rid, hold);
                    }
                }
            }
        }
        let scanned: Vec<ObjectId> = detectors.iter().map(|d| d.0).collect();
        self.play_detector_ir_scan_fx(&scanned, &spotted_detectors, effects);
        self.play_stealth_transition_sounds(&stealth_snap);
    }

    /// C++ StealthUpdate.cpp:725-780 SoundStealthOn / SoundStealthOff.
    pub(super) fn play_stealth_transition_sounds(&mut self, before: &[(ObjectId, bool, bool)]) {
        use crate::game_logic::object::{SOUND_STEALTH_OFF, SOUND_STEALTH_ON};
        let local = self.local_player_id();
        let mut events: Vec<(ObjectId, Vec3, &'static str)> = Vec::new();
        for &(id, was_stealthed, was_detected) in before {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            let pos = obj.get_position();
            let local_ok =
                crate::game_logic::source_is_locally_controlled(obj.owner_player_id, local);
            if obj.status.stealthed != was_stealthed {
                // Cloak and destalth both play StealthOn in retail.
                events.push((id, pos, SOUND_STEALTH_ON));
            }
            if obj.status.detected && !was_detected {
                events.push((id, pos, SOUND_STEALTH_OFF));
            } else if !obj.status.detected && was_detected && local_ok {
                events.push((id, pos, SOUND_STEALTH_ON));
            }
        }
        for (id, pos, name) in events {
            self.queue_audio_event(
                AudioEventRequest::new(name)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(140),
            );
        }
    }

    /// C++ StealthDetectorUpdate.cpp:338-394 IR ping / beacon after DetectionRate.
    pub(super) fn play_detector_ir_scan_fx(
        &mut self,
        detector_ids: &[ObjectId],
        spotted: &std::collections::HashSet<ObjectId>,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::combat_particles::CombatParticleKind;
        use crate::game_logic::host_radar_stealth_vision_residual::{
            DETECTOR_IR_BEACON_PARTICLE, DETECTOR_IR_BRIGHT_PARTICLE, DETECTOR_IR_LOUD_PING_SOUND,
            DETECTOR_IR_PING_PARTICLE, DETECTOR_IR_PING_SOUND,
        };
        let frame = self.frame;
        let mut events: Vec<(ObjectId, Vec3, bool)> = Vec::new();
        for id in detector_ids {
            if !self.detector_ir_fx_visible_to_local(*id) {
                continue;
            }
            let Some(pos) = self.objects.get(id).map(|o| o.get_position()) else {
                continue;
            };
            events.push((*id, pos, spotted.contains(id)));
        }
        for (id, pos, found) in events {
            let sound = if found {
                DETECTOR_IR_LOUD_PING_SOUND
            } else {
                DETECTOR_IR_PING_SOUND
            };
            self.queue_audio_event(
                AudioEventRequest::new(sound)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(150),
            );
            let ping = if found {
                DETECTOR_IR_BRIGHT_PARTICLE
            } else {
                DETECTOR_IR_PING_PARTICLE
            };
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::ParticleSysBone,
                ping,
                pos,
                frame,
                Some(id),
                None,
                particle_owner,
            );
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::ParticleSysBone,
                DETECTOR_IR_BEACON_PARTICLE,
                pos,
                frame,
                Some(id),
                None,
                particle_owner,
            );
        }
    }

    /// C++ StealthDetectorUpdate.cpp:341-343: shroud-clear and not stealthed-to-local.
    pub(super) fn detector_ir_fx_visible_to_local(&self, det_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&det_id) else {
            return false;
        };
        if obj.status.stealthed && !self.is_object_locally_controlled(det_id) {
            return false;
        }
        // C++ Object::getShroudedStatus (Object.cpp:1778-1788): ALWAYS_VISIBLE
        // or no PartitionData (garrisoned) → CLEAR.
        if obj.get_template().always_visible || obj.contained_by.is_some() {
            return true;
        }
        let Some(local) = self.local_player_id() else {
            return true;
        };
        let status = self
            .world_services
            .shroud()
            .lock()
            .ok()
            .and_then(|mgr| mgr.get_host_object_shroud_status(local, det_id.0));
        match status {
            Some(shroud) => {
                (shroud as u8) <= (gamelogic::common::types::ObjectShroudStatus::PartialClear as u8)
            }
            // C++ Object.cpp:1786-1788: no PartitionData → CLEAR.
            None => true,
        }
    }

    /// Leftover StealthDetectorUpdate::clear_grid_particles at each scan wake.
    pub(super) fn clear_detector_ir_grids_for(
        &mut self,
        detector_ids: &[ObjectId],
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::host_radar_stealth_vision_residual::DETECTOR_IR_GRID_PARTICLE;
        let stale: Vec<u32> = self
            .combat_particles
            .active_systems()
            .filter(|e| {
                e.template_name == DETECTOR_IR_GRID_PARTICLE
                    && e.source_object
                        .is_some_and(|src| detector_ids.contains(&src))
            })
            .map(|e| e.id)
            .collect();
        for id in stale {
            self.combat_particles.deactivate(id, particle_owner);
        }
    }

    /// C++ StealthDetectorUpdate.cpp:292-308 IR grid each DetectionRate scan.
    /// Host Y-up: snap XZ %12, height = detector Y + 17 (C++ detector z + 17).
    pub(super) fn spawn_detector_ir_grid_at(
        &mut self,
        pos: Vec3,
        detector_y: f32,
        detector_id: ObjectId,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::combat_particles::CombatParticleKind;
        use crate::game_logic::host_radar_stealth_vision_residual::DETECTOR_IR_GRID_PARTICLE;
        let grid = Vec3::new(
            pos.x - pos.x.rem_euclid(12.0),
            detector_y + 17.0,
            pos.z - pos.z.rem_euclid(12.0),
        );
        let _ = self.combat_particles.spawn_named(
            CombatParticleKind::ParticleSysBone,
            DETECTOR_IR_GRID_PARTICLE,
            grid,
            self.frame,
            Some(detector_id),
            None,
            particle_owner,
        );
    }

    /// C++ StealthUpdate.cpp:748-751 `hintDetectableWhileUnstealthed` and
    /// :807-811 local-player `setStealthLook`.
    pub(super) fn apply_local_stealth_look_and_heat_vision(&mut self) {
        use crate::game_logic::host_upgrades::calc_stealthed_status_for_player;
        let local = self.local_player_id();
        let local_inactive = local
            .and_then(|id| self.players.get(&id))
            .map(|p| !p.is_alive)
            .unwrap_or(true);
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            let friendly = local_inactive
                || self.is_object_locally_controlled(id)
                || match (obj.owner_player_id, local) {
                    (Some(a), Some(b)) => {
                        self.player_relationship(a, b) == gamelogic::common::Relationship::Allies
                    }
                    _ => false,
                };
            let local_owner = self.is_object_locally_controlled(id);
            let module = obj.stealth_update_data().copied();
            let hint = module.is_some_and(|data| {
                !obj.status.stealthed
                    && obj.stealth_update_runs(&data)
                    && local_owner
                    && ((data.hint_detectable.is_firing_weapon && obj.status.is_firing_weapon)
                        || (data.hint_detectable.is_using_ability && obj.status.using_ability))
            });
            let can_disguise = module.is_some_and(|data| data.disguises_as_team);
            // C++ isDisguised() is `m_disguiseAsTemplate != NULL` (set on
            // disguiseAsObject, before halfpoint). Host keeps the template in
            // pending until halfpoint, then `status.disguised`.
            let is_disguised = obj.status.disguised
                || obj.disguise_as_template.is_some()
                || obj.disguise_pending_template.is_some();
            let look = calc_stealthed_status_for_player(
                obj.status.stealthed,
                obj.status.detected,
                friendly,
                can_disguise,
                is_disguised,
            );
            if let Some(obj) = self.objects.get_mut(&id) {
                if hint {
                    // StealthUpdate.cpp:748 precedes changed setStealthLook:810.
                    obj.queue_second_material_pass(1.0);
                }
                let look_u8 = look.as_u8();
                let look_changed = obj.camo_stealth_look != look_u8;
                if look_changed {
                    obj.queue_changed_stealth_material(look_u8, is_disguised);
                    obj.camo_stealth_look = look_u8;
                    obj.update_drawable_hidden_status();
                }
            }
            self.admit_object_drawable_deselection(id);
        }
        self.drain_hidden_drawable_selection();
        self.drain_masked_object_selection();
    }

    /// C++ StealthDetectorUpdate.cpp:199-260 first-detect radar/audio/UI.
    pub(crate) fn fire_stealth_discover_feedback(
        &mut self,
        target_id: ObjectId,
        detector_ids: &[ObjectId],
    ) {
        use crate::game_logic::host_radar_stealth_vision_residual::{
            SPOTTER_AUDIO_STEALTH_DISCOVERED, SPOTTER_AUDIO_STEALTH_NEUTRALIZED,
            SPOTTER_MESSAGE_STEALTH_DISCOVERED, SPOTTER_MESSAGE_STEALTH_NEUTRALIZED,
        };
        use game_engine::common::system::radar::{Coord3D, RadarEventType};

        let Some(target) = self.objects.get(&target_id) else {
            return;
        };
        let t_team = target.team;
        let t_owner = target.owner_player_id;
        let pos = target.get_position();
        let is_mine = target.is_kind_of(KindOf::Mine)
            || target.is_kind_of(KindOf::DemoTrap)
            || target.mine_data.is_some()
            || target.booby_trap_special;
        let loc = Coord3D {
            x: pos.x,
            y: pos.z,
            z: pos.y,
        };

        // C++ StealthDetectorUpdate.cpp:202-203 / :244-245:
        // local player == detector/victim controller, relationship != ALLIES.
        let local_detector = detector_ids.iter().any(|id| {
            if !self.is_object_locally_controlled(*id) {
                return false;
            }
            let Some(d) = self.objects.get(id) else {
                return false;
            };
            match (d.owner_player_id, t_owner) {
                (Some(a), Some(b)) => {
                    self.player_relationship(a, b) != gamelogic::common::Relationship::Allies
                }
                _ => d.team != t_team,
            }
        });
        let local_victim = self.is_object_locally_controlled(target_id);
        let enemy_detector = detector_ids.iter().any(|id| {
            let Some(d) = self.objects.get(id) else {
                return false;
            };
            match (d.owner_player_id, t_owner) {
                (Some(a), Some(b)) => {
                    self.player_relationship(a, b) != gamelogic::common::Relationship::Allies
                }
                _ => d.team != t_team,
            }
        });

        if local_detector {
            let do_feedback = self
                .radar_mut()
                .try_event(RadarEventType::StealthDiscovered, &loc);
            if do_feedback {
                let msg = crate::localization::localize(
                    SPOTTER_MESSAGE_STEALTH_DISCOVERED,
                    "Stealth discovered",
                );
                self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Generic);
                self.queue_audio_event(
                    AudioEventRequest::new(SPOTTER_AUDIO_STEALTH_DISCOVERED)
                        .with_object(target_id)
                        .with_position(pos)
                        .with_priority(160),
                );
                // C++ :233-237 EnemyDetectionEvaEvent inside doFeedback.
                self.try_eva_hero_detected_kind(target_id, false);
            }
        }

        if local_victim && enemy_detector {
            let do_feedback = {
                let radar = self.radar_mut();
                if is_mine {
                    radar.try_event(RadarEventType::StealthNeutralized, &loc)
                } else {
                    radar.create_event(
                        &loc,
                        RadarEventType::StealthNeutralized,
                        crate::game_logic::host_radar_stealth_vision_residual::RADAR_EVENT_DEFAULT_SECONDS_TO_LIVE_RESIDUAL,
                    );
                    true
                }
            };
            if do_feedback {
                let msg = crate::localization::localize(
                    SPOTTER_MESSAGE_STEALTH_NEUTRALIZED,
                    "Stealth neutralized",
                );
                self.queue_radar_message_at(msg, pos, radar_notifications::RadarKind::Attack);
                self.queue_audio_event(
                    AudioEventRequest::new(SPOTTER_AUDIO_STEALTH_NEUTRALIZED)
                        .with_object(target_id)
                        .with_position(pos)
                        .with_priority(160),
                );
                // C++ :269-274 OwnDetectionEvaEvent inside doFeedback.
                self.try_eva_hero_detected_kind(target_id, true);
            }
        }
    }
}
