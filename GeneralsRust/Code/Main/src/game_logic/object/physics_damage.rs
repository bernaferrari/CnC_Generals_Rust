//! Immediate Physics Body effects with services borrowed from the receiving owner.
use super::*;

/// Read-only source observations captured while both objects remain in the receiving map.
#[derive(Default)]
pub(in crate::game_logic) struct PhysicsDamageSources {
    pub players: [Option<(ObjectId, u32)>; 2],
    pub observations: [Option<(ObjectId, LastDamageSourceObservation)>; 2],
}
impl PhysicsDamageSources {
    fn observe(&self, id: Option<ObjectId>) -> LastDamageSourceObservation {
        id.and_then(|id| {
            self.observations
                .iter()
                .flatten()
                .find(|entry| entry.0 == id)
        })
        .map(|entry| entry.1)
        .unwrap_or_default()
    }
}

pub(in crate::game_logic) struct PhysicsDamageServices<'a> {
    pub frame: u32,
    pub sources: &'a PhysicsDamageSources,
    pub health_events: &'a mut crate::game_logic::HostHealthEvents,
    pub repulsors: &'a bool,
    pub player_effects: &'a mut dyn FnMut(BodyPlayerEffect),
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
        let context = DamageHitContext::default().with_last_damage_observations(
            self.sources.observe(source),
            self.sources.observe(victim.last_damage_source),
        );
        let application = victim.begin_damage_with_context_and_player_effects(
            amount,
            source,
            damage_type,
            death_type,
            None,
            self.frame,
            &context,
            self.health_events,
            None, // Physics supplies only Crush/Falling/Unresistable; no ownership transfer.
            &mut *self.player_effects,
        );
        application.finish_standalone(victim, None, self.repulsors)
    }
}
