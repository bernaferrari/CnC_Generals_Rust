//! CPP InGameUI.cpp3173/3203: local selection changes stamp their mutation frame.
use super::super::*;

impl GameLogic {
    /// Called by the local UI producer, after selecting its drawable. Enemy
    /// objects may be selected too; controlling-player identity is not a gate.
    pub(crate) fn admit_local_drawable_selected(&mut self, id: ObjectId) {
        if !self.objects.get(&id).is_some_and(|object| object.selected)
            || self.selected_objects.contains(&id)
        {
            return;
        }
        // Original selected drawable list uses push_front.
        self.selected_objects.insert(0, id);
        self.selection_changed_frame = self.frame;
    }

    /// Removal does not require the object still to exist (onDelete boundary).
    pub(crate) fn admit_local_drawable_deselected(&mut self, id: ObjectId) {
        if self.selected_objects.contains(&id) {
            self.selected_objects.retain(|selected| *selected != id);
            self.selection_changed_frame = self.frame;
        }
    }

    /// Object-local hidden/status/contain effects cannot borrow their world.
    /// Their driving operation admits the actual deselect before continuing.
    pub(crate) fn admit_object_drawable_deselection(&mut self, id: ObjectId) {
        if self.objects.get(&id).is_none_or(|object| !object.selected) {
            self.deselect_drawable(id);
        }
    }

    pub(crate) fn apply_player_drawable_selection(
        &mut self,
        player_id: u32,
        ids: &[ObjectId],
        replace: bool,
    ) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let local = self.is_local_player(player_id);
        let previous = if replace {
            player.selected_objects.clone()
        } else {
            Vec::new()
        };
        for id in previous {
            if let Some(object) = self.objects.get_mut(&id) {
                object.deselect();
            }
            if local {
                self.admit_local_drawable_deselected(id);
            }
        }
        if replace {
            self.players
                .get_mut(&player_id)
                .unwrap()
                .selected_objects
                .clear();
        }
        for &id in ids {
            if let Some(object) = self.objects.get_mut(&id) {
                object.select();
            } else {
                continue;
            }
            if local {
                self.admit_local_drawable_selected(id);
            }
            let selected = &mut self.players.get_mut(&player_id).unwrap().selected_objects;
            if !selected.contains(&id) {
                selected.push(id);
            }
        }
    }

    pub(crate) fn clear_player_drawable_selection(&mut self, player_id: u32) {
        let ids = self
            .players
            .get(&player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        for id in ids {
            self.deselect_drawable(id);
        }
    }

    pub(crate) fn is_local_selection_player(&self, id: u32) -> bool {
        self.is_local_player(id)
    }
}
