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
        self.fire_owned_passive_weapon_with_effects(
            source,
            target,
            slot,
            origin,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn fire_owned_passive_weapon_with_effects(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        slot: u8,
        origin: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(victim) = self.objects.get(&target).filter(|o| o.is_alive()) else {
            return false;
        };
        let infantry = victim.is_kind_of(KindOf::Infantry);
        let faerie = victim.is_faerie_fire();
        let projectile_team_id = self
            .objects
            .get(&source)
            .and_then(|object| object.owner_player_id)
            .and_then(|player_id| self.host_default_team_id(player_id));
        let Some(shooter) = self.objects.get_mut(&source) else {
            return false;
        };
        let Some((slot, shot)) = shooter.prepare_passive_weapon_shot_with_operation_and_solo_set(
            slot,
            target,
            self.frame as f32 * LOGIC_FRAME_TIMESTEP,
            infantry,
            faerie,
            self.frame,
            origin,
            &mut self.combat_system.fire_sound_loop_events,
            &mut self.attack_events,
            self.weapon_definitions
                .as_ref()
                .map(|_| &self.projectile_definitions),
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
            &self.solo_weapon_bonus_set,
            &self.logic_random,
            projectile_team_id,
        ) else {
            return false;
        };
        let capture = shooter.take_pending_weapon_visual_capture();
        self.deliver_owned_weapon_shot_with_effects(slot, shot, capture, presentation_effects);
        true
    }
}
