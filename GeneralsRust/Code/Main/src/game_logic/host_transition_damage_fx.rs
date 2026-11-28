//! Host TransitionDamageFX residual (FX on body-damage state worsening).
//!
//! C++: `TransitionDamageFX::onBodyDamageStateChange` plays FXList / particles
//! when `IS_CONDITION_WORSE(new, old)` (new ordinal > old).
//! `ActiveBody::attemptDamage` plays template `SoundOnDamaged` /
//! `SoundOnReallyDamaged` (not invented BuildingDamaged / VehicleDamaged)
//! and 25% `VoiceFear` when health crosses `YELLOW_DAMAGE_PERCENT`.
//!
//! Residual playability slice:
//! - Detect Pristine→Damaged→ReallyDamaged→Rubble transitions
//! - Queue named FX/audio residual keys for presentation + audio
//! - Template peels for DamagedFXList / SoundOnDamaged / VoiceFear
//!
//! Fail-closed:
//! - Bone-local offsets resolved via leftover pristine bones when present
//! - Particle IDs tracked so previous-state systems are destroyed
//! - Leftover DamageFXTypes / DamageOCLTypes / DamageParticleTypes gate play

use crate::game_logic::ObjectId;
use generals_sim::object::BodyDamageType;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::cell::{Cell, RefCell};

/// C++ BODYDAMAGETYPE_COUNT residual (Pristine/Damaged/ReallyDamaged/Rubble).
pub const TRANSITION_DAMAGE_FX_SLOTS: usize = 4;
const MAX_TRANSITION_EFFECT_SLOTS: usize = 12;
#[path = "host_transition_damage_fx/transition_slots.rs"]
mod transition_slots;
use transition_slots::{TransitionSlotEffect, for_each_transition_slot};

fn default_leftover_damage_type_flags() -> u64 {
    generals_sim::damage::DAMAGE_TYPE_FLAGS_ALL.bits()
}

fn leftover_damage_type_flags(bits: u64) -> generals_sim::damage::DamageTypeFlags {
    generals_sim::damage::DamageTypeFlags::from_bits_truncate(bits)
}

/// C++ `lastDamageInfo == NULL || getDamageTypeFlag(mask, lastDamage)`.
pub fn leftover_should_play_for_damage_type(
    mask_bits: u64,
    last_damage: Option<generals_sim::damage::DamageType>,
) -> bool {
    match last_damage {
        Some(info) => {
            generals_sim::damage::get_damage_type_flag(leftover_damage_type_flags(mask_bits), info)
        }
        None => true,
    }
}

/// Leftover `parse_damage_type_flags` (starts empty; ALL/NONE/+/- names).
fn parse_leftover_damage_type_flags(raw: &str) -> Option<u64> {
    use std::str::FromStr;
    let tokens: Vec<&str> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let mut flags = generals_sim::damage::DamageTypeFlags::empty();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = generals_sim::damage::DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = generals_sim::damage::DamageTypeFlags::empty();
                continue;
            }
            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };
            if let Ok(damage_type) = generals_sim::damage::DamageType::from_str(name) {
                let flag =
                    generals_sim::damage::DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }
    Some(flags.bits())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostTransitionDamageFxData {
    /// First authored FXList residual keyed by body state ordinal (0..3).
    pub fx_for_state: [Option<String>; TRANSITION_DAMAGE_FX_SLOTS],
    /// Audio residual keyed by body state ordinal.
    pub audio_for_state: [Option<String>; TRANSITION_DAMAGE_FX_SLOTS],
    pub enabled: bool,
    /// C++ `m_fxList[state][0..12]` authored FXList names.
    #[serde(default)]
    pub fx_lists_for_state: [Vec<String>; TRANSITION_DAMAGE_FX_SLOTS],
    /// C++ `m_OCL[state][0..12]` authored OCL names.
    #[serde(default)]
    pub ocl_for_state: [Vec<String>; TRANSITION_DAMAGE_FX_SLOTS],
    /// Leftover `FXLocInfo` for each authored FXList slot.
    #[serde(default)]
    pub fx_locs_for_state: [Vec<HostTransitionLoc>; TRANSITION_DAMAGE_FX_SLOTS],
    /// Leftover `FXLocInfo` for each authored OCL slot.
    #[serde(default)]
    pub ocl_locs_for_state: [Vec<HostTransitionLoc>; TRANSITION_DAMAGE_FX_SLOTS],

    /// C++ `m_particleSystem[state][slot]` authored PSys names + loc.
    #[serde(default)]
    pub particles_for_state: [Vec<HostTransitionParticle>; TRANSITION_DAMAGE_FX_SLOTS],
    /// Live attached combat-particle ids per body state.
    #[serde(default)]
    pub attached_ids: [Vec<u32>; TRANSITION_DAMAGE_FX_SLOTS],
    /// C++ `m_damageFXTypes` leftover mask (ALL default).
    #[serde(default = "default_leftover_damage_type_flags")]
    pub damage_fx_types: u64,
    /// C++ `m_damageOCLTypes` leftover mask (ALL default).
    #[serde(default = "default_leftover_damage_type_flags")]
    pub damage_ocl_types: u64,
    /// C++ `m_damageParticleTypes` leftover mask (ALL default).
    #[serde(default = "default_leftover_damage_type_flags")]
    pub damage_particle_types: u64,
}

impl Default for HostTransitionDamageFxData {
    fn default() -> Self {
        Self {
            fx_for_state: Default::default(),
            audio_for_state: Default::default(),
            enabled: false,
            fx_lists_for_state: Default::default(),
            ocl_for_state: Default::default(),
            fx_locs_for_state: Default::default(),
            ocl_locs_for_state: Default::default(),

            particles_for_state: Default::default(),
            attached_ids: Default::default(),
            damage_fx_types: default_leftover_damage_type_flags(),
            damage_ocl_types: default_leftover_damage_type_flags(),
            damage_particle_types: default_leftover_damage_type_flags(),
        }
    }
}

/// C++ `FXDamageParticleSystemInfo` residual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HostTransitionParticle {
    pub name: String,
    pub bone: Option<String>,
    pub loc: [f32; 3],
    pub random_bone: bool,
}

