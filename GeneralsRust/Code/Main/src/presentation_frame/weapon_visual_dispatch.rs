//! Frozen C++ `Weapon::fireWeaponTemplate` → `Drawable` visual dispatch.
//!
//! A plain accepted-shot marker is deliberately not enough to drive W3D
//! recoil.  In the retail path `Weapon::fireWeaponTemplate` first selects an
//! FX pointer, may replace it with null while the *instance* suspend-FX frame
//! is active, and may avoid calling `Drawable` altogether for an undetected
//! stealth shooter.  When Drawable is called, it visits draw modules in
//! declaration order.  A non-null FireFX stops that traversal only at the
//! first module which actually handled an exact FireFX bone; recoil/muzzle
//! handling already performed by preceding modules remains intact.  A null
//! FX never stops traversal, so every compatible W3D module gets its recoil
//! callback.
//!
//! This module is intentionally a pure contract, not a renderer hook.  Main
//! does not yet capture the pre-mutation source Draw state, per-instance
//! `m_suspendFXFrame`, or a monotonic Draw-state revision at every accepted
//! weapon path.  Callers must therefore keep live recoil injection disabled
//! until they can construct these records from that exact source boundary.
//! In particular, do not reconstruct a plan from a later presentation frame:
//! a C++ `setModelState` rebuild clears its recoil records, including an
//! A→B→A return to the same model/state names.

use super::*;

/// What C++ supplied to `Drawable::handleWeaponFireFX` after all source-side
/// gate decisions were made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrozenWeaponVisualFxRoute {
    /// The undetected-stealth gate bypassed Drawable entirely.  This is not a
    /// null-FX broadcast: no recoil, muzzle, or module FireFX callback ran.
    DrawableSuppressed,
    /// Drawable was called with `fxl == NULL`, either because no selected
    /// FireFX existed or because the per-instance suspend-FX frame had not
    /// elapsed.  C++ still broadcasts recoil/muzzle callbacks to all modules.
    BroadcastWithoutFireFx,
    /// Drawable was called with a non-null FireFX pointer.  Traversal stops
    /// after the first module that actually handles its FireFX bone.
    BroadcastWithFireFx,
}

/// The source-side facts C++ samples in `WeaponTemplate::fireWeaponTemplate`
/// before it mutates the firing weapon/object.  This is deliberately not a
/// team-locality approximation: `source_is_locally_controlled` is the exact
/// source-object controller result from C++ `Object::isLocallyControlled()`.
///
/// `selected_fx_is_present` is the already-selected non-null `FXList*` from
/// either `getFireFX(veterancy)` or `getProjectileDetonateFX(veterancy)`.  The
/// future capture adapter must resolve that selection before it supplies this
/// record; this contract only applies C++'s suspend and stealth gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenWeaponVisualSourceGate {
    /// `sourceObj->getDrawable() != NULL`; a missing Drawable means the whole
    /// C++ visual block at `Weapon.cpp:889` did not run.
    pub source_has_drawable: bool,
    pub source_is_locally_controlled: bool,
    pub source_is_stealthed: bool,
    pub source_is_detected: bool,
    pub source_is_disguised: bool,
    pub source_is_mine: bool,
    pub weapon_plays_fx_when_stealthed: bool,
    pub selected_fx_is_present: bool,
    /// The exact C++ logic frame at the accepted call site.
    pub logic_frame: u32,
    /// The owning weapon instance's `m_suspendFXFrame`, initialized at weapon
    /// construction/copy rather than recomputed from a template at delivery.
    pub suspend_fx_frame: u32,
}

