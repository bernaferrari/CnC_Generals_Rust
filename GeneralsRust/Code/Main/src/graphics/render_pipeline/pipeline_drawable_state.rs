//! Renderer-local W3DModelDraw state and its explicit v4 snapshot boundary.
//!
//! C++ owns animation and recoil state per `W3DModelDraw` module, not per
//! mesh name.  Main therefore keeps one unified cache keyed by the frozen
//! `(ObjectId, Draw-module)` identity.  The save DTO never carries WGPU
//! handles or local animation ordinals: it is accepted only after the next
//! full frozen frame proves the exact source identity, then collection checks
//! the fresh W3D asset before applying it.

use super::*;

type ClientDrawableStateSnapshot = crate::save_load::snapshot::ClientDrawableStateSnapshot;
type ClientDrawableWorldSnapshot = crate::save_load::snapshot::ClientDrawableWorldSnapshot;
type ClientDrawableAnimationMode = crate::save_load::snapshot::ClientDrawableAnimationMode;
type ClientDrawableAnimationSnapshot = crate::save_load::snapshot::ClientDrawableAnimationSnapshot;
type ClientDrawableRecoilSnapshot = crate::save_load::snapshot::ClientDrawableRecoilSnapshot;

fn client_animation_mode(
    mode: &crate::assets::AuthoredDrawAnimationMode,
) -> Option<ClientDrawableAnimationMode> {
    match mode {
        crate::assets::AuthoredDrawAnimationMode::Manual => {
            Some(ClientDrawableAnimationMode::Manual)
        }
        crate::assets::AuthoredDrawAnimationMode::Loop => Some(ClientDrawableAnimationMode::Loop),
        crate::assets::AuthoredDrawAnimationMode::Once => Some(ClientDrawableAnimationMode::Once),
        crate::assets::AuthoredDrawAnimationMode::LoopBackwards => {
            Some(ClientDrawableAnimationMode::LoopBackwards)
        }
        crate::assets::AuthoredDrawAnimationMode::OnceBackwards => {
            Some(ClientDrawableAnimationMode::OnceBackwards)
        }
        // Main has no exact ping-pong direction state or unsupported-mode
        // adapter.  A save must not invent one by serializing a nearby mode.
        crate::assets::AuthoredDrawAnimationMode::LoopPingPong
        | crate::assets::AuthoredDrawAnimationMode::Unsupported(_) => None,
    }
}

fn frozen_visual_identity_for_draw_model(
    source_template_name: &str,
    draw_model: &crate::assets::AuthoredDrawModel,
) -> Option<FrozenVisualDrawIdentity> {
    if source_template_name.trim().is_empty() || draw_model.model_key.trim().is_empty() {
        return None;
    }
    let animation = match draw_model.animations.as_slice() {
        [] => None,
        [animation] => Some(FrozenVisualAnimationIdentity {
            hierarchy_animation: animation.name.clone(),
            mode: client_animation_mode(&draw_model.animation_mode)?,
        }),
        // C++ selects among several clips with GameClientRandomValue.  Main
        // intentionally has no compatible durable selector yet, so none of
        // those in-flight states may masquerade as a save-restorable clip.
        _ => return None,
    };
    Some(FrozenVisualDrawIdentity {
        source_template_name: source_template_name.to_string(),
        model_key: draw_model.model_key.clone(),
        selected_condition_state_index: draw_model.selected_condition_state_index,
        animation,
    })
}

fn visual_identity_matches_snapshot(
    identity: &FrozenVisualDrawIdentity,
    snapshot: &ClientDrawableStateSnapshot,
) -> bool {
    snapshot.object_id != 0
        && identity
            .source_template_name
            .eq_ignore_ascii_case(snapshot.source_template_name.trim())
        && identity
            .model_key
            .eq_ignore_ascii_case(snapshot.model_key.trim())
        && identity.selected_condition_state_index == snapshot.selected_condition_state_index
        && match (&identity.animation, &snapshot.animation) {
            (None, None) => true,
            (Some(expected), Some(saved)) => {
                expected
                    .hierarchy_animation
                    .eq_ignore_ascii_case(saved.hierarchy_animation.trim())
                    && expected.mode == saved.mode
            }
            _ => false,
        }
}

