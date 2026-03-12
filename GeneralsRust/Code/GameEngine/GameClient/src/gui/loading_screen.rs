//! Loading Screen System
//!
//! Provides loading screen management with progress tracking, tips display,
//! and background rendering during game loading operations.

use super::gadgets::{ProgressBar, ProgressBarBuilder, ProgressBarStyle};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Loading stage
#[derive(Debug, Clone, PartialEq)]
pub struct LoadingStage {
    pub id: String,
    pub name: String,
    pub weight: f32, // Relative weight for progress calculation
}

impl LoadingStage {
    pub fn new(id: impl Into<String>, name: impl Into<String>, weight: f32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            weight,
        }
    }
}

/// Loading tip
#[derive(Debug, Clone)]
pub struct LoadingTip {
    pub text: String,
    pub display_duration: Duration,
}

impl LoadingTip {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            display_duration: Duration::from_secs(5),
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.display_duration = duration;
        self
    }
}

/// Loading screen state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingState {
    /// Loading screen is hidden
    Hidden,
    /// Loading screen is visible and active
    Active,
    /// Loading is complete, fading out
    FadingOut,
}

/// Loading screen configuration
#[derive(Debug, Clone)]
pub struct LoadingScreenConfig {
    /// Background image path
    pub background_image: Option<String>,

    /// Background color if no image
    pub background_color: [f32; 4],

    /// Show tips
    pub show_tips: bool,

    /// Show progress bar
    pub show_progress: bool,

    /// Show loading stage name
    pub show_stage_name: bool,

    /// Fade in duration
    pub fade_in_duration: Duration,

    /// Fade out duration
    pub fade_out_duration: Duration,

    /// Minimum display time (prevents flash)
    pub minimum_display_time: Duration,

    /// Tip rotation interval
    pub tip_rotation_interval: Duration,
}

impl Default for LoadingScreenConfig {
    fn default() -> Self {
        Self {
            background_image: None,
            background_color: [0.1, 0.1, 0.1, 1.0],
            show_tips: true,
            show_progress: true,
            show_stage_name: true,
            fade_in_duration: Duration::from_millis(300),
            fade_out_duration: Duration::from_millis(500),
            minimum_display_time: Duration::from_millis(500),
            tip_rotation_interval: Duration::from_secs(5),
        }
    }
}

/// Loading screen
pub struct LoadingScreen {
    config: LoadingScreenConfig,
    state: LoadingState,
    stages: Vec<LoadingStage>,
    current_stage_index: usize,
    stage_progress: f32, // 0.0 to 1.0 for current stage
    tips: VecDeque<LoadingTip>,
    current_tip_index: usize,
    progress_bar: Option<ProgressBar>,
    fade_alpha: f32,
    display_start_time: Option<Instant>,
    last_tip_change: Instant,
}

impl LoadingScreen {
    /// Create a new loading screen
    pub fn new(config: LoadingScreenConfig) -> Self {
        Self {
            config,
            state: LoadingState::Hidden,
            stages: Vec::new(),
            current_stage_index: 0,
            stage_progress: 0.0,
            tips: VecDeque::new(),
            current_tip_index: 0,
            progress_bar: None,
            fade_alpha: 0.0,
            display_start_time: None,
            last_tip_change: Instant::now(),
        }
    }

    /// Add loading stage
    pub fn add_stage(&mut self, stage: LoadingStage) {
        self.stages.push(stage);
    }

    /// Add multiple stages
    pub fn add_stages(&mut self, stages: Vec<LoadingStage>) {
        self.stages.extend(stages);
    }

    /// Add loading tip
    pub fn add_tip(&mut self, tip: LoadingTip) {
        self.tips.push_back(tip);
    }

    /// Add multiple tips
    pub fn add_tips(&mut self, tips: Vec<LoadingTip>) {
        self.tips.extend(tips);
    }

    /// Show loading screen
    pub fn show(&mut self) {
        self.state = LoadingState::Active;
        self.current_stage_index = 0;
        self.stage_progress = 0.0;
        self.fade_alpha = 0.0;
        self.display_start_time = Some(Instant::now());
        self.last_tip_change = Instant::now();
    }

    /// Hide loading screen
    pub fn hide(&mut self) {
        // Check minimum display time
        if let Some(start_time) = self.display_start_time {
            if start_time.elapsed() < self.config.minimum_display_time {
                return; // Don't hide yet
            }
        }

        self.state = LoadingState::FadingOut;
    }

    /// Get current state
    pub fn state(&self) -> LoadingState {
        self.state
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        matches!(self.state, LoadingState::Active | LoadingState::FadingOut)
    }

