//! ProgressBar UI Gadget
//!
//! Visual progress indicator with percentage display.

use super::*;

/// ProgressBar orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarOrientation {
    Horizontal,
    Vertical,
}

/// ProgressBar style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarStyle {
    /// Solid fill
    Solid,
    /// Striped pattern
    Striped,
    /// Animated stripes
    AnimatedStripes,
    /// Gradient fill
    Gradient,
}

/// ProgressBar configuration
#[derive(Debug, Clone)]
pub struct ProgressBarConfig {
    pub orientation: ProgressBarOrientation,
    pub style: ProgressBarStyle,
    pub fill_color: Color,
    pub background_color: Color,
    pub border_color: Color,
    pub text_color: Color,
    pub show_percentage: bool,
    pub show_text: bool,
    pub animate: bool,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            orientation: ProgressBarOrientation::Horizontal,
            style: ProgressBarStyle::Solid,
            fill_color: Color::rgb(50, 150, 250),
            background_color: Color::rgb(200, 200, 200),
            border_color: Color::rgb(100, 100, 100),
            text_color: Color::BLACK,
            show_percentage: true,
            show_text: false,
            animate: false,
        }
    }
}

/// ProgressBar gadget
pub struct ProgressBar {
    id: GadgetId,
    bounds: Rect,
    state: GadgetState,
    enabled: bool,
    visible: bool,
    value: f32, // 0.0 to 1.0
    min_value: f32,
    max_value: f32,
    config: ProgressBarConfig,
    text: String,
    tooltip: Option<String>,
    animation_offset: f32,
}

impl ProgressBar {
    /// Create a new progress bar
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            bounds: Rect::new(x, y, width, height),
            state: GadgetState::Normal,
            enabled: true,
            visible: true,
            value: 0.0,
            min_value: 0.0,
            max_value: 1.0,
            config: ProgressBarConfig::default(),
            text: String::new(),
            tooltip: None,
            animation_offset: 0.0,
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: ProgressBarConfig) -> Self {
        self.config = config;
        self
    }

    /// Set value (0.0 to 1.0)
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    /// Get value
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Set value with custom range
    pub fn set_value_range(&mut self, value: f32, min: f32, max: f32) {
        self.min_value = min;
        self.max_value = max;
        let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
        self.value = normalized;
    }

    /// Get percentage (0-100)
    pub fn percentage(&self) -> f32 {
        self.value * 100.0
    }

    /// Set percentage (0-100)
    pub fn set_percentage(&mut self, percentage: f32) {
        if !(0.0..=100.0).contains(&percentage) {
            return;
        }

        self.set_value(percentage / 100.0);
    }

    /// Set progress percentage (0-100), legacy naming for UI callers
    pub fn set_progress(&mut self, percentage: f32) {
        self.set_percentage(percentage);
    }

    /// Set custom text
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Get text
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set orientation
    pub fn with_orientation(mut self, orientation: ProgressBarOrientation) -> Self {
        self.config.orientation = orientation;
        self
    }

    /// Set style
    pub fn with_style(mut self, style: ProgressBarStyle) -> Self {
        self.config.style = style;
        self
    }

    /// Set colors
    pub fn with_colors(mut self, fill: Color, background: Color) -> Self {
        self.config.fill_color = fill;
        self.config.background_color = background;
        self
    }

    /// Show/hide percentage
    pub fn with_percentage(mut self, show: bool) -> Self {
        self.config.show_percentage = show;
        self
    }

    /// Enable animation
    pub fn with_animation(mut self, animate: bool) -> Self {
        self.config.animate = animate;
        self
    }

    /// Render the progress bar
    #[allow(unused_variables)]
    fn render_progressbar(&self, theme: &GadgetTheme) {
        // Render background
        // [Background rendering code]

        // Calculate fill size based on orientation
        let (fill_width, fill_height) = match self.config.orientation {
            ProgressBarOrientation::Horizontal => (
                (self.bounds.width as f32 * self.value) as u32,
                self.bounds.height,
            ),
            ProgressBarOrientation::Vertical => (
                self.bounds.width,
                (self.bounds.height as f32 * self.value) as u32,
            ),
        };

        // Render fill based on style
        match self.config.style {
            ProgressBarStyle::Solid => {
                // Solid fill rendering
                // [Fill rendering code]
            }
            ProgressBarStyle::Striped | ProgressBarStyle::AnimatedStripes => {
                // Striped pattern rendering
                let stripe_width = 10;
                let offset = if self.config.style == ProgressBarStyle::AnimatedStripes {
                    self.animation_offset as i32
                } else {
                    0
                };
                // [Striped rendering code]
            }
            ProgressBarStyle::Gradient => {
                // Gradient fill rendering
                // [Gradient rendering code]
            }
        }

        // Render border
        // [Border rendering code]

        // Render text/percentage
        if self.config.show_percentage || self.config.show_text {
            let (center_x, center_y) = self.bounds.center();

            let display_text = if self.config.show_text && !self.text.is_empty() {
                if self.config.show_percentage {
                    format!("{} - {:.0}%", self.text, self.percentage())
                } else {
                    self.text.clone()
                }
            } else if self.config.show_percentage {
                format!("{:.0}%", self.percentage())
            } else {
                String::new()
            };

            // [Text rendering code]
        }
    }
}

