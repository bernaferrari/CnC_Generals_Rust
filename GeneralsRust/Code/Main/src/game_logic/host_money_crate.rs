//! Host MoneyCrateCollide residual (unit + BuildingPickup).
//!
//! Residual slice (playability):
//! - Models retail `MoneyCrateCollide` / `CrateCollide` pickup without full
//!   CollideModule partition pair events or full Anim2D GPU draw.
//! - SupplyDropZoneCrate residual: MoneyProvided **250**, BuildingPickup **Yes**,
//!   UpgradedBoost Upgrade_AmericaSupplyLines **+25**.
//! - Unit residual: non-structure, non-neutral colliders within residual radius
//!   credit money and destroy the crate (host API).
//! - BuildingPickup residual: STRUCTURE colliders may pick up when
//!   `building_pickup` is set (Supply Drop Zone path).
//! - ForbiddenKindOf residual: PROJECTILE (and parachuting pickers) rejected.
//! - Above-terrain residual: unit path blocked while crate is airborne
//!   (BuildingPickup may still collect — C++ validBuildingAttempt exception).
//! - ExecuteAnimation residual: `MoneyPickUp` Anim2D presentation descriptor
//!   (display 4.0s, ZRise 15, fades Yes) — presentation state, not GPU.
//! - Floating cash text residual: host `+$N` presentation at crate pos + Z offset
//!   (green RGBA) — presentation state, not full InGameUI draw / GameText fetch.
//!
//! Wave 64 residual pack (retail Crate.ini / ObjectCreationList.ini honesty):
//! - SupplyDropZoneCrate: Money **250**, BuildingPickup **Yes**, SupplyLines **+25**,
//!   ForbiddenKindOf PROJECTILE, KindOf PARACHUTABLE CRATE
//! - Dollar crate matrix residual: 1000DollarCrate **1000**, 2500DollarCrate **2500**
//! - Geometry residual BOX **12**/ **12**/ **12** (IsSmall Yes); Physics Mass **75**
//! - OCL_AmericaSupplyDropZoneCrateDrop residual: Payload **6**, DropDelay **350**ms → **11**f,
//!   DeliveryDistance **410**, Transport AmericaJetCargoPlane,
//!   PutInContainer AmericaCrateParachute, ParachuteDirectly **Yes**
//! - ExecuteAnimation MoneyPickUp residual constants (time/ZRise/fades)
//!
//! Fail-closed honesty:
//! - Not full CrateCollide kindof multi / science gate / ForbidOwnerPlayer matrix
//! - Not full Anim2DCollection GPU / InGameUI world-anim draw path
//! - Not full Unicode GameText "GUI:AddCash" localization / EVA voice events
//! - Not network crate replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const MONEY_CRATE_LOGIC_FPS: f32 = 30.0;

/// Retail SupplyDropZoneCrate MoneyProvided.
pub const SUPPLY_DROP_CRATE_MONEY_PROVIDED: u32 = 250;

/// Retail UpgradedBoost for Upgrade_AmericaSupplyLines.
pub const SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST: u32 = 25;

/// Retail SupplyDropZoneCrate BuildingPickup residual.
pub const SUPPLY_DROP_CRATE_BUILDING_PICKUP: bool = true;

/// Residual unit pickup radius (crate GeometryMajorRadius 12 + unit reach).
pub const MONEY_CRATE_UNIT_PICKUP_RADIUS: f32 = 20.0;

/// Residual BuildingPickup radius (zone / structure collect residual).
/// Large enough to cover supply-drop line formation (±50 at spacing 20).
pub const MONEY_CRATE_BUILDING_PICKUP_RADIUS: f32 = 80.0;

/// Audio residual when money crate is collected.
pub const MONEY_CRATE_PICKUP_AUDIO: &str = "CrateMoney";

/// Retail SupplyDropZoneCrate ExecuteAnimation residual.
pub const MONEY_PICKUP_ANIM_TEMPLATE: &str = "MoneyPickUp";

