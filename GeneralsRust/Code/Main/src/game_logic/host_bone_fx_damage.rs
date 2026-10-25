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
    #[serde(default)]
    slot_index: Option<usize>,
    #[serde(default)]
    kind: FireKind,
    pub fx_list: Option<String>,
    pub ocl: Option<String>,
    pub particle_system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
    #[serde(default)]
    pristine: BoneFxPristineCache,
    #[cfg(feature = "game_client")]
    #[serde(skip)]
    particle_owner: Option<game_client::effects::particle_manager::ParticleSystemOwner>,
}

/// C++ m_*BonePositions/m_bonesResolved: local translations survive body
/// state reentry, animation changes, and save/load. No presentation authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoneFxPristineCache {
    fx: [[[f32; 3]; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    ocl: [[[f32; 3]; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    psys: [[[f32; 3]; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    resolved: [bool; BODY_DAMAGE_TYPE_COUNT],
}
impl Default for BoneFxPristineCache {
    fn default() -> Self {
        Self {
            fx: [[[0.; 3]; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            ocl: [[[0.; 3]; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            psys: [[[0.; 3]; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            resolved: [false; BODY_DAMAGE_TYPE_COUNT],
        }
    }
}
/// A numeric ID alone never authorizes retirement in another manager namespace.
#[derive(Debug)]
pub(crate) struct BoneParticleRetirement {
    ids: Vec<u32>,
    #[cfg(feature = "game_client")]
    owner: Option<game_client::effects::particle_manager::ParticleSystemOwner>,
}
impl BoneParticleRetirement {
    pub(crate) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    pub(crate) fn empty() -> Self {
        Self {
            ids: Vec::new(),
            #[cfg(feature = "game_client")]
            owner: None,
        }
    }
}
impl Clone for HostBoneFxDamageData {
    fn clone(&self) -> Self {
        Self {
            transitions: self.transitions,
            last_fx: self.last_fx.clone(),
            pending: self.pending.clone(),
            authored: self.authored.clone(),
            next_fx_frame: self.next_fx_frame,
            next_ocl_frame: self.next_ocl_frame,
            next_ps_frame: self.next_ps_frame,
            cur_state: self.cur_state,
            active: self.active,
            last_damage_type: self.last_damage_type,
            particle_system_ids: self.particle_system_ids.clone(),
            pristine: self.pristine.clone(),
            #[cfg(feature = "game_client")]
            particle_owner: None,
        }
    }
}
pub(crate) fn retire_particle_ids(
    receipt: BoneParticleRetirement,
    owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
) {
    #[cfg(feature = "game_client")]
    if let Some(manager) = owner.particle_systems_mut() {
        if receipt.owner.as_ref() == Some(&manager.runtime_owner()) {
            for id in receipt.ids {
                manager.destroy_particle_system(id);
            }
        }
    }
    #[cfg(not(feature = "game_client"))]
    let _ = (receipt, owner);
}
#[cfg(feature = "game_client")]
impl crate::game_logic::GameLogic {
    pub(crate) fn adopt_loaded_bone_particle_systems(
        &mut self,
        manager: &game_client::effects::ParticleSystemManager,
        receipt: &game_client::effects::particle_manager::ParticleSystemRestoreReceipt,
    ) -> Result<(), String> {
        if !receipt.belongs_to(manager) {
            return Err("BoneFX particle receipt names another manager".into());
        }
        for object in self.objects.registered_values_mut() {
            if let Some(bfx) = object.bone_fx_damage.as_mut() {
                bfx.particle_system_ids.retain(|id| {
                    receipt.contains_system(*id)
                        && manager
                            .find_particle_system(*id)
                            .is_some_and(|system| system.attached_object_id() == object.id.0)
                });
                bfx.particle_owner = Some(manager.runtime_owner());
            }
        }
        Ok(())
    }
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
            pristine: BoneFxPristineCache::default(),
            #[cfg(feature = "game_client")]
            particle_owner: None,
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
        frame: u32,
    ) -> Option<HostBoneFxEvent> {
        if self.authored.is_empty() {
            return None;
        }
        let now = frame as i32;
        // C++ changeBodyDamageState: set state, killRunningParticleSystems, initTimes.
        self.cur_state = new_state.ordinal();
        self.kill_running_particle_systems();
        self.init_times(now);
        self.active = true;
        self.transitions = self.transitions.saturating_add(1);
        self.tick_due(now, old_state, new_state);
        self.pending.last().cloned()
    }

    pub(crate) fn prepare_body_state_retirement(
        &mut self,
        state: HostBodyDamageType,
    ) -> BoneParticleRetirement {
        // C++ publishes the new BoneFX body state before killing old systems.
        self.cur_state = state.ordinal();
        self.take_running_particle_systems()
    }
    pub(crate) fn change_body_state_with_random(
        &mut self,
        new_state: HostBodyDamageType,
        frame: u32,
        mut draw: impl FnMut(f32, f32) -> f32,
    ) {
        if self.authored.is_empty() {
            return;
        }
        self.cur_state = new_state.ordinal();
        // Source retirement receipt was already consumed by the Body receiver.
        self.init_times_with_random(frame as i32, &mut draw);
        // C++ changeBodyDamageState leaves m_active unchanged; first update
        // still performs its own initTimes when damage preceded that update.
        self.transitions = self.transitions.saturating_add(1);
    }
    fn init_times_with_random(&mut self, now: i32, draw: &mut dyn FnMut(f32, f32) -> f32) {
        let state = self.cur_state as usize;
        if state >= BODY_DAMAGE_TYPE_COUNT {
            return;
        }
        for i in 0..BONE_FX_MAX_BONES {
            self.next_fx_frame[state][i] =
                next_from_now_with_random(self.authored.fx[state][i].as_ref(), now, draw);
            self.next_ocl_frame[state][i] =
                next_from_now_with_random(self.authored.ocl[state][i].as_ref(), now, draw);
            self.next_ps_frame[state][i] =
                next_from_now_with_random(self.authored.psys[state][i].as_ref(), now, draw);
        }
    }
    fn source_due_event(&self, kind: FireKind, index: usize, now: i32) -> Option<HostBoneFxEvent> {
        let state = self.cur_state as usize;
        if state >= BODY_DAMAGE_TYPE_COUNT {
            return None;
        }
        let (deadline, slot) = match kind {
            FireKind::Fx => (
                self.next_fx_frame[state][index],
                self.authored.fx[state][index].as_ref(),
            ),
            FireKind::Ocl => (
                self.next_ocl_frame[state][index],
                self.authored.ocl[state][index].as_ref(),
            ),
            FireKind::Psys => (
                self.next_ps_frame[state][index],
                self.authored.psys[state][index].as_ref(),
            ),
        };
        if !due(deadline, now) {
            return None;
        }
        let slot = slot?;
        let name = nonempty_name(slot.name.trim());
        let state = state_from_ordinal(self.cur_state);
        Some(HostBoneFxEvent {
            old_state: state,
            new_state: state,
            bone: slot.bone.clone(),
            slot_index: Some(index),
            kind,
            fx_list: if matches!(kind, FireKind::Fx) {
                name.clone()
            } else {
                None
            },
            ocl: if matches!(kind, FireKind::Ocl) {
                name.clone()
            } else {
                None
            },
            particle_system: if matches!(kind, FireKind::Psys) {
                name
            } else {
                None
            },
        })
    }
    fn complete_source_event(
        &mut self,
        kind: FireKind,
        index: usize,
        now: i32,
        draw: &mut dyn FnMut(f32, f32) -> f32,
    ) {
        // C++ re-reads m_curBodyState AFTER the effect callback for both the
        // definition and destination timer; OCL may have changed it.
        let state = self.cur_state as usize;
        if state >= BODY_DAMAGE_TYPE_COUNT {
            return;
        }
        let (slot, timer) = match kind {
            FireKind::Fx => (
                self.authored.fx[state][index].as_ref(),
                &mut self.next_fx_frame[state][index],
            ),
            FireKind::Ocl => (
                self.authored.ocl[state][index].as_ref(),
                &mut self.next_ocl_frame[state][index],
            ),
            FireKind::Psys => (
                self.authored.psys[state][index].as_ref(),
                &mut self.next_ps_frame[state][index],
            ),
        };
        *timer = match slot {
            Some(slot) if !slot.only_once => {
                now.wrapping_add(delay_with_random(slot.delay_min, slot.delay_max, draw))
            }
            _ => -1,
        };
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
    pub(crate) fn take_running_particle_systems(&mut self) -> BoneParticleRetirement {
        BoneParticleRetirement {
            ids: std::mem::take(&mut self.particle_system_ids),
            #[cfg(feature = "game_client")]
            owner: self.particle_owner.take(),
        }
    }
    pub(crate) fn track_particle_with_owner(
        &mut self,
        id: u32,
        owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        if id == 0 {
            return;
        }
        #[cfg(feature = "game_client")]
        if let Some(manager) = owner.particle_systems_mut() {
            let namespace = manager.runtime_owner();
            if self.particle_owner.as_ref() != Some(&namespace) {
                self.particle_system_ids.clear();
            }
            self.particle_owner = Some(namespace);
            self.particle_system_ids.push(id);
        }
        #[cfg(not(feature = "game_client"))]
        let _ = owner;
    }
    /// Detached component state has no native resources. Production always
    /// consumes the source receipt with kill/stop_with_owner below.
    pub fn kill_running_particle_systems(&mut self) {
        self.particle_system_ids.clear();
    }
    pub(crate) fn kill_running_particle_systems_with_owner(
        &mut self,
        owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        retire_particle_ids(self.take_running_particle_systems(), owner);
    }
    pub(crate) fn stop_all_bone_fx_with_owner(
        &mut self,
        owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        self.next_fx_frame = unset_schedule();
        self.next_ocl_frame = unset_schedule();
        self.next_ps_frame = unset_schedule();
        self.kill_running_particle_systems_with_owner(owner);
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
                    self.push_fire(old_state, new_state, &slot, FireKind::Fx, i);
                    self.next_fx_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_fx_frame[idx][i] = -1;
                }
            }
            if due(self.next_ocl_frame[idx][i], now) {
                if let Some(slot) = self.authored.ocl[idx][i].clone() {
                    self.push_fire(old_state, new_state, &slot, FireKind::Ocl, i);
                    self.next_ocl_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_ocl_frame[idx][i] = -1;
                }
            }
            if due(self.next_ps_frame[idx][i], now) {
                if let Some(slot) = self.authored.psys[idx][i].clone() {
                    self.push_fire(old_state, new_state, &slot, FireKind::Psys, i);
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
        slot_index: usize,
    ) {
        let name = slot.name.trim();
        let ev = match kind {
            FireKind::Fx => HostBoneFxEvent {
                old_state,
                new_state,
                bone: slot.bone.clone(),
                slot_index: Some(slot_index),
                kind,
                fx_list: nonempty_name(name),
                ocl: None,
                particle_system: None,
            },
            FireKind::Ocl => HostBoneFxEvent {
                old_state,
                new_state,
                bone: slot.bone.clone(),
                slot_index: Some(slot_index),
                kind,
                fx_list: None,
                ocl: nonempty_name(name),
                particle_system: None,
            },
            FireKind::Psys => HostBoneFxEvent {
                old_state,
                new_state,
                bone: slot.bone.clone(),
                slot_index: Some(slot_index),
                kind,
                fx_list: None,
                ocl: None,
                particle_system: nonempty_name(name),
            },
        };
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
enum FireKind {
    #[default]
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

fn delay_with_random(lo: f32, hi: f32, draw: &mut dyn FnMut(f32, f32) -> f32) -> i32 {
    if lo == hi {
        lo as i32
    } else {
        draw(lo, hi) as i32
    }
}
fn next_from_now_with_random(
    slot: Option<&HostBoneFxSlot>,
    now: i32,
    draw: &mut dyn FnMut(f32, f32) -> f32,
) -> i32 {
    match slot {
        Some(slot) if !slot.bone.is_empty() => {
            now.wrapping_add(delay_with_random(slot.delay_min, slot.delay_max, draw))
        }
        _ => -1,
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

impl crate::game_logic::GameLogic {
    pub(crate) fn update_owned_bone_fx_with_effects(
        &mut self,
        id: crate::game_logic::ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let now = self.frame as i32;
        if let Some(object) = self.objects.get_mut(&id) {
            if object.bone_fx_damage.is_none() {
                object.bone_fx_damage = HostBoneFxDamageData::from_template(&object.template_name);
            }
            if let Some(bfx) = object.bone_fx_damage.as_mut() {
                bfx.stamp_last_damage_type(object.last_damage_info_type);
                if !bfx.active {
                    bfx.init_times_with_random(now, &mut |lo, hi| {
                        self.logic_random.next_real(lo, hi)
                    });
                    bfx.active = true;
                }
            }
        }
        // Old host snapshots may contain already-admitted pending receipts.
        // Consume them once; new source callbacks never populate this list.
        let saved = self
            .objects
            .get_mut(&id)
            .and_then(|object| object.bone_fx_damage.as_mut())
            .map(|bfx| bfx.drain_pending())
            .unwrap_or_default();
        for event in saved {
            self.execute_owned_bone_fx_step(id, &event, effects)?;
        }
        for index in 0..BONE_FX_MAX_BONES {
            for kind in [FireKind::Fx, FireKind::Ocl, FireKind::Psys] {
                let event = self
                    .objects
                    .get(&id)
                    .and_then(|object| object.bone_fx_damage.as_ref())
                    .and_then(|bfx| bfx.source_due_event(kind, index, now));
                let Some(event) = event else {
                    continue;
                };
                self.execute_owned_bone_fx_step(id, &event, effects)?;
                let frame = self.frame as i32;
                if let Some(bfx) = self
                    .objects
                    .get_mut(&id)
                    .and_then(|object| object.bone_fx_damage.as_mut())
                {
                    match kind {
                        FireKind::Psys => {
                            bfx.complete_source_event(kind, index, frame, &mut |lo, hi| {
                                self.session_view.client_random.next_real(lo, hi)
                            })
                        }
                        _ => bfx.complete_source_event(kind, index, frame, &mut |lo, hi| {
                            self.logic_random.next_real(lo, hi)
                        }),
                    }
                }
            }
        }
        Ok(())
    }
    fn execute_owned_bone_fx_step(
        &mut self,
        id: crate::game_logic::ObjectId,
        event: &HostBoneFxEvent,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(system) = play_bone_fx_event_with_effects(self, event, id.0, effects)? {
            if let Some(bfx) = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.bone_fx_damage.as_mut())
            {
                bfx.track_particle_with_owner(system, effects);
            }
        }
        Ok(())
    }
}

fn pristine_bone_local(
    source: &crate::game_logic::NativeSlaveSource,
    bone: &str,
    effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
) -> gamelogic::GameLogicResult<[f32; 3]> {
    if bone.is_empty() {
        return Ok([0.; 3]);
    }
    let bones = if let Some(assets) = effects.pristine_bones {
        assets.pristine_bone_transforms(&source, bone, 0, 1)
    } else {
        #[cfg(feature = "game_client")]
        {
            effects
                .view
                .as_deref_mut()
                .ok_or_else(|| "BoneFX requires receiving pristine model assets".to_owned())
                .and_then(|view| view.pristine_client_bone_transforms(&source, bone, 0, 1))
        }
        #[cfg(not(feature = "game_client"))]
        {
            Err("BoneFX requires receiving pristine model assets".to_owned())
        }
    }
    .map_err(gamelogic::GameLogicError::Configuration)?;
    Ok(bones
        .first()
        .map(|matrix| matrix.w_axis.truncate().to_array())
        .unwrap_or([0.; 3]))
}

fn resolve_pristine_state(
    world: &mut crate::game_logic::GameLogic,
    owner: u32,
    state: usize,
    effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
) -> gamelogic::GameLogicResult<()> {
    if state >= BODY_DAMAGE_TYPE_COUNT {
        return Err(gamelogic::GameLogicError::Configuration(
            "invalid BoneFX cache state".into(),
        ));
    }
    let object = world
        .host_object(crate::game_logic::ObjectId(owner))
        .ok_or(gamelogic::GameLogicError::InvalidObject(owner))?;
    let bfx = object.bone_fx_damage.as_ref().ok_or_else(|| {
        gamelogic::GameLogicError::Configuration("BoneFX current owner data absent".into())
    })?;
    if bfx.pristine.resolved[state] {
        return Ok(());
    }
    let authored = bfx.authored.clone();
    let source = world
        .current_native_slave_source(
            crate::game_logic::ObjectId(owner),
            world.local_player_id().unwrap_or(0),
        )
        .map_err(|error| {
            gamelogic::GameLogicError::Configuration(format!("BoneFX source {owner}: {error}"))
        })?;
    let mut fx = [[0.; 3]; BONE_FX_MAX_BONES];
    let mut ocl = fx;
    let mut psys = fx;
    for i in 0..BONE_FX_MAX_BONES {
        if let Some(slot) = &authored.fx[state][i] {
            fx[i] = pristine_bone_local(&source, &slot.bone, effects)?;
        }
        if let Some(slot) = &authored.ocl[state][i] {
            ocl[i] = pristine_bone_local(&source, &slot.bone, effects)?;
        }
        if let Some(slot) = &authored.psys[state][i] {
            psys[i] = pristine_bone_local(&source, &slot.bone, effects)?;
        }
    }
    let object = world
        .host_object_mut(crate::game_logic::ObjectId(owner))
        .ok_or(gamelogic::GameLogicError::InvalidObject(owner))?;
    let bfx = object.bone_fx_damage.as_mut().ok_or_else(|| {
        gamelogic::GameLogicError::Configuration("BoneFX current owner data absent".into())
    })?;
    bfx.pristine.fx[state] = fx;
    bfx.pristine.ocl[state] = ocl;
    bfx.pristine.psys[state] = psys;
    bfx.pristine.resolved[state] = true;
    Ok(())
}

/// Play leftover-authored FXList / OCL / PSys at the bone.
/// Returns leftover particle-system id when a PSys was attached (C++ `m_particleSystemIDs`).
/// Actual BoneFX update borrows the receiving native pose, FX, factory and particles.
/// C++ BoneFXUpdate.cpp:375-448: pristine local bone, FX, OCL, then attached PSys.
pub(crate) fn play_bone_fx_event_with_effects(
    world: &mut crate::game_logic::GameLogic,
    ev: &HostBoneFxEvent,
    owner: u32,
    effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
) -> gamelogic::GameLogicResult<Option<u32>> {
    let state = ev.new_state.ordinal() as usize;
    let kind = if ev.ocl.is_some() {
        FireKind::Ocl
    } else if ev.particle_system.is_some() {
        FireKind::Psys
    } else {
        ev.kind
    };
    // Older host snapshots saved names but no row receipt. Recover the authored
    // row before acquiring the canonical state cache; new saves carry exact row.
    let slot_index = if let Some(index) = ev.slot_index {
        index
    } else {
        let bfx = world
            .host_object(crate::game_logic::ObjectId(owner))
            .and_then(|object| object.bone_fx_damage.as_ref())
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration("BoneFX saved event owner absent".into())
            })?;
        if state >= BODY_DAMAGE_TYPE_COUNT {
            return Err(gamelogic::GameLogicError::Configuration(
                "invalid saved BoneFX state".into(),
            ));
        }
        let row = match kind {
            FireKind::Fx => &bfx.authored.fx[state],
            FireKind::Ocl => &bfx.authored.ocl[state],
            FireKind::Psys => &bfx.authored.psys[state],
        };
        row.iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.bone == ev.bone))
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration("saved BoneFX row absent".into())
            })?
    };
    if slot_index >= BONE_FX_MAX_BONES {
        return Err(gamelogic::GameLogicError::Configuration(
            "invalid BoneFX slot index".into(),
        ));
    }
    resolve_pristine_state(world, owner, state, effects)?;
    let object = world
        .host_object(crate::game_logic::ObjectId(owner))
        .ok_or(gamelogic::GameLogicError::InvalidObject(owner))?;
    let bfx = object.bone_fx_damage.as_ref().ok_or_else(|| {
        gamelogic::GameLogicError::Configuration("BoneFX current owner data absent".into())
    })?;
    let (position, allowed) = match kind {
        FireKind::Fx => (
            bfx.pristine.fx[state][slot_index],
            bfx.damage_type_allowed(bfx.authored.damage_fx_types),
        ),
        FireKind::Ocl => (
            bfx.pristine.ocl[state][slot_index],
            bfx.damage_type_allowed(bfx.authored.damage_ocl_types),
        ),
        FireKind::Psys => (
            bfx.pristine.psys[state][slot_index],
            bfx.damage_type_allowed(bfx.authored.damage_particle_types),
        ),
    };
    if !allowed {
        return Ok(None);
    }
    if !position.iter().all(|value| value.is_finite()) {
        return Err(gamelogic::GameLogicError::Configuration(
            "nonfinite saved BoneFX cache".into(),
        ));
    }
    let local = Vec3::from_array(position);
    // Pristine bone is C++ Z-up, whereas the canonical host Object is Y-up.
    let host_local = Vec3::new(local.x, local.z, local.y);
    let effect_position = {
        let Some(object) = world.host_object(crate::game_logic::ObjectId(owner)) else {
            return Ok(None);
        };
        object.get_transform_matrix().transform_point3(host_local)
    };
    #[cfg(feature = "game_client")]
    if let Some(fx) = ev.fx_list.as_deref() {
        world.dispatch_fx_list_at_host_position_with_effects(fx, effect_position, effects);
    }
    if let Some(ocl) = ev.ocl.as_deref() {
        // Reacquire after FX: no ObjectStore loan survives a factory callback.
        let Some(object) = world.host_object(crate::game_logic::ObjectId(owner)) else {
            return Ok(None);
        };
        let (team, veterancy, yaw, velocity) = (
            object.team,
            object.experience.level,
            object.get_orientation(),
            object.movement.velocity,
        );
        world.execute_parsed_weapon_ocl_at_with_effects(
            ocl,
            Some(crate::game_logic::ObjectId(owner)),
            team,
            veterancy,
            yaw,
            velocity,
            effect_position,
            effects,
        );
    }
    #[cfg(feature = "game_client")]
    {
        let Some(ps) = ev.particle_system.as_deref() else {
            return Ok(None);
        };
        let Some(object) = world.host_object(crate::game_logic::ObjectId(owner)) else {
            return Ok(None);
        };
        let hidden = object.drawable_hidden;
        let manager = effects
            .view
            .as_deref_mut()
            .and_then(|view| view.particle_systems_mut())
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "BoneFX requires receiving particle manager".into(),
                )
            })?;
        manager
            .admit_effective_definitions()
            .map_err(|error| gamelogic::GameLogicError::Configuration(error.to_string()))?;
        let Some(template) = manager.find_template(ps) else {
            return Ok(None);
        };
        let id = manager
            .create_particle_system(&template, true)
            .map_err(|error| gamelogic::GameLogicError::Configuration(error.to_string()))?;
        let system = manager.find_particle_system_mut(id).ok_or_else(|| {
            gamelogic::GameLogicError::Configuration("created BoneFX particle system absent".into())
        })?;
        system.set_position(local);
        system.attach_to_object(owner);
        if hidden {
            system.stop();
        }
        Ok(Some(id))
    }
    #[cfg(not(feature = "game_client"))]
    Ok(None)
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
                0,
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
                0,
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
                0,
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
                0,
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
            0,
        );
        assert_eq!(d.running_particle_count(), 0);
        let _ = d.drain_pending();
        d.stop_all_bone_fx();
        d.tick(99);
        assert!(d.drain_pending().is_empty());
    }

    #[test]
    fn damage_type_flags_parse_all_minus() {
        use gamelogic::damage::DamageType;
        let flags = parse_damage_type_flags_attr("ALL -HEALING");
        assert!(flags.contains_damage_type(DamageType::Flame));
        assert!(!flags.contains_damage_type(DamageType::Healing));
    }
}

#[cfg(test)]
mod owned_body_frame_tests {
    use super::*;
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::object::DamageHitContext;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};

    fn world(frame: u32) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.set_current_frame(frame as u64);
        world.add_player(Player::new(0, Team::USA, "BodyClockOwner", true));
        let mut template = ThingTemplate::new("OwnedBodyClockTank");
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object_for_player("OwnedBodyClockTank", 0, Vec3::ZERO)
            .unwrap();
        let mut authored = HostBoneFxAuthored::default();
        for (state, delay, name) in [
            (HostBodyDamageType::Pristine, 7.0, "BodyClockPristineFx"),
            (HostBodyDamageType::Damaged, 5.0, "BodyClockDamagedFx"),
            (
                HostBodyDamageType::ReallyDamaged,
                5.0,
                "BodyClockReallyDamagedFx",
            ),
        ] {
            authored.fx[state.ordinal() as usize][0] = Some(HostBoneFxSlot {
                bone: "BodyClockBone".into(),
                only_once: true,
                delay_min: delay,
                delay_max: delay,
                name: name.into(),
            });
        }
        world.host_object_mut(id).unwrap().bone_fx_damage =
            Some(HostBoneFxDamageData::from_authored(authored));
        (world, id)
    }
    fn damage(world: &mut GameLogic, id: ObjectId, amount: f32, kind: DamageType) {
        world
            .apply_owned_damage(
                id,
                amount,
                None,
                kind,
                HostDeathType::Normal,
                None,
                &DamageHitContext::default(),
            )
            .unwrap();
    }
    fn deadline(world: &GameLogic, id: ObjectId, state: HostBodyDamageType) -> i32 {
        world
            .host_object(id)
            .unwrap()
            .bone_fx_damage
            .as_ref()
            .unwrap()
            .next_fx_frame[state.ordinal() as usize][0]
    }
    fn fire_due(world: &mut GameLogic, id: ObjectId, frame: u32) -> Vec<String> {
        world.set_current_frame(frame as u64);
        let bone = world
            .host_object_mut(id)
            .unwrap()
            .bone_fx_damage
            .as_mut()
            .unwrap();
        bone.tick(frame as i32);
        bone.drain_pending()
            .into_iter()
            .filter_map(|event| event.fx_list)
            .collect()
    }

    #[test]
    fn actual_body_damage_schedules_bone_fx_from_receiving_frames_not_peer_identity() {
        let (mut owner, id) = world(100);
        let (mut peer, peer_id) = world(900);
        assert_eq!(id, peer_id);
        damage(&mut owner, id, 40.0, DamageType::Unresistable);
        assert_eq!(owner.host_object(id).unwrap().health.current, 60.0);
        assert_eq!(
            owner.host_object(id).unwrap().body_damage_state,
            HostBodyDamageType::Damaged
        );
        assert_eq!(deadline(&owner, id, HostBodyDamageType::Damaged), 105);
        damage(&mut peer, peer_id, 40.0, DamageType::Unresistable);
        assert_eq!(deadline(&peer, peer_id, HostBodyDamageType::Damaged), 905);
        assert_eq!(deadline(&owner, id, HostBodyDamageType::Damaged), 105);
        assert!(fire_due(&mut owner, id, 104).is_empty());
        assert_eq!(fire_due(&mut owner, id, 105), vec!["BodyClockDamagedFx"]);
        assert!(fire_due(&mut owner, id, 106).is_empty());
        assert_eq!(deadline(&peer, peer_id, HostBodyDamageType::Damaged), 905);
    }

    #[test]
    fn actual_body_healing_and_saved_bone_fx_future_keep_receiving_deadline() {
        let (mut owner, id) = world(100);
        let (mut peer, peer_id) = world(900);
        damage(&mut owner, id, 40.0, DamageType::Unresistable);
        damage(&mut peer, peer_id, 40.0, DamageType::Unresistable);
        owner.set_current_frame(222);
        // DAMAGE_HEALING uses the same production Body callback path, including
        // lastDamageInfo and lastHealingTimestamp, before visual evaluation.
        damage(&mut owner, id, 40.0, DamageType::Healing);
        assert_eq!(owner.host_object(id).unwrap().health.current, 100.0);
        assert_eq!(
            owner.host_object(id).unwrap().last_healing_timestamp,
            Some(222)
        );
        assert_eq!(deadline(&owner, id, HostBodyDamageType::Pristine), 229);
        assert_eq!(deadline(&peer, peer_id, HostBodyDamageType::Damaged), 905);
        let builder = crate::save_load::SnapshotBuilder::new();
        let saved = builder.create_world_snapshot(&owner).unwrap();
        builder.restore_from_snapshot(&saved, &mut peer).unwrap();
        assert_eq!(deadline(&peer, id, HostBodyDamageType::Pristine), 229);
        assert!(fire_due(&mut owner, id, 228).is_empty());
        assert!(fire_due(&mut peer, id, 228).is_empty());
        assert_eq!(fire_due(&mut owner, id, 229), vec!["BodyClockPristineFx"]);
        assert_eq!(fire_due(&mut peer, id, 229), vec!["BodyClockPristineFx"]);
    }

    #[test]
    fn actual_undead_body_second_life_recalculates_bone_fx_at_same_impact_frame() {
        let (mut owner, id) = world(500);
        let (peer, peer_id) = world(900);
        assert_eq!(id, peer_id);
        owner.host_object_mut(id).unwrap().is_battle_bus_transport = true;
        // CPP UndeadBody excludes UNRESISTABLE; PENALTY is ordinary HP.
        damage(&mut owner, id, 200.0, DamageType::Penalty);
        let object = owner.host_object(id).unwrap();
        assert_eq!(
            object.health.current,
            crate::game_logic::host_battle_bus::BATTLE_BUS_SECOND_LIFE_MAX_HEALTH
        );
        assert!(!object.status.destroyed && !object.status.effectively_dead);
        let body = object.battle_bus_body.as_ref().unwrap();
        assert!(body.is_second_life && body.is_in_first_death);
        assert_eq!(
            body.ground_check_frame,
            500 + crate::game_logic::host_battle_bus::BATTLE_BUS_GROUND_CHECK_DELAY_FRAMES
        );
        assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
        assert_eq!(deadline(&owner, id, HostBodyDamageType::Pristine), 507);
        assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
        assert!(peer.host_object(peer_id).unwrap().battle_bus_body.is_none());
        assert!(fire_due(&mut owner, id, 506).is_empty());
        assert_eq!(fire_due(&mut owner, id, 507), vec!["BodyClockPristineFx"]);
    }
}
