//! # Selection System
//!
//! Manages unit selection state for the in-game UI.  Handles box selection
//! (drag rectangle), click selection, multi-select with modifier keys, and
//! control group assignment/recall.  This module works alongside
//! [`input_bridge`] but focuses specifically on the *selection* side of
//! the input pipeline.
//!
//! ## C++ Reference
//!
//! In the C++ codebase, selection lives primarily in:
//! - `InGameUI::selectThing()` / `InGameUI::deselectAll()`
//! - `GameClient::becomeSelectedGroup()` / `GameClient::clearSelectedGroup()`
//! - `InGameUI::processInput()` which maps area-selection regions to
//!   `GameMessage::MSG_AREA_SELECTION`.
//!
//! The C++ selection system also maintains a "selected group" object that
//! is a container of `Drawable` pointers.  Here we use plain `ObjectID`
//! values since the Rust codebase is object-ID-centric.
//!
//! ## Interaction with `input_bridge`
//!
//! The `GameInputHandler` calls into the selection system when processing
//! area-selection (box-drag) results, click-on-object events, and control
//! group hotkeys.  The selection system can also be queried independently
//! by the UI/rendering layer to draw selection circles, health bars, etc.

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// SelectionType -- how a new selection replaces the existing one
// ---------------------------------------------------------------------------

/// How a new selection interacts with the current selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionType {
    /// Replace the entire selection with the new set.
    Replace,
    /// Add to the current selection (union).
    Add,
    /// Remove from the current selection (difference).
    Remove,
    /// Toggle: add if not present, remove if already selected.
    Toggle,
}

// ---------------------------------------------------------------------------
// SelectionState -- the mutable selection for one player
// ---------------------------------------------------------------------------

/// Per-player selection state.
#[derive(Debug, Clone)]
pub struct SelectionState {
    /// Currently selected object IDs (order matters for "primary" selection).
    objects: Vec<u32>,

    /// Fast membership check.
    set: HashSet<u32>,

    /// Control groups (index 0-9).  Each stores a snapshot of object IDs at
    /// the time the group was created.
    control_groups: [Vec<u32>; 10],
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionState {
    /// Create an empty selection state.
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            set: HashSet::new(),
            control_groups: Default::default(),
        }
    }

    /// Return the current selection as a slice.
    pub fn selected(&self) -> &[u32] {
        &self.objects
    }

    /// Check if an object is currently selected.
    pub fn contains(&self, id: u32) -> bool {
        self.set.contains(&id)
    }

    /// Number of selected objects.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether the selection is empty.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.objects.clear();
        self.set.clear();
    }

    /// Apply a selection change.
    pub fn apply(&mut self, new_ids: Vec<u32>, selection_type: SelectionType) {
        match selection_type {
            SelectionType::Replace => {
                self.clear();
                self.extend(new_ids);
            }
            SelectionType::Add => {
                self.extend(new_ids);
            }
            SelectionType::Remove => {
                for id in &new_ids {
                    self.set.remove(id);
                }
                self.objects.retain(|id| self.set.contains(id));
            }
            SelectionType::Toggle => {
                for id in &new_ids {
                    if self.set.contains(id) {
                        self.set.remove(id);
                    } else {
                        self.set.insert(*id);
                    }
                }
                // Rebuild ordered list from set, preserving original order for
                // still-selected objects, then appending newly-added ones.
                let mut ordered: Vec<u32> = self
                    .objects
                    .iter()
                    .filter(|id| self.set.contains(id))
                    .copied()
                    .collect();
                for id in &new_ids {
                    if self.set.contains(id) && !ordered.contains(id) {
                        ordered.push(*id);
                    }
                }
                self.objects = ordered;
            }
        }
    }

    /// Convenience: select a single object (replace).
    pub fn select_single(&mut self, id: u32) {
        self.apply(vec![id], SelectionType::Replace);
    }

    /// Convenience: add a single object to the selection.
    pub fn add_single(&mut self, id: u32) {
        if !self.set.contains(&id) {
            self.set.insert(id);
            self.objects.push(id);
        }
    }

    /// Convenience: remove a single object from the selection.
    pub fn remove_single(&mut self, id: u32) {
        self.apply(vec![id], SelectionType::Remove);
    }

    /// Extend the selection, deduplicating.
    fn extend(&mut self, ids: Vec<u32>) {
        for id in ids {
            if self.set.insert(id) {
                self.objects.push(id);
            }
        }
    }

    // ----- Control groups ----------------------------------------------------

    /// Store the current selection into a control group (0-9).
    pub fn create_control_group(&mut self, index: usize) {
        if index < 10 {
            self.control_groups[index] = self.objects.clone();
        }
    }

    /// Recall a control group, returning the stored object IDs.
    pub fn recall_control_group(&self, index: usize) -> Option<&[u32]> {
        if index < 10 && !self.control_groups[index].is_empty() {
            Some(&self.control_groups[index])
        } else {
            None
        }
    }

    /// Add the current selection to an existing control group.
    pub fn add_to_control_group(&mut self, index: usize) {
        if index >= 10 {
            return;
        }
        let group = &mut self.control_groups[index];
        for &id in &self.objects {
            if !group.contains(&id) {
                group.push(id);
            }
        }
    }

    /// Remove a control group assignment.
    pub fn remove_control_group(&mut self, index: usize) {
        if index < 10 {
            self.control_groups[index].clear();
        }
    }

    /// Get a control group's contents.
    pub fn get_control_group(&self, index: usize) -> &[u32] {
        if index < 10 {
            &self.control_groups[index]
        } else {
            &[]
        }
    }
}

