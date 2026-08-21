//! Concrete implementation of `GameplayAudioDispatch` for the Main crate.
//!
//! Routes gameplay audio events (weapon fire, unit death, EVA) through the
//! existing `AudioManagerSubsystem` which uses the `SoundEffectsTable` to
//! resolve INI event names to concrete sound file paths and plays them
//! through the rodio audio backend.

use game_engine::common::audio::GameplayAudioDispatch;
use game_engine::common::ascii_string::AsciiString;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Concrete dispatch that queues audio events for the `AudioManagerSubsystem`
/// to process on the next frame.
///
/// This avoids calling async audio directly from gameplay code (which runs on
/// the logic thread) and instead feeds events into the same queue that
/// `GameLogic::process_audio_events()` uses.
pub struct MainAudioDispatch {
    events: Mutex<Vec<GameplayAudioEvent>>,
}

/// An audio event queued for playback.
#[derive(Debug, Clone)]
pub struct GameplayAudioEvent {
    pub event_name: String,
    pub position: Option<(f32, f32, f32)>,
}

impl Default for MainAudioDispatch {
    fn default() -> Self {
        Self::new()
    }
}

impl MainAudioDispatch {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    /// Drain all queued events (called from the subsystem update).
    pub fn drain_events(&self) -> Vec<GameplayAudioEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }
}

impl GameplayAudioDispatch for MainAudioDispatch {
    fn play_positional_sound(&self, event_name: &str, x: f32, y: f32, z: f32) {
        let event = GameplayAudioEvent {
            event_name: event_name.to_string(),
            position: Some((x, y, z)),
        };
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event);
    }

    fn play_2d_sound(&self, event_name: &str) {
        let event = GameplayAudioEvent {
            event_name: event_name.to_string(),
            position: None,
        };
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event);
    }
}

/// C++ `pickAndPlayUnitVoiceResponse` slot (`CommandXlat.cpp:311-635`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitVoiceSlot {
    /// ThingTemplate `VoiceSelect`.
    Select,
    /// ThingTemplate `VoiceMove`.
    Move,
    /// ThingTemplate `VoiceAttack`.
    Attack,
    /// ThingTemplate `VoiceAttackAir`.
    AttackAir,
    /// ThingTemplate `VoiceGuard`.
    Guard,
    /// Per-unit / `VoiceEnter`.
    Enter,
    /// Per-unit `VoiceEnterHostile`.
    EnterHostile,
    /// Per-unit / `VoiceGarrison`.
    Garrison,
    /// Per-unit `VoiceGetHealed`.
    GetHealed,
    /// Per-unit `VoiceUnload`.
    Unload,
    /// Per-unit `VoiceCrush`.
    Crush,
    /// Per-unit `VoiceSupply`.
    Supply,
    /// Per-unit `VoiceRepair`.
    Repair,
    /// Per-unit `VoiceBuildResponse`.
    BuildResponse,
    /// ThingTemplate `VoiceCreated`.
    Created,
    /// Per-unit `VoiceCreate` (first of a production batch).
    Create,
}

impl UnitVoiceSlot {
    /// INI / PerUnitSound key. C++ never concatenates `{template}Voice*`.
    pub fn ini_key(self) -> &'static str {
        match self {
            Self::Select => "VoiceSelect",
            Self::Move => "VoiceMove",
            Self::Attack => "VoiceAttack",
            Self::AttackAir => "VoiceAttackAir",
            Self::Guard => "VoiceGuard",
            Self::Enter => "VoiceEnter",
            Self::EnterHostile => "VoiceEnterHostile",
            Self::Garrison => "VoiceGarrison",
            Self::GetHealed => "VoiceGetHealed",
            Self::Unload => "VoiceUnload",
            Self::Crush => "VoiceCrush",
            Self::Supply => "VoiceSupply",
            Self::Repair => "VoiceRepair",
            Self::BuildResponse => "VoiceBuildResponse",
            Self::Created => "VoiceCreated",
            Self::Create => "VoiceCreate",
        }
    }

    /// C++ `skip=true` slots take the first unit; move/attack scan to last.
    pub fn first_unit_wins(self) -> bool {
        !matches!(self, Self::Move | Self::Attack | Self::AttackAir)
    }
}

thread_local! {
    static TEMPLATE_VOICE_OVERRIDE: RefCell<HashMap<(String, UnitVoiceSlot), String>> =
        RefCell::new(HashMap::new());
}