    /// Set current stage by ID
    pub fn set_stage(&mut self, stage_id: &str) -> bool {
        if let Some(index) = self.stages.iter().position(|s| s.id == stage_id) {
            self.current_stage_index = index;
            self.stage_progress = 0.0;
            true
        } else {
            false
        }
    }

    /// Set current stage by index
    pub fn set_stage_index(&mut self, index: usize) {
        if index < self.stages.len() {
            self.current_stage_index = index;
            self.stage_progress = 0.0;
        }
    }

    /// Set progress for current stage (0.0 to 1.0)
    pub fn set_stage_progress(&mut self, progress: f32) {
        self.stage_progress = progress.clamp(0.0, 1.0);
    }

    /// Advance to next stage
    pub fn next_stage(&mut self) -> bool {
        if self.current_stage_index + 1 < self.stages.len() {
            self.current_stage_index += 1;
            self.stage_progress = 0.0;
            true
        } else {
            false
        }
    }

    /// Get current stage
    pub fn current_stage(&self) -> Option<&LoadingStage> {
        self.stages.get(self.current_stage_index)
    }

    /// Get total progress (0.0 to 1.0)
    pub fn total_progress(&self) -> f32 {
        if self.stages.is_empty() {
            return 1.0;
        }

        let total_weight: f32 = self.stages.iter().map(|s| s.weight).sum();
        if total_weight == 0.0 {
            return 0.0;
        }

        let mut completed_weight = 0.0;
        for (i, stage) in self.stages.iter().enumerate() {
            if i < self.current_stage_index {
                completed_weight += stage.weight;
            } else if i == self.current_stage_index {
                completed_weight += stage.weight * self.stage_progress;
            }
        }

        completed_weight / total_weight
    }

    /// Get current tip
    pub fn current_tip(&self) -> Option<&str> {
        self.tips
            .get(self.current_tip_index)
            .map(|t| t.text.as_str())
    }

    /// Update loading screen
    pub fn update(&mut self, delta_time: f32) {
        match self.state {
            LoadingState::Hidden => {}
            LoadingState::Active => {
                // Fade in
                let fade_in_secs = self.config.fade_in_duration.as_secs_f32();
                if fade_in_secs > 0.0 {
                    self.fade_alpha = (self.fade_alpha + delta_time / fade_in_secs).min(1.0);
                } else {
                    self.fade_alpha = 1.0;
                }

                // Rotate tips
                if self.config.show_tips && !self.tips.is_empty() {
                    let elapsed = self.last_tip_change.elapsed();
                    if elapsed >= self.config.tip_rotation_interval {
                        self.current_tip_index = (self.current_tip_index + 1) % self.tips.len();
                        self.last_tip_change = Instant::now();
                    }
                }
            }
            LoadingState::FadingOut => {
                // Fade out
                let fade_out_secs = self.config.fade_out_duration.as_secs_f32();
                if fade_out_secs > 0.0 {
                    self.fade_alpha = (self.fade_alpha - delta_time / fade_out_secs).max(0.0);
                } else {
                    self.fade_alpha = 0.0;
                }

                // Fully faded out
                if self.fade_alpha <= 0.0 {
                    self.state = LoadingState::Hidden;
                    self.display_start_time = None;
                }
            }
        }

        // Update progress bar
        let total_progress = self.total_progress();
        if let Some(progress_bar) = &mut self.progress_bar {
            progress_bar.set_value(total_progress);
        }
    }

    /// Get fade alpha (0.0 to 1.0)
    pub fn fade_alpha(&self) -> f32 {
        self.fade_alpha
    }

    /// Initialize progress bar
    pub fn create_progress_bar(&mut self, gadget_id: u32, x: i32, y: i32, width: u32, height: u32) {
        self.progress_bar = Some(
            ProgressBarBuilder::new(gadget_id, x, y, width, height)
                .style(ProgressBarStyle::AnimatedStripes)
                .animate(true)
                .build(),
        );
    }

    /// Get progress bar
    pub fn progress_bar(&self) -> Option<&ProgressBar> {
        self.progress_bar.as_ref()
    }

    /// Get mutable progress bar
    pub fn progress_bar_mut(&mut self) -> Option<&mut ProgressBar> {
        self.progress_bar.as_mut()
    }

    /// Render loading screen.
    pub fn render(&self) {
        if !self.is_visible() {
            return;
        }

        // Apply fade alpha to all rendered elements
        let alpha = self.fade_alpha;

        // Render background
        // [Background rendering code]

        // Render stage name
        if self.config.show_stage_name {
            if let Some(stage) = self.current_stage() {
                // [Stage name rendering code]
            }
        }

        // Render progress bar
        if self.config.show_progress {
            if let Some(progress_bar) = &self.progress_bar {
                // [Progress bar rendering with alpha]
            }
        }

        // Render tip
        if self.config.show_tips {
            if let Some(tip) = self.current_tip() {
                // [Tip rendering code]
            }
        }
    }

