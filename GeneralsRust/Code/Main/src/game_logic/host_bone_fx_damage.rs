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
    parse_fx_list_attr, parse_ocl_attr, parse_particle_attr, BONE_FX_MAX_BONES,
    BODY_DAMAGE_TYPE_COUNT,
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
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HostBoneFxAuthored {
    pub fx: [[Option<HostBoneFxSlot>; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    pub ocl: [[Option<HostBoneFxSlot>; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    pub psys: [[Option<HostBoneFxSlot>; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
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
        self.cur_state = new_state.ordinal();
        self.init_times(now);
        self.transitions = self.transitions.saturating_add(1);
        self.tick_due(now, old_state, new_state);
        self.pending.last().cloned()
    }

    pub fn tick(&mut self, now: i32) {
        let state = state_from_ordinal(self.cur_state);
        self.tick_due(now, state, state);
    }

    pub fn drain_pending(&mut self) -> Vec<HostBoneFxEvent> {
        std::mem::take(&mut self.pending)
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

    fn tick_due(
        &mut self,
        now: i32,
        old_state: HostBodyDamageType,
        new_state: HostBodyDamageType,
    ) {
        let idx = self.cur_state as usize;
        if idx >= BODY_DAMAGE_TYPE_COUNT {
            return;
        }
        for i in 0..BONE_FX_MAX_BONES {
            if due(self.next_fx_frame[idx][i], now) {
                if let Some(slot) = self.authored.fx[idx][i].clone() {
                    self.push_fire(old_state, new_state, &slot, FireKind::Fx);
                    self.next_fx_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_fx_frame[idx][i] = -1;
                }
            }
            if due(self.next_ocl_frame[idx][i], now) {
                if let Some(slot) = self.authored.ocl[idx][i].clone() {
                    self.push_fire(old_state, new_state, &slot, FireKind::Ocl);
                    self.next_ocl_frame[idx][i] = next_after_fire(&slot, now);
                } else {
                    self.next_ocl_frame[idx][i] = -1;
                }
            }
            if due(self.next_ps_frame[idx][i], now) {
                if let Some(slot) = self.authored.psys[idx][i].clone() {
                    self.push_fire(old_state, new_state, &slot, FireKind::Psys);
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
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(template_name)?;
    let module = definition
        .behavior_modules
        .iter()
        .find(|m| m.class_name.eq_ignore_ascii_case("BoneFXUpdate"))?;
    let mut authored = HostBoneFxAuthored::default();
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
pub fn play_bone_fx_event(
    ev: &HostBoneFxEvent,
    owner: u32,
    origin: Vec3,
    yaw: f32,
    model: &str,
    scale: f32,
) {
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
    if let Some(ps) = ev.particle_system.as_deref() {
        crate::game_logic::publish_host_fx_object(owner, origin, yaw, -1);
        let cpp_local = host_local_to_cpp(local);
        let _ = gamelogic::helpers::attach_particle_system_to_object_local(
            ps,
            owner,
            Some(&cpp_local),
            None,
        );
    }
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
        assert_eq!(ev.fx_list.as_deref(), Some("FX_ScudLauncherDamageTransition"));
        assert_eq!(ev.bone, "FXBone01");
        assert_eq!(d.transitions, 1);
        assert_eq!(d.drain_pending().len(), 1);
    }

    #[test]
    fn non_peel_skipped() {
        let mut d = HostBoneFxDamageData::default();
        assert!(d
            .on_body_damage_state_change(
                "AmericaTankCrusader",
                HostBodyDamageType::Pristine,
                HostBodyDamageType::Damaged,
            )
            .is_none());
    }

    #[test]
    fn leftover_parse_keeps_only_once_and_delay() {
        let info = parse_fx_list_attr(
            "Bone:Smoke01 OnlyOnce:No 15 45 FXList:FX_BuildingFireMedium",
        )
        .expect("parse");
        assert_eq!(info.base.loc_info.bone_name, "Smoke01");
        assert!(!info.base.only_once);
        assert_eq!(info.fx_name, "FX_BuildingFireMedium");
        assert!((info.base.game_logic_delay.min - 15.0).abs() < f32::EPSILON
            || info.base.game_logic_delay.min > 0.0);
    }

    #[test]
    fn invented_names_are_gone() {
        let src = include_str!("host_bone_fx_damage.rs");
        assert!(!src.contains("FX_ScudDamagedBoneFX"));
        assert!(!src.contains("ToxinLeakBonePSys"));
        assert!(!src.contains("ScudSmokeBonePSys"));
        assert!(!src.contains("StructureDamageBonePSys"));
    }
}
