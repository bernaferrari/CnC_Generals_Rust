//! CPP RiderChangeContain.cpp:182-238 onContaining: an owned-path rider
//! boards the bike and the matching RiderInfo is applied to the owner.
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::common::ObjectStatusMaskType;
use crate::object::contain::rider_change_contain::{RiderChangeContainModuleData, RiderInfo};
use crate::weapon::WeaponSetType;

fn owned_bike_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeOwnedRiderBike".into());
    base.add_kind_of(KindOf::Vehicle);
    let mut data = RiderChangeContainModuleData::default();
    data.base.slot_capacity = 1;
    data.base.base.contain_max = 1;
    data.base.base.number_of_exit_paths = 0;
    data.base.base.allow_inside_kind_of = KindOf::Infantry.cpp_mask();
    data.riders[0] = RiderInfo {
        template_name: "NativeEnterInfantry".into(),
        model_condition_flag: crate::common::ModelConditionFlags::RIDER1,
        weapon_set_flag: WeaponSetType::WeaponRider1,
        object_status: ObjectStatusMaskType::RIDER1,
        ..Default::default()
    };
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "RiderChangeContain".into(),
            module_tag: "NativeOwnedBikeContain".into(),
            data: game_engine::common::thing::module::ModuleDataHandle::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
    })
}

#[test]
fn rider_boards_rider_change_bike_on_owned_path_and_applies_rider_info() {
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if crate::system::engine_stores::try_active_services().is_none() {
        crate::system::engine_stores::install_active(crate::system::engine_stores::new_for_world());
    }
    let mut game = NativeMatch::with_container(50.0, owned_bike_template());
    let rider_id = game.source.read().unwrap().get_id();
    let bike_id = game.target.read().unwrap().get_id();
    game.operate(117, |world| world.enter(rider_id, bike_id)).unwrap();
    let owner = game.target.read().unwrap();
    let rider = game.source.read().unwrap();
    assert_eq!(
        owner.get_contain().unwrap().lock().unwrap().get_contained_objects().as_ref(),
        &[rider.get_id()]
    );
    assert!(owner.get_status_bits().test(crate::common::ObjectStatusTypes::Rider1));
    assert!(owner.test_weapon_set_flag(WeaponSetType::WeaponRider1));
}

fn body_module(tag: &str) -> TemplateModuleInfo {
    let mut body = crate::object::body::active_body::ActiveBodyModuleData::default();
    body.max_health = 100.0;
    body.initial_health = 100.0;
    TemplateModuleInfo {
        name: "ActiveBody".into(),
        module_tag: tag.into(),
        data: game_engine::common::thing::module::ModuleDataHandle::new(body),
        interface_mask: ModuleInterfaceType::BODY,
    }
}

fn ai_module(tag: &str, loco: &str) -> TemplateModuleInfo {
    let mut l = crate::locomotor::LocomotorTemplate::new(loco.into());
    l.surfaces = crate::locomotor::SURFACE_GROUND;
    l.max_speed = 3.0;
    l.acceleration = 0.1;
    crate::system::engine_stores::active_services().locomotor_store().register_template(l);
    let mut data = crate::object::update::AIUpdateModuleData::default();
    data.set_locomotor_set_entries(crate::common::LocomotorSetType::Normal, vec![loco.into()]);
    TemplateModuleInfo {
        name: "AIUpdateInterface".into(),
        module_tag: tag.into(),
        data: game_engine::common::thing::module::ModuleDataHandle::new(data),
        interface_mask: ModuleInterfaceType::UPDATE,
    }
}

fn mortal_rider_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeMortalBikeRider".into());
    base.add_kind_of(KindOf::Infantry);
    Arc::new(EntryTemplate {
        base,
        modules: vec![
            ai_module("NativeMortalRiderAI", "NativeMortalRiderGround"),
            body_module("NativeMortalRiderBody"),
        ],
    })
}

fn mortal_bike_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeMortalRiderBike".into());
    base.add_kind_of(KindOf::Vehicle);
    let mut data = RiderChangeContainModuleData::default();
    data.base.slot_capacity = 1;
    data.base.base.contain_max = 1;
    data.base.base.number_of_exit_paths = 0;
    data.base.base.allow_inside_kind_of = KindOf::Infantry.cpp_mask();
    data.riders[0] = RiderInfo {
        template_name: "NativeMortalBikeRider".into(),
        model_condition_flag: crate::common::ModelConditionFlags::RIDER1,
        weapon_set_flag: WeaponSetType::WeaponRider1,
        object_status: ObjectStatusMaskType::RIDER1,
        ..Default::default()
    };
    Arc::new(EntryTemplate {
        base,
        modules: vec![
            TemplateModuleInfo {
                name: "RiderChangeContain".into(),
                module_tag: "NativeMortalBikeContain".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
                interface_mask: ModuleInterfaceType::CONTAIN,
            },
            ai_module("NativeMortalBikeAI", "NativeMortalBikeGround"),
            body_module("NativeMortalBikeBody"),
        ],
    })
}

/// CPP ActiveBody.cpp:386-418 KillPilot on a stationary RiderChangeContain
/// bike: aiEvacuateInstantly, then damager->scoreTheKill(rider), rider->kill().
#[test]
fn stationary_kill_pilot_evacuates_and_kills_rider_on_owned_path() {
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if crate::system::engine_stores::try_active_services().is_none() {
        crate::system::engine_stores::install_active(crate::system::engine_stores::new_for_world());
    }
    let mut game =
        NativeMatch::with_source_template(50.0, mortal_bike_template(), false, mortal_rider_template());
    let rider_id = game.source.read().unwrap().get_id();
    let bike_id = game.target.read().unwrap().get_id();
    game.operate(117, |world| world.enter(rider_id, bike_id)).unwrap();
    assert_eq!(game.source.read().unwrap().get_contained_by(), Some(bike_id));
    assert!(game.target.read().unwrap().get_status_bits().test(crate::common::ObjectStatusTypes::Rider1));
    let bike = Arc::clone(&game.target);
    let mut damage = crate::damage::DamageInfo::default();
    damage.input.source_id = crate::common::INVALID_ID;
    damage.input.damage_type = crate::damage::DamageType::KillPilot;
    damage.input.amount = 50.0;
    let dealt = game.operate(118, |world| world.attempt_damage_owned(&bike, &mut damage));
    assert!(dealt.is_ok(), "stationary bike KillPilot admitted: {dealt:?}");
    let rider = game.source.read().unwrap();
    assert_eq!(rider.get_contained_by(), None, "aiEvacuateInstantly");
    assert!(rider.is_effectively_dead(), "rider->kill()");
    drop(rider);
    let bike = game.target.read().unwrap();
    assert!(!bike.is_effectively_dead(), "stationary bike survives");
    assert!(!bike.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned));
    assert!(!bike.get_status_bits().test(crate::common::ObjectStatusTypes::Rider1));
    assert!(!bike.test_weapon_set_flag(WeaponSetType::WeaponRider1));
}
