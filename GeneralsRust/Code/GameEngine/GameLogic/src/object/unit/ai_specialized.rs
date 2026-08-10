//! Dozer, deliver-payload, supply/POW/hack, guard, and prone AI helpers.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{
    dual_world_registry_unavailable, get_unit_arc, with_unit_mut, with_unit_ref,
};
use super::types::*;

impl UnitAIUpdate {
    pub(super) fn get_supply_truck_ai_interface(
        &self,
    ) -> Option<&dyn crate::modules::SupplyTruckAIInterface> {
        if let Some(ai) = self.chinook_ai.as_ref() {
            Some(ai as &dyn crate::modules::SupplyTruckAIInterface)
        } else if let Some(ai) = self.supply_truck_ai.as_ref() {
            Some(ai as &dyn crate::modules::SupplyTruckAIInterface)
        } else {
            self.worker_ai
                .as_ref()
                .map(|ai| ai as &dyn crate::modules::SupplyTruckAIInterface)
        }
    }
    pub(super) fn get_supply_truck_ai_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SupplyTruckAIInterface> {
        if let Some(ai) = self.chinook_ai.as_mut() {
            Some(ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        } else if let Some(ai) = self.supply_truck_ai.as_mut() {
            Some(ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        } else {
            self.worker_ai
                .as_mut()
                .map(|ai| ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        }
    }
    pub(super) fn get_pow_truck_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::POWTruckAIUpdateInterface> {
        #[cfg(feature = "allow_surrender")]
        {
            return self
                .pow_truck_ai
                .as_mut()
                .map(|ai| ai as &mut dyn crate::modules::POWTruckAIUpdateInterface);
        }
        #[cfg(not(feature = "allow_surrender"))]
        {
            None
        }
    }
    pub(super) fn get_hack_internet_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::HackInternetAIUpdateInterface> {
        self.hack_internet_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::HackInternetAIUpdateInterface)
    }
    pub(super) fn get_assault_transport_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::AssaultTransportAIUpdateInterface> {
        self.assault_transport_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::AssaultTransportAIUpdateInterface)
    }
    pub(super) fn get_worker_ai_update_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::WorkerAIUpdateInterface> {
        self.worker_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::WorkerAIUpdateInterface)
    }
    pub(super) fn get_dozer_ai_update_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::DozerAIUpdateInterface> {
        self.dozer_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::DozerAIUpdateInterface)
    }
    pub(super) fn get_deliver_payload_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::DeliverPayloadAIUpdateInterface> {
        self.deliver_payload_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::DeliverPayloadAIUpdateInterface)
    }
    pub(super) fn ai_guard_object(
        &mut self,
        target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 258: empty dual-world → Ok(()).

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let target_pos = crate::helpers::TheGameLogic::find_object_by_id(target_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(target_id))
            .and_then(|arc| arc.read().ok().map(|g| *g.get_position()))
            .ok_or("guard target not found")?;
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        self.push_guard_target_type(GuardTargetType::Object);
        self.object_to_guard = target_id;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.current_order = Some(UnitOrder::Guard {
            position: target_pos,
            area_radius: guard.engagement_range,
        });
        guard.order_queue.clear();
        Ok(())
    }
    pub(super) fn ai_go_prone(
        &mut self,
        damage_info: &DamageInfo,
        _cmd_source: crate::ai::CommandSourceType,
    ) {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return;
        };
        let Ok(unit_guard) = unit.read() else {
            return;
        };
        let obj_arc = unit_guard.base_arc();
        let module = {
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            obj_guard.find_update_module("ProneUpdate")
        };
        let Some(module) = module else {
            return;
        };
        let damage = damage_info.output.actual_damage_dealt as i32;
        module.with_module(|module| {
            if let Some(prone) = module.get_prone_control_interface() {
                prone.go_prone(damage);
            }
        });
    }
}
