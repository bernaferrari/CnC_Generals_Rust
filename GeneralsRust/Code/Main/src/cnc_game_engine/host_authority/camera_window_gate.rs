//! Opt-in receiving Engine/window acceptance. This runs on the existing Winit
//! event-loop thread; every saved/loaded value belongs to the real Engine.
//! Explicit frame/multiplier inputs make bounded camera phases repeatable.
//! This is not a playthrough, GPU-frame-rate benchmark or second-Engine proof.
use super::*;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn file_observation(path: &Path) -> Result<Value, String> {
    let mut file =
        std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut crc = crc32fast::Hasher::new();
    let mut block = [0u8; 65536];
    loop {
        let count = file.read(&mut block).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        crc.update(&block[..count]);
    }
    Ok(
        json!({"path":path,"bytes":file.metadata().map_err(|error|error.to_string())?.len(),
        "crc32":format!("{:08x}",crc.finalize()),
        "hash_scope":"runtime CRC; parent launch receipt independently pins SHA256"}),
    )
}
#[derive(serde::Deserialize)]
struct CameraGateRequest {
    nonce: String,
    pid: u32,
    report_path: PathBuf,
    actual_executable: Value,
    map: Value,
    build_receipt: Value,
}
impl CameraGateRequest {
    fn read(map: &Path) -> Result<(Self, Value), String> {
        let path = std::env::var_os("GENERALS_CAMERA_GATE_REQUEST")
            .ok_or("missing explicit fresh camera gate request")?;
        let raw: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let request: Self = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
        require(
            request.nonce.len() == 64 && request.nonce.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid per-launch request nonce",
        )?;
        require(
            request.pid == std::process::id(),
            "request belongs to another process",
        )?;
        require(
            request.report_path.is_absolute() && !request.report_path.exists(),
            "report must be explicit absolute and previously absent",
        )?;
        let actual_executable = std::env::current_exe().map_err(|e| e.to_string())?;
        require(
            file_observation(&actual_executable)? == request.actual_executable,
            "current executable differs from fresh launch request",
        )?;
        require(
            file_observation(map)? == request.map,
            "current map differs from fresh launch request",
        )?;
        require(
            request.build_receipt["source_pin"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "request must link actual parent build receipt source pin",
        )?;
        // The parent pins receipt/executable SHA256 externally. This runtime
        // check cannot establish compilation provenance from a baked string.
        Ok((request, raw))
    }
}
fn logic_camera(logic: &crate::game_logic::GameLogic) -> Result<Value, String> {
    // This existing value is a detached observation, never a mutable camera.
    let camera = crate::presentation_frame::camera::continuation::SessionCameraState::capture(
        &logic.session_view.camera,
    );
    Ok(
        json!({"camera":serde_json::to_value(camera).map_err(|error|error.to_string())?,
        "target":logic.session_view.target.to_array().map(f32::to_bits),
        "impulse":{"intensity":logic.session_view.impulse.intensity.to_bits(),
            "cosine":logic.session_view.impulse.cosine.to_bits(),
            "sine":logic.session_view.impulse.sine.to_bits(),
            "offset":logic.session_view.impulse.offset.to_array().map(f32::to_bits)},
        "view_matrix":logic.session_view.camera.view_matrix.to_cols_array().map(f32::to_bits),
        "projection_matrix":logic.session_view.camera.projection_matrix.to_cols_array().map(f32::to_bits)}),
    )
}
fn retained_view(value: &Value) -> Value {
    json!({"impulse":value["impulse"],"height":value["camera"]["native_camera"],
        "view_matrix":value["view_matrix"],"projection_matrix":value["projection_matrix"]})
}
fn require(condition: bool, reason: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(reason.into())
    }
}
impl CnCGameEngine {
    fn camera_gate_stage_peer(&mut self, slot: &str) -> Result<StagedRestoreWorld, String> {
        let mode = self.game_logic.game_mode();
        let catalog = self.game_logic.templates.clone();
        Self::stage_saved_world_for_restore(
            &mut self.save_file_manager,
            slot,
            mode,
            &catalog,
            &self.game_logic.weapon_definition_base,
            &self.game_logic.projectile_definitions,
            &self.game_logic.ocl_application_definitions(),
            self.game_logic.ai_definitions.baseline(),
            self.game_logic.production_energy_baseline(),
            self.game_logic.physics_rules_baseline(),
            self.game_logic.session_view.camera.settings,
            self.game_logic.session_view.camera.clip_policy,
            self.game_logic.session_view.camera.shake_policy,
            self.game_logic.session_view.camera.movement_policy,
            self.game_logic.session_view.camera.input_policy,
            &self.game_logic.world_services,
        )
    }
    fn camera_gate_seed_hag(&mut self) -> Result<Value, String> {
        require(
            !self.look_at_modes.look_at_host_is_scrolling(),
            "gate requires stationary receiving LookAt mode",
        )?;
        require(
            !self.presentation_or_boot_in_replay_game(),
            "gate requires non-replay match",
        )?;
        require(
            self.game_logic.camera_follow_object_id().is_none()
                && self
                    .game_logic
                    .session_view
                    .camera
                    .camera_slave_mode
                    .is_none(),
            "HAG-only acceptance requires no existing follow/slave camera",
        )?;
        let before = logic_camera(&self.game_logic)?;
        require(
            before["camera"]["native_camera"]["legacy"]["enabled"] == true,
            "actual loaded camera must have HAG adjustment enabled",
        )?;
        // Real wheel admission cancels source scripted motion and uses the
        // receiving settings limits. Try both directions if already at a cap.
        for detents in [3.0, -6.0] {
            let (_, result) = self.with_native_camera_input_effects(|world, effects, mut input| {
                input.delivered_wheel(
                    &winit::event::MouseScrollDelta::LineDelta(0., detents),
                    world,
                    effects,
                )
            });
            result?;
            let terrain = self
                .native_camera_terrain_height_around(self.game_logic.session_view.target)
                .ok_or("real receiving terrain unavailable for active HAG")?;
            let camera = &self.game_logic.session_view.camera;
            let desired = camera
                .native_camera
                .default_zoom(terrain, camera.native_camera.desired());
            let adjustment =
                (camera.camera_zoom - desired) * camera.settings.height_rules().adjust_speed;
            if adjustment.is_finite() && adjustment.abs() >= 0.0001 {
                let seeded = logic_camera(&self.game_logic)?;
                require(
                    !self.game_logic.session_view.scripted.motion.active
                        && seeded["camera"]["camera_yaw_target"].is_null()
                        && seeded["camera"]["camera_pitch_target"].is_null()
                        && seeded["camera"]["camera_zoom_target"].is_null(),
                    "wheel did not cancel scalar requests before HAG-only phase",
                )?;
                return Ok(json!({"before":before,"seeded":seeded,"terrain":terrain,
                    "expected_first_adjustment_bits":adjustment.to_bits(),
                    "ingress":"actual receiving CameraInputLoan.delivered_wheel; no synthetic HAG state"}));
            }
        }
        Err("actual receiving wheel/rules cannot establish a nontrivial HAG continuation".into())
    }
    fn camera_gate_offer(&mut self, frame: u32, multiplier: i32) -> Result<Value, String> {
        // Actual source input owners, not a handwritten DisplayAdmission.
        self.game_logic.set_current_frame(u64::from(frame));
        self.game_client.set_frame(frame);
        self.game_logic.apply_visual_speed_multiplier(
            &crate::game_logic::mission_scripts::VisualSpeedMultiplierRequest { multiplier },
        );
        self.host_finalize_presentation_after_logic();
        let before_intake = self.game_logic.session_view.impulse.intensity.to_bits();
        self.apply_pending_script_camera_requests();
        require(
            self.game_logic.session_view.impulse.intensity.to_bits() == before_intake,
            "canonical shake was replayed by actual frozen-request intake",
        )?;
        let before = logic_camera(&self.game_logic)?;
        let prior_intensity = self.game_logic.session_view.impulse.intensity;
        let terrain = self
            .native_camera_terrain_height_around(self.game_logic.session_view.target)
            .ok_or("actual receiving terrain unavailable before Display")?;
        let camera = &self.game_logic.session_view.camera;
        let prior_zoom = camera.camera_zoom;
        let desired_zoom = camera
            .native_camera
            .default_zoom(terrain, camera.native_camera.desired());
        let adjustment = (prior_zoom - desired_zoom) * camera.settings.height_rules().adjust_speed;
        let expected_zoom = if adjustment.abs() >= 0.0001 {
            prior_zoom - adjustment
        } else {
            prior_zoom
        };

        self.complete_native_display_step()?;
        let after = logic_camera(&self.game_logic)?;
        let display = serde_json::to_value(self.native_display.capture_continuation(frame)?)
            .map_err(|error| error.to_string())?;
        let decision = &display["completed"];
        require(
            decision.is_object(),
            "actual display offer did not complete an envelope",
        )?;
        let full = decision["update_views"]
            .as_bool()
            .ok_or("missing actual update_views")?;
        let fast = decision["fast_camera_only"]
            .as_bool()
            .ok_or("missing actual fast_camera_only")?;
        if full {
            require(
                self.game_logic.session_view.camera.camera_zoom.to_bits()
                    == expected_zoom.to_bits(),
                "full Display did not apply source HAG easing at the receiving terrain/goal",
            )?;
            let expected = if prior_intensity > 0.01 {
                prior_intensity * 0.75
            } else {
                0.
            };
            require(
                self.game_logic.session_view.impulse.intensity.to_bits() == expected.to_bits(),
                "full display did not apply exactly one source impulse spring step",
            )?;
        } else {
            require(
                retained_view(&before) == retained_view(&after),
                "fast/rejected display advanced impulse/HAG/committed matrices",
            )?;
        }
        Ok(
            json!({"frame":frame,"multiplier":multiplier,"display":display,"owner":after,
            "full":full,"fast":fast,"hag_evolved":prior_zoom.to_bits()!=self.game_logic.session_view.camera.camera_zoom.to_bits()}),
        )
    }
    fn camera_gate_trace(&mut self, base: u32) -> Result<Vec<Value>, String> {
        let mut trace = Vec::new();
        let full = self.camera_gate_offer(base.wrapping_add(1), 1)?;
        require(
            full["full"] == true && full["fast"] == false,
            "normal actual envelope was not full",
        )?;
        require(
            full["hag_evolved"] == true,
            "saved HAG continuation was trivial or failed to advance on actual full Display",
        )?;
        trace.push(full);
        let fast = self.camera_gate_offer(base.wrapping_add(2), 30)?;
        require(
            fast["fast"] == true && fast["full"] == false,
            "unfrozen new-frame fast envelope was not admitted",
        )?;
        trace.push(fast);
        // Existing saved counter can legitimately start above one. Drive its
        // actual bounded countdown, never replace or reset the clock.
        let mut frame = base.wrapping_add(3);
        let mut admitted = false;
        for _ in 0..16 {
            let warm = self.camera_gate_offer(frame, 3)?;
            admitted = warm["full"] == true;
            trace.push(warm);
            frame = frame.wrapping_add(1);
            if admitted {
                break;
            }
        }
        require(
            admitted,
            "actual multiplier clock did not reach full admission within bounded gate",
        )?;
        let rejected = self.camera_gate_offer(frame, 3)?;
        require(
            rejected["full"] == false && rejected["fast"] == false,
            "actual multiplier envelope did not reject the next view",
        )?;
        trace.push(rejected);
        Ok(trace)
    }
    /// Real authored movement, native full-view/HAG and archive restore on the
    /// existing receiving Engine. No synthetic pose or replacement owner.
    fn camera_gate_scripted_origin(&mut self, saved_slot: &str) -> Result<Value, String> {
        use gamelogic::scripting::core::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
            ScriptActionType, ScriptList,
        };
        self.host_load_game_authority(saved_slot)
            .map_err(HostLoadFailure::into_message)?;
        let mut action = ScriptAction::new(ScriptActionType::MoveCameraTo);
        action
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::common::Coord3D::new(0., 0., 0.),
            ))
            .map_err(|error| format!("origin coordinate: {error:?}"))?;
        for value in [0., 0., 0., 0.] {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, value))
                .map_err(|error| format!("origin move timing: {error:?}"))?;
        }
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        let mut script = Script::new();
        script.script_name = "ReceivingOriginFinalizerRegression".into();
        script.is_one_shot = true;
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut interpreter =
            gamelogic::scripting::engine::ScriptEngine::new().map_err(|error| error.to_string())?;
        interpreter
            .set_script_list_for_player(0, Some(Box::new(list)))
            .map_err(|error| error.to_string())?;
        self.game_logic
            .update_local_script_engine_with_owner(&interpreter)
            .map_err(|error| error.to_string())?;
        require(
            self.game_logic.session_view.scripted.motion.active,
            "authored MoveCameraTo did not install receiving origin motion",
        )?;
        let frame = self.game_logic.frame.wrapping_add(1);
        self.game_logic.set_current_frame(u64::from(frame));
        self.game_client.set_frame(frame);
        self.game_logic.apply_visual_speed_multiplier(
            &crate::game_logic::mission_scripts::VisualSpeedMultiplierRequest { multiplier: 1 },
        );
        self.host_finalize_presentation_after_logic();
        self.apply_pending_script_camera_requests();
        self.complete_native_display_step()?;
        let display = serde_json::to_value(self.native_display.capture_continuation(frame)?)
            .map_err(|error| error.to_string())?;
        require(
            display["completed"]["update_views"] == true,
            "origin regression requires an actual full native Display",
        )?;
        let target = self.game_logic.session_view.target;
        require(
            target.x == 0. && target.z == 0.,
            "actual full Display replaced authored near-origin camera with base focus",
        )?;
        require(
            !self.game_logic.session_view.scripted.motion.active,
            "actual full Display did not finish zero-duration authored movement",
        )?;
        let slot = "camera_owner_scripted_origin";
        require(
            !self.save_file_manager.get_save_path(slot).exists(),
            "origin continuation archive must be fresh",
        )?;
        let info = self.build_save_info(
            slot,
            "Origin camera acceptance",
            "Real authored motion followed by native HAG and restore",
            SaveFileType::Normal,
        );
        self.host_save_game_authority(slot, &info)?;
        let saved = file_observation(&self.save_file_manager.get_save_path(slot))?;
        let owner = logic_camera(&self.game_logic)?;
        self.host_load_game_authority(slot)
            .map_err(HostLoadFailure::into_message)?;
        require(
            logic_camera(&self.game_logic)? == owner,
            "actual origin archive restore changed receiving camera owner",
        )?;
        let resumed = self.camera_gate_offer(frame.wrapping_add(1), 1)?;
        require(
            resumed["full"] == true,
            "origin continuation requires an actual resumed full Display",
        )?;
        let target = self.game_logic.session_view.target;
        require(
            target.x == 0. && target.z == 0.,
            "actual full Display replaced restored near-origin camera with base focus",
        )?;
        Ok(json!({"authored_display":display,"saved_owner":owner,
            "saved_archive":saved,"resumed_display":resumed,
            "scope":"actual local authored ScriptEngine MoveCameraTo, receiving full native Display/HAG, same Engine save/load and full continuation"}))
    }
    pub(in crate::cnc_game_engine) fn run_camera_window_continuation_gate(
        &mut self,
    ) -> Result<PathBuf, String> {
        require(
            self.runtime_host_active,
            "camera gate requires explicit RuntimeHostBridge opt-in",
        )?;
        require(
            self.current_state == GameState::InGame && !self.native_view_is_paused(),
            "camera gate requires an actual unpaused loaded match",
        )?;
        require(
            !self
                .game_logic
                .simulation_frozen_with_script_engine(None)
                .map_err(|error| error.to_string())?,
            "camera gate requires an unfrozen receiving script/camera owner",
        )?;
        require(
            self.game_logic.peek_pending_screen_shakes().is_empty(),
            "gate requires no preexisting historical queued screen shake",
        )?;
        let map = PathBuf::from(self.game_logic.get_current_map_name());
        require(
            map.is_file(),
            "camera gate requires exact file-backed retail map identity",
        )?;
        let (request, request_value) = CameraGateRequest::read(&map)?;
        let report_path = request.report_path.clone();
        // create_new is an exclusive freshness boundary, including PID reuse.
        let mut report_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&report_path)
            .map_err(|e| format!("fresh exact report: {e}"))?;
        let directory = report_path
            .parent()
            .ok_or("report has no parent")?
            .join("runtime");
        std::fs::create_dir(&directory).map_err(|e| format!("fresh runtime directory: {e}"))?;
        let mut temporary = SaveFileManager::with_save_directory(directory.join("Save"));
        temporary.init().map_err(|error| error.to_string())?;
        let previous_manager = std::mem::replace(&mut self.save_file_manager, temporary);
        // Keep completed evidence even when a later archive/restore phase fails.
        // These are observations from the existing trace, never a second run.
        let mut partial_evidence = json!({});
        let mut phase = "seed_hag";
        let previous_fast_forward = self.replay_fast_forward;
        let previous_foreign_tivo = game_engine::common::global_data::read().tivo_fast_mode;
        // Deliberately disagree with standalone Core. This gate is non-replay:
        // it proves full/fast/rejected HAG admission, not TiVO's replay GPU gate.
        self.replay_fast_forward = true;
        game_engine::common::global_data::write().tivo_fast_mode = false;
        let result = (|| -> Result<Value, String> {
            let metadata = self.receiving_replay_menu_inputs().metadata;
            require(
                metadata == game_client::gui::shell::replay_menu::ReplayMenuMetadataPolicy::capture_application(),
                "native replay metadata was not admitted after content/text loading",
            )?;
            let metadata_evidence = json!({
                "color_count": metadata.limits.color_count,
                "player_template_count": metadata.limits.player_template_count,
                "slot_labels": metadata.slot_labels,
                "map_extension": metadata.map_extension,
                "scope": "receiving Engine snapshot matches completed content/text admission; ordinary input does not poll shared definitions",
            });
            partial_evidence["replay_metadata_admission"] = metadata_evidence.clone();
            let slot = "camera_owner_new";
            let save_path = self.save_file_manager.get_save_path(slot);
            require(!save_path.exists(), "acceptance archive must not preexist")?;
            let hag_seed = self.camera_gate_seed_hag()?;
            partial_evidence["hag_seed"] = hag_seed.clone();
            let frame = self.game_logic.frame;
            // Same canonical source algorithm as the synchronous ScriptAction;
            // dispatcher same-walk ordering is separately covered by CPU tests.
            self.enqueue_script_screen_shake(4);
            require(
                self.game_logic.session_view.impulse.intensity > 0.01,
                "authored receiving rules did not create an active impulse",
            )?;
            self.game_logic.apply_visual_speed_multiplier(
                &crate::game_logic::mission_scripts::VisualSpeedMultiplierRequest { multiplier: 1 },
            );
            self.host_finalize_presentation_after_logic();
            let saved_owner = logic_camera(&self.game_logic)?;
            let info = self.build_save_info(
                slot,
                "Camera owner acceptance",
                "Fresh actual Engine continuation",
                SaveFileType::Normal,
            );
            phase = "save_archive";
            self.host_save_game_authority(slot, &info)?;
            require(
                save_path.is_file(),
                "successful actual save did not create its fresh archive",
            )?;
            let saved_file = file_observation(&save_path)?;
            partial_evidence["saved_archive"] = saved_file.clone();
            phase = "reference_display_trace";
            let reference = self.camera_gate_trace(frame)?;
            partial_evidence["reference_display"] = json!({
                "status":"pass",
                "scope":"existing real Engine/window full/fast/rejected native Display and active wheel-seeded HAG; bounded supplied frame/multiplier inputs",
                "trace":reference.clone(),
                "saved_owner":saved_owner.clone(),
                "map":file_observation(&map)?,
                "engine_replay_fast_forward":self.replay_fast_forward,
                "foreign_core_tivo_fast_mode":game_engine::common::global_data::read().tivo_fast_mode,
                "in_replay_game":self.presentation_or_boot_in_replay_game(),
                "limitations":["Non-replay match: opposite TiVO flags do not exercise replay-specific GPU gating", "No archive restore, peer isolation, live-playthrough or120Hz success follows from this partial observation"]
            });
            phase = "first_restore";
            self.host_load_game_authority(slot)
                .map_err(HostLoadFailure::into_message)?;
            require(
                logic_camera(&self.game_logic)? == saved_owner,
                "actual Engine commit lost saved camera/active impulse",
            )?;
            let before_peer = logic_camera(&self.game_logic)?;
            phase = "stage_peer";
            let mut peer = self.camera_gate_stage_peer(slot)?;
            let shared = self
                .game_logic
                .objects
                .keys()
                .any(|id| peer.logic.objects.contains_key(id));
            require(shared, "real staged peer lacks shared saved Object IDs")?;
            peer.logic.admit_script_screen_shake(1);
            peer.logic.session_view.advance_impulse();
            require(
                peer.logic.combat_particles().can_clear(),
                "staged peer reset lacks its required particle owner",
            )?;
            peer.logic.reset();
            drop(peer);
            require(
                logic_camera(&self.game_logic)? == before_peer,
                "same-thread staged peer mutated receiving camera",
            )?;
            phase = "interleaved_display_trace";
            let interleaved = self.camera_gate_trace(frame)?;
            require(
                interleaved == reference,
                "first camera/display divergence after same-thread staged peer",
            )?;
            phase = "second_restore";
            self.host_load_game_authority(slot)
                .map_err(HostLoadFailure::into_message)?;
            phase = "resumed_display_trace";
            let resumed = self.camera_gate_trace(frame)?;
            require(
                resumed == reference,
                "actual Engine save/load future camera/display trace diverged",
            )?;
            phase = "scripted_origin_finalizer";
            let scripted_origin = self.camera_gate_scripted_origin(slot)?;
            partial_evidence["scripted_origin_finalizer"] = scripted_origin.clone();
            Ok(
                json!({"reference":reference,"after_staged_peer":interleaved,"after_second_restore":resumed,
                "scripted_origin_finalizer":scripted_origin,
                "replay_metadata_admission":metadata_evidence,
                "saved_archive":saved_file,"saved_owner":saved_owner,"hag_seed":hag_seed,"map":file_observation(&map)?,
                "effective_rules":{"settings":self.game_logic.session_view.camera.settings,
                    "clip":self.game_logic.session_view.camera.clip_policy,"shake":self.game_logic.session_view.camera.shake_policy},
                "actual_executable":file_observation(&std::env::current_exe().map_err(|error|error.to_string())?)?,
                "window":format!("{:?}",self.window.id()),"window_size":[self.window.inner_size().width,self.window.inner_size().height]}),
            )
        })();
        self.save_file_manager = previous_manager;
        self.replay_fast_forward = previous_fast_forward;
        game_engine::common::global_data::write().tivo_fast_mode = previous_foreign_tivo;
        let report = match &result {
            Ok(evidence) => {
                json!({"status":"pass","scope":"real Engine/window; bounded source-frame/multiplier injection; staged peer is world isolation, not second Engine","evidence":evidence,
                "request":request_value,"pid":std::process::id(),"nonce":request.nonce,
                "completed_partial_observations":partial_evidence,
                "provenance_scope":"request links parent build receipt and external SHA256 pins; runtime metadata is not intrinsic compiled-source proof",
                "limitations":["No live-playthrough or120Hz claim","No second Engine GPU ownership proof","Canonical shake ingress complements separate authored ScriptAction walk tests"]})
            }
            Err(error) => json!({"status":"fail","first_error":error,"failed_phase":phase,
                "completed_partial_observations":partial_evidence,"report":report_path,
                "provenance_scope":"request links parent build receipt and external SHA256 pins; runtime metadata is not intrinsic compiled-source proof",
                "request":request_value,"pid":std::process::id(),"nonce":request.nonce}),
        };
        report_file
            .write_all(&serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
            .map_err(|e| format!("acceptance report {}: {e}", report_path.display()))?;
        report_file.sync_all().map_err(|e| e.to_string())?;
        result
            .map(|_| report_path.clone())
            .map_err(|error| format!("report={}: {error}", report_path.display()))
    }
}
