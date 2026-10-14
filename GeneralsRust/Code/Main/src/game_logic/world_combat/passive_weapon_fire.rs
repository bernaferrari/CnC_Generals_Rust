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
        let capture = shooter.take_pending_weapon_visual_capture();
        let accepted =
            self.begin_owned_weapon_effects(source, slot, Some(target), None, origin, capture);
        // This driving operation has delivered the effects exactly once,
        // independently of projectile lifetime or source survival.
        shot.fire_fx_name.clear();
        shot.fire_ocl_name.clear();
        self.materialize_owned_passive_shot(shot);
        if let Some(accepted) = accepted {
            self.finish_accepted_weapon_discharge(accepted);
        }
        true
    }
}
