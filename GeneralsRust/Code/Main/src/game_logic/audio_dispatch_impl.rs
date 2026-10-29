//! Receiving audio request admission and original-authored voice resolution.
//! The application queues explicit accepted requests; it never registers as
//! a process-global Core GameplayAudioDispatch sink.

use game_engine::common::ascii_string::AsciiString;
#[cfg(test)]
use std::cell::RefCell;
use std::collections::HashMap;

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
    /// ThingTemplate `VoiceTaskComplete` (SpecialAbilityUpdate pack-complete default).
    TaskComplete,
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
            Self::TaskComplete => "VoiceTaskComplete",
        }
    }

    /// C++ `skip=true` slots take the first unit; move/attack scan to last.
    pub fn first_unit_wins(self) -> bool {
        !matches!(self, Self::Move | Self::Attack | Self::AttackAir)
    }
}

#[cfg(test)]
thread_local! {
    static TEMPLATE_VOICE_OVERRIDE: RefCell<HashMap<(String, UnitVoiceSlot), String>> =
        RefCell::new(HashMap::new());
    static PER_UNIT_SOUND_OVERRIDE: RefCell<HashMap<(String, String), String>> =
        RefCell::new(HashMap::new());
    static INITIATE_SOUND_OVERRIDE: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

fn nonempty_event_name(event: &game_engine::common::audio::AudioEventRts) -> Option<String> {
    let name = event.get_event_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Resolve an Object's own immutable UnitSpecificSounds rather than selecting
/// a process-wide factory by its template name.
pub(crate) fn resolve_receiving_per_unit_sound(
    template: &crate::game_logic::ThingTemplate,
    key: &str,
) -> Option<String> {
    if template.receiving_client_template.is_some() {
        return resolve_admitted_per_unit_sound(template, key);
    }
    #[cfg(test)]
    {
        return resolve_per_unit_sound(&template.name, key);
    }
    #[cfg(not(test))]
    None
}

/// Resolve only this Object's immutable, session-admitted UnitSpecificSounds.
/// Callers that must preserve authored silence should use this instead of the
/// test-fixture-compatible name resolver above.
pub(crate) fn resolve_admitted_per_unit_sound(
    template: &crate::game_logic::ThingTemplate,
    key: &str,
) -> Option<String> {
    template
        .receiving_client_template
        .as_ref()?
        .per_unit_sound_event(key)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

/// C++ pickAndPlayUnitVoiceResponse checks the per-unit slot first, then the
/// fixed ThingTemplate voice slot. Both come from this Object's own template.
pub(crate) fn resolve_receiving_unit_voice(
    template: &crate::game_logic::ThingTemplate,
    slot: UnitVoiceSlot,
) -> Option<String> {
    use game_engine::common::thing::thing_template::ThingTemplateAudioType as Audio;

    if let Some(receiving) = &template.receiving_client_template {
        if let Some(name) = receiving
            .per_unit_sound_event(slot.ini_key())
            .filter(|name| !name.is_empty())
        {
            return Some(name.to_owned());
        }
        let field = match slot {
            UnitVoiceSlot::Select => Audio::VoiceSelect,
            UnitVoiceSlot::Move => Audio::VoiceMove,
            UnitVoiceSlot::Attack => Audio::VoiceAttack,
            UnitVoiceSlot::AttackAir => Audio::VoiceAttackAir,
            UnitVoiceSlot::Guard => Audio::VoiceGuard,
            UnitVoiceSlot::Enter | UnitVoiceSlot::EnterHostile => Audio::VoiceEnter,
            UnitVoiceSlot::Garrison => Audio::VoiceGarrison,
            UnitVoiceSlot::Created => Audio::VoiceCreated,
            UnitVoiceSlot::TaskComplete => Audio::VoiceTaskComplete,
            _ => return None,
        };
        return receiving.audio_event(field).and_then(nonempty_event_name);
    }
    #[cfg(test)]
    {
        return resolve_unit_voice_event(&template.name, slot);
    }
    #[cfg(not(test))]
    None
}

/// Test hook: bind a ThingTemplate INI event without loading Object.ini.
#[cfg(test)]
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

#[cfg(test)]
pub fn clear_test_template_voices() {
    TEMPLATE_VOICE_OVERRIDE.with(|m| m.borrow_mut().clear());
    PER_UNIT_SOUND_OVERRIDE.with(|m| m.borrow_mut().clear());
    INITIATE_SOUND_OVERRIDE.with(|m| m.borrow_mut().clear());
}

/// Test hook: ThingTemplate `UnitSpecificSounds` key without loading Object.ini.
#[cfg(test)]
pub fn set_test_per_unit_sound(template_name: &str, key: &str, event_name: impl Into<String>) {
    PER_UNIT_SOUND_OVERRIDE.with(|m| {
        m.borrow_mut().insert(
            (template_name.to_string(), key.to_string()),
            event_name.into(),
        );
    });
}

/// C++ `ThingTemplate::getPerUnitSound(key)` — building UnderConstruction, etc.
#[cfg(test)]
pub fn resolve_per_unit_sound(template_name: &str, key: &str) -> Option<String> {
    #[cfg(test)]
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
#[cfg(test)]
pub fn set_test_initiate_sound(power_key: &str, event_name: impl Into<String>) {
    INITIATE_SOUND_OVERRIDE.with(|m| {
        m.borrow_mut()
            .insert(power_key.to_string(), event_name.into());
    });
}

/// C++ `spmInterface->getInitiateSound()` (CommandXlat.cpp:637-651).
pub fn resolve_special_power_initiate_sound(
    _power_key: &str,
    module_sound: Option<&str>,
) -> Option<String> {
    #[cfg(test)]
    if let Some(over) = INITIATE_SOUND_OVERRIDE.with(|m| m.borrow().get(_power_key).cloned()) {
        if over.is_empty() {
            return None;
        }
        return Some(over);
    }
    let name = module_sound?.split_whitespace().next()?.trim_matches('"');
    (!name.is_empty() && !name.eq_ignore_ascii_case("None")).then(|| name.to_owned())
}

#[cfg(test)]
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
        UnitVoiceSlot::TaskComplete => tmpl.get_voice_task_complete().and_then(nonempty_event_name),
        _ => None,
    }
}

/// Resolve the authored Voice.ini event for a live host unit.
///
/// C++ `ThingTemplate::getVoice*` / `getPerUnitSound` — never `{template}Voice*`
/// and never invented `UnitVoiceSelect|Move|Attack`.
#[cfg(test)]
pub fn resolve_unit_voice_event(template_name: &str, slot: UnitVoiceSlot) -> Option<String> {
    #[cfg(test)]
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

/// A request whose runtime player/shroud admission was completed on its receiver.
/// Device delivery does not resolve world identities a second time.
#[derive(Debug, Clone)]
pub(crate) struct AdmittedAudioRequest {
    request: crate::game_logic::AudioEventRequest,
    locality: game_engine::common::audio::AudioSubmissionLocality,
}
impl AdmittedAudioRequest {
    pub(crate) fn refresh_script_bus(
        &mut self,
        volumes: game_engine::common::audio::script_policy::ScriptAudioVolumes,
    ) {
        if let Some(policy) = &mut self.request.script_audio_policy {
            policy.volumes = volumes;
        }
    }

    pub(crate) fn into_request(self) -> crate::game_logic::AudioEventRequest {
        self.request
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        crate::game_logic::AudioEventRequest,
        game_engine::common::audio::AudioSubmissionLocality,
    ) {
        (self.request, self.locality)
    }
}
impl std::ops::Deref for AdmittedAudioRequest {
    type Target = crate::game_logic::AudioEventRequest;
    fn deref(&self) -> &Self::Target {
        &self.request
    }
}
/// This adapter borrows only an immutable operation snapshot, never a World
/// hidden behind the native resolver's Send + Sync trait.
struct ReceivingLocalityFacts<'a>(Option<&'a LiveAudioLocality>);
impl game_engine::common::audio::AudioLocalityResolver for ReceivingLocalityFacts<'_> {
    fn get_local_player_index(&self) -> Option<i32> {
        self.0.map(|facts| facts.local_player_index)
    }
    fn get_observer_look_at_player_index(&self) -> Option<i32> {
        self.0.and_then(|facts| facts.observer_look_at)
    }
    fn is_player_active(&self, id: i32) -> bool {
        self.0
            .and_then(|facts| facts.players.get(&id))
            .is_some_and(|player| player.active)
    }
    fn player_exists(&self, id: i32) -> bool {
        self.0
            .and_then(|facts| facts.players.get(&id))
            .is_some_and(|player| player.exists)
    }
    fn has_default_team(&self, id: i32) -> bool {
        self.0
            .and_then(|facts| facts.players.get(&id))
            .is_some_and(|player| player.exists && player.has_default_team)
    }
    fn get_relationship_to_local_team(
        &self,
        source: i32,
        _local: i32,
    ) -> AudioLocalityRelationship {
        self.0
            .and_then(|facts| facts.players.get(&source))
            .map_or(AudioLocalityRelationship::Neutral, |player| {
                player.relationship_to_local
            })
    }
}

/// Freeze locality for a single event using the driving world's player facts.
pub(crate) fn receiving_submission_locality(
    owner: i32,
    facts: Option<&LiveAudioLocality>,
) -> game_engine::common::audio::AudioSubmissionLocality {
    game_engine::common::audio::AudioSubmissionLocality::from_resolver(
        owner,
        &ReceivingLocalityFacts(facts),
    )
}

fn lookup_audio_event_info(event_name: &str) -> Option<AudioEventInfo> {
    let manager = game_engine::common::audio::game_audio::get_global_audio_manager()?;
    let guard = manager.lock().ok()?;
    guard
        .find_audio_event_info(event_name)
        .map(|info| (*info).clone())
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
    snap: Option<&LiveAudioLocality>,
    cell_shroud_clear: &mut dyn FnMut(i32, glam::Vec3) -> bool,
) -> bool {
    if is_invented_unit_voice_name(event_name) || is_generic_eva_sfx_name(event_name) {
        return false;
    }

    let info = lookup_audio_event_info(event_name);
    should_dispatch_live_audio_with_info(
        event_name,
        owning_player_index,
        position,
        info.as_ref(),
        snap,
        cell_shroud_clear,
    )
}

fn should_dispatch_live_audio_with_info(
    event_name: &str,
    owning_player_index: Option<i32>,
    position: Option<glam::Vec3>,
    info: Option<&AudioEventInfo>,
    snap: Option<&LiveAudioLocality>,
    cell_shroud_clear: &mut dyn FnMut(i32, glam::Vec3) -> bool,
) -> bool {
    if is_invented_unit_voice_name(event_name) || is_generic_eva_sfx_name(event_name) {
        return false;
    }
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

    let owning = owning_player_index.or_else(|| None);
    let owning_exists = owning.is_some_and(|index| {
        snap.is_some_and(|facts| {
            facts
                .players
                .get(&index)
                .is_some_and(|player| player.exists)
        })
    });

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
                    .unwrap_or(false),
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

    // GameSounds.cpp203 uses the actual local-player index for cell shroud;
    // observer look-at substitution applies only to GameAudio.cpp988 relations.
    if let (Some(pos), Some(local)) = (position, snap.map(|facts| facts.local_player_index)) {
        let clear = cell_shroud_clear(local, pos);
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
    locality: Option<&LiveAudioLocality>,
) -> Option<i32> {
    let id = object_id?.0;
    locality?.object_owners.get(&id).copied()
}

/// True when this request may enter the live SFX drain.
pub(crate) fn admit_audio_request(
    event: crate::game_logic::AudioEventRequest,
    locality: Option<&LiveAudioLocality>,
    cell_shroud_clear: &mut dyn FnMut(i32, glam::Vec3) -> bool,
) -> Option<AdmittedAudioRequest> {
    admit_audio_request_with_info_lookup(
        event,
        locality,
        cell_shroud_clear,
        lookup_audio_event_info,
    )
}

/// Admit through an explicit receiver-owned metadata lookup. The lookup must
/// return an owned copy, so any service lock is released before locality and
/// shroud callbacks run.
pub(crate) fn admit_audio_request_with_info_lookup(
    event: crate::game_logic::AudioEventRequest,
    locality: Option<&LiveAudioLocality>,
    cell_shroud_clear: &mut dyn FnMut(i32, glam::Vec3) -> bool,
    lookup: impl FnOnce(&str) -> Option<AudioEventInfo>,
) -> Option<AdmittedAudioRequest> {
    let mut event = event;
    let owner = event
        .player_index
        .or_else(|| owning_player_for_audio_object(event.object_id, locality));
    // Preserve the resolved source player so backend admission never rediscovers
    // it through an ambient Core object registry with coincident IDs.
    if event.player_index.is_none() {
        event.player_index = owner;
    }
    let removal = event.stop
        || matches!(
            event.event_type.as_str(),
            "MusicEnable" | "MusicDisable" | "WeaponFireLoopStop" | "AfterburnerStop"
        );
    if !removal {
        if is_invented_unit_voice_name(event.event_type.as_str())
            || is_generic_eva_sfx_name(event.event_type.as_str())
        {
            return None;
        }
        // Lookup returns an owned metadata value. In the receiving path this
        // drops the subsystem's mutex guard before shouldPlayLocally queries
        // the world/shroud callback below.
        let info = lookup(event.event_type.as_str());
        if !should_dispatch_live_audio_with_info(
            event.event_type.as_str(),
            owner,
            event.position,
            info.as_ref(),
            locality,
            cell_shroud_clear,
        ) {
            return None;
        }
    }
    let captured = receiving_submission_locality(owner.unwrap_or(-1), locality);
    Some(AdmittedAudioRequest {
        request: event,
        locality: captured,
    })
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
            resolve_special_power_initiate_sound("SpySatellite", None).as_deref(),
            Some("TestSpySatInitiate")
        );
        clear_test_template_voices();
    }

    #[test]
    fn invented_unit_move_and_generic_eva_never_dispatch() {
        assert!(is_invented_unit_voice_name("UnitMove"));
        assert!(is_generic_eva_sfx_name("EVA_LowPower"));
        assert!(is_generic_eva_sfx_name("EVA_UpgradeComplete"));
        assert!(!is_generic_eva_sfx_name("EvaUSA_BuildingLost"));
        assert!(!should_dispatch_live_audio(
            "UnitMove",
            Some(0),
            None,
            None,
            &mut |_, _| false
        ));
        assert!(!should_dispatch_live_audio(
            "EVA_LowPower",
            Some(0),
            None,
            None,
            &mut |_, _| false
        ));
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

        // No AudioEventInfo → missing bits default Everyone.
        assert!(should_dispatch_live_audio(
            "SomeWorldBoom",
            Some(1),
            None,
            Some(&snap),
            &mut |_, _| false
        ));

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

#[cfg(test)]
mod receiving_audio_admission_tests {
    use super::*;
    use game_engine::common::audio::{ST_EVERYONE, ST_PLAYER};
    fn facts(local: i32, owner: i32) -> LiveAudioLocality {
        let mut result = LiveAudioLocality {
            local_player_index: local,
            local_player_active: true,
            ..Default::default()
        };
        for id in [local, owner] {
            result.players.insert(
                id,
                LiveAudioPlayer {
                    exists: true,
                    active: true,
                    has_default_team: true,
                    relationship_to_local: AudioLocalityRelationship::Allies,
                },
            );
        }
        result.object_owners.insert(41, owner);
        result
    }
    #[test]
    fn same_id_player_and_object_facts_are_receiving_not_last_published() {
        let mut info = AudioEventInfo::default();
        info.type_field = ST_PLAYER;
        let a = facts(0, 0);
        let mut b = facts(0, 1);
        let owner_a =
            owning_player_for_audio_object(Some(crate::game_logic::ObjectId(41)), Some(&a));
        let owner_b =
            owning_player_for_audio_object(Some(crate::game_logic::ObjectId(41)), Some(&b));
        assert!(should_dispatch_live_audio_with_info(
            "AuthoredVoice",
            owner_a,
            None,
            Some(&info),
            Some(&a),
            &mut |_, _| false
        ));
        assert!(!should_dispatch_live_audio_with_info(
            "AuthoredVoice",
            owner_b,
            None,
            Some(&info),
            Some(&b),
            &mut |_, _| false
        ));
        b.players.clear();
        drop(b);
        assert!(should_dispatch_live_audio_with_info(
            "AuthoredVoice",
            owner_a,
            None,
            Some(&info),
            Some(&a),
            &mut |_, _| false
        ));
    }
    #[test]
    fn shroud_query_uses_actual_receiver_and_original_local_not_observer() {
        let mut info = AudioEventInfo::default();
        info.type_field = ST_EVERYONE | ST_WORLD | ST_SHROUDED;
        let mut a = facts(0, 2);
        a.local_player_active = false;
        a.players.get_mut(&0).unwrap().active = false;
        a.observer_look_at = Some(2);
        let position = Some(glam::Vec3::new(3.0, 4.0, 5.0));
        let mut queried = Vec::new();
        assert!(should_dispatch_live_audio_with_info(
            "AuthoredWorld",
            Some(2),
            position,
            Some(&info),
            Some(&a),
            &mut |player, pos| {
                queried.push((player, pos));
                true
            }
        ));
        assert_eq!(queried, vec![(0, position.unwrap())]);
        assert!(!should_dispatch_live_audio_with_info(
            "AuthoredWorld",
            Some(2),
            position,
            Some(&info),
            Some(&a),
            &mut |_, _| false
        ));
    }
}
