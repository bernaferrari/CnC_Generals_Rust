//! Concrete implementation of `GameplayAudioDispatch` for the Main crate.
//!
//! Routes gameplay audio events (weapon fire, unit death, EVA) through the
//! existing `AudioManagerSubsystem` which uses the `SoundEffectsTable` to
//! resolve INI event names to concrete sound file paths and plays them
//! through the rodio audio backend.

use game_engine::common::ascii_string::AsciiString;
use game_engine::common::audio::GameplayAudioDispatch;
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
    /// Per-unit `VoiceSnipePilot` (MSG_DO_WEAPON_AT_OBJECT DAMAGE_KILLPILOT).
    SnipePilot,
    /// ThingTemplate `VoiceCreated`.
    Created,
    /// Per-unit `VoiceCreate` (first of a production batch).
    Create,
    /// Per-unit `VoiceCombatDrop` (MSG_COMBATDROP_AT_*).
    CombatDrop,
    /// Per-unit `VoiceHackInternet` (MSG_INTERNET_HACK).
    HackInternet,
    /// Per-unit `VoiceSalvage` (MSG_DO_SALVAGE when valid).
    Salvage,
    /// Per-unit `VoiceClearBuilding` (DAMAGE_SURRENDER vs structure).
    ClearBuilding,
    /// Per-unit `VoiceSubdue` (DAMAGE_SURRENDER vs non-structure).
    Subdue,
    /// Per-unit `VoiceDisarm` (DAMAGE_DISARM mine clear).
    Disarm,
    /// Per-unit `VoiceMelee` (MSG_DO_WEAPON_AT_OBJECT DAMAGE_MELEE).
    Melee,
    /// Per-unit `VoiceFlameLocation` (non-primary DAMAGE_FLAME ground fire).
    FlameLocation,
    /// Per-unit `VoicePoisonLocation` (non-primary DAMAGE_POISON ground fire).
    PoisonLocation,
    /// Per-unit `VoiceFireRocketPods` (ComancheRocketPodWeapon at location).
    FireRocketPods,
    /// Per-unit `VoicePrimaryWeaponMode` (MSG_SWITCH_WEAPONS PRIMARY).
    PrimaryWeaponMode,
    /// Per-unit `VoiceSecondaryWeaponMode` (MSG_SWITCH_WEAPONS SECONDARY).
    SecondaryWeaponMode,
    /// Per-unit `VoiceTertiaryWeaponMode` (MSG_SWITCH_WEAPONS TERTIARY).
    TertiaryWeaponMode,
    /// Per-unit `VoiceBombard` (MSG_DO_FORCE_ATTACK_GROUND when valid).
    Bombard,
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
            Self::SnipePilot => "VoiceSnipePilot",
            Self::Created => "VoiceCreated",
            Self::Create => "VoiceCreate",
            Self::CombatDrop => "VoiceCombatDrop",
            Self::HackInternet => "VoiceHackInternet",
            Self::Salvage => "VoiceSalvage",
            Self::ClearBuilding => "VoiceClearBuilding",
            Self::Subdue => "VoiceSubdue",
            Self::Disarm => "VoiceDisarm",
            Self::Melee => "VoiceMelee",
            Self::FlameLocation => "VoiceFlameLocation",
            Self::PoisonLocation => "VoicePoisonLocation",
            Self::FireRocketPods => "VoiceFireRocketPods",
            Self::PrimaryWeaponMode => "VoicePrimaryWeaponMode",
            Self::SecondaryWeaponMode => "VoiceSecondaryWeaponMode",
            Self::TertiaryWeaponMode => "VoiceTertiaryWeaponMode",
            Self::Bombard => "VoiceBombard",
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
    static PER_UNIT_SOUND_OVERRIDE: RefCell<HashMap<(String, String), String>> =
        RefCell::new(HashMap::new());
    static INITIATE_SOUND_OVERRIDE: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static BUILDING_LOOPS: RefCell<HashMap<u32, String>> = RefCell::new(HashMap::new());
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
pub fn set_test_template_voice(
    template_name: &str,
    slot: UnitVoiceSlot,
    event_name: impl Into<String>,
) {
    TEMPLATE_VOICE_OVERRIDE.with(|m| {
        m.borrow_mut()
            .insert((template_name.to_string(), slot), event_name.into());
    });
}