fn snapshot_from_visual_state(
    object_id: u32,
    draw_module_index: u32,
    state: &ObjectVisualState,
) -> Option<ClientDrawableStateSnapshot> {
    let identity = state.identity.as_ref()?;
    let animation = match (&identity.animation, &state.animation) {
        (None, None) => None,
        (Some(expected), Some(live))
            if !state.force_bind_pose
                && live
                    .animation_identity
                    .eq_ignore_ascii_case(expected.hierarchy_animation.as_str())
                && client_animation_mode(&live.mode) == Some(expected.mode)
                && live.current_frame.is_finite()
                && live.frame_rate.is_finite()
                && live.num_frames > 0 =>
        {
            Some(ClientDrawableAnimationSnapshot {
                hierarchy_animation: expected.hierarchy_animation.clone(),
                frame: live.current_frame,
                mode: expected.mode,
            })
        }
        // A source state with an HAnim that Main has not resolved is not an
        // authored bind pose.  Omitting it from the v4 payload prevents an
        // absent cache entry from being reloaded as a clip-zero substitute.
        _ => return None,
    };
    let recoil_slots = state.recoil_slots.clone().map(|slot| {
        slot.into_iter()
            .map(|recoil| ClientDrawableRecoilSnapshot {
                phase: recoil.phase,
                shift: recoil.shift,
                recoil_rate: recoil.recoil_rate,
            })
            .collect()
    });
    let snapshot = ClientDrawableStateSnapshot {
        object_id,
        draw_module_index,
        source_template_name: identity.source_template_name.clone(),
        model_key: identity.model_key.clone(),
        selected_condition_state_index: identity.selected_condition_state_index,
        animation,
        last_seen_weapon_discharge_sequence: state.last_seen_weapon_discharge_sequence,
        recoil_slots,
    };
    (snapshot.has_stable_source_identity() && snapshot.has_finite_visual_values())
        .then_some(snapshot)
}

fn recoil_layout_for_topology(
    topology: &crate::assets::W3dWeaponBarrelTopology,
) -> [Vec<ObjectWeaponRecoilState>; 3] {
    std::array::from_fn(|slot| vec![ObjectWeaponRecoilState::default(); topology.slots[slot].len()])
}

fn topology_has_visual_recoil(topology: &crate::assets::W3dWeaponBarrelTopology) -> bool {
    topology
        .slots
        .iter()
        .flatten()
        .any(|barrel| barrel.has_recoil_or_muzzle())
}

fn recoil_slots_match_topology(
    slots: &[Vec<ObjectWeaponRecoilState>; 3],
    topology: &crate::assets::W3dWeaponBarrelTopology,
) -> bool {
    slots
        .iter()
        .zip(topology.slots.iter())
        .all(|(states, barrels)| states.len() == barrels.len())
}

fn restored_recoil_slots_for_topology(
    saved: &ClientDrawableStateSnapshot,
    topology: &crate::assets::W3dWeaponBarrelTopology,
    kinematics: &crate::assets::AuthoredDrawRecoilKinematics,
) -> Option<[Vec<ObjectWeaponRecoilState>; 3]> {
    if !saved.has_finite_visual_values()
        || !saved
            .recoil_slots
            .iter()
            .zip(topology.slots.iter())
            .all(|(states, barrels)| states.len() == barrels.len())
        || (topology_has_visual_recoil(topology) && !kinematics.is_visual_usable())
    {
        return None;
    }

    let slots = saved.recoil_slots.clone().map(|slot| {
        slot.into_iter()
            .map(|recoil| ObjectWeaponRecoilState {
                phase: recoil.phase,
                shift: recoil.shift,
                recoil_rate: recoil.recoil_rate,
            })
            .collect::<Vec<_>>()
    });
    for (slot_index, (states, barrels)) in slots.iter().zip(topology.slots.iter()).enumerate() {
        for (barrel_index, (state, barrel)) in states.iter().zip(barrels.iter()).enumerate() {
            if state.shift < 0.0
                || state.recoil_rate < 0.0
                || state.shift > kinematics.max_recoil_distance()
                || (state.phase == crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle
                    && (state.shift != 0.0 || state.recoil_rate != 0.0))
            {
                return None;
            }
            if !barrel.has_recoil_or_muzzle()
                && (state.phase != crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle
                    || state.shift != 0.0
                    || state.recoil_rate != 0.0)
            {
                return None;
            }
            // C++ can retain RECOIL_START for one visible frame on a
            // muzzle-only barrel, but every later phase needs an actual
            // recoil pivot to move.
            if barrel.recoil_pivot_index.is_none()
                && state.phase != crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle
                && state.phase != crate::save_load::snapshot::ClientDrawableRecoilPhase::RecoilStart
            {
                let _ = (slot_index, barrel_index);
                return None;
            }
        }
    }
    Some(slots)
}

/// Consume events that cannot be safely visualized by the current frozen
/// Draw state.  C++ does not replay a prior drawable broadcast merely because
/// an asset/topology becomes available later; Main must similarly advance the
/// per-module observation watermark while deliberately retaining bind/idle.
fn retain_fire_starts_from_plans(
    drawable_visual_states: &mut HashMap<(u32, u32), ObjectVisualState>,
    plans: &[crate::presentation_frame::FrozenWeaponVisualDispatchPlan],
) {
    for plan in plans {
        if !plan.is_valid() {
            continue;
        }
        for target in &plan.targets {
            if !target.starts_recoil_or_muzzle {
                continue;
            }
            let key = (plan.discharge.source.0, target.state.draw_module_index);
            let state = drawable_visual_states.entry(key).or_default();
            if plan.discharge.sequence == 0
                || plan.discharge.sequence <= state.last_seen_weapon_discharge_sequence
            {
                continue;
            }
            let slot = usize::from(plan.discharge.weapon_slot);
            let barrel = usize::from(plan.discharge.fired_barrel);
            if slot >= state.recoil_slots.len() {
                continue;
            }
            if state.recoil_slots[slot].len() <= barrel {
                state.recoil_slots[slot].resize(barrel + 1, ObjectWeaponRecoilState::default());
            }
            if let Some(recoil) = state.recoil_slots[slot].get_mut(barrel) {
                recoil.phase = crate::save_load::snapshot::ClientDrawableRecoilPhase::RecoilStart;
                recoil.recoil_rate = crate::assets::AuthoredDrawRecoilKinematics::default()
                    .initial_recoil_per_logic_frame();
            }
            state.last_seen_weapon_discharge_sequence = plan.discharge.sequence;
        }
    }
}