/// Leftover `FXLocInfo` (Bone / RandomBone / Loc) for FXList and OCL slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HostTransitionLoc {
    pub bone: Option<String>,
    pub loc: [f32; 3],
    pub random_bone: bool,
}

impl HostTransitionLoc {
    fn as_particle(&self) -> HostTransitionParticle {
        HostTransitionParticle {
            name: String::new(),
            bone: self.bone.clone(),
            loc: self.loc,
            random_bone: self.random_bone,
        }
    }
}

impl HostTransitionDamageFxData {
    pub fn generic_structure_residual() -> Self {
        // C++ TransitionDamageFX plays only authored PSys slots. Inventing
        // BuildingDamageSmoke/Fire (not in ParticleSystem.ini) made every
        // structure billow generic smoke (hq-uzzbc). Audio is ActiveBody.
        Self {
            enabled: true,
            fx_for_state: [None, None, None, None],
            audio_for_state: [None, None, None, None],
            ..Self::default()
        }
    }

    pub fn toxic_bunker_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [None, None, None, None],
            audio_for_state: [None, None, None, None],
            ..Self::default()
        }
    }

    pub fn vehicle_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [None, None, None, None],
            audio_for_state: [None, None, None, None],
            ..Self::default()
        }
    }

    pub fn infantry_audio_residual() -> Self {
        Self {
            enabled: true,
            fx_for_state: [None, None, None, None],
            audio_for_state: [None, None, None, None],
            ..Self::default()
        }
    }

    pub fn take_attached_ids(&mut self, state_ordinal: u8) -> Vec<u32> {
        let idx = state_ordinal as usize;
        if idx >= TRANSITION_DAMAGE_FX_SLOTS {
            return Vec::new();
        }
        std::mem::take(&mut self.attached_ids[idx])
    }

    pub fn store_attached_ids(&mut self, state_ordinal: u8, ids: Vec<u32>) {
        let idx = state_ordinal as usize;
        if idx < TRANSITION_DAMAGE_FX_SLOTS {
            self.attached_ids[idx] = ids;
        }
    }
}

/// One transition residual event (presentation/audio consumers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HostTransitionDamageFxEvent {
    pub old_state: u8,
    pub new_state: u8,
    pub fx_name: Option<String>,
    pub audio_name: Option<String>,
    #[serde(default)]
    pub extra_fx_names: Vec<String>,
    #[serde(default)]
    pub ocl_names: Vec<String>,
    #[serde(default)]
    pub particles: Vec<HostTransitionParticle>,
    /// Leftover loc for `fx_name` then `extra_fx_names`.
    #[serde(default)]
    pub fx_locs: Vec<HostTransitionLoc>,
    /// Leftover loc for each `ocl_names` slot.
    #[serde(default)]
    pub ocl_locs: Vec<HostTransitionLoc>,

    #[serde(default)]
    pub clear_old_state: Option<u8>,
}

/// C++ IS_CONDITION_WORSE(a,b) := a > b (BodyDamageType ordinal).
pub fn is_condition_worse(new_state: BodyDamageType, old_state: BodyDamageType) -> bool {
    new_state.ordinal() > old_state.ordinal()
}

fn authored_fx_lists_for_state(data: &HostTransitionDamageFxData, idx: usize) -> Vec<String> {
    authored_fx_slots_for_state(data, idx).0
}

fn authored_fx_slots_for_state(
    data: &HostTransitionDamageFxData,
    idx: usize,
) -> (Vec<String>, Vec<HostTransitionLoc>) {
    let mut names = data.fx_lists_for_state[idx].clone();
    let mut locs = data.fx_locs_for_state[idx].clone();
    if names.is_empty() {
        if let Some(fx) = data.fx_for_state[idx].clone() {
            names.push(fx);
        }
    }
    if locs.len() < names.len() {
        locs.resize(names.len(), HostTransitionLoc::default());
    }
    (names, locs)
}

/// Build residual event when state worsens.
pub fn transition_event(
    data: &HostTransitionDamageFxData,
    old_state: BodyDamageType,
    new_state: BodyDamageType,
) -> Option<HostTransitionDamageFxEvent> {
    transition_event_for_damage(data, old_state, new_state, None)
}

pub fn transition_event_for_damage(
    data: &HostTransitionDamageFxData,
    old_state: BodyDamageType,
    new_state: BodyDamageType,
    last_damage: Option<generals_sim::damage::DamageType>,
) -> Option<HostTransitionDamageFxEvent> {
    on_body_damage_state_change_for_damage(data, old_state, new_state, last_damage).and_then(|ev| {
        if ev.fx_name.is_none()
            && ev.audio_name.is_none()
            && ev.extra_fx_names.is_empty()
            && ev.ocl_names.is_empty()
            && ev.particles.is_empty()
        {
            None
        } else {
            Some(ev)
        }
    })
}

/// C++ `TransitionDamageFX::onBodyDamageStateChange` — always destroy old
/// state's particle systems; create new ones only when `IS_CONDITION_WORSE`.
pub fn on_body_damage_state_change(
    data: &HostTransitionDamageFxData,
    old_state: BodyDamageType,
    new_state: BodyDamageType,
) -> Option<HostTransitionDamageFxEvent> {
    on_body_damage_state_change_for_damage(data, old_state, new_state, None)
}

