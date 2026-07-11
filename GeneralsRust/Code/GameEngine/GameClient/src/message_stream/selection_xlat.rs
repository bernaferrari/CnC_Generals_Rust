//! Selection Translator - Port of C++ SelectionXlat system
//!
//! This module handles:
//! - Mouse click → object selection logic (single and box selection)
//! - Drag selection implementation
//! - Keyboard shortcuts for control groups (0-9)
//! - Selection filtering (CanSelectDrawable logic from C++)
//! - Double-click for selecting all of same type

use log::{debug, warn};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use super::game_message::*;
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::display::view::{with_tactical_view, with_tactical_view_ref, IPoint2, Point3};
use crate::gui::game_window::{GameWindow, WindowStatus};
use crate::gui::window_manager::with_window_manager_ref;
use crate::helpers::TheInGameUI;
use crate::input::{KeyCode, KeyModifiers};
use game_engine::common::ini::ini_game_data::get_global_data;
use gamelogic::common::types::{KindOf, ObjectShroudStatus, ObjectStatusMaskType};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{player_list, PLAYER_INDEX_INVALID};

/// Drag tolerance in pixels before starting area selection
/// Matches C++ Mouse.cpp m_dragTolerance
pub const DRAG_TOLERANCE: i32 = 5;

/// Drag tolerance in 3D units
/// Matches C++ Mouse.cpp m_dragTolerance3D
pub const DRAG_TOLERANCE_3D: f32 = 5.0;

/// World-space selection radius for click picking
const PICK_RADIUS_WORLD: f32 = 12.0;

/// Drag tolerance in milliseconds
/// Matches C++ Mouse.cpp m_dragToleranceMS
pub const DRAG_TOLERANCE_MS: u64 = 250;

/// Double-click time window (milliseconds)
/// Matches C++ SelectionXlat.cpp double-tap logic
pub const DOUBLE_CLICK_TIME_MS: u64 = 500;

fn is_alternate_mouse_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false)
}

fn selection_window_chain_blocks_world_input(mut window: Option<Rc<RefCell<GameWindow>>>) -> bool {
    while let Some(current) = window {
        let guard = current.borrow();
        if !guard.get_status().contains(WindowStatus::SEE_THRU) {
            return true;
        }
        window = guard.get_parent();
    }

    false
}

fn world_position_is_under_opaque_window(position: &Coord3D) -> bool {
    let point = Point3::new(position.x, position.y, position.z);
    let Some(screen) = with_tactical_view_ref(|view| view.world_to_screen(&point)) else {
        return false;
    };

    with_window_manager_ref(|manager| {
        selection_window_chain_blocks_world_input(
            manager.get_window_under_cursor(screen.x, screen.y, false),
        )
    })
}

/// Drawable selection state
/// Matches C++ Drawable selection flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    NotSelected,
    Selected,
    Highlighted,
}

/// Drawable information for selection
/// Mirrors C++ Drawable class minimal info needed for selection
#[derive(Debug, Clone)]
pub struct SelectableDrawable {
    pub id: DrawableID,
    pub object_id: ObjectID,
    pub position: Coord3D,
    pub is_structure: bool,
    pub is_garrisonable_building: bool,
    pub is_crate: bool,
    pub is_selectable: bool,
    pub is_dead: bool,
    pub is_hidden: bool,
    pub is_local_controlled: bool,
    pub kind_of_flags: u32,
    pub status_bits: u32,
}

impl SelectableDrawable {
    /// Check if this drawable can be selected
    /// Port of C++ CanSelectDrawable() from SelectionXlat.cpp:104-191
    pub fn can_select(&self, drag_selecting: bool) -> bool {
        // Can't select if no object
        if self.object_id == 0 {
            return false;
        }

        // Don't select dead/dying units unless KINDOF_ALWAYS_SELECTABLE
        // Matches C++ SelectionXlat.cpp:113-117
        if self.is_dead && !self.has_kindof(KINDOF_ALWAYS_SELECTABLE) {
            return false;
        }

        // Added support for attacking cargo planes without being able to select them
        // Matches C++ SelectionXlat.cpp:119-127
        if !self.has_kindof(KINDOF_SELECTABLE) && self.has_kindof(KINDOF_FORCEATTACKABLE) {
            return false;
        }

        // Hidden objects cannot be selected
        // Matches C++ SelectionXlat.cpp:129-132
        if self.is_hidden {
            return false;
        }

        // Ignore objects obscured by opaque GUI windows
        // Matches C++ SelectionXlat.cpp:134-153
        if world_position_is_under_opaque_window(&self.position) {
            return false;
        }

        // Structures cannot be selected by drag select
        // Matches C++ SelectionXlat.cpp:156-169
        if drag_selecting && self.is_structure {
            return false;
        }

        // Cannot select if OBJECT_STATUS_UNSELECTABLE or OBJECT_STATUS_MASKED
        // Matches C++ SelectionXlat.cpp:171-175
        if self.has_status(OBJECT_STATUS_UNSELECTABLE) || self.has_status(OBJECT_STATUS_MASKED) {
            return false;
        }

        // Additional isSelectable() check
        // Matches C++ SelectionXlat.cpp:177-180
        if !self.is_selectable {
            return false;
        }

        // Drag selecting only works for locally controlled units
        // Matches C++ SelectionXlat.cpp:182-186
        if drag_selecting && !self.is_local_controlled {
            return false;
        }

        // Now we can select anything that is selectable
        // Matches C++ SelectionXlat.cpp:188-189
        true
    }

    /// Check if drawable has a specific KINDOF flag
    fn has_kindof(&self, flag: u32) -> bool {
        (self.kind_of_flags & flag) != 0
    }

    /// Check if drawable has a specific status bit
    fn has_status(&self, status: u32) -> bool {
        (self.status_bits & status) != 0
    }

    /// Check if drawable is mass selectable (for double-click selection)
    /// Matches C++ Drawable::isMassSelectable()
    pub fn is_mass_selectable(&self) -> bool {
        self.has_kindof(KINDOF_INFANTRY)
            || self.has_kindof(KINDOF_VEHICLE)
            || self.has_kindof(KINDOF_AIRCRAFT)
    }
}

// KINDOF flags from C++ ThingTemplate.h
pub const KINDOF_SELECTABLE: u32 = 0x00000001;
pub const KINDOF_FORCEATTACKABLE: u32 = 0x00000002;
pub const KINDOF_ALWAYS_SELECTABLE: u32 = 0x00000004;
pub const KINDOF_STRUCTURE: u32 = 0x00000008;
pub const KINDOF_INFANTRY: u32 = 0x00000010;
pub const KINDOF_VEHICLE: u32 = 0x00000020;
pub const KINDOF_AIRCRAFT: u32 = 0x00000040;

// Object status bits from C++ Object.h
pub const OBJECT_STATUS_UNSELECTABLE: u32 = 0x00000001;
pub const OBJECT_STATUS_MASKED: u32 = 0x00000002;

