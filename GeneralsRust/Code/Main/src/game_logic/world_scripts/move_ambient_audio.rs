//! Live-host playback for SoundMoveStart/Loop and SoundAmbient.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use crate::game_logic::host_move_ambient_audio::{
    drain_ambient_restarts, drain_move_loop_stops, move_uses_damaged, resolve_ambient_event,
    resolve_for_object, TemplateMoveAmbientSlot,
};

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

    /// C++ `ScriptActions::doSoundPlayFromNamed` on the live host object.
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
        let pos = self.objects.get(&id).map(|unit| unit.get_position());
        if let Some(audio) = gamelogic::helpers::TheAudio::get() {
            let mut event = gamelogic::common::audio::AudioEventRts::new(sound);
            event.set_object_id(id.0);
            event.set_is_logical_audio(true);
            let _ = audio.add_audio_event(&event);
        }
        let mut req = AudioEventRequest::new(sound).with_object(id);
        if let Some(pos) = pos {
            req = req.with_position(pos);
        }
        self.queue_audio_event(req);
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
                    // Handler / leftover already hit TheAudio (C++ addAudioEvent).
                    // Queue presentation so live process_audio_events hears it.
                    let Some(id) = self
                        .host_object_id_by_script_name(&unit)
                        .or_else(|| self.host_named_unit_id(&unit))
                        .or_else(|| self.find_object_id_by_name(&unit))
                    else {
                        continue;
                    };
                    let pos = self.objects.get(&id).map(|o| o.get_position());
                    let mut req = AudioEventRequest::new(&sound).with_object(id);
                    if let Some(pos) = pos {
                        req = req.with_position(pos);
                    }
                    self.queue_audio_event(req);
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
    use super::super::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::host_move_ambient_audio::clear_test_template_move_ambient;
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
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "TechnicalMoveLoop" && e.is_looping && !e.stop
            }),
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
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "CrusaderAmbientDamaged" && e.is_looping && !e.stop
            }),
            "Damaged body must start SoundAmbientDamaged: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn sound_play_named_queues_from_live_object() {
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
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "UnitCheer" && e.object_id == Some(id) && !e.stop
            }),
            "SOUND_PLAY_NAMED must queue from the live named unit: {:?}",
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
        assert!(logic
            .objects
            .get(&id)
            .and_then(|o| o.ambient_audio.as_deref())
            .is_none());
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
            logic.queued_audio_events.iter().all(|e| !e.is_looping || e.stop),
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
            logic.queued_audio_events.iter().any(|e| {
                e.event_type == "WarFactoryAmbientLoop" && e.is_looping && !e.stop
            }),
            "ENABLE_OBJECT_SOUND must restart ambient: {:?}",
            logic.queued_audio_events
        );
    }

    #[test]
    fn apply_host_object_sound_requests_reaches_live_objects() {
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
            logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == "UnitCheer" && e.object_id == Some(id)),
            "leftover SOUND_PLAY_NAMED queue must reach live host: {:?}",
            logic.queued_audio_events
        );
    }

}
