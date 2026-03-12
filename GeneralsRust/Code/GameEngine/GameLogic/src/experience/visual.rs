//! Visual Indicators for Veterancy - Chevrons, stars, and promotion effects
//!
//! This module handles the visual representation of veterancy levels,
//! including rank insignia (chevrons/stars) and promotion particle effects.

use crate::common::types::{Color, Coord3D, VeterancyLevel};

/// Veterancy rank insignia type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VeterancyInsignia {
    /// No insignia (Regular units)
    None,

    /// Single chevron (Veteran)
    SingleChevron,

    /// Double chevron (Elite)
    DoubleChevron,

    /// Star (Heroic)
    Star,
}

impl VeterancyInsignia {
    /// Get the insignia for a veterancy level
    pub fn for_level(level: VeterancyLevel) -> Self {
        match level {
            VeterancyLevel::Regular => Self::None,
            VeterancyLevel::Veteran => Self::SingleChevron,
            VeterancyLevel::Elite => Self::DoubleChevron,
            VeterancyLevel::Heroic => Self::Star,
        }
    }

    /// Get the texture name for this insignia
    pub fn texture_name(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::SingleChevron => Some("Textures/VeterancyChevron1.tga"),
            Self::DoubleChevron => Some("Textures/VeterancyChevron2.tga"),
            Self::Star => Some("Textures/VeterancyStar.tga"),
        }
    }

    /// Get the color tint for this insignia
    pub fn color_tint(&self) -> Color {
        match self {
            Self::None => Color::transparent(),
            Self::SingleChevron => Color::rgb(255, 215, 0), // Gold
            Self::DoubleChevron => Color::rgb(192, 192, 192), // Silver
            Self::Star => Color::rgb(255, 255, 0),          // Bright Yellow
        }
    }
}

/// Promotion effect configuration
#[derive(Debug, Clone)]
pub struct PromotionEffect {
    /// Particle system to spawn
    pub particle_system: Option<String>,

    /// Sound effect to play
    pub sound_effect: Option<String>,

    /// Flash color
    pub flash_color: Color,

    /// Flash duration in seconds
    pub flash_duration: f32,

    /// Whether to show text popup
    pub show_text_popup: bool,

    /// Text to display
    pub popup_text: String,
}

impl PromotionEffect {
    /// Get the default promotion effect for a level
    pub fn for_level(new_level: VeterancyLevel) -> Self {
        match new_level {
            VeterancyLevel::Regular => Self {
                particle_system: None,
                sound_effect: None,
                flash_color: Color::transparent(),
                flash_duration: 0.0,
                show_text_popup: false,
                popup_text: String::new(),
            },
            VeterancyLevel::Veteran => Self {
                particle_system: Some("FX_VeterancyPromotion".to_string()),
                sound_effect: Some("VeterancyPromotion".to_string()),
                flash_color: Color::rgb(255, 215, 0), // Gold
                flash_duration: 0.5,
                show_text_popup: true,
                popup_text: "VETERAN".to_string(),
            },
            VeterancyLevel::Elite => Self {
                particle_system: Some("FX_ElitePromotion".to_string()),
                sound_effect: Some("ElitePromotion".to_string()),
                flash_color: Color::rgb(192, 192, 192), // Silver
                flash_duration: 0.7,
                show_text_popup: true,
                popup_text: "ELITE".to_string(),
            },
            VeterancyLevel::Heroic => Self {
                particle_system: Some("FX_HeroicPromotion".to_string()),
                sound_effect: Some("HeroicPromotion".to_string()),
                flash_color: Color::rgb(255, 255, 0), // Bright Yellow
                flash_duration: 1.0,
                show_text_popup: true,
                popup_text: "HEROIC".to_string(),
            },
        }
    }
}

/// Veterancy visual indicator system
///
/// Manages visual feedback for veterancy levels and promotions
pub struct VeterancyVisuals {
    /// Current veterancy level
    current_level: VeterancyLevel,

    /// Current insignia
    current_insignia: VeterancyInsignia,

    /// Whether to show insignia above unit
    show_insignia: bool,

    /// Offset for insignia rendering (above unit)
    insignia_offset: Coord3D,

    /// Scale of insignia
    insignia_scale: f32,
}

impl VeterancyVisuals {
    /// Create new veterancy visuals
    pub fn new() -> Self {
        Self {
            current_level: VeterancyLevel::Regular,
            current_insignia: VeterancyInsignia::None,
            show_insignia: true,
            insignia_offset: Coord3D::new(0.0, 0.0, 2.0), // 2 units above object
            insignia_scale: 1.0,
        }
    }

