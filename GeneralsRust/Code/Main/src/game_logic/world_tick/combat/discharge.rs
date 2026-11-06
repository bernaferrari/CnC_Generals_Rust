//! Successful-shot commit for a slot with no admitted WeaponTemplate.
//!
//! Authored weapons commit inside the shared shot operation
//! (`authored_shot.rs`, C++ Weapon.cpp:2617-2669 and FiringTracker.cpp:60-160).
//! A bare host `Weapon` has no FireFX/FireSound/LaserName (C++ WeaponTemplate
//! constructor defaults), so only the generic muzzle presentation for nameless
//! probes, ammo/clip, FiringTracker, stealth break and barrel remain.
use super::*;

impl GameLogic {
    pub(super) fn commit_combat_shot(
        &mut self,
        input: CombatAttackInput,
        slot: u8,
        visual: Option<super::super::super::world_combat::AcceptedWeaponDischarge>,
    ) {
        self.commit_combat_shot_with_effects(input, slot, visual, &mut Default::default())
    }

    pub(super) fn commit_combat_shot_with_effects(
        &mut self,
        input: CombatAttackInput,
        slot: u8,
        visual: Option<super::super::super::world_combat::AcceptedWeaponDischarge>,
        _effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let CombatAttackInput {
            attacker_id,
            current_time,
            target_id,
            target_location,
            ..
        } = input;
        let target_has_faerie_fire = target_id
            .and_then(|id| self.objects.get(&id))
            .is_some_and(|target| target.is_faerie_fire());
        let possible_next_shot_frame =
            self.objects
                .get(&attacker_id)
                .map_or(self.frame, |attacker| {
                    let rates = self.receiving_weapon_rates_of_fire(attacker);
                    attacker.possible_next_shot_frame_for_slot_with_owned_rof(
                        slot,
                        self.frame,
                        target_has_faerie_fire,
                        &rates,
                    )
                });
        let fire_frame = self.frame;
        // Nameless probes outside any admitted content still show a primitive
        // muzzle so the shot is observable; admitted content never does.
        if self.weapon_definitions.is_none()
            && !visual
                .as_ref()
                .is_some_and(|shot| shot.has_authored_fire_fx)
        {
            let suppress_fire_fx = self.objects.get(&attacker_id).is_some_and(|o| {
                let locally_controlled =
                    source_is_locally_controlled(o.owner_player_id, self.local_player_id());
                !locally_controlled
                    && o.status.stealthed
                    && !o.status.detected
                    && !o.status.disguised
                    && !o.is_kind_of(KindOf::Mine)
            });
            let muzzle_pos = self
                .objects
                .get(&attacker_id)
                .map_or(Vec3::ZERO, Object::get_position);
            let fire_target = target_id.filter(|id| self.objects.contains_key(id));
            let impact_pos = fire_target
                .and_then(|id| self.objects.get(&id).map(Object::get_position))
                .or(target_location);
            self.spawn_residual_muzzle_when_dispatch_has_no_fire_fx(
                suppress_fire_fx,
                "",
                "",
                muzzle_pos,
                impact_pos,
                fire_frame,
                attacker_id,
                fire_target,
            );
        }
        let rates = self
            .objects
            .get(&attacker_id)
            .map(|attacker| self.receiving_weapon_rates_of_fire(attacker));
        if let (Some(attacker), Some(rates)) = (self.objects.get_mut(&attacker_id), rates) {
            let auto_reloaded_clip = if let Some(weapon) = attacker.weapon_slot_mut(slot) {
                Object::consume_ammo_on_fire_named(weapon, current_time, None);
                Object::auto_reloaded_clip_after_firing(weapon, None)
            } else {
                false
            };
            // Object::fireCurrentWeapon: only the locked slot finishing an
            // auto-reloading clip ends a temporary lock.
            if auto_reloaded_clip
                && attacker.weapon_lock_type == WeaponLockType::LockedTemporarily
                && attacker.weapon_lock_slot == slot
            {
                attacker.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }
            attacker.record_shot_at_target_with_owned_rof(
                target_id,
                fire_frame,
                possible_next_shot_frame,
                &rates,
            );
            attacker.stamp_auto_reload_when_idle_from_slot(slot, fire_frame);
        }
        if let Some(visual) = visual {
            self.finish_accepted_weapon_discharge(visual);
        } else if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            attacker.advance_weapon_barrel_after_shot(slot);
        }
    }
}