pub fn on_body_damage_state_change_for_damage(
    data: &HostTransitionDamageFxData,
    old_state: BodyDamageType,
    new_state: BodyDamageType,
    last_damage: Option<generals_sim::damage::DamageType>,
) -> Option<HostTransitionDamageFxEvent> {
    if !data.enabled {
        return None;
    }
    let worse = is_condition_worse(new_state, old_state);
    let idx = new_state.ordinal() as usize;
    let play_fx = leftover_should_play_for_damage_type(data.damage_fx_types, last_damage);
    let play_ocl = leftover_should_play_for_damage_type(data.damage_ocl_types, last_damage);
    let play_psys = leftover_should_play_for_damage_type(data.damage_particle_types, last_damage);
    let (mut fx_lists, fx_locs, audio, ocl_names, ocl_locs, particles) =
        if worse && idx < TRANSITION_DAMAGE_FX_SLOTS {
            let (names, locs) = if play_fx {
                authored_fx_slots_for_state(data, idx)
            } else {
                (Vec::new(), Vec::new())
            };
            let (ocl_names, ocl_locs) = if play_ocl {
                (
                    data.ocl_for_state[idx].clone(),
                    data.ocl_locs_for_state[idx].clone(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            (
                names,
                locs,
                data.audio_for_state[idx].clone(),
                ocl_names,
                ocl_locs,
                if play_psys {
                    data.particles_for_state[idx].clone()
                } else {
                    Vec::new()
                },
            )
        } else {
            (
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };
    let fx = if fx_lists.is_empty() {
        None
    } else {
        Some(fx_lists.remove(0))
            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("none"))
    };
    if !worse && fx.is_none() && audio.is_none() && particles.is_empty() {
        return Some(HostTransitionDamageFxEvent {
            old_state: old_state.ordinal(),
            new_state: new_state.ordinal(),
            fx_name: None,
            audio_name: None,
            extra_fx_names: Vec::new(),
            ocl_names: Vec::new(),
            particles: Vec::new(),
            fx_locs: Vec::new(),
            ocl_locs: Vec::new(),
            clear_old_state: Some(old_state.ordinal()),
        });
    }
    if fx.is_none()
        && audio.is_none()
        && fx_lists.is_empty()
        && ocl_names.is_empty()
        && particles.is_empty()
        && !worse
    {
        return None;
    }
    Some(HostTransitionDamageFxEvent {
        old_state: old_state.ordinal(),
        new_state: new_state.ordinal(),
        fx_name: fx,
        audio_name: audio,
        extra_fx_names: fx_lists,
        ocl_names,
        particles,
        fx_locs,
        ocl_locs,
        clear_old_state: Some(old_state.ordinal()),
    })
}

pub(crate) fn transition_damage_fx_config_for_template(
    template: &crate::game_logic::ThingTemplate,
    is_structure: bool,
    is_vehicle: bool,
) -> Option<HostTransitionDamageFxData> {
    let n = template.name.to_ascii_lowercase();
    let fallback = || {
        if n.contains("toxic") && n.contains("bunker") {
            HostTransitionDamageFxData::toxic_bunker_residual()
        } else if is_structure {
            HostTransitionDamageFxData::generic_structure_residual()
        } else if is_vehicle
            || n.contains("tank")
            || n.contains("vehicle")
            || n.contains("truck")
            || n.contains("dozer")
        {
            HostTransitionDamageFxData::vehicle_residual()
        } else {
            // Infantry / everything else: still emit template SoundOnDamaged.
            HostTransitionDamageFxData::infantry_audio_residual()
        }
    };
    let mut data = template
        .transition_damage_definition
        .clone()
        .unwrap_or_else(fallback);
    if let Some(typed) = &template.receiving_client_template {
        for (state, sound) in [
            BodyDamageType::Damaged,
            BodyDamageType::ReallyDamaged,
        ]
        .into_iter()
        .zip(typed.damage_sounds())
        {
            data.audio_for_state[state.ordinal() as usize] = sound.map(str::to_owned);
        }
    }
    Some(data)
}

/// Freeze authored transition data when this template is admitted.
pub(crate) fn transition_damage_fx_config_for_definition(
    definition: &crate::assets::ObjectDefinition,
) -> HostTransitionDamageFxData {
    let mut data = HostTransitionDamageFxData::generic_structure_residual();
    for (state, field) in [
        (BodyDamageType::Damaged, "SoundOnDamaged"),
        (BodyDamageType::ReallyDamaged, "SoundOnReallyDamaged"),
    ] {
        data.audio_for_state[state.ordinal() as usize] = definition
            .attributes
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(field).then_some(value.as_str()))
            .and_then(|value| value.split_whitespace().next())
            .map(str::to_owned);
    }
    for module in &definition.behavior_modules {
        if module.class_name.eq_ignore_ascii_case("TransitionDamageFX") {
            overlay_authored_transition_module(&mut data, module);
        }
    }
    data
}

// Definition parsing is independent of the asset publication adapter above.
fn overlay_authored_transition_module(
    data: &mut HostTransitionDamageFxData,
    module: &crate::assets::ini_parser::BehaviorModuleDefinition,
) {
    for (state, prefix) in [
        (BodyDamageType::Damaged, "DamagedParticleSystem"),
        (
            BodyDamageType::ReallyDamaged,
            "ReallyDamagedParticleSystem",
        ),
        (BodyDamageType::Rubble, "RubbleParticleSystem"),
    ] {
        let idx = state.ordinal() as usize;
        let mut parsed = Vec::new();
        for slot in 1..=MAX_TRANSITION_EFFECT_SLOTS {
            let key = format!("{prefix}{slot}");
            if let Some(raw) = module.attribute(&key) {
                if let Some(p) = parse_transition_particle_attr(raw) {
                    parsed.resize(slot, HostTransitionParticle::default());
                    parsed[slot - 1] = p;
                }
            }
        }
        if !parsed.is_empty() {
            data.particles_for_state[idx] = parsed;
        }
    }
    for (state, prefix) in [
        (BodyDamageType::Damaged, "DamagedFXList"),
        (BodyDamageType::ReallyDamaged, "ReallyDamagedFXList"),
        (BodyDamageType::Rubble, "RubbleFXList"),
    ] {
        let idx = state.ordinal() as usize;
        let mut parsed = Vec::new();
        let mut parsed_locs = Vec::new();
        for slot in 1..=MAX_TRANSITION_EFFECT_SLOTS {
            let key = format!("{prefix}{slot}");
            if let Some(raw) = module.attribute(&key) {
                if let Some(name) = parse_transition_named_attr(raw, "fxlist") {
                    parsed.resize(slot, String::new());
                    parsed_locs.resize(slot, HostTransitionLoc::default());
                    parsed[slot - 1] = name;
                    parsed_locs[slot - 1] = parse_transition_loc_attr(raw);
                }
            }
        }
        if !parsed.is_empty() {
            data.fx_lists_for_state[idx] = parsed.clone();
            data.fx_locs_for_state[idx] = parsed_locs;
            data.fx_for_state[idx] = parsed.into_iter().find(|name| !name.is_empty());
        }
    }
    for (state, prefix) in [
        (BodyDamageType::Damaged, "DamagedOCL"),
        (BodyDamageType::ReallyDamaged, "ReallyDamagedOCL"),
        (BodyDamageType::Rubble, "RubbleOCL"),
    ] {
        let idx = state.ordinal() as usize;
        let mut parsed = Vec::new();
        let mut parsed_locs = Vec::new();
        for slot in 1..=MAX_TRANSITION_EFFECT_SLOTS {
            let key = format!("{prefix}{slot}");
            if let Some(raw) = module.attribute(&key) {
                if let Some(name) = parse_transition_named_attr(raw, "ocl") {
                    parsed.resize(slot, String::new());
                    parsed_locs.resize(slot, HostTransitionLoc::default());
                    parsed[slot - 1] = name;
                    parsed_locs[slot - 1] = parse_transition_loc_attr(raw);
                }
            }
        }
        if !parsed.is_empty() {
            data.ocl_for_state[idx] = parsed;
            data.ocl_locs_for_state[idx] = parsed_locs;
        }
    }

    if let Some(raw) = module.attribute("DamageFXTypes") {
        if let Some(bits) = parse_leftover_damage_type_flags(raw) {
            data.damage_fx_types = bits;
        }
    }
    if let Some(raw) = module.attribute("DamageOCLTypes") {
        if let Some(bits) = parse_leftover_damage_type_flags(raw) {
            data.damage_ocl_types = bits;
        }
    }
    if let Some(raw) = module.attribute("DamageParticleTypes") {
        if let Some(bits) = parse_leftover_damage_type_flags(raw) {
            data.damage_particle_types = bits;
        }
    }
}

/// `Bone:Name RandomBone:No FXList:Name` / `OCL:Name` / bare name.
pub fn parse_transition_named_attr(raw: &str, tag: &str) -> Option<String> {
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        let (key, val) = match tok.split_once(':') {
            Some((k, v)) => (k, Some(v)),
            None => (tok, None),
        };
        if key.eq_ignore_ascii_case(tag) {
            let v = if let Some(v) = val.filter(|s| !s.is_empty()) {
                v.to_string()
            } else {
                i += 1;
                tokens.get(i)?.to_string()
            };
            if !v.eq_ignore_ascii_case("none") {
                return Some(v);
            }
            return None;
        }
        i += 1;
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        None
    } else if !trimmed.contains(':') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Leftover `parse_fx_loc_info` for FXList / OCL / PSys loc tokens.
pub fn parse_transition_loc_attr(raw: &str) -> HostTransitionLoc {
    let p = parse_transition_particle_attr(&format!("{raw} PSys:_")).unwrap_or_default();
    HostTransitionLoc {
        bone: p.bone,
        loc: p.loc,
        random_bone: p.random_bone,
    }
}

/// Play positional FX for a detached Object. OCL creation is queued until a
/// receiving GameLogic owns the object and can execute it in that match.
pub(crate) fn dispatch_detached_transition_fx(
    object: &crate::game_logic::Object,
    event: &mut HostTransitionDamageFxEvent,
    mut dispatch: impl FnMut(&str, glam::Vec3),
) {
    let template = object.get_template();
    let model = template.model_name_for_conditions(object.model_condition_bits);
    let matrix = object.get_transform_matrix();
    for_each_transition_slot(event, |effect| {
        if let TransitionSlotEffect::Fx { name, loc, .. } = effect {
            let local = loc
                .map(|loc| {
                    leftover_to_host_local(detached_effect_pos(
                        &loc.as_particle(),
                        &model,
                        template.asset_scale,
                    ))
                })
                .unwrap_or_default();
            dispatch(name, matrix.transform_point3(local));
        }
    });
    event.fx_name = None;
    event.extra_fx_names.clear();
}

/// `Bone:Name RandomBone:No PSys:Template` or `Loc: X:0 Y:0 Z:0 PSys:Template`.
pub fn parse_transition_particle_attr(raw: &str) -> Option<HostTransitionParticle> {
    let mut bone = None;
    let mut random_bone = false;
    let mut loc = [0.0_f32, 0.0, 0.0];
    let mut name = None;
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        let (key, val) = match tok.split_once(':') {
            Some((k, v)) => (k, Some(v)),
            None => (tok, None),
        };
        if key.eq_ignore_ascii_case("bone") {
            let b = if let Some(v) = val.filter(|s| !s.is_empty()) {
                v.to_string()
            } else {
                i += 1;
                tokens.get(i)?.to_string()
            };
            bone = Some(b);
        } else if key.eq_ignore_ascii_case("randombone") {
            let v = val.map(|s| s.to_string()).or_else(|| {
                i += 1;
                tokens.get(i).map(|s| s.to_string())
            })?;
            random_bone = v.eq_ignore_ascii_case("yes") || v.eq_ignore_ascii_case("true");
        } else if key.eq_ignore_ascii_case("psys") {
            let v = if let Some(v) = val.filter(|s| !s.is_empty()) {
                v.to_string()
            } else {
                i += 1;
                tokens.get(i)?.to_string()
            };
            if !v.eq_ignore_ascii_case("none") {
                name = Some(v);
            }
        } else if key.eq_ignore_ascii_case("loc") || key.eq_ignore_ascii_case("x") {
            // Consume X:/Y:/Z: tokens.
            let mut start = i;
            if key.eq_ignore_ascii_case("loc") {
                start = i + 1;
            }
            for t in tokens.iter().skip(start) {
                if let Some((axis, num)) = t.split_once(':') {
                    if let Ok(v) = num.parse::<f32>() {
                        match axis.to_ascii_lowercase().as_str() {
                            "x" => loc[0] = v,
                            "y" => loc[1] = v,
                            "z" => loc[2] = v,
                            _ => {}
                        }
                    }
                }
            }
        }
        i += 1;
    }
    Some(HostTransitionParticle {
        name: name?,
        bone,
        loc,
        random_bone,
    })
}

fn leftover_to_host_local(pos: glam::Vec3) -> glam::Vec3 {
    glam::Vec3::new(pos.x, pos.z, pos.y)
}

/// Standalone asset query for detached objects without rendering services.
fn detached_effect_pos(particle: &HostTransitionParticle, model: &str, scale: f32) -> glam::Vec3 {
    let loc = glam::Vec3::new(particle.loc[0], particle.loc[1], particle.loc[2]);
    let Some(bone) = particle.bone.as_deref() else {
        return loc;
    };
    if model.is_empty() {
        return loc;
    }
    if !particle.random_bone {
        return gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, bone)
            .unwrap_or(loc);
    }
    let mut positions = Vec::new();
    for i in 1..=32 {
        let name = format!("{bone}{i:02}");
        match gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, &name) {
            Some(pos) => positions.push(pos),
            None => break,
        }
    }
    if positions.is_empty() {
        return loc;
    }
    let pick = gamelogic::common::game_logic_random_value(0, positions.len() as u32 - 1) as usize;
    positions[pick]
}