impl Gadget for ProgressBar {
    fn id(&self) -> GadgetId {
        self.id
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
    }

    fn set_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn state(&self) -> GadgetState {
        self.state
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn can_focus(&self) -> bool {
        false // Progress bars typically don't receive focus
    }

    fn has_focus(&self) -> bool {
        false
    }

    fn set_focus(&mut self, _focused: bool) {
        // Progress bars don't receive focus
    }

    fn handle_input(&mut self, _event: &InputEvent) -> Vec<GadgetMessage> {
        Vec::new() // Progress bars don't handle input
    }

    fn update(&mut self, delta_time: f32) {
        if self.config.animate && self.config.style == ProgressBarStyle::AnimatedStripes {
            self.animation_offset += delta_time * 50.0; // Animation speed
            if self.animation_offset > 20.0 {
                self.animation_offset -= 20.0;
            }
        }
    }

    fn render(&self, theme: &GadgetTheme) {
        if !self.visible {
            return;
        }

        self.render_progressbar(theme);
    }

    fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}

/// Builder for creating progress bars
pub struct ProgressBarBuilder {
    id: GadgetId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    config: ProgressBarConfig,
    value: f32,
    text: Option<String>,
}

impl ProgressBarBuilder {
    pub fn new(id: GadgetId, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            x,
            y,
            width,
            height,
            config: ProgressBarConfig::default(),
            value: 0.0,
            text: None,
        }
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self.config.show_text = true;
        self
    }

    pub fn style(mut self, style: ProgressBarStyle) -> Self {
        self.config.style = style;
        self
    }

    pub fn orientation(mut self, orientation: ProgressBarOrientation) -> Self {
        self.config.orientation = orientation;
        self
    }

    pub fn animate(mut self, animate: bool) -> Self {
        self.config.animate = animate;
        self
    }

    pub fn build(self) -> ProgressBar {
        let mut bar = ProgressBar::new(self.id, self.x, self.y, self.width, self.height)
            .with_config(self.config);

        bar.set_value(self.value);

        if let Some(text) = self.text {
            bar.text = text;
        }

        bar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progressbar_creation() {
        let bar = ProgressBar::new(1, 10, 20, 200, 20);
        assert_eq!(bar.value(), 0.0);
        assert_eq!(bar.percentage(), 0.0);
    }

    #[test]
    fn test_set_value() {
        let mut bar = ProgressBar::new(1, 10, 20, 200, 20);
        bar.set_value(0.5);
        assert_eq!(bar.value(), 0.5);
        assert_eq!(bar.percentage(), 50.0);
    }

    #[test]
    fn test_set_percentage() {
        let mut bar = ProgressBar::new(1, 10, 20, 200, 20);
        bar.set_percentage(75.0);
        assert_eq!(bar.value(), 0.75);
        assert_eq!(bar.percentage(), 75.0);
    }

    #[test]
    fn test_set_progress_ignores_out_of_range_like_cpp() {
        let mut bar = ProgressBar::new(1, 10, 20, 200, 20);
        bar.set_progress(40.0);
        assert_eq!(bar.percentage(), 40.0);

        bar.set_progress(-1.0);
        assert_eq!(bar.percentage(), 40.0);

        bar.set_progress(101.0);
        assert_eq!(bar.percentage(), 40.0);

        bar.set_progress(100.0);
        assert_eq!(bar.percentage(), 100.0);
    }

    #[test]
    fn test_value_clamping() {
        let mut bar = ProgressBar::new(1, 10, 20, 200, 20);
        bar.set_value(1.5); // Over max
        assert_eq!(bar.value(), 1.0);

        bar.set_value(-0.5); // Under min
        assert_eq!(bar.value(), 0.0);
    }

    #[test]
    fn test_custom_range() {
        let mut bar = ProgressBar::new(1, 10, 20, 200, 20);
        bar.set_value_range(50.0, 0.0, 100.0);
        assert_eq!(bar.value(), 0.5);
        assert_eq!(bar.percentage(), 50.0);
    }
}