fn discharges_from_valid_plans(
    plans: &[crate::presentation_frame::FrozenWeaponVisualDispatchPlan],
    object_id: crate::game_logic::ObjectId,
    draw_module_index: u32,
    source_template_name: &str,
    draw_model: &crate::assets::AuthoredDrawModel,
) -> (
    Vec<(u8, u8, u64)>,
    Option<crate::presentation_frame::FrozenWeaponVisualImpulse>,
) {
    let mut discharges = Vec::new();
    let mut impulse = None;
    for plan in plans {
        if !plan.is_valid() || plan.discharge.source != object_id {
            continue;
        }
        let Some(target_index) = plan
            .targets
            .iter()
            .position(|target| target.state.draw_module_index == draw_module_index)
        else {
            continue;
        };
        let target = &plan.targets[target_index];
        let current = crate::presentation_frame::FrozenWeaponVisualDrawState {
            draw_module_index,
            source_template_name: source_template_name.to_string(),
            model_key: draw_model.model_key.clone(),
            selected_condition_state_index: draw_model.selected_condition_state_index,
            draw_state_revision: target.state.draw_state_revision,
        };
        let delivery = plan.delivery_for_target(
            target_index,
            &crate::presentation_frame::FrozenWeaponVisualDeliveryContext {
                world_epoch: plan.discharge.world_epoch,
                source: object_id,
                source_object_generation: plan.discharge.source_object_generation,
                current_draw_state: &current,
                will_draw_this_frame: true,
            },
        );
        match delivery {
            crate::presentation_frame::FrozenWeaponVisualDelivery::Deliver => {
                if target.starts_recoil_or_muzzle {
                    discharges.push((
                        plan.discharge.weapon_slot,
                        plan.discharge.fired_barrel,
                        plan.discharge.sequence,
                    ));
                }
                if impulse.is_none() && plan.impulse.recoil_amount != 0.0 {
                    impulse = Some(plan.impulse);
                }
            }
            crate::presentation_frame::FrozenWeaponVisualDelivery::DiscardStateMismatch => {}
            crate::presentation_frame::FrozenWeaponVisualDelivery::RetainUntilVisible
            | crate::presentation_frame::FrozenWeaponVisualDelivery::NotTargeted => {}
        }
    }
    (discharges, impulse)
}

fn discard_unvisualizable_discharges(
    visual_state: &mut ObjectVisualState,
    discharges: &[(u8, u8, u64)],
) {
    for &(_weapon_slot, _fired_barrel, sequence) in discharges {
        if sequence > visual_state.last_seen_weapon_discharge_sequence {
            visual_state.last_seen_weapon_discharge_sequence = sequence;
        }
    }
}

