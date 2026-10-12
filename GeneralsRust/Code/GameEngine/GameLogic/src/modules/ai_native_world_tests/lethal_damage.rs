//! Actual installed InstantDeath invokes the driving destruction phase at onDie.
use super::*;
use crate::common::Relationship;
use crate::damage::{DamageInfo, DamageType};
use crate::object::behavior::instant_death_behavior::InstantDeathBehaviorModuleData;
use crate::object::body::active_body::BodyDefinitionCatalog;
use std::collections::HashMap;

pub(super) fn instant_actor() -> Arc<dyn ThingTemplate> {
    let actual = ordinary_damage::damage_actor(false);
    let mut base = DefaultThingTemplate::new("NativeInstantVictim".into());
    base.add_kind_of(KindOf::Infantry);
    base.add_kind_of(KindOf::Score);
    let mut modules = actual.get_behavior_module_info().to_vec();
    modules.push(TemplateModuleInfo {
        name: "InstantDeathBehavior".into(),
        module_tag: "NativeInstantDeath".into(),
        data: Arc::new(InstantDeathBehaviorModuleData::default()),
        interface_mask: ModuleInterfaceType::DIE,
    });
    Arc::new(EntryTemplate { base, modules })
}
pub(super) fn native_death_world() -> NativeMatch {
    native_world_with_death(instant_actor())
}
fn native_world_with_death(victim: Arc<dyn ThingTemplate>) -> NativeMatch {
    native_world_with_templates(ordinary_damage::damage_actor(false), victim)
}
pub(super) fn native_world_with_templates(
    source: Arc<dyn ThingTemplate>,
    victim: Arc<dyn ThingTemplate>,
) -> NativeMatch {
    let mut game = NativeMatch::new(150.0);
    let mut factory = ObjectFactory::new();
    let mut scheduler = crate::system::game_logic::GameLogic::new();
    let mut players = PlayerList::new();
    players.set_local_player_index(0);
    let mut teams = crate::team::TeamFactory::new();
    let mut owner_teams = Vec::new();
    for index in 0..2 {
        let mut definition = game_engine::common::rts::player_template::PlayerTemplate::new(
            format!("NativeDeathPlayer{index}"),
        );
        definition.playable = true;
        definition.starting_money.set_money(1);
        definition
            .production_cost_changes
            .insert("NativeInstantVictim".into(), 0.0);
        let mut player = Player::new(index);
        player.init(Arc::new(crate::player::PlayerTemplate::from_common(
            &definition,
        )));
        player.set_player_relationship_by_index(1 - index, Relationship::Enemies);
        let prototype = teams
            .init_team_owned(
                format!("NativeDeathTeam{index}").into(),
                format!("Player{index}").into(),
                false,
                None,
                Some(index as u32),
            )
            .unwrap();
        let team = teams
            .create_team_on_prototype_with_id(&prototype, (index + 1) as u32)
            .unwrap();
        player.set_default_team(Some(team.clone()));
        players.add_player(Arc::new(RwLock::new(player)));
        owner_teams.push(team);
    }

    let mut catalog = game_engine::common::thing::module_factory::ModuleFactory::new();
    crate::contain_module_overrides::register_native_module_overrides(&mut catalog).unwrap();
    let templates = [source, victim];
    for template in &templates {
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
    let mut ground = crate::locomotor::LocomotorTemplate::new("NativeDeathGround".into());
    ground.surfaces = crate::locomotor::SURFACE_GROUND;
    ground.max_speed = 3.0;
    ground.acceleration = 0.1;
    let ground = Arc::new(ground);
    let mut lookup = |name: &str| {
        Ok(if name == "NativeDeathGround" {
            Some(ground.clone())
        } else {
            None
        })
    };
    for (index, template) in templates.into_iter().enumerate() {
        let id = factory
            .prepare_object_owned(
                &mut scheduler,
                template,
                Coord3D::new(50.0 + index as f32 * 50.0, 50.0, 0.0),
                Some(owner_teams[index].clone()),
                ObjectCreationFlags::empty(),
                &mut lookup,
                bodies.clone(),
                &catalog,
            )
            .unwrap();
        let object = factory.get_object(id).unwrap().get_base_object().unwrap();
        owner_teams[index].write().unwrap().add_member(id);
        scheduler.admit_scheduling_owner(id, object);
    }
    game.source = factory.get_object(1).unwrap().get_base_object().unwrap();
    game.target = factory.get_object(2).unwrap().get_base_object().unwrap();
    game.factory = factory;
    game.scheduler = scheduler;
    game.players = players;
    game.teams = teams;
    game
}
pub(super) fn fatal(source: u32) -> DamageInfo {
    let mut hit = DamageInfo::default();
    hit.input.source_id = source;
    hit.input.damage_type = DamageType::Unresistable;
    hit.input.amount = 150.0;
    hit
}
#[test]
fn instant_death_is_synchronous_owned_and_repeated_hit_does_not_replay() {
    let mut game = native_death_world();
    let peer = native_death_world();
    let target = game.target.clone();
    let peer_guard = peer.target.write().unwrap();
    let mut hit = fatal(1);
    game.operate(123, |world| {
        assert_eq!(
            world.attempt_damage_owned(&target, &mut hit).unwrap(),
            150.0
        )
    });
    {
        let victim = target.read().unwrap();
        assert_eq!(victim.get_health(), 0.0);
        assert!(victim.is_effectively_dead());
        assert!(
            victim.is_destroyed(),
            "actual InstantDeath must destroy before damage returns"
        );
    }
    let output = hit.output.clone();
    game.operate(124, |world| {
        assert!(world.attempt_damage_owned(&target, &mut hit).is_err())
    });
    assert_eq!(hit.output.actual_damage_dealt, output.actual_damage_dealt);
    drop(peer_guard);
    assert_eq!(peer.target.read().unwrap().get_health(), 100.0);
    assert!(!peer.target.read().unwrap().is_destroyed());
    assert_eq!(
        game.players
            .get_player(1)
            .unwrap()
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_units_lost(),
        1
    );
    assert_eq!(
        game.players
            .get_player(0)
            .unwrap()
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_units_destroyed(),
        1
    );
    assert_eq!(
        peer.players
            .get_player(1)
            .unwrap()
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_units_lost(),
        0
    );
}
#[test]
fn instant_self_damage_and_wrong_factory_admission_use_exact_actor() {
    let mut game = native_death_world();
    let peer = native_death_world();
    let target = game.target.clone();
    let foreign = peer.target.clone();
    game.operate(300, |world| {
        assert!(world.attempt_damage_owned(&foreign, &mut fatal(2)).is_err())
    });
    assert_eq!(target.read().unwrap().get_health(), 100.0);
    game.operate(301, |world| {
        world.attempt_damage_owned(&target, &mut fatal(2)).unwrap()
    });
    assert!(target.read().unwrap().is_destroyed());
    assert!(!peer.target.read().unwrap().is_destroyed());
}

#[test]
fn self_lethal_records_loss_without_killer_credit() {
    let mut game = native_death_world();
    let target = game.target.clone();
    game.operate(321, |world| {
        world.attempt_damage_owned(&target, &mut fatal(2)).unwrap()
    });
    assert_eq!(
        game.players
            .get_player(1)
            .unwrap()
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_units_lost(),
        1
    );
    assert_eq!(
        game.players
            .get_player(1)
            .unwrap()
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_units_destroyed(),
        0
    );
    assert_eq!(
        game.players
            .get_player(0)
            .unwrap()
            .read()
            .unwrap()
            .get_score_keeper()
            .get_total_units_destroyed(),
        0
    );
}

pub(super) fn slow_actor() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeSlowVictim".into());
    base.add_kind_of(KindOf::Infantry);
    base.add_kind_of(KindOf::Score);
    let mut modules = ordinary_damage::damage_actor(false)
        .get_behavior_module_info()
        .to_vec();
    let mut data = crate::object::behavior::slow_death_behavior::SlowDeathBehaviorModuleData::new();
    data.sink_delay = 100;
    data.destruction_delay = 20;
    modules.push(TemplateModuleInfo {
        name: "SlowDeathBehavior".into(),
        module_tag: "NativeSlowDeath".into(),
        data: Arc::new(data),
        interface_mask: ModuleInterfaceType::DIE | ModuleInterfaceType::UPDATE,
    });
    Arc::new(EntryTemplate { base, modules })
}
fn native_slow_world() -> NativeMatch {
    let mut game = native_world_with_death(slow_actor());
    let owner = game.target.clone();
    game.operate(17, |world| world.admit_prepared_slow_death(&owner).unwrap());
    game
}
fn slow_proxy(game: &NativeMatch) -> game_engine::common::thing::update_module::UpdateModulePtr {
    game.target
        .read()
        .unwrap()
        .named_update_registrations("SlowDeathBehavior")
        .pop()
        .unwrap()
}
fn slow_snapshot(game: &NativeMatch) -> Vec<u8> {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let entry = game
        .target
        .read()
        .unwrap()
        .module_by_name(&"SlowDeathBehavior".into())
        .unwrap();
    let mut bytes = Vec::new();
    entry.with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    bytes
}
#[test]
fn slow_death_retains_owner_until_actual_scheduled_final_phase() {
    let mut game = native_slow_world();
    let peer = native_slow_world();
    let owner = game.target.clone();
    let foreign = peer.target.write().unwrap();
    game.operate(100, |world| {
        world.attempt_damage_owned(&owner, &mut fatal(1)).unwrap()
    });
    assert!(owner.read().unwrap().is_effectively_dead());
    assert!(!owner.read().unwrap().is_destroyed());
    let proxy = slow_proxy(&game);
    let wake = game.scheduler.sleepy_entry_for(&proxy).unwrap().1;
    assert!((107..=113).contains(&wake));
    assert_eq!(
        game.operate(119, |world| world.tick_owned_updates(119).unwrap()),
        1
    );
    assert!(!owner.read().unwrap().is_destroyed());
    assert_eq!(
        game.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert!(owner.read().unwrap().is_destroyed());
    drop(foreign);
    assert_eq!(peer.target.read().unwrap().get_health(), 100.0);
}

#[test]
fn slow_death_component_xfer_replaces_deadlines_without_replaying_on_die() {
    use game_engine::common::system::xfer_load::XferLoad;
    use std::io::Cursor;
    let mut original = native_slow_world();
    let mut restored = native_slow_world();
    let first = original.target.clone();
    let second = restored.target.clone();
    original.operate(100, |world| {
        world.attempt_damage_owned(&first, &mut fatal(1)).unwrap()
    });
    restored.operate(400, |world| {
        world.attempt_damage_owned(&second, &mut fatal(1)).unwrap()
    });
    let saved = slow_snapshot(&original);
    assert_ne!(
        saved,
        slow_snapshot(&restored),
        "actual different operation frames must differ before load"
    );
    let entry = second
        .read()
        .unwrap()
        .module_by_name(&"SlowDeathBehavior".into())
        .unwrap();
    entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    });
    assert_eq!(saved, slow_snapshot(&restored));
    // The paired worlds already executed the same lethal body/score/onDie input.
    // This is the installed component's future, not a claim of whole-world Xfer.
    Object::admit_update_schedule_from_handle(&second, &mut restored.scheduler).unwrap();
    let first_proxy = slow_proxy(&original);
    let second_proxy = slow_proxy(&restored);
    assert_eq!(
        original.scheduler.sleepy_entry_for(&first_proxy).unwrap().1,
        restored
            .scheduler
            .sleepy_entry_for(&second_proxy)
            .unwrap()
            .1
    );
    assert_eq!(
        original.operate(119, |world| world.tick_owned_updates(119).unwrap()),
        1
    );
    assert_eq!(
        restored.operate(119, |world| world.tick_owned_updates(119).unwrap()),
        1
    );
    assert!(!first.read().unwrap().is_destroyed());
    assert!(!second.read().unwrap().is_destroyed());
    assert_eq!(
        original.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert_eq!(
        restored.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert!(first.read().unwrap().is_destroyed());
    assert!(second.read().unwrap().is_destroyed());
    assert_eq!(slow_snapshot(&original), slow_snapshot(&restored));
}

fn instant_ai_actor() -> Arc<dyn ThingTemplate> {
    let actual = instant_actor();
    let mut base = DefaultThingTemplate::new("NativeInstantAiVictim".into());
    base.add_kind_of(KindOf::Infantry);
    base.add_kind_of(KindOf::Score);
    let mut modules = actual.get_behavior_module_info().to_vec();
    let mut data = crate::object::update::AIUpdateModuleData::default();
    data.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeDeathGround".into()],
    );
    modules.insert(
        1,
        TemplateModuleInfo {
            name: "AIUpdateInterface".into(),
            module_tag: "NativeDeathAI".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        },
    );
    Arc::new(EntryTemplate { base, modules })
}
#[test]
fn installed_ai_death_latch_and_actual_proxy_wake_precede_destroy() {
    let mut game = native_world_with_death(instant_ai_actor());
    let peer = native_world_with_death(instant_ai_actor());
    let owner = game.target.clone();
    Object::admit_update_schedule_from_handle(&owner, &mut game.scheduler).unwrap();
    let (ai, proxy) = {
        let owner = owner.read().unwrap();
        (
            owner.get_ai_update_interface().unwrap(),
            owner
                .named_update_registrations("AIUpdateInterface")
                .pop()
                .unwrap(),
        )
    };
    game.operate(17, |world| {
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap()
    });
    game.operate(511, |world| {
        world.attempt_damage_owned(&owner, &mut fatal(1)).unwrap()
    });
    assert!(ai.lock().unwrap().is_ai_in_dead_state());
    assert!(owner.read().unwrap().is_destroyed());
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy).unwrap().1, 512);
    let peer = peer.target.read().unwrap();
    assert!(
        !peer
            .get_ai_update_interface()
            .unwrap()
            .lock()
            .unwrap()
            .is_ai_in_dead_state()
    );
    assert_eq!(peer.get_health(), 100.0);
}

