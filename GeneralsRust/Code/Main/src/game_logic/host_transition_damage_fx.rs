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
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// C++ BODYDAMAGETYPE_COUNT residual (Pristine/Damaged/ReallyDamaged/Rubble).
pub const TRANSITION_DAMAGE_FX_SLOTS: usize = 4;

fn default_leftover_damage_type_flags() -> u64 {
    gamelogic::damage::DAMAGE_TYPE_FLAGS_ALL.bits()
}

fn leftover_damage_type_flags(bits: u64) -> gamelogic::damage::DamageTypeFlags {
    gamelogic::damage::DamageTypeFlags::from_bits_truncate(bits)
}

/// C++ `lastDamageInfo == NULL || getDamageTypeFlag(mask, lastDamage)`.
pub fn leftover_should_play_for_damage_type(
    mask_bits: u64,
    last_damage: Option<gamelogic::damage::DamageType>,
) -> bool {
    match last_damage {
        Some(info) => {
            gamelogic::damage::get_damage_type_flag(leftover_damage_type_flags(mask_bits), info)
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
    let mut flags = gamelogic::damage::DamageTypeFlags::empty();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = gamelogic::damage::DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = gamelogic::damage::DamageTypeFlags::empty();
                continue;
            }
            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };
            if let Ok(damage_type) = gamelogic::damage::DamageType::from_str(name) {
                let flag =
                    gamelogic::damage::DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
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

    fn overlay_template_audio(&mut self, name: &str) {
        let audio = lookup_template_damage_audio(name);
        if let Some(s) = audio.sound_on_damaged {
            self.audio_for_state[HostBodyDamageType::Damaged.ordinal() as usize] = Some(s);
        }
        if let Some(s) = audio.sound_on_really_damaged {
            self.audio_for_state[HostBodyDamageType::ReallyDamaged.ordinal() as usize] = Some(s);
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
pub fn is_condition_worse(new_state: HostBodyDamageType, old_state: HostBodyDamageType) -> bool {
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
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
) -> Option<HostTransitionDamageFxEvent> {
    transition_event_for_damage(data, old_state, new_state, None)
}

pub fn transition_event_for_damage(
    data: &HostTransitionDamageFxData,
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
    last_damage: Option<gamelogic::damage::DamageType>,
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
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
) -> Option<HostTransitionDamageFxEvent> {
    on_body_damage_state_change_for_damage(data, old_state, new_state, None)
}

pub fn on_body_damage_state_change_for_damage(
    data: &HostTransitionDamageFxData,
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
    last_damage: Option<gamelogic::damage::DamageType>,
) -> Option<HostTransitionDamageFxEvent> {
    if !data.enabled || old_state == new_state {
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

pub fn transition_damage_fx_config_for_template(
    name: &str,
    is_structure: bool,
    is_vehicle: bool,
) -> Option<HostTransitionDamageFxData> {
    let n = name.to_ascii_lowercase();
    let mut data = if n.contains("toxic") && n.contains("bunker") {
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
    };
    data.overlay_template_audio(name);
    overlay_authored_transition_slots(&mut data, name);
    Some(data)
}

fn overlay_authored_transition_slots(data: &mut HostTransitionDamageFxData, name: &str) {
    let Some(manager) = crate::assets::get_asset_manager() else {
        return;
    };
    let Ok(manager) = manager.lock() else {
        return;
    };
    let Some(definition) = manager.get_object_definition(name) else {
        return;
    };
    for module in &definition.behavior_modules {
        if !module.class_name.eq_ignore_ascii_case("TransitionDamageFX") {
            continue;
        }
        for (state, prefix) in [
            (HostBodyDamageType::Damaged, "DamagedParticleSystem"),
            (
                HostBodyDamageType::ReallyDamaged,
                "ReallyDamagedParticleSystem",
            ),
            (HostBodyDamageType::Rubble, "RubbleParticleSystem"),
        ] {
            let idx = state.ordinal() as usize;
            let mut parsed = Vec::new();
            for slot in 1..=12 {
                let key = format!("{prefix}{slot}");
                if let Some(raw) = module.attribute(&key) {
                    if let Some(p) = parse_transition_particle_attr(raw) {
                        parsed.push(p);
                    }
                }
            }
            if !parsed.is_empty() {
                data.particles_for_state[idx] = parsed;
            }
        }
        for (state, prefix) in [
            (HostBodyDamageType::Damaged, "DamagedFXList"),
            (HostBodyDamageType::ReallyDamaged, "ReallyDamagedFXList"),
            (HostBodyDamageType::Rubble, "RubbleFXList"),
        ] {
            let idx = state.ordinal() as usize;
            let mut parsed = Vec::new();
            let mut parsed_locs = Vec::new();
            for slot in 1..=12 {
                let key = format!("{prefix}{slot}");
                if let Some(raw) = module.attribute(&key) {
                    if let Some(name) = parse_transition_named_attr(raw, "fxlist") {
                        parsed.push(name);
                        parsed_locs.push(parse_transition_loc_attr(raw));
                    }
                }
            }
            if !parsed.is_empty() {
                data.fx_lists_for_state[idx] = parsed.clone();
                data.fx_locs_for_state[idx] = parsed_locs;
                data.fx_for_state[idx] = parsed.into_iter().next();
            }
        }
        for (state, prefix) in [
            (HostBodyDamageType::Damaged, "DamagedOCL"),
            (HostBodyDamageType::ReallyDamaged, "ReallyDamagedOCL"),
            (HostBodyDamageType::Rubble, "RubbleOCL"),
        ] {
            let idx = state.ordinal() as usize;
            let mut parsed = Vec::new();
            let mut parsed_locs = Vec::new();
            for slot in 1..=12 {
                let key = format!("{prefix}{slot}");
                if let Some(raw) = module.attribute(&key) {
                    if let Some(name) = parse_transition_named_attr(raw, "ocl") {
                        parsed.push(name);
                        parsed_locs.push(parse_transition_loc_attr(raw));
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

/// Leftover `get_local_effect_pos` + `convert_bone_pos_to_world_pos` in host space.
pub fn leftover_named_slot_world_pos(
    loc: Option<&HostTransitionLoc>,
    owner: u32,
    host_pos: glam::Vec3,
    yaw: f32,
    model: &str,
    scale: f32,
) -> glam::Vec3 {
    let loc = loc.cloned().unwrap_or_default();
    let particle = loc.as_particle();
    let leftover_owner = gamelogic::helpers::TheGameLogic::find_object_by_id(owner);
    let leftover_guard = leftover_owner.as_ref().and_then(|h| h.read().ok());
    let leftover_yaw = leftover_guard
        .as_ref()
        .map(|obj| obj.get_orientation())
        .unwrap_or(yaw);
    let leftover_local = {
        let leftover_drawable_handle = leftover_guard.as_ref().and_then(|obj| obj.get_drawable());
        let leftover_drawable = leftover_drawable_handle
            .as_ref()
            .and_then(|d| d.read().ok());
        leftover_local_effect_pos_live(&particle, leftover_drawable.as_deref(), model, scale)
    };
    if let Some(obj) = leftover_guard.as_deref() {
        let world = obj.convert_bone_pos_to_world_pos(Some(&leftover_local), None);
        let translation = world.w_axis;
        return leftover_to_host_local(gamelogic::common::Coord3D::new(
            translation.x,
            translation.y,
            translation.z,
        ));
    }
    let host_local = leftover_to_host_local(leftover_local);
    let (sin, cos) = leftover_yaw.sin_cos();
    glam::Vec3::new(
        host_pos.x + host_local.x * cos - host_local.z * sin,
        host_pos.y + host_local.y,
        host_pos.z + host_local.x * sin + host_local.z * cos,
    )
}

/// Leftover `play_fx_for_state` FXList/OCL: `doFXPos` / OCL create at bone/loc world pos.
pub fn play_transition_event_fx_ocl(
    ev: &HostTransitionDamageFxEvent,
    owner: u32,
    host_pos: glam::Vec3,
    yaw: f32,
    model: &str,
    scale: f32,
) {
    if let Some(fx) = ev.fx_name.as_deref() {
        let world =
            leftover_named_slot_world_pos(ev.fx_locs.first(), owner, host_pos, yaw, model, scale);
        let _ = crate::game_logic::dispatch_fx_list_at_pos(fx, world);
    }
    for (i, fx) in ev.extra_fx_names.iter().enumerate() {
        let world = leftover_named_slot_world_pos(
            ev.fx_locs.get(i + 1),
            owner,
            host_pos,
            yaw,
            model,
            scale,
        );
        let _ = crate::game_logic::dispatch_fx_list_at_pos(fx, world);
    }
    for (i, ocl) in ev.ocl_names.iter().enumerate() {
        let world =
            leftover_named_slot_world_pos(ev.ocl_locs.get(i), owner, host_pos, yaw, model, scale);
        play_authored_transition_ocl(ocl, owner, world);
    }
}

/// Play leftover FX/OCL then drop names so origin-dispatch consumers do not replay.
pub fn take_played_transition_event_fx_ocl(
    ev: &mut HostTransitionDamageFxEvent,
    owner: u32,
    host_pos: glam::Vec3,
    yaw: f32,
    model: &str,
    scale: f32,
) {
    play_transition_event_fx_ocl(ev, owner, host_pos, yaw, model, scale);
    ev.fx_name = None;
    ev.extra_fx_names.clear();
    ev.ocl_names.clear();
}

/// Play leftover OCL at a live-host pose (C++ `ObjectCreationList::create`).
///
/// C++ `TransitionDamageFX.cpp:354-355` / leftover `play_fx_for_state`:
/// secondary is `damageSource->getPosition()`, falling back to the bone/loc
/// world pos when no source is snapshotted (`peek_damage_fx_source`).
pub fn play_authored_transition_ocl(name: &str, owner: u32, pos: glam::Vec3) {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return;
    }
    let leftover_pos = host_pos_to_leftover_coord(pos);
    let leftover_secondary = leftover_transition_ocl_secondary(pos);
    let Some(ocl) =
        gamelogic::helpers::TheObjectCreationListStore::find_object_creation_list(trimmed)
    else {
        return;
    };
    let ctx = gamelogic::object_creation_list::live_creation_context();
    let leftover_owner = gamelogic::helpers::TheGameLogic::find_object_by_id(owner);
    let owner_guard = leftover_owner.as_ref().and_then(|h| h.read().ok());
    let _ = ocl.create_with_owner_flag(
        &ctx,
        owner_guard.as_deref(),
        &leftover_pos,
        &leftover_secondary,
        true,
        0,
    );
}

/// Host Y-up `Vec3` → leftover Z-up `Coord3D`.
fn host_pos_to_leftover_coord(pos: glam::Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y)
}

/// Leftover `play_fx_for_state` secondary: damage-source world pos, else `pos`.
pub fn leftover_transition_ocl_secondary(pos: glam::Vec3) -> gamelogic::common::Coord3D {
    peek_damage_fx_source()
        .map(|src| {
            gamelogic::helpers::TheGameLogic::find_object_by_id(src.id)
                .and_then(|source| source.read().ok().map(|guard| *guard.get_position()))
                .unwrap_or_else(|| host_pos_to_leftover_coord(src.pos))
        })
        .unwrap_or_else(|| host_pos_to_leftover_coord(pos))
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

/// Leftover `TransitionDamageFX::get_local_effect_pos` (pristine bone or loc).
fn leftover_local_effect_pos(
    particle: &HostTransitionParticle,
    drawable: Option<&gamelogic::object::drawable::Drawable>,
) -> gamelogic::common::Coord3D {
    let loc = gamelogic::common::Coord3D::new(particle.loc[0], particle.loc[1], particle.loc[2]);
    let Some(bone) = particle.bone.as_deref() else {
        return loc;
    };
    let Some(drawable) = drawable else {
        return loc;
    };
    if !particle.random_bone {
        let mut positions = drawable.get_pristine_bone_positions(bone, 0, 1);
        if let Some(pos) = positions.pop() {
            return pos;
        }
        return loc;
    }
    const MAX_BONES: usize = 32;
    let positions = drawable.get_pristine_bone_positions(bone, 1, MAX_BONES);
    if positions.is_empty() {
        return loc;
    }
    let pick = gamelogic::common::game_logic_random_value(0, positions.len() as u32 - 1) as usize;
    positions.into_iter().nth(pick).unwrap_or(loc)
}

fn leftover_to_host_local(pos: gamelogic::common::Coord3D) -> glam::Vec3 {
    glam::Vec3::new(pos.x, pos.z, pos.y)
}

fn leftover_local_effect_pos_live(
    particle: &HostTransitionParticle,
    drawable: Option<&gamelogic::object::drawable::Drawable>,
    model: &str,
    scale: f32,
) -> gamelogic::common::Coord3D {
    if drawable.is_some() {
        return leftover_local_effect_pos(particle, drawable);
    }
    let loc = gamelogic::common::Coord3D::new(particle.loc[0], particle.loc[1], particle.loc[2]);
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

/// Live-host pose: leftover drawable bones, else model/scale pristine lookup.
pub fn spawn_transition_particles_at_pose(
    registry: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
    particles: &[HostTransitionParticle],
    position: glam::Vec3,
    yaw: f32,
    model: &str,
    scale: f32,
    frame: u32,
    owner: crate::game_logic::ObjectId,
) -> Vec<u32> {
    let leftover_owner = gamelogic::helpers::TheGameLogic::find_object_by_id(owner.0);
    let leftover_guard = leftover_owner.as_ref().and_then(|h| h.read().ok());
    let leftover_yaw = leftover_guard
        .as_ref()
        .map(|obj| obj.get_orientation())
        .unwrap_or(yaw);
    let mut ids = Vec::new();
    for p in particles {
        if p.name.is_empty() || p.name.eq_ignore_ascii_case("none") {
            continue;
        }
        let leftover_drawable_handle = leftover_guard.as_ref().and_then(|obj| obj.get_drawable());
        let leftover_drawable = leftover_drawable_handle
            .as_ref()
            .and_then(|d| d.read().ok());

        let leftover_local =
            leftover_local_effect_pos_live(p, leftover_drawable.as_deref(), model, scale);
        let host_local = leftover_to_host_local(leftover_local);
        if let Some(id) = registry.attach_named_to_object_local(
            owner,
            position,
            leftover_yaw,
            host_local,
            frame,
            &p.name,
            crate::game_logic::combat_particles::CombatParticleKind::DeathSmoke,
            None,
        ) {
            ids.push(id);
            continue;
        }
        let (sin, cos) = leftover_yaw.sin_cos();
        let world = glam::Vec3::new(
            position.x + host_local.x * cos - host_local.z * sin,
            position.y + host_local.y,
            position.z + host_local.x * sin + host_local.z * cos,
        );
        let id = registry.spawn(
            crate::game_logic::combat_particles::CombatParticleKind::DeathSmoke,
            world,
            frame,
            Some(owner),
            None,
        );
        if let Some(entry) = registry.get_mut(id) {
            entry.template_name = p.name.clone();
            entry.attach_offset = host_local;
        }
        ids.push(id);
    }
    ids
}

/// C++ createParticleSystem + setPosition(local) + attachToObject.
pub fn spawn_transition_particles(
    registry: &mut crate::game_logic::combat_particles::CombatParticleRegistry,
    particles: &[HostTransitionParticle],
    position: glam::Vec3,
    frame: u32,
    owner: crate::game_logic::ObjectId,
) -> Vec<u32> {
    spawn_transition_particles_at_pose(registry, particles, position, 0.0, "", 1.0, frame, owner)
}

/// C++ `ActiveBody.cpp` `#define YELLOW_DAMAGE_PERCENT (0.25f)`.
pub const YELLOW_DAMAGE_PERCENT: f32 = 0.25;

/// Template `SoundOnDamaged` / `SoundOnReallyDamaged` / `VoiceFear` names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateDamageAudio {
    pub sound_on_damaged: Option<String>,
    pub sound_on_really_damaged: Option<String>,
    pub voice_fear: Option<String>,
}

thread_local! {
    static TEMPLATE_AUDIO_OVERRIDE: RefCell<HashMap<String, TemplateDamageAudio>> =
        RefCell::new(HashMap::new());
    static DISPATCHED_ARMOR_DAMAGE_FX: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static PENDING_ATTACKED_BY: RefCell<Vec<(u32, ObjectId)>> = const { RefCell::new(Vec::new()) };
    static ATTACKED_BY_LOG: RefCell<Vec<(i32, i32)>> = const { RefCell::new(Vec::new()) };
    static VOICE_FEAR_ROLL: Cell<Option<i32>> = const { Cell::new(None) };
    static DAMAGE_FX_SOURCE: RefCell<Option<HostDamageFxVictim>> = const { RefCell::new(None) };
}

fn nonempty_event_name(event: &game_engine::common::audio::AudioEventRts) -> Option<String> {
    let name = event.get_event_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

pub fn lookup_template_damage_audio(template_name: &str) -> TemplateDamageAudio {
    if let Some(over) = TEMPLATE_AUDIO_OVERRIDE.with(|m| m.borrow().get(template_name).cloned()) {
        return over;
    }
    // Non-blocking Common factory only. TheThingFactory::find_template can
    // rebuild the Object INI database on a miss (seconds) — never on hit FX.
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                return TemplateDamageAudio {
                    sound_on_damaged: tmpl.get_sound_on_damaged().and_then(nonempty_event_name),
                    sound_on_really_damaged: tmpl
                        .get_sound_on_really_damaged()
                        .and_then(nonempty_event_name),
                    voice_fear: tmpl.get_voice_fear().and_then(nonempty_event_name),
                };
            }
        }
    }
    TemplateDamageAudio::default()
}

pub fn set_test_template_audio(template_name: &str, audio: TemplateDamageAudio) {
    TEMPLATE_AUDIO_OVERRIDE.with(|m| {
        m.borrow_mut().insert(template_name.to_string(), audio);
    });
}

pub fn clear_test_template_audio() {
    TEMPLATE_AUDIO_OVERRIDE.with(|m| m.borrow_mut().clear());
}

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

pub fn set_test_voice_fear_roll(roll: Option<i32>) {
    VOICE_FEAR_ROLL.set(roll);
}

pub fn take_voice_fear_roll() -> i32 {
    VOICE_FEAR_ROLL
        .get()
        .unwrap_or_else(|| game_engine::common::random_value::get_game_logic_random_value(0, 99))
}

/// Best ArmorSet `DamageFX =` name for live flags (C++ `validateArmorAndDamageFX`).
pub fn find_best_armor_set_damage_fx(
    sets: &[crate::game_logic::HostArmorSet],
    flags: u8,
) -> Option<String> {
    if sets.is_empty() {
        return None;
    }
    let mut best: Option<&crate::game_logic::HostArmorSet> = None;
    let mut best_yes = 0u32;
    let mut best_extra = u32::MAX;
    for set in sets {
        let yes = u32::from(set.conditions & flags).count_ones();
        let extra = u32::from(set.conditions & !flags).count_ones();
        if yes > best_yes || (yes >= best_yes && extra < best_extra) {
            best = Some(set);
            best_yes = yes;
            best_extra = extra;
        }
    }
    best.and_then(|set| set.damage_fx.clone())
        .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("none"))
}

fn host_to_ini_damage_type(
    host: crate::game_logic::combat::DamageType,
) -> game_engine::common::ini::ini_damage_fx::DamageType {
    use game_engine::common::ini::ini_damage_fx::DamageType as I;
    match host.to_store() {
        gamelogic::damage::DamageType::Explosion => I::Explosion,
        gamelogic::damage::DamageType::Crush => I::Crush,
        gamelogic::damage::DamageType::ArmorPiercing => I::ArmorPiercing,
        gamelogic::damage::DamageType::SmallArms => I::SmallArms,
        gamelogic::damage::DamageType::Gattling => I::Gattling,
        gamelogic::damage::DamageType::Radiation => I::Radiation,
        gamelogic::damage::DamageType::Flame => I::Flame,
        gamelogic::damage::DamageType::Laser => I::Laser,
        gamelogic::damage::DamageType::Sniper => I::Sniper,
        gamelogic::damage::DamageType::Poison => I::Poison,
        gamelogic::damage::DamageType::Healing => I::Healing,
        gamelogic::damage::DamageType::Unresistable => I::Unresistable,
        gamelogic::damage::DamageType::Water => I::Water,
        gamelogic::damage::DamageType::Deploy => I::Deploy,
        gamelogic::damage::DamageType::Surrender => I::Surrender,
        gamelogic::damage::DamageType::Hack => I::Hack,
        gamelogic::damage::DamageType::KillPilot => I::KillPilot,
        gamelogic::damage::DamageType::Penalty => I::Penalty,
        gamelogic::damage::DamageType::Falling => I::Falling,
        gamelogic::damage::DamageType::Melee => I::Melee,
        gamelogic::damage::DamageType::Disarm => I::Disarm,
        gamelogic::damage::DamageType::HazardCleanup => I::HazardCleanup,
        gamelogic::damage::DamageType::ParticleBeam => I::ParticleBeam,
        gamelogic::damage::DamageType::Toppling => I::Toppling,
        gamelogic::damage::DamageType::InfantryMissile => I::InfantryMissile,
        gamelogic::damage::DamageType::AuroraBomb => I::AuroraBomb,
        gamelogic::damage::DamageType::LandMine => I::LandMine,
        gamelogic::damage::DamageType::JetMissiles => I::JetMissiles,
        gamelogic::damage::DamageType::StealthJetMissiles => I::StealthJetMissiles,
        gamelogic::damage::DamageType::MolotovCocktail => I::MolotovCocktail,
        gamelogic::damage::DamageType::ComancheVulcan => I::ComancheVulcan,
        gamelogic::damage::DamageType::SubdualMissile => I::SubdualMissile,
        gamelogic::damage::DamageType::SubdualVehicle => I::SubdualVehicle,
        gamelogic::damage::DamageType::SubdualBuilding => I::SubdualBuilding,
        gamelogic::damage::DamageType::SubdualUnresistable => I::SubdualUnresistable,
        gamelogic::damage::DamageType::Microwave => I::Microwave,
        gamelogic::damage::DamageType::KillGarrisoned => I::KillGarrisoned,
        gamelogic::damage::DamageType::Status => I::Status,
        gamelogic::damage::DamageType::DamageNumTypes => I::Unresistable,
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

pub fn set_damage_fx_source(source: Option<HostDamageFxVictim>) {
    DAMAGE_FX_SOURCE.with(|c| *c.borrow_mut() = source);
}

pub fn peek_damage_fx_source() -> Option<HostDamageFxVictim> {
    DAMAGE_FX_SOURCE.with(|c| c.borrow().clone())
}

pub fn clear_damage_fx_source() {
    DAMAGE_FX_SOURCE.with(|c| *c.borrow_mut() = None);
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
) -> Option<String> {
    let flags = crate::game_logic::host_armor_residual::live_armor_set_flags(obj);
    let Some(dfx_name) = find_best_armor_set_damage_fx(&obj.thing.template.armor_sets, flags)
    else {
        return None;
    };
    let now = crate::game_logic::host_historic_bonus::logic_frame();
    // C++ ActiveBody.cpp:309-315 — same type + now < next time → skip.
    if obj.last_damage_fx_done == Some(damage_type) && now < obj.next_damage_fx_time {
        return None;
    }
    game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
    crate::game_logic::publish_host_fx_object_ex(
        obj.id.0,
        obj.get_position(),
        obj.get_orientation(),
        obj.owner_player_id.map(|p| p as i32).unwrap_or(-1),
        crate::game_logic::host_supply_gather::host_bounding_circle_radius(
            obj.thing.template.geometry_info.authored,
            obj.thing.template.geometry_info.bounding_circle_radius(),
            obj.thing.geometry.radius.max(obj.selection_radius),
        ),
    );

    let ini_dt = host_to_ini_damage_type(damage_type);
    let victim = HostDamageFxVictim {
        name: obj.template_name.clone(),
        id: obj.id.0,
        vet: veterancy_to_damage_fx_level(obj.experience.level),
        pos: obj.get_position(),
    };
    // C++ DamageFX.cpp:61-93 — throttle + major/minor list use SOURCE veterancy.
    // Missing source → LEVEL_REGULAR. Victim stays primary FX object.
    let source = peek_damage_fx_source();
    let source_ref = source
        .as_ref()
        .map(|s| s as &dyn game_engine::common::ini::ini_damage_fx::Object);
    let (list_name, throttle) = {
        let store = game_engine::common::ini::ini_damage_fx::get_damage_fx_store()?;
        let dfx = store.find_damage_fx(&dfx_name)?;
        let throttle = dfx.get_damage_fx_throttle_time(ini_dt, source_ref);
        let list = dfx.get_damage_fx_list(ini_dt, actual_damage, source_ref);
        dfx.do_damage_fx(ini_dt, actual_damage, source_ref, Some(&victim));
        (list, throttle)
    };
    obj.last_damage_fx_done = Some(damage_type);
    obj.next_damage_fx_time = now.saturating_add(throttle);
    DISPATCHED_ARMOR_DAMAGE_FX.with(|v| v.borrow_mut().push(dfx_name.clone()));
    if let Some(list) = &list_name {
        DISPATCHED_ARMOR_DAMAGE_FX.with(|v| v.borrow_mut().push(list.clone()));
        // C++ ActiveBody::doDamageFX → DamageFX::doDamageFX once.
        // `dfx.do_damage_fx` already runs leftover FXList::doFXObj.
    }
    list_name.or(Some(dfx_name))
}

pub fn take_dispatched_armor_damage_fx() -> Vec<String> {
    DISPATCHED_ARMOR_DAMAGE_FX.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

/// C++ `ActiveBody.cpp:574-581` — victim `Player::setAttackedBy(srcIndex)`.
pub fn queue_attacked_by(victim_player: Option<u32>, source: Option<ObjectId>) {
    let (Some(victim), Some(src)) = (victim_player, source) else {
        return;
    };
    PENDING_ATTACKED_BY.with(|p| p.borrow_mut().push((victim, src)));
}

pub fn take_pending_attacked_by() -> Vec<(u32, ObjectId)> {
    PENDING_ATTACKED_BY.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

pub fn apply_victim_attacked_by(victim_player: i32, attacker_player: i32) {
    ATTACKED_BY_LOG.with(|l| l.borrow_mut().push((victim_player, attacker_player)));
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return;
    };
    for arc in list.iter() {
        let Ok(player) = arc.read() else {
            continue;
        };
        if player.get_player_index() != victim_player {
            continue;
        }
        drop(player);
        if let Ok(mut player) = arc.write() {
            player.set_attacked_by(attacker_player);
        }
        return;
    }
}

#[cfg(test)]
pub fn take_attacked_by_log() -> Vec<(i32, i32)> {
    ATTACKED_BY_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Queue C++ VoiceFear when health crosses yellow (ActiveBody.cpp:624-637).
///
/// C++ copies `*getTemplate()->getVoiceFear()`, `setPosition(obj->getPosition())`,
/// `setPlayerIndex(controlling player)`. Missing VoiceFear is an empty
/// AudioEventRTS (silent) — never `{template}VoiceFear`.
pub fn queue_voice_fear_event(
    pending: &mut Vec<HostTransitionDamageFxEvent>,
    template_name: &str,
    old_state: HostBodyDamageType,
    new_state: HostBodyDamageType,
    prev_health: f32,
    current_health: f32,
    max_health: f32,
    victim: ObjectId,
    position: glam::Vec3,
    player_id: Option<u32>,
) {
    if !voice_fear_should_play(
        prev_health,
        current_health,
        max_health,
        take_voice_fear_roll(),
    ) {
        return;
    }
    let Some(fear) = lookup_template_damage_audio(template_name)
        .voice_fear
        .filter(|name| !name.is_empty())
    else {
        return;
    };
    crate::game_logic::host_voice_fear_log::record(victim, position, player_id, fear.clone());
    pending.push(HostTransitionDamageFxEvent {
        old_state: old_state.ordinal(),
        new_state: new_state.ordinal(),
        fx_name: None,
        audio_name: Some(fear),
        extra_fx_names: Vec::new(),
        ocl_names: Vec::new(),
        particles: Vec::new(),
        fx_locs: Vec::new(),
        ocl_locs: Vec::new(),

        clear_old_state: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    #[test]
    fn worse_transition_does_not_invent_structure_smoke() {
        let d = HostTransitionDamageFxData::generic_structure_residual();
        assert!(
            transition_event(
                &d,
                HostBodyDamageType::Damaged,
                HostBodyDamageType::Pristine
            )
            .is_none()
        );
        // Unauthored TransitionDamageFX: no invented BuildingDamageSmoke/Fire.
        assert!(
            transition_event(
                &d,
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .is_none(),
            "must not invent BuildingDamageSmoke for templates without authored PSys"
        );
    }

    #[test]
    fn authored_transition_particles_still_emit() {
        let mut d = HostTransitionDamageFxData::generic_structure_residual();
        d.particles_for_state[HostBodyDamageType::Damaged.ordinal() as usize] =
            vec![HostTransitionParticle {
                name: "AuthoredDamagedPSys".into(),
                bone: Some("Smoke01".into()),
                loc: [0.0, 0.0, 0.0],
                random_bone: false,
            }];
        let e = transition_event(
            &d,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
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
            HostBodyDamageType::ReallyDamaged,
            HostBodyDamageType::Damaged,
        )
        .expect("heal still destroys old PSys");
        assert!(e.particles.is_empty());
        assert_eq!(
            e.clear_old_state,
            Some(HostBodyDamageType::ReallyDamaged.ordinal())
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
        let damaged = HostBodyDamageType::Damaged.ordinal() as usize;
        let really = HostBodyDamageType::ReallyDamaged.ordinal() as usize;
        d.fx_lists_for_state[damaged] = vec!["FX_AuthoredDamaged".into()];
        d.ocl_for_state[damaged] = vec!["OCL_AuthoredDebris".into()];
        d.fx_lists_for_state[really] = vec!["FX_AuthoredReallyDamaged".into()];
        d.ocl_for_state[really] = vec!["OCL_AuthoredReallyDebris".into()];
        let damaged_ev = transition_event(
            &d,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
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
            HostBodyDamageType::Damaged,
            HostBodyDamageType::ReallyDamaged,
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
        set_test_template_audio(
            "AmericaInfantryRanger",
            TemplateDamageAudio {
                sound_on_damaged: Some("RangerVoiceDamaged".into()),
                sound_on_really_damaged: Some("RangerVoiceReallyDamaged".into()),
                voice_fear: Some("RangerVoiceFear".into()),
            },
        );
        let cfg = transition_damage_fx_config_for_template("AmericaInfantryRanger", false, false)
            .expect("infantry must get a config");
        let e = transition_event(
            &cfg,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
        )
        .expect("template audio");
        assert_eq!(e.audio_name.as_deref(), Some("RangerVoiceDamaged"));
        assert_ne!(e.audio_name.as_deref(), Some("BuildingDamaged"));
        assert_ne!(e.audio_name.as_deref(), Some("VehicleDamaged"));
        clear_test_template_audio();
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
        let damaged = HostBodyDamageType::Damaged.ordinal() as usize;
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
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
            Some(gamelogic::damage::DamageType::Explosion),
        )
        .expect("ocl still plays");
        assert!(explosion.fx_name.is_none());
        assert!(explosion.particles.is_empty());
        assert_eq!(explosion.ocl_names, vec!["OCL_AuthoredDebris".to_string()]);
        let flame = transition_event_for_damage(
            &d,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
            Some(gamelogic::damage::DamageType::Flame),
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
        let leftover = leftover_local_effect_pos(&p, None);
        assert_eq!(leftover.x, 1.0);
        assert_eq!(leftover.y, 2.0);
        assert_eq!(leftover.z, 8.0);
        let host = leftover_to_host_local(leftover);
        assert_eq!(host, glam::Vec3::new(1.0, 8.0, 2.0));
        let mut registry = crate::game_logic::combat_particles::CombatParticleRegistry::new();
        let ids = spawn_transition_particles(
            &mut registry,
            &[p],
            glam::Vec3::new(10.0, 0.0, 4.0),
            1,
            crate::game_logic::ObjectId(7),
        );
        assert_eq!(ids.len(), 1);
        let entry = registry.get(ids[0]).expect("spawned");
        assert_eq!(entry.attach_offset, glam::Vec3::new(1.0, 8.0, 2.0));
        assert!((entry.position - glam::Vec3::new(11.0, 8.0, 6.0)).length() < 0.01);
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
        let damaged = HostBodyDamageType::Damaged.ordinal() as usize;
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
        let ev = transition_event(
            &d,
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
        )
        .expect("authored loc");
        assert_eq!(ev.fx_name.as_deref(), Some("FX_AuthoredDamaged"));
        assert_eq!(ev.fx_locs.len(), 1);
        assert_eq!(ev.fx_locs[0].loc, [1.0, 2.0, 8.0]);
        assert_eq!(ev.ocl_locs[0].loc, [3.0, 4.0, 5.0]);
        let world = leftover_named_slot_world_pos(
            ev.fx_locs.first(),
            0,
            glam::Vec3::new(10.0, 0.0, 4.0),
            0.0,
            "",
            1.0,
        );
        assert!((world - glam::Vec3::new(11.0, 8.0, 6.0)).length() < 0.01);
    }

    #[test]
    fn leftover_transition_ocl_secondary_uses_damage_source_pos() {
        // C++ TransitionDamageFX.cpp:354-355 — secondary is attacker pos.
        set_damage_fx_source(Some(HostDamageFxVictim {
            name: "AmericaTankCrusader".into(),
            id: 2,
            vet: 0,
            pos: glam::Vec3::new(10.0, 4.0, 6.0),
        }));
        let secondary = leftover_transition_ocl_secondary(glam::Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(secondary.x, 10.0);
        assert_eq!(secondary.y, 6.0);
        assert_eq!(secondary.z, 4.0);
        clear_damage_fx_source();
        let fallback = leftover_transition_ocl_secondary(glam::Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(fallback.x, 1.0);
        assert_eq!(fallback.y, 3.0);
        assert_eq!(fallback.z, 2.0);
    }
}
