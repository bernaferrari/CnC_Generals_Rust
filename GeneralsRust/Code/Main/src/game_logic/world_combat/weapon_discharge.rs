//! Exact accepted WeaponSet-discharge authority.
//!
//! This is the narrow bridge between a real live weapon discharge and frozen
//! Drawable presentation. It intentionally does not observe the AI
//! `host_fire_intent` residual: those writes can occur without a physical
//! `Weapon::fireWeaponTemplate` equivalent.

use super::super::*;

/// Owned continuation across fire effects and damage callbacks. The captured
/// barrel and source generation survive without retaining a mutable object loan.
pub(in crate::game_logic) struct AcceptedWeaponDischarge {
    pub(in crate::game_logic) has_authored_fire_fx: bool,
    event: crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent,
    source_generation: u64,
    skip_object_barrel: bool,
}

impl GameLogic {
    /// Normalize one already-accepted live discharge.
    ///
    /// Callers invoke this only after their concrete WeaponSet slot consumed
    /// ammo / completed its accepted fire path. The helper reads the barrel
    /// before advancing it, allocates a world-monotonic sequence, stamps the
    /// durable Object marker, and appends the presentation-only event in one
    /// operation. Lower-level per-victim damage helpers must not call this:
    /// a splash weapon still produces one recoil cue, not one per victim.
    pub(in super::super) fn record_accepted_weapon_discharge(
        &mut self,
        source: ObjectId,
        weapon_slot: u8,
    ) -> Option<crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent> {
        let accepted = self.begin_accepted_weapon_discharge(source, weapon_slot)?;
        Some(self.finish_accepted_weapon_discharge(accepted))
    }

