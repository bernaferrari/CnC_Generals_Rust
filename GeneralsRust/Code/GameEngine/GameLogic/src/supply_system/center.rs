// ============================================================================
// SUPPLY CENTER DOCK UPDATE
// ============================================================================

/// Supply center dock update module data
/// Matches C++ SupplyCenterDockUpdateModuleData
#[derive(Debug, Clone)]
pub struct SupplyCenterDockUpdateData {
    /// Frames to grant temporary stealth (0 = disabled)
    pub grant_temporary_stealth_frames: u32,
    /// Number of approach positions
    pub num_approaches: usize,
    /// Action delay time in frames
    pub action_delay: u32,
}

impl Default for SupplyCenterDockUpdateData {
    fn default() -> Self {
        Self {
            grant_temporary_stealth_frames: 0,
            num_approaches: 3,
            action_delay: 30,
        }
    }
}

/// Supply center dock update module
/// Matches C++ SupplyCenterDockUpdate
pub struct SupplyCenterDockUpdate {
    /// Configuration data
    data: SupplyCenterDockUpdateData,
    /// Currently docked object
    active_docker: Option<ObjectID>,
    /// UI system reference (optional)
    ui_system: Option<Arc<dyn UISystem>>,
    /// Stealth system reference (optional)
    stealth_system: Option<Arc<dyn StealthSystem>>,
}

impl std::fmt::Debug for SupplyCenterDockUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SupplyCenterDockUpdate")
            .field("data", &self.data)
            .field("active_docker", &self.active_docker)
            .field("ui_system", &self.ui_system.as_ref().map(|_| "UISystem"))
            .field(
                "stealth_system",
                &self.stealth_system.as_ref().map(|_| "StealthSystem"),
            )
            .finish()
    }
}

impl SupplyCenterDockUpdate {
    pub fn new(data: SupplyCenterDockUpdateData) -> Self {
        Self {
            data,
            active_docker: None,
            ui_system: None,
            stealth_system: None,
        }
    }

    /// Set UI system for floating text
    pub fn set_ui_system(&mut self, ui_system: Arc<dyn UISystem>) {
        self.ui_system = Some(ui_system);
    }

    /// Set stealth system for temporary stealth grants
    pub fn set_stealth_system(&mut self, stealth_system: Arc<dyn StealthSystem>) {
        self.stealth_system = Some(stealth_system);
    }

    /// Perform dock action - deposit money for boxes
    /// Matches C++ SupplyCenterDockUpdate::action() - SupplyCenterDockUpdate.cpp:64
    pub fn action(
        &mut self,
        docker_id: ObjectID,
        docker_position: &Coord3D,
        docker_boxes: &mut i32,
        supply_box_value: u32,
        upgraded_supply_boost: u32,
        player_money: &mut Money,
        player_color: Color,
        ground_z: Real,
        hide_stealth_cash: bool,
    ) -> Result<bool, String> {
        let mut value = 0u32;

        // Take all boxes from the truck
        // Matches C++ SupplyCenterDockUpdate.cpp:77-80
        while *docker_boxes > 0 {
            *docker_boxes -= 1;
            value += supply_box_value;
        }

        // Add money boost from upgrades
        // Matches C++ SupplyCenterDockUpdate.cpp:82-83
        value += upgraded_supply_boost;

        if value > 0 {
            // Deposit money to player
            // Matches C++ SupplyCenterDockUpdate.cpp:87-89
            player_money.deposit(value, true);

            // Grant temporary stealth if configured
            // Matches C++ SupplyCenterDockUpdate.cpp:92-108
            if self.data.grant_temporary_stealth_frames > 0 {
                if let Some(stealth) = &self.stealth_system {
                    stealth.grant_temporary_stealth(
                        docker_id,
                        self.data.grant_temporary_stealth_frames,
                    );
                }
            }

            // Display floating text showing money gained
            // Matches C++ SupplyCenterDockUpdate.cpp:112-137
            // Hide only when the *center* is STEALTHED, not locally controlled, not DETECTED.
            if !hide_stealth_cash {
                if let Some(ui) = &self.ui_system {
                    let text = format_gui_add_cash(value);
                    let pos = Coord3D {
                        x: docker_position.x,
                        y: docker_position.y,
                        z: ground_z,
                    };
                    // Color combines player color with alpha=230
                    // Matches C++ SupplyCenterDockUpdate.cpp:134
                    let color_with_alpha = player_color | 0xE6000000;
                    ui.add_floating_text(&text, &pos, color_with_alpha);
                }
            }
        }

        self.active_docker = Some(docker_id);
        Ok(false) // Always return false (don't stay docked)
    }

    pub fn set_active_docker(&mut self, docker_id: Option<ObjectID>) {
        self.active_docker = docker_id;
    }
}

