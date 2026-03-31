// FILE: control_bar.rs
// Port of ControlBar class from C++
// Original: ControlBar.h and ControlBar.cpp

use std::sync::{Arc, Mutex, OnceLock};
use std::cell::RefCell;
use super::types::*;
use super::command_button::CommandButton;
use super::command_set::CommandSet;
use super::scheme::ControlBarSchemeManager;

/// Side Select Window Data
/// Used to animate the generals select window
pub struct SideSelectWindowData {
    /// Side window reference
    pub side_window: Option<Arc<GameWindow>>,

    /// Animation window
    pub anim_window_win: Option<Arc<GameWindow>>,

    /// General speak audio
    pub general_speak: Option<AudioEventRTS>,

    /// Player template
    player_template: Option<Arc<PlayerTemplate>>,

    /// UI windows
    generals_name_win: Option<Arc<GameWindow>>,
    side_name_win: Option<Arc<GameWindow>>,

    /// Upgrade label windows
    upgrade_label_1_win: Option<Arc<GameWindow>>,
    upgrade_label_2_win: Option<Arc<GameWindow>>,
    upgrade_label_3_win: Option<Arc<GameWindow>>,
    upgrade_label_4_win: Option<Arc<GameWindow>>,

    /// Upgrade image windows
    upgrade_image_1_win: Option<Arc<GameWindow>>,
    upgrade_image_2_win: Option<Arc<GameWindow>>,
    upgrade_image_3_win: Option<Arc<GameWindow>>,
    upgrade_image_4_win: Option<Arc<GameWindow>>,

    /// Upgrade images
    upgrade_image_1: Option<Arc<Image>>,
    upgrade_image_2: Option<Arc<Image>>,
    upgrade_image_3: Option<Arc<Image>>,
    upgrade_image_4: Option<Arc<Image>>,

    upgrade_image_size: ICoord2D,

    /// State and timing
    state: SideSelectState,
    last_time: u32,
    start_time: u32,
    curr_color: Color,

    /// Line and clip regions (simplified, would use full IRegion2D in actual implementation)
    // Various geometric regions for the UI animations
}

impl SideSelectWindowData {
    pub fn new() -> Self {
        Self {
            side_window: None,
            anim_window_win: None,
            general_speak: None,
            player_template: None,
            generals_name_win: None,
            side_name_win: None,
            upgrade_label_1_win: None,
            upgrade_label_2_win: None,
            upgrade_label_3_win: None,
            upgrade_label_4_win: None,
            upgrade_image_1_win: None,
            upgrade_image_2_win: None,
            upgrade_image_3_win: None,
            upgrade_image_4_win: None,
            upgrade_image_1: None,
            upgrade_image_2: None,
            upgrade_image_3: None,
            upgrade_image_4: None,
            upgrade_image_size: ICoord2D { x: 0, y: 0 },
            state: SideSelectState::None,
            last_time: 0,
            start_time: 0,
            curr_color: Color::new(0, 0, 0, 0),
        }
    }

    pub fn init(&mut self, science: ScienceType, control: Arc<GameWindow>) {
        // Initialize side select data
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn update(&mut self) {
        // Update animation state
    }

    pub fn draw(&self) {
        // Draw the side select UI
    }
}

/// Contain Entry
/// Maps GUI controls to contained object IDs
#[derive(Clone, Debug)]
pub struct ContainEntry {
    pub control: Option<Arc<GameWindow>>,
    pub object_id: ObjectID,
}

impl ContainEntry {
    pub fn new() -> Self {
        Self {
            control: None,
            object_id: INVALID_OBJECT_ID,
        }
    }
}

/// Queue Entry
/// Represents an entry in the production queue
#[derive(Clone, Debug)]
pub struct QueueEntry {
    /// Window control tied to this queue entry
    pub control: Option<Arc<GameWindow>>,

    /// Type of production
    pub production_type: ProductionType,