fn nonempty_event_name(event: &game_engine::common::audio::AudioEventRts) -> Option<String> {
    let name = event.get_event_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Test hook: bind a ThingTemplate INI event without loading Object.ini.
pub fn set_test_template_voice(template_name: &str, slot: UnitVoiceSlot, event_name: impl Into<String>) {
    TEMPLATE_VOICE_OVERRIDE.with(|m| {
        m.borrow_mut()
            .insert((template_name.to_string(), slot), event_name.into());
    });
}

pub fn clear_test_template_voices() {
    TEMPLATE_VOICE_OVERRIDE.with(|m| m.borrow_mut().clear());
}

fn named_template_voice(
    tmpl: &game_engine::common::thing::thing_template::ThingTemplate,
    slot: UnitVoiceSlot,
) -> Option<String> {
    match slot {
        UnitVoiceSlot::Select => tmpl.get_voice_select().and_then(nonempty_event_name),
        UnitVoiceSlot::Move => tmpl.get_voice_move().and_then(nonempty_event_name),
        UnitVoiceSlot::Attack => tmpl.get_voice_attack().and_then(nonempty_event_name),
        UnitVoiceSlot::AttackAir => tmpl.get_voice_attack_air().and_then(nonempty_event_name),
        UnitVoiceSlot::Guard => tmpl.get_voice_guard().and_then(nonempty_event_name),
        UnitVoiceSlot::Enter | UnitVoiceSlot::EnterHostile => {
            tmpl.get_voice_enter().and_then(nonempty_event_name)
        }
        UnitVoiceSlot::Garrison => tmpl.get_voice_garrison().and_then(nonempty_event_name),
        UnitVoiceSlot::Created => tmpl.get_voice_created().and_then(nonempty_event_name),
        _ => None,
    }
}

/// Resolve the authored Voice.ini event for a live host unit.
///
/// C++ `ThingTemplate::getVoice*` / `getPerUnitSound` — never `{template}Voice*`
/// and never invented `UnitVoiceSelect|Move|Attack`.
pub fn resolve_unit_voice_event(template_name: &str, slot: UnitVoiceSlot) -> Option<String> {
    if let Some(over) = TEMPLATE_VOICE_OVERRIDE.with(|m| {
        m.borrow()
            .get(&(template_name.to_string(), slot))
            .cloned()
    }) {
        if over.is_empty() {
            return None;
        }
        return Some(over);
    }

    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                let key = slot.ini_key().to_string();
                if let Some(name) = tmpl
                    .get_per_unit_sound(&key)
                    .and_then(nonempty_event_name)
                {
                    return Some(name);
                }
                if let Some(name) = named_template_voice(tmpl.as_ref(), slot) {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// C++ MSG_ENTER voice upgrade (`CommandXlat.cpp:331-371`).
pub fn enter_voice_slot(
    target_is_heal_pad: bool,
    target_is_structure: bool,
    enemies: bool,
    allies: bool,
) -> UnitVoiceSlot {
    if target_is_heal_pad {
        UnitVoiceSlot::GetHealed
    } else if target_is_structure {
        if enemies {
            UnitVoiceSlot::EnterHostile
        } else {
            UnitVoiceSlot::Garrison
        }
    } else if !allies {
        UnitVoiceSlot::EnterHostile
    } else {
        UnitVoiceSlot::Enter
    }
}

/// True when the name is an invented host residual, not a Voice.ini event.
pub fn is_invented_unit_voice_name(name: &str) -> bool {
    matches!(
        name,
        "UnitVoiceSelect" | "UnitVoiceMove" | "UnitVoiceAttack" | "UnitMove"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_thing_template_ini_not_concatenated_or_unit_voice() {
        clear_test_template_voices();
        set_test_template_voice("AmericaInfantryRanger", UnitVoiceSlot::Select, "AmericaRangerVoiceSelect");
        set_test_template_voice("AmericaInfantryRanger", UnitVoiceSlot::Move, "AmericaRangerVoiceMove");
        set_test_template_voice("AmericaInfantryRanger", UnitVoiceSlot::Attack, "AmericaRangerVoiceAttack");
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Select).as_deref(),
            Some("AmericaRangerVoiceSelect")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Move).as_deref(),
            Some("AmericaRangerVoiceMove")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Attack).as_deref(),
            Some("AmericaRangerVoiceAttack")
        );
        let select = resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Select).unwrap();
        assert!(!is_invented_unit_voice_name(&select));
        assert_ne!(select, "AmericaInfantryRangerVoiceSelect");
        assert_ne!(select, "UnitVoiceSelect");
        clear_test_template_voices();
    }

    #[test]
    fn specialty_slots_resolve_crush_unload_enter_garrison_air_guard() {
        clear_test_template_voices();
        set_test_template_voice("AmericaVehicleHumvee", UnitVoiceSlot::Crush, "HumveeVoiceCrush");
        set_test_template_voice("AmericaVehicleTroopCrawler", UnitVoiceSlot::Unload, "TroopCrawlerVoiceUnload");
        set_test_template_voice("AmericaInfantryRanger", UnitVoiceSlot::Garrison, "RangerVoiceGarrison");
        set_test_template_voice("AmericaInfantryRanger", UnitVoiceSlot::Enter, "RangerVoiceEnter");
        set_test_template_voice("AmericaInfantryMissileDefender", UnitVoiceSlot::AttackAir, "MissileDefenderVoiceAttackAir");
        set_test_template_voice("AmericaInfantryRanger", UnitVoiceSlot::Guard, "RangerVoiceGuard");
        assert_eq!(
            resolve_unit_voice_event("AmericaVehicleHumvee", UnitVoiceSlot::Crush).as_deref(),
            Some("HumveeVoiceCrush")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaVehicleTroopCrawler", UnitVoiceSlot::Unload).as_deref(),
            Some("TroopCrawlerVoiceUnload")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Garrison).as_deref(),
            Some("RangerVoiceGarrison")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Enter).as_deref(),
            Some("RangerVoiceEnter")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryMissileDefender", UnitVoiceSlot::AttackAir)
                .as_deref(),
            Some("MissileDefenderVoiceAttackAir")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Guard).as_deref(),
            Some("RangerVoiceGuard")
        );
        assert!(matches!(
            enter_voice_slot(false, true, false, true),
            UnitVoiceSlot::Garrison
        ));
        assert!(matches!(
            enter_voice_slot(true, true, false, true),
            UnitVoiceSlot::GetHealed
        ));
        clear_test_template_voices();
    }
}
