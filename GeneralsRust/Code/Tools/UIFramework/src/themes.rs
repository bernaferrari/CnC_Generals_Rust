//! Theme system for consistent UI styling

use crate::ThemeType;
use eframe::egui;

/// Theme manager for applying consistent styling
pub struct ThemeManager {
    current_theme: ThemeType,
}

impl ThemeManager {
    pub fn new(theme: ThemeType) -> Self {
        Self {
            current_theme: theme,
        }
    }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = self.get_visuals();
        ctx.set_visuals(visuals);

        // Apply custom fonts if needed
        self.apply_fonts(ctx);
    }

    pub fn set_theme(&mut self, theme: ThemeType) {
        self.current_theme = theme;
    }

    pub fn current_theme(&self) -> ThemeType {
        self.current_theme
    }

    fn get_visuals(&self) -> egui::Visuals {
        match self.current_theme {
            ThemeType::Dark => self.dark_theme(),
            ThemeType::Light => self.light_theme(),
            ThemeType::CnCClassic => self.cnc_classic_theme(),
            ThemeType::Modern => self.modern_theme(),
        }
    }

    fn dark_theme(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Customize dark theme
        visuals.panel_fill = egui::Color32::from_gray(30);
        visuals.window_fill = egui::Color32::from_gray(35);
        visuals.faint_bg_color = egui::Color32::from_gray(25);

        visuals
    }

    fn light_theme(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::light();

        // Customize light theme
        visuals.panel_fill = egui::Color32::from_gray(245);
        visuals.window_fill = egui::Color32::from_gray(250);

        visuals
    }

    fn cnc_classic_theme(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Command & Conquer classic green/amber color scheme
        let cnc_green = egui::Color32::from_rgb(0, 255, 0);
        let cnc_amber = egui::Color32::from_rgb(255, 200, 0);
        let cnc_dark = egui::Color32::from_rgb(10, 20, 10);

        visuals.override_text_color = Some(cnc_green);
        visuals.hyperlink_color = cnc_amber;
        visuals.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(0, 255, 0, 64);
        visuals.selection.stroke.color = cnc_green;

        visuals.panel_fill = cnc_dark;
        visuals.window_fill = egui::Color32::from_rgb(15, 25, 15);
        visuals.extreme_bg_color = egui::Color32::BLACK;
        visuals.faint_bg_color = egui::Color32::from_rgb(5, 15, 5);

        // Button styling
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(20, 40, 20);
        visuals.widgets.inactive.bg_stroke.color = cnc_green;
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(30, 60, 30);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(40, 80, 40);

        visuals
    }

    fn modern_theme(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Modern dark theme with blue accents
        let modern_blue = egui::Color32::from_rgb(64, 128, 255);
        let modern_dark = egui::Color32::from_rgb(25, 28, 35);
        let modern_darker = egui::Color32::from_rgb(20, 23, 30);

        visuals.selection.bg_fill = modern_blue;
        visuals.hyperlink_color = egui::Color32::from_rgb(100, 150, 255);

        visuals.panel_fill = modern_dark;
        visuals.window_fill = modern_darker;
        visuals.extreme_bg_color = egui::Color32::from_rgb(15, 18, 25);

        // Modern rounded corners
        visuals.widgets.noninteractive.corner_radius = egui::Rounding::same(6);
        visuals.widgets.inactive.corner_radius = egui::Rounding::same(6);
        visuals.widgets.hovered.corner_radius = egui::Rounding::same(6);
        visuals.widgets.active.corner_radius = egui::Rounding::same(6);
        visuals.widgets.open.corner_radius = egui::Rounding::same(6);

        visuals
    }

    fn apply_fonts(&self, ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        match self.current_theme {
            ThemeType::CnCClassic => {
                // Use monospace font for that retro feel
                fonts.families.insert(
                    egui::FontFamily::Proportional,
                    vec!["Consolas".to_owned(), "Courier New".to_owned()],
                );
            }
            _ => {
                // Use default fonts for other themes
            }
        }

        ctx.set_fonts(fonts);
    }
}

/// Predefined color palettes for different themes
pub struct ColorPalette;

