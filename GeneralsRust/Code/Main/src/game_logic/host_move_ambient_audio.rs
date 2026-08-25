//! Resolve ThingTemplate `SoundMoveStart` / `SoundMoveLoop` / `SoundAmbient`.
//!
//! C++ `AIInternalMoveToState::startMoveSound` (`AIStates.cpp:1666-1704`) and
//! `Drawable::startAmbientSound` (`Drawable.cpp:4437-4541`). Playback lives on
//! live-host `GameLogic` (`world_scripts/move_ambient_audio.rs`).

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::{ObjectId, ThingTemplate};
use glam::Vec3;
use std::cell::RefCell;
use std::collections::HashMap;

/// INI / leftover `ThingTemplateAudioType` keys for move and ambient events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateMoveAmbientSlot {
    SoundMoveStart,
    SoundMoveStartDamaged,
    SoundMoveLoop,
    SoundMoveLoopDamaged,
    SoundAmbient,
    SoundAmbientDamaged,
    SoundAmbientReallyDamaged,
    SoundAmbientRubble,
}

impl TemplateMoveAmbientSlot {
    pub fn ini_key(self) -> &'static str {
        match self {
            Self::SoundMoveStart => "SoundMoveStart",
            Self::SoundMoveStartDamaged => "SoundMoveStartDamaged",
            Self::SoundMoveLoop => "SoundMoveLoop",
            Self::SoundMoveLoopDamaged => "SoundMoveLoopDamaged",
            Self::SoundAmbient => "SoundAmbient",
            Self::SoundAmbientDamaged => "SoundAmbientDamaged",
            Self::SoundAmbientReallyDamaged => "SoundAmbientReallyDamaged",
            Self::SoundAmbientRubble => "SoundAmbientRubble",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MoveAmbientStopEvent {
    pub object: ObjectId,
    pub event_name: String,
    pub position: Vec3,
}

thread_local! {
    static TEMPLATE_OVERRIDE: RefCell<HashMap<(String, TemplateMoveAmbientSlot), String>> =
        RefCell::new(HashMap::new());
    static STOP_LOG: RefCell<Vec<MoveAmbientStopEvent>> = RefCell::new(Vec::new());
    static AMBIENT_RESTART_LOG: RefCell<Vec<ObjectId>> = RefCell::new(Vec::new());
}

pub fn nonempty_audio_event_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("NoSound") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn set_test_template_move_ambient(
    template_name: &str,
    slot: TemplateMoveAmbientSlot,
    event_name: impl Into<String>,
) {
    TEMPLATE_OVERRIDE.with(|m| {
        m.borrow_mut()
            .insert((template_name.to_string(), slot), event_name.into());
    });
}

pub fn clear_test_template_move_ambient() {
    TEMPLATE_OVERRIDE.with(|m| m.borrow_mut().clear());
}

pub fn record_move_loop_stop(object: ObjectId, event_name: String, position: Vec3) {
    STOP_LOG.with(|log| {
        log.borrow_mut().push(MoveAmbientStopEvent {
            object,
            event_name,
            position,
        });
    });
}

pub fn drain_move_loop_stops() -> Vec<MoveAmbientStopEvent> {
    STOP_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn record_ambient_restart(object: ObjectId) {
    AMBIENT_RESTART_LOG.with(|log| log.borrow_mut().push(object));
}

pub fn drain_ambient_restarts() -> Vec<ObjectId> {
    AMBIENT_RESTART_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn host_template_move_ambient(
    template: &ThingTemplate,
    slot: TemplateMoveAmbientSlot,
) -> Option<String> {
    let raw = match slot {
        TemplateMoveAmbientSlot::SoundMoveStart => template.sound_move_start.as_deref(),
        TemplateMoveAmbientSlot::SoundMoveStartDamaged => {
            template.sound_move_start_damaged.as_deref()
        }
        TemplateMoveAmbientSlot::SoundMoveLoop => template.sound_move_loop.as_deref(),
        TemplateMoveAmbientSlot::SoundMoveLoopDamaged => {
            template.sound_move_loop_damaged.as_deref()
        }
        TemplateMoveAmbientSlot::SoundAmbient => template.sound_ambient.as_deref(),
        TemplateMoveAmbientSlot::SoundAmbientDamaged => template.sound_ambient_damaged.as_deref(),
        TemplateMoveAmbientSlot::SoundAmbientReallyDamaged => {
            template.sound_ambient_really_damaged.as_deref()
        }
        TemplateMoveAmbientSlot::SoundAmbientRubble => template.sound_ambient_rubble.as_deref(),
    }?;
    nonempty_audio_event_name(raw)
}

fn leftover_factory_move_ambient(
    template_name: &str,
    slot: TemplateMoveAmbientSlot,
) -> Option<String> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let event = match slot {
        TemplateMoveAmbientSlot::SoundMoveStart => tmpl.get_sound_move_start(),
        TemplateMoveAmbientSlot::SoundMoveStartDamaged => tmpl.get_sound_move_start_damaged(),
        TemplateMoveAmbientSlot::SoundMoveLoop => tmpl.get_sound_move_loop(),
        TemplateMoveAmbientSlot::SoundMoveLoopDamaged => tmpl.get_sound_move_loop_damaged(),
        TemplateMoveAmbientSlot::SoundAmbient => tmpl.get_sound_ambient(),
        TemplateMoveAmbientSlot::SoundAmbientDamaged => tmpl.get_sound_ambient_damaged(),
        TemplateMoveAmbientSlot::SoundAmbientReallyDamaged => {
            tmpl.get_sound_ambient_really_damaged()
        }
        TemplateMoveAmbientSlot::SoundAmbientRubble => tmpl.get_sound_ambient_rubble(),
    }?;
    nonempty_audio_event_name(event.get_event_name())
}

fn asset_definition_move_ambient(
    template_name: &str,
    slot: TemplateMoveAmbientSlot,
) -> Option<String> {
    let manager = crate::assets::get_asset_manager()?;
    let guard = manager.lock().ok()?;
    let definition = guard.get_object_definition(template_name)?;
    let raw = definition.attributes.iter().find_map(|(key, value)| {
        key.eq_ignore_ascii_case(slot.ini_key())
            .then(|| value.as_str())
    })?;
    nonempty_audio_event_name(raw)
}

/// Resolve authored move/ambient INI event. Never invents `{template}MoveStart`.
pub fn resolve_template_move_ambient(
    template_name: &str,
    slot: TemplateMoveAmbientSlot,
) -> Option<String> {
    if let Some(over) =
        TEMPLATE_OVERRIDE.with(|m| m.borrow().get(&(template_name.to_string(), slot)).cloned())
    {
        return nonempty_audio_event_name(&over);
    }
    leftover_factory_move_ambient(template_name, slot)
        .or_else(|| asset_definition_move_ambient(template_name, slot))
}

pub fn resolve_for_object(
    template: &ThingTemplate,
    slot: TemplateMoveAmbientSlot,
) -> Option<String> {
    if let Some(over) =
        TEMPLATE_OVERRIDE.with(|m| m.borrow().get(&(template.name.clone(), slot)).cloned())
    {
        return nonempty_audio_event_name(&over);
    }
    host_template_move_ambient(template, slot)
        .or_else(|| leftover_factory_move_ambient(&template.name, slot))
        .or_else(|| asset_definition_move_ambient(&template.name, slot))
}

pub fn move_uses_damaged(state: HostBodyDamageType) -> bool {
    // C++ `IS_CONDITION_WORSE(getDamageState(), BODY_DAMAGED)` — ReallyDamaged/Rubble.
    matches!(
        state,
        HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble
    )
}

pub fn resolve_ambient_event(
    template: &ThingTemplate,
    state: HostBodyDamageType,
) -> Option<String> {
    let slot = match state {
        HostBodyDamageType::Damaged => TemplateMoveAmbientSlot::SoundAmbientDamaged,
        HostBodyDamageType::ReallyDamaged => TemplateMoveAmbientSlot::SoundAmbientReallyDamaged,
        HostBodyDamageType::Rubble => TemplateMoveAmbientSlot::SoundAmbientRubble,
        HostBodyDamageType::Pristine => TemplateMoveAmbientSlot::SoundAmbient,
    };
    let primary = resolve_for_object(template, slot);
    if primary.is_some() {
        return primary;
    }
    // C++: non-pristine / non-rubble missing damage ambient falls back to pristine.
    if !matches!(
        state,
        HostBodyDamageType::Pristine | HostBodyDamageType::Rubble
    ) {
        return resolve_for_object(template, TemplateMoveAmbientSlot::SoundAmbient);
    }
    None
}

pub fn apply_authored_audio_events(
    template: &mut ThingTemplate,
    definition: &crate::assets::ObjectDefinition,
) {
    let attr = |key: &str| {
        definition
            .attributes
            .iter()
            .find_map(|(attr, value)| attr.eq_ignore_ascii_case(key).then(|| value.as_str()))
    };
    if let Some(v) = attr("SoundMoveStart") {
        template.sound_move_start = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundMoveStartDamaged") {
        template.sound_move_start_damaged = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundMoveLoop") {
        template.sound_move_loop = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundMoveLoopDamaged") {
        template.sound_move_loop_damaged = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundAmbient") {
        template.sound_ambient = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundAmbientDamaged") {
        template.sound_ambient_damaged = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundAmbientReallyDamaged") {
        template.sound_ambient_really_damaged = nonempty_audio_event_name(v);
    }
    if let Some(v) = attr("SoundAmbientRubble") {
        template.sound_ambient_rubble = nonempty_audio_event_name(v);
    }
}

pub fn definition_has_sound_ambient(definition: &crate::assets::ObjectDefinition) -> bool {
    definition.attributes.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case("SoundAmbient") && nonempty_audio_event_name(value).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nosound_and_empty_are_absent() {
        assert!(nonempty_audio_event_name("").is_none());
        assert!(nonempty_audio_event_name("NoSound").is_none());
        assert_eq!(
            nonempty_audio_event_name("CrusaderMoveStart").as_deref(),
            Some("CrusaderMoveStart")
        );
    }

    #[test]
    fn host_template_fields_resolve() {
        let mut tmpl = ThingTemplate::new("AmericaTankCrusader");
        tmpl.sound_move_start = Some("CrusaderMoveStart".into());
        tmpl.sound_ambient = Some("CrusaderAmbientLoop".into());
        assert_eq!(
            host_template_move_ambient(&tmpl, TemplateMoveAmbientSlot::SoundMoveStart).as_deref(),
            Some("CrusaderMoveStart")
        );
        assert_eq!(
            host_template_move_ambient(&tmpl, TemplateMoveAmbientSlot::SoundAmbient).as_deref(),
            Some("CrusaderAmbientLoop")
        );
        assert!(
            host_template_move_ambient(&tmpl, TemplateMoveAmbientSlot::SoundMoveLoop).is_none()
        );
    }
}
