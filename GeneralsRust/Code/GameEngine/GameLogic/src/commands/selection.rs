////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Selection System - Unit selection and group management
//!
//! This module provides the unit selection system that manages
//! player unit selection, group hotkeys, and selection context.
//! Matches C++ SelectionInfo and ContextSensitiveTranslator functionality.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};

use super::command::{Command, CommandType};
use super::rts_command::{RtsCommand, RtsCommandFactory};
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, ICoord2D, IRegion2D, Int, KindOfMaskType, ObjectID,
    PlayerMaskType, Real, Relationship, UnsignedInt,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;

/// Maximum objects in a single selection
pub const MAX_SELECTION_SIZE: usize = 200;

/// Maximum number of control groups (hotkey groups)
pub const MAX_CONTROL_GROUPS: usize = 10;

/// Selection types - matches C++ selection system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionType {
    Replace, // Replace current selection
    Add,     // Add to current selection (Ctrl+click)
    Remove,  // Remove from current selection (Ctrl+click on selected)
    Toggle,  // Toggle selection state
}

/// Object categories for selection filtering - matches C++ KindOf system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Structure,
    Civilian,
    Resource,
    Crate,
    Neutral,
    Enemy,
    Ally,
    Mine,
}

/// Selection criteria for filtering
#[derive(Debug, Clone)]
pub struct SelectionCriteria {
    /// Object kinds to include
    pub include_kinds: HashSet<ObjectKind>,

    /// Object kinds to exclude
    pub exclude_kinds: HashSet<ObjectKind>,

    /// Only select objects owned by these players
    pub allowed_players: HashSet<Int>,

    /// Exclude objects owned by these players
    pub excluded_players: HashSet<Int>,

    /// Only select objects in this region
    pub region_filter: Option<IRegion2D>,

    /// Only select living objects
    pub only_alive: bool,

    /// Only select controllable objects
    pub only_controllable: bool,
}

impl Default for SelectionCriteria {
    fn default() -> Self {
        Self {
            include_kinds: HashSet::new(),
            exclude_kinds: HashSet::new(),
            allowed_players: HashSet::new(),
            excluded_players: HashSet::new(),
            region_filter: None,
            only_alive: true,
            only_controllable: true,
        }
    }
}

/// Information about current selection - matches C++ SelectionInfo
#[derive(Debug, Clone, Default)]
pub struct SelectionInfo {
    // Current selection counts
    pub current_count_enemies: Int,
    pub current_count_civilians: Int,
    pub current_count_mine: Int,
    pub current_count_mine_infantry: Int,
    pub current_count_mine_buildings: Int,
    pub current_count_friends: Int,

    // New selection counts (for preview)
    pub new_count_enemies: Int,
    pub new_count_civilians: Int,
    pub new_count_mine: Int,
    pub new_count_mine_buildings: Int,
    pub new_count_friends: Int,
    pub new_count_garrisonable_buildings: Int,
    pub new_count_crates: Int,

    // Selection flags
    pub select_enemies: Bool,
    pub select_civilians: Bool,
    pub select_mine: Bool,
    pub select_mine_buildings: Bool,
    pub select_friends: Bool,
}

/// A single selected object with metadata
#[derive(Debug, Clone)]
pub struct SelectedObject {
    pub object_id: ObjectID,
    pub drawable_id: Option<DrawableID>,
    pub object_kind: ObjectKind,
    pub owner_id: Int,
    pub position: Coord3D,
    pub is_alive: bool,
    pub is_controllable: bool,
    pub selection_time: UnsignedInt, // Frame when selected
}

/// Control group (hotkey group) for quick selection
#[derive(Debug, Clone)]
pub struct ControlGroup {
    /// Objects in this group
    pub objects: Vec<ObjectID>,

    /// When group was last updated
    pub last_update_frame: UnsignedInt,

    /// Whether to maintain formation when selecting group
    pub maintain_formation: bool,

    /// Group center position (for camera focusing)
    pub center_position: Option<Coord3D>,
}

impl Default for ControlGroup {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            last_update_frame: 0,
            maintain_formation: false,
            center_position: None,
        }
    }
}

/// Main selection manager for a single player
pub struct PlayerSelection {
    /// Player ID this selection belongs to
    player_id: Int,

    /// Currently selected objects
    selected_objects: HashMap<ObjectID, SelectedObject>,

    /// Control groups (0-9)
    control_groups: [ControlGroup; MAX_CONTROL_GROUPS],

    /// Selection history for undo/redo
    selection_history: Vec<Vec<ObjectID>>,
    max_history_size: usize,

    /// Current frame number
    current_frame: UnsignedInt,

    /// Selection bounds for UI display
    selection_bounds: Option<IRegion2D>,

    /// Last selection change time
    last_selection_change: UnsignedInt,

    /// Object lookup interface
    object_lookup: Option<Arc<dyn ObjectLookup>>,
}

