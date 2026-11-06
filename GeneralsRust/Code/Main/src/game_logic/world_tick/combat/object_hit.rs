//! Object-target hit for a slot with no admitted WeaponTemplate.
//!
//! Every authored weapon fires through `authored_shot.rs` (C++
//! `WeaponTemplate::fireWeaponTemplate`). This fallback only serves bare
//! `Weapon` slots that no receiving definition describes (hand-built probes):
//! it deals the slot's damage with the C++ `WeaponTemplate` constructor
//! defaults (`DAMAGE_EXPLOSION`, `DEATH_NORMAL`, Weapon.cpp:249) and its splash
//! radius. No unit, template or weapon name selects behavior here.
use super::*;

impl GameLogic {
    pub(super) fn apply_combat_object_hit(
        &mut self,
        attacker_id: ObjectId,
        attacker_team: Team,
        target_id: ObjectId,
        target_position: Vec3,
        slot: u8,
        enemy_or_forced: bool,
        weapon_damage: f32,
    ) {
        self.apply_combat_object_hit_with_effects(
            attacker_id,
            attacker_team,
            target_id,
            target_position,
            slot,
            enemy_or_forced,
            weapon_damage,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_combat_object_hit_with_effects(
        &mut self,
        attacker_id: ObjectId,
        attacker_team: Team,
        target_id: ObjectId,
        target_position: Vec3,
        slot: u8,
        _enemy_or_forced: bool,
        weapon_damage: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let damage_type = crate::game_logic::combat::DamageType::Explosive;
        let death_type = crate::game_logic::host_usa_pilot::HostDeathType::Normal;
        let context = crate::game_logic::object::DamageHitContext::new_selected(
            self.objects.get(&attacker_id),
            None,
            damage_type,
            None,
        );
        let admitted = self
            .objects
            .get(&target_id)
            .is_some_and(|target| target.get_sneaky_targeting_offset(self.frame).is_none());
        if admitted {
            if let Some(result) = self.apply_owned_damage_with_effects(
                target_id,
                weapon_damage,
                Some(attacker_id),
                damage_type,
                death_type,
                None,
                &context,
                effects,
            ) {
                if result.destroyed {
                    self.continue_or_stop_after_kill(
                        attacker_id,
                        target_id,
                        result.victim_position,
                        result.victim_team,
                        None,
                    );
                }
            }
        }
        let splash = self
            .objects
            .get(&attacker_id)
            .and_then(|attacker| attacker.weapon_slot(slot))
            .map_or(0.0, |weapon| weapon.splash_radius.max(0.0));
        if splash > 0.0 {
            let _ = self.apply_instant_hit_splash_at_with_effects(
                target_position,
                weapon_damage,
                0.0,
                splash,
                0.0,
                attacker_id,
                attacker_team,
                target_id,
                None,
                effects,
            );
        }
    }
}