    /// Production data (union in C++)
    pub production_id: ProductionID,
    pub upgrade_to_research: Option<Arc<UpgradeTemplate>>,
}

impl QueueEntry {
    pub fn new() -> Self {
        Self {
            control: None,
            production_type: ProductionType::None,
            production_id: INVALID_PRODUCTION_ID,
            upgrade_to_research: None,
        }
    }
}

/// Main Control Bar Structure
/// Context-sensitive command interface for the game
pub struct ControlBar {
    /// UI dirty flag - context needs re-evaluation
    ui_dirty: bool,

    /// Command buttons list (linked list in C++, Vec in Rust)
    command_buttons: Vec<Arc<CommandButton>>,

    /// Command sets list
    command_sets: Vec<Arc<CommandSet>>,

    /// Scheme manager
    control_bar_scheme_manager: Option<Arc<Mutex<ControlBarSchemeManager>>>,

    /// Context parent windows
    context_parents: [Option<Arc<GameWindow>>; ContextParent::NumContextParents as usize],

    /// Currently selected drawable driving the UI
    current_selected_drawable: Option<Arc<Drawable>>,

    /// Current displayed context
    curr_context: ControlBarContext,

    /// Rally point drawable ID
    rally_point_drawable_id: DrawableID,

    /// Display state tracking
    displayed_construct_percent: f32,
    displayed_ocl_timer_seconds: u32,
    displayed_queue_count: u32,
    last_recorded_inventory_count: u32,

    /// Right HUD windows
    right_hud_window: Option<Arc<GameWindow>>,
    right_hud_cameo_window: Option<Arc<GameWindow>>,
    right_hud_upgrade_cameos: [Option<Arc<GameWindow>>; MAX_RIGHT_HUD_UPGRADE_CAMEOS],
    right_hud_unit_select_parent: Option<Arc<GameWindow>>,

    /// Communicator button
    communicator_button: Option<Arc<GameWindow>>,

    /// Science purchase windows
    science_layout: Option<Arc<WindowLayout>>,
    science_purchase_windows_rank_1: [Option<Arc<GameWindow>>; MAX_PURCHASE_SCIENCE_RANK_1],
    science_purchase_windows_rank_3: [Option<Arc<GameWindow>>; MAX_PURCHASE_SCIENCE_RANK_3],
    science_purchase_windows_rank_8: [Option<Arc<GameWindow>>; MAX_PURCHASE_SCIENCE_RANK_8],

    /// Special power shortcut windows
    special_power_shortcut_buttons: [Option<Arc<GameWindow>>; MAX_SPECIAL_POWER_SHORTCUTS],
    special_power_shortcut_button_parents: [Option<Arc<GameWindow>>; MAX_SPECIAL_POWER_SHORTCUTS],
    shortcut_display_strings: [Option<Arc<DisplayString>>; MAX_SPECIAL_POWER_SHORTCUTS],
    currently_used_special_powers_buttons: i32,

    special_power_layout: Option<Arc<WindowLayout>>,
    special_power_shortcut_parent: Option<Arc<GameWindow>>,

    /// Command windows
    command_windows: [Option<Arc<GameWindow>>; MAX_COMMANDS_PER_SET],
    common_commands: [Option<Arc<CommandButton>>; MAX_COMMANDS_PER_SET],

    /// Container and queue data
    contain_data: [ContainEntry; MAX_COMMANDS_PER_SET],
    queue_data: [QueueEntry; MAX_BUILD_QUEUE_BUTTONS],

    /// Flash state
    flash: bool,

    /// Animation and video managers
    video_manager: Option<Arc<Mutex<WindowVideoManager>>>,
    animate_window_manager: Option<Arc<Mutex<AnimateWindowManager>>>,
    animate_window_manager_for_gen_shortcuts: Option<Arc<Mutex<AnimateWindowManager>>>,
    generals_screen_animate: Option<Arc<Mutex<AnimateWindowManager>>>,

    /// Side select animation state
    side_select_animate_down: bool,
    animate_down_win_1_size: ICoord2D,
    animate_down_win_2_size: ICoord2D,
    animate_down_win_1_pos: ICoord2D,
    animate_down_win_2_pos: ICoord2D,
    animate_down_window: Option<Arc<GameWindow>>,
    anim_time: u32,

