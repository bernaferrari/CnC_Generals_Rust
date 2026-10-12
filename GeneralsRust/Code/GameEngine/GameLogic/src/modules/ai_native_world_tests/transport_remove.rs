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

#[test]
fn native_remove_rejects_unadmitted_orientation_before_unlink_or_status_change() {
    let mut game = NativeMatch::with_container(50.0, removal_template(true));
    install_weapon_sets(&mut game, true, 7);
    board_cached_source(&mut game, 137);
    let before = saved_module(&game);
    let id = game.source.read().unwrap().get_id();
    let goal = game.target.read().unwrap().get_id();
    game.operate(207, |world| {
        assert!(world.remove_contained(id, goal, false).is_err())
    });
    assert_eq!(saved_module(&game), before);
    let rider = game.source.read().unwrap();
    assert_eq!(rider.get_contained_by(), Some(goal));
    assert!(rider.is_disabled_by_type(crate::common::DisabledType::Held));
    assert!(rider.test_status(crate::common::ObjectStatusTypes::Unselectable));
    assert!(
        game.collision
            .partition_manager()
            .get_object_info(id)
            .is_none()
    );
}