/// Click tracking for double-click detection
/// Matches C++ SelectionXlat.cpp click tracking
#[derive(Debug, Clone)]
struct ClickInfo {
    position: ICoord2D,
    timestamp: Instant,
    button: MouseButton,
}

/// Mouse button enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Selection count by template (for selection feedback)
/// Matches C++ SelectionXlat.cpp SelectCountMap
type SelectCountMap = HashMap<String, usize>;

/// Selection Translator - Port of C++ SelectionTranslator
/// Original: GeneralsMD/Code/GameEngine/Include/GameClient/SelectionXlat.h:17-58
pub struct SelectionTranslator {
    // Mouse state tracking
    // Matches C++ SelectionXlat.h:24-25
    left_mouse_button_is_down: bool,
    drag_selecting: bool,

    // Group selection tracking
    // Matches C++ SelectionXlat.h:26-27
    last_group_sel_time: Instant,
    last_group_sel_group: i32,

    // Feedback anchor points
    // Matches C++ SelectionXlat.h:28-29
    select_feedback_anchor: ICoord2D,
    deselect_feedback_anchor: ICoord2D,

    // Click detection
    // Matches C++ SelectionXlat.h:31
    right_button_down_time_ms: u32,
    last_click_info: Option<ClickInfo>,

    // Camera position for right-click detection
    // Matches C++ SelectionXlat.h:35
    deselect_down_camera_position: Coord3D,

    // Selection warning state
    // Matches C++ SelectionXlat.h:30
    displayed_max_warning: bool,

    // Selection count map
    // Matches C++ SelectionXlat.h:33
    select_count_map: SelectCountMap,

    // Control groups (0-9)
    // Matches C++ player squad hotkey system
    control_groups: [Vec<ObjectID>; 10],

    // Current selection
    current_selection: HashSet<ObjectID>,

    // Drawable registry (would be provided by game client in real implementation)
    drawable_registry: HashMap<DrawableID, SelectableDrawable>,
}

impl SelectionTranslator {
    pub fn new() -> Self {
        Self {
            left_mouse_button_is_down: false,
            drag_selecting: false,
            last_group_sel_time: Instant::now(),
            last_group_sel_group: -1,
            select_feedback_anchor: ICoord2D::default(),
            deselect_feedback_anchor: ICoord2D::default(),
            right_button_down_time_ms: 0,
            last_click_info: None,
            deselect_down_camera_position: Coord3D::default(),
            displayed_max_warning: false,
            select_count_map: HashMap::new(),
            control_groups: Default::default(),
            current_selection: HashSet::new(),
            drawable_registry: HashMap::new(),
        }
    }

    /// Register a drawable for selection
    pub fn register_drawable(&mut self, drawable: SelectableDrawable) {
        self.drawable_registry.insert(drawable.id, drawable);
    }

    fn max_select_count() -> i32 {
        TheInGameUI::get_max_select_count()
    }

    fn selection_limit_reached(&self) -> bool {
        let max = Self::max_select_count();
        max > 0 && self.current_selection.len() >= max as usize
    }

    fn warn_selection_limit_once(&mut self) {
        if self.displayed_max_warning {
            return;
        }
        let max = Self::max_select_count();
        warn!("Maximum selection count ({}) reached", max);
        self.displayed_max_warning = true;
    }

    /// Collect selectable drawables from the registry if present, otherwise build from GameLogic objects.
    fn collect_drawables(&self) -> Vec<SelectableDrawable> {
        if !self.drawable_registry.is_empty() {
            return self.drawable_registry.values().cloned().collect();
        }

        let local_player_index = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(PLAYER_INDEX_INVALID);

        let mut drawables = Vec::new();
        for obj_ref in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj) = obj_ref.read() else {
                continue;
            };

            let shroud = obj.get_shrouded_status(local_player_index);
            let is_hidden = matches!(
                shroud,
                ObjectShroudStatus::Fogged
                    | ObjectShroudStatus::Shrouded
                    | ObjectShroudStatus::InvalidButPreviousValid
            );

            let status = obj.get_status_bits();
            let mut status_bits = 0u32;
            if status.contains(ObjectStatusMaskType::UNSELECTABLE) {
                status_bits |= OBJECT_STATUS_UNSELECTABLE;
            }
            if status.contains(ObjectStatusMaskType::MASKED) {
                status_bits |= OBJECT_STATUS_MASKED;
            }

            let mut kind_of_flags = 0u32;
            if obj.is_kind_of(KindOf::Selectable) {
                kind_of_flags |= KINDOF_SELECTABLE;
            }
            if obj.is_kind_of(KindOf::AlwaysSelectable) {
                kind_of_flags |= KINDOF_ALWAYS_SELECTABLE;
            }
            if obj.is_kind_of(KindOf::Structure) {
                kind_of_flags |= KINDOF_STRUCTURE;
            }
            if obj.is_kind_of(KindOf::Infantry) {
                kind_of_flags |= KINDOF_INFANTRY;
            }
            if obj.is_kind_of(KindOf::Vehicle) {
                kind_of_flags |= KINDOF_VEHICLE;
            }
            if obj.is_kind_of(KindOf::Aircraft) {
                kind_of_flags |= KINDOF_AIRCRAFT;
            }

