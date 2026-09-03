// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

pub const MAX_PURCHASE_SCIENCE_RANK_1: usize = 4;
pub const MAX_PURCHASE_SCIENCE_RANK_3: usize = 15;
pub const MAX_PURCHASE_SCIENCE_RANK_8: usize = 4;
pub const MAX_SPECIAL_POWER_SHORTCUTS: usize = 8;
pub const MAX_RIGHT_HUD_UPGRADE_CAMEOS: usize = 4;
const RADAR_ATTACK_GLOW_FRAMES: u32 = 150;
const RADAR_ATTACK_GLOW_NUM_TIMES: u32 = 15;
const LOGICFRAMES_PER_SECOND: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlBarStage {
    #[default]
    Default,
    Low,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandButtonMappedBorderType {
    None,
    Build,
    Upgrade,
    Action,
    System,
}

#[derive(Debug, Clone, Default)]
pub struct PortraitDisplayState {
    pub portrait_image: String,
    pub veterancy_overlay: Option<String>,
    pub upgrade_cameos: Vec<UpgradeCameoState>,
    pub is_visible: bool,
    /// Selection health from PresentationFrame snapshot (not live OBJECT_REGISTRY).
    pub health_current: f32,
    pub health_maximum: f32,
    /// Number of selected objects reflected on the selection panel.
    pub selected_count: usize,
    /// First production queue item progress from PresentationFrame (0..1).
    pub production_progress: Option<f32>,
    /// First production queue template from PresentationFrame.
    pub production_template: Option<String>,
    /// Wave 986: production pause residual from PresentationFrame / host BuildingData.
    pub production_paused: bool,
    /// Special power ready residual from PresentationFrame.
    pub special_power_ready: bool,
    /// Special power cooldown remaining residual (seconds).
    pub special_power_cooldown_remaining: f32,
    /// Special power full cooldown residual (seconds). C++ getPercentReadyToFire.
    pub special_power_cooldown_total: f32,
    /// Structure rally point residual from PresentationFrame (xyz).
    pub rally_point: Option<[f32; 3]>,
}

#[derive(Debug, Clone)]
pub struct UpgradeCameoState {
    pub upgrade_name: String,
    pub button_image: String,
    pub is_completed: bool,
    pub is_visible: bool,
}

/// Resolve portrait upgrade-cameo art.
///
/// C++ `ControlBar.cpp` `setPortraitByObject`:
/// ```
/// const UpgradeTemplate *ut = TheUpgradeCenter->findUpgrade(upgradeName);
/// m_rightHUDUpgradeCameos[i]->winSetEnabledImage(0, ut->getButtonImage());
/// ```
/// `UpgradeTemplate::ButtonImage` is the mapped-image name. CommandButton
/// `ButtonImage` with matching `Upgrade=` is the shipped CommandSet fallback
/// when the template image was not registered. Unknown upgrades keep the
/// upgrade name as a fail-closed placeholder.
fn resolve_upgrade_cameo_button_image(
    upgrade_name: &str,
    context_commands: Option<&[CommandButton]>,
) -> String {
    let trimmed = upgrade_name.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // 1) C++ TheUpgradeCenter->findUpgrade + getButtonImage()
    let logic_image = with_upgrade_center(|center| {
        center
            .find_upgrade(trimmed)
            .map(|template| template.get_button_image_name().as_str().trim().to_string())
    });
    if let Some(image) = logic_image {
        if !image.is_empty() {
            return image;
        }
    }

    // 2) Common INI UpgradeTemplate.ButtonImage (same Upgrade.ini field)
    {
        let center = game_engine::common::ini::ini_upgrade::get_upgrade_center();
        let center = center.read().expect("UpgradeCenter poisoned");
        let key = game_engine::common::ascii_string::AsciiString::from(trimmed);
        if let Some(template) = center.find_template(&key) {
            let image = template.button_image.as_str().trim();
            if !image.is_empty() {
                return image.to_string();
            }
        }
    }

    // 3) CommandButton.ButtonImage where Upgrade= matches
    if let Some(control_bar) = get_ini_control_bar() {
        for (_, button) in control_bar.iter_resolved_buttons() {
            if button.upgrade.eq_ignore_ascii_case(trimmed)
                && !button.button_image.trim().is_empty()
            {
                return button.button_image.clone();
            }
        }
    }

    // 4) Presentation-synced CommandSet buttons already on the bar
    if let Some(commands) = context_commands {
        for button in commands {
            if button.upgrade.eq_ignore_ascii_case(trimmed)
                && !button.button_image.trim().is_empty()
            {
                return button.button_image.clone();
            }
        }
    }

    // Fail-closed: keep the upgrade name when no shipped art is registered.
    trimmed.to_string()
}

#[derive(Debug, Clone)]
pub struct SciencePurchaseState {
    pub rank1_buttons: Vec<ScienceButtonState>,
    pub rank3_buttons: Vec<ScienceButtonState>,
    pub rank8_buttons: Vec<ScienceButtonState>,
    pub available_points: i32,
    pub rank_level: i32,
    pub experience_progress: f32,
    pub rank_title_label: String,
    pub is_visible: bool,
    /// Unlocked science names residual from PresentationFrame (not live player list).
    pub unlocked_sciences: Vec<String>,
    /// Live host rank bar 0..100 from PresentationFrame (not leftover PlayerList).
    pub live_rank_progress_percent: Option<i32>,
    pub live_skill_points: Option<i32>,
    pub live_science_purchase_points: Option<i32>,
    pub live_rank_level: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct ScienceButtonState {
    pub command_name: String,
    pub science_type: ScienceType,
    pub is_hidden: bool,
    pub is_enabled: bool,
    pub is_purchased: bool,
}

#[derive(Debug, Clone)]
pub struct SpecialPowerShortcutState {
    pub command_name: String,
    pub availability: CommandAvailability,
    pub multiplier_count: i32,
    pub is_hidden: bool,
}

impl Default for SciencePurchaseState {
    fn default() -> Self {
        Self {
            rank1_buttons: Vec::new(),
            rank3_buttons: Vec::new(),
            rank8_buttons: Vec::new(),
            available_points: 0,
            rank_level: 0,
            experience_progress: 0.0,
            rank_title_label: String::new(),
            is_visible: false,
            unlocked_sciences: Vec::new(),
            live_rank_progress_percent: None,
            live_skill_points: None,
            live_science_purchase_points: None,
            live_rank_level: None,
        }
    }
}

/// Wave 985: host residual production pause requests (producer_id, paused).
static HOST_PRODUCTION_PAUSE_QUEUE: std::sync::Mutex<Vec<(u32, bool)>> =
    std::sync::Mutex::new(Vec::new());

/// Queue host production pause residual for Main GameLogic drain.
pub fn queue_host_production_pause(producer_id: u32, paused: bool) {
    if let Ok(mut q) = HOST_PRODUCTION_PAUSE_QUEUE.lock() {
        // Last write for same producer wins.
        q.retain(|(id, _)| *id != producer_id);
        q.push((producer_id, paused));
    }
}

/// Drain host production pause residual queue.
pub fn take_host_production_pause_requests() -> Vec<(u32, bool)> {
    HOST_PRODUCTION_PAUSE_QUEUE
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

/// Discard legacy host-pause residuals without applying them.
///
/// This is used when Control Bar authority changes to the Main host: work
/// queued for the legacy GameLogic path must never be replayed into a later
/// authoritative-world session.
pub fn clear_host_production_pause_requests() {
    HOST_PRODUCTION_PAUSE_QUEUE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

pub struct ControlBar {
    context: Arc<RwLock<ControlBarContext>>,
    window_manager: Option<Arc<WindowManager>>,
    scheme_manager: Option<Arc<dyn ControlBarSchemeManager>>,
    resizer: Option<Arc<dyn ControlBarResizer>>,
    current_window: Option<Arc<GameWindow>>,
    is_animating: bool,
    animation_start_time: Instant,
    animation_duration: Duration,
    button_states: HashMap<String, ButtonState>,
    observer_mode: bool,
    multi_select_mode: bool,
    ui_dirty: bool,
    build_queue_data: Vec<BuildQueueEntry>,
    displayed_queue_count: usize,
    current_frame: u32,
    flash_active: bool,
    control_bar_stage: ControlBarStage,
    portrait_state: PortraitDisplayState,
    science_state: SciencePurchaseState,
    gen_star_flash: bool,
    last_flashed_at_point_value: i32,
    radar_attack_glow_on: bool,
    remaining_radar_attack_glow_frames: u32,
    science_layout_loaded: bool,
    rally_point_drawable_id: u32,
    default_control_bar_x: i32,
    default_control_bar_y: i32,
    default_control_bar_captured: bool,
    special_power_shortcut_layout: String,
    radar_glow_window_enabled: bool,
    special_power_shortcuts: Vec<SpecialPowerShortcutState>,
    special_power_shortcut_count: usize,
    /// Radar provider count residual from PresentationFrame.
    presentation_radar_count: i32,
    /// Radar disabled residual from PresentationFrame.
    presentation_radar_disabled: bool,
    /// Queued upgrade names residual from PresentationFrame.
    presentation_queued_upgrades: Vec<String>,
    /// Primary selection command-set name residual from PresentationFrame.
    presentation_primary_command_set: String,
    /// Multi-select command-set names residual from PresentationFrame (ordered).
    presentation_command_set_names: Vec<String>,
    /// Structure inventory residual from PresentationFrame.
    presentation_max_garrison: usize,
    presentation_garrisoned_count: usize,
    /// Occupant portraits residual from PresentationFrame garrisoned_units.
    presentation_occupants: Vec<super::control_bar_structure_inventory::StructureInventoryOccupant>,
    /// Under-construction residual from PresentationFrame.
    presentation_under_construction: bool,
    /// Wave 1033: sold residual from PresentationFrame.
    presentation_sold: bool,
    /// C++ areSelectedObjectsControllable residual from PresentationFrame.
    presentation_selection_controllable: bool,
    /// Construction percent residual from PresentationFrame (0..1).
    presentation_construction_percent: f32,
    /// OCL timer seconds residual from PresentationFrame.
    presentation_ocl_timer_seconds: u32,
    displayed_construct_percent: f32,
    displayed_ocl_timer_seconds: u32,
    /// C++ InGameUI.cpp lastMoney — skip MoneyDisplay set_text when unchanged.
    last_displayed_money: i32,
    /// Presentation CanMake residual (template → CANMAKE_* ordinal).
    presentation_can_make: Vec<(String, u32)>,
    /// Live-host getCommandAvailability residual (OBJECT_REGISTRY empty).
    presentation_availability: PresentationAvailabilityResidual,

    border_colors: CommandBarBorderColors,
}

#[derive(Debug, Clone, Default)]
struct CommandBarBorderColors {
    build: Option<u32>,
    action: Option<u32>,
    upgrade: Option<u32>,
    system: Option<u32>,
}

#[derive(Debug, Clone)]
struct ButtonState {
    enabled: bool,
    visible: bool,
    pressed: bool,
    progress: f32,
    flash_time: Option<Instant>,
    availability: CommandAvailability,
    check_like_active: bool,
}

pub trait ControlBarSchemeManager: Send + Sync {
    fn load_scheme(&self, scheme_name: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn get_scheme(&self) -> Option<Arc<ControlBarScheme>>;
    fn set_scheme(&mut self, scheme: Arc<ControlBarScheme>);
}

pub trait ControlBarResizer: Send + Sync {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>>;
    fn get_optimal_size(&self) -> (u32, u32);
}

#[derive(Debug, Clone)]
pub struct ControlBarScheme {
    pub name: String,
    pub images: HashMap<String, String>,
    pub animations: HashMap<String, ControlBarAnimation>,
    pub layout: ControlBarLayout,
}

#[derive(Debug, Clone)]
pub struct ControlBarAnimation {
    pub frames: Vec<String>,
    pub frame_duration: Duration,
    pub loop_animation: bool,
}

#[derive(Debug, Clone)]
pub struct ControlBarLayout {
    pub command_buttons: Vec<ButtonLayout>,
    pub info_panels: Vec<PanelLayout>,
    pub construction_queue: QueueLayout,
}

#[derive(Debug, Clone)]
pub struct ButtonLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub command_name: String,
}

#[derive(Debug, Clone)]
pub struct PanelLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub panel_type: String,
}

#[derive(Debug, Clone)]
pub struct QueueLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub max_visible_items: u32,
}
