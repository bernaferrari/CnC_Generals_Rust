//! Actual authored containment/InstantDeath visitation and ordered cleanup.
use super::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::modules::ai_world::AiWorld;
use crate::object::behavior::instant_death_behavior::InstantDeathBehaviorModuleData;
use crate::object::body::active_body::{ActiveBodyModuleData, BodyDefinitionCatalog};
use std::collections::HashMap;

pub(super) fn lifecycle_world(percent: f32, instant_before_contain: bool) -> NativeMatch {
    lifecycle_world_with_tunnel(percent, instant_before_contain, false)
}
fn lifecycle_world_with_tunnel(
    percent: f32,
    instant_before_contain: bool,
    tunnel: bool,
) -> NativeMatch {
    lifecycle_world_with_tunnel_count(percent, instant_before_contain, tunnel, false, None)
}
pub(super) fn lifecycle_world_with_locomotor(
    ground: crate::locomotor::LocomotorTemplate,
) -> NativeMatch {
    lifecycle_world_with_tunnel_count(0.0, false, false, false, Some(ground))
}
fn lifecycle_world_with_tunnel_count(
    percent: f32,
    instant_before_contain: bool,
    tunnel: bool,
    two_entrances: bool,
    ground_definition: Option<crate::locomotor::LocomotorTemplate>,
) -> NativeMatch {
    let mut game = NativeMatch::new(50.0);
    let mut source_base = DefaultThingTemplate::new("OwnedContainRider".into());
    source_base.add_kind_of(KindOf::Infantry);
    let body = ActiveBodyModuleData {
        max_health: 100.0,
        initial_health: 100.0,
        ..Default::default()
    };
    let body_info = TemplateModuleInfo {
        name: "ActiveBody".into(),
        module_tag: "ContainHealth".into(),
        data: Arc::new(body),
        interface_mask: ModuleInterfaceType::BODY,
    };
    let mut ai = crate::object::update::AIUpdateModuleData::default();
    ai.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["OwnedContainGround".into()],
    );
    let ai_info = TemplateModuleInfo {
        name: "AIUpdateInterface".into(),
        module_tag: "ContainMovement".into(),
        data: Arc::new(ai),
        interface_mask: ModuleInterfaceType::UPDATE,
    };
    let instant = TemplateModuleInfo {
        name: "InstantDeathBehavior".into(),
        module_tag: "ContainInstantDeath".into(),
        data: Arc::new(InstantDeathBehaviorModuleData::default()),
        interface_mask: ModuleInterfaceType::DIE,
    };
    let physics_info = TemplateModuleInfo {
        name: "PhysicsBehavior".into(),
        module_tag: "ActualRiderPhysics".into(),
        data: Arc::new(
            crate::object::behavior::physics_update::PhysicsBehaviorModuleData::default(),
        ),
        interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::COLLIDE,
    };
    let source: Arc<dyn ThingTemplate> = Arc::new(EntryTemplate {
        base: source_base,
        modules: vec![body_info.clone(), ai_info, physics_info, instant.clone()],
    });
    let mut owner_base = DefaultThingTemplate::new("OwnedContainVehicle".into());
    owner_base.add_kind_of(KindOf::Vehicle);
    let contain = TemplateModuleInfo {
        name: "TransportContain".into(),
        module_tag: "LiteralOwnedCargo".into(),
        data: Arc::new(
            crate::contain_module_overrides::ContainModuleDataAdapter::new(
                crate::object::contain::transport_contain::TransportContainModuleData {
                    slot_capacity: 3,
                    base: crate::object::contain::open_contain::OpenContainModuleData {
                        contain_max: 3,
                        allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
                        damage_percentage_to_units: percent,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ),
        ),
        interface_mask: ModuleInterfaceType::CONTAIN,
    };
    let contain = if tunnel {
        TemplateModuleInfo {
            name: "TunnelContain".into(),
            module_tag: "ActualTunnelNetwork".into(),
            data: Arc::new(
                crate::contain_module_overrides::ContainModuleDataAdapter::new(
                    crate::object::contain::tunnel_contain::TunnelContainModuleData::default(),
                ),
            ),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }
    } else {
        contain
    };
    let owner: Arc<dyn ThingTemplate> = Arc::new(EntryTemplate {
        base: owner_base,
        modules: if instant_before_contain {
            vec![body_info, instant, contain]
        } else {
            vec![body_info, contain, instant]
        },
    });
    let mut catalog = game_engine::common::thing::module_factory::ModuleFactory::new();
    crate::contain_module_overrides::register_native_module_overrides(&mut catalog).unwrap();
    for template in [&source, &owner] {
        for entry in template.get_behavior_module_info() {
            catalog.add_module_internal(
                None,
                None,
                game_engine::common::thing::module::ModuleType::Behavior,
                &entry.name,
                entry.interface_flags(),
            );
        }
    }
    let bodies = Arc::new(
        BodyDefinitionCatalog::new(
            Some(&game_engine::common::global_data::GlobalData::default()),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap(),
    );
    let mut ground = crate::locomotor::LocomotorTemplate::new("OwnedContainGround".into());
    ground.surfaces = crate::locomotor::SURFACE_GROUND;
    ground.max_speed = 3.0;
    ground.acceleration = 0.1;
    let ground = Arc::new(ground_definition.unwrap_or(ground));
    let mut lookup = |name: &str| Ok((name == "OwnedContainGround").then(|| Arc::clone(&ground)));
    let team = game.source.read().unwrap().get_team().unwrap();
    let mut factory = ObjectFactory::new();
    let mut scheduler = crate::system::game_logic::GameLogic::new();
    let mut objects = vec![
        (source, ObjectCreationFlags::empty()),
        (Arc::clone(&owner), ObjectCreationFlags::NO_AI),
    ];
    if two_entrances {
        objects.push((owner, ObjectCreationFlags::NO_AI));
    }
    for (template, flags) in objects {
        let id = factory
            .prepare_object_owned(
                &mut scheduler,
                template,
                Coord3D::new(50.0, 50.0, 0.0),
                Some(Arc::clone(&team)),
                flags,
                &mut lookup,
                Arc::clone(&bodies),
                &catalog,
            )
            .unwrap();
        let object = factory.get_object(id).unwrap().get_base_object().unwrap();
        Object::admit_update_schedule_from_handle(&object, &mut scheduler).unwrap();
    }
    game.source = factory.get_object(1).unwrap().get_base_object().unwrap();
    game.target = factory.get_object(2).unwrap().get_base_object().unwrap();
    game.factory = factory;
    game.scheduler = scheduler;
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let ai_proxy = game
        .source
        .read()
        .unwrap()
        .named_update_registrations("AIUpdateInterface")
        .pop()
        .unwrap();
    game.operate_with_current(10, Some(ai_proxy), |world| {
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap();
        ai.lock()
            .unwrap()
            .update_scheduled_tick_with_world(world)
            .unwrap();
        if !tunnel {
            crate::modules::ai_world::complete_enter(world, 1, 2).unwrap();
        } else {
            assert!(world.register_tunnel_owned(2).unwrap());
            if two_entrances {
                assert!(world.register_tunnel_owned(3).unwrap());
            }
        }
    });
    game
}

#[test]
fn authored_contain_before_instant_death_evacuates_live_rider_then_retires_group() {
    let mut game = lifecycle_world(0.0, false);
    let peer = lifecycle_world(0.0, false);
    let owner = Arc::clone(&game.target);
    let mut group = crate::ai::AiGroup::new(8);
    group.add(2);
    owner.write().unwrap().enter_runtime_group(&group).unwrap();
    game.groups.push(Arc::new(RwLock::new(group)));
    let mut damage = DamageInfo::with_simple(
        150.0,
        crate::common::INVALID_ID,
        DamageType::Unresistable,
        DeathType::Normal,
    );
    let rider = Arc::clone(&game.source);
    let mut exit_facts = String::new();
    game.operate(30, |world| {
        let (position, layer) = { let object = owner.read().unwrap(); (*object.get_position(), object.get_layer()) };
        let ai = rider.read().unwrap().get_ai_update_interface().unwrap();
        let mut surfaces = None;
        let mut valid = false;
        ai.lock().unwrap().with_cur_locomotor(&mut |loco| {
            surfaces = Some(loco.get_legal_surfaces());
            valid = world.pathfinder().unwrap().valid_movement_terrain_for_locomotor(
                crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32), loco, &position);
        });
        let cell = crate::ai::pathfind_astar::GridCoord::from_world(&position);
        exit_facts = format!("position {position:?}, layer {layer:?}, surfaces {surfaces:?}, valid {valid}, cell {:?}",
            world.pathfinder().unwrap().get_cell_type_at_cell(crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32), cell.x, cell.y));
        world.attempt_damage_owned(&owner, &mut damage).unwrap();
    });
    assert!(owner.read().unwrap().is_destroyed());
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert_eq!(
        game.source.read().unwrap().get_health(),
        100.0,
        "{exit_facts}"
    );
    assert!(!game.source.read().unwrap().is_destroyed());
    assert!(
        game.factory.get_object(2).is_some(),
        "onDie queues destruction; original lookup remains until physical cleanup"
    );
    assert_eq!(
        game.groups.len(),
        1,
        "group survives until the original destructor phase"
    );
    assert_eq!(game.dispose_destroyed(31).unwrap(), 1);
    assert!(game.groups.is_empty());
    assert!(game.factory.get_object(2).is_none());
    assert_eq!(peer.source.read().unwrap().get_contained_by(), Some(2));
    assert_eq!(peer.target.read().unwrap().get_health(), 100.0);
}