impl FrozenWeaponVisualSourceGate {
    /// Reproduce the source gate in C++ `Weapon.cpp:889-940`.
    ///
    /// `None` is intentionally distinct from `DrawableSuppressed`: no
    /// Drawable existed, whereas a suppressed stealth source had a Drawable
    /// but deliberately bypassed it and pretended the FX was handled.
    pub fn fx_route(&self) -> Option<FrozenWeaponVisualFxRoute> {
        if !self.source_has_drawable {
            return None;
        }

        // C++ first nulls the selected pointer during the per-instance delay.
        let fx_is_null = !self.selected_fx_is_present || self.logic_frame < self.suspend_fx_frame;

        // Then it bypasses Drawable entirely only for an undetected, remote,
        // undisguised, non-mine source whose weapon did not opt in.
        let suppress_drawable = !self.source_is_locally_controlled
            && self.source_is_stealthed
            && !self.source_is_detected
            && !self.source_is_disguised
            && !self.source_is_mine
            && !self.weapon_plays_fx_when_stealthed;
        if suppress_drawable {
            return Some(FrozenWeaponVisualFxRoute::DrawableSuppressed);
        }

        Some(if fx_is_null {
            FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx
        } else {
            FrozenWeaponVisualFxRoute::BroadcastWithFireFx
        })
    }
}

/// Stable identity of the exact source `W3DModelDraw` state that received a
/// callback.  `draw_state_revision` is deliberately separate from model and
/// condition-state names: an A→B→A state transition must invalidate an old
/// recoil event even when those visible names become equal again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenWeaponVisualDrawState {
    /// C++ Drawable module declaration index, not a render-list position.
    pub draw_module_index: u32,
    /// Canonical Object INI identity used to select the module.
    pub source_template_name: String,
    /// Exact selected W3D model key.
    pub model_key: String,
    /// Exact selected source `ConditionState` index inside this module.
    pub selected_condition_state_index: u32,
    /// Nonzero monotonic selection/rebuild generation for this Draw module.
    pub draw_state_revision: u64,
}

impl FrozenWeaponVisualDrawState {
    /// A plan is only safe when it carries an identity that can distinguish a
    /// later state rebuild from the state that accepted the C++ callback.
    pub fn is_valid(&self) -> bool {
        !self.source_template_name.trim().is_empty()
            && !self.model_key.trim().is_empty()
            && self.draw_state_revision != 0
    }

    /// C++ NameKey-based source/model comparison is case-insensitive, while
    /// the selected state index and per-selection generation are exact.
    pub fn matches_current(&self, current: &Self) -> bool {
        self.draw_module_index == current.draw_module_index
            && self
                .source_template_name
                .eq_ignore_ascii_case(current.source_template_name.trim())
            && self
                .model_key
                .eq_ignore_ascii_case(current.model_key.trim())
            && self.selected_condition_state_index == current.selected_condition_state_index
            && self.draw_state_revision == current.draw_state_revision
    }
}

/// Immutable identity of one accepted C++ `Weapon::privateFireWeapon` call.
///
/// `world_epoch` and `source_object_generation` are required because raw
/// ObjectID values can be reused across a reset/restore.  They are not yet
/// available on Main's existing bare `WeaponDischarged` event; zero therefore
/// remains invalid rather than acting as an accidental compatibility default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenWeaponVisualDischarge {
    pub world_epoch: u64,
    pub source: ObjectId,
    pub source_object_generation: u64,
    pub weapon_slot: u8,
    pub fired_barrel: u8,
    pub sequence: u64,
    pub logic_frame: u32,
}

impl FrozenWeaponVisualDischarge {
    pub fn is_valid(&self) -> bool {
        self.world_epoch != 0
            && self.source.0 != 0
            && self.source_object_generation != 0
            && self.weapon_slot < 3
            && self.sequence != 0
    }
}

/// Exact observed outcome for the selected barrel in one compatible
/// `W3DModelDraw::handleWeaponFireFX` call.
///
/// These booleans are deliberately *observed callback facts*, not a heuristic
/// from a bone basename.  In particular, `fire_fx_will_handle` is true only
/// when the current W3D state/barrel has a usable FireFX pivot and render
/// object under C++'s callback rules.  A future capture adapter must validate
/// that before it fills this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenW3dWeaponVisualBarrel {
    /// With non-null `fxl`, this module returns `true` from
    /// `handleWeaponFireFX` and terminates Drawable traversal.
    pub fire_fx_will_handle: bool,
    /// C++ starts `RECOIL_START` and/or unhides a muzzle flash for this barrel.
    pub starts_recoil_or_muzzle: bool,
}

