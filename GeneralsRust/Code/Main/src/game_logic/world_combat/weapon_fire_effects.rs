//! CPP Weapon.cpp889–949: fire FX, then fire OCL, before damage/flight.
use super::super::*;
use super::AcceptedWeaponDischarge;

impl GameLogic {
    /// Finish one prepared shot synchronously, after releasing its Object loan.
    /// All callers share CPP FireFX -> FireOCL -> delivery -> barrel ordering.
    pub(in crate::game_logic) fn deliver_owned_weapon_shot(
        &mut self,
        slot: u8,
        mut shot: crate::game_logic::combat::PendingProjectile,
        capture: Option<super::weapon_visual_capture::PendingWeaponVisualDispatchCapture>,
    ) -> bool {
        let accepted = self.begin_owned_weapon_effects(
            shot.shooter_id,
            slot,
            shot.target_id,
            shot.target_pos,
            shot.shooter_pos,
            capture,
        );
        // Accepted effects belong to this callback, never to later flight.
        shot.fire_fx_name.clear();
        shot.fire_ocl_name.clear();
        self.materialize_owned_passive_shot(shot);
        if let Some(accepted) = accepted {
            self.finish_accepted_weapon_discharge(accepted);
            true
        } else {
            false
        }
    }

    pub(in crate::game_logic) fn begin_owned_weapon_effects(
        &mut self,
        source: ObjectId,
        slot: u8,
        target: Option<ObjectId>,
        target_position: Option<Vec3>,
        origin: Vec3,
        prepared_capture: Option<super::weapon_visual_capture::PendingWeaponVisualDispatchCapture>,
    ) -> Option<AcceptedWeaponDischarge> {
        let object = self.objects.get(&source)?;
        let name = object.weapon_name_for_slot(slot).map(str::to_owned);
        let rank = object.experience.level;
        let fx = name.as_deref().map(|name| {
            self.weapon_effect_definitions.fire(name, rank).map(|effects| effects.fire_fx)
                .unwrap_or_else(|| crate::game_logic::weapon_bootstrap::host_fire_fx_for_weapon_name_at_veterancy(name, rank))
        }).unwrap_or_default();
        let play_when_stealthed = name.as_deref().and_then(|name| {
            self.weapon_effect_definitions
                .fire(name, rank)
                .map(|effects| effects.play_fx_when_stealthed)
        });
        let object = self.objects.get_mut(&source)?;
        if let Some(capture) = prepared_capture {
            object.pending_weapon_visual_capture = Some(capture);
        } else {
            object.capture_pending_weapon_visual_dispatch(
                slot,
                self.frame,
                target,
                target_position,
            );
        }
        if let Some(capture) = object.pending_weapon_visual_capture.as_mut() {
            capture.source_pos = [origin.x, origin.y, origin.z];
            capture.selected_fx_is_present =
                !fx.trim().is_empty() && !fx.eq_ignore_ascii_case("None");
            capture.selected_fx_name = fx;
            if let Some(play) = play_when_stealthed {
                capture.weapon_plays_fx_when_stealthed = play;
            }
        }
        let accepted = self.begin_accepted_weapon_discharge(source, slot)?;
        // CPP943 samples the source rank again AFTER FireFX. No object borrow
        // spans this callback, and no queued shot can repeat these effects.
        if let Some(object) = self.objects.get(&source) {
            let rank = object.experience.level;
            let ocl = name.as_deref().map(|name| {
                self.weapon_effect_definitions.fire(name, rank).map(|effects| effects.fire_ocl)
                    .unwrap_or_else(|| crate::game_logic::weapon_bootstrap::host_fire_ocl_for_weapon_name_at_veterancy(name, rank))
            }).unwrap_or_default();
            let (team, orientation, velocity, source_position) = (
                object.team,
                object.get_orientation(),
                object.movement.velocity,
                object.get_position(),
            );
            if !ocl.trim().is_empty() && !ocl.eq_ignore_ascii_case("None") {
                self.execute_parsed_weapon_ocl_at(
                    &ocl,
                    Some(source),
                    team,
                    rank,
                    orientation,
                    velocity,
                    source_position,
                );
            }
        }
        Some(accepted)
    }
}
