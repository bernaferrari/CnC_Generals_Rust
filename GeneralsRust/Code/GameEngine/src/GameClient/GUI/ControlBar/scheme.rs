// FILE: scheme.rs
// Port of ControlBarScheme classes from C++
// Original: ControlBarScheme.h and ControlBarScheme.cpp

use std::sync::Arc;
use std::collections::VecDeque;

pub const MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS: usize = 6;
pub const CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS: usize = 3;

/// Control Bar Scheme Image
/// Holds the images the control bar will draw
#[derive(Clone)]
pub struct ControlBarSchemeImage {
    /// Name of the image
    pub name: String,

    /// Position to draw it at
    pub position: ICoord2D,

    /// Size of the image when drawn
    pub size: ICoord2D,

    /// Actual pointer to the mapped image
    pub image: Option<Arc<Image>>,

    /// Layer depth (0-5, where 0 is on top)
    /// Layers 0-2: foreground draw
    /// Layers 3-5: background draw
    pub layer: i32,
}

impl ControlBarSchemeImage {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            position: ICoord2D { x: 0, y: 0 },
            size: ICoord2D { x: 0, y: 0 },
            image: None,
            layer: 0,
        }
    }
}

impl Default for ControlBarSchemeImage {
    fn default() -> Self {
        Self::new()
    }
}

/// Control Bar Scheme Animation Types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationType {
    SlideRight = 0,
    Max,
}

impl AnimationType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "SLIDE_RIGHT" => Some(Self::SlideRight),
            _ => None,
        }
    }
}

/// Control Bar Scheme Animation
/// Information needed for control bar animations
pub struct ControlBarSchemeAnimation {
    /// Animation name
    pub name: String,

    /// Type of animation
    pub anim_type: AnimationType,

    /// Pointer to the image this animation acts on
    pub anim_image: Option<Arc<ControlBarSchemeImage>>,

    /// Duration of animation in game frames
    pub anim_duration: u32,

    /// Final position when animation completes
    pub final_pos: ICoord2D,

    /// Start position (set when animation begins)
    start_pos: ICoord2D,

    /// Current frame (0 to anim_duration)
    current_frame: u32,
}

impl ControlBarSchemeAnimation {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            anim_type: AnimationType::SlideRight,
            anim_image: None,
            anim_duration: 0,
            final_pos: ICoord2D { x: 0, y: 0 },
            start_pos: ICoord2D { x: 0, y: 0 },
            current_frame: 0,
        }
    }

    pub fn get_current_frame(&self) -> u32 {
        self.current_frame
    }

    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    pub fn get_start_pos(&self) -> ICoord2D {
        self.start_pos
    }

    pub fn set_start_pos(&mut self, pos: ICoord2D) {
        self.start_pos = pos;
    }
}

impl Default for ControlBarSchemeAnimation {
    fn default() -> Self {
        Self::new()
    }
}

/// Control Bar Scheme
/// Contains all information about a visual scheme
pub struct ControlBarScheme {
    /// Scheme name
    pub name: String,

    /// Screen resolution this scheme was created for
    pub screen_creation_res: ICoord2D,

    /// Faction type this command bar was made for
    pub side: String,

    /// Queue button image
    pub button_queue_image: Option<Arc<Image>>,

    /// Right HUD image
    pub right_hud_image: Option<Arc<Image>>,

    /// Build up clock color
    pub build_up_clock_color: Color,

    /// Border colors for different button types
    pub border_build_color: Color,
    pub border_action_color: Color,
    pub border_upgrade_color: Color,
    pub border_system_color: Color,

    /// Command bar border color
    pub command_bar_border_color: Color,

    /// Various button images
    pub options_button_enable: Option<Arc<Image>>,
    pub options_button_highlighted: Option<Arc<Image>>,
    pub options_button_pushed: Option<Arc<Image>>,
    pub options_button_disabled: Option<Arc<Image>>,

