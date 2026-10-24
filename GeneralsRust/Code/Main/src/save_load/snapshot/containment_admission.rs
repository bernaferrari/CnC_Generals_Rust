//! Validate the saved explicit contained-item graph before any receiving retirement.
use super::ObjectSnapshot;
use crate::game_logic::ObjectId;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::collections::{HashMap, HashSet};
pub(super) fn validate(objects: &HashMap<ObjectId, ObjectSnapshot>) -> SaveLoadResult<()> {
    // Match restore_object_references: an older payload may carry an inverse
    // container reference not yet present in its container's explicit list.
    // This temporary validation graph is not a runtime hierarchy or index.
    let mut lists: HashMap<_, _> = objects
        .iter()
        .map(|(id, record)| (*id, record.contained_objects.clone()))
        .collect();
    for (id, record) in objects {
        if let Some(container) = record.container_object {
            if let Some(items) = lists.get_mut(&container) {
                if !items.contains(id) {
                    items.push(*id);
                }
            }
        }
    }
    let mut done = HashSet::new();
    let mut active = HashSet::new();
    for root in objects.keys().copied() {
        if done.contains(&root) {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        active.insert(root);
        while let Some((id, next)) = stack.last_mut() {
            let items = &lists[id];
            if *next == items.len() {
                let id = *id;
                stack.pop();
                active.remove(&id);
                done.insert(id);
                continue;
            }
            let child = items[*next];
            *next += 1;
            if !objects.contains_key(&child) {
                return Err(SaveLoadError::Corrupted(format!(
                    "contained item {child} has no saved Object"
                )));
            }
            if active.contains(&child) {
                return Err(SaveLoadError::Corrupted(
                    "saved contained-item roster contains a cycle".into(),
                ));
            }
            if !done.contains(&child) {
                active.insert(child);
                stack.push((child, 0));
            }
        }
    }
    Ok(())
}
