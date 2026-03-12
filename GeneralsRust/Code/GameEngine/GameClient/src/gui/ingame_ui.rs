//! # In-Game UI System
//!
//! Comprehensive in-game user interface system ported from C++ InGameUI.cpp
//! Handles all in-game UI elements including selection, minimap, resource display,
//! and building placement preview.
//!
//! Original C++ file: GameClient/InGameUI.cpp
//! Original Author: Michael S. Booth, March 2001

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use glam::{Vec2, Vec3};
use thiserror::Error;
use wgpu::TextureView;

use super::ui_renderer::{UIRect, UIRenderer, UIRendererError};
use crate::display::view::{with_tactical_view, with_tactical_view_ref, IPoint2, Point3};
use crate::helpers::TheInGameUI;
use crate::input::keyboard::KeyboardState;
use crate::input::mouse::{ButtonState, MouseButton, MouseState};
use crate::message_stream::game_message::{
    Coord3D as MsgCoord3D, GameMessageType, ICoord2D as MsgICoord2D,
};
use crate::message_stream::message_stream::append_message_to_stream;
use gamelogic::commands::selection::{get_selection_manager, SelectionType};
use gamelogic::common::types::Relationship;
use gamelogic::common::{Coord3D, ICoord2D, IRegion2D, KindOf, ObjectID};
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::object::production::construction::FoundationValidator;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
use gamelogic::player::{PlayerType, ThePlayerList};

/// In-game UI errors
#[derive(Error, Debug)]
pub enum InGameUIError {
    #[error("Renderer error: {0}")]
    RendererError(#[from] UIRendererError),
    #[error("Invalid selection: {0}")]
    InvalidSelection(String),
    #[error("Invalid object ID: {0}")]
    InvalidObjectID(u32),
    #[error("System error: {0}")]
    SystemError(String),
}

type Result<T> = std::result::Result<T, InGameUIError>;

/// Placement opacity for building preview (C++ InGameUI.cpp:77)
const PLACEMENT_OPACITY: f32 = 0.45;

/// Illegal build color - red (C++ InGameUI.cpp:78)
const ILLEGAL_BUILD_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

/// Legal build color - green
const LEGAL_BUILD_COLOR: [f32; 3] = [0.0, 1.0, 0.0];

/// Maximum selection count
const MAX_SELECTION_COUNT: usize = 200;

/// Double-click time threshold (milliseconds)
const DOUBLE_CLICK_TIME_MS: u64 = 500;

/// Minimum drag distance for selection box (pixels)
const MIN_DRAG_DISTANCE: f32 = 5.0;

/// Minimum drag distance for line build placement (pixels)
const PLACEMENT_DRAG_DISTANCE: f32 = 5.0;

/// Selection box representation
#[derive(Debug, Clone, Copy)]
pub struct SelectionBox {
    /// Starting position (screen coordinates)
    pub start: Vec2,
    /// Current position (screen coordinates)
    pub current: Vec2,
    /// Whether the selection box is active
    pub active: bool,
}

impl SelectionBox {
    pub fn new() -> Self {
        Self {
            start: Vec2::ZERO,
            current: Vec2::ZERO,
            active: false,
        }
    }

    pub fn start_at(&mut self, pos: Vec2) {
        self.start = pos;
        self.current = pos;
        self.active = true;
    }

    pub fn update(&mut self, pos: Vec2) {
        self.current = pos;
    }

    pub fn finish(&mut self) {
        self.active = false;
    }

    pub fn get_rect(&self) -> UIRect {
        let min_x = self.start.x.min(self.current.x);
        let min_y = self.start.y.min(self.current.y);
        let max_x = self.start.x.max(self.current.x);
        let max_y = self.start.y.max(self.current.y);

        UIRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn is_significant(&self) -> bool {
        let dx = self.current.x - self.start.x;
        let dy = self.current.y - self.start.y;
        (dx * dx + dy * dy).sqrt() > MIN_DRAG_DISTANCE
    }
}

/// Drawable object reference (simplified for now)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawableID(pub u32);

/// Object selection state
#[derive(Debug)]
pub struct SelectionState {
    /// Currently selected objects
    selected: Vec<DrawableID>,
    /// Maximum allowed selection count
    max_selection: usize,
    /// Last click time for double-click detection
    last_click_time: Option<Instant>,
    /// Last click position
    last_click_pos: Option<Vec2>,
    /// Selection groups (0-9)
    selection_groups: [Vec<DrawableID>; 10],
}

impl SelectionState {
    pub fn new(max_selection: usize) -> Self {
        Self {
            selected: Vec::new(),
            max_selection,
            last_click_time: None,
            last_click_pos: None,
            selection_groups: Default::default(),
        }
    }