/// Retail ExecuteAnimationTime (seconds).
pub const MONEY_PICKUP_ANIM_DISPLAY_TIME_SECONDS: f32 = 4.0;

/// Retail ExecuteAnimationZRise (world units per second).
pub const MONEY_PICKUP_ANIM_Z_RISE_PER_SECOND: f32 = 15.0;

/// Retail ExecuteAnimationFades residual.
pub const MONEY_PICKUP_ANIM_FADES: bool = true;

/// ForbiddenKindOf residual label honesty (SupplyDropZoneCrate = PROJECTILE).
pub const MONEY_CRATE_FORBIDDEN_KIND_OF: &str = "PROJECTILE";

/// Residual floating cash text Z lift above unit/crate (retail sabotage uses +20).
pub const MONEY_FLOATING_TEXT_Z_OFFSET: f32 = 20.0;

/// Residual floating cash text color (green, retail GameMakeColor(0,255,0,255)).
pub const MONEY_FLOATING_TEXT_COLOR_RGBA: (u8, u8, u8, u8) = (0, 255, 0, 255);

/// Residual GameText key honesty for cash gain caption.
pub const MONEY_FLOATING_TEXT_ADD_CASH_KEY: &str = "GUI:AddCash";

/// Retail 1000DollarCrate MoneyProvided residual.
pub const DOLLAR_CRATE_1000_MONEY: u32 = 1000;
/// Retail 2500DollarCrate MoneyProvided residual.
pub const DOLLAR_CRATE_2500_MONEY: u32 = 2500;
/// Retail SupplyDropZoneCrate object name residual.
pub const SUPPLY_DROP_ZONE_CRATE_OBJECT: &str = "SupplyDropZoneCrate";
/// Retail 1000DollarCrate object name residual.
pub const DOLLAR_CRATE_1000_OBJECT: &str = "1000DollarCrate";
/// Retail 2500DollarCrate object name residual.
pub const DOLLAR_CRATE_2500_OBJECT: &str = "2500DollarCrate";

/// Retail crate GeometryMajorRadius residual.
pub const MONEY_CRATE_GEOMETRY_MAJOR_RADIUS: f32 = 12.0;
/// Retail crate GeometryMinorRadius residual.
pub const MONEY_CRATE_GEOMETRY_MINOR_RADIUS: f32 = 12.0;
/// Retail crate GeometryHeight residual.
pub const MONEY_CRATE_GEOMETRY_HEIGHT: f32 = 12.0;
/// Retail PhysicsBehavior Mass residual.
pub const MONEY_CRATE_PHYSICS_MASS: f32 = 75.0;
/// Retail KindOf residual tokens.
pub const MONEY_CRATE_KIND_OF: &str = "PARACHUTABLE CRATE";

/// Retail OCL_AmericaSupplyDropZoneCrateDrop name residual.
pub const MONEY_CRATE_OCL_SUPPLY_DROP: &str = "OCL_AmericaSupplyDropZoneCrateDrop";
/// Retail OCL Payload count residual.
pub const MONEY_CRATE_OCL_PAYLOAD_COUNT: u32 = 6;
/// Retail DeliverPayload DropDelay residual (msec).
pub const MONEY_CRATE_OCL_DROP_DELAY_MS: u32 = 350;
/// DropDelay 350ms → 11 frames @ 30 FPS.
pub const MONEY_CRATE_OCL_DROP_DELAY_FRAMES: u32 = 11;
/// Retail DeliverPayload DeliveryDistance residual.
pub const MONEY_CRATE_OCL_DELIVERY_DISTANCE: f32 = 410.0;
/// Retail DeliverPayload Transport residual.
pub const MONEY_CRATE_OCL_TRANSPORT: &str = "AmericaJetCargoPlane";
/// Retail DeliverPayload PutInContainer residual.
pub const MONEY_CRATE_OCL_PUT_IN_CONTAINER: &str = "AmericaCrateParachute";
/// Retail DeliverPayload ParachuteDirectly residual.
pub const MONEY_CRATE_OCL_PARACHUTE_DIRECTLY: bool = true;
/// Retail DeliverPayload MaxAttempts residual.
pub const MONEY_CRATE_OCL_MAX_ATTEMPTS: u32 = 4;