// ---------------------------------------------------------------------------
// DragBox -- tracks the state of a box-selection drag
// ---------------------------------------------------------------------------

/// Represents an in-progress or completed drag-selection rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragBox {
    /// Anchor corner (mouse-down position).
    pub start_x: i32,
    pub start_y: i32,
    /// Current / release corner.
    pub end_x: i32,
    pub end_y: i32,
    /// Whether the drag has exceeded the minimum threshold.
    pub active: bool,
}

impl DragBox {
    /// Create an inactive drag box.
    pub fn new() -> Self {
        Self {
            start_x: 0,
            start_y: 0,
            end_x: 0,
            end_y: 0,
            active: false,
        }
    }

    /// Begin a new drag at the given position.
    pub fn begin(&mut self, x: i32, y: i32) {
        self.start_x = x;
        self.start_y = y;
        self.end_x = x;
        self.end_y = y;
        self.active = false;
    }

    /// Update the current corner while dragging.
    pub fn update(&mut self, x: i32, y: i32, tolerance: i32) {
        self.end_x = x;
        self.end_y = y;
        if !self.active {
            let dx = (self.end_x - self.start_x).abs();
            let dy = (self.end_y - self.start_y).abs();
            if dx > tolerance || dy > tolerance {
                self.active = true;
            }
        }
    }

    /// Finalize the drag.  Returns `Some(region)` if the drag was active,
    /// `None` if it was just a click.
    pub fn finish(&mut self) -> Option<(i32, i32, i32, i32)> {
        self.active = false;
        let left = self.start_x.min(self.end_x);
        let top = self.start_y.min(self.end_y);
        let right = self.start_x.max(self.end_x);
        let bottom = self.start_y.max(self.end_y);
        if right - left > 0 || bottom - top > 0 {
            Some((left, top, right, bottom))
        } else {
            None
        }
    }

    /// Get the normalized bounding rectangle (left, top, right, bottom).
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        (
            self.start_x.min(self.end_x),
            self.start_y.min(self.end_y),
            self.start_x.max(self.end_x),
            self.start_y.max(self.end_y),
        )
    }
}

impl Default for DragBox {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ObjectPicker trait -- abstracts the "what objects are in this rectangle"
// query so the selection system can be tested without a real game world.
// ---------------------------------------------------------------------------

/// Implemented by the game world (or a test double) to resolve which objects
/// fall inside a screen-space rectangle or at a point.
pub trait ObjectPicker {
    /// Return all player-owned, selectable object IDs whose screen position
    /// falls inside the given pixel rectangle (left, top, right, bottom).
    fn pick_in_rect(&self, player: i32, left: i32, top: i32, right: i32, bottom: i32) -> Vec<u32>;

    /// Return the player-owned object at the given pixel, or `None`.
    fn pick_at_point(&self, player: i32, x: i32, y: i32) -> Option<u32>;

    /// Return all player-owned objects that share the same "kind" (template
    /// name / type) as the given object.
    fn pick_all_of_same_kind(&self, player: i32, object_id: u32) -> Vec<u32>;
}

// ---------------------------------------------------------------------------
// SelectionManager -- high-level selection operations
// ---------------------------------------------------------------------------

/// High-level selection manager that combines `SelectionState` with a
/// `DragBox` and an `ObjectPicker` to provide the full selection API used
/// by the input bridge and the UI layer.
pub struct SelectionManager {
    /// Per-player selection state.
    states: Vec<SelectionState>,

    /// Per-player drag box state.
    drag_boxes: Vec<DragBox>,

    /// Object picker (set once at init).
    picker: Box<dyn ObjectPicker>,

    /// Maximum number of objects that can be selected at once.
    pub max_selection_size: usize,
}

impl std::fmt::Debug for SelectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectionManager")
            .field("states", &self.states.len())
            .field("max_selection_size", &self.max_selection_size)
            .finish()
    }
}