/// Trait for object lookup and information
pub trait ObjectLookup: Send + Sync {
    fn get_object_info(&self, id: ObjectID) -> Option<ObjectInfo>;
    fn get_objects_in_region(&self, region: &IRegion2D) -> Vec<ObjectID>;
    fn get_all_objects(&self) -> Vec<ObjectID>;
    /// Optional: return the set of objects currently within the local player's view.
    ///
    /// When implemented, this enables C++-accurate "on-screen only" selection behaviors (e.g.
    /// double-click to select matching units on screen).
    fn get_objects_on_screen(&self, _player_id: Int) -> Option<Vec<ObjectID>> {
        None
    }
    fn get_object_position(&self, id: ObjectID) -> Option<Coord3D>;
    fn is_object_alive(&self, id: ObjectID) -> bool;
    fn is_object_visible_to_player(&self, player_id: Int, object_id: ObjectID) -> bool;
    fn is_object_detected_by_player(&self, player_id: Int, object_id: ObjectID) -> bool;
    fn get_object_owner(&self, id: ObjectID) -> Option<Int>;
    fn get_object_kind(&self, id: ObjectID) -> Option<ObjectKind>;
    fn can_player_control(&self, player_id: Int, object_id: ObjectID) -> bool;
    fn set_object_selected(&self, object_id: ObjectID, selected: bool);
    /// Resolve selection clicks/box hits on contained, unselectable objects into a selectable target.
    /// Mirrors C++ `SelectionInfo::addDrawableToList` behavior for visible riders.
    fn resolve_selection_target(&self, object_id: ObjectID) -> ObjectID {
        object_id
    }
}

/// Object information for selection system
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub id: ObjectID,
    pub drawable_id: Option<DrawableID>,
    pub position: Coord3D,
    pub owner_id: Int,
    pub kind: ObjectKind,
    pub is_alive: bool,
    pub is_selectable: bool,
    pub is_controllable: bool,
    pub is_crate: bool,
    pub is_garrisonable_building: bool,
}

/// External UI/input mode flags used by the selection context-command logic.
///
/// C++ Reference: `contextCommandForNewSelection` in `SelectionInfo.cpp`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionContextOptions {
    pub force_attack_mode: bool,
    pub force_move_mode: bool,
    pub use_alternate_mouse: bool,
    pub prefer_selection_mode: bool,
}

impl PlayerSelection {
    /// Create new player selection
    pub fn new(player_id: Int) -> Self {
        Self {
            player_id,
            selected_objects: HashMap::new(),
            control_groups: Default::default(),
            selection_history: Vec::new(),
            max_history_size: 20,
            current_frame: 0,
            selection_bounds: None,
            last_selection_change: 0,
            object_lookup: None,
        }
    }

    /// Set object lookup interface
    pub fn set_object_lookup(&mut self, lookup: Arc<dyn ObjectLookup>) {
        self.object_lookup = Some(lookup);
    }

    /// Update selection for current frame
    pub fn update(&mut self, frame: UnsignedInt) {
        self.current_frame = frame;

        // Clean up dead objects from selection
        self.cleanup_dead_objects();

        // Update control groups
        self.update_control_groups();
    }

    /// Select objects by ID list
    pub fn select_objects(
        &mut self,
        object_ids: Vec<ObjectID>,
        selection_type: SelectionType,
    ) -> bool {
        let object_ids = self.resolve_selection_targets(object_ids);
        match selection_type {
            SelectionType::Replace => {
                self.clear_selection();
                self.add_objects_to_selection(object_ids)
            }
            SelectionType::Add => self.add_objects_to_selection(object_ids),
            SelectionType::Remove => self.remove_objects_from_selection(object_ids),
            SelectionType::Toggle => self.toggle_objects_in_selection(object_ids),
        }
    }

    /// Select objects in a region (box selection)
    pub fn select_in_region(
        &mut self,
        region: IRegion2D,
        selection_type: SelectionType,
        criteria: Option<SelectionCriteria>,
    ) -> bool {
        let Some(lookup) = &self.object_lookup else {
            return false;
        };

        let objects_in_region = lookup.get_objects_in_region(&region);

        let filtered = if let Some(criteria) = criteria {
            self.filter_objects_by_criteria(&objects_in_region, &criteria, lookup.as_ref())
        } else {
            let mut default_criteria = SelectionCriteria::default();
            default_criteria.allowed_players.insert(self.player_id);
            self.filter_objects_by_criteria(&objects_in_region, &default_criteria, lookup.as_ref())
        };

        if !filtered.is_empty() {
            self.select_objects(filtered, selection_type);
            self.selection_bounds = Some(region);
            return true;
        }
        false
    }