            let pos = obj.get_position();
            let is_garrisonable_building = obj
                .get_contain()
                .and_then(|contain| contain.lock().ok().map(|guard| guard.is_garrisonable()))
                .unwrap_or(false);
            drawables.push(SelectableDrawable {
                id: obj.get_id(),
                object_id: obj.get_id(),
                position: Coord3D::new(pos.x, pos.y, pos.z),
                is_structure: obj.is_structure(),
                is_garrisonable_building,
                is_crate: obj.is_kind_of(KindOf::Crate),
                is_selectable: obj.is_selectable(),
                is_dead: obj.is_effectively_dead() || obj.is_destroyed(),
                is_hidden,
                is_local_controlled: obj.is_locally_controlled(),
                kind_of_flags,
                status_bits,
            });
        }

        drawables
    }

    fn current_selection_context_counts(&self) -> (usize, usize, usize) {
        let mut mine = 0usize;
        let mut other = 0usize;
        let mut mine_infantry = 0usize;

        for drawable in self.collect_drawables() {
            if !self.current_selection.contains(&drawable.object_id) {
                continue;
            }

            if drawable.is_local_controlled {
                mine += 1;
                if drawable.has_kindof(KINDOF_INFANTRY) {
                    mine_infantry += 1;
                }
            } else {
                other += 1;
            }
        }

        (mine, other, mine_infantry)
    }

    fn should_short_circuit_selection_for_context_command(
        &self,
        clicked: &SelectableDrawable,
        selection_is_point: bool,
    ) -> bool {
        if TheInGameUI::is_in_force_attack_mode()
            || TheInGameUI::is_in_force_move_to_mode()
            || is_alternate_mouse_enabled()
        {
            return false;
        }

        let (current_mine, current_other, current_mine_infantry) =
            self.current_selection_context_counts();
        if current_other > 0 || current_mine == 0 {
            return false;
        }

        if clicked.is_garrisonable_building {
            return current_mine_infantry > 0 || selection_is_point;
        }

        if clicked.is_crate && selection_is_point {
            return true;
        }

        if clicked.is_local_controlled {
            return selection_is_point && !TheInGameUI::is_in_prefer_selection_mode();
        }

        selection_is_point
    }

    fn screen_to_world(&self, screen: &ICoord2D) -> Option<Coord3D> {
        let screen_pt = IPoint2::new(screen.x, screen.y);
        with_tactical_view_ref(|view| {
            view.screen_to_world(&screen_pt)
                .ok()
                .map(|pt| Coord3D::new(pt.x, pt.y, pt.z))
        })
    }

    fn world_to_screen(&self, world: &Coord3D) -> Option<(f32, f32)> {
        let point = Point3::new(world.x, world.y, world.z);
        with_tactical_view_ref(|view| {
            view.world_to_screen(&point)
                .map(|pt| (pt.x as f32, pt.y as f32))
        })
    }

    fn current_camera_position(&self) -> Coord3D {
        with_tactical_view_ref(|view| {
            let pos = view.position();
            Coord3D::new(pos.x, pos.y, pos.z)
        })
    }

    /// Deselect all drawables
    /// Port of C++ deselectAll() from SelectionXlat.cpp:205-210
    fn deselect_all(&mut self) {
        debug!("Deselecting all units");
        self.current_selection.clear();
        self.select_count_map.clear();
        self.displayed_max_warning = false;
    }

    /// Select a single drawable without sound
    /// Port of C++ selectSingleDrawableWithoutSound() from SelectionXlat.cpp:217-235
    fn select_single_drawable_without_sound(&mut self, drawable_id: DrawableID) -> bool {
        // Deselect everything else
        self.deselect_all();

        // Select the drawable
        if let Some(drawable) = self
            .collect_drawables()
            .into_iter()
            .find(|drawable| drawable.id == drawable_id)
        {
            if drawable.can_select(false) {
                self.current_selection.insert(drawable.object_id);
                debug!("Selected single drawable: {}", drawable_id);
                return true;
            }
        }

        false
    }

    /// Check if mouse click was within drag tolerance
    /// Port of C++ Mouse::isClick() from Mouse.cpp:372-388
    fn is_click(
        &self,
        anchor: &ICoord2D,
        dest: &ICoord2D,
        previous_time: Instant,
        current_time: Instant,
    ) -> bool {
        let delta_x = (anchor.x - dest.x).abs();
        let delta_y = (anchor.y - dest.y).abs();
        let duration = current_time.duration_since(previous_time);

        // Check if mouse hasn't moved further than tolerance distance
        // or the click took less than tolerance duration
        // Matches C++ Mouse.cpp:381-386
        if delta_x > DRAG_TOLERANCE
            || delta_y > DRAG_TOLERANCE
            || duration.as_millis() > DRAG_TOLERANCE_MS as u128
        {
            return false;
        }

        true
    }

    /// Handle left mouse button down
    /// Port of C++ MSG_RAW_MOUSE_LEFT_BUTTON_DOWN from SelectionXlat.cpp:893-899
    fn handle_left_button_down(&mut self, position: ICoord2D) {
        // Cannot actually start area selection yet - have to wait for cursor to move a bit
        // Matches C++ SelectionXlat.cpp:895-897
        self.left_mouse_button_is_down = true;
        self.select_feedback_anchor = position;
    }

    /// Handle mouse position updates
    /// Port of C++ MSG_RAW_MOUSE_POSITION from SelectionXlat.cpp:383-450
    fn handle_mouse_position(&mut self, position: ICoord2D) {
        if self.left_mouse_button_is_down {
            let delta_x = (position.x - self.select_feedback_anchor.x).abs();
            let delta_y = (position.y - self.select_feedback_anchor.y).abs();

            // If mouse has moved while left button is down, begin drag selection
            // Matches C++ SelectionXlat.cpp:399-408
            if (delta_x > DRAG_TOLERANCE || delta_y > DRAG_TOLERANCE)
                && !self.drag_selecting {
                    self.drag_selecting = true;
                    TheInGameUI::set_selecting(true);
                    debug!(
                        "Started drag selection at {:?}",
                        self.select_feedback_anchor
                    );
                }

            // Create "hint" messages defining selection region under construction
            // Matches C++ SelectionXlat.cpp:410-420
            if self.drag_selecting {
                // Would create MSG_AREA_SELECTION_HINT here in full implementation
                debug!(
                    "Drag selecting: {:?} to {:?}",
                    self.select_feedback_anchor, position
                );
            }
        }
    }

    /// Handle left mouse button up (click or drag selection)
    /// Port of C++ MSG_RAW_MOUSE_LEFT_BUTTON_UP from SelectionXlat.cpp:905-950
    fn handle_left_button_up(
        &mut self,
        position: ICoord2D,
        modifiers: KeyModifiers,
    ) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        self.left_mouse_button_is_down = false;

        if self.drag_selecting {
            // Stop drag selecting
            // Matches C++ SelectionXlat.cpp:909-915
            self.drag_selecting = false;
            TheInGameUI::set_selecting(false);
            debug!("Ended drag selection");

            let region = self.build_region(&self.select_feedback_anchor, &position);
            // C++ emits MSG_AREA_SELECTION here and resolves selection in that path.
            messages.push(GameMessageType::AreaSelection(region));
        } else {
            TheInGameUI::set_selecting(false);
            // Raw left-up only resolves selection on the higher-level click path now.
            // Preserve the C++ alternate-mouse blank-click deselect behavior here, and
            // otherwise forward a click message so selection and context-command guards
            // happen in the click pipeline.
            let pending_command_active = TheInGameUI::get_pending_command().is_some();
            let pending_place_source_object_id = TheInGameUI::get_pending_place_source_object_id();
            let is_blank_click = modifiers.bits() == 0;
            let use_alternate_mouse = is_alternate_mouse_enabled();
            let prevent_deselect_for_one_click = TheInGameUI::
                get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click();

            if is_blank_click
                && !pending_command_active
                && use_alternate_mouse
                && pending_place_source_object_id == 0
            {
                if prevent_deselect_for_one_click {
                    TheInGameUI::
                        set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
                            false,
                        );
                } else if !self.current_selection.is_empty() {
                    self.deselect_all();
                    messages.push(GameMessageType::CreateSelectedGroup(true, Vec::new()));
                }
            } else {
                messages.push(GameMessageType::MouseLeftClick(
                    IRegion2D {
                        x: position.x,
                        y: position.y,
                        width: 0,
                        height: 0,
                    },
                    modifiers.bits() as u32,
                ));
            }
        }

        messages
    }

    /// Handle high-level left click selection.
    /// Mirrors C++ MSG_MOUSE_LEFT_CLICK from SelectionXlat.cpp:575-646.
    fn handle_mouse_left_click(
        &mut self,
        region: IRegion2D,
        modifiers: KeyModifiers,
    ) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        if region.width != 0 || region.height != 0 {
            return messages;
        }

        if TheInGameUI::is_quit_menu_visible() {
            return messages;
        }

        if TheInGameUI::get_pending_command().is_some() {
            return messages;
        }

        let Some(clicked) = self.find_drawable_at_position(&ICoord2D {
            x: region.x,
            y: region.y,
        }) else {
            return messages;
        };

        // C++ SelectionXlat short-circuits to context-command handling before committing a
        // selection change when the click target can drive a command for the current selection.
        if self.should_short_circuit_selection_for_context_command(&clicked, true) {
            return messages;
        }

        let add_to_group =
            modifiers.contains(KeyModifiers::SHIFT) || TheInGameUI::is_in_prefer_selection_mode();

        if add_to_group && self.current_selection.contains(&clicked.object_id) {
            self.current_selection.remove(&clicked.object_id);
            messages.push(GameMessageType::RemoveFromSelectedGroup(vec![
                clicked.object_id,
            ]));
            return messages;
        }

        if !add_to_group {
            self.deselect_all();
        }

        if !self.selection_limit_reached() && self.current_selection.insert(clicked.object_id) {
            messages.push(GameMessageType::CreateSelectedGroup(
                !add_to_group,
                vec![clicked.object_id],
            ));
        } else if self.selection_limit_reached() {
            self.warn_selection_limit_once();
        }

        messages
    }

    /// Handle right mouse button down
    /// Port of C++ MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN from SelectionXlat.cpp:953-964
    fn handle_right_button_down(
        &mut self,
        position: ICoord2D,
        time: u32,
        camera_position: Coord3D,
    ) {
        // Track position for click detection
        // Matches C++ SelectionXlat.cpp:959-961
        self.deselect_feedback_anchor = position;
        self.right_button_down_time_ms = time;
        self.deselect_down_camera_position = camera_position;
    }

    /// Handle right mouse button up
    /// Port of C++ MSG_RAW_MOUSE_RIGHT_BUTTON_UP from SelectionXlat.cpp:966-1028
    fn handle_right_button_up(
        &mut self,
        position: ICoord2D,
        time: u32,
        camera_position: Coord3D,
    ) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        let delta_x = (self.deselect_feedback_anchor.x - position.x).abs();
        let delta_y = (self.deselect_feedback_anchor.y - position.y).abs();

        // Calculate camera movement
        // Matches C++ SelectionXlat.cpp:973-974
        let camera_delta = Coord3D {
            x: camera_position.x - self.deselect_down_camera_position.x,
            y: camera_position.y - self.deselect_down_camera_position.y,
            z: camera_position.z - self.deselect_down_camera_position.z,
        };
        let camera_distance = (camera_delta.x * camera_delta.x
            + camera_delta.y * camera_delta.y
            + camera_delta.z * camera_delta.z)
            .sqrt();

        // Check if this was a click or drag
        // Matches C++ SelectionXlat.cpp:982-1000
        let mut is_click = true;

        if delta_x > DRAG_TOLERANCE || delta_y > DRAG_TOLERANCE {
            is_click = false;
        }

        if time.wrapping_sub(self.right_button_down_time_ms) as u64 > DRAG_TOLERANCE_MS {
            is_click = false;
        }

        if camera_distance > DRAG_TOLERANCE_3D {
            is_click = false;
        }

        // Right click behavior (not right drag)
        // Matches C++ SelectionXlat.cpp:1002-1025
        if is_click {
            if TheInGameUI::get_pending_command().is_some() {
                // Cancel GUI command mode and do not touch selection.
                TheInGameUI::clear_pending_command();
                TheInGameUI::set_scrolling(false);
                return messages;
            }

            let use_alternate_mouse = is_alternate_mouse_enabled();
            let pending_place_source_object_id = TheInGameUI::get_pending_place_source_object_id();

            if (!use_alternate_mouse || pending_place_source_object_id != 0)
                && !self.current_selection.is_empty() {
                    self.deselect_all();
                    messages.push(GameMessageType::CreateSelectedGroup(true, Vec::new()));
                }
        }

        messages
    }

    /// Handle double-click selection
    /// Port of C++ MSG_MOUSE_LEFT_DOUBLE_CLICK from SelectionXlat.cpp:453-522
    fn handle_double_click(
        &mut self,
        position: ICoord2D,
        modifiers: KeyModifiers,
    ) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        if TheInGameUI::is_quit_menu_visible() || TheInGameUI::get_pending_command().is_some() {
            return messages;
        }

        if TheInGameUI::is_in_force_attack_mode() {
            return messages;
        }

        // Double-click selects all units of same type
        // Matches C++ SelectionXlat.cpp:458-520

        // Find drawable at click position
        if let Some(clicked_drawable) = self.find_drawable_at_position(&position) {
            if !clicked_drawable.is_mass_selectable() {
                return messages;
            }

            // Select all matching units
            // Matches C++ SelectionXlat.cpp:488-501
            let select_across_map = modifiers.contains(KeyModifiers::ALT);
            let add_to_group = modifiers.contains(KeyModifiers::SHIFT);

            let matching = if select_across_map {
                self.collect_matching_across_map(clicked_drawable.object_id)
            } else {
                self.collect_matching_across_screen(clicked_drawable.object_id)
            };

            if matching.is_empty() {
                return messages;
            }

            if !add_to_group {
                self.deselect_all();
            }

            let mut added = Vec::new();
            for id in matching {
                if self.selection_limit_reached() {
                    self.warn_selection_limit_once();
                    break;
                }
                if self.current_selection.insert(id) {
                    added.push(id);
                }
            }

            if !added.is_empty() {
                messages.push(GameMessageType::CreateSelectedGroup(!add_to_group, added));
            }
        }

        messages
    }

    /// Handle control group creation (Ctrl+0-9)
    /// Port of C++ MSG_META_CREATE_TEAM0-9 from SelectionXlat.cpp:1031-1060
    fn handle_create_control_group(&mut self, group: u8) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        if group < 10 {
            debug!("Creating control group {}", group);

            // Assign selected items to a group
            // Matches C++ SelectionXlat.cpp:1045-1056
            let drawables = self.collect_drawables();
            let selected: Vec<_> = self
                .current_selection
                .iter()
                .copied()
                .filter(|object_id| {
                    // C++ only adds locally-controlled objects (drawables can be selected even if not
                    // locally controlled in edge cases, but squads are local-player hotkeys).
                    drawables
                        .iter()
                        .find(|d| d.object_id == *object_id)
                        .map(|d| d.is_local_controlled)
                        .unwrap_or(true)
                })
                .collect();
            self.control_groups[group as usize] = selected.clone();

            messages.push(GameMessageType::CreateTeamSlot(group));
        }

        messages
    }

    /// Handle control group selection (0-9)
    /// Port of C++ MSG_META_SELECT_TEAM0-9 from SelectionXlat.cpp:1063-1134
    fn handle_select_control_group(&mut self, group: u8) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        if group < 10 {
            debug!("Selecting control group {}", group);

            let now = Instant::now();
            let time_since_last = now.duration_since(self.last_group_sel_time);

            // Check for double-press to jump view
            // Matches C++ SelectionXlat.cpp:1086-1103
            if time_since_last.as_millis() < DOUBLE_CLICK_TIME_MS as u128
                && group as i32 == self.last_group_sel_group
            {
                debug!("Double-tap select control group {}", group);
                // Would jump camera to group location here
                // Matches C++ SelectionXlat.cpp:1100
            } else {
                // Deselect all and select group
                // Matches C++ SelectionXlat.cpp:1107-1127
                self.deselect_all();

                // Select all objects in the group
                let mut selected_ids = Vec::new();
                let group_ids = self.control_groups[group as usize].clone();
                for object_id in group_ids {
                    if self.selection_limit_reached() {
                        self.warn_selection_limit_once();
                        break;
                    }
                    if self.current_selection.insert(object_id) {
                        selected_ids.push(object_id);
                    }
                }

                if !selected_ids.is_empty() {
                    messages.push(GameMessageType::CreateSelectedGroup(true, selected_ids));
                } else {
                    messages.push(GameMessageType::CreateSelectedGroup(true, Vec::new()));
                }
                messages.push(GameMessageType::SelectTeamSlot(group));
            }

            self.last_group_sel_time = now;
            self.last_group_sel_group = group as i32;
        }

        messages
    }

    /// Handle adding a control group to current selection (Shift+0-9)
    /// Port of C++ MSG_META_ADD_TEAM0-9 from SelectionXlat.cpp:1136-1214
    fn handle_add_control_group(&mut self, group: u8) -> Vec<GameMessageType> {
        let mut messages = Vec::new();

        if group >= 10 {
            return messages;
        }

        debug!("Adding control group {} to selection", group);

        let now = Instant::now();
        if now.duration_since(self.last_group_sel_time).as_millis() < DOUBLE_CLICK_TIME_MS as u128
            && group as i32 == self.last_group_sel_group
        {
            self.last_group_sel_time = now;
            self.last_group_sel_group = group as i32;
            return messages;
        }

        // If a structure is selected, clear selection before adding (C++ exploit guard).
        let drawables = self.collect_drawables();
        let has_structure_selected = self.current_selection.iter().any(|id| {
            drawables
                .iter()
                .find(|d| d.object_id == *id)
                .is_some_and(|d| d.is_structure)
        });
        if has_structure_selected {
            self.deselect_all();
        }

        let mut added = Vec::new();
        let group_ids = self.control_groups[group as usize].clone();
        for object_id in group_ids {
            if self.selection_limit_reached() {
                self.warn_selection_limit_once();
                break;
            }
            if self.current_selection.insert(object_id) {
                added.push(object_id);
            }
        }

        if !added.is_empty() {
            messages.push(GameMessageType::CreateSelectedGroup(false, added));
        }
        messages.push(GameMessageType::AddTeamSlot(group));

        self.last_group_sel_time = now;
        self.last_group_sel_group = group as i32;

        messages
    }

    /// Handle view-only control group hotkeys (Alt+0-9 in retail bindings).
    /// Matches C++ MSG_META_VIEW_TEAM0-9 by centering the tactical view on the
    /// last live object in the hotkey squad without changing selection.
    fn handle_view_control_group(&self, group: u8) {
        if group >= 10 {
            return;
        }

        let Some(object_id) = self.control_groups[group as usize].last().copied() else {
            return;
        };

        let Some(drawable) = self
            .collect_drawables()
            .into_iter()
            .find(|d| d.object_id == object_id)
        else {
            return;
        };

        let target = Point3::new(
            drawable.position.x,
            drawable.position.y,
            drawable.position.z,
        );
        with_tactical_view(|view| view.look_at(&target));
    }

    /// Build a rectangular region from two points
    /// Matches C++ buildRegion() helper function
    fn build_region(&self, anchor: &ICoord2D, point: &ICoord2D) -> IRegion2D {
        let min_x = anchor.x.min(point.x);
        let min_y = anchor.y.min(point.y);
        let max_x = anchor.x.max(point.x);
        let max_y = anchor.y.max(point.y);

        IRegion2D {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        }
    }

    /// Find drawable at screen position.
    fn find_drawable_at_position(&self, position: &ICoord2D) -> Option<SelectableDrawable> {
        let world = self.screen_to_world(position)?;
        let max_dist_sq = PICK_RADIUS_WORLD * PICK_RADIUS_WORLD;

        self.collect_drawables()
            .into_iter()
            .filter(|d| d.can_select(false))
            .filter_map(|d| {
                let dx = d.position.x - world.x;
                let dy = d.position.y - world.y;
                let dist_sq = dx * dx + dy * dy;
                (dist_sq <= max_dist_sq).then_some((dist_sq, d))
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, d)| d)
    }

    fn collect_matching_across_map(&self, template_object_id: ObjectID) -> Vec<ObjectID> {
        let drawables = self.collect_drawables();
        let Some(reference) = drawables.iter().find(|d| d.object_id == template_object_id) else {
            return Vec::new();
        };
        let reference_kind_of_flags = reference.kind_of_flags;

        let mut matching: Vec<ObjectID> = drawables
            .into_iter()
            .filter(|d| d.can_select(false))
            .filter(|d| d.is_local_controlled)
            .filter(|d| d.is_mass_selectable())
            .filter(|d| d.kind_of_flags == reference_kind_of_flags)
            .map(|d| d.object_id)
            .collect();

        matching.sort_unstable();
        matching
    }

    fn collect_matching_across_screen(&self, template_object_id: ObjectID) -> Vec<ObjectID> {
        let (screen_w, screen_h) = with_tactical_view_ref(|view| (view.width(), view.height()));
        let drawables = self.collect_drawables();
        let Some(reference) = drawables.iter().find(|d| d.object_id == template_object_id) else {
            return Vec::new();
        };
        let reference_kind_of_flags = reference.kind_of_flags;

        let mut matching: Vec<ObjectID> = drawables
            .into_iter()
            .filter(|d| d.can_select(false))
            .filter(|d| d.is_local_controlled)
            .filter(|d| d.is_mass_selectable())
            .filter(|d| d.kind_of_flags == reference_kind_of_flags)
            .filter(|d| {
                self.world_to_screen(&d.position)
                    .map(|(sx, sy)| {
                        sx >= 0.0 && sy >= 0.0 && sx <= screen_w as f32 && sy <= screen_h as f32
                    })
                    .unwrap_or(false)
            })
            .map(|d| d.object_id)
            .collect();

        matching.sort_unstable();
        matching
    }

    fn find_drawables_in_region(
        &self,
        region: &IRegion2D,
        drag_selecting: bool,
    ) -> Vec<SelectableDrawable> {
        let min_x = region.x.min(region.x + region.width);
        let min_y = region.y.min(region.y + region.height);
        let max_x = region.x.max(region.x + region.width);
        let max_y = region.y.max(region.y + region.height);

        let mut out: Vec<SelectableDrawable> = self
            .collect_drawables()
            .into_iter()
            .filter(|d| d.can_select(drag_selecting))
            .filter(|d| {
                self.world_to_screen(&d.position)
                    .map(|(x, y)| {
                        let x = x as i32;
                        let y = y as i32;
                        x >= min_x && x <= max_x && y >= min_y && y <= max_y
                    })
                    .unwrap_or(false)
            })
            .collect();

        out.sort_by_key(|d| d.id);
        out
    }

    /// Public accessors for control bar integration
    /// Matches C++ SelectionXlat.h:48-51
    pub fn set_drag_selecting(&mut self, drag_select: bool) {
        self.drag_selecting = drag_select;
    }

    pub fn set_left_mouse_button(&mut self, state: bool) {
        self.left_mouse_button_is_down = state;
    }

    pub fn is_hand_of_god_selection_mode(&self) -> bool {
        // Debug mode for instantly killing units by clicking
        // Matches C++ SelectionXlat.h:54-55
        false
    }
}

