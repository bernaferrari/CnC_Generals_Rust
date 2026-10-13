//! PhysicsUpdate.cpp:603–915: real installed nonimmune leaf falls.
use super::*;
use crate::common::ObjectStatusMaskType;
use crate::damage::{DamageType, DeathType};
use crate::modules::ai_world::AiWorld;
use crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModuleData;
use crate::object::behavior::instant_death_behavior::InstantDeathBehaviorModuleData;
use crate::object::behavior::physics_update::PhysicsBehaviorModuleData;
use crate::object::body::active_body::{ActiveBodyModuleData, BodyDefinitionCatalog};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::collections::HashMap;
use std::io::Cursor;

const SOUND: &str = "OwnedLandingBounce";

fn actor(
    mass: f32,
    factor: f32,
    heals: bool,
    lethal: bool,
    structure: bool,
    resting: bool,
    drone: bool,
) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("OwnedLandingActor".into());
    base.add_kind_of(if structure {
        KindOf::Structure
    } else {
        KindOf::Infantry
    });
    if drone {
        base.add_kind_of(KindOf::Drone);
    }
    let mut modules = vec![
        TemplateModuleInfo {
            name: "ActiveBody".into(),
            module_tag: "LandingBody".into(),
            data: Arc::new(ActiveBodyModuleData {
                max_health: 100.0,
                initial_health: 100.0,
                ..Default::default()
            }),
            interface_mask: ModuleInterfaceType::BODY,
        },
        TemplateModuleInfo {
            name: "PhysicsBehavior".into(),
            module_tag: "LandingPhysics".into(),
            data: Arc::new(PhysicsBehaviorModuleData {
                mass,
                kill_when_resting_on_ground: resting,
                forward_friction: 0.01,
                lateral_friction: 0.01,
                aerodynamic_friction: 0.0,
                min_fall_speed_for_damage: 32.0,
                fall_height_damage_factor: factor,
                ..Default::default()
            }),
            interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::COLLIDE,
        },
    ];
    if heals {
        modules.push(TemplateModuleInfo {
            name: "AutoHealBehavior".into(),
            module_tag: "LandingHeal".into(),
            data: Arc::new(AutoHealBehaviorModuleData {
                initially_active: true,
                start_healing_delay: 11,
                healing_delay: 60,
                ..Default::default()
            }),
            interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::DAMAGE,
        });
    }
    if lethal {
        modules.push(TemplateModuleInfo {
            name: "InstantDeathBehavior".into(),
            module_tag: "LandingDeath".into(),
            data: Arc::new(InstantDeathBehaviorModuleData::default()),
            interface_mask: ModuleInterfaceType::DIE,
        });
    }
    Arc::new(EntryTemplate { base, modules })
}