/// A declaration-order probe of one C++ Draw module.
///
/// The future capture boundary must include every module reached by C++ in
/// declaration order.  Omitting a module is unsafe: an earlier unknown
/// `ObjectDrawInterface` could handle a non-null FireFX and prevent a later
/// W3D module from receiving recoil.  `OpaqueObjectDrawInterface` consequently
/// makes route construction fail closed whenever it is reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrozenWeaponVisualModuleProbe {
    /// The module is proven not to expose `ObjectDrawInterface`, or is proven
    /// to return false without any recoil/muzzle side effect for this slot and
    /// barrel (for example an inactive W3D state / empty barrel vector).
    KnownNoWeaponFireFx { draw_module_index: u32 },
    /// A compatible current `W3DModelDraw` barrel callback with its complete
    /// pre-mutation source state stamp.
    W3dModelDraw {
        state: FrozenWeaponVisualDrawState,
        barrel: FrozenW3dWeaponVisualBarrel,
    },
    /// A module for which Main cannot prove the C++ callback result.  This is
    /// intentionally distinct from "no bone" so later module routing cannot
    /// be guessed through a custom Draw implementation.
    OpaqueObjectDrawInterface { draw_module_index: u32 },
}

impl FrozenWeaponVisualModuleProbe {
    pub fn draw_module_index(&self) -> u32 {
        match self {
            Self::KnownNoWeaponFireFx { draw_module_index }
            | Self::OpaqueObjectDrawInterface { draw_module_index } => *draw_module_index,
            Self::W3dModelDraw { state, .. } => state.draw_module_index,
        }
    }
}

/// One source module that must receive a renderer-side visual callback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenWeaponVisualDispatchTarget {
    pub state: FrozenWeaponVisualDrawState,
    /// Start/restart C++ `WeaponRecoilInfo::RECOIL_START` for this target.
    pub starts_recoil_or_muzzle: bool,
    /// The non-null FireFX was handled here, so C++ returned from Drawable and
    /// no later Draw module was called for this discharge.
    pub stops_after_fire_fx: bool,
}

/// Body-recoil impulse and FX placement facts sampled at fire time.
///
/// C++ `Drawable::handleWeaponFireFX` converts the absolute recoil direction
/// to object-relative (`recoil_dir - orientation + PI`) and adds
/// `recoil_amount * cos/sin` to loco acceleration pitch/roll rates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrozenWeaponVisualImpulse {
    pub recoil_amount: f32,
    pub recoil_dir: f32,
    pub source_orientation: f32,
    pub target_pos: [f32; 3],
    /// C++ `isContactWeapon()`: contact weapons fall back at `targetPos`.
    pub is_contact_weapon: bool,
}

impl Default for FrozenWeaponVisualImpulse {
    fn default() -> Self {
        Self {
            recoil_amount: 0.0,
            recoil_dir: 0.0,
            source_orientation: 0.0,
            target_pos: [0.0; 3],
            is_contact_weapon: false,
        }
    }
}

impl FrozenWeaponVisualImpulse {
    /// Object-relative recoil angle after C++'s 180-degree flip.
    pub fn object_relative_recoil_angle(self) -> f32 {
        self.recoil_dir - self.source_orientation + std::f32::consts::PI
    }

    pub fn loco_pitch_delta(self) -> f32 {
        if self.recoil_amount == 0.0 {
            0.0
        } else {
            self.recoil_amount * self.object_relative_recoil_angle().cos()
        }
    }

    pub fn loco_roll_delta(self) -> f32 {
        if self.recoil_amount == 0.0 {
            0.0
        } else {
            self.recoil_amount * self.object_relative_recoil_angle().sin()
        }
    }
}