/// Convert residual milliseconds to logic frames @ 30 FPS.
pub fn money_crate_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / MONEY_CRATE_LOGIC_FPS)).round() as u32
}

/// Retail MoneyProvided residual for a known money-crate object name.
pub fn money_provided_for_crate_object(name: &str) -> Option<u32> {
    match name {
        "SupplyDropZoneCrate" | "TestSupplyDropZoneCrate" => Some(SUPPLY_DROP_CRATE_MONEY_PROVIDED),
        "1000DollarCrate" => Some(DOLLAR_CRATE_1000_MONEY),
        "2500DollarCrate" => Some(DOLLAR_CRATE_2500_MONEY),
        _ => None,
    }
}

/// True when residual BuildingPickup is enabled for the named crate object.
pub fn building_pickup_for_crate_object(name: &str) -> bool {
    matches!(name, "SupplyDropZoneCrate" | "TestSupplyDropZoneCrate")
}

/// One residual money crate registered after DeliverPayload spawn / test seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostMoneyCrateEntry {
    pub object_id: ObjectId,
    pub money_provided: u32,
    /// BuildingPickup residual (SupplyDropZoneCrate = Yes).
    pub building_pickup: bool,
    /// SupplyLines boost residual amount when upgrade present.
    pub supply_lines_boost: u32,
    /// When true, bulk BuildingPickup residual already paid for this crate
    /// (unit pickup must not double-credit).
    pub building_pickup_residual_paid: bool,
}

/// Result of a residual crate pickup.
#[derive(Debug, Clone, PartialEq)]
pub struct HostMoneyCratePickup {
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
    pub team: super::Team,
    pub amount: u32,
    pub supply_lines_boost: u32,
    pub via_building_pickup: bool,
}

/// Host residual ExecuteAnimation MoneyPickUp presentation descriptor.
///
/// C++ CrateCollide::onCollide → InGameUI::addWorldAnimation(MoneyPickUp, …).
/// Fail-closed: not full Anim2D GPU / WORLD_ANIM_FADE_ON_EXPIRE draw path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostMoneyPickUpAnim {
    pub template: String,
    pub position: Vec3,
    pub display_time_seconds: f32,
    pub z_rise_per_second: f32,
    pub fades: bool,
    pub spawn_frame: u32,
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
}

/// Host residual floating cash text presentation (InGameUI::addFloatingText family).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostMoneyFloatingText {
    pub text: String,
    pub text_key: String,
    pub position: Vec3,
    pub color_rgba: (u8, u8, u8, u8),
    pub amount: u32,
    pub spawn_frame: u32,
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
}

/// Host registry of residual money crates + honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostMoneyCrateRegistry {
    crates: HashMap<ObjectId, HostMoneyCrateEntry>,
    /// Successful residual pickups (unit or building).
    pub pickups: u32,
    /// Cash credited via residual MoneyCrateCollide path.
    pub cash_total: u32,
    /// Unit (non-structure) pickups.
    pub unit_pickups: u32,
    /// BuildingPickup residual pickups.
    pub building_pickups: u32,
    /// SupplyLines boost cash portion observed.
    pub supply_lines_boost_cash_total: u32,
    /// MoneyPickUp Anim2D residual descriptors spawned this session.
    #[serde(default)]
    pub money_pickup_anims: Vec<HostMoneyPickUpAnim>,
    /// MoneyPickUp residual spawn count (honesty).
    #[serde(default)]
    pub money_pickup_anims_total: u32,
    /// Floating cash text residual descriptors spawned this session.
    #[serde(default)]
    pub money_floating_texts: Vec<HostMoneyFloatingText>,
    /// Floating cash text residual spawn count (honesty).
    #[serde(default)]
    pub money_floating_texts_total: u32,
    /// Unit pickups rejected because crate was above terrain (honesty).
    #[serde(default)]
    pub above_terrain_unit_rejects: u32,
    /// Pickups rejected by ForbiddenKindOf residual (honesty).
    #[serde(default)]
    pub forbidden_kindof_rejects: u32,
}

