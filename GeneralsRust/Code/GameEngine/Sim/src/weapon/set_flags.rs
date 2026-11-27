//! Authored WeaponSet condition ordinals and their bit mask.

/// Weapon set type conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponSetType {
    Veteran = 0,
    Elite,
    Hero,
    PlayerUpgrade,
    CrateUpgradeOne,
    CrateUpgradeTwo,
    VehicleHijack,
    CarBomb,
    MineClearingDetail,
    WeaponRider1,
    WeaponRider2,
    WeaponRider3,
    WeaponRider4,
    WeaponRider5,
    WeaponRider6,
    WeaponRider7,
    WeaponRider8,
}

/// Weapon set flags for conditional weapon sets
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponSetFlags(u32);

impl WeaponSetFlags {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn set(&mut self, weapon_set_type: WeaponSetType) {
        self.0 |= 1 << (weapon_set_type as u8);
    }

    pub fn clear(&mut self, weapon_set_type: WeaponSetType) {
        self.0 &= !(1 << (weapon_set_type as u8));
    }

    pub fn test(&self, weapon_set_type: WeaponSetType) -> bool {
        (self.0 & (1 << (weapon_set_type as u8))) != 0
    }

    pub fn clear_all(&mut self) {
        self.0 = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn bits(&self) -> u32 {
        self.0
    }

    pub fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
}

impl WeaponSetType {
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(WeaponSetType::Veteran),
            1 => Some(WeaponSetType::Elite),
            2 => Some(WeaponSetType::Hero),
            3 => Some(WeaponSetType::PlayerUpgrade),
            4 => Some(WeaponSetType::CrateUpgradeOne),
            5 => Some(WeaponSetType::CrateUpgradeTwo),
            6 => Some(WeaponSetType::VehicleHijack),
            7 => Some(WeaponSetType::CarBomb),
            8 => Some(WeaponSetType::MineClearingDetail),
            9 => Some(WeaponSetType::WeaponRider1),
            10 => Some(WeaponSetType::WeaponRider2),
            11 => Some(WeaponSetType::WeaponRider3),
            12 => Some(WeaponSetType::WeaponRider4),
            13 => Some(WeaponSetType::WeaponRider5),
            14 => Some(WeaponSetType::WeaponRider6),
            15 => Some(WeaponSetType::WeaponRider7),
            16 => Some(WeaponSetType::WeaponRider8),
            _ => None,
        }
    }
}
