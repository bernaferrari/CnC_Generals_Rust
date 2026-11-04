//! CPP HiveStructureBody.cpp attemptDamage: propagated damage types go to the
//! contain's closest rider (OpenContain::getClosestRider) through the rider's
//! own Body; with no rider, swallowed types are eaten (noEffect).
use super::*;
use crate::damage::{DamageInfo, DamageType};
use crate::modules::ai_world::AiWorld;
use crate::object::body::hive_structure_body::HiveStructureBodyModuleData;

fn services() {
    if game_engine::common::common_services::try_common_services().is_none() {
        game_engine::common::common_services::install_common_services(Arc::new(
            game_engine::common::common_services::CommonServices::new(),
        ));
    }
    if crate::system::engine_stores::try_active_services().is_none() {
        crate::system::engine_stores::install_active(crate::system::engine_stores::new_for_world());
    }
}

fn active_body(tag: &str) -> TemplateModuleInfo {
    let mut body = crate::object::body::active_body::ActiveBodyModuleData::default();
    body.max_health = 100.0;
    body.initial_health = 100.0;
    TemplateModuleInfo {
        name: "ActiveBody".into(),
        module_tag: tag.into(),
        data: Arc::new(body),
        interface_mask: ModuleInterfaceType::BODY,
    }
}

fn ai_module(tag: &str, loco: &str) -> TemplateModuleInfo {
    let mut l = crate::locomotor::LocomotorTemplate::new(loco.into());
    l.surfaces = crate::locomotor::SURFACE_GROUND;
    l.max_speed = 3.0;
    l.acceleration = 0.1;
    crate::locomotor::LOCOMOTOR_STORE.register_template(l);
    let mut data = crate::object::update::AIUpdateModuleData::default();
    data.set_locomotor_set_entries(crate::common::LocomotorSetType::Normal, vec![loco.into()]);
    TemplateModuleInfo {
        name: "AIUpdateInterface".into(),
        module_tag: tag.into(),
        data: Arc::new(data),
        interface_mask: ModuleInterfaceType::UPDATE,
    }
}

fn rider_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeHiveRider".into());
    base.add_kind_of(KindOf::Infantry);
    Arc::new(EntryTemplate { base, modules: vec![ai_module("NativeHiveRiderAI", "NativeHiveRiderGround"), active_body("NativeHiveRiderBody")] })
}

fn hive_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeHive".into());
    base.add_kind_of(KindOf::Structure);
    let contain = crate::object::contain::transport_contain::TransportContainModuleData {
        slot_capacity: 2,
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 2,
            number_of_exit_paths: 0,
            allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut hive = HiveStructureBodyModuleData::default();
    hive.base.base.max_health = 500.0;
    hive.base.base.initial_health = 500.0;
    hive.damage_types_to_propagate_to_slaves.set_damage_type(DamageType::Explosion);
    hive.damage_types_to_swallow.set_damage_type(DamageType::Explosion);
    Arc::new(EntryTemplate {
        base,
        modules: vec![
            TemplateModuleInfo {
                name: "TransportContain".into(),
                module_tag: "NativeHiveCargo".into(),
                data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(contain)),
                interface_mask: ModuleInterfaceType::CONTAIN,
            },
            TemplateModuleInfo {
                name: "HiveStructureBody".into(),
                module_tag: "NativeHiveBody".into(),
                data: Arc::new(hive),
                interface_mask: ModuleInterfaceType::BODY,
            },
        ],
    })
}

fn health(object: &Arc<RwLock<Object>>) -> f32 {
    object.read().unwrap().get_body_module().unwrap().lock().unwrap().get_health()
}

fn explosion(source: crate::common::ObjectID) -> DamageInfo {
    let mut damage = DamageInfo::default();
    damage.input.source_id = source;
    damage.input.damage_type = DamageType::Explosion;
    damage.input.amount = 30.0;
    damage
}

#[test]
fn hive_propagates_damage_to_closest_rider_body_on_owned_path() {
    services();
    let mut game = NativeMatch::with_source_template(20.0, hive_template(), false, rider_template());
    let rider_id = game.source.read().unwrap().get_id();
    let hive_id = game.target.read().unwrap().get_id();
    game.operate(10, |world| world.enter(rider_id, hive_id)).unwrap();
    assert_eq!(game.source.read().unwrap().get_contained_by(), Some(hive_id));
    let hive = Arc::clone(&game.target);
    let mut damage = explosion(rider_id);
    let dealt = game.operate(11, |world| world.attempt_damage_owned(&hive, &mut damage));
    assert_eq!(dealt, Ok(30.0), "rider->attemptDamage");
    assert_eq!(health(&game.source), 70.0, "resident rider Body took the hit");
    assert_eq!(health(&game.target), 500.0, "hive Body untouched");
}

#[test]
fn empty_hive_swallows_configured_damage_on_owned_path() {
    services();
    let mut game = NativeMatch::with_source_template(20.0, hive_template(), false, rider_template());
    let shooter = game.source.read().unwrap().get_id();
    let hive = Arc::clone(&game.target);
    let mut damage = explosion(shooter);
    let dealt = game.operate(11, |world| world.attempt_damage_owned(&hive, &mut damage));
    assert_eq!(dealt, Ok(0.0));
    assert!(damage.output.no_effect, "swallowed: noEffect");
    assert_eq!(damage.output.actual_damage_dealt, 0.0);
    assert_eq!(health(&game.target), 500.0);
    assert_eq!(health(&game.source), 100.0);
}