fn game(mass: f32, factor: f32, heals: bool, lethal: bool, structure: bool) -> NativeMatch {
    game_config(mass, factor, heals, lethal, structure, false, false)
}
fn game_config(
    mass: f32,
    factor: f32,
    heals: bool,
    lethal: bool,
    structure: bool,
    resting: bool,
    drone: bool,
) -> NativeMatch {
    let mut game = NativeMatch::new(150.0);
    let template = actor(mass, factor, heals, lethal, structure, resting, drone);
    let mut factory = ObjectFactory::new();
    let mut catalog = game_engine::common::thing::module_factory::ModuleFactory::new();
    crate::contain_module_overrides::register_native_module_overrides(&mut catalog).unwrap();
    for entry in template.get_behavior_module_info() {
        catalog.add_module_internal(
            None,
            None,
            game_engine::common::thing::module::ModuleType::Behavior,
            &entry.name,
            entry.interface_flags(),
        );
    }
    let definitions = Arc::new(
        BodyDefinitionCatalog::new(
            Some(&game_engine::common::global_data::GlobalData::default()),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap(),
    );
    let mut lookup = |_name: &str| Ok(None);
    factory
        .prepare_saved_object(template, 1, &mut lookup, definitions, &catalog)
        .unwrap();
    game.source = factory.get_object(1).unwrap().get_base_object().unwrap();
    game.factory = factory;
    game.scheduler = crate::system::game_logic::GameLogic::new();
    Object::admit_update_schedule_from_handle(&game.source, &mut game.scheduler).unwrap();
    if heals {
        let owner = Arc::clone(&game.source);
        game.operate(0, |world| world.admit_prepared_auto_heal(&owner).unwrap());
    }
    game.audio.init();
    game.audio
        .register_audio_event_info(game_engine::common::audio::AudioEventInfo {
            audio_name: SOUND.into(),
            volume: 1.0,
            sounds: vec!["landing.wav".into()],
            filename: "landing.wav".into(),
            type_field: game_engine::common::audio::ST_GLOBAL,
            limit: -1,
            loop_count: 1,
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            ..Default::default()
        });
    let physics = game.source.read().unwrap().get_physics().unwrap();
    physics
        .access()
        .unwrap()
        .set_bounce_sound(Some(crate::common::audio::AudioEventRts::new(SOUND)));
    game
}

fn start(game: &mut NativeMatch, z: f32, velocity: Coord3D) {
    let owner = Arc::clone(&game.source);
    game.operate(0, |world| {
        world
            .set_object_position(&mut owner.write().unwrap(), &Coord3D::new(50.0, 50.0, z))
            .unwrap()
    });
    let physics = owner.read().unwrap().get_physics().unwrap();
    physics.access().unwrap().set_velocity(&velocity);
    physics
        .set_freefall_for_test(&owner.read().unwrap())
        .unwrap();
}
fn phase(game: &mut NativeMatch, frame: u32) -> Result<crate::modules::UpdateSleepTime, String> {
    let owner = Arc::clone(&game.source);
    let proxy = owner
        .read()
        .unwrap()
        .named_update_registrations("PhysicsBehavior")
        .pop()
        .unwrap();
    game.operate_with_current(frame, Some(proxy), |world| {
        world.update_physics_phase(&owner)
    })
}
fn bytes(game: &NativeMatch) -> Vec<u8> {
    let module = game
        .source
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    let mut bytes = Vec::new();
    module.with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    bytes
}
fn health(game: &NativeMatch) -> f32 {
    game.source.read().unwrap().get_health()
}

#[test]
fn first_tick_landing_preserves_cpp_speed_threshold_and_source_damage() {
    for (vz, expected) in [(4.0, 100.0), (0.0, 100.0), (-8.0, 84.0)] {
        let mut game = game(2.0, 1.0, false, false, false);
        start(&mut game, 20.0, Coord3D::new(0.0, 0.0, vz));
        phase(&mut game, 1).unwrap();
        assert_eq!(health(&game), expected);
        assert_eq!(game.source.read().unwrap().get_position().z, 0.0);
        assert_eq!(game.audio.pending_play_request_count_for(SOUND), 1);
        assert!(
            !game
                .source
                .read()
                .unwrap()
                .is_disabled_by_type(crate::common::DisabledType::DisabledFreefall)
        );
        if expected < 100.0 {
            let body = game.source.read().unwrap().get_body_module().unwrap();
            let body = body.lock().unwrap();
            let hit = body.get_last_damage_info().unwrap();
            assert_eq!(hit.input.source_id, 1);
            assert_eq!(hit.input.damage_type, DamageType::Falling);
            assert_eq!(hit.input.death_type, DeathType::Splatted);
            assert_eq!(hit.input.shock_wave_amount, 0.0);
            assert_eq!(body.get_last_damage_timestamp(), 1);
        }
        phase(&mut game, 2).unwrap();
        assert_eq!(
            game.audio.pending_play_request_count_for(SOUND),
            1,
            "landing occurs once"
        );
    }
}

#[test]
fn shallow_falls_skip_damage_and_no_collisions_still_allows_falling_damage() {
    for (vx, no_collisions, expected) in
        [(20.0, false, 100.0), (10.0, false, 84.0), (0.0, true, 84.0)]
    {
        let mut game = game(2.0, 1.0, false, false, false);
        start(&mut game, 20.0, Coord3D::new(vx, 0.0, -8.0));
        if no_collisions {
            game.source
                .write()
                .unwrap()
                .set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
        }
        phase(&mut game, 1).unwrap();
        assert_eq!(health(&game), expected);
        assert_eq!(game.audio.pending_play_request_count_for(SOUND), 1);
    }
}

#[test]
fn fall_damage_releases_physics_for_actual_autoheal_wake_and_keeps_peer_independent() {
    let mut first = game(2.0, 1.0, true, false, false);
    let mut peer = game(5.0, 1.0, true, false, false);
    start(&mut first, 20.0, Coord3D::new(0.0, 0.0, -8.0));
    start(&mut peer, 20.0, Coord3D::new(0.0, 0.0, -8.0));
    let peer_before = bytes(&peer);
    let peer_pose = peer.source.read().unwrap().get_transform_matrix();
    phase(&mut first, 18).unwrap();
    assert_eq!(health(&first), 84.0);
    assert_eq!(health(&peer), 100.0);
    assert_eq!(bytes(&peer), peer_before);
    assert_eq!(
        peer.source.read().unwrap().get_transform_matrix(),
        peer_pose
    );
    assert_eq!(peer.audio.pending_play_request_count_for(SOUND), 0);
    let heal = first
        .source
        .read()
        .unwrap()
        .named_update_registrations("AutoHealBehavior")
        .pop()
        .unwrap();
    assert_eq!(first.scheduler.sleepy_entry_for(&heal).unwrap().1, 29);
    phase(&mut peer, 18).unwrap();
    assert_eq!(health(&peer), 60.0);
    assert_eq!(health(&first), 84.0);
}

#[test]
fn unsupported_damage_admission_preserves_physics_pose_health_and_audio() {
    let mut game = game(2.0, 1.0, false, false, true);
    start(&mut game, 20.0, Coord3D::new(0.0, 0.0, -8.0));
    let before = bytes(&game);
    let pose = game.source.read().unwrap().get_transform_matrix();
    let error = phase(&mut game, 1).unwrap_err();
    assert!(error.contains("damage actor family"), "{error}");
    assert_eq!(bytes(&game), before);
    assert_eq!(game.source.read().unwrap().get_transform_matrix(), pose);
    assert_eq!(health(&game), 100.0);
    assert_eq!(game.audio.pending_play_request_count_for(SOUND), 0);
}

#[test]
fn saved_midflight_physics_continues_with_same_future_damage_and_sound() {
    let mut first = game(2.0, 0.01, false, false, false);
    let mut receiver = game(2.0, 0.01, false, false, false);
    start(&mut first, 4000.0, Coord3D::ZERO);
    start(&mut receiver, 4000.0, Coord3D::ZERO);
    phase(&mut first, 1).unwrap();
    let saved = bytes(&first);
    let pose = first.source.read().unwrap().get_transform_matrix();
    let receiving_module = receiver
        .source
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    receiving_module.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(&saved), 1))
            .unwrap()
    });
    assert_eq!(bytes(&receiver), saved);
    let owner = Arc::clone(&receiver.source);
    receiver.operate(1, |world| {
        world
            .set_object_position(&mut owner.write().unwrap(), &pose.w_axis.truncate())
            .unwrap()
    });
    for frame in 2..=24 {
        phase(&mut first, frame).unwrap();
        phase(&mut receiver, frame).unwrap();
        assert_eq!(
            first.source.read().unwrap().get_transform_matrix(),
            receiver.source.read().unwrap().get_transform_matrix()
        );
        assert_eq!(health(&first), health(&receiver));
        assert_eq!(bytes(&first), bytes(&receiver));
        assert_eq!(
            first.audio.pending_play_request_count_for(SOUND),
            receiver.audio.pending_play_request_count_for(SOUND)
        );
    }
    assert!(health(&first) < 100.0 && health(&first) > 0.0);
    assert_eq!(first.audio.pending_play_request_count_for(SOUND), 1);
}

