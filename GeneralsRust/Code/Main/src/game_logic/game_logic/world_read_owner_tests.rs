//! Ordinary production queries borrow their actual world, even while another
//! same-ID comparison session is published on the calling thread.
use super::pose_owner_tests::isolated_at;
use super::*;
use crate::gameworld_shadow::{CoupledTickGuard, GameWorldShadow, with_coupled_shadow};

fn fixture(health: f32, cash: u32, x: f32) -> (GameLogic, ObjectId, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(
                r#"
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
End
"#,
                "world_read_owner.ini"
            )
            .unwrap(),
        1
    );
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "OwnerUSA", true));
    world.add_player(Player::new(1, Team::China, "OwnerChina", false));
    assert_eq!(
        world.seed_asset_definition_templates_from_snapshot(
            parser
                .get_all_definitions()
                .iter()
                .map(|(n, d)| (n.clone(), d.clone()))
        ),
        1
    );
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
    object.attack_substate = crate::game_logic::AttackSubState::ApproachTarget;
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

fn conflicting_shadow() -> (GameLogic, ObjectId, ObjectId, GameWorldShadow) {
    let (foreign, id, target) = fixture(0.0, 901, 91.0);
    let mut shadow = GameWorldShadow::new(16);
    shadow.sync_from_host(&foreign);
    let eid = shadow.entity_for_host(id).unwrap();
    let entity = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
    entity.weapon_ammo = 0;
    entity.weapon_clip_size = 9;
    entity.occupant_count = 7;
    entity.attack_substate_ordinal = 0;
    entity.attack_target = None;
    entity.move_target = None;
    entity.construction_percent = 1.0;
    entity.under_construction = false;
    (foreign, id, target, shadow)
}

#[test]
fn all_object_and_player_queries_use_the_receiving_owner() {
    isolated_at(
        module_path!(),
        "all_object_and_player_queries_use_the_receiving_owner",
        || {
            let (world, id, target) = fixture(80.0, 101, 1.0);
            let (_foreign, foreign_id, foreign_target, mut shadow) = conflicting_shadow();
            assert_eq!((id, target), (foreign_id, foreign_target));
            let _couple = CoupledTickGuard::enter();
            with_coupled_shadow(&mut shadow, || {
                assert_eq!(
                    crate::gameworld_shadow::coupled_entity_health(id),
                    Some(0.0),
                    "positive control observes the foreign entity"
                );
                assert_eq!(crate::gameworld_shadow::coupled_player_cash(0), Some(901));
                let object = world.host_object(id).unwrap();
                assert!(
                    object.is_alive(),
                    "ordinary liveness must use the receiving body"
                );
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
                assert_eq!(
                    world.host_authoritative_health(ObjectId(9001)),
                    None,
                    "a foreign mapping cannot admit a missing host object"
                );
            });
        },
    );
}

#[test]
fn ordinary_mutable_borrows_never_overlay_or_publish_a_foreign_world() {
    isolated_at(
        module_path!(),
        "ordinary_mutable_borrows_never_overlay_or_publish_a_foreign_world",
        || {
            let (mut world, id, target) = fixture(80.0, 101, 1.0);
            let (foreign, foreign_id, _, mut shadow) = conflicting_shadow();
            assert_eq!(id, foreign_id);
            let eid = shadow.entity_for_host(id).unwrap();
            let _couple = CoupledTickGuard::enter();
            with_coupled_shadow(&mut shadow, || {
                let object = world.host_object_mut(id).unwrap();
                assert_eq!(
                    object.health.current, 80.0,
                    "borrow must not copy foreign HP"
                );
                assert_eq!(object.get_position(), Vec3::new(1.0, 0.0, 2.0));
                assert_eq!(object.target, Some(target));
                object.health.current = 65.0;
                object.set_position(Vec3::new(17.0, 0.0, 19.0));
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
                assert_eq!(world.host_authoritative_health(id), Some(55.0));
                assert_eq!(
                    crate::gameworld_shadow::coupled_entity_health(id),
                    Some(0.0)
                );
            });
            assert_eq!(shadow.world().entity(eid).unwrap().health, 0.0);
            assert_eq!(foreign.objects[&id].health.current, 0.0);
            assert_eq!(world.objects[&id].health.current, 55.0);
        },
    );
}

#[test]
fn a_completed_logic_step_cannot_ingress_same_id_foreign_shadow_values() {
    isolated_at(
        module_path!(),
        "a_completed_logic_step_cannot_ingress_same_id_foreign_shadow_values",
        || {
            let (mut world, id, _) = fixture(80.0, 101, 1.0);
            let (mut foreign, foreign_id, _, mut shadow) = conflicting_shadow();
            assert_eq!(id, foreign_id);
            let before = world.objects[&id].get_position();
            let _couple = CoupledTickGuard::enter();
            with_coupled_shadow(&mut shadow, || {
                let result = world.update_with_dt(1.0 / 30.0);
                assert_eq!(result.steps_run, 1, "actual Main fixed step ran");
                assert_eq!(
                    world.objects[&id].health.current, 80.0,
                    "begin-step must not replace the canonical body"
                );
                assert!(!world.objects[&id].status.destroyed);
                assert_eq!(world.objects[&id].get_position(), before);
                assert_eq!(world.host_authoritative_health(id), Some(80.0));
            });
            foreign.reset();
            assert_eq!(world.objects[&id].health.current, 80.0);
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
            let (_foreign, foreign_id, _, mut shadow) = conflicting_shadow();
            assert_eq!(id, foreign_id);
            world.players.get_mut(&0).unwrap().is_alive = false;
            let _couple = CoupledTickGuard::enter();
            with_coupled_shadow(&mut shadow, || {
                assert_eq!(
                    crate::gameworld_shadow::coupled_entity_health(id),
                    Some(0.0)
                );
                world.evaluate_victory_condition();
                assert!(
                    world.players[&0].is_alive,
                    "victory derives life from this world's actual objects"
                );
                assert_eq!(world.objects[&id].health.current, 80.0);
            });
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