/// C++ `ActiveBody.cpp` `#define YELLOW_DAMAGE_PERCENT (0.25f)`.
pub const YELLOW_DAMAGE_PERCENT: f32 = 0.25;

#[cfg(test)]
struct TransitionDamageFxState {
    dispatched_armor_damage_fx: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
static TRANSITION_DAMAGE_FX_STATE: std::sync::LazyLock<TransitionDamageFxState> =
    std::sync::LazyLock::new(|| TransitionDamageFxState {
        dispatched_armor_damage_fx: std::sync::Mutex::new(Vec::new()),
    });

/// C++ `ActiveBody.cpp:625-631` yellow-health VoiceFear gate + 25% roll.
pub fn voice_fear_should_play(
    prev_health: f32,
    current_health: f32,
    max_health: f32,
    roll_0_99: i32,
) -> bool {
    if max_health <= 0.0 || current_health <= 0.0 {
        return false;
    }
    let prev_ratio = prev_health / max_health;
    let cur_ratio = current_health / max_health;
    prev_ratio > YELLOW_DAMAGE_PERCENT && cur_ratio < YELLOW_DAMAGE_PERCENT && roll_0_99 < 25
}

/// Draw the ActiveBody fear chance from the session that owns the damage
/// continuation.
pub fn take_voice_fear_roll_from_session(
    random: &game_engine::common::random_value::SessionRandom,
) -> i32 {
    random.next_int(0, 99)
}

/// Detached Object adapters have no session owner; retain their compatibility
/// draw separately from the receiving GameLogic path.
pub(crate) fn take_detached_voice_fear_roll() -> i32 {
    game_engine::common::random_value::get_game_logic_random_value(0, 99)
}

/// Best ArmorSet `DamageFX =` name for live flags (C++ `validateArmorAndDamageFX`).
pub fn find_best_armor_set_damage_fx(
    sets: &[crate::game_logic::HostArmorSet],
    flags: u8,
) -> Option<String> {
    generals_sim::armor::best_armor_set(sets, flags)
        .and_then(|set| set.damage_fx.clone())
        .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("none"))
}