#[test]
fn lethal_self_fall_uses_actual_instant_death_then_deferred_cleanup() {
    let mut game = game(2.0, 1.0, false, true, false);
    start(&mut game, 20.0, Coord3D::new(0.0, 0.0, -100.0));
    let owner = Arc::clone(&game.source);
    phase(&mut game, 1).unwrap();
    assert_eq!(health(&game), 0.0);
    assert!(owner.read().unwrap().is_effectively_dead());
    assert!(owner.read().unwrap().is_destroyed());
    assert!(
        game.factory.get_object(1).is_some(),
        "original cleanup phase is later"
    );
    assert_eq!(game.audio.pending_play_request_count_for(SOUND), 1);
    assert_eq!(game.dispose_destroyed(2).unwrap(), 1);
    assert!(game.factory.get_object(1).is_none());
}

#[test]
fn resting_kill_uses_actual_body_and_preserves_drone_exemption() {
    let peer = game_config(3.0, 1.0, false, true, false, true, false);
    let peer_bytes = bytes(&peer);
    let peer_owner = Arc::clone(&peer.source);
    let _peer_loan = peer_owner.write().unwrap();
    let mut killed = game_config(1.0, 1.0, false, true, false, true, false);
    phase(&mut killed, 17).unwrap();
    assert_eq!(health(&killed), 0.0);
    assert!(killed.source.read().unwrap().is_destroyed());
    let body = killed.source.read().unwrap().get_body_module().unwrap();
    let body = body.lock().unwrap();
    let hit = body.get_last_damage_info().unwrap();
    assert!(hit.input.kill);
    assert_eq!(hit.input.amount, 100.0);
    assert_eq!(hit.input.source_id, crate::common::INVALID_ID);
    assert_eq!(hit.input.damage_type, DamageType::Unresistable);
    assert_eq!(hit.input.death_type, DeathType::Normal);
    assert_eq!(body.get_last_damage_timestamp(), 17);
    drop(body);
    drop(_peer_loan);
    assert_eq!(bytes(&peer), peer_bytes);
    let mut drone = game_config(1.0, 1.0, false, true, false, true, true);
    phase(&mut drone, 17).unwrap();
    assert_eq!(health(&drone), 100.0);
    assert!(!drone.source.read().unwrap().is_effectively_dead());
}

