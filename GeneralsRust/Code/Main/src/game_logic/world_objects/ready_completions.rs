//! Receiving Main object and projectile ID allocation, in source admission order.
use super::super::*;
pub(in crate::game_logic) fn allocate_live_object_id(
    ids: &mut generals_sim::object::ObjectIds,
    objects: HostObjectRead<'_>,
    combat: &crate::game_logic::combat::CombatSystem,
) -> ObjectId {
    ids.allocate(|id| objects.lookup(&id).is_some() || combat.get_projectiles().contains_key(&id))
}
impl GameLogic {
    pub(in super::super) fn allocate_object_id(&mut self) -> ObjectId {
        allocate_live_object_id(
            &mut self.object_ids,
            self.objects.read(),
            &self.combat_system,
        )
    }
}
