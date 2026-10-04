//! Queries that already borrow the driving Object and its actual AI interface.

use super::Object;
use crate::common::{BodyDamageType, Real};
use crate::modules::AIUpdateInterface;

impl Object {
    /// AIUpdate.cpp:774-780 uses this owner's body and this AI's current member.
    /// The caller already holds its cached AI; do not rediscover a Unit or AI.
    pub(crate) fn cur_locomotor_speed_with_ai(&self, ai: &dyn AIUpdateInterface) -> Real {
        let mut speed = 0.0;
        ai.with_cur_locomotor(&mut |locomotor| {
            // C++ only queries the body when a current locomotor exists.
            // Borrow the already-owned cached interface instead of cloning it.
            let condition = self
                .body
                .as_ref()
                .and_then(|body| body.lock().ok().map(|body| body.get_damage_state()))
                .unwrap_or(BodyDamageType::Pristine);
            let condition = match condition {
                BodyDamageType::Pristine => crate::locomotor::BodyDamageType::Pristine,
                BodyDamageType::Damaged => crate::locomotor::BodyDamageType::Damaged,
                BodyDamageType::ReallyDamaged => crate::locomotor::BodyDamageType::ReallyDamaged,
                BodyDamageType::Rubble => crate::locomotor::BodyDamageType::Rubble,
            };
            // The body guard ends before evaluating the locomotor speed.
            speed = locomotor.get_max_speed_for_condition(condition);
        });
        speed
    }
}