impl ColorPalette {
    /// Get color palette for the given theme
    pub fn get_palette(theme: ThemeType) -> ThemePalette {
        match theme {
            ThemeType::Dark => ThemePalette {
                primary: egui::Color32::from_rgb(100, 149, 237),
                secondary: egui::Color32::from_rgb(70, 130, 180),
                success: egui::Color32::from_rgb(60, 179, 113),
                warning: egui::Color32::from_rgb(255, 165, 0),
                error: egui::Color32::from_rgb(220, 20, 60),
                background: egui::Color32::from_gray(30),
                surface: egui::Color32::from_gray(35),
                text: egui::Color32::from_gray(240),
                text_secondary: egui::Color32::from_gray(180),
            },
            ThemeType::Light => ThemePalette {
                primary: egui::Color32::from_rgb(25, 118, 210),
                secondary: egui::Color32::from_rgb(69, 90, 100),
                success: egui::Color32::from_rgb(46, 125, 50),
                warning: egui::Color32::from_rgb(255, 152, 0),
                error: egui::Color32::from_rgb(211, 47, 47),
                background: egui::Color32::from_gray(250),
                surface: egui::Color32::WHITE,
                text: egui::Color32::from_gray(33),
                text_secondary: egui::Color32::from_gray(117),
            },
            ThemeType::CnCClassic => ThemePalette {
                primary: egui::Color32::from_rgb(0, 255, 0),
                secondary: egui::Color32::from_rgb(255, 200, 0),
                success: egui::Color32::from_rgb(0, 255, 0),
                warning: egui::Color32::from_rgb(255, 200, 0),
                error: egui::Color32::from_rgb(255, 0, 0),
                background: egui::Color32::from_rgb(10, 20, 10),
                surface: egui::Color32::from_rgb(15, 25, 15),
                text: egui::Color32::from_rgb(0, 255, 0),
                text_secondary: egui::Color32::from_rgb(0, 200, 0),
            },
            ThemeType::Modern => ThemePalette {
                primary: egui::Color32::from_rgb(64, 128, 255),
                secondary: egui::Color32::from_rgb(108, 117, 125),
                success: egui::Color32::from_rgb(40, 167, 69),
                warning: egui::Color32::from_rgb(255, 193, 7),
                error: egui::Color32::from_rgb(220, 53, 69),
                background: egui::Color32::from_rgb(25, 28, 35),
                surface: egui::Color32::from_rgb(30, 33, 40),
                text: egui::Color32::from_gray(245),
                text_secondary: egui::Color32::from_gray(200),
            },
        }
    }
}

/// Color palette for a theme
#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub primary: egui::Color32,
    pub secondary: egui::Color32,
    pub success: egui::Color32,
    pub warning: egui::Color32,
    pub error: egui::Color32,
    pub background: egui::Color32,
    pub surface: egui::Color32,
    pub text: egui::Color32,
    pub text_secondary: egui::Color32,
}

/// Helper for drawing themed UI elements
pub struct ThemeHelper;

impl ThemeHelper {
    /// Draw a status indicator with the appropriate color
    pub fn status_indicator(ui: &mut egui::Ui, status: StatusType, text: &str) {
        let palette = ColorPalette::get_palette(ThemeType::Dark); // TODO: Get current theme

        let color = match status {
            StatusType::Success => palette.success,
            StatusType::Warning => palette.warning,
            StatusType::Error => palette.error,
            StatusType::Info => palette.primary,
        };

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());

            ui.painter().circle_filled(rect.center(), 6.0, color);

            ui.label(egui::RichText::new(text).color(color));
        });
    }

    /// Draw a themed separator
    pub fn themed_separator(ui: &mut egui::Ui) {
        ui.separator();
    }

    /// Draw a themed header
    pub fn header(ui: &mut egui::Ui, text: &str) {
        ui.heading(egui::RichText::new(text).strong());
        ui.separator();
    }
}

/// Status types for themed indicators
#[derive(Debug, Clone, Copy)]
pub enum StatusType {
    Success,
    Warning,
    Error,
    Info,
}