    pub idle_worker_button_enable: Option<Arc<Image>>,
    pub idle_worker_button_highlighted: Option<Arc<Image>>,
    pub idle_worker_button_pushed: Option<Arc<Image>>,
    pub idle_worker_button_disabled: Option<Arc<Image>>,

    pub buddy_button_enable: Option<Arc<Image>>,
    pub buddy_button_highlighted: Option<Arc<Image>>,
    pub buddy_button_pushed: Option<Arc<Image>>,
    pub buddy_button_disabled: Option<Arc<Image>>,

    pub beacon_button_enable: Option<Arc<Image>>,
    pub beacon_button_highlighted: Option<Arc<Image>>,
    pub beacon_button_pushed: Option<Arc<Image>>,
    pub beacon_button_disabled: Option<Arc<Image>>,

    pub gen_bar_button_in: Option<Arc<Image>>,
    pub gen_bar_button_on: Option<Arc<Image>>,

    pub toggle_button_up_in: Option<Arc<Image>>,
    pub toggle_button_up_on: Option<Arc<Image>>,
    pub toggle_button_up_pushed: Option<Arc<Image>>,
    pub toggle_button_down_in: Option<Arc<Image>>,
    pub toggle_button_down_on: Option<Arc<Image>>,
    pub toggle_button_down_pushed: Option<Arc<Image>>,

    pub general_button_enable: Option<Arc<Image>>,
    pub general_button_highlighted: Option<Arc<Image>>,
    pub general_button_pushed: Option<Arc<Image>>,
    pub general_button_disabled: Option<Arc<Image>>,

    pub u_attack_button_enable: Option<Arc<Image>>,
    pub u_attack_button_highlighted: Option<Arc<Image>>,
    pub u_attack_button_pushed: Option<Arc<Image>>,

    pub min_max_button_enable: Option<Arc<Image>>,
    pub min_max_button_highlighted: Option<Arc<Image>>,
    pub min_max_button_pushed: Option<Arc<Image>>,

    pub gen_arrow: Option<Arc<Image>>,

    /// UI element coordinates
    pub money_ul: ICoord2D,
    pub money_lr: ICoord2D,
    pub min_max_ul: ICoord2D,
    pub min_max_lr: ICoord2D,
    pub general_ul: ICoord2D,
    pub general_lr: ICoord2D,
    pub u_attack_ul: ICoord2D,
    pub u_attack_lr: ICoord2D,
    pub options_ul: ICoord2D,
    pub options_lr: ICoord2D,
    pub worker_ul: ICoord2D,
    pub worker_lr: ICoord2D,
    pub chat_ul: ICoord2D,
    pub chat_lr: ICoord2D,
    pub beacon_ul: ICoord2D,
    pub beacon_lr: ICoord2D,
    pub power_bar_ul: ICoord2D,
    pub power_bar_lr: ICoord2D,

    /// Other images
    pub exp_bar_foreground: Option<Arc<Image>>,
    pub command_marker_image: Option<Arc<Image>>,
    pub power_purchase_image: Option<Arc<Image>>,

    /// Image layers (6 layers, 0-2 foreground, 3-5 background)
    pub layers: [Vec<Arc<ControlBarSchemeImage>>; MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS],

    /// Animations
    pub animations: Vec<Arc<ControlBarSchemeAnimation>>,
}