/// Fully frozen source route for one accepted weapon discharge.
///
/// A plan may be serialized with a presentation/event payload, but it is not
/// save-game state by itself.  Pending visual delivery must still be retained
/// across ordinary frustum/FOW culling and discarded only when the world,
/// object generation, or exact source Draw state differs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrozenWeaponVisualDispatchPlan {
    pub discharge: FrozenWeaponVisualDischarge,
    pub fx_route: FrozenWeaponVisualFxRoute,
    /// W3D callback targets in the exact C++ invocation order.
    pub targets: Vec<FrozenWeaponVisualDispatchTarget>,
    /// C++ calls `FXList::doFXPos` at the Drawable position only when a
    /// non-null FireFX reached the end of traversal unhandled.  The external
    /// FX playback is separate from W3D recoil but needs this frozen outcome.
    pub fire_fx_falls_back_to_drawable_position: bool,
    /// Pre-mutation loco / placement facts.  Absent impulse is zeros.
    #[serde(default)]
    pub impulse: FrozenWeaponVisualImpulse,
}

impl FrozenWeaponVisualDispatchPlan {
    pub fn is_valid(&self) -> bool {
        if !self.discharge.is_valid() {
            return false;
        }
        match self.fx_route {
            FrozenWeaponVisualFxRoute::DrawableSuppressed => {
                if !self.targets.is_empty() || self.fire_fx_falls_back_to_drawable_position {
                    return false;
                }
            }
            FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx => {
                if self.fire_fx_falls_back_to_drawable_position
                    || self.targets.iter().any(|target| target.stops_after_fire_fx)
                {
                    return false;
                }
            }
            FrozenWeaponVisualFxRoute::BroadcastWithFireFx => {
                let stops = self
                    .targets
                    .iter()
                    .filter(|target| target.stops_after_fire_fx)
                    .count();
                if stops > 1 {
                    return false;
                }
                if stops == 1 {
                    if self.fire_fx_falls_back_to_drawable_position
                        || !self
                            .targets
                            .last()
                            .is_some_and(|target| target.stops_after_fire_fx)
                    {
                        return false;
                    }
                } else if !self.fire_fx_falls_back_to_drawable_position {
                    return false;
                }
            }
        }

        let mut previous_index = None;
        self.targets.iter().all(|target| {
            let ordered = previous_index
                .map(|previous| previous < target.state.draw_module_index)
                .unwrap_or(true);
            previous_index = Some(target.state.draw_module_index);
            ordered
                && target.state.is_valid()
                && (target.starts_recoil_or_muzzle || target.stops_after_fire_fx)
        })
    }

    /// Decide delivery for one routed module without mutating queue state.
    ///
    /// A cull is not a stale state: retain the event until this exact module
    /// is drawn.  A state mismatch is stale even if its model/key later comes
    /// back, because the nonzero Draw-state revision changes on every source
    /// rebuild and C++ cleared its recoil data at that transition.
    pub fn delivery_for_target(
        &self,
        target_index: usize,
        context: &FrozenWeaponVisualDeliveryContext<'_>,
    ) -> FrozenWeaponVisualDelivery {
        let Some(target) = self.targets.get(target_index) else {
            return FrozenWeaponVisualDelivery::NotTargeted;
        };
        if self.discharge.world_epoch != context.world_epoch
            || self.discharge.source != context.source
            || self.discharge.source_object_generation != context.source_object_generation
            || !target.state.matches_current(context.current_draw_state)
        {
            return FrozenWeaponVisualDelivery::DiscardStateMismatch;
        }
        if !context.will_draw_this_frame {
            return FrozenWeaponVisualDelivery::RetainUntilVisible;
        }
        FrozenWeaponVisualDelivery::Deliver
    }
}

/// Current renderer identity supplied when considering a queued target.
#[derive(Debug, Clone, Copy)]
pub struct FrozenWeaponVisualDeliveryContext<'a> {
    pub world_epoch: u64,
    pub source: ObjectId,
    pub source_object_generation: u64,
    pub current_draw_state: &'a FrozenWeaponVisualDrawState,
    /// False for any ordinary FOW/frustum/model cull.  Such a cull retains a
    /// valid source event; it does not make the event stale.
    pub will_draw_this_frame: bool,
}

/// Queue action selected by [`FrozenWeaponVisualDispatchPlan::delivery_for_target`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenWeaponVisualDelivery {
    NotTargeted,
    RetainUntilVisible,
    DiscardStateMismatch,
    Deliver,
}