impl SelectionManager {
    /// Create a new selection manager for `max_players`.
    pub fn new(max_players: usize, picker: Box<dyn ObjectPicker>) -> Self {
        let mut states = Vec::with_capacity(max_players);
        let mut drag_boxes = Vec::with_capacity(max_players);
        for _ in 0..max_players {
            states.push(SelectionState::new());
            drag_boxes.push(DragBox::new());
        }
        Self {
            states,
            drag_boxes,
            picker,
            max_selection_size: 64,
        }
    }

    /// Access the selection state for a player.
    pub fn state(&self, player: i32) -> Option<&SelectionState> {
        self.states.get(player as usize)
    }

    /// Access the selection state for a player (mutable).
    pub fn state_mut(&mut self, player: i32) -> Option<&mut SelectionState> {
        self.states.get_mut(player as usize)
    }

    /// Access the drag box for a player.
    pub fn drag_box(&self, player: i32) -> Option<&DragBox> {
        self.drag_boxes.get(player as usize)
    }

    /// Access the drag box for a player (mutable).
    pub fn drag_box_mut(&mut self, player: i32) -> Option<&mut DragBox> {
        self.drag_boxes.get_mut(player as usize)
    }

    // ----- High-level operations -------------------------------------------

    /// Begin a drag (left mouse down).  Call this before `update_drag`.
    pub fn begin_drag(&mut self, player: i32, x: i32, y: i32) {
        if let Some(box_) = self.drag_box_mut(player) {
            box_.begin(x, y);
        }
    }

    /// Update the drag position.  Returns `true` once the drag threshold is
    /// exceeded.
    pub fn update_drag(&mut self, player: i32, x: i32, y: i32, tolerance: i32) -> bool {
        if let Some(box_) = self.drag_box_mut(player) {
            let was_active = box_.active;
            box_.update(x, y, tolerance);
            box_.active
        } else {
            false
        }
    }

    /// Finish the drag.  If the drag produced a rectangle, perform a box
    /// selection.  Returns the number of newly selected objects.
    pub fn finish_drag(
        &mut self,
        player: i32,
        modifiers_ctrl: bool,
        modifiers_shift: bool,
    ) -> usize {
        let region = match self.drag_box_mut(player) {
            Some(box_) => box_.finish(),
            None => return 0,
        };

        if let Some((left, top, right, bottom)) = region {
            let picked = self.picker.pick_in_rect(player, left, top, right, bottom);
            let sel_type = if modifiers_ctrl {
                SelectionType::Toggle
            } else if modifiers_shift {
                SelectionType::Add
            } else {
                SelectionType::Replace
            };
            let trimmed = self.trim_selection(&picked);
            let count = trimmed.len();
            if let Some(state) = self.state_mut(player) {
                state.apply(trimmed, sel_type);
            }
            count
        } else {
            0
        }
    }

    /// Click on a point.  If an object is under the cursor, select it;
    /// otherwise clear the selection.  Respects modifier keys.
    pub fn click_select(&mut self, player: i32, x: i32, y: i32, ctrl: bool, shift: bool) {
        let clicked = self.picker.pick_at_point(player, x, y);
        if let Some(state) = self.state_mut(player) {
            if let Some(id) = clicked {
                let sel_type = if ctrl {
                    SelectionType::Toggle
                } else if shift {
                    SelectionType::Add
                } else {
                    SelectionType::Replace
                };
                state.apply(vec![id], sel_type);
            } else if !shift {
                state.clear();
            }
        }
    }

    /// Double-click: select all units of the same kind as the clicked unit.
    pub fn double_click_select(&mut self, player: i32, x: i32, y: i32) {
        if let Some(id) = self.picker.pick_at_point(player, x, y) {
            let all_kind = self.picker.pick_all_of_same_kind(player, id);
            let trimmed = self.trim_selection(&all_kind);
            if let Some(state) = self.state_mut(player) {
                state.apply(trimmed, SelectionType::Replace);
            }
        }
    }

    /// Create a control group (Ctrl+number).
    pub fn create_group(&mut self, player: i32, index: usize) {
        if let Some(state) = self.state_mut(player) {
            state.create_control_group(index);
        }
    }

    /// Recall a control group (number alone).
    pub fn recall_group(&mut self, player: i32, index: usize) -> bool {
        let group_ids = {
            let state = match self.state(player) {
                Some(s) => s,
                None => return false,
            };
            match state.recall_control_group(index) {
                Some(ids) => ids.to_vec(),
                None => return false,
            }
        };
        if let Some(state) = self.state_mut(player) {
            state.apply(group_ids, SelectionType::Replace);
            true
        } else {
            false
        }
    }