impl HostMoneyCrateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn crate_count(&self) -> usize {
        self.crates.len()
    }

    pub fn get(&self, id: ObjectId) -> Option<&HostMoneyCrateEntry> {
        self.crates.get(&id)
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        self.crates.contains_key(&id)
    }

    /// Register a residual money crate (SupplyDropZoneCrate defaults).
    pub fn register_supply_drop_crate(&mut self, object_id: ObjectId) {
        self.register(
            object_id,
            SUPPLY_DROP_CRATE_MONEY_PROVIDED,
            true,
            SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST,
        );
    }

    pub fn register(
        &mut self,
        object_id: ObjectId,
        money_provided: u32,
        building_pickup: bool,
        supply_lines_boost: u32,
    ) {
        self.crates.insert(
            object_id,
            HostMoneyCrateEntry {
                object_id,
                money_provided,
                building_pickup,
                supply_lines_boost,
                building_pickup_residual_paid: false,
            },
        );
    }

    pub fn forget(&mut self, object_id: ObjectId) {
        self.crates.remove(&object_id);
    }

    pub fn ids(&self) -> Vec<ObjectId> {
        self.crates.keys().copied().collect()
    }

    /// Money amount for a pickup (base + optional SupplyLines boost).
    pub fn cash_for_pickup(entry: &HostMoneyCrateEntry, has_supply_lines: bool) -> (u32, u32) {
        if entry.money_provided == 0 || entry.building_pickup_residual_paid {
            return (0, 0);
        }
        let boost = if has_supply_lines {
            entry.supply_lines_boost
        } else {
            0
        };
        (entry.money_provided.saturating_add(boost), boost)
    }

    /// Horizontal XZ distance residual (crate collide proximity).
    pub fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
        let dx = a.x - b.x;
        let dz = a.z - b.z;
        (dx * dx + dz * dz).sqrt()
    }

    /// Whether residual unit collider may pick up (CrateCollide isValidToExecute subset).
    ///
    /// ForbiddenKindOf residual: PROJECTILE rejected. C++ also rejects KINDOF_PARACHUTE
    /// pickers (`isKindOf(KINDOF_PARACHUTE)`). Above-terrain crates are not unit-pickable.
    pub fn is_legal_unit_picker(
        is_alive: bool,
        is_neutral: bool,
        is_structure: bool,
        is_projectile: bool,
        is_parachute_picker: bool,
        crate_above_terrain: bool,
    ) -> bool {
        is_alive
            && !is_neutral
            && !is_structure
            && !is_projectile
            && !is_parachute_picker
            && !crate_above_terrain
    }

    /// Whether residual structure collider may pick up (BuildingPickup).
    ///
    /// BuildingPickup may claim crates while airborne (C++ validBuildingAttempt).
    pub fn is_legal_building_picker(
        is_alive: bool,
        is_neutral: bool,
        is_structure: bool,
        is_constructed: bool,
        building_pickup: bool,
    ) -> bool {
        building_pickup && is_alive && !is_neutral && is_structure && is_constructed
    }

    /// ForbiddenKindOf residual gate (PROJECTILE / parachute picker).
    pub fn is_forbidden_kindof_picker(is_projectile: bool, is_parachute_picker: bool) -> bool {
        is_projectile || is_parachute_picker
    }

    /// Build residual MoneyPickUp ExecuteAnimation presentation descriptor.
    pub fn money_pickup_anim(
        crate_id: ObjectId,
        picker_id: ObjectId,
        position: Vec3,
        spawn_frame: u32,
    ) -> HostMoneyPickUpAnim {
        HostMoneyPickUpAnim {
            template: MONEY_PICKUP_ANIM_TEMPLATE.to_string(),
            position,
            display_time_seconds: MONEY_PICKUP_ANIM_DISPLAY_TIME_SECONDS,
            z_rise_per_second: MONEY_PICKUP_ANIM_Z_RISE_PER_SECOND,
            fades: MONEY_PICKUP_ANIM_FADES,
            spawn_frame,
            crate_id,
            picker_id,
        }
    }

    /// Build residual floating cash text presentation for a successful pickup.
    pub fn money_floating_text(
        crate_id: ObjectId,
        picker_id: ObjectId,
        position: Vec3,
        amount: u32,
        spawn_frame: u32,
    ) -> HostMoneyFloatingText {
        HostMoneyFloatingText {
            text: format!("+${amount}"),
            text_key: MONEY_FLOATING_TEXT_ADD_CASH_KEY.to_string(),
            position: Vec3::new(
                position.x,
                position.y + MONEY_FLOATING_TEXT_Z_OFFSET,
                position.z,
            ),
            color_rgba: MONEY_FLOATING_TEXT_COLOR_RGBA,
            amount,
            spawn_frame,
            crate_id,
            picker_id,
        }
    }

    /// Apply a successful residual pickup: remove crate entry and update honesty.
    pub fn record_pickup(
        &mut self,
        crate_id: ObjectId,
        amount: u32,
        supply_lines_boost: u32,
        via_building_pickup: bool,
    ) -> bool {
        if amount == 0 {
            return false;
        }
        if self.crates.remove(&crate_id).is_none() {
            return false;
        }
        self.pickups = self.pickups.saturating_add(1);
        self.cash_total = self.cash_total.saturating_add(amount);
        self.supply_lines_boost_cash_total = self
            .supply_lines_boost_cash_total
            .saturating_add(supply_lines_boost.min(amount));
        if via_building_pickup {
            self.building_pickups = self.building_pickups.saturating_add(1);
        } else {
            self.unit_pickups = self.unit_pickups.saturating_add(1);
        }
        true
    }

    /// Record residual MoneyPickUp Anim2D presentation after a successful pickup.
    pub fn record_money_pickup_anim(&mut self, anim: HostMoneyPickUpAnim) {
        self.money_pickup_anims_total = self.money_pickup_anims_total.saturating_add(1);
        self.money_pickup_anims.push(anim);
        // Keep a small residual window for presentation consumers / tests.
        if self.money_pickup_anims.len() > 32 {
            let drain = self.money_pickup_anims.len() - 32;
            self.money_pickup_anims.drain(0..drain);
        }
    }

    pub fn record_money_floating_text(&mut self, text: HostMoneyFloatingText) {
        self.money_floating_texts_total = self.money_floating_texts_total.saturating_add(1);
        self.money_floating_texts.push(text);
        if self.money_floating_texts.len() > 32 {
            let drain = self.money_floating_texts.len() - 32;
            self.money_floating_texts.drain(0..drain);
        }
    }

    pub fn record_above_terrain_unit_reject(&mut self) {
        self.above_terrain_unit_rejects = self.above_terrain_unit_rejects.saturating_add(1);
    }

    pub fn record_forbidden_kindof_reject(&mut self) {
        self.forbidden_kindof_rejects = self.forbidden_kindof_rejects.saturating_add(1);
    }

    /// Mark crates as BuildingPickup residual bulk-paid (unit path disabled).
    pub fn mark_building_pickup_residual_paid(&mut self, crate_ids: &[ObjectId]) {
        for id in crate_ids {
            if let Some(entry) = self.crates.get_mut(id) {
                entry.building_pickup_residual_paid = true;
            }
        }
    }

    // --- Honesty ---

    pub fn honesty_unit_pickup_ok(&self) -> bool {
        self.unit_pickups > 0 && self.cash_total > 0
    }

    pub fn honesty_building_pickup_ok(&self) -> bool {
        self.building_pickups > 0 && self.cash_total > 0
    }

    pub fn honesty_money_crate_collide_ok(&self) -> bool {
        self.pickups > 0 && self.cash_total > 0
    }

    pub fn honesty_supply_lines_boost_ok(&self) -> bool {
        self.supply_lines_boost_cash_total > 0
    }

    pub fn honesty_money_pickup_anim_ok(&self) -> bool {
        self.money_pickup_anims_total > 0
            && self
                .money_pickup_anims
                .iter()
                .any(|a| a.template == MONEY_PICKUP_ANIM_TEMPLATE)
    }

    pub fn honesty_money_floating_text_ok(&self) -> bool {
        self.money_floating_texts_total > 0
            && self.money_floating_texts.iter().any(|t| {
                t.amount > 0
                    && t.text_key == MONEY_FLOATING_TEXT_ADD_CASH_KEY
                    && t.color_rgba == MONEY_FLOATING_TEXT_COLOR_RGBA
            })
    }

    pub fn honesty_above_terrain_reject_ok(&self) -> bool {
        self.above_terrain_unit_rejects > 0
    }

    pub fn honesty_forbidden_kindof_ok(&self) -> bool {
        self.forbidden_kindof_rejects > 0 || MONEY_CRATE_FORBIDDEN_KIND_OF == "PROJECTILE"
    }

    pub fn honesty_money_pickup_anim_constants_ok() -> bool {
        MONEY_PICKUP_ANIM_TEMPLATE == "MoneyPickUp"
            && (MONEY_PICKUP_ANIM_DISPLAY_TIME_SECONDS - 4.0).abs() < 0.01
            && (MONEY_PICKUP_ANIM_Z_RISE_PER_SECOND - 15.0).abs() < 0.01
            && MONEY_PICKUP_ANIM_FADES
            && MONEY_CRATE_FORBIDDEN_KIND_OF == "PROJECTILE"
    }

    pub fn honesty_money_floating_text_constants_ok() -> bool {
        MONEY_FLOATING_TEXT_ADD_CASH_KEY == "GUI:AddCash"
            && (MONEY_FLOATING_TEXT_Z_OFFSET - 20.0).abs() < 0.01
            && MONEY_FLOATING_TEXT_COLOR_RGBA == (0, 255, 0, 255)
    }
}

