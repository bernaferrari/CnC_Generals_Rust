//! Queries, mutable borrows, and fixed steps use their receiving world even
//! when another world registers the same ObjectIDs.
use super::pose_owner_tests::isolated_at;
use super::*;
use crate::presentation_frame::PresentationFrame;

fn fixture(health: f32, cash: u32, x: f32) -> (GameLogic, ObjectId, ObjectId) {
    // C++ ThingFactory builds Objects only from INI it parsed into module
    // data (ThingFactory.cpp parseObjectDefinition); admit the source as content.
    let source = r#"
Object WorldReadOwnerProbe
  Draw = W3DModelDraw ModuleTag_Draw
    DefaultConditionState
      Model = WorldReadOwnerProbe
    End
  End
  KindOf = STRUCTURE SELECTABLE MP_COUNT_FOR_VICTORY
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
  ; The fixture states an attack target and move destination, which are the
  ; machine state of the object's AIUpdateInterface (AIUpdate.cpp xfer).
  Behavior = AIUpdateInterface ModuleTag_AI
  End
End
"#;
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "OwnerUSA", true));
    world.add_player(Player::new(1, Team::China, "OwnerChina", false));
    world
        .admit_object_ini_sources([("world_read_owner.ini".to_string(), source.to_string())])
        .expect("fixture Object content admitted before NewGame");
    let source = world
        .create_object("WorldReadOwnerProbe", Team::USA, Vec3::new(x, 0.0, 2.0))
        .unwrap();
    let target = world
        .create_object(
            "WorldReadOwnerProbe",
            Team::China,
            Vec3::new(x + 4.0, 0.0, 6.0),
        )
        .unwrap();
    world.get_player_mut(0).unwrap().resources.supplies = cash;
    let object = world.objects.get_mut(&source).unwrap();
    object.health.current = health;
    object.construction_percent = 0.25;
    object.status.under_construction = true;
    object.movement.target_position = Some(Vec3::new(x + 10.0, 0.0, 12.0));
    object.target = Some(target);
    object.set_attack_substate(crate::game_logic::AttackSubState::ApproachTarget);
    if let Some(building) = object.building_data.as_mut() {
        building.garrisoned_units = vec![target];
    } else {
        object.occupants = vec![target];
    }
    object.weapon = Some(Weapon {
        ammo: Some(2),
        clip_size: 4,
        ..Default::default()
    });
    object.secondary_weapon = Some(Weapon {
        ammo: Some(3),
        clip_size: 5,
        ..Default::default()
    });
    object.tertiary_weapon = Some(Weapon {
        ammo: Some(4),
        clip_size: 6,
        ..Default::default()
    });
    (world, source, target)
}

fn conflicting_world() -> (GameLogic, ObjectId, ObjectId) {
    let (mut world, id, target) = fixture(0.0, 901, 91.0);
    let object = world.host_object_mut(id).unwrap();
    let weapon = object.weapon.as_mut().unwrap();
    weapon.ammo = Some(0);
    weapon.clip_size = 9;
    object.set_attack_substate(crate::game_logic::AttackSubState::AimAtTarget);
    object.target = None;
    object.movement.target_position = None;
    object.construction_percent = 1.0;
    object.status.under_construction = false;
    if let Some(building) = object.building_data.as_mut() {
        building.garrisoned_units.clear();
    } else {
        object.occupants.clear();
    }
    (world, id, target)
}

