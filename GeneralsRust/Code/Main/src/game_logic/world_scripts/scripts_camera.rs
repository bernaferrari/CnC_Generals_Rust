//! Host scripts `impl GameLogic` — `scripts_camera`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! script eval / EVA process / camera path / script camera
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub(in super::super) fn build_script_game_state_context(
        &self,
    ) -> gamelogic::scripting::GameStateContext {
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
    pub(in super::super) fn process_eva_events(&mut self) {
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
    pub(in super::super) fn mission_script_count(&self) -> usize {
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

    /// Read-only host name→ObjectId map for crate script evaluator.
    /// Does **not** populate OBJECT_REGISTRY or wrap crate `Object`s.
    pub fn host_named_unit_id_map(&self) -> std::collections::HashMap<String, u32> {
        let mut map = std::collections::HashMap::new();
        for (id, obj) in self.host_objects() {
            if obj.name.is_empty() {
                continue;
            }
            map.insert(obj.name.clone(), id.0);
        }
        map
    }

    /// Host named-unit query (scripts/AI). Prefer this over empty crate THE_AI groups.
    pub fn host_named_unit_id(&self, name: &str) -> Option<ObjectId> {
        if name.is_empty() {
            return None;
        }
        self.host_objects()
            .iter()
            .find(|(_, o)| o.name == name)
            .map(|(id, _)| *id)
    }

    /// Host team query: live host objects on `team`.
    pub fn host_team_unit_ids(&self, team: crate::game_logic::Team) -> Vec<ObjectId> {
        self.host_objects()
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive() && !o.status.destroyed)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Host area query: live host objects whose XZ is inside `min..=max`.
    pub fn host_area_unit_ids(&self, min: glam::Vec3, max: glam::Vec3) -> Vec<ObjectId> {
        self.host_objects()
            .iter()
            .filter(|(_, o)| {
                if !o.is_alive() || o.status.destroyed {
                    return false;
                }
                let p = o.position;
                p.x >= min.x && p.x <= max.x && p.z >= min.z && p.z <= max.z
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Inject host names into the crate NamedObjectTracker (IDs only).
    /// Crate evaluator can resolve names; it still must not require crate Objects.
    pub fn inject_host_named_unit_map_into_crate_tracker(&self) {
        use gamelogic::scripting::engine::get_named_object_tracker;
        let tracker = get_named_object_tracker();
        for (name, id) in self.host_named_unit_id_map() {
            let _ = tracker.register_named_object(name, id);
        }
        self.inject_host_script_query_snapshot();
    }

    /// Fill crate condition host-query snapshot from `host_named_unit_id*`.
    pub fn inject_host_script_query_snapshot(&self) {
        use gamelogic::scripting::{HostScriptQueryObject, HostScriptQuerySnapshot};
        let mut snap = HostScriptQuerySnapshot::default();
        snap.named = self.host_named_unit_id_map();
        for team in [
            crate::game_logic::Team::USA,
            crate::game_logic::Team::China,
            crate::game_logic::Team::GLA,
            crate::game_logic::Team::Neutral,
        ] {
            let ids = self.host_team_unit_ids(team);
            if !ids.is_empty() {
                snap.team_ids
                    .insert(team as u32, ids.iter().map(|id| id.0).collect());
            }
        }
        for (id, obj) in self.host_objects() {
            snap.objects.push(HostScriptQueryObject {
                id: id.0,
                name: obj.name.clone(),
                team: obj.team as u32,
                x: obj.position.x,
                z: obj.position.z,
                alive: obj.is_alive() && !obj.status.destroyed,
            });
        }
        for (name, aabb) in gamelogic::scripting::engine::get_area_tracker().all_area_aabbs() {
            snap.areas.insert(name, aabb);
        }
        gamelogic::scripting::set_host_script_query_snapshot(snap);
    }

    pub(in super::super) fn evaluate_and_execute_scripts(&mut self, dt: f32) {
        if !self.scripts_loaded {
            return;
        }

        // Host script path: named-unit/team/area queries hit HOST objects.
        // Crate evaluator sees the name→id map + query snapshot (no crate Objects).
        self.inject_host_named_unit_map_into_crate_tracker();

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
        // C++ GameLogic.cpp:3600 — one TheScriptEngine->UPDATE() per logic frame
        // (ScriptEngine.cpp:5479-5574).  Host MissionScriptRuntime is the
        // action-handler drain/queue, not a second walker over the same lists
        // (hq-fxq1).  GAME_SHELL uses this same single pass; no menu budget.
        if let Ok(mut guard) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                if let Err(err) = engine.update() {
                    log::error!("ScriptEngine::update failed: {err}");
                }
            }
        }
        self.mission_scripts.note_logic_frame(self.frame as u64);

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
            // C++ InGameUI owns one popup layout: every new popup replaces the
            // previously visible one.  Keep only the newest presentation
            // residual; MissionScriptHooks itself remains the future-event
            // queue and is already drained above.
            let active_popup = popup_messages.last().cloned();
            #[cfg(feature = "game_client")]
            if let Some(popup) = active_popup.as_ref() {
                // C++ clears/replaces the single InGameUI popup layout. Send
                // only its newest request to GameClient and retain its opaque
                // identity so a delayed ButtonOk/Esc cannot dismiss a later
                // replacement popup in Main.
                game_client::core::script_action_handler::script_popup_message_with_host_generation(
                    &popup.message,
                    popup.x_percent,
                    popup.y_percent,
                    popup.width,
                    popup.pause,
                    popup.pause_music,
                    Some(popup.popup_generation),
                );
            }

            for popup in popup_messages {
                if popup.pause_music {
                    self.pending_music_stop = true;
                }
                self.script_broadcasts.push(ScriptBroadcast {
                    text: popup.message.clone(),
                    expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
                });
                self.new_script_messages.push(popup.message.clone());
            }

            self.pending_popup_messages.clear();
            if let Some(active_popup) = active_popup {
                self.pending_popup_messages.push(active_popup);
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

    pub(in super::super) fn start_camera_path_move(&mut self, request: CameraPathRequest) {
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

    pub(in super::super) fn start_camera_move_to(&mut self, request: CameraMoveToRequest) {
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

    pub(in super::super) fn script_camera_remaining_seconds(&self) -> f32 {
        if let Some(move_to) = self.script_camera_move_to.as_ref() {
            return move_to.remaining_time_seconds();
        }
        if let Some(path) = self.script_camera_path.as_ref() {
            return path.remaining_time_seconds();
        }
        0.0
    }

    pub(in super::super) fn is_script_camera_angle_frozen(&self) -> bool {
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

    pub(in super::super) fn apply_script_camera_mod_freeze_time(&mut self) {
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

    pub(in super::super) fn apply_script_camera_mod_freeze_angle(&mut self) {
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

    pub(in super::super) fn apply_script_camera_mod_final_speed_multiplier(
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

    pub(in super::super) fn apply_script_camera_mod_rolling_average(
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

    pub(in super::super) fn apply_visual_speed_multiplier(
        &mut self,
        request: &VisualSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier.max(1) as f32;
        if multiplier.is_finite() {
            self.visual_speed_multiplier = multiplier;
        }
    }

    pub(in super::super) fn apply_set_fps_limit(&mut self, request: &SetFpsLimitRequest) {
        self.pending_script_fps_limit = Some(request.fps);
    }

    pub(in super::super) fn apply_script_camera_default(
        &mut self,
        request: CameraSetDefaultRequest,
    ) {
        self.script_default_camera_pitch = request.pitch;
        // Match C++ W3DView::setDefaultView(): angle is ignored for the active 3D path.
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = if request.max_height.is_finite() {
            request.max_height.max(0.0)
        } else {
            1.0
        };
    }

    pub(in super::super) fn update_script_camera(&mut self, dt: f32) {
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

    pub(in super::super) fn military_caption_duration_seconds(duration_ms: i32) -> f32 {
        (duration_ms as f32 / 1000.0).max(0.0)
    }
}