    /// Add objects to current selection
    fn add_objects_to_selection(&mut self, object_ids: Vec<ObjectID>) -> bool {
        if self.selected_objects.len() + object_ids.len() > MAX_SELECTION_SIZE {
            return false; // Selection would be too large
        }

        let mut added_any = false;

        if let Some(lookup) = &self.object_lookup {
            let allow_uncontrollable_single =
                self.selected_objects.is_empty() && object_ids.len() == 1;
            for object_id in object_ids {
                if !self.selected_objects.contains_key(&object_id) {
                    if let Some(obj_info) = lookup.get_object_info(object_id) {
                        let can_control = lookup.can_player_control(self.player_id, object_id);
                        let allow_uncontrollable =
                            allow_uncontrollable_single && obj_info.is_selectable;

                        if !can_control && !allow_uncontrollable {
                            continue;
                        }

                        if !can_control {
                            let rel = self.relationship_to_owner(obj_info.owner_id);
                            if !lookup.is_object_visible_to_player(self.player_id, object_id) {
                                continue;
                            }
                            if matches!(rel, Relationship::Enemies | Relationship::Neutral)
                                && !lookup.is_object_detected_by_player(self.player_id, object_id)
                            {
                                continue;
                            }
                        }

                        if allow_uncontrollable {
                            if !self.selected_objects.is_empty() {
                                continue;
                            }
                        } else if !self.selected_objects.is_empty()
                            && self
                                .selected_objects
                                .values()
                                .any(|selected| selected.owner_id != self.player_id)
                        {
                            continue;
                        }

                        if can_control || allow_uncontrollable {
                            let selected_obj = SelectedObject {
                                object_id,
                                drawable_id: obj_info.drawable_id,
                                object_kind: obj_info.kind,
                                owner_id: obj_info.owner_id,
                                position: obj_info.position,
                                is_alive: obj_info.is_alive,
                                is_controllable: obj_info.is_controllable,
                                selection_time: self.current_frame,
                            };

                            self.selected_objects.insert(object_id, selected_obj);
                            lookup.set_object_selected(object_id, true);
                            added_any = true;
                        }
                    }
                }
            }
        }

        if added_any {
            self.on_selection_changed();
        }

        added_any
    }

    /// Remove objects from current selection
    fn remove_objects_from_selection(&mut self, object_ids: Vec<ObjectID>) -> bool {
        let mut removed_any = false;

        if let Some(lookup) = &self.object_lookup {
            for object_id in object_ids {
                if self.selected_objects.remove(&object_id).is_some() {
                    lookup.set_object_selected(object_id, false);
                    removed_any = true;
                }
            }
        } else {
            for object_id in object_ids {
                if self.selected_objects.remove(&object_id).is_some() {
                    removed_any = true;
                }
            }
        }

        if removed_any {
            self.on_selection_changed();
        }

        removed_any
    }

    /// Toggle objects in selection
    fn toggle_objects_in_selection(&mut self, object_ids: Vec<ObjectID>) -> bool {
        let mut to_add = Vec::new();
        let mut to_remove = Vec::new();

        for object_id in object_ids {
            if self.selected_objects.contains_key(&object_id) {
                to_remove.push(object_id);
            } else {
                to_add.push(object_id);
            }
        }

        let added = if !to_add.is_empty() {
            self.add_objects_to_selection(to_add)
        } else {
            false
        };

        let removed = if !to_remove.is_empty() {
            self.remove_objects_from_selection(to_remove)
        } else {
            false
        };

        added || removed
    }

    /// Clear current selection
    pub fn clear_selection(&mut self) {
        if !self.selected_objects.is_empty() {
            if let Some(lookup) = &self.object_lookup {
                for object_id in self.selected_objects.keys().copied() {
                    lookup.set_object_selected(object_id, false);
                }
            }
            self.selected_objects.clear();
            self.selection_bounds = None;
            self.on_selection_changed();
        }
    }

    /// Get currently selected object IDs
    pub fn get_selected_objects(&self) -> Vec<ObjectID> {
        self.selected_objects.keys().copied().collect()
    }

    /// Get selected objects with metadata
    pub fn get_selected_objects_info(&self) -> Vec<&SelectedObject> {
        self.selected_objects.values().collect()
    }

    /// Check if object is selected
    pub fn is_object_selected(&self, object_id: ObjectID) -> bool {
        self.selected_objects.contains_key(&object_id)
    }

    /// Get selection count
    pub fn get_selection_count(&self) -> usize {
        self.selected_objects.len()
    }

    /// Create control group from current selection
    pub fn create_control_group(&mut self, group_index: usize) -> bool {
        if group_index >= MAX_CONTROL_GROUPS || self.selected_objects.is_empty() {
            return false;
        }

        let object_ids: Vec<ObjectID> = self.selected_objects.keys().copied().collect();

        // Calculate center position
        let center = self.calculate_selection_center(&object_ids);

        self.control_groups[group_index] = ControlGroup {
            objects: object_ids,
            last_update_frame: self.current_frame,
            maintain_formation: false, // Could be configurable
            center_position: center,
        };

        true
    }

    /// Select control group
    pub fn select_control_group(&mut self, group_index: usize, add_to_selection: bool) -> bool {
        if group_index >= MAX_CONTROL_GROUPS {
            return false;
        }

        let group = &self.control_groups[group_index];
        if group.objects.is_empty() {
            return false;
        }

        // Filter out dead objects
        let alive_objects = self.filter_alive_objects(&group.objects);
        if alive_objects.is_empty() {
            return false;
        }

        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };

        self.select_objects(alive_objects, selection_type)
    }

    /// Add current selection to control group
    pub fn add_to_control_group(&mut self, group_index: usize) -> bool {
        if group_index >= MAX_CONTROL_GROUPS || self.selected_objects.is_empty() {
            return false;
        }

        let selected_ids: Vec<ObjectID> = self.selected_objects.keys().copied().collect();

        let snapshot = {
            let group = &mut self.control_groups[group_index];
            for object_id in &selected_ids {
                if !group.objects.contains(object_id) {
                    group.objects.push(*object_id);
                }
            }
            group.last_update_frame = self.current_frame;
            group.objects.clone()
        };

        let center = self.calculate_selection_center(&snapshot);
        if let Some(group) = self.control_groups.get_mut(group_index) {
            group.center_position = center;
        }

        true
    }

    /// Get control group object count
    pub fn get_control_group_size(&self, group_index: usize) -> usize {
        if group_index >= MAX_CONTROL_GROUPS {
            return 0;
        }

        self.control_groups[group_index].objects.len()
    }

    /// Get control group object IDs. Returns empty slice for invalid index.
    pub fn get_control_group_objects(&self, group_index: usize) -> &[ObjectID] {
        if group_index >= MAX_CONTROL_GROUPS {
            return &[];
        }
        &self.control_groups[group_index].objects
    }

    /// Set control group contents directly (used for save/load xfer restore).
    pub fn set_control_group_objects(&mut self, group_index: usize, objects: Vec<ObjectID>) {
        if group_index >= MAX_CONTROL_GROUPS {
            return;
        }
        self.control_groups[group_index].objects = objects;
        self.control_groups[group_index].last_update_frame = self.current_frame;
    }

    /// Select matching units (double-click behavior)
    pub fn select_matching_units(
        &mut self,
        template_object: ObjectID,
        on_screen_only: bool,
    ) -> bool {
        if let Some(lookup_arc) = &self.object_lookup {
            let lookup = Arc::clone(lookup_arc);
            if let Some(template_info) = lookup.get_object_info(template_object) {
                // Find all objects of the same kind owned by the player
                let mut criteria = SelectionCriteria::default();
                criteria.include_kinds.insert(template_info.kind);
                criteria.allowed_players.insert(self.player_id);
                let all_objects = if on_screen_only {
                    lookup
                        .get_objects_on_screen(self.player_id)
                        .unwrap_or_else(|| lookup.get_all_objects())
                } else {
                    lookup.get_all_objects()
                };

                let matching_objects =
                    self.filter_objects_by_criteria(&all_objects, &criteria, lookup.as_ref());

                let matching_objects = if on_screen_only {
                    matching_objects
                        .into_iter()
                        .filter(|id| lookup.is_object_visible_to_player(self.player_id, *id))
                        .collect::<Vec<_>>()
                } else {
                    matching_objects
                };

                if !matching_objects.is_empty() {
                    self.select_objects(matching_objects, SelectionType::Replace);
                    return true;
                }
            };
        }

        false
    }

    /// Get selection info for UI display - matches C++ SelectionInfo
    pub fn get_selection_info(&self) -> SelectionInfo {
        let mut info = SelectionInfo::default();

        for selected_obj in self.selected_objects.values() {
            if selected_obj.owner_id == self.player_id {
                info.current_count_mine += 1;
                if selected_obj.object_kind == ObjectKind::Infantry {
                    info.current_count_mine_infantry += 1;
                }
                if matches!(
                    selected_obj.object_kind,
                    ObjectKind::Building | ObjectKind::Structure
                ) {
                    info.current_count_mine_buildings += 1;
                }
                continue;
            }

            match self.relationship_to_owner(selected_obj.owner_id) {
                Relationship::Enemies => info.current_count_enemies += 1,
                Relationship::Neutral => info.current_count_civilians += 1,
                Relationship::Allies => {
                    info.current_count_friends += 1;
                }
            }
        }

        info
    }

    /// Compute selection info for a prospective new selection set.
    ///
    /// Mirrors the count-gathering behavior in C++ `contextCommandForNewSelection` (SelectionInfo.cpp),
    /// but expressed in terms of `ObjectID`/`ObjectInfo` instead of drawables.
    pub fn get_selection_info_for_new_selection(
        &self,
        newly_selected: &[ObjectID],
    ) -> SelectionInfo {
        let mut info = self.get_selection_info();

        let Some(lookup) = &self.object_lookup else {
            return info;
        };

        for &object_id in newly_selected {
            let object_id = lookup.resolve_selection_target(object_id);
            let Some(obj_info) = lookup.get_object_info(object_id) else {
                continue;
            };
            if !obj_info.is_alive || !obj_info.is_selectable {
                continue;
            }

            if obj_info.is_garrisonable_building {
                info.new_count_garrisonable_buildings += 1;
            }
            if obj_info.is_crate {
                info.new_count_crates += 1;
            }

            if obj_info.owner_id == self.player_id {
                info.new_count_mine += 1;
                if matches!(obj_info.kind, ObjectKind::Building | ObjectKind::Structure) {
                    info.new_count_mine_buildings += 1;
                }
            } else {
                match self.relationship_to_owner(obj_info.owner_id) {
                    Relationship::Enemies => info.new_count_enemies += 1,
                    Relationship::Neutral => info.new_count_civilians += 1,
                    Relationship::Allies => {
                        info.new_count_friends += 1;
                    }
                }
            }
        }

        info
    }

    /// Decide whether the click/drag should invoke a context command instead of changing selection.
    ///
    /// This is a direct port of the boolean flow in C++ `contextCommandForNewSelection`.
    /// The `evaluate_context_command` callback corresponds to C++ `TheGameClient->evaluateContextCommand(...) != MSG_INVALID`.
    pub fn context_command_for_new_selection<E>(
        &self,
        newly_selected: &[ObjectID],
        selection_is_point: bool,
        options: SelectionContextOptions,
        evaluate_context_command: E,
    ) -> bool
    where
        E: Fn(ObjectID, Coord3D) -> bool,
    {
        if options.force_attack_mode || options.force_move_mode {
            return false;
        }

        if options.use_alternate_mouse {
            return false;
        }

        let info = self.get_selection_info_for_new_selection(newly_selected);

        if info.current_count_enemies > 0
            || info.current_count_friends > 0
            || info.current_count_civilians > 0
        {
            return false;
        }

        let Some(lookup) = &self.object_lookup else {
            return false;
        };

        // Identify one representative object for context-command evaluation.
        let mut new_mine = None;
        let mut new_friendly = None;
        let mut new_enemy = None;
        let mut new_civilian = None;

        for &object_id in newly_selected {
            let object_id = lookup.resolve_selection_target(object_id);
            let Some(obj_info) = lookup.get_object_info(object_id) else {
                continue;
            };
            if !obj_info.is_alive || !obj_info.is_selectable {
                continue;
            }

            if obj_info.owner_id == self.player_id {
                new_mine.get_or_insert((object_id, obj_info.position));
            } else {
                match self.relationship_to_owner(obj_info.owner_id) {
                    Relationship::Enemies => {
                        new_enemy.get_or_insert((object_id, obj_info.position));
                    }
                    Relationship::Neutral => {
                        new_civilian.get_or_insert((object_id, obj_info.position));
                    }
                    Relationship::Allies => {
                        new_friendly.get_or_insert((object_id, obj_info.position));
                    }
                }
            }
        }

        if info.current_count_mine > 0 {
            if info.new_count_enemies > 0 {
                if info.new_count_enemies == 1 && selection_is_point {
                    if let Some((enemy_id, pos)) = new_enemy {
                        return evaluate_context_command(enemy_id, pos);
                    }
                }
                return selection_is_point;
            }

            if info.new_count_mine > 0 {
                if info.new_count_mine == 1 && selection_is_point && !options.prefer_selection_mode
                {
                    if let Some((mine_id, pos)) = new_mine {
                        return evaluate_context_command(mine_id, pos);
                    }
                }
                return false;
            }

            if info.new_count_friends > 0 {
                if info.new_count_friends == 1 && selection_is_point {
                    if let Some((friend_id, pos)) = new_friendly {
                        return evaluate_context_command(friend_id, pos);
                    }
                }
                return false;
            }

            if info.current_count_mine_infantry > 0 && info.new_count_garrisonable_buildings == 1 {
                return true;
            }

            if info.new_count_civilians > 0 {
                if info.new_count_civilians == 1 && selection_is_point {
                    if let Some((civilian_id, pos)) = new_civilian {
                        return evaluate_context_command(civilian_id, pos);
                    }
                }
                return false;
            }

            if info.new_count_crates > 0 {
                return info.new_count_crates == 1 && selection_is_point;
            }
        }

        if info.current_count_mine == 0 {
            return false;
        }

        selection_is_point
    }

    fn relationship_to_owner(&self, owner_id: Int) -> Relationship {
        if owner_id == self.player_id {
            return Relationship::Allies;
        }

        let Ok(list) = player_list().read() else {
            return Relationship::Neutral;
        };

        let Some(me) = list.get_player(self.player_id) else {
            return Relationship::Neutral;
        };
        let Some(them) = list.get_player(owner_id) else {
            return Relationship::Neutral;
        };

        let (Ok(me_guard), Ok(them_guard)) = (me.read(), them.read()) else {
            return Relationship::Neutral;
        };

        me_guard.get_relationship(&them_guard)
    }

    /// Filter objects by criteria
    fn filter_objects_by_criteria(
        &self,
        object_ids: &[ObjectID],
        criteria: &SelectionCriteria,
        lookup: &dyn ObjectLookup,
    ) -> Vec<ObjectID> {
        let mut filtered = Vec::new();
        let mut seen = HashSet::new();

        for &object_id in object_ids {
            let object_id = lookup.resolve_selection_target(object_id);
            if !seen.insert(object_id) {
                continue;
            }
            if let Some(obj_info) = lookup.get_object_info(object_id) {
                // Check alive status
                if criteria.only_alive && !obj_info.is_alive {
                    continue;
                }

                // Check controllable status
                if criteria.only_controllable && !obj_info.is_controllable {
                    continue;
                }

                // Check object kind inclusion
                if !criteria.include_kinds.is_empty()
                    && !criteria.include_kinds.contains(&obj_info.kind)
                {
                    continue;
                }

                // Check object kind exclusion
                if criteria.exclude_kinds.contains(&obj_info.kind) {
                    continue;
                }

                // Check player inclusion
                if !criteria.allowed_players.is_empty()
                    && !criteria.allowed_players.contains(&obj_info.owner_id)
                {
                    continue;
                }

                // Check player exclusion
                if criteria.excluded_players.contains(&obj_info.owner_id) {
                    continue;
                }

                // Check region filter
                if let Some(region) = &criteria.region_filter {
                    if !self.point_in_region(obj_info.position, region) {
                        continue;
                    }
                }

                filtered.push(object_id);
            }
        }

        filtered
    }

    fn resolve_selection_targets(&self, object_ids: Vec<ObjectID>) -> Vec<ObjectID> {
        let Some(lookup) = &self.object_lookup else {
            return object_ids;
        };

        let mut resolved = Vec::with_capacity(object_ids.len());
        let mut seen = HashSet::new();
        for object_id in object_ids {
            let target = lookup.resolve_selection_target(object_id);
            if seen.insert(target) {
                resolved.push(target);
            }
        }
        resolved
    }

    /// Filter out dead objects
    fn filter_alive_objects(&self, object_ids: &[ObjectID]) -> Vec<ObjectID> {
        let mut alive_objects = Vec::new();

        if let Some(lookup) = &self.object_lookup {
            for &object_id in object_ids {
                if lookup.is_object_alive(object_id) {
                    alive_objects.push(object_id);
                }
            }
        }

        alive_objects
    }

    /// Calculate center position of objects
    fn calculate_selection_center(&self, object_ids: &[ObjectID]) -> Option<Coord3D> {
        if object_ids.is_empty() {
            return None;
        }

        if let Some(lookup) = &self.object_lookup {
            let mut total_pos = Coord3D::new(0.0, 0.0, 0.0);
            let mut count = 0;

            for &object_id in object_ids {
                if let Some(pos) = lookup.get_object_position(object_id) {
                    total_pos.x += pos.x;
                    total_pos.y += pos.y;
                    total_pos.z += pos.z;
                    count += 1;
                }
            }

            if count > 0 {
                let count_real = count as Real;
                return Some(Coord3D::new(
                    total_pos.x / count_real,
                    total_pos.y / count_real,
                    total_pos.z / count_real,
                ));
            }
        }

        None
    }

    /// Check if point is in region
    fn point_in_region(&self, point: Coord3D, region: &IRegion2D) -> bool {
        let px = point.x as Int;
        let py = point.y as Int;
        px >= region.lo.x && px <= region.hi.x && py >= region.lo.y && py <= region.hi.y
    }

    /// Clean up dead objects from selection and control groups
    fn cleanup_dead_objects(&mut self) {
        let Some(lookup) = &self.object_lookup else {
            return;
        };

        let mut removed_any = false;
        let mut to_remove = Vec::new();

        for &object_id in self.selected_objects.keys() {
            if !lookup.is_object_alive(object_id) {
                to_remove.push(object_id);
            }
        }

        for object_id in to_remove {
            if self.selected_objects.remove(&object_id).is_some() {
                lookup.set_object_selected(object_id, false);
                removed_any = true;
            }
        }

        for group in &mut self.control_groups {
            group.objects.retain(|&id| lookup.is_object_alive(id));
        }

        if removed_any {
            self.on_selection_changed();
        }
    }

    /// Update control groups
    fn update_control_groups(&mut self) {
        let centers: Vec<_> = self
            .control_groups
            .iter()
            .map(|group| {
                if group.objects.is_empty() {
                    None
                } else {
                    self.calculate_selection_center(&group.objects)
                }
            })
            .collect();

        for (group, center) in self.control_groups.iter_mut().zip(centers) {
            group.center_position = center;
        }
    }

    /// Called when selection changes
    fn on_selection_changed(&mut self) {
        self.last_selection_change = self.current_frame;

        // Add to history
        let current_selection: Vec<ObjectID> = self.selected_objects.keys().copied().collect();
        self.selection_history.push(current_selection);

        // Trim history
        if self.selection_history.len() > self.max_history_size {
            self.selection_history.remove(0);
        }

        // Recalculate bounds
        self.update_selection_bounds();
    }

    /// Update selection bounds for UI display
    fn update_selection_bounds(&mut self) {
        if self.selected_objects.is_empty() {
            self.selection_bounds = None;
            return;
        }

        let mut min_x = Real::INFINITY;
        let mut min_y = Real::INFINITY;
        let mut max_x = Real::NEG_INFINITY;
        let mut max_y = Real::NEG_INFINITY;

        for selected_obj in self.selected_objects.values() {
            let pos = selected_obj.position;
            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x);
            max_y = max_y.max(pos.y);
        }

        self.selection_bounds = Some(IRegion2D::new(
            ICoord2D::new(min_x as Int, min_y as Int),
            ICoord2D::new(max_x as Int, max_y as Int),
        ));
    }

    /// Get selection bounds
    pub fn get_selection_bounds(&self) -> Option<IRegion2D> {
        self.selection_bounds
    }

    pub fn get_last_selection_change(&self) -> UnsignedInt {
        self.last_selection_change
    }

    /// Undo last selection change
    pub fn undo_selection(&mut self) -> bool {
        if self.selection_history.len() > 1 {
            self.selection_history.pop(); // Remove current
            if let Some(previous_selection) = self.selection_history.last() {
                let previous = previous_selection.clone();
                self.select_objects(previous, SelectionType::Replace);
                return true;
            }
        }
        false
    }
}

