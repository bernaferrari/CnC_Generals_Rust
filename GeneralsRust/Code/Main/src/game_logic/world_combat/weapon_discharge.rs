//! Exact accepted WeaponSet-discharge authority.
//!
//! This is the narrow bridge between a real live weapon discharge and frozen
//! Drawable presentation. It intentionally does not observe the AI
//! `host_fire_intent` residual: those writes can occur without a physical
//! `Weapon::fireWeaponTemplate` equivalent.

use super::super::*;

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
            self.play_dispatch_fire_fx(source, capture, plan);
        } else if let Some(capture) = pending.as_ref() {
            // No frozen Drawable plan (missing modules / probe fail-closed).
            // C++ still falls through to FXList::doFXPos when FireFX is non-null.
            if !capture.selected_fx_name.is_empty() {
                let pos = Vec3::new(
                    capture.source_pos[0],
                    capture.source_pos[1],
                    capture.source_pos[2],
                );
                let target = capture
                    .target_pos
                    .map(|p| Vec3::new(p[0], p[1], p[2]));
                let speed = fire_fx_weapon_speed(self.objects.get(&source), capture.weapon_slot);
                let _ = crate::game_logic::dispatch_fx_list_at_pos_ex(
                    &capture.selected_fx_name,
                    pos,
                    target,
                    speed,
                    0.0,
                );
            }
        }
        let marker = WeaponDischargeMarker {
            sequence,
            weapon_slot,
            fired_barrel,
            logic_frame: self.frame,
        };
        let event = crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent {
            source,
            weapon_slot,
            fired_barrel,
            sequence,
            logic_frame: self.frame,
            visual_plan,
        };
        self.objects
            .get_mut(&source)?
            .stamp_weapon_discharge_marker(marker);
        // C++ passes this precise barrel to Drawable before Weapon advances.
        // Freeze the source event at the same point; the retained cursor moves
        // only after the event/marker identity is fully established.
        self.weapon_discharge_log.record(event.clone());
        let skip_object_barrel = pending
            .as_ref()
            .is_some_and(|capture| capture.skip_object_barrel_advance);
        if !skip_object_barrel {
            self.objects
                .get_mut(&source)?
                .advance_weapon_barrel_after_shot(weapon_slot);
        }
        Some(event)
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

    /// Consume every accepted discharge accumulated since the previous
    /// presentation build. This has `&self` deliberately: taking a snapshot
    /// is a visual boundary, not an authority mutation.
    #[inline]
    fn play_dispatch_fire_fx(
        &mut self,
        source: ObjectId,
        capture: &super::weapon_visual_capture::PendingWeaponVisualDispatchCapture,
        plan: &crate::presentation_frame::FrozenWeaponVisualDispatchPlan,
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
        // C++ handleWeaponFireFX plays at the FX bone when handled, else
        // Weapon.cpp:934-939 doFXPos at drawable/contact `where`. Live host
        // has no leftover bone matrix; source pos is the barrel-origin residual.
        // spawn_weapon_fire_fx_named_ocl dispatches the authored FXList with
        // victim secondary so Tracer/RayEffect nuggets run.
        let where_pos = if capture.is_contact_weapon {
            target_pos
        } else {
            source_pos
        };
        let speed = fire_fx_weapon_speed(self.objects.get(&source), capture.weapon_slot);
        let _ = self.combat_particles.spawn_weapon_fire_fx_named_ocl(
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
        );
    }

    pub fn take_weapon_discharges_for_presentation(
        &self,
    ) -> Vec<crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent> {
        self.weapon_discharge_log.take_for_presentation()
    }
}

/// C++ `Weapon::getWeaponSpeed()` for FireFX / TracerFXNugget primarySpeed.
fn fire_fx_weapon_speed(object: Option<&Object>, slot: u8) -> f32 {
    let Some(object) = object else {
        return 0.0;
    };
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
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0);
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
            crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
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
}