    pub fn select(&mut self, drawable_id: DrawableID, add_to_selection: bool) {
        if !add_to_selection {
            self.selected.clear();
        }

        if !self.selected.contains(&drawable_id) && self.selected.len() < self.max_selection {
            self.selected.push(drawable_id);
        }
    }

    pub fn deselect(&mut self, drawable_id: DrawableID) {
        self.selected.retain(|&id| id != drawable_id);
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn is_selected(&self, drawable_id: DrawableID) -> bool {
        self.selected.contains(&drawable_id)
    }

    pub fn get_selected(&self) -> &[DrawableID] {
        &self.selected
    }

    pub fn count(&self) -> usize {
        self.selected.len()
    }

    pub fn set_group(&mut self, group: usize, selection: Vec<DrawableID>) {
        if group < 10 {
            self.selection_groups[group] = selection;
        }
    }

    pub fn get_group(&self, group: usize) -> Option<&[DrawableID]> {
        if group < 10 {
            Some(&self.selection_groups[group])
        } else {
            None
        }
    }

    pub fn detect_double_click(&mut self, pos: Vec2) -> bool {
        let now = Instant::now();
        let is_double = if let (Some(last_time), Some(last_pos)) =
            (self.last_click_time, self.last_click_pos)
        {
            let time_ok = now.duration_since(last_time).as_millis() < DOUBLE_CLICK_TIME_MS as u128;
            let dist = (pos - last_pos).length();
            time_ok && dist < 10.0
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_pos = Some(pos);

        is_double
    }
}

/// Building placement preview state
#[derive(Debug, Clone)]
pub struct PlacementPreview {
    /// Building template name
    pub template_name: String,
    /// World position
    pub position: Vec3,
    /// Rotation angle (radians)
    pub rotation: f32,
    /// Whether placement is legal at current position
    pub is_legal: bool,
    /// Building footprint size
    pub footprint: Vec2,
    /// Preview mesh/texture
    pub preview_texture: Option<String>,
}

impl PlacementPreview {
    pub fn new(template_name: String, footprint: Vec2) -> Self {
        Self {
            template_name,
            position: Vec3::ZERO,
            rotation: 0.0,
            is_legal: false,
            footprint,
            preview_texture: None,
        }
    }

    pub fn update_position(&mut self, position: Vec3, is_legal: bool) {
        self.position = position;
        self.is_legal = is_legal;
    }

    pub fn rotate(&mut self, delta: f32) {
        self.rotation = (self.rotation + delta) % (2.0 * std::f32::consts::PI);
    }

    pub fn get_color(&self) -> [f32; 4] {
        if self.is_legal {
            [
                LEGAL_BUILD_COLOR[0],
                LEGAL_BUILD_COLOR[1],
                LEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        } else {
            [
                ILLEGAL_BUILD_COLOR[0],
                ILLEGAL_BUILD_COLOR[1],
                ILLEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        }
    }
}

/// Minimap state and rendering
#[derive(Debug)]
pub struct Minimap {
    /// Position on screen (bottom-left corner)
    pub position: Vec2,
    /// Size in pixels
    pub size: Vec2,
    /// World bounds represented by minimap
    pub world_bounds: (Vec2, Vec2), // (min, max)
    /// Current camera position in world
    pub camera_position: Vec3,
    /// Camera viewport size
    pub camera_viewport: Vec2,
    /// Minimap texture
    pub texture: Option<Arc<TextureView>>,
    /// Whether minimap is visible
    pub visible: bool,
    /// Unit icons on minimap
    pub unit_icons: HashMap<DrawableID, MinimapIcon>,
}

#[derive(Debug, Clone)]
pub struct MinimapIcon {
    pub position: Vec2,
    pub color: [f32; 4],
    pub size: f32,
}

impl Minimap {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self {
            position,
            size,
            world_bounds: (Vec2::ZERO, Vec2::new(1000.0, 1000.0)),
            camera_position: Vec3::ZERO,
            camera_viewport: Vec2::new(800.0, 600.0),
            texture: None,
            visible: true,
            unit_icons: HashMap::new(),
        }
    }

    pub fn world_to_minimap(&self, world_pos: Vec2) -> Vec2 {
        let (min, max) = self.world_bounds;
        let normalized = (world_pos - min) / (max - min);
        self.position + normalized * self.size
    }

    pub fn minimap_to_world(&self, minimap_pos: Vec2) -> Vec2 {
        let (min, max) = self.world_bounds;
        let normalized = (minimap_pos - self.position) / self.size;
        min + normalized * (max - min)
    }

    pub fn contains_point(&self, screen_pos: Vec2) -> bool {
        let rect = UIRect::new(self.position.x, self.position.y, self.size.x, self.size.y);
        rect.contains(screen_pos.x, screen_pos.y)
    }

    pub fn update_icon(&mut self, id: DrawableID, world_pos: Vec2, color: [f32; 4]) {
        let minimap_pos = self.world_to_minimap(world_pos);
        self.unit_icons.insert(
            id,
            MinimapIcon {
                position: minimap_pos,
                color,
                size: 2.0,
            },
        );
    }

    pub fn remove_icon(&mut self, id: DrawableID) {
        self.unit_icons.remove(&id);
    }
}

/// Resource display HUD
#[derive(Debug, Clone)]
pub struct ResourceDisplay {
    /// Money/credits
    pub credits: i32,
    /// Power available
    pub power_available: i32,
    /// Power used
    pub power_used: i32,
    /// Display position
    pub position: Vec2,
    /// Whether to show detailed info
    pub show_details: bool,
}

impl ResourceDisplay {
    pub fn new(position: Vec2) -> Self {
        Self {
            credits: 0,
            power_available: 0,
            power_used: 0,
            position,
            show_details: true,
        }
    }

    pub fn update(&mut self, credits: i32, power_available: i32, power_used: i32) {
        self.credits = credits;
        self.power_available = power_available;
        self.power_used = power_used;
    }

    pub fn get_power_percentage(&self) -> f32 {
        if self.power_available > 0 {
            (self.power_used as f32 / self.power_available as f32).min(1.0)
        } else {
            0.0
        }
    }

    pub fn is_power_deficit(&self) -> bool {
        self.power_used > self.power_available
    }
}

/// Main in-game UI manager
pub struct InGameUI {
    /// Selection box state
    selection_box: SelectionBox,

    /// Selection state
    selection_state: SelectionState,

    /// Current placement preview (if any)
    placement_preview: Option<PlacementPreview>,

    /// Minimap
    minimap: Minimap,

    /// Resource display
    resource_display: ResourceDisplay,

    /// UI renderer
    renderer: Arc<RwLock<UIRenderer>>,

    /// Screen dimensions
    screen_size: Vec2,

    /// Whether UI is enabled
    enabled: bool,

    /// Current player id (local player)
    player_id: u32,

    /// Accumulated UI time (seconds)
    ui_time: f32,

    /// Last update time
    last_update: Instant,
}

impl InGameUI {
    pub fn new(renderer: Arc<RwLock<UIRenderer>>, screen_width: f32, screen_height: f32) -> Self {
        let minimap_size = 200.0;
        let minimap_margin = 10.0;

        Self {
            selection_box: SelectionBox::new(),
            selection_state: SelectionState::new(MAX_SELECTION_COUNT),
            placement_preview: None,
            minimap: Minimap::new(
                Vec2::new(
                    screen_width - minimap_size - minimap_margin,
                    screen_height - minimap_size - minimap_margin,
                ),
                Vec2::new(minimap_size, minimap_size),
            ),
            resource_display: ResourceDisplay::new(Vec2::new(10.0, 10.0)),
            renderer,
            screen_size: Vec2::new(screen_width, screen_height),
            enabled: true,
            player_id: 0,
            ui_time: 0.0,
            last_update: Instant::now(),
        }
    }

    /// Handle mouse input for selection box
    pub fn handle_mouse_input(
        &mut self,
        mouse: &MouseState,
        keyboard: &KeyboardState,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mouse_pos = Vec2::new(mouse.position().0, mouse.position().1);
        let left_button = mouse.button_state(MouseButton::Left);
        let right_button = mouse.button_state(MouseButton::Right);
        let add_to_selection = keyboard.is_ctrl_pressed() || keyboard.is_shift_pressed();

        // Check if clicking on minimap
        if self.minimap.contains_point(mouse_pos) {
            if left_button.just_pressed() {
                // Click on minimap - move camera
                let world_pos = self.minimap.minimap_to_world(mouse_pos);
                log::debug!("Minimap click at world position: {:?}", world_pos);
                with_tactical_view(|view| {
                    view.look_at(&Point3::new(world_pos.x, world_pos.y, 0.0));
                });
            }
            return Ok(());
        }

        if self.handle_pending_special_power(mouse_pos, left_button, right_button)? {
            return Ok(());
        }

        // Handle selection box
        match left_button {
            ButtonState::JustPressed => {
                // Start selection box
                self.selection_box.start_at(mouse_pos);

                // Check for double-click
                if self.selection_state.detect_double_click(mouse_pos) {
                    log::debug!("Double-click detected at {:?}", mouse_pos);
                    if let Some(clicked_id) = self.pick_object_at_screen(mouse_pos) {
                        self.select_similar_units(clicked_id, add_to_selection)?;
                    }
                }
            }
            ButtonState::Pressed => {
                // Update selection box
                if self.selection_box.active {
                    self.selection_box.update(mouse_pos);
                }
            }
            ButtonState::JustReleased => {
                // Finish selection box
                if self.selection_box.active {
                    if self.selection_box.is_significant() {
                        // Perform box selection
                        let rect = self.selection_box.get_rect();
                        log::debug!("Selection box: {:?}", rect);
                        let selection_type = if add_to_selection {
                            SelectionType::Add
                        } else {
                            SelectionType::Replace
                        };
                        self.perform_box_selection(rect, selection_type)?;
                    } else {
                        // Single click selection
                        let selection_type = if keyboard.is_ctrl_pressed() {
                            SelectionType::Toggle
                        } else if keyboard.is_shift_pressed() {
                            SelectionType::Add
                        } else {
                            SelectionType::Replace
                        };
                        self.perform_click_selection(mouse_pos, selection_type)?;
                    }
                    self.selection_box.finish();
                }
            }
            _ => {}
        }

        // Handle building placement
        if self.placement_preview.is_some() {
            if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                if let Some(preview) = self.placement_preview.as_mut() {
                    preview.position = Vec3::new(world_pos.x, world_pos.y, world_pos.z);
                    let validator = FoundationValidator::new_strict();
                    preview.is_legal = validator
                        .validate_placement(
                            &world_pos,
                            &preview.template_name,
                            preview.rotation,
                            self.player_id as ObjectID,
                        )
                        .is_ok();
                    TheInGameUI::set_placement_angle(preview.rotation);
                }
            }

            if TheInGameUI::is_placement_anchored() {
                if let Some(preview) = self.placement_preview.as_ref() {
                    if let Some(template) = TheThingFactory::find_template(&preview.template_name) {
                        if template.is_kind_of(KindOf::Barrier) {
                            if let Some((start, _)) = TheInGameUI::get_placement_points() {
                                let current =
                                    MsgICoord2D::new(mouse_pos.x as i32, mouse_pos.y as i32);
                                let dx = (current.x - start.x) as f32;
                                let dy = (current.y - start.y) as f32;
                                if (dx * dx + dy * dy).sqrt() >= PLACEMENT_DRAG_DISTANCE {
                                    TheInGameUI::set_placement_end(Some(current));
                                }
                            }
                        }
                    }
                }
            }

            if mouse.button_state(MouseButton::Left).just_pressed() {
                let (is_legal, template_name, rotation) = match self.placement_preview.as_ref() {
                    Some(preview) => (
                        preview.is_legal,
                        preview.template_name.clone(),
                        preview.rotation,
                    ),
                    None => (false, String::new(), 0.0),
                };

                if is_legal {
                    let template = match TheThingFactory::find_template(&template_name) {
                        Some(template) => template,
                        None => return Ok(()),
                    };
                    let build_id = template.get_id();
                    let is_line_build = template.is_kind_of(KindOf::Barrier);

                    if is_line_build {
                        let start = MsgICoord2D::new(mouse_pos.x as i32, mouse_pos.y as i32);
                        if !TheInGameUI::is_placement_anchored() {
                            TheInGameUI::set_placement_start(Some(start));
                            return Ok(());
                        }
                        TheInGameUI::set_placement_end(Some(start.clone()));
                        if let Some((start, end)) = TheInGameUI::get_placement_points() {
                            let dx = (end.x - start.x) as f32;
                            let dy = (end.y - start.y) as f32;
                            if (dx * dx + dy * dy).sqrt() < PLACEMENT_DRAG_DISTANCE {
                                return Ok(());
                            }
                            let Some(start_world) =
                                self.screen_to_world(Vec2::new(start.x as f32, start.y as f32))
                            else {
                                return Ok(());
                            };
                            let Some(end_world) =
                                self.screen_to_world(Vec2::new(end.x as f32, end.y as f32))
                            else {
                                return Ok(());
                            };
                            let _ = append_message_to_stream(GameMessageType::DozerConstructLine(
                                build_id,
                                MsgCoord3D::new(start_world.x, start_world.y, start_world.z),
                                MsgCoord3D::new(end_world.x, end_world.y, end_world.z),
                                rotation,
                            ));
                        }
                    } else if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                        let _ = append_message_to_stream(GameMessageType::DozerConstruct(
                            build_id,
                            MsgCoord3D::new(world_pos.x, world_pos.y, world_pos.z),
                            rotation,
                        ));
                    }

                    TheInGameUI::place_build_available(None, None);
                    TheInGameUI::set_placement_start(None);
                    self.placement_preview = None;
                }
            }
        }

        Ok(())
    }

    fn handle_pending_special_power(
        &mut self,
        mouse_pos: Vec2,
        left_button: ButtonState,
        right_button: ButtonState,
    ) -> Result<bool> {
        let Some(pending) = TheInGameUI::get_pending_special_power() else {
            return Ok(false);
        };

        if right_button.just_pressed() {
            TheInGameUI::clear_pending_special_power();
            return Ok(true);
        }

        if !left_button.just_pressed() {
            return Ok(true);
        }

        let options = SpecialPowerCommandOption::from_bits_truncate(pending.options);
        let mut issued = false;

        if options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        ) {
            if let Some(target_id) = self.pick_object_at_screen(mouse_pos) {
                if self.is_valid_special_power_target(target_id, options) {
                    let _ = append_message_to_stream(GameMessageType::DoSpecialPowerAtObject(
                        pending.power_id,
                        target_id,
                        pending.options,
                        pending.source_object_id,
                    ));
                    issued = true;
                }
            }
        }

        if !issued
            && options.intersects(
                SpecialPowerCommandOption::NEED_TARGET_POS
                    | SpecialPowerCommandOption::ATTACK_OBJECTS_POSITION,
            )
        {
            if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                let _ = append_message_to_stream(GameMessageType::DoSpecialPowerAtLocation(
                    pending.power_id,
                    MsgCoord3D::new(world_pos.x, world_pos.y, world_pos.z),
                    0.0,
                    0,
                    pending.options,
                    pending.source_object_id,
                ));
                issued = true;
            }
        }

