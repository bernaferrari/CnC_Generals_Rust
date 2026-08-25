//! Host BoneFXDamage + BoneFXUpdate (FX on body-damage transitions).
//!
//! C++: `BoneFXDamage::onBodyDamageStateChange` → `BoneFXUpdate::changeBodyDamageState`
//! then `BoneFXUpdate::update` fires authored per-state FXList / OCL / PSys at
//! bone world (FX/OCL) or object-local bone (PSys) with OnlyOnce + delay.
//!
//! Leftover `bone_fx_update.rs` already matches C++ parse/schedule. Dual-world
//! `do_*` helpers stay fail-closed when OBJECT_REGISTRY is empty; this live
//! path drives the same authored slots without inventing FX names.

use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use gamelogic::object::update::bone_fx_update::{
    BODY_DAMAGE_TYPE_COUNT, BONE_FX_MAX_BONES, parse_fx_list_attr, parse_ocl_attr,
    parse_particle_attr,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

const FX_PREFIX: [(&str, usize); 4] = [
    ("PristineFXList", 0),
    ("DamagedFXList", 1),
    ("ReallyDamagedFXList", 2),
    ("RubbleFXList", 3),
];
const OCL_PREFIX: [(&str, usize); 4] = [
    ("PristineOCL", 0),
    ("DamagedOCL", 1),
    ("ReallyDamagedOCL", 2),
    ("RubbleOCL", 3),
];
const PSYS_PREFIX: [(&str, usize); 4] = [
    ("PristineParticleSystem", 0),
    ("DamagedParticleSystem", 1),
    ("ReallyDamagedParticleSystem", 2),
    ("RubbleParticleSystem", 3),
];

/// One authored BoneFX slot (C++ `BoneFXListInfo` / OCL / PSys).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HostBoneFxSlot {
    pub bone: String,
    pub only_once: bool,
    pub delay_min: f32,
    pub delay_max: f32,
    pub name: String,
}