    /// Update visuals for a new veterancy level
    ///
    /// Returns the promotion effect to apply, if any
    pub fn update_for_level(&mut self, new_level: VeterancyLevel) -> Option<PromotionEffect> {
        let old_level = self.current_level;

        if new_level == old_level {
            return None; // No change
        }

        self.current_level = new_level;
        self.current_insignia = VeterancyInsignia::for_level(new_level);

        // Only show promotion effect if this is an increase
        if new_level > old_level {
            Some(PromotionEffect::for_level(new_level))
        } else {
            None
        }
    }

    /// Get the current insignia
    pub fn get_insignia(&self) -> VeterancyInsignia {
        self.current_insignia
    }

    /// Get the current level
    pub fn get_level(&self) -> VeterancyLevel {
        self.current_level
    }

    /// Set whether to show insignia
    pub fn set_show_insignia(&mut self, show: bool) {
        self.show_insignia = show;
    }

    /// Check if insignia should be shown
    pub fn should_show_insignia(&self) -> bool {
        self.show_insignia && self.current_insignia != VeterancyInsignia::None
    }

    /// Get insignia offset
    pub fn get_insignia_offset(&self) -> Coord3D {
        self.insignia_offset
    }

    /// Set insignia offset
    pub fn set_insignia_offset(&mut self, offset: Coord3D) {
        self.insignia_offset = offset;
    }

    /// Get insignia scale
    pub fn get_insignia_scale(&self) -> f32 {
        self.insignia_scale
    }

    /// Set insignia scale
    pub fn set_insignia_scale(&mut self, scale: f32) {
        self.insignia_scale = scale;
    }

    /// Get render parameters for the insignia
    pub fn get_render_params(&self) -> Option<InsigniaRenderParams> {
        if !self.should_show_insignia() {
            return None;
        }

        Some(InsigniaRenderParams {
            texture: self.current_insignia.texture_name()?.to_string(),
            offset: self.insignia_offset,
            scale: self.insignia_scale,
            color: self.current_insignia.color_tint(),
            rotation: 0.0, // Always face camera
        })
    }
}

impl Default for VeterancyVisuals {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters for rendering a veterancy insignia
#[derive(Debug, Clone)]
pub struct InsigniaRenderParams {
    /// Texture path
    pub texture: String,

    /// World-space offset from object origin
    pub offset: Coord3D,

    /// Scale factor
    pub scale: f32,

    /// Color tint
    pub color: Color,