impl ControlBarScheme {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            screen_creation_res: ICoord2D { x: 0, y: 0 },
            side: String::new(),
            button_queue_image: None,
            right_hud_image: None,
            build_up_clock_color: Color::new(0, 0, 0, 100),
            border_build_color: Color::undefined(),
            border_action_color: Color::undefined(),
            border_upgrade_color: Color::undefined(),
            border_system_color: Color::undefined(),
            command_bar_border_color: Color::new(0, 0, 0, 100),
            options_button_enable: None,
            options_button_highlighted: None,
            options_button_pushed: None,
            options_button_disabled: None,
            idle_worker_button_enable: None,
            idle_worker_button_highlighted: None,
            idle_worker_button_pushed: None,
            idle_worker_button_disabled: None,
            buddy_button_enable: None,
            buddy_button_highlighted: None,
            buddy_button_pushed: None,
            buddy_button_disabled: None,
            beacon_button_enable: None,
            beacon_button_highlighted: None,
            beacon_button_pushed: None,
            beacon_button_disabled: None,
            gen_bar_button_in: None,
            gen_bar_button_on: None,
            toggle_button_up_in: None,
            toggle_button_up_on: None,
            toggle_button_up_pushed: None,
            toggle_button_down_in: None,
            toggle_button_down_on: None,
            toggle_button_down_pushed: None,
            general_button_enable: None,
            general_button_highlighted: None,
            general_button_pushed: None,
            general_button_disabled: None,
            u_attack_button_enable: None,
            u_attack_button_highlighted: None,
            u_attack_button_pushed: None,
            min_max_button_enable: None,
            min_max_button_highlighted: None,
            min_max_button_pushed: None,
            gen_arrow: None,
            money_ul: ICoord2D { x: 0, y: 0 },
            money_lr: ICoord2D { x: 0, y: 0 },
            min_max_ul: ICoord2D { x: 0, y: 0 },
            min_max_lr: ICoord2D { x: 0, y: 0 },
            general_ul: ICoord2D { x: 0, y: 0 },
            general_lr: ICoord2D { x: 0, y: 0 },
            u_attack_ul: ICoord2D { x: 0, y: 0 },
            u_attack_lr: ICoord2D { x: 0, y: 0 },
            options_ul: ICoord2D { x: 0, y: 0 },
            options_lr: ICoord2D { x: 0, y: 0 },
            worker_ul: ICoord2D { x: 0, y: 0 },
            worker_lr: ICoord2D { x: 0, y: 0 },
            chat_ul: ICoord2D { x: 0, y: 0 },
            chat_lr: ICoord2D { x: 0, y: 0 },
            beacon_ul: ICoord2D { x: 0, y: 0 },
            beacon_lr: ICoord2D { x: 0, y: 0 },
            power_bar_ul: ICoord2D { x: 0, y: 0 },
            power_bar_lr: ICoord2D { x: 0, y: 0 },
            exp_bar_foreground: None,
            command_marker_image: None,
            power_purchase_image: None,
            layers: Default::default(),
            animations: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        // Initialize scheme
    }

    pub fn update(&mut self) {
        // Update animations
        for anim in &mut self.animations {
            // Update each animation based on its type
            // This would call specific animation update functions
        }
    }

    pub fn draw_foreground(&self, multi: Coord2D, offset: ICoord2D) {
        // Draw foreground layers (0-2)
        for layer_idx in 0..CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS {
            for image in &self.layers[layer_idx] {
                if let Some(img) = &image.image {
                    // Draw the image at the appropriate position
                    // Implementation would call graphics system
                }
            }
        }
    }

    pub fn draw_background(&self, multi: Coord2D, offset: ICoord2D) {
        // Draw background layers (3-5)
        for layer_idx in CONTROL_BAR_SCHEME_FOREGROUND_IMAGE_LAYERS..MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS {
            for image in &self.layers[layer_idx] {
                if let Some(img) = &image.image {
                    // Draw the image at the appropriate position
                    // Implementation would call graphics system
                }
            }
        }
    }

    pub fn reset(&mut self) {
        // Clear all layers
        for layer in &mut self.layers {
            layer.clear();
        }

        // Clear animations
        self.animations.clear();

        // Reset other fields to defaults
        *self = Self::new();
    }

    pub fn add_animation(&mut self, anim: Arc<ControlBarSchemeAnimation>) {
        self.animations.push(anim);
    }

    pub fn add_image(&mut self, image: Arc<ControlBarSchemeImage>) {
        let layer_idx = image.layer.max(0).min((MAX_CONTROL_BAR_SCHEME_IMAGE_LAYERS - 1) as i32) as usize;
        self.layers[layer_idx].push(image);
    }

    pub fn update_anim(&mut self, anim: &mut ControlBarSchemeAnimation) {
        match anim.anim_type {
            AnimationType::SlideRight => {
                // Implement slide right animation
                anim_slide_right(anim);
            },
            _ => {}
        }
    }
}