/// Why a pre-mutation source probe could not safely become a frozen route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrozenWeaponVisualDispatchBuildError {
    InvalidDischarge,
    MismatchedCaptureFrame {
        discharge_logic_frame: u32,
        source_gate_logic_frame: u32,
    },
    InvalidDrawState {
        draw_module_index: u32,
    },
    NonMonotonicDeclarationOrder {
        previous: u32,
        current: u32,
    },
    OpaqueObjectDrawInterface {
        draw_module_index: u32,
    },
}

/// Freeze a visual plan from the exact source gate and declaration-order
/// modules captured at one accepted weapon call.
///
/// A `None` plan is a faithful no-op for a source with no Drawable.  This
/// helper is the intended live capture boundary; callers should not derive a
/// route from a later renderer frame or an object's current team membership.
pub fn freeze_weapon_visual_dispatch_plan(
    discharge: FrozenWeaponVisualDischarge,
    source_gate: FrozenWeaponVisualSourceGate,
    module_probes: impl IntoIterator<Item = FrozenWeaponVisualModuleProbe>,
) -> Result<Option<FrozenWeaponVisualDispatchPlan>, FrozenWeaponVisualDispatchBuildError> {
    if discharge.logic_frame != source_gate.logic_frame {
        return Err(
            FrozenWeaponVisualDispatchBuildError::MismatchedCaptureFrame {
                discharge_logic_frame: discharge.logic_frame,
                source_gate_logic_frame: source_gate.logic_frame,
            },
        );
    }

    let Some(fx_route) = source_gate.fx_route() else {
        return Ok(None);
    };
    build_frozen_weapon_visual_dispatch_plan(discharge, fx_route, module_probes).map(Some)
}

