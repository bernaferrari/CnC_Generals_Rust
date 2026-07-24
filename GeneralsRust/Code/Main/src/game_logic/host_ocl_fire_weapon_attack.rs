//! Host ObjectCreationList FireWeapon + Attack nugget residual.
//!
//! C++:
//! - `FireWeaponNugget::create` → `WeaponStore::createAndFireTempWeapon(weapon, primary, secondary)`
//! - `AttackNugget::create` → temp weapon lock + `aiAttackPosition(secondary, numberOfShots)`
//!   + optional `RadiusDecalUpdate` delivery decal
//!
//! Retail peels:
//! - `SUPERWEAPON_NeutronMissile` / `SupW_SUPERWEAPON_NeutronMissile` → FireWeapon NeutronMissileWeapon
//! - `SUPERWEAPON_CruiseMissile` → FireWeapon CruiseMissileWeapon
//! - `SUPERWEAPON_ScudStorm` → Attack PRIMARY NumberOfShots=9 DeliveryDecalRadius=200
//!
//! Residual playability slice:
//! - Weapon → projectile template peel (NeutronMissile / CruiseMissile / ScudStormMissile)
//! - Spawn projectile residual at primary aiming secondary (fail-closed vs full loft)
//! - Attack nugget queues multi-shot attack-position residual + delivery decal radius
//!
//! Fail-closed: not full WeaponStore temp weapon matrix / projectile update loft /
//! clip scatter / full RadiusDecalUpdate kill-when-not-attacking path.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail SUPERWEAPON_ScudStorm Attack NumberOfShots residual.
pub const SCUD_STORM_OCL_ATTACK_SHOTS: u32 = 9;
/// Retail DeliveryDecalRadius residual.
pub const SCUD_STORM_OCL_DECAL_RADIUS: f32 = 200.0;

/// Weapon slot residual ordinal (PRIMARY=0, SECONDARY=1, TERTIARY=2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum OclWeaponSlot {
    #[default]
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