fn host_to_ini_damage_type(
    host: crate::game_logic::combat::DamageType,
) -> game_engine::common::ini::ini_damage_fx::DamageType {
    use game_engine::common::ini::ini_damage_fx::DamageType as I;
    match host.to_store() {
        generals_sim::damage::DamageType::Explosion => I::Explosion,
        generals_sim::damage::DamageType::Crush => I::Crush,
        generals_sim::damage::DamageType::ArmorPiercing => I::ArmorPiercing,
        generals_sim::damage::DamageType::SmallArms => I::SmallArms,
        generals_sim::damage::DamageType::Gattling => I::Gattling,
        generals_sim::damage::DamageType::Radiation => I::Radiation,
        generals_sim::damage::DamageType::Flame => I::Flame,
        generals_sim::damage::DamageType::Laser => I::Laser,
        generals_sim::damage::DamageType::Sniper => I::Sniper,
        generals_sim::damage::DamageType::Poison => I::Poison,
        generals_sim::damage::DamageType::Healing => I::Healing,
        generals_sim::damage::DamageType::Unresistable => I::Unresistable,
        generals_sim::damage::DamageType::Water => I::Water,
        generals_sim::damage::DamageType::Deploy => I::Deploy,
        generals_sim::damage::DamageType::Surrender => I::Surrender,
        generals_sim::damage::DamageType::Hack => I::Hack,
        generals_sim::damage::DamageType::KillPilot => I::KillPilot,
        generals_sim::damage::DamageType::Penalty => I::Penalty,
        generals_sim::damage::DamageType::Falling => I::Falling,
        generals_sim::damage::DamageType::Melee => I::Melee,
        generals_sim::damage::DamageType::Disarm => I::Disarm,
        generals_sim::damage::DamageType::HazardCleanup => I::HazardCleanup,
        generals_sim::damage::DamageType::ParticleBeam => I::ParticleBeam,
        generals_sim::damage::DamageType::Toppling => I::Toppling,
        generals_sim::damage::DamageType::InfantryMissile => I::InfantryMissile,
        generals_sim::damage::DamageType::AuroraBomb => I::AuroraBomb,
        generals_sim::damage::DamageType::LandMine => I::LandMine,
        generals_sim::damage::DamageType::JetMissiles => I::JetMissiles,
        generals_sim::damage::DamageType::StealthJetMissiles => I::StealthJetMissiles,
        generals_sim::damage::DamageType::MolotovCocktail => I::MolotovCocktail,
        generals_sim::damage::DamageType::ComancheVulcan => I::ComancheVulcan,
        generals_sim::damage::DamageType::SubdualMissile => I::SubdualMissile,
        generals_sim::damage::DamageType::SubdualVehicle => I::SubdualVehicle,
        generals_sim::damage::DamageType::SubdualBuilding => I::SubdualBuilding,
        generals_sim::damage::DamageType::SubdualUnresistable => I::SubdualUnresistable,
        generals_sim::damage::DamageType::Microwave => I::Microwave,
        generals_sim::damage::DamageType::KillGarrisoned => I::KillGarrisoned,
        generals_sim::damage::DamageType::Status => I::Status,
        generals_sim::damage::DamageType::DamageNumTypes => I::Unresistable,
    }
}

/// C++ DamageFX source/victim snapshot (`getVeterancyLevel` for throttle/list).
#[derive(Debug, Clone)]
pub struct HostDamageFxVictim {
    pub name: String,
    pub id: u32,
    pub vet: usize,
    /// Host Y-up world position (C++ `damageSource->getPosition()`).
    pub pos: glam::Vec3,
}

