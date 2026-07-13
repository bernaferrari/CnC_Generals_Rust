//! RTS Interface Elements
//!
//! This module implements RTS-specific interface elements like unit selection,
//! command panels, and building interfaces.
//!
//! Selection panel health/name is presentation-owned (see
//! `PresentationFrame::apply_to_rts_interface`) so dual-tick consumers do not
//! re-read live GameLogic for HUD identity.

use super::{
    ControlBarSelectionPanelState, Interactive, KeyCode, MouseButton, Renderable, UIRenderContext,
    UnitDisplayInfo,
};
use crate::game_logic::ObjectId;

/// RTS interface for unit commands and selection.
///
/// Holds a presentation-fed selection panel so WND/ControlBar consumers share
/// the same snapshot identity as GameHUD / GameUIState.
pub struct RTSInterface {
    visible: bool,
    /// Snapshot-owned selection (portrait + health strip).
    selection_panel: ControlBarSelectionPanelState,
    selected_ids: Vec<ObjectId>,
}

impl Default for RTSInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl RTSInterface {
    pub fn new() -> Self {
        Self {
            visible: true,
            selection_panel: ControlBarSelectionPanelState::default(),
            selected_ids: Vec::new(),
        }
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn update(&mut self, _delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn resize(&mut self, _width: u32, _height: u32) {}

    /// Apply selection identity from a presentation-owned panel (no live re-read).
    pub fn apply_selection_panel(
        &mut self,
        panel: ControlBarSelectionPanelState,
        selected_ids: Vec<ObjectId>,
    ) {
        self.selection_panel = panel;
        self.selected_ids = selected_ids;
        // Keep panel IDs consistent when presentation only supplies infos.
        if self.selected_ids.is_empty() {
            self.selected_ids = self
                .selection_panel
                .unit_infos
                .iter()
                .map(|u| u.object_id)
                .collect();
        }
    }

    pub fn selection_panel(&self) -> &ControlBarSelectionPanelState {
        &self.selection_panel
    }

    pub fn selected_ids(&self) -> &[ObjectId] {
        &self.selected_ids
    }

    pub fn selected_unit_infos(&self) -> &[UnitDisplayInfo] {
        &self.selection_panel.unit_infos
    }

    pub fn clear_selection(&mut self) {
        self.selection_panel = ControlBarSelectionPanelState::default();
        self.selected_ids.clear();
    }
}

impl Interactive for RTSInterface {
    fn handle_mouse_move(&mut self, _x: i32, _y: i32) -> bool {
        false
    }
    fn handle_mouse_click(&mut self, _x: i32, _y: i32, _button: MouseButton) -> bool {
        false
    }
    fn handle_key_press(&mut self, _key: KeyCode) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for RTSInterface {
    fn render(&self, _context: &mut UIRenderContext) {}
    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, 0, 0)
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Unit command panel (context-sensitive command grid).
///
/// Selection identity is presentation-fed so command enablement can use snapshot HP.
pub struct UnitCommandPanel {
    visible: bool,
    selection_panel: ControlBarSelectionPanelState,
    selected_ids: Vec<ObjectId>,
}

impl Default for UnitCommandPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl UnitCommandPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            selection_panel: ControlBarSelectionPanelState::default(),
            selected_ids: Vec::new(),
        }
    }

    /// Show/hide based on whether presentation reports a selection.
    pub fn apply_selection_panel(
        &mut self,
        panel: ControlBarSelectionPanelState,
        selected_ids: Vec<ObjectId>,
    ) {
        self.selection_panel = panel;
        self.selected_ids = selected_ids;
        if self.selected_ids.is_empty() {
            self.selected_ids = self
                .selection_panel
                .unit_infos
                .iter()
                .map(|u| u.object_id)
                .collect();
        }
        self.visible = self.selection_panel.visible && !self.selected_ids.is_empty();
    }

    pub fn selection_panel(&self) -> &ControlBarSelectionPanelState {
        &self.selection_panel
    }

    pub fn selected_ids(&self) -> &[ObjectId] {
        &self.selected_ids
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn clear_selection(&mut self) {
        self.selection_panel = ControlBarSelectionPanelState::default();
        self.selected_ids.clear();
        self.visible = false;
    }
}

/// Building interface for construction
pub struct BuildingInterface {
    visible: bool,
}

impl Default for BuildingInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildingInterface {
    pub fn new() -> Self {
        Self { visible: false }
    }
}
