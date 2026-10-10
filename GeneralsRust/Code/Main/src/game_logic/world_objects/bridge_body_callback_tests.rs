//! Real owner boundaries; CPP bridge callbacks finish before the next action/FX.
use super::super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;
use gamelogic::common::{BodyDamageType, Coord3D};
use gamelogic::terrain::BridgeInfo;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn bridge(world: &mut GameLogic, position: Vec3) -> (ObjectId, ObjectId, ObjectId, u8) {
    for (name, kind, health) in [
        ("OwnerSpan", KindOf::Bridge, 200.0),
        ("OwnerTower", KindOf::BridgeTower, 100.0),
    ] {
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(kind)
            .add_kind_of(KindOf::Structure)
            .set_health(health);
        world.templates.insert(name.into(), template);
    }
    let span = world
        .create_object("OwnerSpan", Team::USA, position)
        .unwrap();
    let first = world
        .create_object("OwnerTower", Team::USA, position - Vec3::X * 40.0)
        .unwrap();
    let second = world
        .create_object("OwnerTower", Team::USA, position + Vec3::X * 40.0)
        .unwrap();
    let corners = [
        position + Vec3::new(-30.0, 0.0, -10.0),
        position + Vec3::new(-30.0, 0.0, 10.0),
        position + Vec3::new(30.0, 0.0, -10.0),
        position + Vec3::new(30.0, 0.0, 10.0),
    ];
    world
        .bridge_behavior
        .register_span(span, corners[0], corners[1], corners[2], corners[3]);
    world
        .bridge_behavior
        .bind_towers(span, [first, second, ObjectId(0), ObjectId(0)]);
    let coord = |p: Vec3| Coord3D::new(p.x, p.z, p.y);
    let mut info = BridgeInfo::new();
    info.bridge_object_id = span.0;
    info.from_left = coord(corners[0]);
    info.from_right = coord(corners[1]);
    info.to_left = coord(corners[2]);
    info.to_right = coord(corners[3]);
    let layer = world.reserve_owned_bridge(&info);
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .prepend_bridge_on_layer(
            info,
            "OwnerSpan".into(),
            gamelogic::path::PathfindLayerEnum::from_u32(layer as u32),
        );
    (span, first, second, layer)
}

