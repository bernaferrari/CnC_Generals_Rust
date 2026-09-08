//! Live-host playback for SoundMoveStart/Loop and SoundAmbient.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use crate::game_logic::host_move_ambient_audio::{
    TemplateMoveAmbientSlot, drain_ambient_restarts, drain_move_loop_stops, move_uses_damaged,
    resolve_ambient_event, resolve_for_object,
};
use std::collections::HashMap;

/// C++ `Drawable::update` re-adds a culled permanent SoundAmbient, but retail
/// `AudioSettings.ini` `TimeBetweenDrawableSounds`
/// (`AudioSettings::drawable_ambient_frames`) throttles drawable sound
/// retries. Fallback when the setting is absent/zero: 30 logic frames = 1 s
/// at the 30 Hz logic rate.
const DEFAULT_DRAWABLE_AMBIENT_RETRY_FRAMES: u32 = 30;

/// Per-object ambient restart pacing (host session lifetime). `was_playing`
/// keeps C++ `Drawable::update` semantics — a loop that was playing and got
/// culled restarts immediately; a restart attempt that never produced sound
/// (mute game) backs off instead of re-queueing every frame. The logic tick
/// is single-threaded; `thread_local!` matches the ambient restart log in
/// `host_move_ambient_audio` and keeps parallel test sessions isolated.
#[derive(Debug, Default, Clone)]
struct AmbientRestartPacing {
    /// Ambient event name this pacing state was recorded for (damage-state
    /// switches change the name; a different sound starts with fresh
    /// pacing).
    name: String,
    last_attempt_frame: Option<u32>,
    was_playing: bool,
}

