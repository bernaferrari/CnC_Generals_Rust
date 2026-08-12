use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationTargetHint {
    pub id: ObjectId,
    pub is_alive: bool,
    pub is_structure: bool,
    pub is_resource: bool,
    pub under_construction: bool,
    pub sold: bool,
    pub team: crate::game_logic::Team,
    pub is_enemy_of_local: bool,
    pub is_neutral: bool,
    pub template_name: String,
    pub can_be_entered: bool,
    /// Wave 235: damaged structure/unit residual for repair/service classification.
    pub is_damaged: bool,
    /// Wave 235: ally of local player (same team).
    pub is_friendly_of_local: bool,
    /// Wave 235: structure provides vehicle/aircraft repair pad residual.
    pub provides_vehicle_repair: bool,
    /// Wave 235: structure provides aircraft repair residual.
    pub provides_aircraft_repair: bool,
    /// Wave 235: heal pad / medical residual.
    pub provides_heal: bool,
}

/// Wave 229: presentation-frozen selected-unit capability for RMB classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSelectedUnitHint {
    pub id: ObjectId,
    pub is_alive: bool,
    /// C++ `KINDOF_HARVESTER`, frozen from the presentation snapshot for
    /// resource Gather classification.  Missing data must not grant gather
    /// permission when loading an older serialized input context.
    #[serde(default)]
    pub is_resource_collector: bool,
    /// Legacy builder/worker capability used by construction and repair
    /// classification.  It is deliberately not used for resource Gather.
    pub is_worker: bool,
    pub can_attack: bool,
    pub can_move: bool,
    pub can_capture: bool,
    pub template_name: String,
    /// Wave 235: dozer/worker repair residual.
    pub can_repair: bool,
    /// Wave 235: damaged unit residual (seek repair/heal).
    pub is_damaged: bool,
    pub is_vehicle: bool,
    pub is_aircraft: bool,
    pub is_infantry: bool,
}

/// Information needed for command creation from mouse input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseCommandContext {
    pub world_position: Vec3,
    pub target_object: Option<ObjectId>,
    /// Presentation freeze for target classification (InGame).
    pub target_presentation: Option<PresentationTargetHint>,
    /// Wave 229: presentation freeze for selected-unit capabilities (InGame).
    pub selected_presentation: Vec<PresentationSelectedUnitHint>,
    /// Wave 236: presentation-frozen box-select unit ids (drag LMB).
    #[serde(default)]
    pub presentation_box_select_units: Vec<ObjectId>,
    /// Wave 236: presentation-frozen select-similar unit ids (double-click LMB).
    #[serde(default)]
    pub presentation_select_similar_units: Vec<ObjectId>,
    pub screen_position: Vec2,
    pub viewport_size: Option<Vec2>,
    pub world_min: Option<Vec3>,
    pub world_max: Option<Vec3>,
    pub mouse_button: MouseButton,
    pub modifier_keys: ModifierKeys,
    pub is_drag: bool,
    pub drag_start: Option<Vec2>,
    pub drag_end: Option<Vec2>,
    pub drag_start_world: Option<Vec3>,
    pub drag_end_world: Option<Vec3>,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Command system state for tracking mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandMode {
    Normal,
    ForceAttack,
    ForceMove,
    Waypoint,
    BuildMode { template_name: String },
    SpecialPower { power_type: SpecialPowerType },
}