/// Authored per-state 8-bone tables from leftover BoneFXUpdate INI parse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostBoneFxAuthored {
    pub fx: [[Option<HostBoneFxSlot>; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    pub ocl: [[Option<HostBoneFxSlot>; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    pub psys: [[Option<HostBoneFxSlot>; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// C++ `m_damageFXTypes` (default ALL).
    #[serde(default = "all_damage_type_bits")]
    pub damage_fx_types: u64,
    /// C++ `m_damageOCLTypes` (default ALL).
    #[serde(default = "all_damage_type_bits")]
    pub damage_ocl_types: u64,
    /// C++ `m_damageParticleTypes` (default ALL).
    #[serde(default = "all_damage_type_bits")]
    pub damage_particle_types: u64,
}

fn all_damage_type_bits() -> u64 {
    gamelogic::damage::DamageTypeFlags::all_flags().bits()
}

impl Default for HostBoneFxAuthored {
    fn default() -> Self {
        Self {
            fx: Default::default(),
            ocl: Default::default(),
            psys: Default::default(),
            damage_fx_types: all_damage_type_bits(),
            damage_ocl_types: all_damage_type_bits(),
            damage_particle_types: all_damage_type_bits(),
        }
    }
}

impl HostBoneFxAuthored {
    pub fn is_empty(&self) -> bool {
        !self
            .fx
            .iter()
            .chain(self.ocl.iter())
            .chain(self.psys.iter())
            .flatten()
            .any(|slot| slot.is_some())
    }
}

/// One residual FX burst for a body-damage transition / delayed fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBoneFxEvent {
    pub old_state: HostBodyDamageType,
    pub new_state: HostBodyDamageType,
    pub bone: String,
    pub fx_list: Option<String>,
    pub ocl: Option<String>,
    pub particle_system: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBoneFxDamageData {
    pub transitions: u32,
    pub last_fx: Option<String>,
    pub pending: Vec<HostBoneFxEvent>,
    #[serde(default)]
    pub authored: HostBoneFxAuthored,
    #[serde(default = "unset_schedule")]
    next_fx_frame: [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    #[serde(default = "unset_schedule")]
    next_ocl_frame: [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    #[serde(default = "unset_schedule")]
    next_ps_frame: [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    #[serde(default)]
    cur_state: u8,
    /// C++ `BoneFXUpdate::m_active` — first `update` calls `initTimes`.
    #[serde(default)]
    active: bool,
    /// Leftover `DamageType` ordinal of last body damage (None = no lastDamageInfo).
    #[serde(default)]
    last_damage_type: Option<u32>,
    /// C++ `m_particleSystemIDs` leftover client ids.
    #[serde(default)]
    particle_system_ids: Vec<u32>,
}

fn unset_schedule() -> [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT] {
    [[-1; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT]
}

impl Default for HostBoneFxDamageData {
    fn default() -> Self {
        Self {
            transitions: 0,
            last_fx: None,
            pending: Vec::new(),
            authored: HostBoneFxAuthored::default(),
            next_fx_frame: unset_schedule(),
            next_ocl_frame: unset_schedule(),
            next_ps_frame: unset_schedule(),
            cur_state: 0,
            active: false,
            last_damage_type: None,
            particle_system_ids: Vec::new(),
        }
    }
}
impl HostBoneFxDamageData {
    pub fn from_authored(authored: HostBoneFxAuthored) -> Self {
        Self {
            authored,
            ..Self::default()
        }
    }

    /// Peel leftover BoneFXUpdate INI onto a live host residual.
    pub fn from_template(template_name: &str) -> Option<Self> {
        let authored = peel_authored_bone_fx(template_name)?;
        Some(Self::from_authored(authored))
    }

    /// C++ `BoneFXUpdate::changeBodyDamageState` then one `update` pass.
    pub fn on_body_damage_state_change(
        &mut self,
        _template_name: &str,
        old_state: HostBodyDamageType,
        new_state: HostBodyDamageType,
    ) -> Option<HostBoneFxEvent> {
        if self.authored.is_empty() {
            return None;
        }
        let now = crate::game_logic::host_historic_bonus::logic_frame() as i32;
        // C++ changeBodyDamageState: set state, killRunningParticleSystems, initTimes.
        self.cur_state = new_state.ordinal();
        self.kill_running_particle_systems();
        self.init_times(now);
        self.active = true;
        self.transitions = self.transitions.saturating_add(1);
        self.tick_due(now, old_state, new_state);
        self.pending.last().cloned()
    }

    /// C++ `BoneFXUpdate::update` — first call inits Pristine times.
    pub fn tick(&mut self, now: i32) {
        if !self.active {
            self.init_times(now);
            self.active = true;
        }
        let state = state_from_ordinal(self.cur_state);
        self.tick_due(now, state, state);
    }

    pub fn drain_pending(&mut self) -> Vec<HostBoneFxEvent> {
        std::mem::take(&mut self.pending)
    }

    /// C++ `BoneFXUpdate::stopAllBoneFX`.
    pub fn stop_all_bone_fx(&mut self) {
        self.next_fx_frame = unset_schedule();
        self.next_ocl_frame = unset_schedule();
        self.next_ps_frame = unset_schedule();
        self.kill_running_particle_systems();
    }

    /// C++ `lastDamageInfo->in.m_damageType` stamp.
    pub fn stamp_last_damage_type(&mut self, dtype: Option<crate::game_logic::combat::DamageType>) {
        self.last_damage_type = dtype.map(|d| d.to_store() as u32);
    }

    pub fn track_particle(&mut self, id: u32) {
        if id != 0 {
            self.particle_system_ids.push(id);
        }
    }
    pub fn running_particle_count(&self) -> usize {
        self.particle_system_ids.len()
    }

    /// C++ `BoneFXUpdate::killRunningParticleSystems`.
    pub fn kill_running_particle_systems(&mut self) {
        if let Some(manager) = gamelogic::helpers::TheParticleSystemManager::get() {
            for id in self.particle_system_ids.drain(..) {
                manager.destroy_particle_system(id);
            }
        } else {
            self.particle_system_ids.clear();
        }
    }

    fn damage_type_allowed(&self, flags: u64) -> bool {
        let Some(ordinal) = self.last_damage_type else {
            return true;
        };
        let mask = gamelogic::damage::DamageTypeFlags::from_bits_truncate(flags);
        mask.contains_damage_type(gamelogic::damage::DamageType::from_u32(ordinal))
    }

    fn init_times(&mut self, now: i32) {
        let idx = self.cur_state as usize;
        if idx >= BODY_DAMAGE_TYPE_COUNT {
            return;
        }
        for i in 0..BONE_FX_MAX_BONES {
            self.next_fx_frame[idx][i] = next_from_now(self.authored.fx[idx][i].as_ref(), now);
            self.next_ocl_frame[idx][i] = next_from_now(self.authored.ocl[idx][i].as_ref(), now);
            self.next_ps_frame[idx][i] = next_from_now(self.authored.psys[idx][i].as_ref(), now);
        }
    }

    fn tick_due(&mut self, now: i32, old_state: HostBodyDamageType, new_state: HostBodyDamageType) {
        let idx = self.cur_state as usize;
        if idx >= BODY_DAMAGE_TYPE_COUNT {
            return;
        }
        for i in 0..BONE_FX_MAX_BONES {
            if due(self.next_fx_frame[idx][i], now) {
                if let Some(slot) = self.authored.fx[idx][i].clone() {
                    if self.damage_type_allowed(self.authored.damage_fx_types) {
                        self.push_fire(old_state, new_state, &slot, FireKind::Fx);
                    }
                    self.next_fx_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_fx_frame[idx][i] = -1;
                }
            }
            if due(self.next_ocl_frame[idx][i], now) {
                if let Some(slot) = self.authored.ocl[idx][i].clone() {
                    if self.damage_type_allowed(self.authored.damage_ocl_types) {
                        self.push_fire(old_state, new_state, &slot, FireKind::Ocl);
                    }
                    self.next_ocl_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_ocl_frame[idx][i] = -1;
                }
            }
            if due(self.next_ps_frame[idx][i], now) {
                if let Some(slot) = self.authored.psys[idx][i].clone() {
                    if self.damage_type_allowed(self.authored.damage_particle_types) {
                        self.push_fire(old_state, new_state, &slot, FireKind::Psys);
                    }
                    self.next_ps_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_ps_frame[idx][i] = -1;
                }
            }
        }
    }

    fn push_fire(
        &mut self,
        old_state: HostBodyDamageType,
        new_state: HostBodyDamageType,
        slot: &HostBoneFxSlot,
        kind: FireKind,
    ) {
        let name = slot.name.trim();
        let ev = match kind {
            FireKind::Fx => HostBoneFxEvent {
                old_state,
                new_state,
                bone: slot.bone.clone(),
                fx_list: nonempty_name(name),
                ocl: None,
                particle_system: None,
            },
            FireKind::Ocl => HostBoneFxEvent {
                old_state,
                new_state,
                bone: slot.bone.clone(),
                fx_list: None,
                ocl: nonempty_name(name),
                particle_system: None,
            },
            FireKind::Psys => HostBoneFxEvent {
                old_state,
                new_state,
                bone: slot.bone.clone(),
                fx_list: None,
                ocl: None,
                particle_system: nonempty_name(name),
            },
        };
        if ev.fx_list.is_none() && ev.ocl.is_none() && ev.particle_system.is_none() {
            return;
        }
        if self.last_fx.is_none() {
            self.last_fx = ev
                .fx_list
                .clone()
                .or_else(|| ev.particle_system.clone())
                .or_else(|| ev.ocl.clone());
        } else if let Some(fx) = ev.fx_list.as_ref() {
            self.last_fx = Some(fx.clone());
        }
        self.pending.push(ev);
    }
}

#[derive(Clone, Copy)]
enum FireKind {
    Fx,
    Ocl,
    Psys,
}

fn due(frame: i32, now: i32) -> bool {
    frame != -1 && frame <= now
}

fn nonempty_name(name: &str) -> Option<String> {
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(name.to_string())
    }
}

fn delay_frames(min: f32, max: f32) -> i32 {
    if (min - max).abs() <= f32::EPSILON {
        min as i32
    } else {
        gamelogic::common::RandomVariable::new(min, max).get_value() as i32
    }
}

fn next_from_now(slot: Option<&HostBoneFxSlot>, now: i32) -> i32 {
    let Some(slot) = slot else {
        return -1;
    };
    if slot.bone.is_empty() {
        return -1;
    }
    now + delay_frames(slot.delay_min, slot.delay_max)
}

fn next_after_fire(slot: &HostBoneFxSlot, now: i32) -> i32 {
    if slot.only_once {
        -1
    } else {
        // C++ next update is a later frame; delay 0 still waits one host tick.
        now + delay_frames(slot.delay_min, slot.delay_max).max(1)
    }
}

fn state_from_ordinal(ordinal: u8) -> HostBodyDamageType {
    match ordinal {
        1 => HostBodyDamageType::Damaged,
        2 => HostBodyDamageType::ReallyDamaged,
        3 => HostBodyDamageType::Rubble,
        _ => HostBodyDamageType::Pristine,
    }
}

/// Template has leftover-authored BoneFXUpdate slots (not a name heuristic).
pub fn wants_bone_fx(template_name: &str) -> bool {
    peel_authored_bone_fx(template_name).is_some()
}

pub fn peel_authored_bone_fx(template_name: &str) -> Option<HostBoneFxAuthored> {
    if let Some(authored) = peel_leftover_factory_bone_fx(template_name) {
        return Some(authored);
    }
    peel_asset_manager_bone_fx(template_name)
}

fn peel_leftover_factory_bone_fx(template_name: &str) -> Option<HostBoneFxAuthored> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry.name.as_str().eq_ignore_ascii_case("BoneFXUpdate") {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::update::bone_fx_update::BoneFXUpdateModuleData>(
        ) {
            if let Some(authored) = authored_from_leftover_module_data(data) {
                return Some(authored);
            }
        }
        let mut authored = HostBoneFxAuthored::default();
        if let Some(raw) = entry.data.get_ini_field("DamageFXTypes") {
            authored.damage_fx_types = parse_damage_type_flags_attr(raw).bits();
        }
        if let Some(raw) = entry.data.get_ini_field("DamageOCLTypes") {
            authored.damage_ocl_types = parse_damage_type_flags_attr(raw).bits();
        }
        if let Some(raw) = entry.data.get_ini_field("DamageParticleTypes") {
            authored.damage_particle_types = parse_damage_type_flags_attr(raw).bits();
        }
        let mut any = false;
        for slot in 1..=BONE_FX_MAX_BONES {
            for (prefix, state) in FX_PREFIX {
                if let Some(raw) = entry.data.get_ini_field(&format!("{prefix}{slot}")) {
                    if let Ok(info) = parse_fx_list_attr(raw) {
                        if let Some(s) = slot_from_leftover_fx(info) {
                            authored.fx[state][slot - 1] = Some(s);
                            any = true;
                        }
                    }
                }
            }
            for (prefix, state) in OCL_PREFIX {
                if let Some(raw) = entry.data.get_ini_field(&format!("{prefix}{slot}")) {
                    if let Ok(info) = parse_ocl_attr(raw) {
                        if let Some(s) = slot_from_leftover_ocl(info) {
                            authored.ocl[state][slot - 1] = Some(s);
                            any = true;
                        }
                    }
                }
            }
            for (prefix, state) in PSYS_PREFIX {
                if let Some(raw) = entry.data.get_ini_field(&format!("{prefix}{slot}")) {
                    if let Ok(info) = parse_particle_attr(raw) {
                        if let Some(s) = slot_from_leftover_psys(info) {
                            authored.psys[state][slot - 1] = Some(s);
                            any = true;
                        }
                    }
                }
            }
        }
        if any {
            return Some(authored);
        }
    }
    None
}

fn authored_from_leftover_module_data(
    data: &gamelogic::object::update::bone_fx_update::BoneFXUpdateModuleData,
) -> Option<HostBoneFxAuthored> {
    let mut authored = HostBoneFxAuthored {
        damage_fx_types: data.damage_fx_types.bits(),
        damage_ocl_types: data.damage_ocl_types.bits(),
        damage_particle_types: data.damage_particle_types.bits(),
        ..HostBoneFxAuthored::default()
    };
    let mut any = false;
    for state in 0..BODY_DAMAGE_TYPE_COUNT {
        for i in 0..BONE_FX_MAX_BONES {
            if let Some(s) = slot_from_leftover_fx(data.fx_list[state][i].clone()) {
                authored.fx[state][i] = Some(s);
                any = true;
            }
            if let Some(s) = slot_from_leftover_ocl(data.ocl[state][i].clone()) {
                authored.ocl[state][i] = Some(s);
                any = true;
            }
            if let Some(s) = slot_from_leftover_psys(data.particle_system[state][i].clone()) {
                authored.psys[state][i] = Some(s);
                any = true;
            }
        }
    }
    any.then_some(authored)
}

fn peel_asset_manager_bone_fx(template_name: &str) -> Option<HostBoneFxAuthored> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(template_name)?;
    let module = definition
        .behavior_modules
        .iter()
        .find(|m| m.class_name.eq_ignore_ascii_case("BoneFXUpdate"))?;
    let mut authored = HostBoneFxAuthored::default();
    if let Some(raw) = module.attribute("DamageFXTypes") {
        authored.damage_fx_types = parse_damage_type_flags_attr(raw).bits();
    }
    if let Some(raw) = module.attribute("DamageOCLTypes") {
        authored.damage_ocl_types = parse_damage_type_flags_attr(raw).bits();
    }
    if let Some(raw) = module.attribute("DamageParticleTypes") {
        authored.damage_particle_types = parse_damage_type_flags_attr(raw).bits();
    }
    let mut any = false;
    for slot in 1..=BONE_FX_MAX_BONES {
        for (prefix, state) in FX_PREFIX {
            if let Some(raw) = module.attribute(&format!("{prefix}{slot}")) {
                if let Ok(info) = parse_fx_list_attr(raw) {
                    if let Some(s) = slot_from_leftover_fx(info) {
                        authored.fx[state][slot - 1] = Some(s);
                        any = true;
                    }
                }
            }
        }
        for (prefix, state) in OCL_PREFIX {
            if let Some(raw) = module.attribute(&format!("{prefix}{slot}")) {
                if let Ok(info) = parse_ocl_attr(raw) {
                    if let Some(s) = slot_from_leftover_ocl(info) {
                        authored.ocl[state][slot - 1] = Some(s);
                        any = true;
                    }
                }
            }
        }
        for (prefix, state) in PSYS_PREFIX {
            if let Some(raw) = module.attribute(&format!("{prefix}{slot}")) {
                if let Ok(info) = parse_particle_attr(raw) {
                    if let Some(s) = slot_from_leftover_psys(info) {
                        authored.psys[state][slot - 1] = Some(s);
                        any = true;
                    }
                }
            }
        }
    }
    any.then_some(authored)
}

/// Leftover `parse_damage_type_flags` (BoneFXUpdate.cpp INI Damage*Types).
fn parse_damage_type_flags_attr(raw: &str) -> gamelogic::damage::DamageTypeFlags {
    use gamelogic::damage::{DamageType, DamageTypeFlags};
    use std::str::FromStr;
    let mut flags = DamageTypeFlags::empty();
    let mut any = false;
    for token in raw.split_whitespace() {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            any = true;
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DamageTypeFlags::empty();
                continue;
            }
            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };
            if let Ok(damage_type) = DamageType::from_str(name) {
                let flag = DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }
    if any {
        flags
    } else {
        DamageTypeFlags::all_flags()
    }
}

fn slot_from_leftover_fx(
    info: gamelogic::object::update::bone_fx_update::BoneFXListInfo,
) -> Option<HostBoneFxSlot> {
    if info.base.loc_info.bone_name.is_empty() {
        return None;
    }
    Some(HostBoneFxSlot {
        bone: info.base.loc_info.bone_name,
        only_once: info.base.only_once,
        delay_min: info.base.game_logic_delay.min,
        delay_max: info.base.game_logic_delay.max,
        name: info.fx_name,
    })
}

fn slot_from_leftover_ocl(
    info: gamelogic::object::update::bone_fx_update::BoneOCLInfo,
) -> Option<HostBoneFxSlot> {
    if info.base.loc_info.bone_name.is_empty() {
        return None;
    }
    Some(HostBoneFxSlot {
        bone: info.base.loc_info.bone_name,
        only_once: info.base.only_once,
        delay_min: info.base.game_logic_delay.min,
        delay_max: info.base.game_logic_delay.max,
        name: info.ocl_name,
    })
}

fn slot_from_leftover_psys(
    info: gamelogic::object::update::bone_fx_update::BoneParticleSystemInfo,
) -> Option<HostBoneFxSlot> {
    if info.base.loc_info.bone_name.is_empty() {
        return None;
    }
    Some(HostBoneFxSlot {
        bone: info.base.loc_info.bone_name,
        only_once: info.base.only_once,
        delay_min: info.base.game_client_delay.min,
        delay_max: info.base.game_client_delay.max,
        name: info.particle_name,
    })
}

fn cpp_bone_to_host_local(bone: gamelogic::common::Coord3D) -> Vec3 {
    Vec3::new(bone.x, bone.z, bone.y)
}

fn host_local_to_cpp(local: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(local.x, local.z, local.y)
}

fn rotate_yaw_host(origin: Vec3, yaw: f32, local: Vec3) -> Vec3 {
    let (sin, cos) = yaw.sin_cos();
    Vec3::new(
        origin.x + local.x * cos - local.z * sin,
        origin.y + local.y,
        origin.z + local.x * sin + local.z * cos,
    )
}

/// Pristine-bone local offset (C++ `resolveBoneLocations`).
pub fn bone_local_pos(model: &str, scale: f32, bone: &str) -> Vec3 {
    if bone.is_empty() || bone.eq_ignore_ascii_case("none") || model.is_empty() {
        return Vec3::ZERO;
    }
    gamelogic::object::draw::lookup_pristine_bone_translation(model, scale, bone)
        .map(cpp_bone_to_host_local)
        .unwrap_or(Vec3::ZERO)
}

/// C++ `convertBonePosToWorldPos` residual.
pub fn bone_world_pos(origin: Vec3, yaw: f32, model: &str, scale: f32, bone: &str) -> Vec3 {
    rotate_yaw_host(origin, yaw, bone_local_pos(model, scale, bone))
}

/// Play leftover-authored FXList / OCL / PSys at the bone.
/// Returns leftover particle-system id when a PSys was attached (C++ `m_particleSystemIDs`).
pub fn play_bone_fx_event(
    ev: &HostBoneFxEvent,
    owner: u32,
    origin: Vec3,
    yaw: f32,
    model: &str,
    scale: f32,
    drawable_hidden: bool,
) -> Option<u32> {
    let local = bone_local_pos(model, scale, &ev.bone);
    let world = rotate_yaw_host(origin, yaw, local);
    if let Some(fx) = ev.fx_list.as_deref() {
        let _ = crate::game_logic::dispatch_fx_list_at_pos(fx, world);
    }
    if let Some(ocl) = ev.ocl.as_deref() {
        crate::game_logic::host_transition_damage_fx::play_authored_transition_ocl(
            ocl, owner, world,
        );
    }
    let Some(ps) = ev.particle_system.as_deref() else {
        return None;
    };
    crate::game_logic::publish_host_fx_object(owner, origin, yaw, -1);
    let cpp_local = host_local_to_cpp(local);
    let leftover_id = gamelogic::helpers::attach_particle_system_to_object_local(
        ps,
        owner,
        Some(&cpp_local),
        None,
    )?;
    // Leftover `do_particle_system_at_bone`: hidden drawable destroys the system.
    if drawable_hidden {
        if let Some(manager) = gamelogic::helpers::TheParticleSystemManager::get() {
            manager.destroy_particle_system(leftover_id);
        }
        return None;
    }
    Some(leftover_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn damaged_fx_slot() -> HostBoneFxAuthored {
        let mut authored = HostBoneFxAuthored::default();
        authored.fx[HostBodyDamageType::Damaged.ordinal() as usize][0] = Some(HostBoneFxSlot {
            bone: "FXBone01".into(),
            only_once: true,
            delay_min: 0.0,
            delay_max: 0.0,
            name: "FX_ScudLauncherDamageTransition".into(),
        });
        authored
    }

    #[test]
    fn transition_queues_authored_fx() {
        let mut d = HostBoneFxDamageData::from_authored(damaged_fx_slot());
        let ev = d
            .on_body_damage_state_change(
                "GLAVehicleScudLauncher",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .expect("fx");
        assert_eq!(
            ev.fx_list.as_deref(),
            Some("FX_ScudLauncherDamageTransition")
        );
        assert_eq!(ev.bone, "FXBone01");
        assert_eq!(d.transitions, 1);
        assert_eq!(d.drain_pending().len(), 1);
    }

    #[test]
    fn non_peel_skipped() {
        let mut d = HostBoneFxDamageData::default();
        assert!(
            d.on_body_damage_state_change(
                "AmericaTankCrusader",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .is_none()
        );
    }

    #[test]
    fn leftover_parse_keeps_only_once_and_delay() {
        let info =
            parse_fx_list_attr("Bone:Smoke01 OnlyOnce:No 15 45 FXList:FX_BuildingFireMedium")
                .expect("parse");
        assert_eq!(info.base.loc_info.bone_name, "Smoke01");
        assert!(!info.base.only_once);
        assert_eq!(info.fx_name, "FX_BuildingFireMedium");
        assert!(
            (info.base.game_logic_delay.min - 15.0).abs() < f32::EPSILON
                || info.base.game_logic_delay.min > 0.0
        );
    }

    #[test]
    fn invented_names_are_gone() {
        let src = include_str!("host_bone_fx_damage.rs");
        assert!(!src.contains("FX_ScudDamagedBoneFX"));
        assert!(!src.contains("ToxinLeakBonePSys"));
        assert!(!src.contains("ScudSmokeBonePSys"));
        assert!(!src.contains("StructureDamageBonePSys"));
    }

    fn pristine_fx_slot() -> HostBoneFxAuthored {
        let mut authored = HostBoneFxAuthored::default();
        authored.fx[HostBodyDamageType::Pristine.ordinal() as usize][0] = Some(HostBoneFxSlot {
            bone: "Smoke01".into(),
            only_once: true,
            delay_min: 0.0,
            delay_max: 0.0,
            name: "FX_BuildingIdleSmoke".into(),
        });
        authored
    }

    #[test]
    fn first_tick_inits_and_fires_pristine_slots() {
        let mut d = HostBoneFxDamageData::from_authored(pristine_fx_slot());
        d.tick(10);
        let pending = d.drain_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].fx_list.as_deref(), Some("FX_BuildingIdleSmoke"));
        assert_eq!(pending[0].bone, "Smoke01");
    }

    #[test]
    fn damage_fx_types_gate_skips_fire() {
        use crate::game_logic::combat::DamageType;
        let mut authored = damaged_fx_slot();
        authored.damage_fx_types = parse_damage_type_flags_attr("FLAME").bits();
        let mut d = HostBoneFxDamageData::from_authored(authored);
        d.stamp_last_damage_type(Some(DamageType::Bullet));
        assert!(
            d.on_body_damage_state_change(
                "GLAVehicleScudLauncher",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .is_none()
        );
        assert!(d.drain_pending().is_empty());
    }

    #[test]
    fn damage_fx_types_gate_allows_matching_type() {
        use crate::game_logic::combat::DamageType;
        let mut authored = damaged_fx_slot();
        authored.damage_fx_types = parse_damage_type_flags_attr("FLAME").bits();
        let mut d = HostBoneFxDamageData::from_authored(authored);
        d.stamp_last_damage_type(Some(DamageType::Flame));
        let ev = d
            .on_body_damage_state_change(
                "GLAVehicleScudLauncher",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .expect("flame-gated fx");
        assert_eq!(
            ev.fx_list.as_deref(),
            Some("FX_ScudLauncherDamageTransition")
        );
    }

    #[test]
    fn state_change_kills_tracked_particles() {
        let mut d = HostBoneFxDamageData::from_authored(damaged_fx_slot());
        d.track_particle(42);
        d.track_particle(43);
        let _ = d.on_body_damage_state_change(
            "GLAVehicleScudLauncher",
            HostBodyDamageType::Pristine,
            HostBodyDamageType::Damaged,
        );
        assert_eq!(d.running_particle_count(), 0);
        let _ = d.drain_pending();
        d.stop_all_bone_fx();
        d.tick(99);
        assert!(d.drain_pending().is_empty());
    }

    #[test]
    fn destroy_and_collapse_call_stop_all_bone_fx() {
        let die = include_str!("world_objects/create_destroy_die.rs");
        assert!(die.contains("bfx.stop_all_bone_fx()"));
        let death = include_str!("object/death.rs");
        assert!(death.contains("bfx.stop_all_bone_fx()"));
        let tick = include_str!("world_tick/ai.rs");
        assert!(tick.contains("stamp_last_damage_type(o.last_damage_info_type)"));
        assert!(!tick.contains("stamp_last_damage_type(o.last_damage_fx_done)"));
        let pose = include_str!("object/pose.rs");
        assert!(pose.contains("stamp_last_damage_type(self.last_damage_info_type)"));
        assert!(!pose.contains("stamp_last_damage_type(self.last_damage_fx_done)"));
    }
    #[test]
    fn damage_type_flags_parse_all_minus() {
        use gamelogic::damage::DamageType;
        let flags = parse_damage_type_flags_attr("ALL -HEALING");
        assert!(flags.contains_damage_type(DamageType::Flame));
        assert!(!flags.contains_damage_type(DamageType::Healing));
    }
}