/// C++ `source ? source->getVeterancyLevel() : LEVEL_REGULAR` (Rookie = 0).
pub fn veterancy_to_damage_fx_level(level: crate::game_logic::VeterancyLevel) -> usize {
    match level {
        crate::game_logic::VeterancyLevel::Rookie => 0,
        crate::game_logic::VeterancyLevel::Veteran => 1,
        crate::game_logic::VeterancyLevel::Elite => 2,
        crate::game_logic::VeterancyLevel::Heroic => 3,
    }
}

pub fn snapshot_damage_fx_source(obj: &crate::game_logic::Object) -> HostDamageFxVictim {
    HostDamageFxVictim {
        name: obj.template_name.clone(),
        id: obj.id.0,
        vet: veterancy_to_damage_fx_level(obj.experience.level),
        pos: obj.get_position(),
    }
}

impl game_engine::common::ini::ini_damage_fx::Object for HostDamageFxVictim {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_id(&self) -> u32 {
        self.id
    }
    fn get_veterancy_level(&self) -> usize {
        self.vet
    }
}

/// C++ `ActiveBody::doDamageFX` after `attemptDamage` / `attemptHealing`.
/// Always runs even when `actual_damage == 0` (throttle bookkeeping).
/// `getDamageFXList` still returns no FXList for 0-amount (DamageFX.cpp:84-85).
pub fn dispatch_armor_damage_fx(
    obj: &mut crate::game_logic::Object,
    damage_type: crate::game_logic::combat::DamageType,
    actual_damage: f32,
    now: u32,
    source: Option<&HostDamageFxVictim>,
) -> Option<String> {
    dispatch_armor_damage_fx_with_dispatch(
        obj,
        damage_type,
        actual_damage,
        now,
        source,
        &mut dispatch_standalone_armor_fx,
    )
}

pub(crate) fn dispatch_owned_armor_damage_fx(
    obj: &mut crate::game_logic::Object,
    damage_type: crate::game_logic::combat::DamageType,
    actual_damage: f32,
    now: u32,
    source: Option<&HostDamageFxVictim>,
    store: &game_engine::common::ini::ini_damage_fx::DamageFXStore,
) -> Option<String> {
    dispatch_armor_damage_fx_with_store(
        obj,
        damage_type,
        actual_damage,
        now,
        source,
        store,
        &mut dispatch_standalone_armor_fx,
    )
}

fn dispatch_standalone_armor_fx(
    obj: &mut crate::game_logic::Object,
    name: &str,
    source: Option<&HostDamageFxVictim>,
) {
    // Standalone compatibility retains its registered Common runtime
    // and fallback, but neither definition guard spans this callback.
    crate::game_logic::publish_host_fx_object_ex(
        obj.id.0,
        obj.get_position(),
        obj.get_orientation(),
        obj.owner_player_id.map(|p| p as i32).unwrap_or(-1),
        crate::game_logic::host_supply_gather::host_bounding_circle_radius(
            (*obj.geometry_info()).authored,
            (*obj.geometry_info()).bounding_circle_radius(),
            obj.thing().geometry.radius.max(obj.selection_radius),
        ),
    );
    let victim = snapshot_damage_fx_source(obj);
    game_engine::common::ini::ini_damage_fx::DamageFX::execute_selected_fx_list(
        name,
        source.map(|value| value as &dyn game_engine::common::ini::ini_damage_fx::Object),
        Some(&victim),
    );
}

/// Select from immutable authored definitions, commit the original throttle,
/// then execute one borrowed selected-list callback with no catalog guard.
/// The callback receives the live victim to support synchronous reentry.
pub(crate) fn dispatch_armor_damage_fx_with_dispatch(
    obj: &mut crate::game_logic::Object,
    damage_type: crate::game_logic::combat::DamageType,
    actual_damage: f32,
    now: u32,
    source: Option<&HostDamageFxVictim>,
    dispatch: &mut dyn FnMut(&mut crate::game_logic::Object, &str, Option<&HostDamageFxVictim>),
) -> Option<String> {
    game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
    let selected = {
        let store = game_engine::common::ini::ini_damage_fx::get_damage_fx_store()?;
        select_armor_damage_fx(obj, damage_type, actual_damage, now, source, &store)?
    };
    finish_armor_damage_fx(obj, source, dispatch, selected)
}

pub(crate) fn dispatch_armor_damage_fx_with_store(
    obj: &mut crate::game_logic::Object,
    damage_type: crate::game_logic::combat::DamageType,
    actual_damage: f32,
    now: u32,
    source: Option<&HostDamageFxVictim>,
    store: &game_engine::common::ini::ini_damage_fx::DamageFXStore,
    dispatch: &mut dyn FnMut(&mut crate::game_logic::Object, &str, Option<&HostDamageFxVictim>),
) -> Option<String> {
    let selected = select_armor_damage_fx(obj, damage_type, actual_damage, now, source, store)?;
    finish_armor_damage_fx(obj, source, dispatch, selected)
}

fn select_armor_damage_fx(
    obj: &mut crate::game_logic::Object,
    damage_type: crate::game_logic::combat::DamageType,
    actual_damage: f32,
    now: u32,
    source: Option<&HostDamageFxVictim>,
    store: &game_engine::common::ini::ini_damage_fx::DamageFXStore,
) -> Option<(String, Option<String>)> {
    let flags = crate::game_logic::armor::live_armor_set_flags(obj);
    let dfx_name = find_best_armor_set_damage_fx(&obj.thing().template.armor_sets, flags)?;
    if obj.last_damage_fx_done == Some(damage_type) && now < obj.next_damage_fx_time {
        return None;
    }
    let ini_dt = host_to_ini_damage_type(damage_type);
    let source_ref =
        source.map(|value| value as &dyn game_engine::common::ini::ini_damage_fx::Object);
    let (list_name, throttle) = {
        let dfx = store.find_damage_fx(&dfx_name)?;
        (
            dfx.get_damage_fx_list(ini_dt, actual_damage, source_ref),
            dfx.get_damage_fx_throttle_time(ini_dt, source_ref),
        )
    };
    // ActiveBody.cpp:312-315 commits BEFORE doDamageFX. C++ UnsignedInt
    // addition wraps; saturating here would change near-wrap expiry behavior.
    obj.last_damage_fx_done = Some(damage_type);
    obj.next_damage_fx_time = now.wrapping_add(throttle);
    Some((dfx_name, list_name))
}

