//! C++ contained passengers fire their current weapon without leaving AI_BUSY.
use super::super::*;

impl GameLogic {
    pub(in crate::game_logic) fn fire_owned_passive_weapon(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        slot: u8,
        origin: Vec3,
    ) -> bool {
        let Some(victim) = self.objects.get(&target).filter(|o| o.is_alive()) else {
            return false;
        };
        let infantry = victim.is_kind_of(KindOf::Infantry);
        let faerie = victim.is_faerie_fire();
        let Some(shooter) = self.objects.get_mut(&source) else {
            return false;
        };
        let Some((slot, mut shot)) = shooter.prepare_passive_weapon_shot(
            slot,
            target,
            self.frame as f32 * LOGIC_FRAME_TIMESTEP,
            infantry,
            faerie,
            self.frame,
            origin,
            &mut self.combat_system.fire_sound_loop_events,
            &mut self.attack_events,
        ) else {
            return false;
        };
        // The accepted Drawable dispatch owns this selected FireFX, including
        // its visibility/suspension decision. Preserve the separate FireOCL.
        let visual_owns_fx =
            shooter
                .pending_weapon_visual_capture
                .as_mut()
                .is_some_and(|capture| {
                    capture.source_pos = [origin.x, origin.y, origin.z];
                    capture
                        .selected_fx_name
                        .eq_ignore_ascii_case(&shot.fire_fx_name)
                });
        // One accepted discharge owns ammo, tracker and barrel state. Damage
        // callbacks run after that bounded shooter borrow has ended.
        let discharge = self.record_accepted_weapon_discharge(source, slot);
        if visual_owns_fx && discharge.is_some() {
            shot.fire_fx_name.clear();
        }
        self.materialize_owned_passive_shot(shot);
        true
    }
}
