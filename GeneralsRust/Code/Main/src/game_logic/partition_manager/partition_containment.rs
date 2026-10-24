//! OpenContain's recursive registration over the existing contained-item roster.
//! A parking link is not a Contain module and is handled at its own boundary.
use super::PartitionManager;
use crate::game_logic::{GameLogic, HostObjectStore, ObjectId};
impl GameLogic {
    /// Explicit split fields permit audio/movement loans to remain independent.
    /// Children follow their container's actual list order; no ID sorting or
    /// inverse contained_by scan is used to manufacture a hierarchy.
    pub(crate) fn set_owned_containment_in_parts(
        objects: &mut HostObjectStore,
        partition: &mut PartitionManager,
        id: ObjectId,
        container: Option<ObjectId>,
        enclosing: bool,
    ) {
        let actor = objects
            .get(&id)
            .expect("receiving containment actor remains installed");
        actor.require_partition_owner(Some(partition));
        let was_enclosed = actor.contained_by.is_some() && actor.status.masked;
        if container.is_some() && enclosing {
            change_world_membership(objects, partition, id, false, &mut Vec::new());
        } else if container.is_none() && was_enclosed {
            change_world_membership(objects, partition, id, true, &mut Vec::new());
        }
        objects
            .get_mut(&id)
            .expect("containment membership does not replace actors")
            .finish_containment_reference_with_partition_owner(container, enclosing, partition);
    }
}
fn change_world_membership(
    objects: &mut HostObjectStore,
    partition: &mut PartitionManager,
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
            partition.unregister_object(id.0);
        }
        actor.set_drawable_hidden(!add);
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
            change_world_membership(objects, partition, child_id, add, path);
        }
    }
    assert_eq!(path.pop(), Some(id));
}
