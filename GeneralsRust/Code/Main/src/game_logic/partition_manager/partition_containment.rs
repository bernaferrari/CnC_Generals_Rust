//! OpenContain's recursive registration over the existing contained-item roster.
//! A parking link is not a Contain module and is handled at its own boundary.
use super::PartitionManager;
use crate::game_logic::game_logic::ai_group_owner::AiGroupOwner;
use crate::game_logic::pathfinding::PathfindingSystem;
use crate::game_logic::{GameLogic, HostObjectStore, KindOf, ObjectId};
use std::sync::Arc;
impl GameLogic {
    /// Split OpenContain world membership from onContainedBy/onRemovedFrom so
    /// module callbacks observe the original intermediate state.
    pub(crate) fn change_owned_containment_world_membership(&mut self, id: ObjectId, add: bool) {
        change_world_membership(&mut self.objects, &mut self.partition_manager,
            &mut self.ai_groups, &mut self.pathfinding_system, id, add, &mut Vec::new());
    }

    /// Explicit split fields permit audio/movement loans to remain independent.
    /// Children follow their container's actual list order; no ID sorting or
    /// inverse contained_by scan is used to manufacture a hierarchy.
    pub(crate) fn set_owned_containment_in_parts(
        objects: &mut HostObjectStore,
        partition: &mut PartitionManager,
        ai_groups: &mut AiGroupOwner,
        pathfinding: &mut PathfindingSystem,
        id: ObjectId,
        container: Option<ObjectId>,
        enclosing: bool,
    ) {
        let actor = objects
            .get(&id)
            .expect("receiving containment actor remains installed");
        actor.require_partition_owner(Some(partition));
        let was_enclosed = actor.contained_by.is_some() && actor.status.masked;
        // OpenContain::addToContain ignores an already-contained rider. Avoid
        // replaying the world-removal and leaveGroup phase for the same link.
        let is_new_container = actor.contained_by != container;
        if container.is_some() && enclosing && is_new_container {
            change_world_membership(
                objects,
                partition,
                ai_groups,
                pathfinding,
                id,
                false,
                &mut Vec::new(),
            );
        } else if container.is_none() && was_enclosed && !partition.is_registered(id.0) {
            change_world_membership(
                objects,
                partition,
                ai_groups,
                pathfinding,
                id,
                true,
                &mut Vec::new(),
            );
        }
        let actor = objects
            .get_mut(&id)
            .expect("containment membership does not replace actors");
        actor.finish_containment_reference_with_partition_owner(container, enclosing, partition);
    }
}
fn change_world_membership(
    objects: &mut HostObjectStore,
    partition: &mut PartitionManager,
    ai_groups: &mut AiGroupOwner,
    pathfinding: &mut PathfindingSystem,
    id: ObjectId,
    add: bool,
    path: &mut Vec<ObjectId>,
) {
    assert!(
        !path.contains(&id),
        "runtime containment must remain acyclic"
    );
    path.push(id);
    {
        let actor = objects
            .get_mut(&id)
            .expect("contained roster references an installed Object");
        actor.require_partition_owner(Some(partition));
        if add {
            partition.attach_receiving_object(actor);
        } else {
            // OpenContain::addOrRemoveObjFromWorld leaves each visited actor's
            // group before removing its partition and hiding its drawable.
            ai_groups.remove_if_birth(id, &Arc::downgrade(&actor.visual_draw_identity_token));
            partition.unregister_object(id.0);
        }
        actor.set_drawable_hidden(!add);
        // OpenContain removes the rider from the pathfinder after hiding it.
        // Mobile position and goal cells remain owned by the receiving grid.
        if !add
            && !actor.is_kind_of(KindOf::Mine)
            && !actor.is_kind_of(KindOf::Projectile)
            && !actor.is_kind_of(KindOf::BridgeTower)
            && !actor.is_kind_of(KindOf::BlastCrater)
            && !(actor.get_template().fence_width > 0.0 && !actor.is_kind_of(KindOf::DefensiveWall))
            && !actor.is_kind_of(KindOf::Immobile)
            && actor.has_ai_update_interface()
        {
            let radius = actor.geometry_info().bounding_circle_radius();
            if pathfinding.map_ready {
                pathfinding.grid.clear_path_position_square(
                    id.0,
                    radius,
                    actor.pathfind_cur_cell,
                    actor.pathfind_layer,
                );
                actor.pathfind_cur_cell = (-1, -1);
            }
            pathfinding.grid.clear_path_goal_square(
                id.0,
                radius,
                actor.pathfind_goal_cell,
                actor.pathfind_goal_layer,
            );
            actor.pathfind_goal_cell = (-1, -1);
        }
    }
    let children = objects
        .get(&id)
        .expect("installed container")
        .contained_units();
    for child_id in children {
        // Re-read this birth and source classifier at its original visit.
        let actor = objects
            .get(&id)
            .expect("installed parent during contained-item walk");
        let child = objects
            .get(&child_id)
            .expect("contained item remains installed until physical retirement");
        if !actor.is_enclosing_container_for(child) {
            change_world_membership(
                objects,
                partition,
                ai_groups,
                pathfinding,
                child_id,
                add,
                path,
            );
        }
    }
    assert_eq!(path.pop(), Some(id));
}