impl Default for ControlBarScheme {
    fn default() -> Self {
        Self::new()
    }
}

/// Slide right animation implementation
fn anim_slide_right(anim: &mut ControlBarSchemeAnimation) {
    if anim.current_frame < anim.anim_duration {
        anim.current_frame += 1;

        if let Some(image) = &anim.anim_image {
            // Calculate new position based on interpolation
            let progress = anim.current_frame as f32 / anim.anim_duration as f32;
            // Linear interpolation from start to final position
            // This would update the image's position
        }
    }
}

/// Control Bar Scheme Manager
/// Manages all control bar schemes and the currently active one
pub struct ControlBarSchemeManager {
    /// Current active scheme
    current_scheme: Option<Arc<ControlBarScheme>>,

    /// Multiplier for positioning
    multiplier: Coord2D,

    /// List of all available schemes
    scheme_list: Vec<Arc<ControlBarScheme>>,
}

impl ControlBarSchemeManager {
    pub fn new() -> Self {
        Self {
            current_scheme: None,
            multiplier: Coord2D { x: 1.0, y: 1.0 },
            scheme_list: Vec::new(),
        }
    }

    pub fn init(&mut self) {
        // Load schemes from INI files
        // This would parse ControlBarScheme.ini
    }

    pub fn update(&mut self) {
        if let Some(scheme) = &self.current_scheme {
            // Update current scheme (animations, etc.)
        }
    }

    pub fn draw_foreground(&self, offset: ICoord2D) {
        if let Some(scheme) = &self.current_scheme {
            scheme.draw_foreground(self.multiplier, offset);
        }
    }

    pub fn draw_background(&self, offset: ICoord2D) {
        if let Some(scheme) = &self.current_scheme {
            scheme.draw_background(self.multiplier, offset);
        }
    }

    pub fn set_control_bar_scheme_by_player(&mut self, player: &Player) {
        if let Some(template) = player.get_player_template() {
            self.set_control_bar_scheme_by_player_template(template, false);
        }
    }

    pub fn set_control_bar_scheme_by_player_template(
        &mut self,
        template: &PlayerTemplate,
        use_small: bool
    ) {
        // Find and set the appropriate scheme based on player template
        // This would look up the scheme by faction name
    }

    pub fn set_control_bar_scheme(&mut self, scheme_name: &str) {
        if let Some(scheme) = self.find_control_bar_scheme(scheme_name) {
            self.current_scheme = Some(Arc::clone(scheme));
        }
    }

    pub fn find_control_bar_scheme(&self, name: &str) -> Option<&Arc<ControlBarScheme>> {
        self.scheme_list.iter().find(|s| s.name == name)
    }

    pub fn new_control_bar_scheme(&mut self, name: String) -> Arc<ControlBarScheme> {
        let scheme = Arc::new(ControlBarScheme {
            name: name.clone(),
            ..Default::default()
        });
        self.scheme_list.push(Arc::clone(&scheme));
        scheme
    }

    pub fn preload_assets(&self, time_of_day: TimeOfDay) {
        // Preload all images and assets for the schemes
    }
}

impl Default for ControlBarSchemeManager {
    fn default() -> Self {
        Self::new()
    }
}

// Placeholder types
#[derive(Clone, Copy, Debug, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn undefined() -> Self {
        Self { r: 255, g: 255, b: 255, a: 255 }
    }
}

pub struct Image;
pub struct Player;
pub struct PlayerTemplate;

#[derive(Clone, Copy, Debug)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
    Night,
}

impl Player {
    pub fn get_player_template(&self) -> Option<&PlayerTemplate> {
        None
    }
}