fn evolve_recoil_and_build_controls(
    visual_state: &mut ObjectVisualState,
    topology: &crate::assets::W3dWeaponBarrelTopology,
    kinematics: &crate::assets::AuthoredDrawRecoilKinematics,
    discharges: &[(u8, u8, u64)],
) -> Vec<crate::assets::W3dWeaponVisualControl> {
    if !recoil_slots_match_topology(&visual_state.recoil_slots, topology) {
        visual_state.recoil_slots = recoil_layout_for_topology(topology);
    }

    let usable_kinematics = kinematics.is_visual_usable();
    for &(weapon_slot, fired_barrel, sequence) in discharges {
        if sequence == 0 || sequence <= visual_state.last_seen_weapon_discharge_sequence {
            continue;
        }
        let slot_index = usize::from(weapon_slot);
        if let (Some(barrels), Some(states)) = (
            topology.slots.get(slot_index),
            visual_state.recoil_slots.get_mut(slot_index),
        ) {
            if let Some((barrel, recoil)) = bars_get_with_state(barrels, states, fired_barrel) {
                if usable_kinematics && barrel.has_recoil_or_muzzle() {
                    // C++ starts a fresh visual discharge from the current shift
                    // (including a settling recoil), replacing only phase/rate.
                    recoil.phase =
                        crate::save_load::snapshot::ClientDrawableRecoilPhase::RecoilStart;
                    recoil.recoil_rate = kinematics.initial_recoil_per_logic_frame();
                }
            }
        }
        visual_state.last_seen_weapon_discharge_sequence = sequence;
    }

    let mut controls = Vec::new();
    for (slot_index, barrels) in topology.slots.iter().enumerate() {
        for (barrel, recoil) in barrels
            .iter()
            .zip(visual_state.recoil_slots[slot_index].iter_mut())
        {
            let muzzle_flash_visible =
                recoil.phase == crate::save_load::snapshot::ClientDrawableRecoilPhase::RecoilStart;
            if barrel.recoil_pivot_index.is_some() && usable_kinematics {
                match recoil.phase {
                    crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle => {}
                    crate::save_load::snapshot::ClientDrawableRecoilPhase::RecoilStart
                    | crate::save_load::snapshot::ClientDrawableRecoilPhase::Recoil => {
                        recoil.shift += recoil.recoil_rate;
                        recoil.recoil_rate *= kinematics.recoil_damping();
                        if recoil.shift >= kinematics.max_recoil_distance() {
                            recoil.shift = kinematics.max_recoil_distance();
                            recoil.phase =
                                crate::save_load::snapshot::ClientDrawableRecoilPhase::Settle;
                        } else if recoil.recoil_rate.abs() < 0.01 {
                            recoil.phase =
                                crate::save_load::snapshot::ClientDrawableRecoilPhase::Settle;
                        } else {
                            recoil.phase =
                                crate::save_load::snapshot::ClientDrawableRecoilPhase::Recoil;
                        }
                    }
                    crate::save_load::snapshot::ClientDrawableRecoilPhase::Settle => {
                        recoil.shift -= kinematics.recoil_settle_per_logic_frame();
                        if recoil.shift <= 0.0 {
                            recoil.shift = 0.0;
                            recoil.phase =
                                crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle;
                        }
                    }
                }
            } else {
                // This includes C++ muzzle-only barrels: their start frame is
                // visible above, then `handleClientRecoil` immediately idles
                // them because there is no recoil pivot to control.
                recoil.phase = crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle;
                recoil.shift = 0.0;
                recoil.recoil_rate = 0.0;
            }
            controls.push(crate::assets::W3dWeaponVisualControl {
                recoil_pivot_index: barrel.recoil_pivot_index,
                recoil_shift: recoil.shift,
                muzzle_flash_pivot_index: barrel.muzzle_flash_pivot_index,
                muzzle_flash_visible,
            });
        }
    }
    controls
}

fn recoil_controls_for_topology(
    visual_state: &mut ObjectVisualState,
    topology: Option<&crate::assets::W3dWeaponBarrelTopology>,
    kinematics: &crate::assets::AuthoredDrawRecoilKinematics,
    discharges: &[(u8, u8, u64)],
) -> Vec<crate::assets::W3dWeaponVisualControl> {
    let Some(topology) = topology else {
        discard_unvisualizable_discharges(visual_state, discharges);
        return Vec::new();
    };
    if topology_has_visual_recoil(topology) && !kinematics.is_visual_usable() {
        // Retain the concrete slot layout only as an idle baseline.  Bad
        // parsed kinematics must not move a bone or briefly reveal a muzzle.
        visual_state.recoil_slots = recoil_layout_for_topology(topology);
        discard_unvisualizable_discharges(visual_state, discharges);
        return Vec::new();
    }
    evolve_recoil_and_build_controls(visual_state, topology, kinematics, discharges)
}

fn bars_get_with_state<'a>(
    barrels: &'a [crate::assets::W3dWeaponBarrelBinding],
    states: &'a mut [ObjectWeaponRecoilState],
    fired_barrel: u8,
) -> Option<(
    &'a crate::assets::W3dWeaponBarrelBinding,
    &'a mut ObjectWeaponRecoilState,
)> {
    let index = usize::from(fired_barrel);
    Some((barrels.get(index)?, states.get_mut(index)?))
}

/// C++ `setPauseAnimation(!getDrawable()->getShouldAnimate(m_animationsRequirePower))`.
fn leftover_should_animate_for_presentation(
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
    object_id: crate::game_logic::ObjectId,
    consider_power: bool,
) -> bool {
    let Some(ro) = presentation.and_then(|frame| frame.objects.iter().find(|o| o.id == object_id))
    else {
        // C++ Drawable.cpp:638 — no object returns TRUE.
        return true;
    };
    let produced_at_helipad =
        crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
            &ro.template_name,
        );
    let is_disabled = ro.disabled
        || ro.disabled_emp
        || ro.disabled_hacked
        || ro.disabled_paralyzed
        || ro.disabled_subdued
        || ro.disabled_unmanned
        || ro.disabled_underpowered
        || ro.disabled_script_underpowered;
    gamelogic::object::draw::object_should_animate_flags(
        consider_power,
        ro.disabled_script_underpowered,
        is_disabled,
        produced_at_helipad,
        ro.disabled_hacked,
        ro.disabled_paralyzed,
        ro.disabled_emp,
        ro.disabled_subdued,
        ro.disabled_unmanned,
        ro.disabled_underpowered,
    )
}