fn hit(world: &mut GameLogic, id: ObjectId, amount: f32, kind: DamageType) {
    world
        .apply_owned_damage(
            id,
            amount,
            None,
            kind,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
}

#[test]
fn tower_mirrors_input_fraction_in_slot_order_before_damage_fx() {
    isolated(
        "tower_mirrors_input_fraction_in_slot_order_before_damage_fx",
        || {
            let mut world = GameLogic::new();
            let (span, first, second, _) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            world
                .apply_owned_damage_with_killer_team_and_after_fx(
                    first,
                    40.0,
                    None,
                    DamageType::Bullet,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                    None,
                    |owner, victim| {
                        assert_eq!(victim, first);
                        let targets: Vec<_> = owner
                            .health_events
                            .snapshot_damage()
                            .iter()
                            .map(|e| e.target)
                            .collect();
                        assert_eq!(
                            targets,
                            [first, second, span],
                            "tower siblings precede span at real DamageFX"
                        );
                        // StructureArmor SMALL_ARMS=50%; mirror uses raw 40/100,
                        // not the victim's post-armor 20/100.
                        assert_eq!(owner.objects[&first].health.current, 80.0);
                        assert_eq!(owner.objects[&second].health.current, 80.0);
                        assert_eq!(owner.objects[&span].health.current, 160.0);
                    },
                )
                .unwrap();
            assert!(crate::game_logic::host_bridge_behavior::drain_mirrors().is_empty());
            assert!(crate::game_logic::host_bridge_behavior::drain_death_links().is_empty());
        },
    );
}

#[test]
fn owned_linked_bridge_callbacks_ignore_another_world_with_identical_ids() {
    isolated(
        "owned_linked_bridge_callbacks_ignore_another_world_with_identical_ids",
        || {
            let mut first = GameLogic::new();
            let a = bridge(&mut first, Vec3::new(50.0, 5.0, 50.0));
            let mut second = GameLogic::new();
            let b = bridge(&mut second, Vec3::new(50.0, 5.0, 50.0));
            assert_eq!(a, b);
            hit(&mut first, a.1, 25.0, DamageType::Unresistable);
            second.sync_host_bridge_rubble_and_scaffolds();
            assert_eq!(first.objects[&a.0].health.current, 150.0);
            assert_eq!(first.objects[&a.2].health.current, 75.0);
            for id in [b.0, b.1, b.2] {
                assert_eq!(
                    second.objects[&id].health.current,
                    second.objects[&id].health.maximum
                );
            }
            second.reset();
            hit(&mut first, a.1, 25.0, DamageType::Unresistable);
            assert_eq!(first.objects[&a.0].health.current, 100.0);
        },
    );
}

#[test]
fn full_bridge_scan_resets_every_record_and_preserves_the_frame_query_gate() {
    isolated(
        "full_bridge_scan_resets_every_record_and_preserves_the_frame_query_gate",
        || {
            let mut world = GameLogic::new();
            let a = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            let b = bridge(&mut world, Vec3::new(150.0, 5.0, 150.0));
            hit(&mut world, a.0, 200.0, DamageType::Unresistable);
            assert!(
                world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_bridge_broken(a.0.0)
            );
            assert!(
                world.objects[&a.1].status.on_die_started
                    && world.objects[&a.2].status.on_die_started
            );
            assert!(crate::game_logic::host_bridge_behavior::drain_death_links().is_empty());
            // Crossing a non-rubble threshold still invokes the full terrain scan.
            hit(&mut world, b.0, 80.0, DamageType::Unresistable);
            let terrain = world.world_services.terrain().read().unwrap();
            assert!(terrain.bridge_damage_states_changed());
            assert!(
                !terrain.is_bridge_broken(a.0.0),
                "unchanged earlier bridge record is reset by this full scan"
            );
            let mut order = Vec::new();
            terrain.for_each_bridge(|record| order.push(record.get_bridge_info().bridge_object_id));
            assert_eq!(
                order,
                [b.0.0, a.0.0],
                "original source admission prepends terrain records"
            );
            drop(terrain);
            world
                .world_services
                .terrain()
                .write()
                .unwrap()
                .begin_host_bridge_frame();
            assert!(
                !world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .bridge_damage_states_changed()
            );
        },
    );
}

#[test]
fn rubble_splats_only_matching_layer_and_height_before_fx_with_self_source() {
    isolated(
        "rubble_splats_only_matching_layer_and_height_before_fx_with_self_source",
        || {
            let mut world = GameLogic::new();
            let (span, _, _, layer) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            let mut template = ThingTemplate::new("OwnerOccupant");
            template.add_kind_of(KindOf::Infantry).set_health(100.0);
            world.templates.insert(template.name.clone(), template);
            let deck = world
                .create_object("OwnerOccupant", Team::USA, Vec3::new(50.0, 5.0, 50.0))
                .unwrap();
            let ground = world
                .create_object("OwnerOccupant", Team::USA, Vec3::new(50.0, 5.0, 50.0))
                .unwrap();
            let high = world
                .create_object("OwnerOccupant", Team::USA, Vec3::new(50.0, 100.0, 50.0))
                .unwrap();
            world.objects.get_mut(&deck).unwrap().pathfind_layer = layer;
            world.objects.get_mut(&high).unwrap().pathfind_layer = layer;
            world
                .apply_owned_damage_with_killer_team_and_after_fx(
                    span,
                    200.0,
                    None,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                    None,
                    |owner, _| {
                        let victim = &owner.objects[&deck];
                        assert_eq!(victim.health.current, 0.0);
                        assert_eq!(victim.status.death_type, HostDeathType::Splatted);
                        assert!(victim.status.on_die_started);
                        let fall = owner
                            .health_events
                            .snapshot_damage()
                            .into_iter()
                            .find(|e| e.target == deck)
                            .unwrap();
                        assert_eq!(fall.source, Some(deck));
                        assert_eq!(
                            fall.damage_type_ordinal,
                            DamageType::Falling.to_store() as u32
                        );
                        assert_eq!(owner.objects[&ground].health.current, 100.0);
                        assert_eq!(owner.objects[&high].health.current, 100.0);
                    },
                )
                .unwrap();
        },
    );
}

#[test]
fn bridge_healing_finishes_mirrors_and_keeps_scaffold_layer_closed() {
    isolated(
        "bridge_healing_finishes_mirrors_and_keeps_scaffold_layer_closed",
        || {
            let mut world = GameLogic::new();
            let (span, first, second, layer) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            hit(&mut world, span, 200.0, DamageType::Unresistable);
            assert!(world.bridge_behavior.create_scaffolding(span));
            hit(&mut world, first, 25.0, DamageType::Healing);
            assert_eq!(world.objects[&span].health.current, 50.0);
            assert_eq!(world.objects[&second].health.current, 25.0);
            assert!(
                world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_bridge_repaired(span.0)
            );
            let cell = world
                .pathfinding_system
                .grid
                .world_to_grid(Vec3::new(50.0, 5.0, 50.0));
            assert_eq!(
                world.pathfinding_system.grid.layer_cell_type(layer, cell),
                Some(gamelogic::ai::pathfind_astar::PathfindCellType::BridgeImpassable)
            );
            let motion = world
                .bridge_behavior
                .span(span)
                .unwrap()
                .scaffold_motion_frames;
            hit(&mut world, first, 25.0, DamageType::Healing);
            assert_eq!(
                world
                    .bridge_behavior
                    .span(span)
                    .unwrap()
                    .scaffold_motion_frames,
                motion,
                "body callbacks never advance scaffold animations"
            );
            assert!(
                !world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_bridge_repaired(span.0),
                "the next full scan clears the previous repaired flag"
            );
        },
    );
}

#[test]
fn repaired_bridge_can_run_its_death_callbacks_again() {
    isolated("repaired_bridge_can_run_its_death_callbacks_again", || {
        let mut world = GameLogic::new();
        let (span, first, second, _) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
        hit(&mut world, span, 200.0, DamageType::Unresistable);
        for id in [span, first, second] {
            assert!(world.objects[&id].status.on_die_started);
        }
        hit(&mut world, first, 25.0, DamageType::Healing);
        for id in [span, first, second] {
            assert!(!world.objects[&id].status.on_die_started);
        }
        world.frame = 50;
        hit(&mut world, first, 100.0, DamageType::Unresistable);
        for id in [span, first, second] {
            assert!(world.objects[&id].status.on_die_started);
        }
        assert_eq!(world.bridge_behavior.span(span).unwrap().death_frame, 50);
    });
}

#[test]
fn owned_water_damage_completes_linked_bridge_callbacks_before_returning() {
    isolated(
        "owned_water_damage_completes_linked_bridge_callbacks_before_returning",
        || {
            let mut world = GameLogic::new();
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 20;
            map.height = 20;
            map.heightmap = vec![0; 400];
            let mut water =
                gamelogic::polygon_trigger::PolygonTrigger::new(7, "Lake".into(), Vec::new());
            water.set_water_area(true);
            for (x, y) in [(0, 0), (80, 0), (80, 80), (0, 80)] {
                water.add_point(gamelogic::common::ICoord3D::new(x, y, 20));
            }
            map.polygon_triggers.push(water);
            world
                .world_services
                .terrain()
                .write()
                .unwrap()
                .load_map_data(map);
            let (span, first, second, _) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            assert_eq!(world.damage_owned_water_candidates(vec![span], 200.0), 1);
            for id in [span, first, second] {
                assert_eq!(world.objects[&id].health.current, 0.0);
                assert!(world.objects[&id].status.on_die_started);
            }
            assert!(
                world
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_bridge_broken(span.0)
            );
            assert!(crate::game_logic::host_bridge_behavior::drain_mirrors().is_empty());
            assert!(crate::game_logic::host_bridge_behavior::drain_death_links().is_empty());
        },
    );
}

#[test]
fn span_and_tower_callbacks_use_their_distinct_cpp_source_kind_guards() {
    isolated(
        "span_and_tower_callbacks_use_their_distinct_cpp_source_kind_guards",
        || {
            let mut world = GameLogic::new();
            let (span, first, second, _) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            let (other_span, _, _, _) = bridge(&mut world, Vec3::new(150.0, 5.0, 150.0));
            world
                .apply_owned_damage(
                    span,
                    20.0,
                    Some(other_span),
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                )
                .unwrap();
            assert_eq!(
                world.objects[&first].health.current, 90.0,
                "a span suppresses tower sources, not another BRIDGE source"
            );
            assert_eq!(world.objects[&second].health.current, 90.0);
            world
                .apply_owned_damage(
                    first,
                    10.0,
                    Some(other_span),
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                )
                .unwrap();
            assert_eq!(
                world.objects[&span].health.current, 180.0,
                "a tower suppresses both BRIDGE and BRIDGE_TOWER sources"
            );
            assert_eq!(world.objects[&second].health.current, 90.0);
        },
    );
}

#[test]
fn bridge_scan_identity_does_not_depend_on_a_unique_pathfinder_layer() {
    isolated(
        "bridge_scan_identity_does_not_depend_on_a_unique_pathfinder_layer",
        || {
            let mut world = GameLogic::new();
            let (first, _, _, _) = bridge(&mut world, Vec3::new(50.0, 5.0, 50.0));
            let (second, _, _, _) = bridge(&mut world, Vec3::new(150.0, 5.0, 150.0));
            world
                .world_services
                .terrain()
                .write()
                .unwrap()
                .for_each_bridge_mut(|record| {
                    record.set_layer(gamelogic::path::PathfindLayerEnum::Ground)
                });
            hit(&mut world, first, 200.0, DamageType::Unresistable);
            let terrain = world.world_services.terrain().read().unwrap();
            let mut states = Vec::new();
            terrain.for_each_bridge(|record| {
                states.push((
                    record.get_bridge_info().bridge_object_id,
                    record.get_bridge_info().cur_damage_state,
                ))
            });
            assert_eq!(
                states,
                [
                    (second.0, BodyDamageType::Pristine),
                    (first.0, BodyDamageType::Rubble)
                ],
                "layer fallback cannot make distinct terrain records share a body"
            );
            assert!(terrain.is_bridge_broken(first.0));
            assert!(!terrain.is_bridge_broken(second.0));
        },
    );
}