    /// Reset loading screen
    pub fn reset(&mut self) {
        self.state = LoadingState::Hidden;
        self.current_stage_index = 0;
        self.stage_progress = 0.0;
        self.current_tip_index = 0;
        self.fade_alpha = 0.0;
        self.display_start_time = None;
    }
}

/// Predefined loading stages for common game loading operations
pub mod stages {
    use super::*;

    pub fn initialization() -> LoadingStage {
        LoadingStage::new("init", "Initializing...", 0.1)
    }

    pub fn loading_assets() -> LoadingStage {
        LoadingStage::new("assets", "Loading assets...", 0.3)
    }

    pub fn loading_map() -> LoadingStage {
        LoadingStage::new("map", "Loading map...", 0.2)
    }

    pub fn loading_textures() -> LoadingStage {
        LoadingStage::new("textures", "Loading textures...", 0.15)
    }

    pub fn loading_models() -> LoadingStage {
        LoadingStage::new("models", "Loading models...", 0.15)
    }

    pub fn finalizing() -> LoadingStage {
        LoadingStage::new("finalize", "Finalizing...", 0.1)
    }

    /// Get standard loading stages for map loading
    pub fn standard_map_loading() -> Vec<LoadingStage> {
        vec![
            initialization(),
            loading_map(),
            loading_textures(),
            loading_models(),
            loading_assets(),
            finalizing(),
        ]
    }
}

/// Predefined loading tips
pub mod tips {
    use super::*;

    pub fn generals_tips() -> Vec<LoadingTip> {
        vec![
            LoadingTip::new("Build power plants to keep your base operational."),
            LoadingTip::new("Upgrade your command center to access more powerful units."),
            LoadingTip::new("Use the terrain to your advantage in combat."),
            LoadingTip::new("Garrison buildings with infantry to create defensive positions."),
            LoadingTip::new("Resource management is key to victory."),
            LoadingTip::new("Scout enemy positions before launching attacks."),
            LoadingTip::new("Combine different unit types for maximum effectiveness."),
            LoadingTip::new("Protect your supply lines to maintain your economy."),
            LoadingTip::new("Use generals' powers strategically at critical moments."),
            LoadingTip::new("Capture tech buildings for strategic advantages."),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_screen_creation() {
        let loading_screen = LoadingScreen::new(LoadingScreenConfig::default());
        assert_eq!(loading_screen.state(), LoadingState::Hidden);
        assert!(!loading_screen.is_visible());
    }

    #[test]
    fn test_show_hide() {
        let mut loading_screen = LoadingScreen::new(LoadingScreenConfig::default());

        loading_screen.show();
        assert_eq!(loading_screen.state(), LoadingState::Active);
        assert!(loading_screen.is_visible());

        loading_screen.hide();
        assert_eq!(loading_screen.state(), LoadingState::FadingOut);
        assert!(loading_screen.is_visible());
    }

    #[test]
    fn test_stages() {
        let mut loading_screen = LoadingScreen::new(LoadingScreenConfig::default());
        loading_screen.add_stage(LoadingStage::new("stage1", "Stage 1", 1.0));
        loading_screen.add_stage(LoadingStage::new("stage2", "Stage 2", 1.0));

        assert_eq!(loading_screen.total_progress(), 0.0);

        loading_screen.set_stage_progress(0.5);
        assert_eq!(loading_screen.total_progress(), 0.25); // 50% of first stage

        loading_screen.next_stage();
        assert_eq!(loading_screen.total_progress(), 0.5); // Completed first stage

        loading_screen.set_stage_progress(1.0);
        assert_eq!(loading_screen.total_progress(), 1.0); // Completed all stages
    }

    #[test]
    fn test_tips() {
        let mut loading_screen = LoadingScreen::new(LoadingScreenConfig::default());
        loading_screen.add_tip(LoadingTip::new("Tip 1"));
        loading_screen.add_tip(LoadingTip::new("Tip 2"));

        assert_eq!(loading_screen.current_tip(), Some("Tip 1"));
    }

    #[test]
    fn test_standard_stages() {
        let stages = stages::standard_map_loading();
        assert_eq!(stages.len(), 6);

        let total_weight: f32 = stages.iter().map(|s| s.weight).sum();
        assert!((total_weight - 1.0).abs() < 0.01); // Should sum to approximately 1.0
    }
}