pub fn clear_test_template_voices() {
    TEMPLATE_VOICE_OVERRIDE.with(|m| m.borrow_mut().clear());
    PER_UNIT_SOUND_OVERRIDE.with(|m| m.borrow_mut().clear());
    INITIATE_SOUND_OVERRIDE.with(|m| m.borrow_mut().clear());
}

/// Test hook: ThingTemplate `UnitSpecificSounds` key without loading Object.ini.
pub fn set_test_per_unit_sound(template_name: &str, key: &str, event_name: impl Into<String>) {
    PER_UNIT_SOUND_OVERRIDE.with(|m| {
        m.borrow_mut().insert(
            (template_name.to_string(), key.to_string()),
            event_name.into(),
        );
    });
}

/// C++ `ThingTemplate::getPerUnitSound(key)` — building UnderConstruction, etc.
pub fn resolve_per_unit_sound(template_name: &str, key: &str) -> Option<String> {
    if let Some(over) = PER_UNIT_SOUND_OVERRIDE.with(|m| {
        m.borrow()
            .get(&(template_name.to_string(), key.to_string()))
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
                let key = key.to_string();
                if let Some(name) = tmpl.get_per_unit_sound(&key).and_then(nonempty_event_name) {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// Test hook: SpecialPower / SpecialAbility `InitiateSound`.
pub fn set_test_initiate_sound(power_key: &str, event_name: impl Into<String>) {
    INITIATE_SOUND_OVERRIDE.with(|m| {
        m.borrow_mut()
            .insert(power_key.to_string(), event_name.into());
    });
}

/// C++ `spmInterface->getInitiateSound()` (CommandXlat.cpp:637-651).
pub fn resolve_special_power_initiate_sound(
    power_key: &str,
    module_template: Option<&str>,
    retail_fallback: Option<&str>,
) -> Option<String> {
    if let Some(over) = INITIATE_SOUND_OVERRIDE.with(|m| m.borrow().get(power_key).cloned()) {
        if over.is_empty() {
            return None;
        }
        return Some(over);
    }
    if let Some(name) = module_template {
        let store = game_engine::common::rts::special_power::get_special_power_store();
        if let Some(tmpl) = store.find_template(name) {
            if !tmpl.initiate_sound.is_empty() {
                return Some(tmpl.initiate_sound.clone());
            }
        }
    }
    retail_fallback
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Remember a live UnderConstruction loop on the construction site.
pub fn note_building_loop(site_id: u32, event: &str) -> Option<String> {
    BUILDING_LOOPS.with(|m| m.borrow_mut().insert(site_id, event.to_string()))
}

pub fn building_loop_event(site_id: u32) -> Option<String> {
    BUILDING_LOOPS.with(|m| m.borrow().get(&site_id).cloned())
}

pub fn take_building_loop(site_id: u32) -> Option<String> {
    BUILDING_LOOPS.with(|m| m.borrow_mut().remove(&site_id))
}

pub fn clear_building_loops() {
    BUILDING_LOOPS.with(|m| m.borrow_mut().clear());
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
    if let Some(over) = TEMPLATE_VOICE_OVERRIDE
        .with(|m| m.borrow().get(&(template_name.to_string(), slot)).cloned())
    {
        if over.is_empty() {
            return None;
        }
        return Some(over);
    }

    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                let key = slot.ini_key().to_string();
                if let Some(name) = tmpl.get_per_unit_sound(&key).and_then(nonempty_event_name) {
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

/// One selected unit's current / commanded weapon for attack-voice upgrade.
pub struct AttackVoiceWeapon {
    pub name: String,
    pub slot: u8,
}

/// C++ `CommandXlat.cpp:511-627` — upgrade VoiceAttack / VoiceAttackAir from the weapon.
///
/// `skip` is C++ `skip=true` (first specialty wins). Location DISARM / FLAME / POISON
/// keep scanning.
pub fn specialty_attack_voice_upgrade(
    weapon_name: &str,
    weapon_slot: u8,
    target_is_structure: bool,
    specialty_weapon: bool,
    at_location: bool,
) -> Option<(UnitVoiceSlot, bool)> {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name;

    if at_location && weapon_name.eq_ignore_ascii_case("ComancheRocketPodWeapon") {
        return Some((UnitVoiceSlot::FireRocketPods, true));
    }

    let mut damage = host_damage_type_for_weapon_name(weapon_name);
    if matches!(damage, DamageType::Bullet) {
        let n = weapon_name.to_ascii_lowercase();
        if n.contains("flashbang") {
            damage = DamageType::Surrender;
        } else if n.contains("melee") || n.contains("knife") || n.contains("stab") {
            damage = DamageType::Melee;
        } else if n.contains("flame") || n.contains("firewall") || n.contains("firestorm") {
            damage = DamageType::Flame;
        } else if n.contains("poison") || n.contains("toxin") {
            damage = DamageType::Toxin;
        }
    }

    if at_location {
        return match damage {
            DamageType::Disarm => Some((UnitVoiceSlot::Disarm, false)),
            DamageType::Flame | DamageType::Fire if weapon_slot != 0 => {
                Some((UnitVoiceSlot::FlameLocation, false))
            }
            DamageType::Toxin | DamageType::Anthrax if weapon_slot != 0 => {
                Some((UnitVoiceSlot::PoisonLocation, false))
            }
            _ => None,
        };
    }

    match damage {
        DamageType::Surrender => {
            let slot = if target_is_structure {
                UnitVoiceSlot::ClearBuilding
            } else {
                UnitVoiceSlot::Subdue
            };
            Some((slot, true))
        }
        DamageType::Disarm => Some((UnitVoiceSlot::Disarm, true)),
        DamageType::KillPilot if specialty_weapon => Some((UnitVoiceSlot::SnipePilot, true)),
        DamageType::Melee if specialty_weapon => Some((UnitVoiceSlot::Melee, true)),
        _ => None,
    }
}

/// Walk selected weapons like `pickAndPlayUnitVoiceResponse` (`CommandXlat.cpp:294-568`).
pub fn pick_specialty_attack_voice(
    default: UnitVoiceSlot,
    weapons: impl IntoIterator<Item = AttackVoiceWeapon>,
    target_is_structure: bool,
    specialty_weapon: bool,
    at_location: bool,
) -> UnitVoiceSlot {
    let mut slot = default;
    let mut skip = false;
    for weapon in weapons {
        if skip {
            break;
        }
        if let Some((upgraded, stop)) = specialty_attack_voice_upgrade(
            &weapon.name,
            weapon.slot,
            target_is_structure,
            specialty_weapon,
            at_location,
        ) {
            slot = upgraded;
            skip = stop;
        }
    }
    slot
}

/// True when the name is an invented host residual, not a Voice.ini event.
pub fn is_invented_unit_voice_name(name: &str) -> bool {
    matches!(
        name,
        "UnitVoiceSelect" | "UnitVoiceMove" | "UnitVoiceAttack" | "UnitMove"
    )
}

use game_engine::common::audio::{
    AudioEventInfo, AudioLocalityRelationship, AudioPriority, AudioType, ST_SHROUDED, ST_WORLD,
    should_play_locally_for_players, shrouded_positional_event_is_blocked,
};

/// Live-host snapshot for C++ shouldPlayLocally / canPlayNow (player path).
#[derive(Clone, Debug, Default)]
pub struct LiveAudioLocality {
    pub local_player_index: i32,
    pub local_player_active: bool,
    pub observer_look_at: Option<i32>,
    pub players: HashMap<i32, LiveAudioPlayer>,
    pub object_owners: HashMap<u32, i32>,
}

#[derive(Clone, Copy, Debug)]
pub struct LiveAudioPlayer {
    pub exists: bool,
    pub active: bool,
    pub has_default_team: bool,
    pub relationship_to_local: AudioLocalityRelationship,
}

static LIVE_AUDIO_LOCALITY: Mutex<Option<LiveAudioLocality>> = Mutex::new(None);

pub fn set_live_audio_locality(snap: LiveAudioLocality) {
    *LIVE_AUDIO_LOCALITY
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(snap);
}

pub fn live_audio_locality() -> Option<LiveAudioLocality> {
    LIVE_AUDIO_LOCALITY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn lookup_audio_event_info(event_name: &str) -> Option<AudioEventInfo> {
    let manager = game_engine::common::audio::game_audio::get_global_audio_manager()?;
    let guard = manager.lock().ok()?;
    guard
        .find_audio_event_info(event_name)
        .map(|info| (*info).clone())
}

fn live_position_is_cellshroud_clear(local_player: i32, host_pos: glam::Vec3) -> bool {
    // C++ SoundManager::canPlayNow: ThePartitionManager->getShroudStatusForPlayer
    // (GameSounds.cpp:203-205). Leftover ThePartitionManager maps that onto
    // ShroudManager::get_shroud_state — DiscreteCircle looker cells, not the
    // PARTITION_MANAGER square r² disk (partition_shroud.rs:137-163).
    // Host AudioEventRequest is Y-up (x, height, z_ground); leftover Coord3D
    // is Z-up (x, y_ground, z_height).
    let Some(pm) = gamelogic::helpers::ThePartitionManager::get() else {
        return false;
    };
    let world = gamelogic::common::Coord3D::new(host_pos.x, host_pos.z, host_pos.y);
    matches!(
        pm.get_shroud_status_for_player(local_player, &world),
        game_engine::common::system::radar::CellShroudStatus::Clear
    )
}

/// C++ `shouldPlayLocally` + `canPlayNow` ST_SHROUDED for the live SFX drain.
///
/// Missing AudioEventInfo type bits default Everyone (C++ GameAudio.cpp:1005-1007).
/// Missing locality snapshot fails open so boot/tests without a host still hear
/// unrestricted events; restricted bits still require a snapshot to pass.
pub fn should_dispatch_live_audio(
    event_name: &str,
    owning_player_index: Option<i32>,
    position: Option<glam::Vec3>,
) -> bool {
    if is_invented_unit_voice_name(event_name) || is_generic_eva_sfx_name(event_name) {
        return false;
    }

    let info = lookup_audio_event_info(event_name);
    let type_field = info.as_ref().map(|i| i.type_field).unwrap_or(0);
    let is_music = info
        .as_ref()
        .is_some_and(|i| i.sound_type == AudioType::Music);
    let is_critical = info
        .as_ref()
        .is_some_and(|i| i.priority == AudioPriority::Critical);
    let is_positional = position.is_some()
        && info
            .as_ref()
            .map(|i| (i.type_field & ST_WORLD) != 0)
            .unwrap_or(position.is_some());

    let snap = live_audio_locality();
    let owning = owning_player_index.or_else(|| None);
    let owning_exists = owning
        .and_then(|idx| {
            snap.as_ref()
                .and_then(|s| s.players.get(&idx).map(|p| p.exists))
        })
        .unwrap_or(owning.is_some());

    let (local_idx, local_active, observer, local_has_team, relationship) =
        if let Some(s) = snap.as_ref() {
            let mut local = s.local_player_index;
            let active = s
                .players
                .get(&local)
                .map(|p| p.active)
                .unwrap_or(s.local_player_active);
            if !active {
                if let Some(obs) = s.observer_look_at {
                    local = obs;
                }
            }
            let local_rec = s.players.get(&local);
            let rel = owning
                .and_then(|idx| s.players.get(&idx).map(|p| p.relationship_to_local))
                .unwrap_or(AudioLocalityRelationship::Neutral);
            (
                Some(local),
                active,
                s.observer_look_at,
                local_rec
                    .map(|p| p.exists && p.has_default_team)
                    .unwrap_or(true),
                rel,
            )
        } else {
            (None, false, None, false, AudioLocalityRelationship::Neutral)
        };

    if !should_play_locally_for_players(
        type_field,
        is_music,
        owning.unwrap_or(-1),
        owning_exists,
        local_idx,
        local_active,
        observer,
        local_has_team,
        relationship,
    ) {
        return false;
    }

    if let (Some(pos), Some(local)) = (position, local_idx) {
        let clear = live_position_is_cellshroud_clear(local, pos);
        if shrouded_positional_event_is_blocked(type_field, is_positional, is_critical, clear) {
            return false;
        }
    }

    true
}

/// Generic `EVA_*` SFX names invented by the presentation drain. C++ Eva.ini
/// SideSounds (`EvaUSA_BuildingLost`, …) are the only EVA playback path.
pub fn is_generic_eva_sfx_name(name: &str) -> bool {
    name.starts_with("EVA_")
}

/// C++ MiscAudio `TerroristInCar*Voice` (CommandXlat.cpp:690-728).
pub fn terrorist_in_car_voice_event(slot: UnitVoiceSlot) -> Option<String> {
    let misc = game_engine::common::ini::ini_misc_audio::get_misc_audio()?;
    let misc = misc.read();
    let event = match slot {
        UnitVoiceSlot::Select => &misc.terrorist_in_car_select_voice,
        UnitVoiceSlot::Move | UnitVoiceSlot::Crush | UnitVoiceSlot::Repair => {
            &misc.terrorist_in_car_move_voice
        }
        UnitVoiceSlot::Attack | UnitVoiceSlot::AttackAir => &misc.terrorist_in_car_attack_voice,
        _ => return None,
    };
    let name = event.playable_event_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Fallback INI token when MiscAudio.ini has not been parsed yet.
pub fn terrorist_in_car_voice_token(slot: UnitVoiceSlot) -> Option<&'static str> {
    match slot {
        UnitVoiceSlot::Select => Some("TerroristInCarSelectVoice"),
        UnitVoiceSlot::Move | UnitVoiceSlot::Crush | UnitVoiceSlot::Repair => {
            Some("TerroristInCarMoveVoice")
        }
        UnitVoiceSlot::Attack | UnitVoiceSlot::AttackAir => Some("TerroristInCarAttackVoice"),
        _ => None,
    }
}

pub fn resolve_terrorist_in_car_voice(slot: UnitVoiceSlot) -> Option<String> {
    terrorist_in_car_voice_event(slot)
        .or_else(|| terrorist_in_car_voice_token(slot).map(str::to_string))
}

pub fn owning_player_for_audio_object(
    object_id: Option<crate::game_logic::ObjectId>,
) -> Option<i32> {
    let id = object_id?.0;
    live_audio_locality()?.object_owners.get(&id).copied()
}

/// True when this request may enter the live SFX drain.
pub fn should_dispatch_audio_request(event: &crate::game_logic::AudioEventRequest) -> bool {
    let owner = event
        .player_index
        .or_else(|| owning_player_for_audio_object(event.object_id));
    should_dispatch_live_audio(event.event_type.as_str(), owner, event.position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_uses_thing_template_ini_not_concatenated_or_unit_voice() {
        clear_test_template_voices();
        set_test_template_voice(
            "AmericaInfantryRanger",
            UnitVoiceSlot::Select,
            "AmericaRangerVoiceSelect",
        );
        set_test_template_voice(
            "AmericaInfantryRanger",
            UnitVoiceSlot::Move,
            "AmericaRangerVoiceMove",
        );
        set_test_template_voice(
            "AmericaInfantryRanger",
            UnitVoiceSlot::Attack,
            "AmericaRangerVoiceAttack",
        );
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
        let select =
            resolve_unit_voice_event("AmericaInfantryRanger", UnitVoiceSlot::Select).unwrap();
        assert!(!is_invented_unit_voice_name(&select));
        assert_ne!(select, "AmericaInfantryRangerVoiceSelect");
        assert_ne!(select, "UnitVoiceSelect");
        clear_test_template_voices();
    }

    #[test]
    fn specialty_slots_resolve_crush_unload_enter_garrison_air_guard() {
        clear_test_template_voices();
        set_test_template_voice(
            "AmericaVehicleHumvee",
            UnitVoiceSlot::Crush,
            "HumveeVoiceCrush",
        );
        set_test_template_voice(
            "AmericaVehicleTroopCrawler",
            UnitVoiceSlot::Unload,
            "TroopCrawlerVoiceUnload",
        );
        set_test_template_voice(
            "AmericaInfantryRanger",
            UnitVoiceSlot::Garrison,
            "RangerVoiceGarrison",
        );
        set_test_template_voice(
            "AmericaInfantryRanger",
            UnitVoiceSlot::Enter,
            "RangerVoiceEnter",
        );
        set_test_template_voice(
            "AmericaInfantryMissileDefender",
            UnitVoiceSlot::AttackAir,
            "MissileDefenderVoiceAttackAir",
        );
        set_test_template_voice(
            "AmericaInfantryRanger",
            UnitVoiceSlot::Guard,
            "RangerVoiceGuard",
        );
        set_test_template_voice(
            "GLAInfantryJarmenKell",
            UnitVoiceSlot::SnipePilot,
            "JarmenKellVoiceSnipe",
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaVehicleHumvee", UnitVoiceSlot::Crush).as_deref(),
            Some("HumveeVoiceCrush")
        );
        assert_eq!(
            resolve_unit_voice_event("AmericaVehicleTroopCrawler", UnitVoiceSlot::Unload)
                .as_deref(),
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
        assert_eq!(
            resolve_unit_voice_event("GLAInfantryJarmenKell", UnitVoiceSlot::SnipePilot).as_deref(),
            Some("JarmenKellVoiceSnipe")
        );
        assert_eq!(UnitVoiceSlot::SnipePilot.ini_key(), "VoiceSnipePilot");
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

    #[test]
    fn specialty_attack_voice_upgrades_from_weapon() {
        assert_eq!(
            specialty_attack_voice_upgrade("RangerFlashBangGrenadeWeapon", 1, true, false, false,),
            Some((UnitVoiceSlot::ClearBuilding, true))
        );
        assert_eq!(
            specialty_attack_voice_upgrade("RangerFlashBangGrenadeWeapon", 1, false, false, false,),
            Some((UnitVoiceSlot::Subdue, true))
        );
        assert_eq!(
            specialty_attack_voice_upgrade("DozerMineDisarmingWeapon", 0, false, false, false),
            Some((UnitVoiceSlot::Disarm, true))
        );
        assert_eq!(
            specialty_attack_voice_upgrade("DozerMineDisarmingWeapon", 0, false, false, true),
            Some((UnitVoiceSlot::Disarm, false))
        );
        assert_eq!(
            specialty_attack_voice_upgrade("BurtonKnifeWeapon", 2, false, true, false),
            Some((UnitVoiceSlot::Melee, true))
        );
        assert!(
            specialty_attack_voice_upgrade("BurtonKnifeWeapon", 2, false, false, false).is_none()
        );
        assert_eq!(
            specialty_attack_voice_upgrade(
                "GLAJarmenKellVehiclePilotSniperRifle",
                1,
                false,
                true,
                false,
            ),
            Some((UnitVoiceSlot::SnipePilot, true))
        );
        assert_eq!(
            specialty_attack_voice_upgrade("DragonTankFireWallWeapon", 1, false, true, true),
            Some((UnitVoiceSlot::FlameLocation, false))
        );
        assert!(
            specialty_attack_voice_upgrade("DragonTankFlameWeapon", 0, false, true, true).is_none()
        );
        assert_eq!(
            specialty_attack_voice_upgrade("ToxinTractorSprayWeapon", 1, false, true, true),
            Some((UnitVoiceSlot::PoisonLocation, false))
        );
        assert_eq!(
            specialty_attack_voice_upgrade("ComancheRocketPodWeapon", 2, false, true, true),
            Some((UnitVoiceSlot::FireRocketPods, true))
        );
        assert_eq!(UnitVoiceSlot::ClearBuilding.ini_key(), "VoiceClearBuilding");
        assert_eq!(UnitVoiceSlot::Subdue.ini_key(), "VoiceSubdue");
        assert_eq!(UnitVoiceSlot::Disarm.ini_key(), "VoiceDisarm");
        assert_eq!(UnitVoiceSlot::Melee.ini_key(), "VoiceMelee");
        assert_eq!(UnitVoiceSlot::FlameLocation.ini_key(), "VoiceFlameLocation");
        assert_eq!(
            UnitVoiceSlot::PoisonLocation.ini_key(),
            "VoicePoisonLocation"
        );
        assert_eq!(
            UnitVoiceSlot::FireRocketPods.ini_key(),
            "VoiceFireRocketPods"
        );
    }

    #[test]
    fn per_unit_under_construction_and_initiate_sound_resolve() {
        clear_test_template_voices();
        set_test_per_unit_sound(
            "AmericaPowerPlant",
            "UnderConstruction",
            "BuildingConstructionLoop",
        );
        set_test_initiate_sound("SpySatellite", "TestSpySatInitiate");
        assert_eq!(
            resolve_per_unit_sound("AmericaPowerPlant", "UnderConstruction").as_deref(),
            Some("BuildingConstructionLoop")
        );
        assert_eq!(
            resolve_special_power_initiate_sound("SpySatellite", None, None).as_deref(),
            Some("TestSpySatInitiate")
        );
        clear_test_template_voices();
        clear_building_loops();
    }

    #[test]
    fn invented_unit_move_and_generic_eva_never_dispatch() {
        assert!(is_invented_unit_voice_name("UnitMove"));
        assert!(is_generic_eva_sfx_name("EVA_LowPower"));
        assert!(is_generic_eva_sfx_name("EVA_UpgradeComplete"));
        assert!(!is_generic_eva_sfx_name("EvaUSA_BuildingLost"));
        assert!(!should_dispatch_live_audio("UnitMove", Some(0), None));
        assert!(!should_dispatch_live_audio("EVA_LowPower", Some(0), None));
    }

    #[test]
    fn should_dispatch_honors_st_player_allies_enemies() {
        use game_engine::common::audio::{ST_ALLIES, ST_ENEMIES, ST_PLAYER};
        let mut snap = LiveAudioLocality {
            local_player_index: 0,
            local_player_active: true,
            observer_look_at: None,
            players: HashMap::new(),
            object_owners: HashMap::new(),
        };
        snap.players.insert(
            0,
            LiveAudioPlayer {
                exists: true,
                active: true,
                has_default_team: true,
                relationship_to_local: AudioLocalityRelationship::Allies,
            },
        );
        snap.players.insert(
            1,
            LiveAudioPlayer {
                exists: true,
                active: true,
                has_default_team: true,
                relationship_to_local: AudioLocalityRelationship::Enemies,
            },
        );
        set_live_audio_locality(snap);

        // No AudioEventInfo → missing bits default Everyone.
        assert!(should_dispatch_live_audio("SomeWorldBoom", Some(1), None));

        assert!(should_play_locally_for_players(
            ST_PLAYER,
            false,
            0,
            true,
            Some(0),
            true,
            None,
            true,
            AudioLocalityRelationship::Allies,
        ));
        assert!(!should_play_locally_for_players(
            ST_PLAYER,
            false,
            1,
            true,
            Some(0),
            true,
            None,
            true,
            AudioLocalityRelationship::Enemies,
        ));
        assert!(should_play_locally_for_players(
            ST_ALLIES,
            false,
            2,
            true,
            Some(0),
            true,
            None,
            true,
            AudioLocalityRelationship::Allies,
        ));
        assert!(should_play_locally_for_players(
            ST_ENEMIES,
            false,
            1,
            true,
            Some(0),
            true,
            None,
            true,
            AudioLocalityRelationship::Enemies,
        ));
        set_live_audio_locality(LiveAudioLocality::default());
    }

    #[test]
    fn shrouded_world_event_blocked_when_cell_not_clear() {
        assert!(shrouded_positional_event_is_blocked(
            ST_SHROUDED | ST_WORLD,
            true,
            false,
            false,
        ));
        assert!(!shrouded_positional_event_is_blocked(
            ST_SHROUDED | ST_WORLD,
            true,
            false,
            true,
        ));
        assert!(!shrouded_positional_event_is_blocked(
            ST_WORLD, true, false, false,
        ));
    }

    #[test]
    fn carbomb_slots_resolve_terrorist_in_car_tokens() {
        assert_eq!(
            terrorist_in_car_voice_token(UnitVoiceSlot::Select),
            Some("TerroristInCarSelectVoice")
        );
        assert_eq!(
            terrorist_in_car_voice_token(UnitVoiceSlot::Move),
            Some("TerroristInCarMoveVoice")
        );
        assert_eq!(
            terrorist_in_car_voice_token(UnitVoiceSlot::Attack),
            Some("TerroristInCarAttackVoice")
        );
        assert!(terrorist_in_car_voice_token(UnitVoiceSlot::Guard).is_none());
        let name = resolve_terrorist_in_car_voice(UnitVoiceSlot::Select).unwrap();
        assert!(!is_invented_unit_voice_name(&name));
    }
}
