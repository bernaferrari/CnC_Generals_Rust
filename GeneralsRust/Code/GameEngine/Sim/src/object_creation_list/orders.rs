use super::calc_random_force_with;
use crate::weapon::WeaponSlotType;
use game_engine::common::thing::radius_decal_data::RadiusDecalTemplate;
use glam::Vec3;

/// FireWeaponNugget - fires a temporary weapon
/// Matches C++ FireWeaponNugget (ObjectCreationList.cpp:105-148)
///
/// Used for effects that need to fire weapons without permanent objects
#[derive(Debug, Clone)]
pub struct FireWeaponNugget {
    pub weapon: Option<String>, // Weapon template name
}

impl Default for FireWeaponNugget {
    fn default() -> Self {
        Self { weapon: None }
    }
}

/// AttackNugget - makes object attack a position
/// Matches C++ AttackNugget (ObjectCreationList.cpp:151-221)
///
/// Used for scripted attacks and special power targeting
#[derive(Debug, Clone)]
pub struct AttackNugget {
    pub number_of_shots: i32,
    pub weapon_slot: WeaponSlotType,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: f32,
}

impl Default for AttackNugget {
    fn default() -> Self {
        Self {
            number_of_shots: 1,
            weapon_slot: WeaponSlotType::Primary,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

/// ApplyRandomForceNugget - applies random forces to an object
/// Matches C++ ApplyRandomForceNugget (ObjectCreationList.cpp:595-670)
///
/// Used for creating visual variety in explosions and impacts
#[derive(Debug, Clone)]
pub struct ApplyRandomForceNugget {
    pub spin_rate: f32,
    pub min_mag: f32,
    pub max_mag: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
}

impl Default for ApplyRandomForceNugget {
    fn default() -> Self {
        Self {
            spin_rate: 0.0,
            min_mag: 0.0,
            max_mag: 0.0,
            min_pitch: 0.0,
            max_pitch: 0.0,
        }
    }
}

impl ApplyRandomForceNugget {
    /// Draw the force in C++ Z-up coordinates with the caller's logic RNG.
    pub fn draw_force(&self, draw: impl FnMut(f32, f32) -> f32) -> Vec3 {
        calc_random_force_with(
            draw,
            self.min_mag,
            self.max_mag,
            self.min_pitch,
            self.max_pitch,
        )
    }

    /// Draw yaw, roll, and pitch in assignment order after applying the force.
    pub fn draw_spin(&self, mut draw: impl FnMut(f32, f32) -> f32) -> [f32; 3] {
        std::array::from_fn(|_| draw(-self.spin_rate, self.spin_rate))
    }
}
