// ============================================================================
// AUTO DEPOSIT UPDATE
// ============================================================================

/// Auto deposit update module data
/// Matches C++ AutoDepositUpdateModuleData
#[derive(Debug, Clone)]
pub struct AutoDepositUpdateData {
    /// Amount to deposit
    pub deposit_amount: u32,
    /// Frame interval for deposits
    pub deposit_interval: u32,
    /// Initial capture bonus
    pub initial_capture_bonus: u32,
    /// Whether this is actual money (not just score)
    pub is_actual_money: bool,
}

impl Default for AutoDepositUpdateData {
    fn default() -> Self {
        Self {
            deposit_amount: 0,
            deposit_interval: 150, // 5 seconds at 30 FPS
            initial_capture_bonus: 0,
            is_actual_money: true,
        }
    }
}

/// Auto deposit update module (for oil derricks, black market, hackers)
/// Matches C++ AutoDepositUpdate
pub struct AutoDepositUpdate {
    /// Configuration data
    data: AutoDepositUpdateData,
    /// Frame to deposit on next
    deposit_on_frame: u32,
    /// Whether to award initial capture bonus
    award_initial_capture_bonus: bool,
    /// Whether initialized
    initialized: bool,
    /// Player index for upgrade checks
    player_index: PlayerIndex,
    /// UI system reference (optional)
    ui_system: Option<Arc<dyn UISystem>>,
    /// Upgrade system reference (optional)
    upgrade_system: Option<Arc<dyn UpgradeSystem>>,
}

impl std::fmt::Debug for AutoDepositUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoDepositUpdate")
            .field("data", &self.data)
            .field("deposit_on_frame", &self.deposit_on_frame)
            .field(
                "award_initial_capture_bonus",
                &self.award_initial_capture_bonus,
            )
            .field("initialized", &self.initialized)
            .field("player_index", &self.player_index)
            .field("ui_system", &self.ui_system.as_ref().map(|_| "UISystem"))
            .field(
                "upgrade_system",
                &self.upgrade_system.as_ref().map(|_| "UpgradeSystem"),
            )
            .finish()
    }
}

impl AutoDepositUpdate {
    pub fn new(data: AutoDepositUpdateData, current_frame: u32, player_index: PlayerIndex) -> Self {
        Self {
            deposit_on_frame: current_frame + data.deposit_interval,
            data,
            award_initial_capture_bonus: false,
            initialized: false,
            player_index,
            ui_system: None,
            upgrade_system: None,
        }
    }

    /// Set UI system for floating text
    pub fn set_ui_system(&mut self, ui_system: Arc<dyn UISystem>) {
        self.ui_system = Some(ui_system);
    }

    /// Set upgrade system for supply boost calculation
    pub fn set_upgrade_system(&mut self, upgrade_system: Arc<dyn UpgradeSystem>) {
        self.upgrade_system = Some(upgrade_system);
    }

    /// Award initial capture bonus
    /// Matches C++ AutoDepositUpdate::awardInitialCaptureBonus()
    pub fn award_initial_capture_bonus(
        &mut self,
        player_money: &mut Money,
        current_frame: u32,
        building_position: &Coord3D,
        player_color: Color,
    ) {
        self.deposit_on_frame = current_frame + self.data.deposit_interval;

        if !self.award_initial_capture_bonus || self.data.initial_capture_bonus == 0 {
            return;
        }

        player_money.deposit(self.data.initial_capture_bonus, true);

        // Display floating text for initial capture bonus
        if let Some(ui) = &self.ui_system {
            let text = format!("+${}", self.data.initial_capture_bonus);
            let color_with_alpha = player_color | 0xE6000000;
            ui.add_floating_text(&text, building_position, color_with_alpha);
        }

        self.award_initial_capture_bonus = false;
    }

    /// Update the auto deposit (called each frame)
    /// Matches C++ AutoDepositUpdate::update() - AutoDepositUpdate.cpp:126
    pub fn update(
        &mut self,
        current_frame: u32,
        is_neutral: bool,
        construction_complete: bool,
        player_money: &mut Money,
        building_position: &Coord3D,
        player_color: Color,
    ) -> bool {
        if current_frame >= self.deposit_on_frame {
            if !self.initialized {
                // Set on first update, not on load
                self.award_initial_capture_bonus = true;
                self.initialized = true;
            }

            self.deposit_on_frame = current_frame + self.data.deposit_interval;

            if is_neutral || self.data.deposit_amount == 0 {
                return false;
            }

            // Buildings under construction don't get bonuses
            if !construction_complete {
                return false;
            }

            if self.data.is_actual_money {
                // Add upgraded supply boost
                // Matches C++ AutoDepositUpdate.cpp:143
                let upgraded_boost = if let Some(upgrade) = &self.upgrade_system {
                    upgrade.get_supply_boost(self.player_index)
                } else {
                    0
                };

                let total_amount = self.data.deposit_amount + upgraded_boost;
                player_money.deposit(total_amount, true);

                // Display floating text
                // Matches C++ AutoDepositUpdate.cpp:162-187
                if let Some(ui) = &self.ui_system {
                    let text = format!("+${}", total_amount);
                    let color_with_alpha = player_color | 0xE6000000;
                    ui.add_floating_text(&text, building_position, color_with_alpha);
                }

                return true;
            }
        }

        false
    }
}