/// Construct the exact W3D side of C++ `Drawable::handleWeaponFireFX`.
///
/// The inputs must already have been captured *before* source object mutation
/// from an actual accepted weapon invocation.  This helper intentionally does
/// not accept an `AuthoredDrawModel` list: that list omits non-model source
/// modules and can therefore never prove the C++ FireFX stop point by itself.
pub fn build_frozen_weapon_visual_dispatch_plan(
    discharge: FrozenWeaponVisualDischarge,
    fx_route: FrozenWeaponVisualFxRoute,
    module_probes: impl IntoIterator<Item = FrozenWeaponVisualModuleProbe>,
) -> Result<FrozenWeaponVisualDispatchPlan, FrozenWeaponVisualDispatchBuildError> {
    if !discharge.is_valid() {
        return Err(FrozenWeaponVisualDispatchBuildError::InvalidDischarge);
    }

    // C++ stealth suppression bypasses Drawable before it touches the module
    // list.  Do not demand a later/current source layout for this no-callback
    // case: it could only make a truly suppressed event look visualizable.
    if fx_route == FrozenWeaponVisualFxRoute::DrawableSuppressed {
        return Ok(FrozenWeaponVisualDispatchPlan {
            discharge,
            fx_route,
            targets: Vec::new(),
            fire_fx_falls_back_to_drawable_position: false,
            impulse: FrozenWeaponVisualImpulse::default(),
        });
    }

    let mut targets = Vec::new();
    let mut previous_module_index = None;
    for probe in module_probes {
        let draw_module_index = probe.draw_module_index();
        if let Some(previous) = previous_module_index {
            if previous >= draw_module_index {
                return Err(
                    FrozenWeaponVisualDispatchBuildError::NonMonotonicDeclarationOrder {
                        previous,
                        current: draw_module_index,
                    },
                );
            }
        }
        previous_module_index = Some(draw_module_index);

        match probe {
            FrozenWeaponVisualModuleProbe::KnownNoWeaponFireFx { .. } => {}
            FrozenWeaponVisualModuleProbe::OpaqueObjectDrawInterface { .. } => {
                return Err(
                    FrozenWeaponVisualDispatchBuildError::OpaqueObjectDrawInterface {
                        draw_module_index,
                    },
                );
            }
            FrozenWeaponVisualModuleProbe::W3dModelDraw { state, barrel } => {
                if !state.is_valid() {
                    return Err(FrozenWeaponVisualDispatchBuildError::InvalidDrawState {
                        draw_module_index,
                    });
                }
                let stops_after_fire_fx = fx_route
                    == FrozenWeaponVisualFxRoute::BroadcastWithFireFx
                    && barrel.fire_fx_will_handle;
                if barrel.starts_recoil_or_muzzle || stops_after_fire_fx {
                    targets.push(FrozenWeaponVisualDispatchTarget {
                        state,
                        starts_recoil_or_muzzle: barrel.starts_recoil_or_muzzle,
                        stops_after_fire_fx,
                    });
                }
                // This is exactly Drawable's early return after the module
                // callback returns true.  Later modules are not probed or
                // required to be representable because C++ never called them.
                if stops_after_fire_fx {
                    return Ok(FrozenWeaponVisualDispatchPlan {
                        discharge,
                        fx_route,
                        targets,
                        fire_fx_falls_back_to_drawable_position: false,
                        impulse: FrozenWeaponVisualImpulse::default(),
                    });
                }
            }
        }
    }

    // All checked modules returned false.  C++ only emits its position-level
    // fallback when the original pointer was non-null.
    Ok(FrozenWeaponVisualDispatchPlan {
        discharge,
        fx_route,
        targets,
        fire_fx_falls_back_to_drawable_position: fx_route
            == FrozenWeaponVisualFxRoute::BroadcastWithFireFx,
        impulse: FrozenWeaponVisualImpulse::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discharge() -> FrozenWeaponVisualDischarge {
        FrozenWeaponVisualDischarge {
            world_epoch: 7,
            source: ObjectId(41),
            source_object_generation: 3,
            weapon_slot: 0,
            fired_barrel: 1,
            sequence: 9,
            logic_frame: 123,
        }
    }

    fn state(module_index: u32, revision: u64) -> FrozenWeaponVisualDrawState {
        FrozenWeaponVisualDrawState {
            draw_module_index: module_index,
            source_template_name: "GLATankScorpion".into(),
            model_key: format!("Scorpion_{module_index}"),
            selected_condition_state_index: module_index + 10,
            draw_state_revision: revision,
        }
    }

    fn w3d(
        module_index: u32,
        revision: u64,
        fire_fx_will_handle: bool,
        starts_recoil_or_muzzle: bool,
    ) -> FrozenWeaponVisualModuleProbe {
        FrozenWeaponVisualModuleProbe::W3dModelDraw {
            state: state(module_index, revision),
            barrel: FrozenW3dWeaponVisualBarrel {
                fire_fx_will_handle,
                starts_recoil_or_muzzle,
            },
        }
    }

    fn source_gate() -> FrozenWeaponVisualSourceGate {
        FrozenWeaponVisualSourceGate {
            source_has_drawable: true,
            source_is_locally_controlled: false,
            source_is_stealthed: false,
            source_is_detected: false,
            source_is_disguised: false,
            source_is_mine: false,
            weapon_plays_fx_when_stealthed: false,
            selected_fx_is_present: true,
            logic_frame: 123,
            suspend_fx_frame: 0,
        }
    }

    #[test]
    fn loco_impulse_uses_object_relative_angle_and_pi_flip() {
        let impulse = FrozenWeaponVisualImpulse {
            recoil_amount: 2.0,
            recoil_dir: 0.0,
            source_orientation: 0.0,
            target_pos: [1.0, 0.0, 0.0],
            is_contact_weapon: false,
        };
        let angle = impulse.object_relative_recoil_angle();
        assert!((angle - std::f32::consts::PI).abs() < 1e-5);
        assert!((impulse.loco_pitch_delta() - 2.0 * angle.cos()).abs() < 1e-5);
        assert!((impulse.loco_roll_delta() - 2.0 * angle.sin()).abs() < 1e-5);
        assert_eq!(FrozenWeaponVisualImpulse::default().loco_pitch_delta(), 0.0);
    }

    #[test]
    fn source_gate_uses_instance_suspend_then_remote_stealth_drawable_bypass() {
        let gate = source_gate();
        assert_eq!(
            gate.fx_route(),
            Some(FrozenWeaponVisualFxRoute::BroadcastWithFireFx)
        );

        let delayed = FrozenWeaponVisualSourceGate {
            suspend_fx_frame: 124,
            ..gate
        };
        assert_eq!(
            delayed.fx_route(),
            Some(FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx),
            "the suspend gate nulls only the FX pointer; Drawable still receives recoil callbacks"
        );

        let hidden_remote = FrozenWeaponVisualSourceGate {
            source_is_stealthed: true,
            ..delayed
        };
        assert_eq!(
            hidden_remote.fx_route(),
            Some(FrozenWeaponVisualFxRoute::DrawableSuppressed),
            "remote undetected stealth bypasses Drawable even after the pointer was nulled"
        );

        for exempted_source in [
            FrozenWeaponVisualSourceGate {
                source_is_locally_controlled: true,
                ..hidden_remote
            },
            FrozenWeaponVisualSourceGate {
                source_is_detected: true,
                ..hidden_remote
            },
            FrozenWeaponVisualSourceGate {
                source_is_disguised: true,
                ..hidden_remote
            },
            FrozenWeaponVisualSourceGate {
                source_is_mine: true,
                ..hidden_remote
            },
            FrozenWeaponVisualSourceGate {
                weapon_plays_fx_when_stealthed: true,
                ..hidden_remote
            },
        ] {
            assert_eq!(
                exempted_source.fx_route(),
                Some(FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx),
                "each C++ stealth exemption still calls Drawable with the delayed null FX"
            );
        }
    }

    #[test]
    fn source_gate_without_drawable_has_no_plan_and_capture_frames_must_agree() {
        let absent_drawable = FrozenWeaponVisualSourceGate {
            source_has_drawable: false,
            ..source_gate()
        };
        assert!(
            freeze_weapon_visual_dispatch_plan(
                discharge(),
                absent_drawable,
                [FrozenWeaponVisualModuleProbe::OpaqueObjectDrawInterface {
                    draw_module_index: 0,
                }],
            )
            .expect("no source Drawable is a faithful visual no-op")
            .is_none()
        );

        let mismatched = FrozenWeaponVisualSourceGate {
            logic_frame: 124,
            ..source_gate()
        };
        assert_eq!(
            freeze_weapon_visual_dispatch_plan(discharge(), mismatched, []).unwrap_err(),
            FrozenWeaponVisualDispatchBuildError::MismatchedCaptureFrame {
                discharge_logic_frame: 123,
                source_gate_logic_frame: 124,
            }
        );
    }

    #[test]
    fn suppressed_stealth_bypasses_drawable_without_requiring_modules() {
        let plan = build_frozen_weapon_visual_dispatch_plan(
            discharge(),
            FrozenWeaponVisualFxRoute::DrawableSuppressed,
            [FrozenWeaponVisualModuleProbe::OpaqueObjectDrawInterface {
                draw_module_index: 0,
            }],
        )
        .expect("suppressed path never calls Drawable");

        assert!(plan.targets.is_empty());
        assert!(!plan.fire_fx_falls_back_to_drawable_position);
        assert!(plan.is_valid());
    }

    #[test]
    fn null_fx_broadcasts_recoil_through_every_proven_module() {
        let plan = build_frozen_weapon_visual_dispatch_plan(
            discharge(),
            FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx,
            [
                FrozenWeaponVisualModuleProbe::KnownNoWeaponFireFx {
                    draw_module_index: 0,
                },
                w3d(1, 1, true, true),
                w3d(2, 1, true, true),
            ],
        )
        .expect("null FX must not stop at a FireFX bone");

        assert_eq!(plan.targets.len(), 2);
        assert!(
            plan.targets
                .iter()
                .all(|target| target.starts_recoil_or_muzzle)
        );
        assert!(
            plan.targets
                .iter()
                .all(|target| !target.stops_after_fire_fx)
        );
        assert!(!plan.fire_fx_falls_back_to_drawable_position);
        assert!(plan.is_valid());
    }

    #[test]
    fn non_null_fx_stops_only_at_first_module_that_actually_handles_it() {
        let plan = build_frozen_weapon_visual_dispatch_plan(
            discharge(),
            FrozenWeaponVisualFxRoute::BroadcastWithFireFx,
            [
                w3d(0, 1, false, true),
                w3d(1, 1, true, false),
                // C++ never calls this one after module 1 returns true.
                FrozenWeaponVisualModuleProbe::OpaqueObjectDrawInterface {
                    draw_module_index: 2,
                },
            ],
        )
        .expect("later opaque module is unreachable after FireFX handling");

        assert_eq!(plan.targets.len(), 2);
        assert!(plan.targets[0].starts_recoil_or_muzzle);
        assert!(!plan.targets[0].stops_after_fire_fx);
        assert!(!plan.targets[1].starts_recoil_or_muzzle);
        assert!(plan.targets[1].stops_after_fire_fx);
        assert!(!plan.fire_fx_falls_back_to_drawable_position);
        assert!(plan.is_valid());
    }

    #[test]
    fn non_null_unhandled_fx_uses_drawable_position_fallback_after_all_recoil_callbacks() {
        let plan = build_frozen_weapon_visual_dispatch_plan(
            discharge(),
            FrozenWeaponVisualFxRoute::BroadcastWithFireFx,
            [w3d(0, 1, false, true), w3d(1, 1, false, true)],
        )
        .expect("all modules were proven to return false");

        assert_eq!(plan.targets.len(), 2);
        assert!(
            plan.targets
                .iter()
                .all(|target| !target.stops_after_fire_fx)
        );
        assert!(plan.fire_fx_falls_back_to_drawable_position);
        assert!(plan.is_valid());
    }

    #[test]
    fn opaque_module_before_a_possible_handler_fails_closed() {
        let err = build_frozen_weapon_visual_dispatch_plan(
            discharge(),
            FrozenWeaponVisualFxRoute::BroadcastWithFireFx,
            [
                FrozenWeaponVisualModuleProbe::OpaqueObjectDrawInterface {
                    draw_module_index: 0,
                },
                w3d(1, 1, true, true),
            ],
        )
        .expect_err("unknown earlier handler could terminate C++ traversal");

        assert_eq!(
            err,
            FrozenWeaponVisualDispatchBuildError::OpaqueObjectDrawInterface {
                draw_module_index: 0
            }
        );
    }

    #[test]
    fn culling_retains_but_state_rebuild_discards_even_after_same_named_return() {
        let plan = build_frozen_weapon_visual_dispatch_plan(
            discharge(),
            FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx,
            [w3d(0, 5, false, true)],
        )
        .expect("valid plan");
        let saved_state = plan.targets[0].state.clone();

        let culled = FrozenWeaponVisualDeliveryContext {
            world_epoch: 7,
            source: ObjectId(41),
            source_object_generation: 3,
            current_draw_state: &saved_state,
            will_draw_this_frame: false,
        };
        assert_eq!(
            plan.delivery_for_target(0, &culled),
            FrozenWeaponVisualDelivery::RetainUntilVisible
        );

        let visible = FrozenWeaponVisualDeliveryContext {
            will_draw_this_frame: true,
            ..culled
        };
        assert_eq!(
            plan.delivery_for_target(0, &visible),
            FrozenWeaponVisualDelivery::Deliver
        );

        // A C++ A→B→A selection can restore these textual fields but
        // `rebuildWeaponRecoilInfo` cleared the old state. The revision makes
        // that distinction observable and prevents an offscreen replay.
        let returned_to_same_names = FrozenWeaponVisualDrawState {
            draw_state_revision: 6,
            ..saved_state.clone()
        };
        let rebuilt = FrozenWeaponVisualDeliveryContext {
            current_draw_state: &returned_to_same_names,
            ..visible
        };
        assert_eq!(
            plan.delivery_for_target(0, &rebuilt),
            FrozenWeaponVisualDelivery::DiscardStateMismatch
        );
    }
}