thread_local! {
    static AMBIENT_RESTART_PACING: std::cell::RefCell<HashMap<u32, AmbientRestartPacing>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Live `drawable_ambient_frames` from THE_AUDIO AudioSettings (retail
/// `TimeBetweenDrawableSounds`), defaulting to 30 frames.
fn drawable_ambient_retry_frames() -> u32 {
    game_engine::common::audio::game_audio::get_global_audio_manager()
        .and_then(|manager| {
            // Nest so the guard dies inside the closure (E0515 otherwise).
            manager
                .lock()
                .ok()
                .map(|guard| guard.get_audio_settings().drawable_ambient_frames)
        })
        .filter(|frames| *frames > 0)
        .unwrap_or(DEFAULT_DRAWABLE_AMBIENT_RETRY_FRAMES)
}

#[cfg(test)]
fn reset_ambient_restart_pacing_for_tests() {
    AMBIENT_RESTART_PACING.with(|pacing| pacing.borrow_mut().clear());
}

fn leftover_ambient_is_playing(object_id: ObjectId, name: &str) -> bool {
    let Some(manager) = game_engine::common::audio::game_audio::get_global_audio_manager() else {
        return false;
    };
    manager
        .lock()
        .ok()
        .map(|guard| guard.is_named_event_playing_for_object(object_id.0, name))
        .unwrap_or(false)
}

fn leftover_ambient_is_permanent(name: &str) -> bool {
    // C++ Drawable::update: `eventInfo == NULL || isPermanentSound()`.
    let Some(manager) = game_engine::common::audio::game_audio::get_global_audio_manager() else {
        return true;
    };
    manager
        .lock()
        .ok()
        .and_then(|guard| guard.find_audio_event_info(name))
        .map(|info| info.is_permanent_sound())
        .unwrap_or(true)
}

impl GameLogic {
    pub(crate) fn drain_pending_move_ambient_audio(&mut self) {
        for ev in drain_move_loop_stops() {
            self.queue_audio_event(
                AudioEventRequest::new(&ev.event_name)
                    .with_object(ev.object)
                    .with_position(ev.position)
                    .stopping(),
            );
        }
        let restarts = drain_ambient_restarts();
        for id in restarts {
            self.start_ambient_sound(id);
        }
        self.restart_ambient_sounds_if_dropped();
    }

    /// C++ `Drawable::update` (`Drawable.cpp:1290-1314`): permanent (loop-forever)
    /// SoundAmbient is re-added when Miles / leftover TheAudio culls it out of range.
    fn restart_ambient_sounds_if_dropped(&mut self) {
        if !crate::assets::audio::leftover_the_audio_is_live() {
            return;
        }
        let candidates: Vec<(ObjectId, String, glam::Vec3)> = self
            .objects
            .iter()
            .filter(|(_, unit)| unit.ambient_sound_enabled_from_script)
            .filter_map(|(id, unit)| {
                let name = unit.ambient_audio.as_ref()?;
                if name.is_empty() {
                    return None;
                }
                Some((*id, name.clone(), unit.get_position()))
            })
            .collect();
        AMBIENT_RESTART_PACING.with(|pacing| {
            // Session pacing tracks live candidates only; destroyed objects
            // drop out of the map.
            pacing
                .borrow_mut()
                .retain(|id, _| candidates.iter().any(|(candidate, _, _)| candidate.0 == *id));
        });
        let now = crate::game_logic::host_historic_bonus::logic_frame();
        let retry_frames = drawable_ambient_retry_frames();
        for (id, name, pos) in candidates {
            if self.queued_audio_events.iter().any(|event| {
                event.object_id == Some(id)
                    && event.event_type == name
                    && event.is_looping
                    && !event.stop
            }) {
                continue;
            }
            // The RefMut cannot escape `thread_local!::with`, so the pacing
            // decision runs inside and only the verdict comes out.
            let should_restart = AMBIENT_RESTART_PACING.with(|p| {
                let mut pacing = p.borrow_mut();
                let state = pacing.entry(id.0).or_default();
                if state.name != name {
                    // Damage-state / script switch to a different ambient
                    // event: fresh pacing (was_playing and backoff belong to
                    // the old sound).
                    *state = AmbientRestartPacing {
                        name: name.clone(),
                        ..Default::default()
                    };
                }
                if leftover_ambient_is_playing(id, &name) {
                    state.was_playing = true;
                    return false;
                }
                if !leftover_ambient_is_permanent(&name) {
                    return false;
                }
                // C++ `Drawable::update` restarts a previously-playing
                // culled loop immediately; a restart attempt that never
                // produced sound backs off to the drawable-ambient cadence
                // (`TimeBetweenDrawableSounds`) instead of re-queueing every
                // frame while the play fails.
                let backoff_elapsed = match state.last_attempt_frame {
                    Some(last) => now.saturating_sub(last) >= retry_frames,
                    None => true,
                };
                if !(state.was_playing || backoff_elapsed) {
                    return false;
                }
                state.was_playing = false;
                state.last_attempt_frame = Some(now);
                true
            });
            if should_restart {
                self.queue_audio_event(
                    AudioEventRequest::new(&name)
                        .with_object(id)
                        .with_position(pos)
                        .with_priority(80)
                        .looping(),
                );
            }
        }
    }

    /// C++ `AIInternalMoveToState::startMoveSound`.
    pub(crate) fn start_move_sound(&mut self, id: ObjectId) {
        self.drain_pending_move_ambient_audio();
        let Some(unit) = self.objects.get(&id) else {
            return;
        };
        if !unit.is_alive() {
            return;
        }
        let use_damaged = move_uses_damaged(unit.body_damage_state);
        let start_slot = if use_damaged {
            TemplateMoveAmbientSlot::SoundMoveStartDamaged
        } else {
            TemplateMoveAmbientSlot::SoundMoveStart
        };
        let loop_slot = if use_damaged {
            TemplateMoveAmbientSlot::SoundMoveLoopDamaged
        } else {
            TemplateMoveAmbientSlot::SoundMoveLoop
        };
        let start_name = resolve_for_object(&unit.thing.template, start_slot);
        let loop_name = resolve_for_object(&unit.thing.template, loop_slot);
        let pos = unit.get_position();
        if let Some(name) = start_name {
            self.stop_move_loop_sound(id);
            self.queue_audio_event(
                AudioEventRequest::new(&name)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(140),
            );
            return;
        }
        let Some(name) = loop_name else {
            self.stop_move_loop_sound(id);
            return;
        };
        if self
            .objects
            .get(&id)
            .and_then(|unit| unit.move_loop_audio.as_deref())
            == Some(name.as_str())
        {
            return;
        }
        self.stop_move_loop_sound(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.move_loop_audio = Some(name.clone());
        }
        self.queue_audio_event(
            AudioEventRequest::new(&name)
                .with_object(id)
                .with_position(pos)
                .with_priority(120)
                .looping(),
        );
    }

    pub(crate) fn stop_move_loop_sound(&mut self, id: ObjectId) {
        let Some(unit) = self.objects.get_mut(&id) else {
            return;
        };
        let Some(name) = unit.move_loop_audio.take() else {
            return;
        };
        let pos = unit.get_position();
        self.queue_audio_event(
            AudioEventRequest::new(&name)
                .with_object(id)
                .with_position(pos)
                .stopping(),
        );
    }

    /// C++ `Drawable::startAmbientSound` (enabled, current body state).
    pub(crate) fn start_ambient_sound(&mut self, id: ObjectId) {
        let Some(unit) = self.objects.get(&id) else {
            return;
        };
        // C++ startAmbientSound returns if !m_ambientSoundEnabledFromScript.
        if !unit.ambient_sound_enabled_from_script {
            return;
        }
        let state = unit.body_damage_state;
        let name = resolve_ambient_event(&unit.thing.template, state);
        let pos = unit.get_position();
        let current = unit.ambient_audio.clone();
        if current.as_deref() == name.as_deref() {
            return;
        }
        if let Some(old) = current {
            self.queue_audio_event(
                AudioEventRequest::new(&old)
                    .with_object(id)
                    .with_position(pos)
                    .stopping(),
            );
        }
        let Some(name) = name else {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.ambient_audio = None;
            }
            return;
        };
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.ambient_audio = Some(name.clone());
        }
        self.queue_audio_event(
            AudioEventRequest::new(&name)
                .with_object(id)
                .with_position(pos)
                .with_priority(80)
                .looping(),
        );
    }

    pub(crate) fn stop_ambient_sound(&mut self, id: ObjectId) {
        let Some(unit) = self.objects.get_mut(&id) else {
            return;
        };
        let Some(name) = unit.ambient_audio.take() else {
            return;
        };
        let pos = unit.get_position();
        self.queue_audio_event(
            AudioEventRequest::new(&name)
                .with_object(id)
                .with_position(pos)
                .stopping(),
        );
    }

    /// C++ `ScriptActions::doSoundPlayFromNamed` — one TheAudio add, no second enqueue.
    pub fn play_sound_from_named(&mut self, sound: &str, unit_name: &str) -> bool {
        if sound.is_empty() || unit_name.is_empty() {
            return false;
        }
        let Some(id) = self
            .host_object_id_by_script_name(unit_name)
            .or_else(|| self.host_named_unit_id(unit_name))
            .or_else(|| self.find_object_id_by_name(unit_name))
        else {
            return false;
        };
        if let Some(audio) = gamelogic::helpers::TheAudio::get() {
            let mut event = gamelogic::common::audio::AudioEventRts::new(sound);
            event.set_object_id(id.0);
            event.set_is_logical_audio(true);
            let _ = audio.add_audio_event(&event);
        }
        true
    }

    /// C++ `Drawable::enableAmbientSoundFromScript`.
    pub fn enable_object_sound_from_script(&mut self, unit_name: &str, enable: bool) -> bool {
        let Some(id) = self
            .host_object_id_by_script_name(unit_name)
            .or_else(|| self.host_named_unit_id(unit_name))
            .or_else(|| self.find_object_id_by_name(unit_name))
        else {
            return false;
        };
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.ambient_sound_enabled_from_script = enable;
        }
        if enable {
            // C++ skips the already-enabled check so one-shots retrigger.
            self.stop_ambient_sound(id);
            self.start_ambient_sound(id);
        } else {
            self.stop_ambient_sound(id);
        }
        true
    }

    /// Drain leftover SOUND_PLAY_NAMED / ENABLE/DISABLE_OBJECT_SOUND.
    pub(crate) fn apply_host_object_sound_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptObjectSoundRequest;
        for req in gamelogic::scripting::take_host_script_object_sound_requests() {
            match req {
                HostScriptObjectSoundRequest::PlayNamed { sound, unit } => {
                    // Leftover / handler already hit TheAudio (C++ addAudioEvent).
                    // Do not queue a second presentation copy.
                    let _ = (sound, unit);
                }

                HostScriptObjectSoundRequest::Enable { unit, enable } => {
                    let _ = self.enable_object_sound_from_script(&unit, enable);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::host_move_ambient_audio::clear_test_template_move_ambient;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    fn tank_logic() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaTankCrusader");
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Selectable);
        tmpl.sound_move_start = Some("CrusaderMoveStart".into());
        tmpl.sound_move_loop = Some("CrusaderMoveLoop".into());
        tmpl.sound_ambient = Some("CrusaderAmbientLoop".into());
        tmpl.sound_ambient_damaged = Some("CrusaderAmbientDamaged".into());
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), tmpl);
        let id = logic
            .create_object("AmericaTankCrusader", Team::USA, Vec3::ZERO)
            .expect("tank");
        logic.queued_audio_events.clear();
        (logic, id)
    }

    #[test]
    fn move_start_plays_template_sound_not_invented_name() {
        let (mut logic, id) = tank_logic();
        assert!(logic.unit_command_move_free(
            id,
            Vec3::new(80.0, 0.0, 0.0),
            Vec3::new(80.0, 0.0, 0.0)
        ));
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "CrusaderMoveStart" && e.object_id == Some(id) && !e.is_looping
            }),
            "SoundMoveStart must queue on march: {:?}",
            logic.queued_audio_events
        );
        assert!(
            logic.queued_audio_events.iter().all(|e| {
                e.event_type != "AmericaTankCrusaderMoveStart" && e.event_type != "UnitMove"
            }),
            "must not invent concatenated move SFX"
        );
    }

    #[test]
    fn move_loop_starts_when_start_is_empty_and_stops_on_idle() {
        clear_test_template_move_ambient();
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("GLAVehicleTechnical");
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.sound_move_loop = Some("TechnicalMoveLoop".into());
        logic
            .templates
            .insert("GLAVehicleTechnical".to_string(), tmpl);
        let id = logic
            .create_object("GLAVehicleTechnical", Team::GLA, Vec3::ZERO)
            .expect("tech");
        logic.queued_audio_events.clear();
        assert!(logic.unit_command_move_free(
            id,
            Vec3::new(40.0, 0.0, 0.0),
            Vec3::new(40.0, 0.0, 0.0)
        ));
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| { e.event_type == "TechnicalMoveLoop" && e.is_looping && !e.stop }),
            "empty SoundMoveStart falls through to SoundMoveLoop: {:?}",
            logic.queued_audio_events
        );
        assert_eq!(
            logic
                .objects
                .get(&id)
                .and_then(|o| o.move_loop_audio.as_deref()),
            Some("TechnicalMoveLoop")
        );
        if let Some(unit) = logic.objects.get_mut(&id) {
            unit.stop_moving();
        }
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == "TechnicalMoveLoop" && e.stop),
            "onExit removes the move loop: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn create_starts_template_sound_ambient() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.sound_ambient = Some("WarFactoryAmbientLoop".into());
        logic
            .templates
            .insert("AmericaWarFactory".to_string(), tmpl);
        let id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("factory");
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "WarFactoryAmbientLoop"
                    && e.object_id == Some(id)
                    && e.is_looping
                    && !e.stop
            }),
            "create must start SoundAmbient: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn body_damage_restarts_damaged_ambient() {
        let (mut logic, id) = tank_logic();
        logic.queued_audio_events.clear();
        if let Some(unit) = logic.objects.get_mut(&id) {
            unit.apply_body_damage_state_change_residual(
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            );
        }
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| { e.event_type == "CrusaderAmbientDamaged" && e.is_looping && !e.stop }),
            "Damaged body must start SoundAmbientDamaged: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn sound_play_named_hits_the_audio_once() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaInfantryRanger");
        tmpl.add_kind_of(KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".to_string(), tmpl);
        let id = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(20.0, 0.0, 30.0),
            )
            .expect("ranger");
        if let Some(obj) = logic.objects.get_mut(&id) {
            obj.name = "NamedScout".into();
        }
        logic.queued_audio_events.clear();
        assert!(logic.play_sound_from_named("UnitCheer", "NamedScout"));
        assert!(
            logic.queued_audio_events.is_empty(),
            "C++ doSoundPlayFromNamed is one TheAudio add, no presentation queue: {:?}",
            logic.queued_audio_events
        );
        assert!(!logic.play_sound_from_named("UnitCheer", "MissingUnit"));
    }

    #[test]
    fn enable_disable_object_sound_from_script() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.sound_ambient = Some("WarFactoryAmbientLoop".into());
        logic
            .templates
            .insert("AmericaWarFactory".to_string(), tmpl);
        let id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("factory");
        if let Some(obj) = logic.objects.get_mut(&id) {
            obj.name = "NamedFactory".into();
        }
        assert_eq!(
            logic
                .objects
                .get(&id)
                .and_then(|o| o.ambient_audio.as_deref()),
            Some("WarFactoryAmbientLoop")
        );
        logic.queued_audio_events.clear();
        assert!(logic.enable_object_sound_from_script("NamedFactory", false));
        assert!(
            !logic
                .objects
                .get(&id)
                .expect("factory")
                .ambient_sound_enabled_from_script
        );
        assert!(
            logic
                .objects
                .get(&id)
                .and_then(|o| o.ambient_audio.as_deref())
                .is_none()
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == "WarFactoryAmbientLoop" && e.stop),
            "DISABLE_OBJECT_SOUND must stop ambient: {:?}",
            logic.queued_audio_events
        );

        logic.queued_audio_events.clear();
        if let Some(unit) = logic.objects.get_mut(&id) {
            unit.apply_body_damage_state_change_residual(
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            );
        }
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|e| !e.is_looping || e.stop),
            "disabled-from-script ambient must not restart on body damage: {:?}",
            logic.queued_audio_events
        );

        logic.queued_audio_events.clear();
        assert!(logic.enable_object_sound_from_script("NamedFactory", true));
        assert!(
            logic
                .objects
                .get(&id)
                .expect("factory")
                .ambient_sound_enabled_from_script
        );
        assert!(
            logic
                .queued_audio_events
                .iter()
                .any(|e| { e.event_type == "WarFactoryAmbientLoop" && e.is_looping && !e.stop }),
            "ENABLE_OBJECT_SOUND must restart ambient: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn apply_host_play_named_does_not_queue_second_copy() {
        let _ = gamelogic::scripting::take_host_script_object_sound_requests();
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaInfantryRanger");
        tmpl.add_kind_of(KindOf::Infantry);
        logic
            .templates
            .insert("AmericaInfantryRanger".to_string(), tmpl);
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("ranger");
        if let Some(obj) = logic.objects.get_mut(&id) {
            obj.name = "NamedScout".into();
        }
        logic.queued_audio_events.clear();
        gamelogic::scripting::request_host_script_object_sound(
            gamelogic::scripting::HostScriptObjectSoundRequest::PlayNamed {
                sound: "UnitCheer".into(),
                unit: "NamedScout".into(),
            },
        );
        logic.apply_host_object_sound_script_requests();
        assert!(
            logic.queued_audio_events.is_empty(),
            "leftover SOUND_PLAY_NAMED must not enqueue a second live copy: {:?}",
            logic.queued_audio_events
        );
    }

    fn register_ambient_event(name: &str, permanent: bool) {
        let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        if let Ok(mut guard) = manager.lock() {
            guard.register_audio_event_info(game_engine::common::audio::AudioEventInfo {
                sound_type: game_engine::common::audio::AudioType::SoundEffect,
                control: if permanent {
                    game_engine::common::audio::AC_LOOP
                } else {
                    0
                },
                audio_name: name.to_string(),
                volume: 0.8,
                sounds_morning: Vec::new(),
                sounds: Vec::new(),
                sounds_night: Vec::new(),
                sounds_evening: Vec::new(),
                attack_sounds: Vec::new(),
                decay_sounds: Vec::new(),
                pitch_shift_min: 1.0,
                pitch_shift_max: 1.0,
                volume_shift: 0.0,
                min_volume: 0.0,
                limit: 0,
                loop_count: if permanent { 0 } else { 1 },
                delay_min: 0.0,
                delay_max: 0.0,
                filename: String::new(),
                sound_type_field: game_engine::common::audio::AudioType::SoundEffect,
                type_field: 0,
                priority: game_engine::common::audio::AudioPriority::Normal,
                min_distance: 25.0,
                max_distance: 1000.0,
                ..Default::default()
            });
        }
    }

    #[test]
    fn permanent_ambient_restarts_after_the_audio_cull() {
        // C++ Drawable::update restarts loop-forever SoundAmbient when
        // Miles processPlayingList drops the out-of-range 3D loop.
        register_ambient_event("WarFactoryAmbientLoop", true);
        super::reset_ambient_restart_pacing_for_tests();
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.sound_ambient = Some("WarFactoryAmbientLoop".into());
        logic
            .templates
            .insert("AmericaWarFactory".to_string(), tmpl);
        let id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("factory");
        logic.queued_audio_events.clear();
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "WarFactoryAmbientLoop"
                    && e.object_id == Some(id)
                    && e.is_looping
                    && !e.stop
            }),
            "culled permanent ambient must re-queue: {:?}",
            logic.queued_audio_events
        );

        logic.queued_audio_events.clear();
        {
            let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
            let mut guard = manager.lock().expect("THE_AUDIO lock");
            guard.test_insert_active_named_event(id.0, "WarFactoryAmbientLoop");
        }
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|e| { !(e.event_type == "WarFactoryAmbientLoop" && e.is_looping && !e.stop) }),
            "playing permanent ambient must not re-queue: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn failing_ambient_restart_retries_at_drawable_cadence_not_per_frame() {
        // A permanent loop whose restart attempt never produces sound (mute
        // game) must not re-queue every frame: retries pace at
        // drawable_ambient_frames (TimeBetweenDrawableSounds; default 30
        // frames), so a failing loop costs one queue per second, not one
        // per frame.
        register_ambient_event("WarFactoryAmbientLoop", true);
        super::reset_ambient_restart_pacing_for_tests();
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.sound_ambient = Some("WarFactoryAmbientLoop".into());
        logic
            .templates
            .insert("AmericaWarFactory".to_string(), tmpl);
        let id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("factory");
        logic.queued_audio_events.clear();

        let set_frame = crate::game_logic::host_historic_bonus::set_logic_frame;
        fn restarts_queued(logic: &GameLogic, id: ObjectId) -> bool {
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "WarFactoryAmbientLoop"
                    && e.object_id == Some(id)
                    && e.is_looping
                    && !e.stop
            })
        }

        set_frame(100);
        logic.drain_pending_move_ambient_audio();
        assert!(
            restarts_queued(&logic, id),
            "first attempt must queue: {:?}",
            logic.queued_audio_events
        );

        // Frames 101..=129: within the 30-frame backoff the failed restart
        // (never observed playing) must not re-queue.
        for frame in 101..=129 {
            logic.queued_audio_events.clear();
            set_frame(frame);
            logic.drain_pending_move_ambient_audio();
            assert!(
                !restarts_queued(&logic, id),
                "frame {frame}: failed restart must back off, not re-queue"
            );
        }

        set_frame(130);
        logic.drain_pending_move_ambient_audio();
        assert!(
            restarts_queued(&logic, id),
            "30 frames after the failed attempt the retry is due"
        );
    }

    #[test]
    fn previously_playing_ambient_restarts_immediately_after_cull() {
        // C++ Drawable::update restarts a loop that was playing and got
        // culled right away — the backoff only paces retries of restarts
        // that never produced sound.
        register_ambient_event("WarFactoryAmbientLoop", true);
        super::reset_ambient_restart_pacing_for_tests();
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.sound_ambient = Some("WarFactoryAmbientLoop".into());
        logic
            .templates
            .insert("AmericaWarFactory".to_string(), tmpl);
        let id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("factory");
        logic.queued_audio_events.clear();

        let set_frame = crate::game_logic::host_historic_bonus::set_logic_frame;
        set_frame(200);
        logic.drain_pending_move_ambient_audio(); // initial attempt at 200
        logic.queued_audio_events.clear();

        {
            let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
            let mut guard = manager.lock().expect("THE_AUDIO lock");
            guard.test_insert_active_named_event(id.0, "WarFactoryAmbientLoop");
        }
        logic.drain_pending_move_ambient_audio(); // playing → was_playing
        logic.queued_audio_events.clear();

        {
            // Miles processPlayingList culls the loop out of range.
            let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
            manager
                .lock()
                .expect("THE_AUDIO lock")
                .remove_audio_event(1001);
        }
        set_frame(201); // inside the 30-frame backoff window
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "WarFactoryAmbientLoop"
                    && e.object_id == Some(id)
                    && e.is_looping
                    && !e.stop
            }),
            "previously-playing culled ambient must restart immediately: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn one_shot_ambient_does_not_restart_after_cull() {
        register_ambient_event("FactoryOneShotAmbient", false);
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.sound_ambient = Some("FactoryOneShotAmbient".into());
        logic
            .templates
            .insert("AmericaWarFactory".to_string(), tmpl);
        let id = logic
            .create_object("AmericaWarFactory", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("factory");
        assert_eq!(
            logic
                .objects
                .get(&id)
                .and_then(|o| o.ambient_audio.as_deref()),
            Some("FactoryOneShotAmbient")
        );
        logic.queued_audio_events.clear();
        logic.drain_pending_move_ambient_audio();
        assert!(
            logic
                .queued_audio_events
                .iter()
                .all(|e| { !(e.event_type == "FactoryOneShotAmbient" && e.is_looping && !e.stop) }),
            "non-permanent ambient must not restart after cull: {:?}",
            logic.queued_audio_events
        );
    }
}
