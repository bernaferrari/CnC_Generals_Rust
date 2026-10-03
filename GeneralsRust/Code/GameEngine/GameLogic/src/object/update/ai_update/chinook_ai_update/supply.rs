//! Supply availability, flight queries and supply interface delegation.

use super::{
    ChinookAIUpdate, ChinookFlightStatus, chinook_free_to_exit, dual_world_registry_unavailable,
};
use crate::common::{KindOf, ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterfaceExt, SupplyTruckAIInterface};
use crate::object::Object;
use crate::player::player_list;
use crate::supply_system::SupplyTruckState;
use crate::upgrade::center::get_upgrade_center;

impl ChinookAIUpdate {
    pub fn is_idle(&self) -> bool {
        // Wave 349: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.pending_command.is_some() {
            return false;
        }
        let mut result = self.base.get_state() == SupplyTruckState::Idle;
        if result && self.flight_status == ChinookFlightStatus::Landed {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
                if let Some(contain) = guard.get_contain() {
                    if contain.has_objects_wanting_to_enter_or_exit() {
                        result = false;
                    }
                }
            });
        }
        result
    }

    pub fn is_currently_ferrying_supplies(&self) -> bool {
        self.base.is_currently_ferrying_supplies()
    }

    pub fn is_available_for_supplying(&self) -> bool {
        // Wave 349: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if !self.base.is_available_for_supplying() {
            return false;
        }
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return false;
        };
        let Ok(guard) = owner.read() else {
            return false;
        };
        let Some(contain) = guard.get_contain() else {
            return false;
        };
        if contain.has_objects_wanting_to_enter_or_exit() {
            return false;
        }
        if contain.get_contained_count() > 0 {
            return false;
        }
        if contain.is_special_overlord_style_container() {
            return false;
        }
        true
    }

    pub fn is_allowed_to_adjust_destination(&self) -> bool {
        self.flight_status != ChinookFlightStatus::Landed
    }

    pub fn is_landed(&self) -> bool {
        self.flight_status == ChinookFlightStatus::Landed
    }

    pub fn get_ai_free_to_exit(
        &self,
        exiter: &Object,
    ) -> crate::object::production::AIFreeToExitType {
        if chinook_free_to_exit(
            self.flight_status == ChinookFlightStatus::Landed,
            self.flight_status == ChinookFlightStatus::DoingCombatDrop,
            exiter.is_kind_of(KindOf::CanRappel),
        ) {
            crate::object::production::AIFreeToExitType::FreeToExit
        } else {
            crate::object::production::AIFreeToExitType::WaitToExit
        }
    }

    pub fn get_upgraded_supply_boost(&self) -> u32 {
        // Wave 349: empty dual-world → 0.
        if dual_world_registry_unavailable() {
            return 0;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return 0;
        };
        let Ok(owner_guard) = owner.read() else {
            return 0;
        };
        let Some(player_id) = owner_guard.get_controlling_player_id() else {
            return 0;
        };
        let upgrade = get_upgrade_center()
            .read()
            .ok()
            .and_then(|center| center.find_upgrade("Upgrade_AmericaSupplyLines"));
        if let Some(upgrade) = upgrade {
            let player_has = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(player_id as i32).cloned())
                .and_then(|player| {
                    let guard = player.read().ok()?;
                    Some(guard.has_upgrade_complete(&upgrade))
                })
                .unwrap_or(false);
            if player_has {
                return self.data.upgraded_supply_boost.max(0) as u32;
            }
        }
        0
    }
}

impl SupplyTruckAIInterface for ChinookAIUpdate {
    fn get_supplies_count(&self) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.base.get_number_boxes())
    }

    fn get_number_boxes(&self) -> i32 {
        self.base.get_number_boxes()
    }

    fn get_action_delay_for_dock(
        &self,
        dock_id: ObjectID,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        SupplyTruckAIInterface::get_action_delay_for_dock(&self.base, dock_id)
    }

    fn set_force_wanting_state(&mut self, enabled: bool) {
        self.base.set_force_wanting_state(enabled);
    }

    fn is_forced_into_wanting_state(&self) -> bool {
        self.base.is_forced_into_wanting_state()
    }

    fn set_force_busy_state(&mut self, enabled: bool) {
        self.base.set_force_busy_state(enabled);
    }

    fn is_forced_into_busy_state(&self) -> bool {
        self.base.is_forced_into_busy_state()
    }

    fn get_preferred_dock_id(&self) -> Option<ObjectID> {
        self.base.get_preferred_dock()
    }

    fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Option<Real> {
        Some(self.base.get_warehouse_scan_distance(is_ai_player))
    }

    fn is_available_for_supplying(&self) -> bool {
        self.is_available_for_supplying()
    }

    fn is_currently_ferrying_supplies(&self) -> bool {
        self.is_currently_ferrying_supplies()
    }

    fn lose_one_box(&mut self) -> bool {
        self.base.lose_one_box()
    }

    fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        self.base.gain_one_box(remaining_stock)
    }

    fn get_upgraded_supply_boost(&self) -> u32 {
        self.get_upgraded_supply_boost()
    }
}
