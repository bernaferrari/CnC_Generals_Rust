/// Wave 298: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

pub type ObjectID = u32;
pub type PlayerIndex = u32;
pub type Real = f32;
pub type Color = u32;

pub const INVALID_ID: ObjectID = 0;
pub const BASE_VALUE_PER_SUPPLY_BOX: i32 = 100; // Matches C++ GlobalData::m_baseValuePerSupplyBox

// Faction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Faction {
    USA,
    China,
    GLA,
}

// ============================================================================
// EXTERNAL SYSTEM INTERFACES
// ============================================================================

/// Audio system interface
/// In production this would connect to the real audio system
pub trait AudioSystem: Send + Sync {
    /// Play money withdraw sound
    /// Matches C++ Money::withdraw() - TheAudio->addAudioEvent(&event)
    fn play_money_withdraw_sound(&self, player_index: PlayerIndex);

    /// Play money deposit sound
    /// Matches C++ Money::deposit() - TheAudio->addAudioEvent(&event)
    fn play_money_deposit_sound(&self, player_index: PlayerIndex);

    /// Play voice event
    /// Matches C++ SupplyTruckAIUpdate::gainOneBox() - TheAudio->addAudioEvent(&m_suppliesDepletedVoice)
    fn play_voice_event(&self, event_name: &str, object_id: ObjectID);
}

/// UI system interface for floating text
/// In production this would connect to InGameUI
pub trait UISystem: Send + Sync {
    /// Add floating text at position
    /// Matches C++ TheInGameUI->addFloatingText(moneys, &pos, color)
    /// From SupplyCenterDockUpdate.cpp:136 and AutoDepositUpdate.cpp:186
    fn add_floating_text(&self, text: &str, position: &Coord3D, color: Color);
}

/// Academy stats tracking interface
/// Matches C++ Player::getAcademyStats()->recordIncome()
pub trait AcademyStats: Send + Sync {
    /// Record income for statistics
    /// From Money.cpp:65
    fn record_income(&self);
}

/// Stealth system interface
/// Matches C++ StealthUpdate from SupplyCenterDockUpdate.cpp:92-108
pub trait StealthSystem: Send + Sync {
    /// Grant temporary stealth to an object
    /// Matches C++ stealth->receiveGrant(TRUE, frames)
    fn grant_temporary_stealth(&self, object_id: ObjectID, frames: u32);
}

/// Upgrade system interface
/// Matches C++ Player::hasUpgradeComplete(upgradeTemplate)
pub trait UpgradeSystem: Send + Sync {
    /// Check if player has a specific upgrade
    /// Returns bonus amount if upgrade is present, 0 otherwise
    /// Matches C++ WorkerAIUpdate::getUpgradedSupplyBoost() - WorkerAIUpdate.cpp:1376
    fn get_supply_boost(&self, player_index: PlayerIndex) -> u32;
}

/// 3D coordinate
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Coord3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn distance_to(&self, other: &Coord3D) -> Real {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn distance_squared_to(&self, other: &Coord3D) -> Real {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }
}

/// C++ `TheGameText->fetch("GUI:AddCash")` formatted with the deposit value.
pub fn format_gui_add_cash(value: u32) -> String {
    let template = crate::helpers::TheGameText::fetch("GUI:AddCash");
    if template.contains("%d") {
        template.replace("%d", &value.to_string())
    } else if template.contains("%i") {
        template.replace("%i", &value.to_string())
    } else if template.is_empty() || template == "GUI:AddCash" {
        format!("+${}", value)
    } else {
        format!("{}{}", template, value)
    }
}