    /// Color settings
    build_up_clock_color: Color,
    command_button_border_build_color: Color,
    command_button_border_action_color: Color,
    command_button_border_upgrade_color: Color,
    command_button_border_system_color: Color,
    command_bar_border_color: Color,

    /// Observer mode
    is_observer_command_bar: bool,
    observer_look_at_player: Option<Arc<Player>>,

    /// Build tooltip layout
    build_tooltip_layout: Option<Arc<WindowLayout>>,
    show_build_tooltip_layout: bool,

    /// Stars and icons
    gen_star_on: Option<Arc<Image>>,
    gen_star_off: Option<Arc<Image>>,
    gen_star_flash: bool,
    last_flashed_at_point_value: i32,

    /// Button images
    toggle_button_up_in: Option<Arc<Image>>,
    toggle_button_up_on: Option<Arc<Image>>,
    toggle_button_up_pushed: Option<Arc<Image>>,
    toggle_button_down_in: Option<Arc<Image>>,
    toggle_button_down_on: Option<Arc<Image>>,
    toggle_button_down_pushed: Option<Arc<Image>>,

    general_button_enable: Option<Arc<Image>>,
    general_button_highlight: Option<Arc<Image>>,

    /// Transition handler
    gen_arrow: Option<Arc<Image>>,

    /// Rank icons (static in C++)
    rank_veteran_icon: Option<Arc<Image>>,
    rank_elite_icon: Option<Arc<Image>>,
    rank_heroic_icon: Option<Arc<Image>>,

    /// Control bar position and stage
    default_control_bar_position: ICoord2D,
    current_control_bar_stage: ControlBarStages,

    /// Marker positions
    control_bar_foreground_marker_pos: ICoord2D,
    control_bar_background_marker_pos: ICoord2D,

    /// Radar attack glow
    radar_attack_glow_on: bool,
    remaining_radar_attack_glow_frames: i32,
    radar_attack_glow_window: Option<Arc<GameWindow>>,

    /// Debug tracking
    #[cfg(any(feature = "internal", debug_assertions))]
    last_frame_marked_dirty: u32,
    #[cfg(any(feature = "internal", debug_assertions))]
    consecutive_dirty_frames: u32,
}

impl ControlBar {
    /// Create a new ControlBar
    pub fn new() -> Self {
        Self {
            ui_dirty: false,
            command_buttons: Vec::new(),
            command_sets: Vec::new(),
            control_bar_scheme_manager: None,
            context_parents: Default::default(),
            current_selected_drawable: None,
            curr_context: ControlBarContext::None,
            rally_point_drawable_id: INVALID_DRAWABLE_ID,
            displayed_construct_percent: -1.0,
            displayed_ocl_timer_seconds: 0,
            displayed_queue_count: 0,
            last_recorded_inventory_count: 0,
            right_hud_window: None,
            right_hud_cameo_window: None,
            right_hud_upgrade_cameos: Default::default(),
            right_hud_unit_select_parent: None,
            communicator_button: None,
            science_layout: None,
            science_purchase_windows_rank_1: Default::default(),
            science_purchase_windows_rank_3: Default::default(),
            science_purchase_windows_rank_8: Default::default(),
            special_power_shortcut_buttons: Default::default(),
            special_power_shortcut_button_parents: Default::default(),
            shortcut_display_strings: Default::default(),
            currently_used_special_powers_buttons: 0,
            special_power_layout: None,
            special_power_shortcut_parent: None,
            command_windows: Default::default(),
            common_commands: Default::default(),
            contain_data: array_init::array_init(|_| ContainEntry::new()),
            queue_data: array_init::array_init(|_| QueueEntry::new()),
            flash: false,
            video_manager: None,
            animate_window_manager: None,
            animate_window_manager_for_gen_shortcuts: None,
            generals_screen_animate: None,
            side_select_animate_down: false,
            animate_down_win_1_size: ICoord2D { x: 0, y: 0 },
            animate_down_win_2_size: ICoord2D { x: 0, y: 0 },
            animate_down_win_1_pos: ICoord2D { x: 0, y: 0 },
            animate_down_win_2_pos: ICoord2D { x: 0, y: 0 },
            animate_down_window: None,
            anim_time: 0,
            build_up_clock_color: Color::new(0, 0, 0, 100),
            command_button_border_build_color: Color::undefined(),
            command_button_border_action_color: Color::undefined(),
            command_button_border_upgrade_color: Color::undefined(),
            command_button_border_system_color: Color::undefined(),
            command_bar_border_color: Color::new(0, 0, 0, 100),
            is_observer_command_bar: false,
            observer_look_at_player: None,
            build_tooltip_layout: None,
            show_build_tooltip_layout: false,
            gen_star_on: None,
            gen_star_off: None,
            gen_star_flash: true,
            last_flashed_at_point_value: -1,
            toggle_button_up_in: None,
            toggle_button_up_on: None,
            toggle_button_up_pushed: None,
            toggle_button_down_in: None,
            toggle_button_down_on: None,
            toggle_button_down_pushed: None,
            general_button_enable: None,
            general_button_highlight: None,
            gen_arrow: None,
            rank_veteran_icon: None,
            rank_elite_icon: None,
            rank_heroic_icon: None,
            default_control_bar_position: ICoord2D { x: 0, y: 0 },
            current_control_bar_stage: ControlBarStages::Default,
            control_bar_foreground_marker_pos: ICoord2D { x: 0, y: 0 },
            control_bar_background_marker_pos: ICoord2D { x: 0, y: 0 },
            radar_attack_glow_on: false,
            remaining_radar_attack_glow_frames: 0,
            radar_attack_glow_window: None,
            #[cfg(any(feature = "internal", debug_assertions))]
            last_frame_marked_dirty: 0,
            #[cfg(any(feature = "internal", debug_assertions))]
            consecutive_dirty_frames: 0,
        }
    }

