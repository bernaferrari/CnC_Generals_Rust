//! Live-host CollideModule::onCollide residual (C++ Object::onCollide).

use crate::game_logic::{AIState, ObjectId};

impl super::super::GameLogic {
    /// C++ `Object::onCollide` — walk contain-enter, crate pickup, and
    /// `FireWeaponCollide::onCollide` (`FireWeaponCollide.cpp:52-67`).
    pub(in super::super) fn dispatch_host_collide_modules(
        &mut self,
        self_id: ObjectId,
        other_id: ObjectId,
    ) {
        if self_id == other_id {
            return;
        }
        let (entering_other, crate_other) = {
            let Some(us) = self.objects.get(&self_id) else {
                return;
            };
            let Some(other) = self.objects.get(&other_id) else {
                return;
            };
            let entering = matches!(us.ai_state, AIState::Entering)
                && (us.target == Some(other_id) || us.container_id() == Some(other_id))
                && other.can_contain();
            let crate_name = other.template_name.to_ascii_lowercase();
            let is_crate = crate_name.contains("crate");
            (entering, is_crate)
        };
        if entering_other {
            let boarded = self
                .objects
                .get_mut(&other_id)
                .is_some_and(|c| c.add_occupant(self_id));
            if boarded {
                if let Some(us) = self.objects.get_mut(&self_id) {
                    us.set_contained_by(Some(other_id));
                    us.ai_state = AIState::Garrisoned;
                    us.status.moving = false;
                }
            }
        }
        if crate_other {
            self.update_money_crate_collides();
        }
        self.fire_host_weapon_collide(self_id, other_id);
    }

    /// C++ `FireWeaponCollide::onCollide` — skip ground (other is always an
    /// object here), then `loadAmmoNow` + `fireWeapon` every frame using the
    /// INI `CollideWeapon` template (damage type / amount), not invented Flame.
    fn fire_host_weapon_collide(&mut self, self_id: ObjectId, other_id: ObjectId) {
        use super::collide_dispatch::{
            host_fire_weapon_collide_damage, host_fire_weapon_collide_spec,
            host_should_fire_weapon_collide,
        };
        let Some(us) = self.objects.get(&self_id) else {
            return;
        };
        if !us.is_alive() {
            return;
        }
        let Some(spec) = host_fire_weapon_collide_spec(us) else {
            return;
        };
        // C++ never sets m_everFired — match, not a gap.
        if !host_should_fire_weapon_collide(us.object_status_bits, &spec, false) {
            return;
        }
        let damage = host_fire_weapon_collide_damage(&spec.weapon_name);
        if damage <= 0.0 {
            return;
        }
        let damage_type = crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
            &spec.weapon_name,
        );
        let death_type = crate::game_logic::host_armor_residual::resolve_host_death_type(
            Some(&spec.weapon_name),
            damage_type,
        );
        if let Some(other) = self.objects.get_mut(&other_id) {
            if !other.is_alive() {
                return;
            }
            let _ =
                other.take_damage_from_typed_death(damage, Some(self_id), damage_type, death_type);
        }
    }
}