#[test]
fn instant_death_before_contain_deletes_literal_rider_before_later_empty_die_callback() {
    let mut game = lifecycle_world(0.0, true);
    let owner = Arc::clone(&game.target);
    let rider = Arc::clone(&game.source);
    let mut damage = DamageInfo::with_simple(
        150.0,
        crate::common::INVALID_ID,
        DamageType::Unresistable,
        DeathType::Normal,
    );
    game.operate(30, |world| {
        world.attempt_damage_owned(&owner, &mut damage).unwrap();
    });
    assert!(rider.read().unwrap().is_destroyed());
    assert_eq!(
        rider.read().unwrap().get_health(),
        100.0,
        "literal onDelete destroys, it does not synthesize lethal damage"
    );
    assert_eq!(rider.read().unwrap().get_contained_by(), None);
    assert_eq!(game.dispose_destroyed(31).unwrap(), 2);
}

#[test]
fn contained_damage_runs_actual_child_death_before_parent_evacuation() {
    let mut game = lifecycle_world(1.0, false);
    let owner = Arc::clone(&game.target);
    let rider = Arc::clone(&game.source);
    let mut damage = DamageInfo::with_simple(
        150.0,
        crate::common::INVALID_ID,
        DamageType::Unresistable,
        DeathType::Normal,
    );
    game.operate(30, |world| {
        world.attempt_damage_owned(&owner, &mut damage).unwrap();
    });
    assert!(rider.read().unwrap().is_effectively_dead());
    assert!(rider.read().unwrap().is_destroyed());
    assert_eq!(rider.read().unwrap().get_contained_by(), None);
    assert!(
        owner
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_objects()
            .is_empty()
    );
    assert_eq!(game.dispose_destroyed(31).unwrap(), 2);
}