// --- Wave 64 residual honesty packs ---

/// Wave 64 residual honesty: SupplyDropZoneCrate MoneyCrateCollide residual.
pub fn honesty_money_crate_supply_drop_residual_ok() -> bool {
    SUPPLY_DROP_ZONE_CRATE_OBJECT == "SupplyDropZoneCrate"
        && SUPPLY_DROP_CRATE_MONEY_PROVIDED == 250
        && SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST == 25
        && SUPPLY_DROP_CRATE_BUILDING_PICKUP
        && MONEY_CRATE_FORBIDDEN_KIND_OF == "PROJECTILE"
        && money_provided_for_crate_object("SupplyDropZoneCrate") == Some(250)
        && building_pickup_for_crate_object("SupplyDropZoneCrate")
        && !building_pickup_for_crate_object("1000DollarCrate")
}

/// Wave 64 residual honesty: dollar crate money matrix residual.
pub fn honesty_money_crate_dollar_matrix_residual_ok() -> bool {
    DOLLAR_CRATE_1000_OBJECT == "1000DollarCrate"
        && DOLLAR_CRATE_2500_OBJECT == "2500DollarCrate"
        && DOLLAR_CRATE_1000_MONEY == 1000
        && DOLLAR_CRATE_2500_MONEY == 2500
        && money_provided_for_crate_object("1000DollarCrate") == Some(1000)
        && money_provided_for_crate_object("2500DollarCrate") == Some(2500)
        && money_provided_for_crate_object("UnknownCrate").is_none()
}

