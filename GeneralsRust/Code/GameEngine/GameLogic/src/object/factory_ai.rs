//! Unit AI runtime preparation for ObjectFactory.

use super::*;

/// Construct the Unit AI runtime from the same authored module-data precedence
/// used by ObjectFactory, then apply the primary AI module data before sharing it.
pub(in crate::object) fn prepare_unit_ai(
    base_object: &Arc<RwLock<Object>>,
    template: &dyn ThingTemplate,
    object_id: ObjectID,
) -> Arc<Mutex<UnitAIUpdate>> {
    prepare_unit_ai_impl(base_object, template, object_id, None)
        .expect("legacy Unit preparation has no fallible receiving catalog")
}
pub(in crate::object) fn prepare_unit_ai_with_locomotors(
    base_object: &Arc<RwLock<Object>>,
    template: &dyn ThingTemplate,
    object_id: ObjectID,
    lookup: &mut crate::locomotor::LocomotorTemplateLookup<'_>,
) -> Result<Arc<Mutex<UnitAIUpdate>>, String> {
    prepare_unit_ai_impl(base_object, template, object_id, Some(lookup))
}
fn prepare_unit_ai_impl(
    base_object: &Arc<RwLock<Object>>,
    template: &dyn ThingTemplate,
    object_id: ObjectID,
    lookup: Option<&mut crate::locomotor::LocomotorTemplateLookup<'_>>,
) -> Result<Arc<Mutex<UnitAIUpdate>>, String> {
    if lookup.is_some()
        && (template.is_kind_of(KindOf::Harvester)
            || template.get_behavior_module_info().iter().any(|entry| {
                let data = entry.data.read();
                data.is_ai_module_data()
                    && !(entry.name.as_str() == "AIUpdateInterface"
                        || (entry.name.as_str() == "DozerAIUpdate"
                            && data.downcast_ref::<DozerAIUpdateModuleData>().is_some()))
            }))
    {
        return Err(
            "receiving Unit constructor does not admit specialized AI birth callbacks".into(),
        );
    }
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
        .and_then(|entry| entry.data.read().downcast_ref::<AIUpdateModuleData>().cloned());

    let railed_transport_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "RailedTransportAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<RailedTransportAIUpdateModuleData>().cloned());

    let hack_internet_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "HackInternetAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<HackInternetAIUpdateModuleData>().cloned());

    let assault_transport_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "AssaultTransportAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<AssaultTransportAIUpdateModuleData>().cloned());

    let deliver_payload_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "DeliverPayloadAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<DeliverPayloadAIUpdateModuleData>().cloned());

    let deploy_style_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "DeployStyleAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<DeployStyleAIUpdateModuleData>().cloned());

    let transport_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "TransportAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<TransportAIUpdateModuleData>().cloned());
    let has_transport_ai = transport_ai_module_data.is_some();

    let wander_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "WanderAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<WanderAIUpdateModuleData>().cloned());
    let has_wander_ai = wander_ai_module_data.is_some();

    let dozer_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "DozerAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<DozerAIUpdateModuleData>().cloned());

    let chinook_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "ChinookAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<ChinookAIUpdateModuleData>().cloned());

    let jet_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "JetAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<JetAIUpdateModuleData>().cloned());

    let supply_truck_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "SupplyTruckAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<SupplyTruckAIUpdateModuleData>().cloned());

    let worker_ai_module_data = template
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "WorkerAIUpdate")
        .and_then(|entry| entry.data.read().downcast_ref::<WorkerAIUpdateModuleData>().cloned());


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
        {
            let mut ai = if let Some(entry) = template.get_behavior_module_info().iter()
                .find(|entry| entry.name.as_str() == "SupplyTruckAIUpdate")
            {
                SupplyTruckAIUpdate::new_with_module_data(
                    entry.data.clone(), object_id, player_index as crate::supply_system::PlayerIndex,
                )
            } else {
                SupplyTruckAIUpdate::new(
                    SupplyTruckAIUpdateData::default(), object_id,
                    player_index as crate::supply_system::PlayerIndex,
                )
            };
            ai.bind_owner(base_object);
            Some(ai)
        }
    } else {
        None
    };

    let worker_ai = if needs_worker_ai {
        let player_index = base_object
            .read()
            .ok()
            .and_then(|obj| obj.get_controlling_player_id())
            .unwrap_or(0) as PlayerIndex;
        {
            let entry = template.get_behavior_module_info().iter()
                .find(|entry| entry.name.as_str() == "WorkerAIUpdate")
                .expect("worker descriptor selected above");
            let mut ai = WorkerAIUpdate::new_with_module_data(
                entry.data.clone(),
                object_id,
                player_index as crate::supply_system::PlayerIndex,
            );
            ai.bind_owner(base_object);
            Some(ai)
        }
    } else {
        None
    };

    let dozer_ai = if needs_dozer_ai {
        template.get_behavior_module_info().iter()
            .find(|entry| entry.name.as_str() == "DozerAIUpdate")
            .map(|entry| DozerAIUpdate::new(entry.data.clone(), object_id))
    } else {
        None
    };

    let mut chinook_ai = if needs_chinook_ai {
        let player_index = base_object
            .read()
            .ok()
            .and_then(|obj| obj.get_controlling_player_id())
            .unwrap_or(0) as PlayerIndex;
        template.get_behavior_module_info().iter()
            .find(|entry| entry.name.as_str() == "ChinookAIUpdate")
            .map(|entry| ChinookAIUpdate::new(entry.data.clone(), object_id, player_index))
    } else {
        None
    };
    if let Some(ref mut chinook_ai) = chinook_ai {
        if let Ok(obj_guard) = base_object.read() {
            chinook_ai.record_original_position(*obj_guard.get_position());
        }
    }

    let jet_ai = if needs_jet_ai {
        template.get_behavior_module_info().iter()
            .find(|entry| entry.name.as_str() == "JetAIUpdate")
            .map(|entry| JetAIUpdate::new(entry.data.clone(), object_id))
    } else {
        None
    };

    #[cfg(feature = "allow_surrender")]
    let pow_truck_ai = if needs_pow_truck_ai {
        let data = template.get_behavior_module_info().iter()
            .find(|entry| entry.name.as_str() == "POWTruckAIUpdate")
            .map(|entry| entry.data.clone())
            .unwrap_or_else(|| ModuleDataHandle::new(POWTruckAIUpdateModuleData::default()));
        Some(POWTruckAIUpdate::new(data, object_id))
    } else {
        None
    };

    let railed_transport_ai = template.get_behavior_module_info().iter()
        .find(|entry| entry.name.as_str() == "RailedTransportAIUpdate")
        .map(|entry| RailedTransportAIUpdate::new(entry.data.clone(), object_id));

    let hack_internet_ai = template.get_behavior_module_info().iter()
        .find(|entry| entry.name.as_str() == "HackInternetAIUpdate")
        .map(|entry| HackInternetAIUpdate::new(entry.data.clone(), object_id));

    let assault_transport_ai = template.get_behavior_module_info().iter()
        .find(|entry| entry.name.as_str() == "AssaultTransportAIUpdate")
        .map(|entry| AssaultTransportAIUpdate::new(entry.data.clone(), object_id));

    let deliver_payload_ai = template.get_behavior_module_info().iter()
        .find(|entry| entry.name.as_str() == "DeliverPayloadAIUpdate")
        .map(|entry| DeliverPayloadAIUpdate::new(entry.data.clone(), object_id));

    let deploy_style_ai = template.get_behavior_module_info().iter()
        .find(|entry| entry.name.as_str() == "DeployStyleAIUpdate")
        .map(|entry| DeployStyleAIUpdate::new(entry.data.clone(), object_id));

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

    let mut ai_update = UnitAIUpdate::new_for_object(
        object_id,
        base_object,
        crate::object::unit::UnitAiComponents {
            supply_truck_ai: supply_ai,
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
        },
    );

    if let Some(data) = ai_update_module_data {
        if let Some(lookup) = lookup {
            ai_update.apply_factory_ai_update_module_data_with_locomotors(&data, lookup)?;
        } else {
            ai_update.apply_factory_ai_update_module_data(&data);
        }
    }
    Ok(Arc::new(Mutex::new(ai_update)))
}