        if issued {
            TheInGameUI::clear_pending_special_power();
        }

        Ok(true)
    }

    fn is_valid_special_power_target(
        &self,
        target_id: ObjectID,
        options: SpecialPowerCommandOption,
    ) -> bool {
        let needs_object = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );
        if !needs_object {
            return true;
        }

        let target = OBJECT_REGISTRY.get_object(target_id);
        let Some(target) = target else {
            return false;
        };
        let Ok(target_guard) = target.read() else {
            return false;
        };

        let target_player_id = target_guard
            .get_controlling_player_id()
            .map(|id| id as i32)
            .filter(|id| *id >= 0);
        let Ok(player_list) = ThePlayerList().read() else {
            return true;
        };

        let local_player = player_list
            .get_local_player()
            .and_then(|player| player.read().ok());
        let Some(local_player) = local_player else {
            return true;
        };

        let relationship = target_player_id
            .and_then(|id| player_list.get_player(id))
            .and_then(|player| player.read().ok())
            .map(|player| {
                if player.get_player_type() == PlayerType::Neutral {
                    Relationship::Neutral
                } else {
                    local_player.get_relationship(&player)
                }
            })
            .unwrap_or(Relationship::Neutral);

        if options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
            && relationship == Relationship::Enemy
        {
            return true;
        }

