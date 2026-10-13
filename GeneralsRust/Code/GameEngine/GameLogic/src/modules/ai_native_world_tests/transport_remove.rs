//! Real installed Core Transport removal, CPP OpenContain607-660 and
//! TransportContain306-415. No global object admission or serialized fixture.
use super::armed_transport_tests::{board_cached_source, install_weapon_sets, weapon_set};
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::weapon::WeaponSetType;

fn removal_template(orient: bool) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeRemoveTransport".into());
    base.add_kind_of(KindOf::Structure);
    let data = crate::object::contain::transport_contain::TransportContainModuleData {
        armed_riders_upgrade_weapon_set: true,
        slot_capacity: 2,
        exit_delay: 4,
        keep_container_velocity_on_exit: true,
        exit_pitch_rate: 1.5,
        go_aggressive_on_exit: true,
        orient_like_container_on_exit: orient,
        // CPP missing Drawable leaves pose unchanged, never a zero fallback.
        exit_bone: "AbsentExitBone".into(),
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 2,
            number_of_exit_paths: 0,
            allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
            ..Default::default()
        },
        ..Default::default()
    };
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "TransportContain".into(),
            module_tag: "NativeRemovalCargo".into(),
            data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
    })
}
fn removal_game() -> NativeMatch {
    let mut game = NativeMatch::with_container(50.0, removal_template(false));
    install_weapon_sets(&mut game, true, 7);
    board_cached_source(&mut game, 137);
    game
}
fn another_armed_rider(game: &mut NativeMatch) -> Arc<RwLock<Object>> {
    let team = game
        .players
        .get_player(0)
        .unwrap()
        .read()
        .unwrap()
        .get_default_team();
    let id = game
        .factory
        .prepare_object(
            &mut game.scheduler,
            infantry_template(),
            Coord3D::new(50.0, 50.0, 0.0),
            team,
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let object = game
        .factory
        .get_object(id)
        .unwrap()
        .get_base_object()
        .unwrap();
    {
        let mut object = object.write().unwrap();
        object.weapon_set.add_weapon_template_set(weapon_set(
            "SecondArmedRider",
            20.0,
            100.0,
            false,
        ));
        object
            .weapon_set
            .update_weapon_set_with_services(
                id,
                &Default::default(),
                Default::default(),
                147,
                &game.bonuses,
            )
            .unwrap();
    }
    {
        let actor = object.read().unwrap();
        let pos = *actor.get_position();
        let geometry = Object::collision_geometry_from_bounds(
            actor.get_geometry_info(),
            actor.get_template().get_template_geometry_type(),
        );
        game.collision
            .register_object(
                id,
                crate::object::collide::Coord3D::new(pos.x, pos.y, pos.z),
                geometry,
                None,
            )
            .unwrap();
        game.spatial.update_object_position(id, pos);
    }
    Object::admit_update_schedule_from_handle(&object, &mut game.scheduler).unwrap();
    let ai = object.read().unwrap().get_ai_update_interface().unwrap();
    let owner = game.target.read().unwrap().get_id();
    game.operate(147, |world| {
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromAI),
                world,
            )
            .unwrap();
        world.enter(id, owner).unwrap();
    });
    object
}
fn saved_module(game: &NativeMatch) -> Vec<u8> {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let contain = game.target.read().unwrap().get_contain().unwrap();
    let mut bytes = Vec::new();
    contain
        .lock()
        .unwrap()
        .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}