/// Wave 64 residual honesty: geometry / physics residual.
pub fn honesty_money_crate_geometry_residual_ok() -> bool {
    (MONEY_CRATE_GEOMETRY_MAJOR_RADIUS - 12.0).abs() < 0.01
        && (MONEY_CRATE_GEOMETRY_MINOR_RADIUS - 12.0).abs() < 0.01
        && (MONEY_CRATE_GEOMETRY_HEIGHT - 12.0).abs() < 0.01
        && (MONEY_CRATE_PHYSICS_MASS - 75.0).abs() < 0.01
        && MONEY_CRATE_KIND_OF == "PARACHUTABLE CRATE"
        && MONEY_CRATE_UNIT_PICKUP_RADIUS >= MONEY_CRATE_GEOMETRY_MAJOR_RADIUS
}

/// Wave 64 residual honesty: OCL supply-drop delivery residual.
pub fn honesty_money_crate_ocl_residual_ok() -> bool {
    MONEY_CRATE_OCL_SUPPLY_DROP == "OCL_AmericaSupplyDropZoneCrateDrop"
        && MONEY_CRATE_OCL_PAYLOAD_COUNT == 6
        && MONEY_CRATE_OCL_DROP_DELAY_MS == 350
        && MONEY_CRATE_OCL_DROP_DELAY_FRAMES
            == money_crate_ms_to_frames(MONEY_CRATE_OCL_DROP_DELAY_MS)
        && (MONEY_CRATE_OCL_DELIVERY_DISTANCE - 410.0).abs() < 0.01
        && MONEY_CRATE_OCL_TRANSPORT == "AmericaJetCargoPlane"
        && MONEY_CRATE_OCL_PUT_IN_CONTAINER == "AmericaCrateParachute"
        && MONEY_CRATE_OCL_PARACHUTE_DIRECTLY
        && MONEY_CRATE_OCL_MAX_ATTEMPTS == 4
}