    /// Initialize the control bar
    pub fn init(&mut self) {
        // Load command buttons from INI
        // Load command sets from INI
        // Initialize scheme manager
        // Get all UI windows
        // Set up default context
    }

    /// Reset the control bar
    pub fn reset(&mut self) {
        self.rally_point_drawable_id = INVALID_DRAWABLE_ID;
        self.displayed_construct_percent = -1.0;
        self.displayed_ocl_timer_seconds = 0;
        self.is_observer_command_bar = false;
        self.observer_look_at_player = None;
        self.show_build_tooltip_layout = false;
        self.side_select_animate_down = false;
        self.last_flashed_at_point_value = -1;
        self.gen_star_flash = true;

        // Switch back to default context
        self.switch_to_context(ControlBarContext::None, None);
    }

    /// Update the control bar
    pub fn update(&mut self) {
        // Update scheme manager animations
        // Update video manager
        // Update animation managers
        // Update tooltip layout
        // Update special power shortcuts
        // Check for flashing buttons
        // Evaluate context UI if dirty
    }

    /// Mark the UI as dirty
    pub fn mark_ui_dirty(&mut self) {
        self.ui_dirty = true;

        #[cfg(any(feature = "internal", debug_assertions))]
        {
            let now = get_game_frame();
            if now == self.last_frame_marked_dirty {
                // Same frame, do nothing
            } else if now == self.last_frame_marked_dirty + 1 {
                self.consecutive_dirty_frames += 1;
            } else {
                self.consecutive_dirty_frames = 1;
            }
            self.last_frame_marked_dirty = now;

            if self.consecutive_dirty_frames > 20 {
                panic!("Serious flaw in interface system! UI marked dirty every frame.");
            }
        }
    }

    /// Drawable has been selected
    pub fn on_drawable_selected(&mut self, drawable: Arc<Drawable>) {
        // Handle drawable selection
        self.mark_ui_dirty();
    }

    /// Drawable has been deselected
    pub fn on_drawable_deselected(&mut self, drawable: Arc<Drawable>) {
        // Handle drawable deselection
        self.mark_ui_dirty();
    }

    /// Player rank changed
    pub fn on_player_rank_changed(&mut self, player: &Player) {
        self.mark_ui_dirty();
    }

    /// Player science purchase points changed
    pub fn on_player_science_purchase_points_changed(&mut self, player: &Player) {
        self.mark_ui_dirty();
    }