/// Global selection manager - manages selection for all players
pub struct SelectionManager {
    /// Player selections
    player_selections: HashMap<Int, PlayerSelection>,

    /// Global object lookup
    object_lookup: Option<Arc<dyn ObjectLookup>>,

    /// Current frame
    current_frame: UnsignedInt,

    /// Frame when selection last changed (matches C++ InGameUI->getFrameSelectionChanged)
    last_selection_change: UnsignedInt,
}

impl SelectionManager {
    /// Create new selection manager
    pub fn new() -> Self {
        Self {
            player_selections: HashMap::new(),
            object_lookup: None,
            current_frame: 0,
            last_selection_change: 0,
        }
    }

    /// Set object lookup interface
    pub fn set_object_lookup(&mut self, lookup: Arc<dyn ObjectLookup>) {
        self.object_lookup = Some(lookup.clone());

        // Update all existing player selections
        for selection in self.player_selections.values_mut() {
            selection.set_object_lookup(lookup.clone());
        }
    }

    /// Initialize player selection
    pub fn initialize_player(&mut self, player_id: Int) {
        let mut player_selection = PlayerSelection::new(player_id);

        if let Some(lookup) = &self.object_lookup {
            player_selection.set_object_lookup(lookup.clone());
        }

        self.player_selections.insert(player_id, player_selection);
    }