#[test]
fn native_remove_last_armed_rider_restores_own_world_and_wakes_real_cached_ai() {
    let mut game = removal_game();
    let peer = removal_game();
    assert_eq!(
        game.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let id = game.source.read().unwrap().get_id();
    let goal = game.target.read().unwrap().get_id();
    let peer_rider = peer.source.write().unwrap();
    let peer_owner = peer.target.write().unwrap();
    game.operate(207, |world| {
        world.remove_contained(id, goal, false).unwrap()
    });
    let rider = game.source.read().unwrap();
    assert_eq!(rider.get_contained_by(), None);
    assert_eq!(rider.get_contained_by_frame(), 0);
    assert!(!rider.is_disabled_by_type(crate::common::DisabledType::Held));
    assert!(!rider.test_status(crate::common::ObjectStatusTypes::Unselectable));
    assert!(!rider.test_status(crate::common::ObjectStatusTypes::Masked));
    assert_eq!(*rider.get_position(), Coord3D::new(50.0, 50.0, 0.0));
    let ai = rider.get_ai_update_interface().unwrap();
    drop(rider);
    assert_eq!(
        ai.lock().unwrap().get_attitude(),
        crate::modules::AIAttitudeType::Aggressive
    );
    assert_eq!(ai.lock().unwrap().get_next_mood_check_time(), 207);
    let owner = game.target.read().unwrap();
    assert_eq!(
        owner
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contain_count(),
        0
    );
    assert!(!owner.test_weapon_set_flag(WeaponSetType::PlayerUpgrade));
    assert_eq!(
        owner
            .get_current_weapon()
            .unwrap()
            .0
            .get_template()
            .get_name(),
        "NativeBaseWeapon"
    );
    assert_eq!(
        owner
            .get_current_weapon()
            .unwrap()
            .0
            .when_last_reload_started,
        207
    );
    drop(owner);
    assert!(
        game.collision
            .partition_manager()
            .get_object_info(id)
            .is_some()
    );
    game.operate(210, |world| assert!(world.contain_exit_busy(goal).unwrap()));
    game.operate(211, |world| {
        assert!(!world.contain_exit_busy(goal).unwrap())
    });
    assert_eq!(peer_rider.get_contained_by(), Some(peer_owner.get_id()));
    assert!(peer_rider.is_disabled_by_type(crate::common::DisabledType::Held));
    assert!(peer_owner.test_weapon_set_flag(WeaponSetType::PlayerUpgrade));
    assert_eq!(
        peer_owner
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contain_count(),
        1
    );
}

#[test]
fn native_remove_preserves_remaining_armed_rider_and_module_xfer_future() {
    let mut original = removal_game();
    let mut restored = removal_game();
    let rider = another_armed_rider(&mut original);
    let loaded_rider = another_armed_rider(&mut restored);
    let first = original.source.read().unwrap().get_id();
    let goal = original.target.read().unwrap().get_id();
    original.operate(207, |world| {
        world.remove_contained(first, goal, false).unwrap()
    });
    restored.operate(407, |world| {
        world.remove_contained(first, goal, false).unwrap()
    });
    assert!(
        original
            .target
            .read()
            .unwrap()
            .test_weapon_set_flag(WeaponSetType::PlayerUpgrade)
    );
    assert_eq!(rider.read().unwrap().get_contained_by(), Some(goal));
    let bytes = saved_module(&original);
    let module = Object::install_fresh_contain_snapshot_for_test(&restored.target, bytes);
    {
        let _foreign = original.target.write().unwrap();
        restored.operate(1000, |world| {
            module
                .lock()
                .unwrap()
                .snapshot_load_post_process_with_world(world)
                .unwrap()
        });
    }
    assert_eq!(
        loaded_rider.read().unwrap().get_contained_by_frame(),
        147,
        "CPP friend-set pointer must retain the saved admission timestamp"
    );
    let last = rider.read().unwrap().get_id();
    assert_eq!(last, loaded_rider.read().unwrap().get_id());
    for game in [&mut original, &mut restored] {
        game.operate(210, |world| assert!(world.contain_exit_busy(goal).unwrap()));
        game.operate(211, |world| {
            assert!(!world.contain_exit_busy(goal).unwrap())
        });
        game.operate(213, |world| {
            world.remove_contained(last, goal, false).unwrap()
        });
        let owner = game.target.read().unwrap();
        assert!(!owner.test_weapon_set_flag(WeaponSetType::PlayerUpgrade));
        assert_eq!(
            owner
                .get_current_weapon()
                .unwrap()
                .0
                .when_last_reload_started,
            213
        );
        assert_eq!(
            owner
                .get_contain()
                .unwrap()
                .lock()
                .unwrap()
                .get_contain_count(),
            0
        );
        drop(owner);
        game.operate(216, |world| assert!(world.contain_exit_busy(goal).unwrap()));
        game.operate(217, |world| {
            assert!(!world.contain_exit_busy(goal).unwrap())
        });
    }
    assert_eq!(rider.read().unwrap().get_contained_by(), None);
    assert_eq!(loaded_rider.read().unwrap().get_contained_by(), None);
    assert_eq!(saved_module(&original), saved_module(&restored));
}

#[derive(Debug)]
struct OrientedTransport(Arc<dyn ThingTemplate>);
impl ThingTemplate for OrientedTransport {
    fn calc_vision_range(&self) -> f32 {
        self.0.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.0.calc_shroud_clearing_range()
    }
    fn get_name(&self) -> &crate::common::AsciiString {
        self.0.get_name()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.0.is_kind_of(kind)
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        self.0.get_behavior_module_info()
    }
    fn get_template_geometry_type(&self) -> Option<game_engine::common::system::GeometryType> {
        self.0.get_template_geometry_type()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        let mut geometry = self.0.get_template_geometry_info();
        geometry.angle = 1.25;
        geometry
    }
}

#[test]
fn native_remove_applies_owner_orientation_and_layer_at_original_phase() {
    let mut game =
        NativeMatch::with_container(50.0, Arc::new(OrientedTransport(removal_template(true))));
    assert_eq!(game.target.read().unwrap().get_orientation(), 1.25);
    install_weapon_sets(&mut game, true, 7);
    board_cached_source(&mut game, 137);
    let source = Arc::clone(&game.source);
    let target = Arc::clone(&game.target);
    let id = source.read().unwrap().get_id();
    let goal = target.read().unwrap().get_id();
    game.operate(207, |world| {
        world.set_object_layer(
            &mut source.write().unwrap(),
            crate::common::PathfindLayerEnum::Wall,
        );
        world.remove_contained(id, goal, false).unwrap();
    });
    let rider = source.read().unwrap();
    assert_eq!(rider.get_orientation(), 1.25);
    assert_eq!(rider.get_layer(), target.read().unwrap().get_layer());
    assert_eq!(rider.get_contained_by(), None);
    assert!(!rider.is_disabled_by_type(crate::common::DisabledType::Held));
}

fn with_physics(
    template: Arc<dyn ThingTemplate>,
    mass: f32,
    center: f32,
) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new(template.get_name().to_string());
    base.add_kind_of(if template.is_kind_of(KindOf::Structure) {
        KindOf::Structure
    } else {
        KindOf::Infantry
    });
    let mut modules = template
        .get_behavior_module_info()
        .iter()
        .map(|info| TemplateModuleInfo {
            name: info.name.clone(),
            module_tag: info.module_tag.clone(),
            data: Arc::clone(&info.data),
            interface_mask: info.interface_flags(),
        })
        .collect::<Vec<_>>();
    modules.push(TemplateModuleInfo {
        name: "PhysicsBehavior".into(),
        module_tag: "OwnedRemovalPhysics".into(),
        data: Arc::new(
            crate::object::behavior::physics_update::PhysicsBehaviorModuleData {
                mass,
                center_of_mass_offset: center,
                ..Default::default()
            },
        ),
        interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::COLLIDE,
    });
    Arc::new(EntryTemplate { base, modules })
}