    /// Process context-sensitive button click
    pub fn process_context_sensitive_button_click(
        &mut self,
        button: Arc<GameWindow>,
        gadget_message: GadgetGameMessage
    ) -> CBCommandStatus {
        // Process button clicks
        CBCommandStatus::NotUsed
    }

    /// Process context-sensitive button transition
    pub fn process_context_sensitive_button_transition(
        &mut self,
        button: Arc<GameWindow>,
        gadget_message: GadgetGameMessage
    ) -> CBCommandStatus {
        // Process mouse enter/leave
        CBCommandStatus::NotUsed
    }

    /// Check if drawable is driving the context UI
    pub fn is_driving_context_ui(&self, drawable: &Drawable) -> bool {
        self.current_selected_drawable.as_ref()
            .map(|d| Arc::ptr_eq(d, &Arc::new(drawable.clone())))
            .unwrap_or(false)
    }

    /// Find a command button by name
    pub fn find_command_button(&self, name: &str) -> Option<&Arc<CommandButton>> {
        self.command_buttons.iter().find(|cb| cb.get_name() == name)
    }

    /// Find a command set by name
    pub fn find_command_set(&self, name: &str) -> Option<&Arc<CommandSet>> {
        self.command_sets.iter().find(|cs| cs.get_name() == name)
    }

    /// Switch to a different UI context
    fn switch_to_context(&mut self, context: ControlBarContext, drawable: Option<Arc<Drawable>>) {
        // Hide all context parents
        // Show the appropriate context parent
        // Populate the new context
        self.curr_context = context;
        self.current_selected_drawable = drawable;
    }

    /// Evaluate what context UI should be shown
    fn evaluate_context_ui(&mut self) {
        // Determine which context to show based on selection
        self.ui_dirty = false;
    }

    /// Reset common command data
    fn reset_common_command_data(&mut self) {
        self.common_commands = Default::default();
    }

    /// Reset container data
    fn reset_contain_data(&mut self) {
        self.contain_data = array_init::array_init(|_| ContainEntry::new());
    }

    /// Reset build queue data
    fn reset_build_queue_data(&mut self) {
        self.queue_data = array_init::array_init(|_| QueueEntry::new());
    }

    // Many more methods for population, updating contexts, etc.
    // These would be ported from the various ControlBar*.cpp files
}

// Global singleton instance
static CONTROL_BAR: OnceLock<Arc<Mutex<ControlBar>>> = OnceLock::new();

impl ControlBar {
    /// Get the global instance
    pub fn get_instance() -> Option<Arc<Mutex<ControlBar>>> {
        CONTROL_BAR.get().cloned()
    }

    /// Initialize the global instance
    pub fn initialize_instance() {
        CONTROL_BAR.get_or_init(|| Arc::new(Mutex::new(ControlBar::new())));
    }
}

// Placeholder types
pub use super::scheme::{ICoord2D, Color};
pub struct GameWindow;
pub struct Drawable;
pub struct WindowLayout;
pub struct DisplayString;
pub struct WindowVideoManager;
pub struct AnimateWindowManager;
pub struct Image;
pub struct Player;
pub struct PlayerTemplate;
pub struct AudioEventRTS;

pub type DrawableID = u32;
pub type ObjectID = u32;
pub type ProductionID = u32;

pub const INVALID_DRAWABLE_ID: DrawableID = 0xFFFFFFFF;
pub const INVALID_OBJECT_ID: ObjectID = 0xFFFFFFFF;
pub const INVALID_PRODUCTION_ID: ProductionID = 0xFFFFFFFF;

#[derive(Clone, Copy, Debug)]
pub enum ProductionType {
    None,
    Unit,
    Upgrade,
}

#[derive(Clone, Copy, Debug)]
pub enum GadgetGameMessage {
    Selected,
    SelectedRight,
    MouseEntering,
    MouseLeaving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideSelectState {
    None,
    State1,
    State2,
    State3,
    State4,
    State5,
    State6,
}

pub type ScienceType = u32;

#[cfg(any(feature = "internal", debug_assertions))]
fn get_game_frame() -> u32 {
    0 // Placeholder
}

impl Clone for Drawable {
    fn clone(&self) -> Self {
        Self
    }
}
