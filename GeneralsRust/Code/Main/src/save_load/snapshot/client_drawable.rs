//! Renderer-owned client Drawable state carried beside a world snapshot.
//!
//! These records intentionally contain only stable, source-facing identities
//! and in-flight visual values.  They do **not** contain WGPU resources,
//! process-local animation indices, hierarchy pivot indices, or authored W3D
//! topology.  A fresh presentation frame must validate every record against
//! its selected Draw module before the renderer imports it.
//!
//! Wire ownership lives with the snapshot migration layer.  This file is a
//! serde-only DTO boundary so graphics can convert to/from it without making
//! the save schema depend on renderer implementation types.

use serde::{Deserialize, Serialize};

/// All renderer-local Drawable state persisted for one logical world.
///
/// Capture must keep this vector in deterministic `(object_id,
/// draw_module_index)` order.  Restore validates every entry independently;
/// one incompatible asset or Draw state cannot make another object's visual
/// timeline invalid.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClientDrawableWorldSnapshot {
    #[serde(default)]
    pub drawables: Vec<ClientDrawableStateSnapshot>,
}

/// Stable, renderer-owned state for one exact frozen W3D draw module.
///
/// `object_id` plus `draw_module_index` identifies the timeline within one
/// world.  The remaining source identity fields ensure an ID reused by a
/// different object, selected condition state, model, or animation cannot
/// resume a stale visual timeline after load.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClientDrawableStateSnapshot {
    /// Numeric `ObjectId`; zero is not a valid active Main object identity.
    #[serde(default)]
    pub object_id: u32,
    /// Exact frozen `AuthoredDrawModel::module_index`.
    #[serde(default)]
    pub draw_module_index: u32,
    /// Exact source Thing identity, never inferred from model basename.
    #[serde(default)]
    pub source_template_name: String,
    /// Exact frozen `AuthoredDrawModel::model_key`.
    #[serde(default)]
    pub model_key: String,
    /// Exact frozen `AuthoredDrawModel::selected_condition_state_index`.
    #[serde(default)]
    pub selected_condition_state_index: u32,
    /// `None` is deliberate bind pose.  It must never turn into local clip
    /// zero while importing a restored renderer cache.
    #[serde(default)]
    pub animation: Option<ClientDrawableAnimationSnapshot>,
    /// Last exact accepted-discharge sequence observed by this Draw module.
    /// Zero means the module has not observed a post-discharge baseline.
    #[serde(default)]
    pub last_seen_weapon_discharge_sequence: u64,
    /// C++ `m_weaponRecoilInfoVec[WEAPONSLOT_COUNT]`-shaped client state.
    /// Fresh W3D topology validates the exact per-slot vector lengths before
    /// import; the topology itself remains asset/frame authority.
    #[serde(default)]
    pub recoil_slots: [Vec<ClientDrawableRecoilSnapshot>; 3],
}

impl ClientDrawableStateSnapshot {
    /// Minimal source identity validation shared by capture/import adapters.
    ///
    /// This deliberately cannot validate hierarchy compatibility or barrel
    /// topology: those are fresh W3D asset facts, not durable snapshot data.
    pub fn has_stable_source_identity(&self) -> bool {
        self.object_id != 0
            && !self.source_template_name.trim().is_empty()
            && !self.model_key.trim().is_empty()
    }

    /// Snapshot-local numeric validation.  A failed check means import must
    /// discard this one visual record and remain bind-pose/idle rather than
    /// feeding NaN into a palette or recoil transform.
    pub fn has_finite_visual_values(&self) -> bool {
        self.animation
            .as_ref()
            .map_or(true, ClientDrawableAnimationSnapshot::has_finite_values)
            && self
                .recoil_slots
                .iter()
                .flatten()
                .all(ClientDrawableRecoilSnapshot::has_finite_values)
    }
}