#[test]
fn local_death_removes_receiving_radar_and_creates_fake_event_at_operation_frame() {
    use super::super::ai_native_world::NativeDeathNotification;
    use game_engine::common::system::radar::{RadarEventType, RadarObject};
    let mut game = native_death_world();
    let mut peer = native_death_world();
    game.players.set_local_player_index(1);
    peer.players.set_local_player_index(1);
    for world in [&mut game, &mut peer] {
        let mut data = RadarObject::new(2);
        data.priority = game_engine::common::system::radar::RadarPriorityType::Unit;
        data.world_pos = game_engine::common::system::radar::Coord3D::new(150.0, 50.0, 0.0);
        world.radar.add_object(data.clone());
        world
            .target
            .write()
            .unwrap()
            .set_radar_data_for_test(Some(Box::new(data)));
    }
    let target = game.target.clone();
    let peer_guard = peer.target.write().unwrap();
    game.operate(731, |world| {
        assert_eq!(
            world.attempt_damage_owned(&target, &mut fatal(1)).unwrap(),
            150.0
        );
    });
    {
        let object = target.read().unwrap();
        assert_eq!(object.get_health(), 0.0);
        assert!(
            object.is_destroyed(),
            "lethal hit must finish installed onDie"
        );
        assert!(
            !game.radar.contains_object(2),
            "radar link remains: {:?}",
            object.owned_radar_data().is_some()
        );
    }
    assert!(target.read().unwrap().owned_radar_data().is_none());
    assert_eq!(
        game.death_notifications,
        vec![NativeDeathNotification::UnitLost { player: 1 }]
    );
    let (_, _, events, _, _) = game.radar.snapshot_persist_state();
    assert!(
        game.radar.get_active_events().is_empty(),
        "CPP Fake event is retained for spacebar, never drawn"
    );
    let event = events
        .iter()
        .find(|event| event.event_type == RadarEventType::Fake)
        .unwrap();
    assert_eq!(event.create_frame, 731);
    assert_eq!(event.world_loc.x, target.read().unwrap().get_position().x);
    assert!(peer.radar.contains_object(2));
    assert!(peer.radar.get_active_events().is_empty());
    assert!(peer.death_notifications.is_empty());
    assert_eq!(peer_guard.get_health(), 100.0);
}

