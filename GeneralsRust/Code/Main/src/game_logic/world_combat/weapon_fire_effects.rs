//! CPP Weapon.cpp889–949: fire FX, then fire OCL, before damage/flight.
use super::super::*;
use super::AcceptedWeaponDischarge;

impl GameLogic {
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
            let (team, orientation, velocity) = (
                object.team,
                object.get_orientation(),
                object.movement.velocity,
            );
            if !ocl.trim().is_empty() && !ocl.eq_ignore_ascii_case("None") {
                self.execute_parsed_weapon_ocl_at(
                    &ocl,
                    Some(source),
                    team,
                    rank,
                    orientation,
                    velocity,
                    origin,
                );
            }
        }
        Some(accepted)
    }
}