    pub(in super::super) fn record_accepted_weapon_discharge_with_effects(
        &mut self,
        source: ObjectId,
        weapon_slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent> {
        let accepted = self.begin_accepted_weapon_discharge_with_effects(
            source,
            weapon_slot,
            presentation_effects,
        )?;
        Some(self.finish_accepted_weapon_discharge(accepted))
    }

    pub(in crate::game_logic) fn begin_accepted_weapon_discharge(
        &mut self,
        source: ObjectId,
        weapon_slot: u8,
    ) -> Option<AcceptedWeaponDischarge> {
        self.begin_accepted_weapon_discharge_with_dispatch(
            source,
            weapon_slot,
            Some(leftover_handle_weapon_fire_fx_at_fx_bone),
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn begin_accepted_weapon_discharge_with_effects(
        &mut self,
        source: ObjectId,
        weapon_slot: u8,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<AcceptedWeaponDischarge> {
        self.begin_accepted_weapon_discharge_with_dispatch(
            source,
            weapon_slot,
            None,
            presentation_effects,
        )
    }

    fn begin_accepted_weapon_discharge_with_dispatch(
        &mut self,
        source: ObjectId,
        weapon_slot: u8,
        compatibility_fx: Option<
            fn(ObjectId, &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture) -> bool,
        >,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<AcceptedWeaponDischarge> {
        if weapon_slot >= 3 {
            return None;
        }

        // C++ `Weapon::privateFireWeapon` queries the current Drawable's
        // exact barrel count immediately before it captures/advances the
        // slot cursor. Main's equivalent is a cache-only host configuration:
        // successful map/start prewarm may have validated this exact Draw
        // state; an unavailable/busy/unsupported source deliberately leaves
        // the existing one-barrel or staged-restore cursor untouched.
        let _ = self.configure_cached_weapon_barrel_topology_for_object(source);

        let pending = self
            .objects
            .get_mut(&source)
            .and_then(|object| object.take_pending_weapon_visual_capture());
        let locally_controlled = self.objects.get(&source).is_some_and(|object| {
            source_is_locally_controlled(object.owner_player_id, self.local_player_id())
        });
        let source_object_generation = self.stamp_visual_object_generation(source);

        // Retain the pre-advance barrel exactly once. If no concrete source
        // Weapon is attached, fail closed rather than inventing PRIMARY/0.
        let fired_barrel = pending
            .as_ref()
            .map(|capture| capture.fired_barrel)
            .or_else(|| {
                self.objects
                    .get_mut(&source)?
                    .fired_barrel_for_slot(weapon_slot)
            })?;

        // Zero is reserved for an unseen Object marker. A malformed restored
        // counter is normalized by the setter below; keep this defensive guard
        // too because this is the only allocator.
        let sequence = self.next_weapon_discharge_sequence.max(1);
        self.next_weapon_discharge_sequence = sequence.saturating_add(1).max(1);
        let visual_plan = pending.as_ref().and_then(|capture| {
            self.freeze_pending_weapon_visual_plan(
                source,
                capture,
                sequence,
                source_object_generation,
                locally_controlled,
            )
        });
        if let (Some(capture), Some(plan)) = (pending.as_ref(), visual_plan.as_ref()) {
            self.play_dispatch_fire_fx_with_dispatch(
                source,
                capture,
                plan,
                compatibility_fx,
                presentation_effects,
            );
        } else if let Some(capture) = pending.as_ref().filter(|capture| {
            capture.source_gate(locally_controlled).fx_route()
                == Some(crate::presentation_frame::FrozenWeaponVisualFxRoute::BroadcastWithFireFx)
        }) {
            // No frozen Drawable plan (missing modules / probe fail-closed).
            // C++ still calls handleWeaponFireFX, then FXList::doFXPos when
            // FireFX is non-null and leftover did not consume the shot.
            if compatibility_fx.is_some_and(|dispatch| dispatch(source, capture)) {
                // Leftover W3DModelDraw played FireFX at the FX-bone matrix.
            } else if !capture.selected_fx_name.is_empty() {
                // C++ Weapon.cpp:923-926 plays at the fired barrel's FX-bone
                // world matrix; the source position is only the no-topology
                // fallback. Contact weapons detonate at the victim instead.
                let fire_fx_bone = if capture.is_contact_weapon {
                    None
                } else {
                    self.weapon_fire_fx_bone_world(
                        source,
                        capture.weapon_slot,
                        capture.fired_barrel,
                    )
                };
                let pos = fire_fx_bone
                    .as_ref()
                    .map(|(pos, _)| *pos)
                    .unwrap_or_else(|| {
                        Vec3::new(
                            capture.source_pos[0],
                            capture.source_pos[1],
                            capture.source_pos[2],
                        )
                    });
                let target = capture.target_pos.map(|p| Vec3::new(p[0], p[1], p[2]));
                let speed = fire_fx_weapon_speed(self.objects.get(&source), capture.weapon_slot);
                let radius =
                    fire_fx_primary_damage_radius(self.objects.get(&source), capture.weapon_slot);
                let matrix = self
                    .objects
                    .get(&source)
                    .map(|o| o.get_transform_matrix())
                    .map(|m| match fire_fx_bone {
                        Some((_, bone_yaw)) => m * glam::Mat4::from_rotation_y(bone_yaw),
                        None => m,
                    });
                let _ = self.spawn_receiving_weapon_fire_fx_with_effects(
                    pos,
                    target,
                    capture.logic_frame,
                    source,
                    capture.target_id,
                    &capture.selected_fx_name,
                    "",
                    "",
                    "",
                    speed,
                    radius,
                    matrix,
                    presentation_effects,
                );
            }
        }
        let event = crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent {
            source,
            weapon_slot,
            fired_barrel,
            sequence,
            logic_frame: self.frame,
            visual_plan,
        };
        Some(AcceptedWeaponDischarge {
            has_authored_fire_fx: pending
                .as_ref()
                .is_some_and(|capture| capture.selected_fx_is_present),
            event,
            source_generation: source_object_generation,
            skip_object_barrel: pending
                .as_ref()
                .is_some_and(|capture| capture.skip_object_barrel_advance),
        })
    }

    /// Advance only the original receiving source after its shot callback.
    pub(in crate::game_logic) fn finish_accepted_weapon_discharge(
        &mut self,
        accepted: AcceptedWeaponDischarge,
    ) -> crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent {
        let event = accepted.event;
        if let Some(object) = self
            .objects
            .get_mut(&event.source)
            .filter(|object| object.visual_object_generation == accepted.source_generation)
        {
            // Both combat owners count the same accepted discharge, never an
            // aiming-only update or a replacement source with the same ID.
            if crate::game_logic::host_slave_drones::is_hellfire_drone_template(
                &object.template_name,
            ) {
                self.hellfire_drone_residual_auto_fires =
                    self.hellfire_drone_residual_auto_fires.saturating_add(1);
            }
            object.stamp_weapon_discharge_marker(WeaponDischargeMarker {
                sequence: event.sequence,
                weapon_slot: event.weapon_slot,
                fired_barrel: event.fired_barrel,
                logic_frame: event.logic_frame,
            });
            if !accepted.skip_object_barrel {
                object.advance_weapon_barrel_after_shot(event.weapon_slot);
            }
        }
        // A callback may destroy the source; its already accepted effect is
        // still published, but a replacement with the same ID is never changed.
        self.weapon_discharge_log.record(event.clone());
        event
    }

    /// The next unused logical sequence for v4 save capture.
    #[inline]
    pub fn weapon_discharge_next_sequence_for_snapshot(&self) -> u64 {
        self.next_weapon_discharge_sequence.max(1)
    }

    /// Restore the next unused logical sequence. Presentation events are
    /// transient, so a staged load deliberately begins with an empty queue;
    /// durable per-object markers establish the no-replay baseline instead.
    #[inline]
    pub fn restore_weapon_discharge_next_sequence(&mut self, next_sequence: u64) {
        self.next_weapon_discharge_sequence = next_sequence.max(1);
        self.weapon_discharge_log.clear();
    }

    #[inline]
    fn play_dispatch_fire_fx(
        &mut self,
        source: ObjectId,
        capture: &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture,
        plan: &crate::presentation_frame::FrozenWeaponVisualDispatchPlan,
    ) {
        self.play_dispatch_fire_fx_with_dispatch(
            source,
            capture,
            plan,
            Some(leftover_handle_weapon_fire_fx_at_fx_bone),
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn play_dispatch_fire_fx_with_effects(
        &mut self,
        source: ObjectId,
        capture: &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture,
        plan: &crate::presentation_frame::FrozenWeaponVisualDispatchPlan,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.play_dispatch_fire_fx_with_dispatch(source, capture, plan, None, presentation_effects)
    }

    fn play_dispatch_fire_fx_with_dispatch(
        &mut self,
        source: ObjectId,
        capture: &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture,
        plan: &crate::presentation_frame::FrozenWeaponVisualDispatchPlan,
        compatibility_fx: Option<
            fn(ObjectId, &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture) -> bool,
        >,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::presentation_frame::FrozenWeaponVisualFxRoute;
        if plan.fx_route != FrozenWeaponVisualFxRoute::BroadcastWithFireFx {
            return;
        }
        if capture.selected_fx_name.is_empty() {
            return;
        }
        let source_pos = Vec3::new(
            capture.source_pos[0],
            capture.source_pos[1],
            capture.source_pos[2],
        );
        let target_pos = self.resolved_visual_target_pos(capture);
        // Leftover handle_weapon_fire_fx is RIGHT (W3DModelDraw.cpp:3672-3727).
        // C++ Weapon.cpp:923-939 only falls back to drawable/contact origin
        // when leftover returns false.
        if compatibility_fx.is_some_and(|dispatch| dispatch(source, capture)) {
            return;
        }
        // C++ Weapon.cpp:923-926 plays at the fired barrel's FX-bone world
        // matrix; the source position is only the no-topology fallback.
        // Contact weapons detonate at the victim instead.
        let fire_fx_bone = if capture.is_contact_weapon {
            None
        } else {
            self.weapon_fire_fx_bone_world(source, capture.weapon_slot, capture.fired_barrel)
        };
        let where_pos = if capture.is_contact_weapon {
            target_pos
        } else {
            fire_fx_bone
                .as_ref()
                .map(|(pos, _)| *pos)
                .unwrap_or(source_pos)
        };
        let speed = fire_fx_weapon_speed(self.objects.get(&source), capture.weapon_slot);
        let radius = fire_fx_primary_damage_radius(self.objects.get(&source), capture.weapon_slot);
        let matrix = self
            .objects
            .get(&source)
            .map(|o| o.get_transform_matrix())
            .map(|m| match fire_fx_bone {
                Some((_, bone_yaw)) => m * glam::Mat4::from_rotation_y(bone_yaw),
                None => m,
            });
        let _ = self.spawn_receiving_weapon_fire_fx_with_effects(
            where_pos,
            Some(target_pos),
            capture.logic_frame,
            source,
            capture.target_id,
            &capture.selected_fx_name,
            "",
            "",
            "",
            speed,
            radius,
            matrix,
            presentation_effects,
        );
    }

    /// Move one accepted-shot batch out of this driving world at publication.
    /// Borrowed frame queries must not call this mutating transport boundary.
    pub fn take_weapon_discharges_for_presentation(
        &mut self,
    ) -> Vec<crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent> {
        self.weapon_discharge_log.take_for_presentation()
    }
}

/// C++ `Weapon::getWeaponSpeed()` for FireFX / TracerFXNugget primarySpeed.
pub(in super::super) fn fire_fx_weapon_speed(object: Option<&Object>, slot: u8) -> f32 {
    let Some(object) = object else {
        return 0.0;
    };
    if let Some(rule) = object.selected_native_weapon_rule(slot) {
        return if rule.weapon_speed > 0.0 && rule.weapon_speed < 999_000.0 {
            rule.weapon_speed
        } else {
            0.0
        };
    }
    if let Some(weapon) = object.weapon_slot(slot).or(object.weapon.as_ref()) {
        if weapon.projectile_speed > 0.0 && weapon.projectile_speed < 999_000.0 {
            return weapon.projectile_speed;
        }
    }
    let Some(name) = object.weapon_name_for_slot(slot) else {
        return 0.0;
    };
    let peel = crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(name);
    if peel.weapon_speed > 0.0 && peel.weapon_speed < 999_000.0 {
        peel.weapon_speed
    } else {
        0.0
    }
}

/// C++ `Weapon::getPrimaryDamageRadius(bonus)` for FireFX `overrideRadius`.
pub(in super::super) fn fire_fx_primary_damage_radius(object: Option<&Object>, slot: u8) -> f32 {
    if let Some(rule) = object.and_then(|object| object.selected_native_weapon_rule(slot)) {
        return rule.primary_damage_radius;
    }
    let Some(name) = object.and_then(|o| o.weapon_name_for_slot(slot)) else {
        return 0.0;
    };
    crate::game_logic::weapon_bootstrap::host_primary_damage_radius_for_weapon_name(name)
}

/// Leftover `Drawable::handle_weapon_fire_fx` / `W3DModelDraw::handle_weapon_fire_fx`.
/// Plays FireFX at the barrel FX-bone world matrix when leftover handles it.
fn leftover_handle_weapon_fire_fx_at_fx_bone(
    source: ObjectId,
    capture: &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture,
) -> bool {
    let leftover_obj = crate::game_logic::crate_object_bridge::crate_object(source.0)
        .or_else(|| gamelogic::object::registry::OBJECT_REGISTRY.get_object(source.0));
    let Some(leftover_obj) = leftover_obj else {
        return false;
    };
    let (drawable, source_pose) = {
        let Ok(guard) = leftover_obj.read() else {
            return false;
        };
        (
            guard.get_drawable(),
            gamelogic::object::draw::draw_module::WeaponFireFxSource {
                position: *guard.get_position(),
                transform: guard.get_transform_matrix(),
            },
        )
    };
    let Some(drawable) = drawable else {
        return false;
    };
    let Ok(mut draw_guard) = drawable.write() else {
        return false;
    };
    let slot = match capture.weapon_slot {
        0 => gamelogic::common::WeaponSlotType::Primary,
        1 => gamelogic::common::WeaponSlotType::Secondary,
        _ => gamelogic::common::WeaponSlotType::Tertiary,
    };
    let victim = leftover_fire_fx_victim_coord(capture);
    draw_guard.handle_weapon_fire_fx(
        slot,
        i32::from(capture.fired_barrel),
        None,
        &victim,
        0.0,
        0.0,
        Some(&source_pose),
    )
}

/// Host Y-up `(x, height, z_ground)` → leftover/C++ Z-up `(x, y_ground, z_height)`.
fn leftover_fire_fx_victim_coord(
    capture: &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture,
) -> gamelogic::common::Coord3D {
    let p = capture.target_pos.unwrap_or(capture.source_pos);
    gamelogic::common::Coord3D::new(p[0], p[2], p[1])
}

/// The receiving session supplies immediate FX while the particle registry is
/// borrowed independently. Both authored shot paths use this same boundary.
impl GameLogic {
    pub(crate) fn spawn_receiving_weapon_fire_fx(
        &mut self,
        muzzle_position: Vec3,
        impact_position: Option<Vec3>,
        logic_frame: u32,
        shooter: ObjectId,
        target: Option<ObjectId>,
        fire_fx: &str,
        detonation_fx: &str,
        fire_ocl: &str,
        detonation_ocl: &str,
        weapon_speed: f32,
        damage_radius: f32,
        drawable_transform: Option<glam::Mat4>,
    ) -> Vec<u32> {
        self.spawn_receiving_weapon_fire_fx_with_effects(
            muzzle_position,
            impact_position,
            logic_frame,
            shooter,
            target,
            fire_fx,
            detonation_fx,
            fire_ocl,
            detonation_ocl,
            weapon_speed,
            damage_radius,
            drawable_transform,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_receiving_weapon_fire_fx_with_effects(
        &mut self,
        muzzle_position: Vec3,
        impact_position: Option<Vec3>,
        logic_frame: u32,
        shooter: ObjectId,
        target: Option<ObjectId>,
        fire_fx: &str,
        detonation_fx: &str,
        fire_ocl: &str,
        detonation_ocl: &str,
        weapon_speed: f32,
        damage_radius: f32,
        drawable_transform: Option<glam::Mat4>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Vec<u32> {
        #[cfg(feature = "game_client")]
        {
            // The registry only records presentation marker IDs; its dispatch
            // callback cannot mutate gameplay. Finish that bounded bookkeeping
            // loan, then run every original nugget synchronously in source order
            // before returning to the weapon continuation.
            let mut callbacks = Vec::new();
            let ids = self
                .combat_particles
                .spawn_weapon_fire_fx_named_ocl_oriented_with_dispatch(
                    muzzle_position,
                    impact_position,
                    logic_frame,
                    shooter,
                    target,
                    fire_fx,
                    detonation_fx,
                    fire_ocl,
                    detonation_ocl,
                    weapon_speed,
                    damage_radius,
                    drawable_transform,
                    &mut |name, position, secondary, speed, radius, matrix| {
                        callbacks.push((
                            name.to_owned(),
                            position,
                            secondary,
                            speed,
                            radius,
                            matrix,
                        ));
                        true
                    },
                );
            for (name, position, secondary, speed, radius, matrix) in callbacks {
                self.dispatch_fx_list_at_host_position_oriented_with_effects(
                    &name,
                    position,
                    secondary,
                    speed,
                    radius,
                    matrix,
                    presentation_effects,
                );
            }
            ids
        }
        #[cfg(not(feature = "game_client"))]
        {
            self.combat_particles
                .spawn_weapon_fire_fx_named_ocl_oriented(
                    muzzle_position,
                    impact_position,
                    logic_frame,
                    shooter,
                    target,
                    fire_fx,
                    detonation_fx,
                    fire_ocl,
                    detonation_ocl,
                    weapon_speed,
                    damage_radius,
                    drawable_transform,
                )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_discharge_normalizes_preadvance_barrel_and_freezes_every_event() {
        let mut logic = GameLogic::new();
        logic.frame = 77;
        let source = ObjectId(701);
        let mut object = Object::new(ThingTemplate::new("DischargeSource"), source, Team::USA);
        object.weapon = Some(Weapon {
            damage: 1.0,
            range: 100.0,
            ..Weapon::default()
        });
        assert!(object.set_weapon_barrel_count_for_slot(0, 3));
        object.weapon_barrel_states[0].current_barrel = 2;
        object.weapon_barrel_states[0].shots_left_on_barrel = 1;
        logic.objects.insert(source, object);

        let first = logic
            .record_accepted_weapon_discharge(source, 0)
            .expect("first accepted primary discharge");
        let second = logic
            .record_accepted_weapon_discharge(source, 0)
            .expect("second accepted primary discharge");
        assert_eq!((first.sequence, first.fired_barrel), (1, 2));
        assert_eq!((second.sequence, second.fired_barrel), (2, 0));
        assert_eq!(logic.weapon_discharge_next_sequence_for_snapshot(), 3);

        let marker = logic
            .host_object(source)
            .expect("source")
            .weapon_discharge_marker();
        assert_eq!(
            marker,
            WeaponDischargeMarker {
                sequence: 2,
                weapon_slot: 0,
                fired_barrel: 0,
                logic_frame: 77,
            }
        );
        assert_eq!(
            logic
                .host_object(source)
                .expect("source")
                .weapon_barrel_state_for_slot(0)
                .expect("primary barrel")
                .current_barrel,
            1,
            "two accepted shots must advance after capturing barrels 2 then 0"
        );

        // A presentation frame can trail multiple fixed logic steps. It must
        // receive both accepted events, rather than only the last drain batch.
        let frame =
            crate::presentation_frame::PresentationFrame::publish_for_engine(&mut logic, 0, None);
        let frozen: Vec<_> = frame
            .events
            .iter()
            .filter_map(|event| match event {
                crate::presentation_frame::PresentationEvent::WeaponDischarged {
                    source: event_source,
                    weapon_slot,
                    fired_barrel,
                    sequence,
                    logic_frame,
                    ..
                } if *event_source == source => {
                    Some((*sequence, *weapon_slot, *fired_barrel, *logic_frame))
                }
                _ => None,
            })
            .collect();
        assert_eq!(frozen, vec![(1, 0, 2, 77), (2, 0, 0, 77)]);
        assert!(
            crate::presentation_frame::PresentationFrame::publish_for_engine(&mut logic, 0, None)
                .events
                .iter()
                .all(|event| !matches!(
                    event,
                    crate::presentation_frame::PresentationEvent::WeaponDischarged { .. }
                )),
            "accepted discharges must be frozen once, not replayed forever"
        );
    }

    #[test]
    fn weapon_discharge_restore_clamps_counter_and_rejects_bad_marker_slot() {
        let mut logic = GameLogic::new();
        logic.restore_weapon_discharge_next_sequence(0);
        assert_eq!(logic.weapon_discharge_next_sequence_for_snapshot(), 1);

        let mut object = Object::new(ThingTemplate::new("MarkerSource"), ObjectId(702), Team::USA);
        assert!(!object.restore_weapon_discharge_marker(9, 3, 1, 22));
        assert_eq!(
            object.weapon_discharge_marker(),
            WeaponDischargeMarker::unseen()
        );
    }

    #[test]
    fn fire_fx_primary_damage_radius_uses_weapon_template() {
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        let mut tpl = ThingTemplate::new("RadiusHowitzer");
        tpl.set_primary_weapon_name("HowitzerBogus");
        let object = Object::new(tpl, ObjectId(703), Team::USA);
        assert!(
            (fire_fx_primary_damage_radius(Some(&object), 0) - 25.0).abs() < f32::EPSILON,
            "seeded howitzer PrimaryDamageRadius residual"
        );
        assert_eq!(fire_fx_primary_damage_radius(None, 0), 0.0);
    }
}

#[cfg(test)]
#[path = "weapon_discharge_owner_tests.rs"]
mod accepted_discharge_owner_boundary_tests;