    /// Update for current frame
    pub fn update(&mut self, frame: UnsignedInt) {
        self.current_frame = frame;

        for selection in self.player_selections.values_mut() {
            selection.update(frame);
        }

        self.last_selection_change = self
            .player_selections
            .values()
            .map(|selection| selection.get_last_selection_change())
            .max()
            .unwrap_or(0);
    }

    /// Get player selection
    pub fn get_player_selection(&mut self, player_id: Int) -> Option<&mut PlayerSelection> {
        self.player_selections.get_mut(&player_id)
    }

    /// Get player selection (read-only)
    pub fn get_player_selection_ref(&self, player_id: Int) -> Option<&PlayerSelection> {
        self.player_selections.get(&player_id)
    }

    pub fn get_frame_selection_changed(&self) -> UnsignedInt {
        self.player_selections
            .values()
            .map(|selection| selection.get_last_selection_change())
            .max()
            .unwrap_or(0)
    }

    pub fn is_object_selected_by_any_player(&self, object_id: ObjectID) -> bool {
        self.player_selections
            .values()
            .any(|selection| selection.is_object_selected(object_id))
    }
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global selection manager instance
use once_cell::sync::Lazy;
static SELECTION_MANAGER: Lazy<Arc<RwLock<SelectionManager>>> =
    Lazy::new(|| Arc::new(RwLock::new(SelectionManager::new())));

/// Get global selection manager
pub fn get_selection_manager() -> Arc<RwLock<SelectionManager>> {
    SELECTION_MANAGER.clone()
}

/// Selection lookup backed by the global `OBJECT_REGISTRY`.
pub struct RegistryObjectLookup;

impl RegistryObjectLookup {
    fn classify_kind(
        is_neutral: bool,
        is_infantry: bool,
        is_vehicle: bool,
        is_aircraft: bool,
        is_building: bool,
        is_structure: bool,
        is_resource: bool,
        is_mine: bool,
    ) -> ObjectKind {
        if is_mine {
            return ObjectKind::Mine;
        }
        if is_resource {
            return ObjectKind::Resource;
        }
        if is_infantry {
            return if is_neutral {
                ObjectKind::Civilian
            } else {
                ObjectKind::Infantry
            };
        }
        if is_vehicle {
            return if is_neutral {
                ObjectKind::Civilian
            } else {
                ObjectKind::Vehicle
            };
        }
        if is_aircraft {
            return if is_neutral {
                ObjectKind::Civilian
            } else {
                ObjectKind::Aircraft
            };
        }
        if is_building {
            return if is_neutral {
                ObjectKind::Civilian
            } else {
                ObjectKind::Building
            };
        }
        if is_structure {
            return if is_neutral {
                ObjectKind::Civilian
            } else {
                ObjectKind::Structure
            };
        }

        if is_neutral {
            ObjectKind::Civilian
        } else {
            ObjectKind::Neutral
        }
    }
}

impl ObjectLookup for RegistryObjectLookup {
    fn get_object_info(&self, id: ObjectID) -> Option<ObjectInfo> {
        use crate::common::KindOf;
        use crate::object::drawable::DrawableArcExt;

        let obj = OBJECT_REGISTRY.get_object(id)?;
        let guard = obj.read().ok()?;

        let owner_id = guard
            .get_controlling_player_id()
            .map(|p| p as Int)
            .unwrap_or(-1);
        let is_neutral = guard.is_neutral_controlled();

        let kind = Self::classify_kind(
            is_neutral,
            guard.is_kind_of(KindOf::Infantry),
            guard.is_kind_of(KindOf::Vehicle),
            guard.is_kind_of(KindOf::Aircraft),
            guard.is_kind_of(KindOf::Building),
            guard.is_kind_of(KindOf::Structure),
            guard.is_kind_of(KindOf::ResourceNode),
            guard.is_kind_of(KindOf::Mine),
        );

        let is_alive = !guard.is_effectively_dead();
        let is_selectable = guard.is_selectable();
        let is_controllable = is_selectable;

        let drawable_id = guard.get_drawable().map(|d| d.get_id() as DrawableID);
        let is_crate = guard.is_kind_of(KindOf::Crate);
        let is_garrisonable_building = (guard.is_kind_of(KindOf::Structure)
            || guard.is_kind_of(KindOf::Building))
            && guard.get_contain().is_some();

        Some(ObjectInfo {
            id,
            drawable_id,
            position: *guard.get_position(),
            owner_id,
            kind,
            is_alive,
            is_selectable,
            is_controllable,
            is_crate,
            is_garrisonable_building,
        })
    }