        if options.contains(SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
            && relationship == Relationship::Neutral
        {
            return true;
        }

        if options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
            && matches!(
                relationship,
                Relationship::Ally | Relationship::Allies | Relationship::Friend
            )
        {
            return true;
        }

        if options.contains(SpecialPowerCommandOption::NEED_TARGET_PRISONER) {
            return true;
        }

        false
    }

    /// Perform box selection
    fn perform_box_selection(&mut self, rect: UIRect, selection_type: SelectionType) -> Result<()> {
        let start = Vec2::new(rect.x, rect.y);
        let end = Vec2::new(rect.x + rect.width, rect.y + rect.height);
        let Some(world_start) = self.screen_to_world(start) else {
            return Ok(());
        };
        let Some(world_end) = self.screen_to_world(end) else {
            return Ok(());
        };

        let min_x = world_start.x.min(world_end.x).floor() as i32;
        let max_x = world_start.x.max(world_end.x).ceil() as i32;
        let min_y = world_start.y.min(world_end.y).floor() as i32;
        let max_y = world_start.y.max(world_end.y).ceil() as i32;

        let region = IRegion2D::new(ICoord2D::new(min_x, min_y), ICoord2D::new(max_x, max_y));

        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_in_region(region, selection_type, None);
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    /// Perform single click selection
    fn perform_click_selection(&mut self, pos: Vec2, selection_type: SelectionType) -> Result<()> {
        if let Some(object_id) = self.pick_object_at_screen(pos) {
            let selection_manager = get_selection_manager();
            let mut manager = match selection_manager.write() {
                Ok(manager) => manager,
                Err(_) => {
                    self.sync_selection_state();
                    return Ok(());
                }
            };
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![object_id], selection_type);
            }
        } else if matches!(selection_type, SelectionType::Replace) {
            let selection_manager = get_selection_manager();
            let mut manager = match selection_manager.write() {
                Ok(manager) => manager,
                Err(_) => {
                    self.sync_selection_state();
                    return Ok(());
                }
            };
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.clear_selection();
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    fn screen_to_world(&self, screen_pos: Vec2) -> Option<Coord3D> {
        let screen_pt = IPoint2::new(screen_pos.x as i32, screen_pos.y as i32);
        with_tactical_view_ref(|view| {
            view.screen_to_world(&screen_pt)
                .ok()
                .map(|pt| Coord3D::new(pt.x, pt.y, pt.z))
        })
    }

    fn world_to_screen(&self, world: &Coord3D) -> Option<Vec2> {
        let point = Point3::new(world.x, world.y, world.z);
        with_tactical_view_ref(|view| {
            view.world_to_screen(&point)
                .map(|pt| Vec2::new(pt.x as f32, pt.y as f32))
        })
    }

    fn pick_object_at_screen(&self, screen_pos: Vec2) -> Option<ObjectID> {
        const PICK_RADIUS_WORLD: f32 = 12.0;
        let Some(world) = self.screen_to_world(screen_pos) else {
            return None;
        };

        let mut best: Option<(ObjectID, f32)> = None;
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            let pos = guard.get_position();
            let dx = pos.x - world.x;
            let dy = pos.y - world.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= PICK_RADIUS_WORLD * PICK_RADIUS_WORLD {
                if best
                    .map(|(_, best_dist)| dist_sq < best_dist)
                    .unwrap_or(true)
                {
                    best = Some((guard.get_id(), dist_sq));
                }
            }
        }
        best.map(|(id, _)| id)
    }