#[test]
fn all_object_and_player_queries_use_the_receiving_owner() {
    isolated_at(
        module_path!(),
        "all_object_and_player_queries_use_the_receiving_owner",
        || {
            let (world, id, target) = fixture(80.0, 101, 1.0);
            let (mut foreign, foreign_id, foreign_target) = conflicting_world();
            assert_eq!((id, target), (foreign_id, foreign_target));

            let frame = PresentationFrame::try_build_from_logic(&mut foreign, 0).unwrap();
            let actor = frame.objects.iter().find(|row| row.id == id).unwrap();
            assert_eq!(actor.health_current, 0.0);
            assert_eq!(actor.position, Vec3::new(91.0, 0.0, 2.0));
            assert_eq!(frame.local_supplies, 901);
            assert_eq!(actor.attack_target, None);
            assert_eq!(foreign.host_authoritative_move_dest(id), None);
            assert_eq!(
                foreign.host_authoritative_construction(id),
                Some((1.0, false))
            );
            assert_eq!(foreign.host_authoritative_weapon_ammo(id), Some(0));
            assert_eq!(foreign.host_authoritative_occupant_count(id), Some(0));
            assert_eq!(
                foreign.host_authoritative_attack_substate(id),
                Some(crate::game_logic::AttackSubState::AimAtTarget.to_ordinal())
            );

            let object = world.host_object(id).unwrap();
            assert!(object.is_alive());
            assert_eq!(object.get_health_percentage(), 0.8);
            assert!(!object.is_constructed());
            assert_eq!(world.host_authoritative_health(id), Some(80.0));
            assert_eq!(world.host_authoritative_pose(id), Some([1.0, 0.0, 2.0]));
            assert_eq!(world.host_authoritative_cash(0), Some(101));
            assert_eq!(world.host_authoritative_target(id), Some(target));
            assert_eq!(
                world.host_authoritative_move_dest(id),
                Some([11.0, 0.0, 12.0])
            );
            assert_eq!(
                world.host_authoritative_construction(id),
                Some((0.25, true))
            );
            assert_eq!(world.host_authoritative_weapon_ammo(id), Some(2));
            assert_eq!(
                world.host_authoritative_projectile_clip_statuses(id),
                [Some((2, 4)), Some((3, 5)), Some((4, 6))]
            );
            assert_eq!(world.host_authoritative_occupant_count(id), Some(1));
            assert_eq!(world.host_authoritative_contained_units(id), vec![target]);
            assert_eq!(
                world.host_authoritative_attack_substate(id),
                Some(crate::game_logic::AttackSubState::ApproachTarget.to_ordinal())
            );
            let cloned = object.clone();
            assert!(cloned.is_alive());
            assert_eq!(cloned.get_health_percentage(), 0.8);
            let saved = crate::save_load::snapshot::SnapshotBuilder::new()
                .create_world_snapshot(&world)
                .unwrap();
            assert_eq!(saved.objects[&id].health.current, 80.0);
            assert_eq!(
                saved.objects[&id].geometry.position,
                Vec3::new(1.0, 0.0, 2.0)
            );
            assert_eq!(world.host_authoritative_health(ObjectId(9001)), None);
        },
    );
}

#[test]
fn ordinary_mutable_borrows_edit_only_the_receiving_world() {
    isolated_at(
        module_path!(),
        "ordinary_mutable_borrows_edit_only_the_receiving_world",
        || {
            let (mut world, id, target) = fixture(80.0, 101, 1.0);
            let (foreign, foreign_id, _) = conflicting_world();
            assert_eq!(id, foreign_id);

            // C++ Object::setPosition updates the Object's PartitionManager entry.
            let (object, partition) = world.host_object_and_partition_mut(id).unwrap();
            assert_eq!(object.health.current, 80.0);
            assert_eq!(object.get_position(), Vec3::new(1.0, 0.0, 2.0));
            assert_eq!(object.target, Some(target));
            object.health.current = 65.0;
            object.set_position_with_partition_owner(
                Vec3::new(17.0, 0.0, 19.0),
                0,
                &mut Default::default(),
                partition,
            );
            world
                .with_host_object_mut(id, |object| {
                    assert_eq!(object.health.current, 65.0);
                    assert_eq!(object.get_position(), Vec3::new(17.0, 0.0, 19.0));
                    object.health.current = 55.0;
                })
                .unwrap();
            let (object, events) = world.host_object_and_health_events_mut(id).unwrap();
            assert_eq!(object.health.current, 55.0);
            assert!(events.snapshot_damage().is_empty());

            let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
            let actor = frame.objects.iter().find(|row| row.id == id).unwrap();
            assert_eq!(actor.health_current, 55.0);
            assert_eq!(actor.position, Vec3::new(17.0, 0.0, 19.0));
            assert_eq!(foreign.host_authoritative_health(id), Some(0.0));
            assert_eq!(foreign.host_authoritative_pose(id), Some([91.0, 0.0, 2.0]));
            assert_eq!(foreign.host_authoritative_target(id), None);
        },
    );
}