impl Default for SelectionTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for SelectionTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let is_mouse_click_message = matches!(
            msg.get_type(),
            GameMessageType::MouseLeftClick(..) | GameMessageType::MouseLeftDoubleClick(..)
        );

        let new_messages = match msg.get_type() {
            // Mouse position tracking
            GameMessageType::RawMousePosition(pos) => {
                self.handle_mouse_position(pos.clone());
                Vec::new()
            }

            // Left mouse button
            GameMessageType::RawMouseLeftButtonDown(pos, modifiers, _time) => {
                let modifiers = KeyModifiers::from_bits_truncate((*modifiers & 0xFF) as u8);
                let _ = modifiers;
                self.handle_left_button_down(pos.clone());
                Vec::new()
            }

            GameMessageType::RawMouseLeftButtonUp(pos, modifiers, _time) => {
                let modifiers = KeyModifiers::from_bits_truncate((*modifiers & 0xFF) as u8);
                self.handle_left_button_up(pos.clone(), modifiers)
            }

            GameMessageType::MouseLeftClick(region, modifiers) => {
                let modifiers = KeyModifiers::from_bits_truncate((*modifiers & 0xFF) as u8);
                self.handle_mouse_left_click(region.clone(), modifiers)
            }

            GameMessageType::MouseLeftDoubleClick(region, modifiers) => {
                let modifiers = KeyModifiers::from_bits_truncate((*modifiers & 0xFF) as u8);
                let pos = ICoord2D {
                    x: region.x,
                    y: region.y,
                };
                self.handle_double_click(pos, modifiers)
            }

            // Right mouse button
            GameMessageType::RawMouseRightButtonDown(pos, _modifiers, time) => {
                self.handle_right_button_down(pos.clone(), *time, self.current_camera_position());
                return GameMessageDisposition::KeepMessage;
            }

            GameMessageType::RawMouseRightButtonUp(pos, _modifiers, time) => {
                let pending_before = TheInGameUI::get_pending_command().is_some();
                let new_messages =
                    self.handle_right_button_up(pos.clone(), *time, self.current_camera_position());
                for new_msg in new_messages {
                    emit_message(GameMessage::new(new_msg));
                }

                // Match C++ SelectionXlat behavior: consume raw right-up only when it cancels
                // GUI command mode; otherwise keep it so CommandXlat can evaluate click context.
                if pending_before && TheInGameUI::get_pending_command().is_none() {
                    return GameMessageDisposition::DestroyMessage;
                }
                return GameMessageDisposition::KeepMessage;
            }

            // Control group creation (Ctrl+0-9)
            GameMessageType::MetaCreateTeam(group) => self.handle_create_control_group(*group),

            // Control group selection (0-9)
            GameMessageType::MetaSelectTeam(group) => self.handle_select_control_group(*group),

            // Control group add (Shift+0-9)
            GameMessageType::MetaAddTeam(group) => self.handle_add_control_group(*group),

            GameMessageType::MetaViewTeam(group) => {
                self.handle_view_control_group(*group);
                return GameMessageDisposition::DestroyMessage;
            }

            // Match C++ SelectionXlat MSG_META_OPTIONS behavior:
            // stop left-button selection feedback state, then let CommandXlat process options.
            GameMessageType::MetaOptions => {
                self.left_mouse_button_is_down = false;
                self.drag_selecting = false;
                TheInGameUI::set_selecting(false);
                return GameMessageDisposition::KeepMessage;
            }

            // Pass through other messages
            _ => {
                return GameMessageDisposition::KeepMessage;
            }
        };

        // Dispatch translated messages into the message stream.
        // C++ SelectionXlat keeps raw mouse position/left-button messages and only consumes
        // specific meta/selection flow messages.
        let keep_raw_mouse_message = matches!(
            msg.get_type(),
            GameMessageType::RawMousePosition(_)
                | GameMessageType::RawMouseLeftButtonDown(..)
                | GameMessageType::RawMouseLeftButtonUp(..)
        );
        let should_keep_message = keep_raw_mouse_message
            || (is_mouse_click_message
                && new_messages.is_empty()
                && !TheInGameUI::is_quit_menu_visible());

        for new_msg in new_messages {
            emit_message(GameMessage::new(new_msg));
        }

        if should_keep_message {
            return GameMessageDisposition::KeepMessage;
        }

        // Raw input messages are destroyed after processing
        GameMessageDisposition::DestroyMessage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn test_state_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn test_can_select_drawable() {
        let _guard = test_state_lock();
        let drawable = SelectableDrawable {
            id: 1,
            object_id: 100,
            position: Coord3D::default(),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        };

        // Can select normally
        assert!(drawable.can_select(false));
        assert!(drawable.can_select(true));

        // Can't select dead units
        let mut dead = drawable.clone();
        dead.is_dead = true;
        assert!(!dead.can_select(false));

        // Can't drag-select structures
        let mut structure = drawable.clone();
        structure.is_structure = true;
        assert!(structure.can_select(false)); // Can click-select
        assert!(!structure.can_select(true)); // Can't drag-select

        // Can't select hidden units
        let mut hidden = drawable.clone();
        hidden.is_hidden = true;
        assert!(!hidden.can_select(false));
    }

    #[test]
    fn opaque_window_chain_blocks_selection_like_cpp() {
        let _guard = test_state_lock();
        let opaque = Rc::new(RefCell::new(GameWindow::new()));
        opaque
            .borrow_mut()
            .set_status_exact(WindowStatus::ENABLED | WindowStatus::ACTIVE);

        assert!(selection_window_chain_blocks_world_input(Some(opaque)));
    }

    #[test]
    fn see_thru_window_chain_allows_selection_like_cpp() {
        let _guard = test_state_lock();
        let see_thru = Rc::new(RefCell::new(GameWindow::new()));
        see_thru.borrow_mut().set_status_exact(
            WindowStatus::ENABLED | WindowStatus::ACTIVE | WindowStatus::SEE_THRU,
        );

        assert!(!selection_window_chain_blocks_world_input(Some(see_thru)));
    }

    #[test]
    fn see_thru_child_with_opaque_parent_blocks_selection_like_cpp() {
        let _guard = test_state_lock();
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        parent
            .borrow_mut()
            .set_status_exact(WindowStatus::ENABLED | WindowStatus::ACTIVE);
        let child = Rc::new(RefCell::new(GameWindow::new()));
        child.borrow_mut().set_status_exact(
            WindowStatus::ENABLED | WindowStatus::ACTIVE | WindowStatus::SEE_THRU,
        );
        child.borrow_mut().set_parent(Some(&parent));

        assert!(selection_window_chain_blocks_world_input(Some(child)));
    }

    #[test]
    fn test_context_short_circuit_prefers_command_over_enemy_selection() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        translator.register_drawable(SelectableDrawable {
            id: 1,
            object_id: 101,
            position: Coord3D::default(),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        });
        translator.register_drawable(SelectableDrawable {
            id: 2,
            object_id: 202,
            position: Coord3D::default(),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: false,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        });
        translator.current_selection.insert(101);

        let clicked = translator
            .drawable_registry
            .get(&2)
            .cloned()
            .expect("test drawable must exist");

        assert!(
            translator.should_short_circuit_selection_for_context_command(&clicked, true),
            "point clicks on non-local targets should stay in the command pipeline"
        );
    }

    #[test]
    fn test_context_short_circuit_respects_prefer_selection_and_empty_selection() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        let clicked = SelectableDrawable {
            id: 1,
            object_id: 303,
            position: Coord3D::default(),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        };
        translator.register_drawable(clicked.clone());

        assert!(
            !translator.should_short_circuit_selection_for_context_command(&clicked, true),
            "empty selection should never short-circuit"
        );

        translator.current_selection.insert(303);
        TheInGameUI::set_prefer_selection_mode(true);
        assert!(
            !translator.should_short_circuit_selection_for_context_command(&clicked, true),
            "prefer-selection mode should keep the click in selection flow"
        );
        TheInGameUI::set_prefer_selection_mode(false);
    }

    #[test]
    fn test_context_short_circuit_allows_point_click_on_already_selected_local_unit() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        let clicked = SelectableDrawable {
            id: 1,
            object_id: 404,
            position: Coord3D::default(),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        };
        translator.register_drawable(clicked.clone());
        translator.current_selection.insert(404);

        assert!(
            translator.should_short_circuit_selection_for_context_command(&clicked, true),
            "C++ contextCommandForNewSelection still short-circuits when clicking an already selected local unit"
        );
    }

    #[test]
    fn test_context_short_circuit_allows_garrisonable_drag_when_infantry_selected() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        translator.register_drawable(SelectableDrawable {
            id: 10,
            object_id: 500,
            position: Coord3D::default(),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        });
        let clicked = SelectableDrawable {
            id: 11,
            object_id: 501,
            position: Coord3D::default(),
            is_structure: true,
            is_garrisonable_building: true,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: false,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_STRUCTURE,
            status_bits: 0,
        };
        translator.register_drawable(clicked.clone());
        translator.current_selection.insert(500);

        assert!(
            translator.should_short_circuit_selection_for_context_command(&clicked, false),
            "infantry selecting a garrisonable building should short-circuit even for non-point selection"
        );
    }

    #[test]
    fn test_selection_translator_creation() {
        let _guard = test_state_lock();
        let translator = SelectionTranslator::new();
        assert!(!translator.left_mouse_button_is_down);
        assert!(!translator.drag_selecting);
        assert_eq!(translator.current_selection.len(), 0);
    }

    #[test]
    fn test_drag_selection_start() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        // Simulate mouse down
        let down_pos = ICoord2D { x: 100, y: 100 };
        translator.handle_left_button_down(down_pos);

        assert!(translator.left_mouse_button_is_down);
        assert!(!translator.drag_selecting); // Not dragging yet

        // Move mouse beyond drag tolerance
        let drag_pos = ICoord2D { x: 120, y: 120 };
        translator.handle_mouse_position(drag_pos);

        assert!(translator.drag_selecting); // Now dragging
    }

    #[test]
    fn test_raw_left_button_up_forwards_click_when_not_dragging() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        let messages =
            translator.handle_left_button_up(ICoord2D { x: 12, y: 34 }, KeyModifiers::empty());

        assert_eq!(messages.len(), 1);
        match &messages[0] {
            GameMessageType::MouseLeftClick(region, modifiers) => {
                assert_eq!(region.x, 12);
                assert_eq!(region.y, 34);
                assert_eq!(region.width, 0);
                assert_eq!(region.height, 0);
                assert_eq!(*modifiers, 0);
            }
            other => panic!("expected MouseLeftClick, got {other:?}"),
        }
    }

    #[test]
    fn test_raw_left_button_up_honors_one_click_deselect_prevention_in_alternate_mouse_mode() {
        let _guard = test_state_lock();
        game_engine::common::ini::ini_game_data::init_global_data();
        let previous_alt_mouse = game_engine::common::ini::ini_game_data::get_global_data()
            .map(|data| data.read().use_alternate_mouse)
            .unwrap_or(false);
        if let Some(data) = game_engine::common::ini::ini_game_data::get_global_data() {
            data.write().use_alternate_mouse = true;
        }

        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(true);
        let mut translator = SelectionTranslator::new();
        translator.current_selection.insert(777);

        let messages =
            translator.handle_left_button_up(ICoord2D { x: 16, y: 24 }, KeyModifiers::empty());

        assert!(messages.is_empty());
        assert_eq!(translator.current_selection.len(), 1);
        assert!(
            !TheInGameUI::get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            )
        );

        if let Some(data) = game_engine::common::ini::ini_game_data::get_global_data() {
            data.write().use_alternate_mouse = previous_alt_mouse;
        }
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
    }

    #[test]
    fn test_double_click_selection_keeps_message_during_force_attack() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();
        translator.current_selection.insert(44);
        TheInGameUI::set_force_attack_mode(true);

        let disposition = translator.translate_game_message(&GameMessage::new(
            GameMessageType::MouseLeftDoubleClick(
                IRegion2D {
                    x: 20,
                    y: 30,
                    width: 0,
                    height: 0,
                },
                0,
            ),
        ));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert!(translator.current_selection.contains(&44));
        TheInGameUI::set_force_attack_mode(false);
    }

    #[test]
    fn test_control_group_management() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        // Add some units to selection
        translator.current_selection.insert(100);
        translator.current_selection.insert(101);
        translator.current_selection.insert(102);

        // Create control group 1
        translator.handle_create_control_group(1);

        assert_eq!(translator.control_groups[1].len(), 3);

        // Clear selection
        translator.deselect_all();
        assert_eq!(translator.current_selection.len(), 0);

        // Select control group 1
        translator.handle_select_control_group(1);

        assert_eq!(translator.current_selection.len(), 3);
        assert_eq!(translator.last_group_sel_group, 1);
    }

    #[test]
    fn test_add_control_group_double_tap_does_not_append_again() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();
        TheInGameUI::set_max_select_count(-1);
        translator.control_groups[1] = vec![10, 11];
        translator.current_selection.insert(99);

        let first = translator.handle_add_control_group(1);
        assert_eq!(translator.current_selection.len(), 3);
        assert_eq!(
            first,
            vec![
                GameMessageType::CreateSelectedGroup(false, vec![10, 11]),
                GameMessageType::AddTeamSlot(1)
            ]
        );

        let second = translator.handle_add_control_group(1);
        assert!(second.is_empty());
        assert_eq!(translator.current_selection.len(), 3);
        TheInGameUI::set_max_select_count(-1);
    }

    #[test]
    fn selection_limit_disabled_allows_more_than_forty_like_cpp() {
        let _guard = test_state_lock();
        let original_max = TheInGameUI::get_max_select_count();
        TheInGameUI::set_max_select_count(0);

        let mut translator = SelectionTranslator::new();
        translator.control_groups[1] = (1..=45).collect();
        let messages = translator.handle_select_control_group(1);

        assert_eq!(translator.current_selection.len(), 45);
        assert!(!translator.displayed_max_warning);
        assert!(messages.iter().any(|message| matches!(
            message,
            GameMessageType::CreateSelectedGroup(true, ids) if ids.len() == 45
        )));

        TheInGameUI::set_max_select_count(original_max);
    }

    #[test]
    fn selection_limit_positive_caps_control_group_like_cpp() {
        let _guard = test_state_lock();
        let original_max = TheInGameUI::get_max_select_count();
        TheInGameUI::set_max_select_count(40);

        let mut translator = SelectionTranslator::new();
        translator.control_groups[1] = (1..=45).collect();
        let messages = translator.handle_select_control_group(1);

        assert_eq!(translator.current_selection.len(), 40);
        assert!(translator.displayed_max_warning);
        assert!(messages.iter().any(|message| matches!(
            message,
            GameMessageType::CreateSelectedGroup(true, ids) if ids.len() == 40
        )));

        TheInGameUI::set_max_select_count(original_max);
    }

    #[test]
    fn test_view_control_group_centers_on_last_group_object_without_selecting() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();
        translator.control_groups[2] = vec![10, 11];
        translator.current_selection.insert(99);
        translator.register_drawable(SelectableDrawable {
            id: 10,
            object_id: 10,
            position: Coord3D::new(100.0, 200.0, 0.0),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        });
        translator.register_drawable(SelectableDrawable {
            id: 11,
            object_id: 11,
            position: Coord3D::new(300.0, 420.0, 0.0),
            is_structure: false,
            is_garrisonable_building: false,
            is_crate: false,
            is_selectable: true,
            is_dead: false,
            is_hidden: false,
            is_local_controlled: true,
            kind_of_flags: KINDOF_SELECTABLE | KINDOF_INFANTRY,
            status_bits: 0,
        });

        with_tactical_view(|view| {
            view.set_width(100);
            view.set_height(80);
            view.set_position(&Point3::new(0.0, 0.0, 0.0));
        });

        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaViewTeam(2)));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        assert_eq!(translator.current_selection, HashSet::from([99]));
        with_tactical_view_ref(|view| {
            assert_eq!(view.position().x, 250.0);
            assert_eq!(view.position().y, 380.0);
        });
    }

    #[test]
    fn test_is_click_tolerance() {
        let _guard = test_state_lock();
        let translator = SelectionTranslator::new();

        let anchor = ICoord2D { x: 100, y: 100 };
        let start_time = Instant::now();

        // Within tolerance
        let near = ICoord2D { x: 102, y: 102 };
        assert!(translator.is_click(&anchor, &near, start_time, start_time));

        // Outside distance tolerance
        let far = ICoord2D { x: 120, y: 120 };
        assert!(!translator.is_click(&anchor, &far, start_time, start_time));

        // Would test time tolerance but can't easily mock time in test
    }

    #[test]
    fn test_build_region() {
        let _guard = test_state_lock();
        let translator = SelectionTranslator::new();

        let p1 = ICoord2D { x: 100, y: 100 };
        let p2 = ICoord2D { x: 200, y: 150 };

        let region = translator.build_region(&p1, &p2);

        assert_eq!(region.x, 100);
        assert_eq!(region.y, 100);
        assert_eq!(region.width, 100);
        assert_eq!(region.height, 50);

        // Test reverse order
        let region2 = translator.build_region(&p2, &p1);
        assert_eq!(region, region2);
    }

    #[test]
    fn test_right_click_cancels_pending_command_without_clearing_selection() {
        let _guard = test_state_lock();
        use crate::helpers::TheInGameUI;
        use gamelogic::commands::command::CommandType;

        let mut translator = SelectionTranslator::new();
        translator.current_selection.insert(7);
        TheInGameUI::set_pending_command(CommandType::PlaceBeacon, 0x20, 99);
        TheInGameUI::set_scrolling(true);

        translator.handle_right_button_down(ICoord2D { x: 50, y: 60 }, 0, Coord3D::default());
        let messages =
            translator.handle_right_button_up(ICoord2D { x: 50, y: 60 }, 0, Coord3D::default());

        assert!(messages.is_empty());
        assert_eq!(translator.current_selection.len(), 1);
        assert!(TheInGameUI::get_pending_command().is_none());
        assert!(!TheInGameUI::is_scrolling());
    }

    #[test]
    fn test_right_click_slow_release_does_not_cancel_pending_command() {
        let _guard = test_state_lock();
        use crate::helpers::TheInGameUI;
        use gamelogic::commands::command::CommandType;

        let mut translator = SelectionTranslator::new();
        translator.current_selection.insert(7);
        TheInGameUI::set_pending_command(CommandType::PlaceBeacon, 0x20, 99);

        translator.handle_right_button_down(ICoord2D { x: 50, y: 60 }, 10, Coord3D::default());
        let messages = translator.handle_right_button_up(
            ICoord2D { x: 50, y: 60 },
            10 + DRAG_TOLERANCE_MS as u32 + 1,
            Coord3D::default(),
        );

        assert!(messages.is_empty());
        assert_eq!(translator.current_selection.len(), 1);
        assert!(TheInGameUI::get_pending_command().is_some());
        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_mouse_left_click_quit_menu_guard() {
        let _guard = test_state_lock();
        use crate::helpers::TheInGameUI;

        let mut translator = SelectionTranslator::new();
        TheInGameUI::set_quit_menu_visible(true);

        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MouseLeftClick(
                IRegion2D {
                    x: 20,
                    y: 30,
                    width: 0,
                    height: 0,
                },
                0,
            )));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        TheInGameUI::set_quit_menu_visible(false);
    }

    #[test]
    fn test_meta_options_clears_left_selection_feedback_state() {
        let _guard = test_state_lock();
        use crate::helpers::TheInGameUI;

        let mut translator = SelectionTranslator::new();
        translator.set_left_mouse_button(true);
        translator.set_drag_selecting(true);
        TheInGameUI::set_selecting(true);

        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaOptions));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert!(!translator.left_mouse_button_is_down);
        assert!(!translator.drag_selecting);
        assert!(!TheInGameUI::is_selecting());
    }
}
