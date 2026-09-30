/// C++ `CommandAvailability` residual on the live GameHUD strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnitCommandAvailability {
    Hidden,
    Restricted,
    NotReady,
    CantAfford,
    Active,
    #[default]
    Available,
}

/// Frozen command-strip slot passed from presentation state to the HUD.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitCommandButton {
    pub command_name: String,
    pub enabled: bool,
    pub exit_object_id: Option<u32>,
    pub button_image: String,
    pub overlay_image: Option<String>,
    pub availability: UnitCommandAvailability,
}