    fn get_objects_in_region(&self, region: &IRegion2D) -> Vec<ObjectID> {
        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj| {
                let guard = obj.read().ok()?;
                let pos = guard.get_position();
                let x = pos.x as Int;
                let y = pos.y as Int;
                if x >= region.lo.x && x <= region.hi.x && y >= region.lo.y && y <= region.hi.y {
                    Some(guard.get_id())
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_all_objects(&self) -> Vec<ObjectID> {
        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj| obj.read().ok().map(|guard| guard.get_id()))
            .collect()
    }

    fn get_object_position(&self, id: ObjectID) -> Option<Coord3D> {
        let obj = OBJECT_REGISTRY.get_object(id)?;
        let guard = obj.read().ok()?;
        Some(*guard.get_position())
    }

    fn is_object_alive(&self, id: ObjectID) -> bool {
        let Some(obj) = OBJECT_REGISTRY.get_object(id) else {
            return false;
        };
        obj.read().ok().is_some_and(|guard| !guard.is_destroyed())
    }

    fn is_object_visible_to_player(&self, player_id: Int, object_id: ObjectID) -> bool {
        let player_id: u32 = match player_id.try_into() {
            Ok(value) => value,
            Err(_) => return false,
        };
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return false;
        };
        obj.read()
            .ok()
            .is_some_and(|guard| guard.is_visible_to_player(player_id))
    }

    fn is_object_detected_by_player(&self, _player_id: Int, object_id: ObjectID) -> bool {
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return false;
        };
        obj.read().ok().is_some_and(|guard| guard.is_detected())
    }