fn finish_armor_damage_fx(
    obj: &mut crate::game_logic::Object,
    source: Option<&HostDamageFxVictim>,
    dispatch: &mut dyn FnMut(&mut crate::game_logic::Object, &str, Option<&HostDamageFxVictim>),
    (dfx_name, list_name): (String, Option<String>),
) -> Option<String> {
    if let Some(name) = list_name.as_deref() {
        dispatch(obj, name, source);
    }
    #[cfg(test)]
    {
        TRANSITION_DAMAGE_FX_STATE.dispatched_armor_damage_fx.lock().unwrap_or_else(|p| p.into_inner()).push(dfx_name.clone());
        if let Some(list) = &list_name {
            TRANSITION_DAMAGE_FX_STATE.dispatched_armor_damage_fx.lock().unwrap_or_else(|p| p.into_inner()).push(list.clone());
        }
    }
    list_name.or(Some(dfx_name))
}

#[cfg(test)]
pub fn take_dispatched_armor_damage_fx() -> Vec<String> {
    std::mem::take(&mut *TRANSITION_DAMAGE_FX_STATE.dispatched_armor_damage_fx.lock().unwrap_or_else(|p| p.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use generals_sim::object::BodyDamageType;

    #[test]
    fn worse_transition_does_not_invent_structure_smoke() {
        let d = HostTransitionDamageFxData::generic_structure_residual();
        assert!(
            transition_event(
                &d,
                BodyDamageType::Damaged,
                BodyDamageType::Pristine
            )
            .is_none()
        );
        // Unauthored TransitionDamageFX: no invented BuildingDamageSmoke/Fire.
        assert!(
            transition_event(
                &d,
                BodyDamageType::Pristine,
                BodyDamageType::Damaged,
            )
            .is_none(),
            "must not invent BuildingDamageSmoke for templates without authored PSys"
        );
    }

    #[test]
    fn authored_transition_particles_still_emit() {
        let mut d = HostTransitionDamageFxData::generic_structure_residual();
        d.particles_for_state[BodyDamageType::Damaged.ordinal() as usize] =
            vec![HostTransitionParticle {
                name: "AuthoredDamagedPSys".into(),
                bone: Some("Smoke01".into()),
                loc: [0.0, 0.0, 0.0],
                random_bone: false,
            }];
        let e = transition_event(
            &d,
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
        )
        .expect("authored");
        assert_eq!(e.particles.len(), 1);
        assert_eq!(e.particles[0].name, "AuthoredDamagedPSys");
        assert_eq!(e.clear_old_state, Some(0));
    }

    #[test]
    fn heal_clears_old_state_particles() {
        let d = HostTransitionDamageFxData::generic_structure_residual();
        let e = on_body_damage_state_change(
            &d,
            BodyDamageType::ReallyDamaged,
            BodyDamageType::Damaged,
        )
        .expect("heal still destroys old PSys");
        assert!(e.particles.is_empty());
        assert_eq!(
            e.clear_old_state,
            Some(BodyDamageType::ReallyDamaged.ordinal())
        );
    }

    #[test]
    fn parse_damaged_particle_bone_psys() {
        let p = parse_transition_particle_attr("Bone:Fire01 RandomBone:No PSys:BuildingDamageFire")
            .expect("parse");
        assert_eq!(p.name, "BuildingDamageFire");
        assert_eq!(p.bone.as_deref(), Some("Fire01"));
        assert!(!p.random_bone);
    }

    #[test]
    fn parse_authored_fxlist_and_ocl_slots() {
        assert_eq!(
            parse_transition_named_attr(
                "Bone:FXBone01 RandomBone:No FXList:FX_AuthoredDamaged",
                "fxlist"
            )
            .as_deref(),
            Some("FX_AuthoredDamaged")
        );
        assert_eq!(
            parse_transition_named_attr("Loc: X:0 Y:0 Z:8 OCL:OCL_AuthoredDebris", "ocl")
                .as_deref(),
            Some("OCL_AuthoredDebris")
        );
    }

    #[test]
    fn authored_damaged_fxlist_and_ocl_play_not_invented_names() {
        let mut d = HostTransitionDamageFxData::vehicle_residual();
        let damaged = BodyDamageType::Damaged.ordinal() as usize;
        let really = BodyDamageType::ReallyDamaged.ordinal() as usize;
        d.fx_lists_for_state[damaged] = vec!["FX_AuthoredDamaged".into()];
        d.ocl_for_state[damaged] = vec!["OCL_AuthoredDebris".into()];
        d.fx_lists_for_state[really] = vec!["FX_AuthoredReallyDamaged".into()];
        d.ocl_for_state[really] = vec!["OCL_AuthoredReallyDebris".into()];
        let damaged_ev = transition_event(
            &d,
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
        )
        .expect("authored damaged");
        assert_eq!(damaged_ev.fx_name.as_deref(), Some("FX_AuthoredDamaged"));
        assert_eq!(damaged_ev.ocl_names, vec!["OCL_AuthoredDebris".to_string()]);
        assert_ne!(
            damaged_ev.fx_name.as_deref(),
            Some("FX_VehicleDamagedTransition")
        );
        let really_ev = transition_event(
            &d,
            BodyDamageType::Damaged,
            BodyDamageType::ReallyDamaged,
        )
        .expect("authored really damaged");
        assert_eq!(
            really_ev.fx_name.as_deref(),
            Some("FX_AuthoredReallyDamaged")
        );
        assert_eq!(
            really_ev.ocl_names,
            vec!["OCL_AuthoredReallyDebris".to_string()]
        );
        assert_ne!(
            really_ev.fx_name.as_deref(),
            Some("FX_VehicleReallyDamagedTransition")
        );
    }
    #[test]
    fn template_sound_on_damaged_not_invented_names() {
        // C++ ActiveBody.cpp:605-621 getSoundOnDamaged / getSoundOnReallyDamaged.
        let mut definition = crate::assets::ObjectDefinition::new("AmericaInfantryRanger".into());
        definition
            .attributes
            .insert("SoundOnDamaged".into(), "RangerVoiceDamaged".into());
        definition.attributes.insert(
            "SoundOnReallyDamaged".into(),
            "RangerVoiceReallyDamaged".into(),
        );
        let cfg = transition_damage_fx_config_for_definition(&definition);
        let e = transition_event(
            &cfg,
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
        )
        .expect("template audio");
        assert_eq!(e.audio_name.as_deref(), Some("RangerVoiceDamaged"));
        assert_ne!(e.audio_name.as_deref(), Some("BuildingDamaged"));
        assert_ne!(e.audio_name.as_deref(), Some("VehicleDamaged"));
    }

    #[test]
    fn voice_fear_yellow_25_percent() {
        // C++ ActiveBody.cpp:624-631 YELLOW_DAMAGE_PERCENT + GameLogicRandomValue < 25.
        assert!(voice_fear_should_play(40.0, 20.0, 100.0, 0));
        assert!(voice_fear_should_play(40.0, 20.0, 100.0, 24));
        assert!(!voice_fear_should_play(40.0, 20.0, 100.0, 25));
        assert!(!voice_fear_should_play(20.0, 10.0, 100.0, 0));
        assert!(!voice_fear_should_play(80.0, 40.0, 100.0, 0));
        assert!(!voice_fear_should_play(40.0, 0.0, 100.0, 0));
    }

    #[test]
    fn leftover_damage_fx_types_gate_fx_not_ocl() {
        let mut d = HostTransitionDamageFxData::vehicle_residual();
        let damaged = BodyDamageType::Damaged.ordinal() as usize;
        d.fx_lists_for_state[damaged] = vec!["FX_AuthoredDamaged".into()];
        d.ocl_for_state[damaged] = vec!["OCL_AuthoredDebris".into()];
        d.particles_for_state[damaged] = vec![HostTransitionParticle {
            name: "BuildingDamageSmoke".into(),
            bone: Some("Smoke01".into()),
            loc: [0.0, 0.0, 8.0],
            random_bone: false,
        }];
        d.damage_fx_types = parse_leftover_damage_type_flags("FLAME").expect("flame mask");
        d.damage_ocl_types = default_leftover_damage_type_flags();
        d.damage_particle_types = parse_leftover_damage_type_flags("FLAME").expect("flame mask");
        let explosion = transition_event_for_damage(
            &d,
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
            Some(generals_sim::damage::DamageType::Explosion),
        )
        .expect("ocl still plays");
        assert!(explosion.fx_name.is_none());
        assert!(explosion.particles.is_empty());
        assert_eq!(explosion.ocl_names, vec!["OCL_AuthoredDebris".to_string()]);
        let flame = transition_event_for_damage(
            &d,
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
            Some(generals_sim::damage::DamageType::Flame),
        )
        .expect("flame plays fx");
        assert_eq!(flame.fx_name.as_deref(), Some("FX_AuthoredDamaged"));
        assert_eq!(flame.particles.len(), 1);
        assert_eq!(flame.ocl_names, vec!["OCL_AuthoredDebris".to_string()]);
    }

    #[test]
    fn leftover_loc_swaps_z_up_onto_host_y_up() {
        let p = HostTransitionParticle {
            name: "BuildingDamageSmoke".into(),
            bone: None,
            loc: [1.0, 2.0, 8.0],
            random_bone: false,
        };
        let local = detached_effect_pos(&p, "", 1.0);
        assert_eq!(local, glam::Vec3::new(1.0, 2.0, 8.0));
        assert_eq!(leftover_to_host_local(local), glam::Vec3::new(1.0, 8.0, 2.0));
    }

    #[test]
    fn leftover_fxlist_and_ocl_loc_info_is_parsed() {
        let fx = parse_transition_loc_attr("Bone:FXBone01 RandomBone:No FXList:FX_AuthoredDamaged");
        assert_eq!(fx.bone.as_deref(), Some("FXBone01"));
        assert!(!fx.random_bone);
        let ocl = parse_transition_loc_attr("Loc: X:0 Y:0 Z:8 OCL:OCL_AuthoredDebris");
        assert!(ocl.bone.is_none());
        assert_eq!(ocl.loc, [0.0, 0.0, 8.0]);
    }

    #[test]
    fn leftover_get_local_effect_pos_used_for_fx_and_ocl() {
        let mut d = HostTransitionDamageFxData::vehicle_residual();
        let damaged = BodyDamageType::Damaged.ordinal() as usize;
        d.fx_lists_for_state[damaged] = vec!["FX_AuthoredDamaged".into()];
        d.fx_locs_for_state[damaged] = vec![HostTransitionLoc {
            bone: None,
            loc: [1.0, 2.0, 8.0],
            random_bone: false,
        }];
        d.ocl_for_state[damaged] = vec!["OCL_AuthoredDebris".into()];
        d.ocl_locs_for_state[damaged] = vec![HostTransitionLoc {
            bone: None,
            loc: [3.0, 4.0, 5.0],
            random_bone: false,
        }];
        let mut ev = transition_event(
            &d,
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
        )
        .expect("authored loc");
        assert_eq!(ev.fx_name.as_deref(), Some("FX_AuthoredDamaged"));
        assert_eq!(ev.fx_locs.len(), 1);
        assert_eq!(ev.fx_locs[0].loc, [1.0, 2.0, 8.0]);
        assert_eq!(ev.ocl_locs[0].loc, [3.0, 4.0, 5.0]);
        let mut template = crate::game_logic::ThingTemplate::new("TransitionPose");
        template.set_health(100.0);
        let mut object = crate::game_logic::Object::new(
            template,
            crate::game_logic::ObjectId(1),
            crate::game_logic::Team::USA,
        );
        object.set_position(glam::Vec3::new(10.0, 0.0, 4.0));
        let mut observed = Vec::new();
        dispatch_detached_transition_fx(&object, &mut ev, |name, world| {
            observed.push((name.to_owned(), world));
        });
        assert_eq!(observed, [("FX_AuthoredDamaged".into(), glam::Vec3::new(11.0, 8.0, 6.0))]);
    }
}

#[cfg(test)]
#[path = "host_transition_damage_fx/armor_selection_tests.rs"]
mod armor_selection_tests;