#[test]
fn last_actual_tunnel_caves_in_its_player_roster_without_touching_peer() {
    let mut game = lifecycle_world_with_tunnel(0.0, false, true);
    let peer = lifecycle_world_with_tunnel(0.0, false, true);
    // Restore the tracker-owned saved roster, without replaying an entry callback.
    let player = game.players.get_player(0).unwrap().clone();
    player
        .write()
        .unwrap()
        .get_tunnel_system_mut()
        .unwrap()
        .add_to_contain_list_id(1)
        .unwrap();
    game.source
        .write()
        .unwrap()
        .admit_contained_by_at(2, true, 10);
    let owner = Arc::clone(&game.target);
    game.operate(11, |world| {
        world.destroy_object_owned(2).unwrap();
    });
    assert!(game.source.read().unwrap().is_destroyed());
    assert_eq!(game.source.read().unwrap().get_contained_by(), None);
    assert!(
        player
            .read()
            .unwrap()
            .get_tunnel_system()
            .unwrap()
            .get_contained_item_ids()
            .is_empty()
    );
    assert!(!peer.source.read().unwrap().is_destroyed());
    assert!(!peer.target.read().unwrap().is_destroyed());
    assert_eq!(
        peer.players
            .get_player(0)
            .unwrap()
            .read()
            .unwrap()
            .get_tunnel_system()
            .unwrap()
            .get_container_list()
            .unwrap(),
        vec![2]
    );
    assert!(owner.read().unwrap().is_destroyed());
}

#[test]
fn retired_tunnel_retargets_actual_shared_roster_at_original_frame() {
    let mut game = lifecycle_world_with_tunnel_count(0.0, false, true, true, None);
    let peer = lifecycle_world_with_tunnel_count(0.0, false, true, true, None);
    let player = game.players.get_player(0).unwrap().clone();
    player
        .write()
        .unwrap()
        .get_tunnel_system_mut()
        .unwrap()
        .add_to_contain_list_id(1)
        .unwrap();
    game.source
        .write()
        .unwrap()
        .admit_contained_by_at(2, true, 10);
    game.operate(37, |world| {
        world.destroy_object_owned(2).unwrap();
    });
    let rider = game.source.read().unwrap();
    assert!(!rider.is_destroyed());
    assert_eq!(rider.get_contained_by(), Some(3));
    assert_eq!(rider.get_contained_by_frame(), 37);
    assert_eq!(
        player
            .read()
            .unwrap()
            .get_tunnel_system()
            .unwrap()
            .get_container_list()
            .unwrap(),
        vec![3]
    );
    assert_eq!(
        player
            .read()
            .unwrap()
            .get_tunnel_system()
            .unwrap()
            .get_contained_item_ids(),
        &[1]
    );
    assert_eq!(
        peer.players
            .get_player(0)
            .unwrap()
            .read()
            .unwrap()
            .get_tunnel_system()
            .unwrap()
            .get_container_list()
            .unwrap(),
        vec![2, 3]
    );
}