    fn select_similar_units(
        &mut self,
        template_object_id: ObjectID,
        add_to_selection: bool,
    ) -> Result<()> {
        let Some(reference) = OBJECT_REGISTRY.get_object(template_object_id) else {
            return Ok(());
        };
        let Ok(reference_guard) = reference.read() else {
            return Ok(());
        };
        let template_name = reference_guard.get_template_name().to_string();
        let owner_id = reference_guard
            .get_controlling_player_id()
            .map(|id| id as i32);

        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            if guard.get_template_name() != template_name {
                continue;
            }
            if let Some(owner) = owner_id {
                if guard.get_controlling_player_id().map(|id| id as i32) != Some(owner) {
                    continue;
                }
            }
            matching.push(guard.get_id());
        }

        if matching.is_empty() {
            return Ok(());
        }

        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, selection_type);
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    fn sync_selection_state(&mut self) {
        let selection_manager = get_selection_manager();
        let selected_objects = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.selection_state.selected = selected_objects.into_iter().map(DrawableID).collect();
    }

    fn find_selected_builder(&self) -> Option<ObjectID> {
        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        for object_id in &selected_ids {
            if let Some(object_arc) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(object_guard) = object_arc.read() {
                    if object_guard.is_kind_of(KindOf::Dozer) {
                        return Some(*object_id);
                    }
                }
            }
        }