impl OclWeaponSlot {
    pub fn from_name(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "SECONDARY" => OclWeaponSlot::Secondary,
            "TERTIARY" => OclWeaponSlot::Tertiary,
            _ => OclWeaponSlot::Primary,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostOclFireWeaponPlan {
    pub ocl_name: String,
    pub weapon_name: String,
    pub projectile_template: String,
    pub damage_type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostOclAttackPlan {
    pub ocl_name: String,
    pub number_of_shots: u32,
    pub weapon_slot: OclWeaponSlot,
    pub delivery_decal_radius: f32,
    pub delivery_decal_texture: String,
}

/// Map OCL / weapon names to FireWeapon residual peels.
pub fn fire_weapon_plan_for_ocl(ocl_or_weapon: &str) -> Option<HostOclFireWeaponPlan> {
    let n = ocl_or_weapon.to_ascii_lowercase();
    if n.contains("neutron") {
        return Some(HostOclFireWeaponPlan {
            ocl_name: "SUPERWEAPON_NeutronMissile".into(),
            weapon_name: "NeutronMissileWeapon".into(),
            projectile_template: "NeutronMissile".into(),
            damage_type_name: "RADIATION".into(),
        });
    }
    if n.contains("cruise") {
        return Some(HostOclFireWeaponPlan {
            ocl_name: "SUPERWEAPON_CruiseMissile".into(),
            weapon_name: "CruiseMissileWeapon".into(),
            projectile_template: "CruiseMissile".into(),
            damage_type_name: "RADIATION".into(),
        });
    }
    if n.contains("scudstorm") && n.contains("weapon") {
        // ScudStorm uses Attack nugget primarily; weapon peel for completeness.
        return Some(HostOclFireWeaponPlan {
            ocl_name: "SUPERWEAPON_ScudStorm".into(),
            weapon_name: "ScudStormWeapon".into(),
            projectile_template: "ScudStormMissile".into(),
            damage_type_name: "EXPLOSION".into(),
        });
    }
    None
}

/// Map OCL name to Attack residual peels.
pub fn attack_plan_for_ocl(ocl_name: &str) -> Option<HostOclAttackPlan> {
    let n = ocl_name.to_ascii_lowercase();
    if n.contains("scudstorm") || n.contains("scud_storm") {
        return Some(HostOclAttackPlan {
            ocl_name: "SUPERWEAPON_ScudStorm".into(),
            number_of_shots: SCUD_STORM_OCL_ATTACK_SHOTS,
            weapon_slot: OclWeaponSlot::Primary,
            delivery_decal_radius: SCUD_STORM_OCL_DECAL_RADIUS,
            delivery_decal_texture: "SCCScudStorm_GLA".into(),
        });
    }
    None
}

/// Map host superweapon kind label → OCL FireWeapon/Attack residual name.
pub fn ocl_nugget_for_host_kind(kind_label: &str) -> Option<OclNuggetKind> {
    match kind_label {
        "NuclearMissile" => Some(OclNuggetKind::FireWeapon("SUPERWEAPON_NeutronMissile")),
        "CruiseMissile" => Some(OclNuggetKind::FireWeapon("SUPERWEAPON_CruiseMissile")),
        "ScudStorm" => Some(OclNuggetKind::Attack("SUPERWEAPON_ScudStorm")),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OclNuggetKind {
    FireWeapon(&'static str),
    Attack(&'static str),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOclFireWeaponAttackRegistry {
    pub fire_weapon_plans: u32,
    pub projectiles_spawned: u32,
    pub attack_plans: u32,
    pub attack_shots_queued: u32,
    pub delivery_decals: u32,
    pub last_weapon: String,
    pub last_attack_shots: u32,
}

impl HostOclFireWeaponAttackRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_fire_weapon(&mut self, weapon: &str) {
        self.fire_weapon_plans = self.fire_weapon_plans.saturating_add(1);
        self.last_weapon = weapon.into();
    }
    pub fn record_projectile(&mut self) {
        self.projectiles_spawned = self.projectiles_spawned.saturating_add(1);
    }
    pub fn record_attack(&mut self, shots: u32) {
        self.attack_plans = self.attack_plans.saturating_add(1);
        self.attack_shots_queued = self.attack_shots_queued.saturating_add(shots);
        self.last_attack_shots = shots;
    }
    pub fn record_decal(&mut self) {
        self.delivery_decals = self.delivery_decals.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.fire_weapon_plans > 0 || self.attack_plans > 0
    }
}

pub fn honesty_ocl_fire_weapon_attack_residual_ok() -> bool {
    fire_weapon_plan_for_ocl("SUPERWEAPON_NeutronMissile")
        .map(|p| p.projectile_template == "NeutronMissile")
        .unwrap_or(false)
        && fire_weapon_plan_for_ocl("SUPERWEAPON_CruiseMissile")
            .map(|p| p.weapon_name == "CruiseMissileWeapon")
            .unwrap_or(false)
        && attack_plan_for_ocl("SUPERWEAPON_ScudStorm")
            .map(|p| {
                p.number_of_shots == SCUD_STORM_OCL_ATTACK_SHOTS
                    && (p.delivery_decal_radius - SCUD_STORM_OCL_DECAL_RADIUS).abs() < 0.1
            })
            .unwrap_or(false)
        && matches!(
            ocl_nugget_for_host_kind("NuclearMissile"),
            Some(OclNuggetKind::FireWeapon(_))
        )
        && matches!(
            ocl_nugget_for_host_kind("ScudStorm"),
            Some(OclNuggetKind::Attack(_))
        )
        && SCUD_STORM_OCL_ATTACK_SHOTS == 9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peels_and_kind_map() {
        assert!(honesty_ocl_fire_weapon_attack_residual_ok());
        let a = attack_plan_for_ocl("SUPERWEAPON_ScudStorm").unwrap();
        assert_eq!(a.weapon_slot, OclWeaponSlot::Primary);
        assert_eq!(OclWeaponSlot::from_name("SECONDARY"), OclWeaponSlot::Secondary);
    }
}
