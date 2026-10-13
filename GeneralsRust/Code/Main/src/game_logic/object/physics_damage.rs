//! Immediate Physics Body effects with services borrowed from the receiving owner.
use super::*;

pub(in crate::game_logic) struct PhysicsDamageServices<'a> {
    pub frame: u32,
    pub health_events: &'a mut crate::game_logic::HostHealthEvents,
    pub repulsors: &'a bool,
    pub attacked_by: &'a mut dyn FnMut(Option<u32>, Option<ObjectId>),
}
impl PhysicsDamageServices<'_> {
    pub(super) fn apply(
        &mut self,
        victim: &mut Object,
        amount: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        let context = DamageHitContext::default();
        let application = victim.begin_damage_with_context_and_attacked_by(
            amount,
            source,
            damage_type,
            death_type,
            None,
            self.frame,
            &context,
            self.health_events,
            None, // Physics supplies only Crush/Falling/Unresistable; no ownership transfer.
            &mut *self.attacked_by,
        );
        application.finish_standalone(victim, None, self.repulsors)
    }
}