#[test]
fn fixed_step_and_foreign_world_reset_preserve_the_receiving_body() {
    isolated_at(
        module_path!(),
        "fixed_step_and_foreign_world_reset_preserve_the_receiving_body",
        || {
            let (mut world, id, _) = fixture(80.0, 101, 1.0);
            let (mut foreign, foreign_id, _) = conflicting_world();
            assert_eq!(id, foreign_id);
            assert_eq!(foreign.host_authoritative_health(id), Some(0.0));
            let before = world.host_object(id).unwrap().get_position();
            let result = world.update_with_dt(1.0 / 30.0);
            assert_eq!(result.steps_run, 1);
            let object = world.host_object(id).unwrap();
            assert_eq!(object.health.current, 80.0);
            assert!(!object.status.destroyed);
            assert_eq!(object.get_position(), before);

            foreign.reset();
            assert!(foreign.host_object(id).is_none());
            assert_eq!(world.host_authoritative_health(id), Some(80.0));
            assert_eq!(world.get_frame(), 1);
            drop(foreign);
            assert_eq!(world.host_authoritative_health(id), Some(80.0));
        },
    );
}

#[test]
fn victory_phase_updates_the_receiving_players_from_owned_objects() {
    isolated_at(
        module_path!(),
        "victory_phase_updates_the_receiving_players_from_owned_objects",
        || {
            let (mut world, id, _) = fixture(80.0, 101, 1.0);
            let (mut foreign, foreign_id, _) = conflicting_world();
            assert_eq!(id, foreign_id);
            world.players.get_mut(&0).unwrap().is_alive = false;
            foreign.evaluate_victory_condition().unwrap();
            assert!(!foreign.players[&0].is_alive);
            assert_eq!(foreign.host_authoritative_health(id), Some(0.0));

            world.evaluate_victory_condition().unwrap();
            assert!(world.players[&0].is_alive);
            assert_eq!(world.host_authoritative_health(id), Some(80.0));
            assert!(!foreign.players[&0].is_alive);
        },
    );
}

#[test]
fn factory_exit_clearance_does_not_borrow_a_foreign_core_ai_store() {
    isolated_at(
        module_path!(),
        "factory_exit_clearance_does_not_borrow_a_foreign_core_ai_store",
        || {
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            let spawn = |world: &mut GameLogic| {
                let mut template = ThingTemplate::new("ClearanceOwnerInfantry");
                template.add_kind_of(KindOf::Infantry);
                world.templates.insert(template.name.clone(), template);
                let mover = world
                    .create_object(
                        "ClearanceOwnerInfantry",
                        Team::USA,
                        Vec3::new(5.0, 0.0, 5.0),
                    )
                    .unwrap();
                let ally = world
                    .create_object(
                        "ClearanceOwnerInfantry",
                        Team::USA,
                        Vec3::new(15.0, 0.0, 5.0),
                    )
                    .unwrap();
                (mover, ally)
            };
            let (mover, ally) = spawn(&mut a);
            assert_eq!((mover, ally), spawn(&mut b));
            let foreign = gamelogic::system::engine_stores::new_for_world();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                // A genuine outstanding loan to another world's Core AI must not
                // block this world's synchronous factory exit.
                let _unavailable_foreign_ai = foreign.ai().write().unwrap();
                a.move_allies_away_from_destination(mover, Vec3::new(25.0, 0.0, 5.0));
                assert_eq!(a.objects[&ally].move_away_from, Some(mover));
                assert_eq!(b.objects[&ally].move_away_from, None);
                assert_eq!(a.objects[&mover].move_away_from, None);
            });
            b.reset();
            assert_eq!(a.objects[&ally].move_away_from, Some(mover));
        },
    );
}
