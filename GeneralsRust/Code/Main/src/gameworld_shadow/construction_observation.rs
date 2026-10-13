//! Read-only construction/rebuild disagreement checks. Only Main mutates them.
use super::*;
impl GameWorldShadow {
    pub fn compare_rebuild_producer_to_host(&self, logic: &GameLogic) -> usize {
        self.host_to_entity
            .iter()
            .filter(|(hid, eid)| {
                let (Some(ent), Some(obj)) =
                    (self.world.entity(**eid), logic.host_object(ObjectId(**hid)))
                else {
                    return false;
                };
                obj.is_rebuild_hole != ent.is_rebuild_hole
                    || obj.rebuild_template_name.as_deref().unwrap_or("")
                        != ent.rebuild_template_name
                    || obj.rebuild_ready_frame != ent.rebuild_ready_frame
                    || obj.rebuild_spawner_id.map(|id| id.0) != ent.rebuild_spawner_id
                    || obj.rebuild_worker_id.map(|id| id.0) != ent.rebuild_worker_id
                    || obj.rebuild_reconstructing_id.map(|id| id.0) != ent.rebuild_reconstructing_id
                    || obj.producer_id.map(|id| id.0) != ent.producer_id
                    || obj.construction_complete_clear_frame
                        != ent.construction_complete_clear_frame
            })
            .count()
    }
    pub fn compare_construction_to_host(&self, logic: &GameLogic) -> usize {
        self.host_to_entity
            .iter()
            .filter(|(hid, eid)| {
                let (Some(ent), Some(obj)) =
                    (self.world.entity(**eid), logic.host_object(ObjectId(**hid)))
                else {
                    return false;
                };
                (obj.construction_percent - ent.construction_percent).abs() > 1e-5
                    || obj.status.under_construction != ent.under_construction
                    || obj.status.sold != ent.sold
                    || obj.status.reconstructing != ent.reconstructing
                    || obj.status.unselectable != ent.unselectable
            })
            .count()
    }
}