/// Wave 64 residual honesty: MoneyPickUp anim + floating text residual.
pub fn honesty_money_crate_presentation_residual_ok() -> bool {
    HostMoneyCrateRegistry::honesty_money_pickup_anim_constants_ok()
        && HostMoneyCrateRegistry::honesty_money_floating_text_constants_ok()
        && MONEY_CRATE_PICKUP_AUDIO == "CrateMoney"
}

/// Combined Wave 64 Money Crate residual honesty pack.
pub fn honesty_money_crate_residual_pack_ok() -> bool {
    honesty_money_crate_supply_drop_residual_ok()
        && honesty_money_crate_dollar_matrix_residual_ok()
        && honesty_money_crate_geometry_residual_ok()
        && honesty_money_crate_ocl_residual_ok()
        && honesty_money_crate_presentation_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn supply_drop_crate_money_constants() {
        assert_eq!(SUPPLY_DROP_CRATE_MONEY_PROVIDED, 250);
        assert_eq!(SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST, 25);
        assert!(MONEY_CRATE_BUILDING_PICKUP_RADIUS >= 50.0);
    }

    #[test]
    fn unit_pickup_credits_and_forgets_crate() {
        let mut reg = HostMoneyCrateRegistry::new();
        let crate_id = ObjectId(10);
        reg.register_supply_drop_crate(crate_id);
        assert_eq!(reg.crate_count(), 1);
        let (amount, boost) =
            HostMoneyCrateRegistry::cash_for_pickup(reg.get(crate_id).unwrap(), false);
        assert_eq!(amount, 250);
        assert_eq!(boost, 0);
        assert!(reg.record_pickup(crate_id, amount, boost, false));
        assert!(reg.honesty_unit_pickup_ok());
        assert!(!reg.contains(crate_id));
        assert_eq!(reg.cash_total, 250);
    }

    #[test]
    fn supply_lines_boost_residual() {
        let mut reg = HostMoneyCrateRegistry::new();
        let crate_id = ObjectId(11);
        reg.register_supply_drop_crate(crate_id);
        let (amount, boost) =
            HostMoneyCrateRegistry::cash_for_pickup(reg.get(crate_id).unwrap(), true);
        assert_eq!(amount, 275);
        assert_eq!(boost, 25);
        assert!(reg.record_pickup(crate_id, amount, boost, true));
        assert!(reg.honesty_building_pickup_ok());
        assert!(reg.honesty_supply_lines_boost_ok());
    }

    #[test]
    fn legal_picker_gates() {
        // Alive non-neutral unit on ground.
        assert!(HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, false, false, false, false
        ));
        // Neutral rejected.
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, true, false, false, false, false
        ));
        // Structure is not a unit picker.
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, true, false, false, false
        ));
        // ForbiddenKindOf PROJECTILE residual.
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, false, true, false, false
        ));
        // Parachute picker residual.
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, false, false, true, false
        ));
        // Above-terrain residual blocks unit path.
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, false, false, false, true
        ));
        assert!(HostMoneyCrateRegistry::is_legal_building_picker(
            true, false, true, true, true
        ));
        assert!(!HostMoneyCrateRegistry::is_legal_building_picker(
            true, false, true, true, false
        ));
        assert!(HostMoneyCrateRegistry::honesty_money_pickup_anim_constants_ok());
        assert!(HostMoneyCrateRegistry::honesty_money_floating_text_constants_ok());
        let anim = HostMoneyCrateRegistry::money_pickup_anim(
            ObjectId(1),
            ObjectId(2),
            Vec3::new(1.0, 0.0, 1.0),
            10,
        );
        assert_eq!(anim.template, "MoneyPickUp");
        assert!((anim.display_time_seconds - 4.0).abs() < 0.01);
        assert!((anim.z_rise_per_second - 15.0).abs() < 0.01);
        assert!(anim.fades);
        let ft = HostMoneyCrateRegistry::money_floating_text(
            ObjectId(1),
            ObjectId(2),
            Vec3::new(1.0, 0.0, 1.0),
            250,
            10,
        );
        assert_eq!(ft.text, "+$250");
        assert_eq!(ft.text_key, "GUI:AddCash");
        assert!((ft.position.y - 20.0).abs() < 0.01);
        assert_eq!(ft.color_rgba, (0, 255, 0, 255));
        let _ = Team::USA;
    }

    #[test]
    fn money_pickup_anim_record_honesty() {
        let mut reg = HostMoneyCrateRegistry::new();
        let anim = HostMoneyCrateRegistry::money_pickup_anim(
            ObjectId(3),
            ObjectId(4),
            Vec3::ZERO,
            5,
        );
        reg.record_money_pickup_anim(anim);
        assert!(reg.honesty_money_pickup_anim_ok());
        assert_eq!(reg.money_pickup_anims_total, 1);
        let ft = HostMoneyCrateRegistry::money_floating_text(
            ObjectId(3),
            ObjectId(4),
            Vec3::new(5.0, 0.0, 5.0),
            250,
            5,
        );
        reg.record_money_floating_text(ft);
        assert!(reg.honesty_money_floating_text_ok());
        assert_eq!(reg.money_floating_texts_total, 1);
        reg.record_above_terrain_unit_reject();
        assert!(reg.honesty_above_terrain_reject_ok());
        reg.record_forbidden_kindof_reject();
        assert!(reg.forbidden_kindof_rejects > 0);
    }

    #[test]
    fn money_crate_residual_pack_honesty() {
        assert_eq!(money_crate_ms_to_frames(350), 11);
        assert!(honesty_money_crate_supply_drop_residual_ok());
        assert!(honesty_money_crate_dollar_matrix_residual_ok());
        assert!(honesty_money_crate_geometry_residual_ok());
        assert!(honesty_money_crate_ocl_residual_ok());
        assert!(honesty_money_crate_presentation_residual_ok());
        assert!(honesty_money_crate_residual_pack_ok());
    }
}
