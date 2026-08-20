//! Live-host CollideModule::onCollide residual (C++ Object::onCollide).

use crate::game_logic::{AIState, ObjectId};

impl super::super::GameLogic {
    /// C++ `Object::onCollide` — walk contain-enter and crate pickup on contact.
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
    }
}