#[test]
fn missing_receiving_radar_link_rejects_death_before_health() {
    use game_engine::common::system::radar::RadarObject;
    let mut game = native_death_world();
    let target = game.target.clone();
    target
        .write()
        .unwrap()
        .set_radar_data_for_test(Some(Box::new(RadarObject::new(2))));
    game.operate(732, |world| {
        assert!(world.attempt_damage_owned(&target, &mut fatal(1)).is_err())
    });
    assert_eq!(target.read().unwrap().get_health(), 100.0);
    assert!(!target.read().unwrap().is_destroyed());
    assert!(game.death_notifications.is_empty());
}

#[test]
fn direct_destroy_keeps_radar_until_original_destructor_phase() {
    use game_engine::common::system::radar::RadarObject;
    let mut game = native_death_world();
    let target = game.target.clone();
    let mut data = RadarObject::new(2);
    data.priority = game_engine::common::system::radar::RadarPriorityType::Unit;
    game.radar.add_object(data.clone());
    target
        .write()
        .unwrap()
        .set_radar_data_for_test(Some(Box::new(data)));
    game.operate(733, |world| world.destroy_object_owned(2).unwrap());
    assert!(game.radar.contains_object(2));
    assert!(target.read().unwrap().owned_radar_data().is_some());
    assert!(game.death_notifications.is_empty());
    assert_eq!(game.dispose_destroyed(734).unwrap(), 1);
    assert!(!game.radar.contains_object(2));
    assert!(target.read().unwrap().owned_radar_data().is_none());
    assert!(game.factory.get_object(2).is_none());
}