impl RenderPipeline {
    /// Resolve an exact frozen Draw HAnim without collection-time archive I/O.
    /// Local data is immediately usable; an external companion must already
    /// have been loaded into AssetManager's exact identity/hierarchy cache.
    pub(super) fn cached_draw_animation_binding(
        model: &crate::assets::W3DModel,
        identity: &str,
    ) -> Option<crate::assets::W3dAnimationBinding> {
        if let Some(local) = model.local_animation_binding_for_draw_identity(identity) {
            return Some(local);
        }
        let asset_manager_arc = crate::assets::get_asset_manager()?;
        let asset_manager = asset_manager_arc.lock().ok()?;
        asset_manager.cached_w3d_draw_animation_binding(model, identity)
    }

    /// Capture only exact initialized per-Draw state in deterministic order.
    ///
    /// This is deliberately available without a `GraphicsSystem`: state has
    /// already passed fresh W3D validation during collection, and SaveFile
    /// capture must not open archives or touch live GameLogic.
    pub fn capture_client_drawable_snapshot(&self) -> ClientDrawableWorldSnapshot {
        let mut drawables = self
            .drawable_visual_states
            .iter()
            .filter_map(|(&(object_id, draw_module_index), state)| {
                snapshot_from_visual_state(object_id, draw_module_index, state)
            })
            .collect::<Vec<_>>();
        if let Some(client) = gamelogic::helpers::TheGameClient::get() {
            for (id, state) in client.snapshot_objectless_drawables() {
                let template = state.template_name.trim();
                if id == 0 || template.is_empty() {
                    continue;
                }
                drawables.push(ClientDrawableStateSnapshot {
                    object_id: 0,
                    draw_module_index: id,
                    source_template_name: template.to_string(),
                    model_key: template.to_string(),
                    selected_condition_state_index: 0,
                    animation: None,
                    last_seen_weapon_discharge_sequence: 0,
                    recoil_slots: std::array::from_fn(|_| Vec::new()),
                });
            }
        }
        drawables.sort_by_key(|state| (state.object_id, state.draw_module_index));
        ClientDrawableWorldSnapshot { drawables }
    }

    /// Stage a renderer-owned v4 Drawable restore after the host has
    /// successfully installed and seeded a new logical world.  Replacing the
    /// pending payload is intentional: a later successful restore supersedes
    /// an earlier one, and an empty payload explicitly clears it.
    pub fn queue_client_drawable_restore(&mut self, snapshot: ClientDrawableWorldSnapshot) {
        self.pending_client_drawable_restore = (!snapshot.drawables.is_empty()).then_some(snapshot);
        self.pending_client_drawable_imports.clear();
    }

    /// Convert a staged payload to source-identity candidates against one full
    /// frozen frame.  This performs no model/archive I/O; collection removes a
    /// candidate before normal loading, so a missing model cannot retry an old
    /// saved visual state indefinitely.
    pub(super) fn prepare_pending_client_drawable_restore_for_frame(
        &mut self,
        frame: &crate::presentation_frame::PresentationFrame,
    ) {
        let Some(snapshot) = self.pending_client_drawable_restore.take() else {
            return;
        };

        let mut identities = HashMap::new();
        for unit in frame.unit_render_inputs() {
            for draw_model in unit.draw_models {
                let key = (unit.id.0, draw_model.module_index);
                if let Some(identity) =
                    frozen_visual_identity_for_draw_model(unit.template_name.as_str(), &draw_model)
                {
                    identities.insert(key, identity);
                }
            }
        }

        let mut seen = HashSet::new();
        for drawable in snapshot.drawables {
            let key = (drawable.object_id, drawable.draw_module_index);
            if !drawable.has_stable_source_identity()
                || !drawable.has_finite_visual_values()
                || !seen.insert(key)
            {
                continue;
            }
            let Some(identity) = identities.get(&key) else {
                continue;
            };
            if visual_identity_matches_snapshot(identity, &drawable) {
                self.pending_client_drawable_imports.insert(key, drawable);
            }
        }
    }