    /// Rotation in radians (typically 0 to face camera)
    pub rotation: f32,
}

/// Helper to create promotion particle effects
pub struct PromotionEffectSpawner;

impl PromotionEffectSpawner {
    /// Spawn a promotion effect at the given location
    ///
    /// # Parameters
    /// - `effect`: The promotion effect configuration
    /// - `position`: World position to spawn effect at
    /// - `object_id`: ID of the promoted object (for text popup attachment)
    ///
    /// # Returns
    /// True if effect was spawned successfully
    pub fn spawn_effect(effect: &PromotionEffect, position: Coord3D, object_id: u32) -> bool {
        // In a full implementation, this would:
        // 1. Spawn particle system at position
        // 2. Play sound effect
        // 3. Apply flash effect to object
        // 4. Show floating text popup

        if let Some(ref particle) = effect.particle_system {
            log::info!(
                "Spawning promotion particle '{}' at {:?}",
                particle,
                position
            );
        }

        if let Some(ref sound) = effect.sound_effect {
            log::info!(
                "Playing promotion sound '{}' for object {}",
                sound,
                object_id
            );
        }

        if effect.show_text_popup {
            log::info!(
                "Showing promotion text '{}' for object {}",
                effect.popup_text,
                object_id
            );
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insignia_for_level() {
        assert_eq!(
            VeterancyInsignia::for_level(VeterancyLevel::Regular),
            VeterancyInsignia::None
        );
        assert_eq!(
            VeterancyInsignia::for_level(VeterancyLevel::Veteran),
            VeterancyInsignia::SingleChevron
        );
        assert_eq!(
            VeterancyInsignia::for_level(VeterancyLevel::Elite),
            VeterancyInsignia::DoubleChevron
        );
        assert_eq!(
            VeterancyInsignia::for_level(VeterancyLevel::Heroic),
            VeterancyInsignia::Star
        );
    }

    #[test]
    fn test_insignia_texture_names() {
        assert!(VeterancyInsignia::None.texture_name().is_none());
        assert!(VeterancyInsignia::SingleChevron.texture_name().is_some());
        assert_eq!(
            VeterancyInsignia::SingleChevron.texture_name().unwrap(),
            "Textures/VeterancyChevron1.tga"
        );
    }

    #[test]
    fn test_insignia_colors() {
        let gold = VeterancyInsignia::SingleChevron.color_tint();
        assert_eq!(gold.r, 255);
        assert_eq!(gold.g, 215);
        assert_eq!(gold.b, 0);

        let star = VeterancyInsignia::Star.color_tint();
        assert_eq!(star.r, 255);
        assert_eq!(star.g, 255);
        assert_eq!(star.b, 0);
    }

    #[test]
    fn test_veterancy_visuals_creation() {
        let visuals = VeterancyVisuals::new();
        assert_eq!(visuals.get_level(), VeterancyLevel::Regular);
        assert_eq!(visuals.get_insignia(), VeterancyInsignia::None);
        assert!(!visuals.should_show_insignia()); // No insignia for Regular
    }

    #[test]
    fn test_veterancy_visuals_update() {
        let mut visuals = VeterancyVisuals::new();

        // Promote to Veteran
        let effect = visuals.update_for_level(VeterancyLevel::Veteran);
        assert!(effect.is_some());
        assert_eq!(visuals.get_level(), VeterancyLevel::Veteran);
        assert_eq!(visuals.get_insignia(), VeterancyInsignia::SingleChevron);
        assert!(visuals.should_show_insignia());

        let effect = effect.unwrap();
        assert!(effect.show_text_popup);
        assert_eq!(effect.popup_text, "VETERAN");
    }

    #[test]
    fn test_veterancy_visuals_no_effect_on_same_level() {
        let mut visuals = VeterancyVisuals::new();

        let effect = visuals.update_for_level(VeterancyLevel::Regular);
        assert!(effect.is_none()); // No change
    }

    #[test]
    fn test_veterancy_visuals_render_params() {
        let mut visuals = VeterancyVisuals::new();

        // Regular has no render params
        assert!(visuals.get_render_params().is_none());

        // Veteran has render params
        visuals.update_for_level(VeterancyLevel::Veteran);
        let params = visuals.get_render_params();
        assert!(params.is_some());

        let params = params.unwrap();
        assert!(params.texture.contains("Chevron1"));
        assert_eq!(params.scale, 1.0);
    }

    #[test]
    fn test_veterancy_visuals_insignia_toggle() {
        let mut visuals = VeterancyVisuals::new();
        visuals.update_for_level(VeterancyLevel::Elite);

        assert!(visuals.should_show_insignia());

        visuals.set_show_insignia(false);
        assert!(!visuals.should_show_insignia());
        assert!(visuals.get_render_params().is_none());
    }

    #[test]
    fn test_veterancy_visuals_offset_and_scale() {
        let mut visuals = VeterancyVisuals::new();

        let custom_offset = Coord3D::new(1.0, 2.0, 3.0);
        visuals.set_insignia_offset(custom_offset);
        assert_eq!(visuals.get_insignia_offset(), custom_offset);

        visuals.set_insignia_scale(2.5);
        assert_eq!(visuals.get_insignia_scale(), 2.5);
    }

    #[test]
    fn test_promotion_effect_levels() {
        let veteran_effect = PromotionEffect::for_level(VeterancyLevel::Veteran);
        assert!(veteran_effect.show_text_popup);
        assert_eq!(veteran_effect.popup_text, "VETERAN");
        assert_eq!(veteran_effect.flash_duration, 0.5);

        let heroic_effect = PromotionEffect::for_level(VeterancyLevel::Heroic);
        assert!(heroic_effect.show_text_popup);
        assert_eq!(heroic_effect.popup_text, "HEROIC");
        assert_eq!(heroic_effect.flash_duration, 1.0);
    }

    #[test]
    fn test_promotion_effect_spawner() {
        let effect = PromotionEffect::for_level(VeterancyLevel::Elite);
        let position = Coord3D::new(100.0, 200.0, 0.0);

        let result = PromotionEffectSpawner::spawn_effect(&effect, position, 123);
        assert!(result);
    }

    #[test]
    fn test_render_params_structure() {
        let params = InsigniaRenderParams {
            texture: "test.tga".to_string(),
            offset: Coord3D::new(0.0, 0.0, 2.0),
            scale: 1.5,
            color: Color::rgb(255, 0, 0),
            rotation: 0.0,
        };

        assert_eq!(params.texture, "test.tga");
        assert_eq!(params.scale, 1.5);
        assert_eq!(params.color.r, 255);
    }
}