        selected_ids.first().copied()
    }

    /// Start building placement preview
    pub fn start_building_placement(&mut self, template_name: String, footprint: Vec2) {
        self.placement_preview = Some(PlacementPreview::new(template_name, footprint));
        let builder_id = self.find_selected_builder();
        TheInGameUI::place_build_available(
            self.placement_preview
                .as_ref()
                .map(|preview| preview.template_name.clone()),
            builder_id,
        );
    }

    /// Cancel building placement
    pub fn cancel_building_placement(&mut self) {
        self.placement_preview = None;
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::set_placement_start(None);
    }

    /// Update resources display
    pub fn update_resources(&mut self, credits: i32, power_available: i32, power_used: i32) {
        self.resource_display
            .update(credits, power_available, power_used);
    }

    /// Update minimap world bounds
    pub fn set_minimap_world_bounds(&mut self, min: Vec2, max: Vec2) {
        self.minimap.world_bounds = (min, max);
    }

    /// Update minimap camera position
    pub fn update_camera(&mut self, position: Vec3, viewport: Vec2) {
        self.minimap.camera_position = position;
        self.minimap.camera_viewport = viewport;
    }

    /// Add or update unit icon on minimap
    pub fn update_minimap_unit(&mut self, id: u32, world_pos: Vec2, color: [f32; 4]) {
        self.minimap.update_icon(DrawableID(id), world_pos, color);
    }

    /// Remove unit from minimap
    pub fn remove_minimap_unit(&mut self, id: u32) {
        self.minimap.remove_icon(DrawableID(id));
    }

    /// Select object
    pub fn select_object(&mut self, id: u32, add_to_selection: bool) {
        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![id as ObjectID], selection_type);
            }
        }
        self.sync_selection_state();
    }

    /// Deselect object
    pub fn deselect_object(&mut self, id: u32) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![id as ObjectID], SelectionType::Remove);
            }
        }
        self.sync_selection_state();
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.clear_selection();
            }
        }
        self.sync_selection_state();
    }

    /// Get current selection
    pub fn get_selection(&self) -> Vec<u32> {
        let selection_manager = get_selection_manager();
        if let Ok(manager) = selection_manager.read() {
            if let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) {
                return selection
                    .get_selected_objects()
                    .into_iter()
                    .map(|id| id as u32)
                    .collect();
            }
        }
        self.selection_state
            .get_selected()
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// Set selection group
    pub fn set_selection_group(&mut self, group: usize) {
        if group < 10 {
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                    selection.create_control_group(group);
                }
            }
            self.sync_selection_state();
        }
    }

    /// Recall selection group
    pub fn recall_selection_group(&mut self, group: usize) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_control_group(group, false);
            }
        }
        self.sync_selection_state();
    }

    /// Set local player id for selection routing.
    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = player_id;
        self.sync_selection_state();
    }

    /// Render the in-game UI
    pub fn render(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut renderer = self
            .renderer
            .write()
            .map_err(|_| InGameUIError::SystemError("Failed to lock renderer".into()))?;

        // Render selection box
        if self.selection_box.active && self.selection_box.is_significant() {
            self.render_selection_box(&mut renderer)?;
        }

        // Render minimap
        if self.minimap.visible {
            self.render_minimap(&mut renderer)?;
        }

        // Render resource display
        self.render_resources(&mut renderer)?;

        // Render placement preview
        if let Some(ref preview) = self.placement_preview {
            self.render_placement_preview(&mut renderer, preview)?;
        }

        Ok(())
    }

    /// Render selection box
    fn render_selection_box(&self, renderer: &mut UIRenderer) -> Result<()> {
        let rect = self.selection_box.get_rect();

        // Draw box outline
        renderer.draw_rect_outline_with_scissor(
            rect,
            2.0,
            [0.0, 1.0, 0.0, 0.8], // Green with alpha
            None,
        )?;

        // Draw semi-transparent fill
        renderer.draw_rect_with_scissor(
            rect,
            [0.0, 1.0, 0.0, 0.2], // Green with low alpha
            None,
        )?;

        Ok(())
    }

    /// Render minimap
    fn render_minimap(&self, renderer: &mut UIRenderer) -> Result<()> {
        let minimap_rect = UIRect::new(
            self.minimap.position.x,
            self.minimap.position.y,
            self.minimap.size.x,
            self.minimap.size.y,
        );

        // Draw minimap background
        renderer.draw_rect_with_scissor(minimap_rect, [0.1, 0.1, 0.1, 0.8], None)?;

        // Draw border
        renderer.draw_rect_outline_with_scissor(minimap_rect, 2.0, [0.5, 0.5, 0.5, 1.0], None)?;

        // Draw camera viewport indicator
        let cam_pos_2d = Vec2::new(
            self.minimap.camera_position.x,
            self.minimap.camera_position.z,
        );
        let cam_minimap = self.minimap.world_to_minimap(cam_pos_2d);
        let viewport_size = self.minimap.camera_viewport
            * (self.minimap.size / (self.minimap.world_bounds.1 - self.minimap.world_bounds.0));

        let viewport_rect = UIRect::new(
            cam_minimap.x - viewport_size.x / 2.0,
            cam_minimap.y - viewport_size.y / 2.0,
            viewport_size.x,
            viewport_size.y,
        );

        renderer.draw_rect_outline_with_scissor(viewport_rect, 1.0, [1.0, 1.0, 1.0, 0.8], None)?;

        // Draw unit icons
        for (_, icon) in &self.minimap.unit_icons {
            renderer.draw_rect_with_scissor(
                UIRect::new(
                    icon.position.x - icon.size / 2.0,
                    icon.position.y - icon.size / 2.0,
                    icon.size,
                    icon.size,
                ),
                icon.color,
                None,
            )?;
        }

        Ok(())
    }

    /// Render resource display
    fn render_resources(&self, renderer: &mut UIRenderer) -> Result<()> {
        let pos = self.resource_display.position;

        // Background panel
        renderer.draw_rect_with_scissor(
            UIRect::new(pos.x, pos.y, 250.0, 80.0),
            [0.0, 0.0, 0.0, 0.7],
            None,
        )?;

        // Credits text
        let credits_text = format!("${}", self.resource_display.credits);
        renderer.draw_text_simple(
            &credits_text,
            Vec2::new(pos.x + 10.0, pos.y + 10.0),
            16.0,
            [1.0, 1.0, 0.0, 1.0], // Yellow
        )?;

        // Power text
        let power_color = if self.resource_display.is_power_deficit() {
            [1.0, 0.0, 0.0, 1.0] // Red if deficit
        } else {
            [0.0, 1.0, 0.0, 1.0] // Green if OK
        };

        let power_text = format!(
            "Power: {}/{}",
            self.resource_display.power_used, self.resource_display.power_available
        );
        renderer.draw_text_simple(
            &power_text,
            Vec2::new(pos.x + 10.0, pos.y + 35.0),
            14.0,
            power_color,
        )?;

        // Power bar
        let power_pct = self.resource_display.get_power_percentage();
        let bar_width = 200.0;
        let bar_height = 15.0;

        // Bar background
        renderer.draw_rect_with_scissor(
            UIRect::new(pos.x + 10.0, pos.y + 55.0, bar_width, bar_height),
            [0.3, 0.3, 0.3, 1.0],
            None,
        )?;

        // Bar fill
        renderer.draw_rect_with_scissor(
            UIRect::new(
                pos.x + 10.0,
                pos.y + 55.0,
                bar_width * power_pct,
                bar_height,
            ),
            power_color,
            None,
        )?;

        Ok(())
    }

    /// Render building placement preview
    fn render_placement_preview(
        &self,
        renderer: &mut UIRenderer,
        preview: &PlacementPreview,
    ) -> Result<()> {
        let world = Coord3D::new(preview.position.x, preview.position.y, preview.position.z);
        let Some(screen_pos) = self.world_to_screen(&world) else {
            return Ok(());
        };
        let size = preview.footprint * 50.0; // Scale for visibility

        let rect = UIRect::new(
            screen_pos.x - size.x / 2.0,
            screen_pos.y - size.y / 2.0,
            size.x,
            size.y,
        );

        // Draw semi-transparent preview
        renderer.draw_rect_with_scissor(rect, preview.get_color(), None)?;

        // Draw border
        let border_color = if preview.is_legal {
            [0.0, 1.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };

        renderer.draw_rect_outline_with_scissor(rect, 2.0, border_color, None)?;

        Ok(())
    }

    /// Update UI state
    pub fn update(&mut self, delta_time: Duration) {
        self.last_update = Instant::now();
        self.ui_time += delta_time.as_secs_f32();
        if let Ok(mut renderer) = self.renderer.write() {
            renderer.set_time(self.ui_time);
        }
    }

    /// Resize UI elements
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height);

        // Reposition minimap to bottom-right
        let minimap_margin = 10.0;
        self.minimap.position = Vec2::new(
            width - self.minimap.size.x - minimap_margin,
            height - self.minimap.size.y - minimap_margin,
        );
    }

    /// Enable/disable UI
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if UI is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for SelectionBox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_box() {
        let mut box_sel = SelectionBox::new();
        assert!(!box_sel.active);

        box_sel.start_at(Vec2::new(10.0, 10.0));
        assert!(box_sel.active);

        box_sel.update(Vec2::new(100.0, 100.0));
        assert!(box_sel.is_significant());

        let rect = box_sel.get_rect();
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 10.0);
        assert_eq!(rect.width, 90.0);
        assert_eq!(rect.height, 90.0);
    }

    #[test]
    fn test_minimap_conversion() {
        let minimap = Minimap::new(Vec2::new(600.0, 400.0), Vec2::new(200.0, 200.0));

        let world_pos = Vec2::new(500.0, 500.0);
        let minimap_pos = minimap.world_to_minimap(world_pos);

        // Should be roughly in middle of minimap
        assert!((minimap_pos.x - 700.0).abs() < 1.0);
        assert!((minimap_pos.y - 500.0).abs() < 1.0);
    }

    #[test]
    fn test_selection_state() {
        let mut state = SelectionState::new(10);

        state.select(DrawableID(1), false);
        assert_eq!(state.count(), 1);

        state.select(DrawableID(2), true);
        assert_eq!(state.count(), 2);

        state.deselect(DrawableID(1));
        assert_eq!(state.count(), 1);
        assert!(!state.is_selected(DrawableID(1)));
        assert!(state.is_selected(DrawableID(2)));
    }

    #[test]
    fn test_placement_preview() {
        let mut preview = PlacementPreview::new("GLA_SupplyStash".into(), Vec2::new(3.0, 3.0));

        preview.update_position(Vec3::new(100.0, 0.0, 100.0), true);
        assert!(preview.is_legal);

        let color = preview.get_color();
        assert_eq!(color[0], LEGAL_BUILD_COLOR[0]);
        assert_eq!(color[3], PLACEMENT_OPACITY);
    }

    #[test]
    fn test_resource_display() {
        let mut display = ResourceDisplay::new(Vec2::ZERO);

        display.update(10000, 100, 50);
        assert_eq!(display.credits, 10000);
        assert!(!display.is_power_deficit());
        assert!((display.get_power_percentage() - 0.5).abs() < 0.01);

        display.update(5000, 100, 150);
        assert!(display.is_power_deficit());
    }
}
