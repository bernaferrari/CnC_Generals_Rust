//! Unit AI runtime preparation for ObjectFactory.

use super::*;

/// Construct the Unit AI runtime from the same authored module-data precedence
/// used by ObjectFactory, then apply the primary AI module data before sharing it.
pub(super) fn prepare_unit_ai(
    base_object: &Arc<RwLock<Object>>,
    template: &dyn ThingTemplate,
    object_id: ObjectID,
) -> Arc<Mutex<UnitAIUpdate>> {
    let needs_supply_ai = template.is_kind_of(KindOf::Harvester);
    #[cfg(feature = "allow_surrender")]
    let needs_pow_truck_ai = template
        .get_behavior_module_info()
        .iter()
        .any(|entry| entry.name.as_str() == "POWTruckBehavior");

    let ai_update_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "AIUpdateInterface")
        .and_then(|entry| entry.data.as_ref().downcast_ref::<AIUpdateModuleData>())
        .map(|data| data.clone());

    let railed_transport_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "RailedTransportAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<RailedTransportAIUpdateModuleData>()
        })
        .cloned();

    let hack_internet_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "HackInternetAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<HackInternetAIUpdateModuleData>()
        })
        .cloned();

    let assault_transport_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "AssaultTransportAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<AssaultTransportAIUpdateModuleData>()
        })
        .cloned();

    let deliver_payload_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "DeliverPayloadAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<DeliverPayloadAIUpdateModuleData>()
        })
        .cloned();

    let deploy_style_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "DeployStyleAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<DeployStyleAIUpdateModuleData>()
        })
        .cloned();

    let transport_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "TransportAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<TransportAIUpdateModuleData>()
        })
        .cloned();
    let has_transport_ai = transport_ai_module_data.is_some();

    let wander_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "WanderAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<WanderAIUpdateModuleData>()
        })
        .cloned();
    let has_wander_ai = wander_ai_module_data.is_some();

    let dozer_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "DozerAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<DozerAIUpdateModuleData>()
        })
        .cloned();

    let chinook_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "ChinookAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<ChinookAIUpdateModuleData>()
        })
        .cloned();

    let jet_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "JetAIUpdate")
        .and_then(|entry| entry.data.as_ref().downcast_ref::<JetAIUpdateModuleData>())
        .cloned();

    let supply_truck_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "SupplyTruckAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<SupplyTruckAIUpdateModuleData>()
        })
        .cloned();

    let worker_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "WorkerAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<WorkerAIUpdateModuleData>()
        })
        .cloned();

    #[cfg(feature = "allow_surrender")]
    let pow_truck_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "POWTruckAIUpdate")
        .and_then(|entry| {
            entry
                .data
                .as_ref()
                .downcast_ref::<POWTruckAIUpdateModuleData>()
        })
        .map(|data| data.base.clone());

    let needs_worker_ai = template
        .get_behavior_module_info()
        .iter()
        .any(|entry| entry.name.as_str() == "WorkerAIUpdate");

    let needs_dozer_ai = template
        .get_behavior_module_info()
        .iter()
        .any(|entry| entry.name.as_str() == "DozerAIUpdate");

    let needs_chinook_ai = template
        .get_behavior_module_info()
        .iter()
        .any(|entry| entry.name.as_str() == "ChinookAIUpdate");

    let needs_jet_ai = template
        .get_behavior_module_info()
        .iter()
        .any(|entry| entry.name.as_str() == "JetAIUpdate");

    let supply_ai = if needs_supply_ai {
        let player_index = base_object
            .read()
            .ok()
            .and_then(|obj| obj.get_controlling_player_id())
            .unwrap_or(0) as PlayerIndex;
        let data = supply_truck_ai_module_data.clone().map_or_else(
            SupplyTruckAIUpdateData::default,
            |data| SupplyTruckAIUpdateData {
                max_boxes: data.max_boxes_data,
                warehouse_scan_distance: data.warehouse_scan_distance,
                warehouse_delay: data.warehouse_delay,
                center_delay: data.center_delay,
                supplies_depleted_voice: data.supplies_depleted_voice.to_string(),
            },
        );
        Some(SupplyTruckAIUpdate::new(
            data,
            object_id,
            player_index as crate::supply_system::PlayerIndex,
        ))
    } else {
        None
    };

    let worker_ai = if needs_worker_ai {
        let player_index = base_object
            .read()
            .ok()
            .and_then(|obj| obj.get_controlling_player_id())
            .unwrap_or(0) as PlayerIndex;
        let data = worker_ai_module_data
            .clone()
            .map_or_else(WorkerAIUpdateData::default, |data| WorkerAIUpdateData {
                max_boxes: data.max_boxes_data,
                warehouse_scan_distance: data.warehouse_scan_distance,
                warehouse_delay: data.warehouse_delay,
                center_delay: data.center_delay,
                supplies_depleted_voice: data.supplies_depleted_voice.to_string(),
                repair_health_percent_per_second: data.repair_health_percent_per_second,
                bored_time: data.bored_time,
                bored_range: data.bored_range,
                upgraded_supply_boost: data.upgraded_supply_boost.max(0) as u32,
            });
        Some(WorkerAIUpdate::new(
            data,
            object_id,
            player_index as crate::supply_system::PlayerIndex,
        ))
    } else {
        None
    };

    let dozer_ai = if needs_dozer_ai {
        let data = dozer_ai_module_data
            .clone()
            .map_or_else(DozerAIUpdateData::default, |data| DozerAIUpdateData {
                repair_health_percent_per_second: data.repair_health_percent_per_second,
                bored_time: data.bored_time,
                bored_range: data.bored_range,
            });
        Some(DozerAIUpdate::new(data, object_id))
    } else {
        None
    };

    let mut chinook_ai = if needs_chinook_ai {
        let player_index = base_object
            .read()
            .ok()
            .and_then(|obj| obj.get_controlling_player_id())
            .unwrap_or(0) as PlayerIndex;
        let data = chinook_ai_module_data
            .clone()
            .map_or_else(ChinookAIUpdateData::default, |data| {
                ChinookAIUpdateData::from_module(&data)
            });
        Some(ChinookAIUpdate::new(data, object_id, player_index))
    } else {
        None
    };
    if let Some(ref mut chinook_ai) = chinook_ai {
        if let Ok(obj_guard) = base_object.read() {
            chinook_ai.record_original_position(*obj_guard.get_position());
        }
    }

    let jet_ai = if needs_jet_ai {
        jet_ai_module_data
            .as_ref()
            .map(|data| JetAIUpdate::new(data.clone(), object_id))
    } else {
        None
    };

    #[cfg(feature = "allow_surrender")]
    let pow_truck_ai = if needs_pow_truck_ai {
        let data = pow_truck_ai_module_data.unwrap_or_else(POWTruckAIUpdateData::default);
        Some(POWTruckAIUpdate::new(data, object_id))
    } else {
        None
    };

    let railed_transport_ai = railed_transport_ai_module_data.as_ref().map(|data| {
        let data = RailedTransportAIUpdateData {
            path_prefix_name: data.path_prefix_name.clone(),
        };
        RailedTransportAIUpdate::new(data, object_id)
    });

    let hack_internet_ai = hack_internet_ai_module_data.as_ref().map(|data| {
        let data = HackInternetAIUpdateData {
            unpack_time: data.unpack_time,
            pack_time: data.pack_time,
            cash_update_delay: data.cash_update_delay,
            cash_update_delay_fast: data.cash_update_delay_fast,
            regular_cash_amount: data.regular_cash_amount,
            veteran_cash_amount: data.veteran_cash_amount,
            elite_cash_amount: data.elite_cash_amount,
            heroic_cash_amount: data.heroic_cash_amount,
            xp_per_cash_update: data.xp_per_cash_update,
            pack_unpack_variation_factor: data.pack_unpack_variation_factor,
        };
        HackInternetAIUpdate::new(data, object_id)
    });

    let assault_transport_ai = assault_transport_ai_module_data.as_ref().map(|data| {
        let data = AssaultTransportAIUpdateData {
            members_get_healed_at_life_ratio: data.members_get_healed_at_life_ratio,
            clear_range_required_to_continue_attack_move: data
                .clear_range_required_to_continue_attack_move,
        };
        AssaultTransportAIUpdate::new(data, object_id)
    });

    let deliver_payload_ai = deliver_payload_ai_module_data
        .as_ref()
        .map(|data| DeliverPayloadAIUpdate::new(data.clone(), object_id));

    let deploy_style_ai = deploy_style_ai_module_data.as_ref().map(|data| {
        let data = DeployStyleAIUpdateData {
            unpack_time: data.unpack_time,
            pack_time: data.pack_time,
            reset_turret_before_packing: data.reset_turret_before_packing,
            turrets_function_only_when_deployed: data.turrets_function_only_when_deployed,
            turrets_must_center_before_packing: data.turrets_must_center_before_packing,
            manual_deploy_animations: data.manual_deploy_animations,
        };
        DeployStyleAIUpdate::new(data, object_id)
    });

    let transport_ai = has_transport_ai.then(|| TransportAIUpdate::new(object_id));
    let wander_ai = has_wander_ai.then(|| WanderAIUpdate::new(object_id));

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        railed_transport_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        hack_internet_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        assault_transport_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        deliver_payload_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        deploy_style_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        chinook_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data =
        ai_update_module_data.or_else(|| jet_ai_module_data.as_ref().map(|data| data.base.clone()));

    let ai_update_module_data = ai_update_module_data
        .or_else(|| dozer_ai_module_data.as_ref().map(|data| data.base.clone()));

    let ai_update_module_data = ai_update_module_data
        .or_else(|| worker_ai_module_data.as_ref().map(|data| data.base.clone()));

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        supply_truck_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data.or_else(|| {
        transport_ai_module_data
            .as_ref()
            .map(|data| data.base.clone())
    });

    let ai_update_module_data = ai_update_module_data
        .or_else(|| wander_ai_module_data.as_ref().map(|data| data.base.clone()));

    let mut ai_update = UnitAIUpdate::new(
        object_id,
        supply_ai,
        chinook_ai,
        jet_ai,
        worker_ai,
        dozer_ai,
        #[cfg(feature = "allow_surrender")]
        pow_truck_ai,
        railed_transport_ai,
        hack_internet_ai,
        assault_transport_ai,
        deliver_payload_ai,
        transport_ai,
        deploy_style_ai,
        wander_ai,
    );

    if let Some(data) = ai_update_module_data {
        ai_update.apply_ai_update_module_data(&data);
    }
    Arc::new(Mutex::new(ai_update))
}