    fn get_object_owner(&self, id: ObjectID) -> Option<Int> {
        let obj = OBJECT_REGISTRY.get_object(id)?;
        let guard = obj.read().ok()?;
        guard.get_controlling_player_id().map(|p| p as Int)
    }

    fn get_object_kind(&self, id: ObjectID) -> Option<ObjectKind> {
        use crate::common::KindOf;

        let obj = OBJECT_REGISTRY.get_object(id)?;
        let guard = obj.read().ok()?;
        Some(Self::classify_kind(
            guard.is_neutral_controlled(),
            guard.is_kind_of(KindOf::Infantry),
            guard.is_kind_of(KindOf::Vehicle),
            guard.is_kind_of(KindOf::Aircraft),
            guard.is_kind_of(KindOf::Building),
            guard.is_kind_of(KindOf::Structure),
            guard.is_kind_of(KindOf::ResourceNode),
            guard.is_kind_of(KindOf::Mine),
        ))
    }

    fn can_player_control(&self, player_id: Int, object_id: ObjectID) -> bool {
        use crate::common::KindOf;

        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return false;
        };
        let Ok(guard) = obj.read() else {
            return false;
        };
        let Some(owner) = guard.get_controlling_player_id() else {
            return false;
        };
        if owner as Int != player_id {
            return false;
        }

        let is_selectable =
            guard.is_kind_of(KindOf::Selectable) || guard.is_kind_of(KindOf::AlwaysSelectable);
        is_selectable
            && !guard
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::UNSELECTABLE)
    }

    fn set_object_selected(&self, object_id: ObjectID, selected: bool) {
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };

        let Ok(mut guard) = obj.write() else {
            return;
        };
        {
            if let Some(drawable) = guard.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    drawable_guard.set_selected(selected);
                }
            }
            let _ = if selected {
                guard.set_model_condition_flags(crate::common::ModelConditionFlags::SELECTED)
            } else {
                guard.clear_model_condition_flags(crate::common::ModelConditionFlags::SELECTED)
            };
        }
    }

    fn resolve_selection_target(&self, object_id: ObjectID) -> ObjectID {
        use crate::common::types::ObjectStatusMaskType;
        use crate::modules::ContainModuleInterfaceExt;

        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return object_id;
        };
        let Ok(guard) = obj.read() else {
            return object_id;
        };

        if !guard
            .get_status_bits()
            .contains(ObjectStatusMaskType::UNSELECTABLE)
        {
            return object_id;
        }

        let Some(container_id) = guard.get_contained_by() else {
            return object_id;
        };

        // Enclosing containers hide their contents; don't propagate selection in that case.
        // Prefer the container's contain module answer if available, otherwise fall back to MASKED.
        let Some(container) = OBJECT_REGISTRY.get_object(container_id) else {
            return object_id;
        };
        let Ok(container_guard) = container.read() else {
            return object_id;
        };

        let is_enclosing = container_guard
            .get_contain()
            .map(|contain| contain.is_enclosing_container_for(&*guard))
            .unwrap_or_else(|| {
                guard
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::MASKED)
            });
        if is_enclosing {
            return object_id;
        }

        if container_guard.get_drawable().is_none() {
            return object_id;
        }

        container_id
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
