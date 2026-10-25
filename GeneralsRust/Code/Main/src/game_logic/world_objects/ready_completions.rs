//! Receiving Main object and projectile ID allocation, in source admission order.
use super::super::*;
pub(in crate::game_logic) fn allocate_live_object_id(
    next: &mut ObjectId,
    objects: HostObjectRead<'_>,
    combat: &crate::game_logic::combat::CombatSystem,
) -> ObjectId {
    loop {
        let id = *next;
        next.0 = next
            .0
            .checked_add(1)
            .expect("world ObjectID space exhausted");
        // Restored older saves retain their IDs. Skip live saved carriers rather
        // than renumbering references or reserving an arbitrary numeric range.
        if id.0 != 0 && objects.lookup(&id).is_none() && !combat.get_projectiles().contains_key(&id)
        {
            return id;
        }
    }
}
impl GameLogic {
    pub(in super::super) fn allocate_object_id(&mut self) -> ObjectId {
        allocate_live_object_id(
            &mut self.next_object_id,
            self.objects.read(),
            &self.combat_system,
        )
    }
}