    /// Add current selection to an existing control group (Shift+number).
    pub fn add_to_group(&mut self, player: i32, index: usize) {
        if let Some(state) = self.state_mut(player) {
            state.add_to_control_group(index);
        }
    }

    /// Clear all selections for all players.
    pub fn clear_all(&mut self) {
        for state in &mut self.states {
            state.clear();
        }
        for box_ in &mut self.drag_boxes {
            *box_ = DragBox::new();
        }
    }

    /// Trim a list of IDs to `max_selection_size`.
    fn trim_selection(&self, ids: &[u32]) -> Vec<u32> {
        if ids.len() <= self.max_selection_size {
            ids.to_vec()
        } else {
            ids[..self.max_selection_size].to_vec()
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Test picker that pretends objects 1-10 exist and maps them to a
    /// deterministic screen position.
    struct TestPicker;

    impl ObjectPicker for TestPicker {
        fn pick_in_rect(
            &self,
            _player: i32,
            left: i32,
            _top: i32,
            right: i32,
            _bottom: i32,
        ) -> Vec<u32> {
            // Objects at x positions: obj 1 at x=10, obj 2 at x=20, ... obj 10 at x=100.
            (1u32..=10)
                .filter(|&id| {
                    let obj_x = (id as i32) * 10;
                    obj_x >= left && obj_x <= right
                })
                .collect()
        }

        fn pick_at_point(&self, _player: i32, x: i32, _y: i32) -> Option<u32> {
            let id = (x / 10).max(1).min(10);
            if (1..=10).contains(&id) && (id * 10) == x {
                Some(id as u32)
            } else {
                None
            }
        }

        fn pick_all_of_same_kind(&self, _player: i32, _object_id: u32) -> Vec<u32> {
            // Pretend all objects are the same kind for testing.
            (1..=10).map(|i| i as u32).collect()
        }
    }

    fn make_manager() -> SelectionManager {
        SelectionManager::new(2, Box::new(TestPicker))
    }

    #[test]
    fn test_click_select_single() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        let state = mgr.state(0).unwrap();
        assert_eq!(state.selected(), &[3]);
    }

    #[test]
    fn test_click_select_empty_deselects() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        mgr.click_select(0, 999, 999, false, false);
        assert!(mgr.state(0).unwrap().is_empty());
    }

    #[test]
    fn test_click_select_with_shift_adds() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        mgr.click_select(0, 50, 0, false, true);
        let state = mgr.state(0).unwrap();
        assert_eq!(state.selected(), &[3, 5]);
    }

    #[test]
    fn test_click_select_with_ctrl_toggles() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        mgr.click_select(0, 30, 0, true, false);
        assert!(mgr.state(0).unwrap().is_empty());
    }

    #[test]
    fn test_box_selection() {
        let mut mgr = make_manager();
        mgr.begin_drag(0, 5, 0);
        mgr.update_drag(0, 35, 0, 5);
        let count = mgr.finish_drag(0, false, false);
        assert_eq!(count, 3); // objects at x=10, 20, 30
        let state = mgr.state(0).unwrap();
        assert_eq!(state.selected(), &[1, 2, 3]);
    }

    #[test]
    fn test_double_click_selects_all_same_kind() {
        let mut mgr = make_manager();
        mgr.double_click_select(0, 30, 0);
        let state = mgr.state(0).unwrap();
        assert_eq!(state.len(), 10);
    }

    #[test]
    fn test_control_group_create_and_recall() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        mgr.click_select(0, 50, 0, false, true);
        mgr.create_group(0, 0);
        mgr.click_select(0, 999, 999, false, false); // deselect
        assert!(mgr.state(0).unwrap().is_empty());

        let ok = mgr.recall_group(0, 0);
        assert!(ok);
        let state = mgr.state(0).unwrap();
        assert_eq!(state.selected(), &[3, 5]);
    }

    #[test]
    fn test_control_group_add() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        mgr.create_group(0, 0);
        mgr.click_select(0, 50, 0, false, false);
        mgr.add_to_group(0, 0);
        assert_eq!(mgr.state(0).unwrap().get_control_group(0), &[3, 5]);
    }

    #[test]
    fn test_max_selection_size() {
        let mut mgr = make_manager();
        mgr.max_selection_size = 3;
        mgr.double_click_select(0, 30, 0);
        // Should trim to 3.
        assert_eq!(mgr.state(0).unwrap().len(), 3);
    }

    #[test]
    fn test_shift_keeps_selection_on_empty_click() {
        let mut mgr = make_manager();
        mgr.click_select(0, 30, 0, false, false);
        mgr.click_select(0, 999, 999, false, true); // shift-click empty
        let state = mgr.state(0).unwrap();
        assert_eq!(state.selected(), &[3]); // kept
    }
}