/// Exact selected HAnim playback retained separately from its process-local
/// W3D binding key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientDrawableAnimationSnapshot {
    /// Full frozen `Hierarchy.Animation` identity, not a local animation
    /// vector index or an unqualified clip basename.
    pub hierarchy_animation: String,
    /// Continuous W3D source frame.
    pub frame: f32,
    /// Only modes with active Main playback semantics are representable.
    pub mode: ClientDrawableAnimationMode,
}

impl Default for ClientDrawableAnimationSnapshot {
    fn default() -> Self {
        Self {
            hierarchy_animation: String::new(),
            frame: 0.0,
            mode: ClientDrawableAnimationMode::Manual,
        }
    }
}

impl ClientDrawableAnimationSnapshot {
    pub fn has_finite_values(&self) -> bool {
        !self.hierarchy_animation.trim().is_empty() && self.frame.is_finite() && self.frame >= 0.0
    }
}

/// Supported C++ W3DModelDraw playback modes.
///
/// Ping-pong and unsupported source modes have no complete active Main state
/// representation, so they are intentionally absent from the durable DTO.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientDrawableAnimationMode {
    #[default]
    Manual,
    Loop,
    Once,
    LoopBackwards,
    OnceBackwards,
}

/// C++ `W3DModelDraw::WeaponRecoilInfo::RecoilState`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientDrawableRecoilPhase {
    #[default]
    Idle,
    RecoilStart,
    Recoil,
    Settle,
}

/// One in-flight C++ `WeaponRecoilInfo` record.
///
/// Kinematics and exact affected pivot are deliberately not duplicated here:
/// a fresh source-selected Draw state and validated W3D hierarchy own them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientDrawableRecoilSnapshot {
    pub phase: ClientDrawableRecoilPhase,
    pub shift: f32,
    pub recoil_rate: f32,
}

impl Default for ClientDrawableRecoilSnapshot {
    fn default() -> Self {
        Self {
            phase: ClientDrawableRecoilPhase::Idle,
            shift: 0.0,
            recoil_rate: 0.0,
        }
    }
}

impl ClientDrawableRecoilSnapshot {
    pub fn has_finite_values(&self) -> bool {
        self.shift.is_finite() && self.recoil_rate.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_drawable_dto_defaults_to_no_visual_replay() {
        let world = ClientDrawableWorldSnapshot::default();
        assert!(world.drawables.is_empty());

        let drawable = ClientDrawableStateSnapshot::default();
        assert!(!drawable.has_stable_source_identity());
        assert!(drawable.has_finite_visual_values());
        assert_eq!(drawable.last_seen_weapon_discharge_sequence, 0);
        assert!(drawable.recoil_slots.iter().all(Vec::is_empty));
    }

    #[test]
    fn client_drawable_dto_rejects_incomplete_identity_and_nonfinite_visual_values() {
        let mut drawable = ClientDrawableStateSnapshot {
            object_id: 7,
            draw_module_index: 2,
            source_template_name: "GLATankScorpion".into(),
            model_key: "UVLiteTank".into(),
            selected_condition_state_index: 4,
            animation: Some(ClientDrawableAnimationSnapshot {
                hierarchy_animation: "UVLiteTank.UVLiteTank".into(),
                frame: 3.5,
                mode: ClientDrawableAnimationMode::Loop,
            }),
            last_seen_weapon_discharge_sequence: 19,
            recoil_slots: std::array::from_fn(|_| Vec::new()),
        };
        assert!(drawable.has_stable_source_identity());
        assert!(drawable.has_finite_visual_values());

        drawable.recoil_slots[0].push(ClientDrawableRecoilSnapshot {
            phase: ClientDrawableRecoilPhase::Recoil,
            shift: f32::NAN,
            recoil_rate: 1.0,
        });
        assert!(!drawable.has_finite_visual_values());

        drawable.recoil_slots[0].clear();
        drawable.source_template_name.clear();
        assert!(!drawable.has_stable_source_identity());
    }
}