#[test]
fn native_remove_inherits_actual_physics_force_and_fall_with_owned_schedule() {
    let mut game = NativeMatch::with_source_at(
        Coord3D::new(50.0, 50.0, 80.0),
        with_physics(removal_template(true), 20.0, 0.0),
        false,
        with_physics(infantry_template(), 4.0, 2.0),
    );
    let mut peer = NativeMatch::with_source_at(
        Coord3D::new(50.0, 50.0, 80.0),
        with_physics(removal_template(true), 20.0, 0.0),
        false,
        with_physics(infantry_template(), 4.0, 2.0),
    );
    for game in [&mut game, &mut peer] {
        let source = Arc::clone(&game.source);
        game.operate(130, |world| {
            world
                .set_object_position(
                    &mut source.write().unwrap(),
                    &Coord3D::new(50.0, 50.0, 80.0),
                )
                .unwrap()
        });
        board_cached_source(game, 137);
    }
    let source = Arc::clone(&game.source);
    let target = Arc::clone(&game.target);
    let id = source.read().unwrap().get_id();
    let goal = target.read().unwrap().get_id();
    let rider_physics = source.read().unwrap().get_physics().unwrap();
    let parent_physics = target.read().unwrap().get_physics().unwrap();
    parent_physics
        .access()
        .unwrap()
        .set_velocity(&glam::Vec3::new(3.0, 5.0, 7.0));
    let _foreign_owner = peer.target.write().unwrap();
    let _foreign_rider = peer.source.write().unwrap();
    game.operate(207, |world| {
        world.remove_contained(id, goal, false).unwrap();
    });
    let physics = rider_physics.access().unwrap();
    assert!(physics.is_motive_at(207).unwrap());
    assert!(physics.get_allow_to_fall());
    drop(physics);
    assert!(game.scheduler.get_number_sleepy_updates() > 0);
    assert!(Arc::ptr_eq(
        &game.scheduler.get_object(id).unwrap(),
        &source
    ));
    let registrations = source
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior");
    assert_eq!(registrations.len(), 1);
    assert_eq!(
        game.scheduler.sleepy_entry_for(&registrations[0]),
        Some((id, 208))
    );
    assert_eq!(source.read().unwrap().get_contained_by(), None);
    assert_eq!(source.read().unwrap().get_position().z, 80.0);
}

#[test]
fn native_dead_transport_scatter_enters_real_cached_move_before_idle_wake() {
    let mut game = removal_game();
    let mut peer = removal_game();
    let source = Arc::clone(&game.source);
    let target = Arc::clone(&game.target);
    let id = source.read().unwrap().get_id();
    let goal = target.read().unwrap().get_id();
    let ai = source.read().unwrap().get_ai_update_interface().unwrap();
    let old_mood_deadline = ai.lock().unwrap().get_next_mood_check_time();
    target.write().unwrap().set_effectively_dead(true);
    let _foreign_owner = peer.target.write().unwrap();
    let _foreign_rider = peer.source.write().unwrap();
    game.operate(207, |world| {
        world.remove_contained(id, goal, false).unwrap()
    });
    let actor = source.read().unwrap();
    assert_eq!(actor.get_contained_by(), None);
    assert!(!actor.is_disabled_by_type(crate::common::DisabledType::Held));
    assert_eq!(*actor.get_position(), Coord3D::new(50.0, 50.0, 0.0));
    drop(actor);
    let ai = ai.lock().unwrap();
    assert_eq!(
        ai.get_current_state_id(),
        Some(crate::ai::states::AIStateType::MoveTo as u32)
    );
    assert_eq!(
        ai.get_next_mood_check_time(),
        old_mood_deadline,
        "CPP wakeUpAndAttemptToTarget does nothing after scatter entered movement"
    );
}