#[test]
fn resting_kill_rejects_unadmitted_callback_before_physics_mutation() {
    let mut game = game_config(1.0, 1.0, false, false, true, true, false);
    // Structure damage requires callback families beyond the native leaf.
    let before = bytes(&game);
    let position = *game.source.read().unwrap().get_position();
    assert!(phase(&mut game, 17).is_err());
    assert_eq!(bytes(&game), before);
    assert_eq!(*game.source.read().unwrap().get_position(), position);
    assert_eq!(health(&game), 100.0);
}

#[test]
fn resting_kill_after_lethal_fall_does_not_repeat_body_callbacks() {
    let mut game = game_config(2.0, 10.0, false, true, false, true, false);
    start(&mut game, 20.0, Coord3D::new(0.0, 0.0, -8.0));
    phase(&mut game, 1).unwrap();
    assert_eq!(health(&game), 0.0);
    phase(&mut game, 2).unwrap();
    let body = game.source.read().unwrap().get_body_module().unwrap();
    let body = body.lock().unwrap();
    assert_eq!(
        body.get_last_damage_info().unwrap().input.death_type,
        DeathType::Splatted
    );
}

#[test]
fn owned_bounce_applies_receiving_stiffness_then_rebounds_with_saved_impulse() {
    let mut first = game(2.0, 0.0, false, false, false);
    let mut peer = game(2.0, 0.0, false, false, false);
    first.physics_rules.ground_stiffness = 0.6;
    peer.physics_rules.ground_stiffness = 2.0; // C++ clamps to MAX_STIFF.
    for game in [&mut first, &mut peer] {
        start(game, 400.0, Coord3D::new(0.0, 0.0, -1000.0));
        game.source
            .read()
            .unwrap()
            .get_physics()
            .unwrap()
            .access()
            .unwrap()
            .set_allow_bouncing(true);
        phase(game, 0).unwrap();
        assert_eq!(game.source.read().unwrap().get_position().z, 0.0);
        assert_eq!(
            game.source
                .read()
                .unwrap()
                .get_physics()
                .unwrap()
                .access()
                .unwrap()
                .get_velocity()
                .z,
            -400.0
        );
        assert_eq!(health(game), 100.0);
    }
    let saved = bytes(&first);
    let mut restored = game(2.0, 0.0, false, false, false);
    restored.physics_rules = first.physics_rules;
    start(&mut restored, 0.0, Coord3D::ZERO);
    let module = restored
        .source
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    module.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved), 1))
            .unwrap()
    });
    phase(&mut peer, 1).unwrap();
    phase(&mut first, 1).unwrap();
    phase(&mut restored, 1).unwrap();
    let expected = -400.0 + (1000.0 - crate::physics::GRAVITY) * 0.6 + crate::physics::GRAVITY;
    assert!((first.source.read().unwrap().get_position().z - expected).abs() < 0.0001);
    assert!(
        peer.source.read().unwrap().get_position().z
            > first.source.read().unwrap().get_position().z
    );
    assert_eq!(
        *first.source.read().unwrap().get_position(),
        *restored.source.read().unwrap().get_position()
    );
    assert_eq!(bytes(&first), bytes(&restored));
}

#[test]
fn bouncing_stunned_upside_down_unit_kills_before_final_pose_and_retains_impulse() {
    let mut game = game(2.0, 0.0, false, true, false);
    start(&mut game, 400.0, Coord3D::new(0.0, 0.0, -1000.0));
    let owner = game.source.clone();
    let matrix = glam::Mat4::from_rotation_translation(
        glam::Quat::from_rotation_x(std::f32::consts::PI),
        *owner.read().unwrap().get_position(),
    );
    owner
        .write()
        .unwrap()
        .set_transform_matrix_with_services(
            &matrix,
            &game.terrain,
            &mut game.collision,
            0,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
    let physics = owner.read().unwrap().get_physics().unwrap();
    physics.access().unwrap().set_allow_bouncing(true);
    physics
        .finish_stunned_at(&mut *owner.write().unwrap())
        .unwrap();
    phase(&mut game, 0).unwrap();
    assert!(owner.read().unwrap().is_effectively_dead());
    assert!(owner.read().unwrap().is_destroyed());
    assert_eq!(owner.read().unwrap().get_position().z, 0.0);
    assert!(
        owner.read().unwrap().get_transform_matrix().z_axis.z > 0.99,
        "second setAngles rights the dead owner before outer pose"
    );
}
