//! Synchronous presentation effects attached to the receiving native match.
use super::{HostScriptExecutionDriver, ObjectId};
use game_engine::common::audio::{AudioEventRts, audio_event_rts::Coord3D};
use game_engine::common::system::radar::RadarEventType;
use generals_sim::scripting::camera_actions::ScriptCameraWorld;
use generals_sim::scripting::interpreter::ScriptInterpreter;
use generals_sim::scripting::map_actions::ScriptMapWorld;
use generals_sim::scripting::map_reveals::{NamedReveal, NamedRevealOperation, WaypointVisibility};
use generals_sim::scripting::media_actions::ScriptMediaWorld;
use generals_sim::scripting::radar::RadarEventTarget;
use generals_sim::scripting::world::{
    HostScriptObjectSoundRequest, ScriptAudioRequest, ScriptCameraRequest, ScriptDisplayRequest,
    ScriptMapVisibilityOperation, ScriptWorldRequest,
};
use generals_sim::{GameLogicError, GameLogicResult};
use glam::Vec3;

impl HostScriptExecutionDriver<'_> {
    pub(super) fn play_script_speech(
        &mut self,
        name: &str,
        allow_overlap: bool,
    ) -> GameLogicResult<()> {
        let mut event = game_engine::common::audio::AudioEventRts::with_event_name(name);
        event.set_is_logical_audio(true);
        event.set_uninterruptable(!allow_overlap);
        let player = self
            .world
            .local_player_id()
            .map(|id| id as i32)
            .unwrap_or(-1);
        event.set_player_index(player);
        let result = self.admit_owned_script_audio(&event, player);
        if result.is_ok() {
            self.world.mission_scripts.admit_speech_caption(name);
        }
        result
    }

    pub(super) fn apply_script_audio(
        &mut self,
        request: ScriptAudioRequest<'_>,
    ) -> GameLogicResult<()> {
        use ScriptAudioRequest as Request;
        let valid = match request {
            Request::SetVolume { volume, .. } | Request::NamedVolume { volume, .. } => {
                volume.is_finite()
            }
            _ => true,
        };
        if !valid {
            return Err(GameLogicError::Configuration(
                "nonfinite script audio policy".into(),
            ));
        }
        match request {
            Request::SetVolume { volume, affect } => {
                self.world.queued_audio_events.set_volume(volume, affect)
            }
            Request::NamedVolume { name, volume } => self
                .world
                .queued_audio_events
                .set_named_volume(name, volume),
            Request::Pause(_) => self.world.queued_audio_events.pause_pending(),
            _ => {}
        }
        let extra_handles = self.owned_audio_extra_receipts();
        if let Some(audio) = self.ui.audio.as_deref_mut() {
            audio
                .script_audio(
                    request,
                    self.world.queued_audio_events.policy(),
                    &extra_handles,
                )
                .map_err(GameLogicError::Configuration)
        } else {
            // Explicit non-presentation/headless execution owns policy but no playing receipts.
            Ok(())
        }
    }

    pub(super) fn apply_script_object_sound(
        &mut self,
        request: &HostScriptObjectSoundRequest,
        this_object: Option<u32>,
    ) -> GameLogicResult<()> {
        use HostScriptObjectSoundRequest as Sound;

        match request {
            Sound::PlayNamed { sound, unit } => {
                if let Some(id) = self
                    .world
                    .owned_named_script_object(unit, this_object.map(ObjectId))
                {
                    // The native audio service is a real external boundary;
                    // identity is selected only from the driving simulation.
                    let mut event =
                        game_engine::common::audio::AudioEventRts::with_object_id(sound, id.0);
                    event.set_is_logical_audio(true);
                    let object = self.world.host_object(id).ok_or_else(|| {
                        GameLogicError::Configuration(
                            "named audio object left its receiving world during admission".into(),
                        )
                    })?;
                    let position = object.get_position();
                    event.set_receiving_object_spatial_owner(
                        id.0,
                        &object.visual_draw_identity_token,
                        game_engine::common::audio::audio_event_rts::Coord3D {
                            x: position.x,
                            y: position.z,
                            z: position.y,
                        },
                    );
                    let player = object.owner_player_id.map(|p| p as i32).unwrap_or(-1);
                    event.set_player_index(player);
                    self.admit_owned_script_audio(&event, player)?;
                }
            }
            Sound::Enable { unit, enable } => {
                if let Some(id) = self
                    .world
                    .owned_named_script_object(unit, this_object.map(ObjectId))
                {
                    self.world
                        .enable_owned_object_sound_with_effects(id, *enable, &mut self.ui)
                        .map_err(GameLogicError::Configuration)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn apply_script_camera(
        &mut self,
        request: ScriptCameraRequest<'_>,
    ) -> GameLogicResult<()> {
        match request {
            ScriptCameraRequest::ScreenShake { intensity } => {
                self.world.admit_script_screen_shake(intensity);
                Ok(())
            }
            _ => self.world.mission_scripts.apply_camera_request(request),
        }
    }

    pub(super) fn apply_script_display(
        &mut self,
        request: ScriptDisplayRequest<'_>,
    ) -> GameLogicResult<()> {
        // Queue on this world, at the same drain boundary as the legacy
        // callback. Do not use an engine's retained presentation owner.
        match request {
            ScriptDisplayRequest::Text(text) => {
                self.world.mission_scripts.push_message(text.into())
            }
            ScriptDisplayRequest::Cinematic {
                text,
                font,
                duration_seconds,
            } => {
                self.world.mission_scripts.push_cinematic_text(
                    text.into(),
                    font.into(),
                    duration_seconds,
                );
            }
            ScriptDisplayRequest::MilitaryCaption { text, duration_ms } => {
                self.world
                    .mission_scripts
                    .push_military_caption(text.into(), duration_ms);
            }
        }
        Ok(())
    }

    pub(super) fn apply_script_map_visibility(
        &mut self,
        operation: ScriptMapVisibilityOperation,
        player_name: &str,
        current_player: Option<&str>,
    ) -> GameLogicResult<()> {
        let player_id = if player_name.is_empty() {
            None
        } else {
            self.script_player_id(player_name, current_player)
        };
        // Core's find_player_by_name branch returns after a match; a failed
        // non-empty lookup falls through to the same all-human behavior as an
        // omitted parameter. script_player_id also handles LocalPlayer,
        // ThisPlayer, ThisPlayerEnemy, and the challenge-only ThePlayer alias.
        let mut player_ids: Vec<u32> = match player_id {
            Some(player_id) => vec![player_id],
            None => self
                .world
                .players
                .values()
                .filter(|player| player.is_human)
                .map(|player| player.id)
                .collect(),
        };
        player_ids.sort_unstable();
        let Ok(mut shroud) = self.world.world_services.shroud().lock() else {
            return Err(GameLogicError::Threading(
                "Failed to lock receiving ShroudManager".to_string(),
            ));
        };
        for player_id in player_ids {
            shroud.apply_whole_map_visibility(operation, player_id);
        }
        Ok(())
    }

    pub(super) fn apply_script_radar_visibility(&mut self, enabled: bool) -> GameLogicResult<()> {
        self.world.radar_enabled = enabled;
        self.world.mission_scripts.push_radar_enabled(enabled);
        Ok(())
    }
}

impl ScriptCameraWorld for HostScriptExecutionDriver<'_> {
    fn waypoint_position(&mut self, name: &str) -> GameLogicResult<Option<Vec3>> {
        self.receiving_map_waypoint(name)
    }

    fn named_object_id(&mut self, name: &str, interpreter: &ScriptInterpreter) -> Option<u32> {
        self.world
            .owned_named_script_object(name, interpreter.script_object_id().map(ObjectId))
            .map(|object| object.0)
    }

    fn camera(&mut self, request: ScriptCameraRequest<'_>) -> GameLogicResult<()> {
        self.apply_script_camera(request)
    }

    fn oversize_terrain(&mut self, amount: i32) -> GameLogicResult<()> {
        self.world.mission_scripts.push_oversize_terrain(amount);
        Ok(())
    }
}

impl ScriptMapWorld for HostScriptExecutionDriver<'_> {
    fn radar_event(
        &mut self,
        target: RadarEventTarget<'_>,
        kind: RadarEventType,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()> {
        self.apply_radar_event(
            target,
            kind,
            interpreter.script_object_id(),
            interpreter.calling_team(),
            interpreter.condition_team(),
        )
    }

    fn radar_visibility(&mut self, enabled: bool) -> GameLogicResult<()> {
        self.apply_script_radar_visibility(enabled)
    }

    fn world(&mut self, request: ScriptWorldRequest<'_>) -> GameLogicResult<()> {
        match request {
            ScriptWorldRequest::MapSwitchBoundary(boundary) => self
                .world
                .switch_owned_map_boundary(boundary, &mut self.ui)
                .map_err(GameLogicError::Configuration),
            ScriptWorldRequest::RadarRefreshTerrain => {
                self.world.host_radar_refresh_terrain();
                Ok(())
            }
            ScriptWorldRequest::RadarForced(forced) => {
                self.world.set_radar_forced(forced);
                self.world.mission_scripts.push_radar_forced(forced);
                Ok(())
            }
            _ => Err(GameLogicError::Configuration(
                "unexpected map script request".into(),
            )),
        }
    }

    fn map_visibility_all(
        &mut self,
        operation: ScriptMapVisibilityOperation,
        player: &str,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()> {
        self.apply_script_map_visibility(operation, player, interpreter.current_player().as_deref())
    }

    fn waypoint_visibility(
        &mut self,
        operation: WaypointVisibility,
        waypoint: &str,
        radius: f32,
        player: &str,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()> {
        self.apply_receiving_waypoint_visibility(
            operation,
            waypoint,
            radius,
            player,
            interpreter.current_player().as_deref(),
        )
    }

    fn named_map_visibility(
        &mut self,
        operation: NamedRevealOperation,
        reveal: &NamedReveal,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()> {
        self.apply_receiving_named_reveal(
            operation,
            reveal,
            interpreter.current_player().as_deref(),
        )
    }

    fn border_shroud(&mut self, enabled: bool) -> GameLogicResult<()> {
        let data = self
            .world
            .world_services
            .common()
            .system
            .global_data
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let level = data.map_or(if enabled { 0 } else { 255 }, |data| {
            let data = data.read();
            if enabled {
                data.shroud_alpha
            } else {
                data.clear_alpha
            }
        });
        self.world.mission_scripts.push_border_shroud_level(level);
        Ok(())
    }
}

impl ScriptMediaWorld for HostScriptExecutionDriver<'_> {
    fn display(&mut self, request: ScriptDisplayRequest<'_>) -> GameLogicResult<()> {
        self.apply_script_display(request)
    }

    fn military_caption_disabled(&self) -> bool {
        self.world
            .world_services
            .common()
            .system
            .runtime_global_data
            .read()
            .writable
            .disable_military_caption
    }

    fn waypoint_position(&mut self, name: &str) -> GameLogicResult<Option<Vec3>> {
        self.receiving_map_waypoint(name)
    }

    fn sound_effect(&mut self, name: &str, position: Option<Vec3>) -> GameLogicResult<()> {
        // ScriptActions.cpp:329–353 submits logical audio for the local side.
        let mut event = AudioEventRts::with_event_name(name);
        if let Some(Vec3 { x, y, z }) = position {
            event.set_position(&Coord3D { x, y, z });
        }
        event.set_is_logical_audio(true);
        let player = self.world.local_player_id().map_or(-1, |id| id as i32);
        event.set_player_index(player);
        self.admit_owned_script_audio(&event, player)
    }

    fn object_sound(
        &mut self,
        request: HostScriptObjectSoundRequest,
        interpreter: &ScriptInterpreter,
    ) -> GameLogicResult<()> {
        self.apply_script_object_sound(&request, interpreter.script_object_id())
    }

    fn speech(&mut self, name: &str, allow_overlap: bool) -> GameLogicResult<()> {
        self.play_script_speech(name, allow_overlap)
    }

    fn music(&mut self, track: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()> {
        self.apply_script_audio(ScriptAudioRequest::StopMusic { fade: fade_out })?;
        let mut event = AudioEventRts::with_event_name(track);
        event.set_should_fade(fade_in);
        let player = self.world.local_player_id().map_or(-1, |id| id as i32);
        event.set_player_index(player);
        self.admit_owned_script_audio(&event, player)
    }

    fn movie(&mut self, name: &str, fullscreen: bool) -> GameLogicResult<()> {
        let hooks = &self.world.mission_scripts;
        if fullscreen {
            hooks.push_movie_request(name.to_owned());
        } else {
            hooks.push_radar_movie_request(name.to_owned());
        }
        Ok(())
    }

    fn audio(&mut self, request: ScriptAudioRequest<'_>) -> GameLogicResult<()> {
        self.apply_script_audio(request)
    }
}
