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
/// Snapshot-owned command button residual for the unit command panel.
///
/// Fail-closed: not full CommandSet INI / WND button art parity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitCommandButton {
    pub command_name: String,
    pub enabled: bool,
}

pub struct UnitCommandPanel {
    visible: bool,
    selection_panel: ControlBarSelectionPanelState,
    selected_ids: Vec<ObjectId>,
    /// Commands derived from PresentationFrame selection residual.
    commands: Vec<UnitCommandButton>,
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
            commands: Vec::new(),
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

    /// Replace command buttons from presentation residual (no live GameLogic).
    pub fn apply_commands(&mut self, commands: Vec<UnitCommandButton>) {
        self.commands = commands;
        if !self.commands.is_empty() {
            self.visible = true;
        }
    }

    pub fn commands(&self) -> &[UnitCommandButton] {
        &self.commands
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

    /// Activate a presentation command button by name → [`crate::command_system::CommandType`].
    ///
    /// Returns None when disabled, unknown, or selection empty. Fills CancelConstruction
    /// object_id from primary selection residual.
    pub fn activate_command(
        &self,
        command_name: &str,
    ) -> Option<crate::command_system::CommandType> {
        let btn = self
            .commands
            .iter()
            .find(|c| c.command_name.eq_ignore_ascii_case(command_name))?;
        if !btn.enabled {
            return None;
        }
        let mut cmd = crate::command_system::command_type_from_button_name(&btn.command_name)?;
        if let crate::command_system::CommandType::DozerCancelConstruct { object_id } = &mut cmd {
            if let Some(id) = self.selected_ids.first().copied() {
                *object_id = id;
            } else if let Some(id) = self.selection_panel.primary_object_id {
                *object_id = id;
            }
        }
        Some(cmd)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_system::CommandType;
    use crate::game_logic::ObjectId;

    #[test]
    fn activate_command_maps_upgrade_and_respects_enabled_residual() {
        let mut panel = UnitCommandPanel::new();
        panel.apply_selection_panel(
            ControlBarSelectionPanelState {
                visible: true,
                primary_object_id: Some(ObjectId(1)),
                ..ControlBarSelectionPanelState::default()
            },
            vec![ObjectId(1)],
        );
        panel.apply_commands(vec![
            UnitCommandButton {
                command_name: "Command_UpgradeAmericaSupplyLines".into(),
                enabled: true,
            },
            UnitCommandButton {
                command_name: "Command_UpgradeAmericaRangerFlashBangGrenade".into(),
                enabled: false,
            },
        ]);
        match panel
            .activate_command("Command_UpgradeAmericaSupplyLines")
            .expect("enabled upgrade")
        {
            CommandType::QueueUpgrade { upgrade_name } => {
                assert_eq!(upgrade_name, "Upgrade_AmericaSupplyLines");
            }
            other => panic!("got {other:?}"),
        }
        assert!(
            panel
                .activate_command("Command_UpgradeAmericaRangerFlashBangGrenade")
                .is_none(),
            "disabled button must not activate"
        );
        match panel
            .activate_command("Command_CancelUpgrade")
            .map(|_| ())
            .or_else(|| {
                // Cancel not in list → None
                None
            }) {
            None => {}
            Some(()) => panic!("unexpected"),
        }
        // Add cancel and activate.
        panel.apply_commands(vec![UnitCommandButton {
            command_name: "Command_CancelUpgrade".into(),
            enabled: true,
        }]);
        assert!(matches!(
            panel.activate_command("Command_CancelUpgrade"),
            Some(CommandType::CancelUpgrade { .. })
        ));
    }
}