    /// Advance one source-selected `W3DModelDraw` animation while owning the
    /// unified visual cache.  `pending_restore` was removed before the normal
    /// model load for this `(Object, Draw-module)`, so no unavailable asset can
    /// retain an old saved state for a later retry.
    pub(super) fn advance_authored_draw_animation(
        &mut self,
        object_id: crate::game_logic::ObjectId,
        draw_module_index: u32,
        model: &crate::assets::W3DModel,
        source_template_name: &str,
        draw_model: &crate::assets::AuthoredDrawModel,
        delta_time: f32,
        pending_restore: Option<ClientDrawableStateSnapshot>,
        visual_plans: &[crate::presentation_frame::FrozenWeaponVisualDispatchPlan],
    ) -> (
        Option<crate::assets::W3dAnimationBinding>,
        f32,
        Vec<crate::assets::W3dWeaponVisualControl>,
    ) {
        let Some(identity) =
            frozen_visual_identity_for_draw_model(source_template_name, draw_model)
        else {
            return (None, 0.0, Vec::new());
        };
        retain_fire_starts_from_plans(&mut self.drawable_visual_states, visual_plans);
        let key = (object_id.0, draw_module_index);
        let state = self.drawable_visual_states.entry(key).or_default();
        if state.identity.as_ref() != Some(&identity) {
            let first_bind = state.identity.is_none();
            let retained_recoil = first_bind.then(|| state.recoil_slots.clone());
            let retained_sequence = first_bind.then_some(state.last_seen_weapon_discharge_sequence);
            *state = ObjectVisualState {
                identity: Some(identity.clone()),
                ..Default::default()
            };
            if let Some(recoil_slots) = retained_recoil {
                state.recoil_slots = recoil_slots;
            }
            if let Some(sequence) = retained_sequence {
                state.last_seen_weapon_discharge_sequence = sequence;
            }
        }
        let (discharges, loco_impulse) = discharges_from_valid_plans(
            visual_plans,
            object_id,
            draw_module_index,
            source_template_name,
            draw_model,
        );
        if let Some(impulse) = loco_impulse {
            state.loco_acceleration_pitch_rate += impulse.loco_pitch_delta();
            state.loco_acceleration_roll_rate += impulse.loco_roll_delta();
        }
        let discharges = discharges.as_slice();

        if let Some(saved) = pending_restore {
            // Pre-frame validation proved the source fields, but only this
            // point owns the freshly loaded model/binding.  Rejecting a bad
            // entry affects no unrelated Draw module.
            let imported = match (&identity.animation, &saved.animation) {
                (None, None) => {
                    let topology = model.weapon_barrel_topology_for_authored_bindings(
                        &draw_model.weapon_bone_bindings,
                    );
                    let Some(recoil_slots) = topology.as_ref().and_then(|topology| {
                        restored_recoil_slots_for_topology(
                            &saved,
                            topology,
                            &draw_model.recoil_kinematics,
                        )
                    }) else {
                        state.force_bind_pose = true;
                        state.animation = None;
                        state.recoil_slots = std::array::from_fn(|_| Vec::new());
                        discard_unvisualizable_discharges(state, &discharges);
                        return (None, 0.0, Vec::new());
                    };
                    state.force_bind_pose = true;
                    state.animation = None;
                    state.last_seen_weapon_discharge_sequence =
                        saved.last_seen_weapon_discharge_sequence;
                    state.recoil_slots = recoil_slots;
                    let controls = recoil_controls_for_topology(
                        state,
                        topology.as_ref(),
                        &draw_model.recoil_kinematics,
                        &discharges,
                    );
                    return (None, 0.0, controls);
                }
                (Some(expected), Some(saved_animation))
                    if expected
                        .hierarchy_animation
                        .eq_ignore_ascii_case(saved_animation.hierarchy_animation.as_str())
                        && expected.mode == saved_animation.mode
                        && saved_animation.frame.is_finite()
                        && saved_animation.frame >= 0.0 =>
                {
                    let binding = Self::cached_draw_animation_binding(
                        model,
                        expected.hierarchy_animation.as_str(),
                    );
                    binding.and_then(|binding| {
                        model
                            .animation_binding_metadata(&binding)
                            .and_then(|(frames, rate)| {
                                (frames > 0
                                    && rate > 0
                                    && saved_animation.frame <= (frames.saturating_sub(1) as f32))
                                    .then_some((binding, frames, rate))
                            })
                    })
                }
                _ => None,
            };

            if let Some((binding, num_frames, frame_rate)) = imported {
                state.force_bind_pose = false;
                state.animation = Some(ObjectAnimationState {
                    animation_binding_key: Some(binding.state_key()),
                    animation_identity: identity
                        .animation
                        .as_ref()
                        .expect("checked Some animation")
                        .hierarchy_animation
                        .clone(),
                    current_frame: saved
                        .animation
                        .as_ref()
                        .expect("checked Some animation")
                        .frame,
                    frame_rate: frame_rate as f32,
                    num_frames,
                    mode: draw_model.animation_mode.clone(),
                });
                let topology = model
                    .weapon_barrel_topology_for_authored_bindings(&draw_model.weapon_bone_bindings);
                let Some(recoil_slots) = topology.as_ref().and_then(|topology| {
                    restored_recoil_slots_for_topology(
                        &saved,
                        topology,
                        &draw_model.recoil_kinematics,
                    )
                }) else {
                    state.force_bind_pose = true;
                    state.animation = None;
                    state.recoil_slots = std::array::from_fn(|_| Vec::new());
                    discard_unvisualizable_discharges(state, &discharges);
                    return (None, 0.0, Vec::new());
                };
                state.last_seen_weapon_discharge_sequence =
                    saved.last_seen_weapon_discharge_sequence;
                state.recoil_slots = recoil_slots;
                let controls = recoil_controls_for_topology(
                    state,
                    topology.as_ref(),
                    &draw_model.recoil_kinematics,
                    &discharges,
                );
                return (
                    Some(binding),
                    state
                        .animation
                        .as_ref()
                        .expect("just restored animation")
                        .current_frame,
                    controls,
                );
            }

            // The frame/source identity matched but fresh model/HAnim did
            // not. C++ has no legal way to reinterpret that cache as another
            // clip; retain a bind/idle state instead of clip zero or retry.
            state.force_bind_pose = true;
            state.animation = None;
            state.recoil_slots = std::array::from_fn(|_| Vec::new());
            return (None, 0.0, Vec::new());
        }

        if state.force_bind_pose {
            let topology = model
                .weapon_barrel_topology_for_authored_bindings(&draw_model.weapon_bone_bindings);
            let controls = recoil_controls_for_topology(
                state,
                topology.as_ref(),
                &draw_model.recoil_kinematics,
                &discharges,
            );
            return (None, 0.0, controls);
        }
        let Some(expected) = identity.animation.as_ref() else {
            state.animation = None;
            let topology = model
                .weapon_barrel_topology_for_authored_bindings(&draw_model.weapon_bone_bindings);
            let controls = recoil_controls_for_topology(
                state,
                topology.as_ref(),
                &draw_model.recoil_kinematics,
                &discharges,
            );
            return (None, 0.0, controls);
        };
        let Some(animation_binding) =
            Self::cached_draw_animation_binding(model, expected.hierarchy_animation.as_str())
        else {
            discard_unvisualizable_discharges(state, &discharges);
            return (None, 0.0, Vec::new());
        };
        let Some((num_frames, frame_rate)) = model.animation_binding_metadata(&animation_binding)
        else {
            discard_unvisualizable_discharges(state, &discharges);
            return (None, 0.0, Vec::new());
        };
        let animation_binding_key = animation_binding.state_key();
        let start_frame = match draw_model.animation_mode {
            crate::assets::AuthoredDrawAnimationMode::LoopBackwards
            | crate::assets::AuthoredDrawAnimationMode::OnceBackwards => {
                num_frames.saturating_sub(1) as f32
            }
            _ => 0.0,
        };
        let reset_animation = state.animation.as_ref().is_none_or(|animation| {
            animation.animation_binding_key != Some(animation_binding_key.clone())
                || !animation
                    .animation_identity
                    .eq_ignore_ascii_case(expected.hierarchy_animation.as_str())
                || animation.mode != draw_model.animation_mode
        });
        if reset_animation {
            state.animation = Some(ObjectAnimationState {
                animation_binding_key: Some(animation_binding_key),
                animation_identity: expected.hierarchy_animation.clone(),
                current_frame: start_frame,
                frame_rate: frame_rate as f32,
                num_frames,
                mode: draw_model.animation_mode.clone(),
            });
        }
        let animation = state
            .animation
            .as_mut()
            .expect("initialized animation state");
        let should_animate = leftover_should_animate_for_presentation(
            self.presentation_frame.as_ref(),
            object_id,
            draw_model.animations_require_power.get(),
        );
        if should_animate && delta_time > 0.0 && delta_time < 1.0 && animation.num_frames > 1 {
            let terminal = (animation.num_frames - 1) as f32;
            let delta = delta_time * animation.frame_rate;
            animation.current_frame = match &animation.mode {
                crate::assets::AuthoredDrawAnimationMode::Manual => animation.current_frame,
                crate::assets::AuthoredDrawAnimationMode::Once => {
                    (animation.current_frame + delta).min(terminal)
                }
                crate::assets::AuthoredDrawAnimationMode::Loop => {
                    if terminal > 0.0 {
                        (animation.current_frame + delta) % terminal
                    } else {
                        0.0
                    }
                }
                crate::assets::AuthoredDrawAnimationMode::OnceBackwards => {
                    (animation.current_frame - delta).max(0.0)
                }
                crate::assets::AuthoredDrawAnimationMode::LoopBackwards => {
                    if terminal > 0.0 {
                        (animation.current_frame - delta).rem_euclid(terminal)
                    } else {
                        0.0
                    }
                }
                crate::assets::AuthoredDrawAnimationMode::LoopPingPong
                | crate::assets::AuthoredDrawAnimationMode::Unsupported(_) => {
                    animation.current_frame
                }
            };
        }
        if should_animate {
            match &animation.mode {
                crate::assets::AuthoredDrawAnimationMode::Once
                    if animation.num_frames > 0
                        && animation.current_frame + 1e-3
                            >= (animation.num_frames.saturating_sub(1) as f32) =>
                {
                    crate::assets::notify_live_draw_animation_complete(
                        object_id.0,
                        draw_module_index,
                    );
                }
                crate::assets::AuthoredDrawAnimationMode::OnceBackwards
                    if animation.current_frame <= 1e-3 =>
                {
                    crate::assets::notify_live_draw_animation_complete(
                        object_id.0,
                        draw_module_index,
                    );
                }
                _ => {}
            }
        }

        let current_frame = animation.current_frame;
        let topology =
            model.weapon_barrel_topology_for_authored_bindings(&draw_model.weapon_bone_bindings);
        let controls = recoil_controls_for_topology(
            state,
            topology.as_ref(),
            &draw_model.recoil_kinematics,
            &discharges,
        );
        (Some(animation_binding), current_frame, controls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_drawable_identity_requires_exact_frozen_draw_state() {
        let identity = FrozenVisualDrawIdentity {
            source_template_name: "GLATankScorpion".into(),
            model_key: "UVLiteTank".into(),
            selected_condition_state_index: 7,
            animation: Some(FrozenVisualAnimationIdentity {
                hierarchy_animation: "UVLiteTank.UVLiteTank".into(),
                mode: ClientDrawableAnimationMode::Loop,
            }),
        };
        let mut saved = ClientDrawableStateSnapshot {
            object_id: 12,
            draw_module_index: 3,
            source_template_name: "glatankscorpion".into(),
            model_key: "uvlitetank".into(),
            selected_condition_state_index: 7,
            animation: Some(ClientDrawableAnimationSnapshot {
                hierarchy_animation: "uvlitetank.uvlitetank".into(),
                frame: 2.0,
                mode: ClientDrawableAnimationMode::Loop,
            }),
            ..Default::default()
        };
        assert!(visual_identity_matches_snapshot(&identity, &saved));
        saved.selected_condition_state_index = 8;
        assert!(!visual_identity_matches_snapshot(&identity, &saved));
    }

    #[test]
    fn client_drawable_capture_skips_unresolved_hanim_but_keeps_explicit_bind_pose() {
        let bind_state = ObjectVisualState {
            identity: Some(FrozenVisualDrawIdentity {
                source_template_name: "BindOnly".into(),
                model_key: "BindOnly".into(),
                selected_condition_state_index: 0,
                animation: None,
            }),
            ..Default::default()
        };
        assert!(
            snapshot_from_visual_state(1, 0, &bind_state)
                .expect("explicit source bind pose is durable")
                .animation
                .is_none()
        );

        let unresolved = ObjectVisualState {
            identity: Some(FrozenVisualDrawIdentity {
                source_template_name: "NeedsAnim".into(),
                model_key: "NeedsAnim".into(),
                selected_condition_state_index: 0,
                animation: Some(FrozenVisualAnimationIdentity {
                    hierarchy_animation: "NeedsAnim.Run".into(),
                    mode: ClientDrawableAnimationMode::Loop,
                }),
            }),
            ..Default::default()
        };
        assert!(snapshot_from_visual_state(1, 0, &unresolved).is_none());
    }

    #[test]
    fn culled_module_retains_recoil_start_without_a_queue() {
        let plan = crate::presentation_frame::FrozenWeaponVisualDispatchPlan {
            discharge: crate::presentation_frame::FrozenWeaponVisualDischarge {
                world_epoch: 1,
                source: crate::game_logic::ObjectId(41),
                source_object_generation: 1,
                weapon_slot: 0,
                fired_barrel: 0,
                sequence: 4,
                logic_frame: 10,
            },
            fx_route: crate::presentation_frame::FrozenWeaponVisualFxRoute::BroadcastWithoutFireFx,
            targets: vec![
                crate::presentation_frame::FrozenWeaponVisualDispatchTarget {
                    state: crate::presentation_frame::FrozenWeaponVisualDrawState {
                        draw_module_index: 2,
                        source_template_name: "CullHost".into(),
                        model_key: "CullMesh".into(),
                        selected_condition_state_index: 0,
                        draw_state_revision: 1,
                    },
                    starts_recoil_or_muzzle: true,
                    stops_after_fire_fx: false,
                },
            ],
            fire_fx_falls_back_to_drawable_position: false,
            impulse: crate::presentation_frame::FrozenWeaponVisualImpulse::default(),
        };
        assert!(plan.is_valid());
        let mut states = HashMap::new();
        retain_fire_starts_from_plans(&mut states, &[plan]);
        let state = states.get(&(41, 2)).expect("retained visual state");
        assert_eq!(state.last_seen_weapon_discharge_sequence, 4);
        assert_eq!(
            state.recoil_slots[0][0].phase,
            crate::save_load::snapshot::ClientDrawableRecoilPhase::RecoilStart
        );
    }

    #[test]
    fn leftover_get_should_animate_pauses_emp_hacked_underpowered() {
        assert!(!gamelogic::object::draw::object_should_animate_flags(
            true, false, true, false, false, false, true, false, false, false
        ));
        assert!(!gamelogic::object::draw::object_should_animate_flags(
            true, false, true, false, true, false, false, false, false, false
        ));
        assert!(!gamelogic::object::draw::object_should_animate_flags(
            true, false, true, false, false, false, false, false, false, true
        ));
        assert!(leftover_should_animate_for_presentation(
            None,
            crate::game_logic::ObjectId(1),
            true
        ));
    }

    #[test]
    fn leftover_hidden_status_deselects_hidden_or_stealth() {
        assert!(gamelogic::object::draw::leftover_hidden_status_deselects(
            true, false
        ));
        assert!(gamelogic::object::draw::leftover_hidden_status_deselects(
            false, true
        ));
        assert!(!gamelogic::object::draw::leftover_hidden_status_deselects(
            false, false
        ));
    }
}
