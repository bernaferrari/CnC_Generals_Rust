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
    if kind == DamageType::Healing {
        // Original bridge repair uses Object::attemptHealing, not weapon damage.
        world.apply_owned_healing(id, amount, None).unwrap();
        return;
    }
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
        for id in [span, second] {
            world.objects.get_mut(&id).unwrap().set_indestructible(true);
        }
        hit(&mut world, first, 25.0, DamageType::Healing);
        for id in [span, first, second] {
            assert!(!world.objects[&id].status.on_die_started);
            assert!(world.objects[&id].health.current > 0.0);
            assert!(!world.objects[&id].status.effectively_dead);
        }
        assert_eq!(world.objects[&span].last_damage_source, Some(first));
        assert_eq!(world.objects[&second].last_damage_source, Some(first));
        for id in [span, second] {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .set_indestructible(false);
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

fn payload_radius_bridge_case(amount: f32) {
    let position = Vec3::new(50.0, 5.0, 50.0);
    let mut world = GameLogic::new();
    let own = bridge(&mut world, position);
    let mut foreign = GameLogic::new();
    let other = bridge(&mut foreign, position);
    assert_eq!(own, other);
    world.frame = 10;
    let result = world.apply_fuel_air_radius_damage(
        ObjectId(u32::MAX),
        None,
        Team::USA,
        position,
        amount,
        1.0,
        DamageType::Unresistable,
    );
    // Only the span was directly admitted; repairable bridge husks are not
    // physically destroyed and linked recipients do not inflate strike stats.
    assert_eq!(result, (amount, 1, 0));
    assert_eq!(world.objects[&own.0].health.current, 200.0 - amount);
    for id in [own.1, own.2] {
        assert_eq!(
            world.objects[&id].health.current,
            100.0 - amount * 0.5,
            "linked towers outside the radius finish before the payload returns"
        );
    }
    if amount == 200.0 {
        for id in [own.0, own.1, own.2] {
            assert!(world.objects[&id].status.on_die_started);
            assert!(world.objects[&id].status.keep_as_rubble);
        }
        let terrain = world.world_services.terrain().read().unwrap();
        assert!(terrain.bridge_damage_states_changed());
        assert!(terrain.is_bridge_broken(own.0.0));
    }

    foreign.sync_host_bridge_rubble_and_scaffolds();
    for id in [other.0, other.1, other.2] {
        assert_eq!(
            foreign.objects[&id].health.current,
            foreign.objects[&id].health.maximum
        );
    }
    foreign.reset();
    assert_eq!(world.objects[&own.0].health.current, 200.0 - amount);
}

#[test]
fn payload_radius_finishes_linked_damage_before_returning() {
    isolated(
        "payload_radius_finishes_linked_damage_before_returning",
        || payload_radius_bridge_case(20.0),
    );
}

#[test]
fn payload_radius_finishes_linked_deaths_without_counting_repairable_husks() {
    isolated(
        "payload_radius_finishes_linked_deaths_without_counting_repairable_husks",
        || payload_radius_bridge_case(200.0),
    );
}

#[test]
fn fuel_air_final_phase_finishes_linked_bridge_callbacks_before_returning() {
    isolated(
        "fuel_air_final_phase_finishes_linked_bridge_callbacks_before_returning",
        || {
            use crate::game_logic::host_fuel_air_gas_slow_death::{
                FuelAirGasPhase, HostFuelAirGasSlowDeathData,
            };
            let position = Vec3::new(50.0, 5.0, 50.0);
            let mut world = GameLogic::new();
            let (span, first, second, _) = bridge(&mut world, position);
            // Final SupW radius is70. Both towers are callback recipients only.
            world
                .objects
                .get_mut(&first)
                .unwrap()
                .set_position(position - Vec3::X * 200.0);
            world
                .objects
                .get_mut(&second)
                .unwrap()
                .set_position(position + Vec3::X * 200.0);
            let mut template = ThingTemplate::new("SupW_AuroraFuelAirGas");
            template.set_health(1.0);
            world.templates.insert(template.name.clone(), template);
            let gas = world
                .create_object("SupW_AuroraFuelAirGas", Team::USA, position)
                .unwrap();
            let mut data = HostFuelAirGasSlowDeathData::start(0, true);
            data.phase = FuelAirGasPhase::MidpointDone;
            data.initial_fx_queued = true;
            world.objects.get_mut(&gas).unwrap().fuel_air_gas_slow_death = Some(data);
            world.frame = 30;
            let admitted_die = world.objects[&gas].status.on_die_started;
            world.update_fuel_air_gas_slow_death();
            assert_eq!(
                world.objects[&gas].status.on_die_started, admitted_die,
                "FINAL destruction must not admit Object::onDie again"
            );
            assert_eq!(world.fuel_air_gas_reg.final_detonations, 1);
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
            assert!(world.objects[&gas].status.destroyed);
            assert_eq!(
                world
                    .objects_to_destroy
                    .iter()
                    .filter(|event| event.id == gas)
                    .count(),
                1
            );
        },
    );
}

// Append to existing world_objects/bridge_body_callback_tests.rs.
// Reuses actual bridge()/hit()/isolated() owner fixtures; no existing-test edits.
#[test]
fn detached_tower_damage_cannot_be_consumed_by_same_id_peer_bridge() {
    isolated(
        "detached_tower_damage_cannot_be_consumed_by_same_id_peer_bridge",
        || {
            let mut owner = GameLogic::new();
            let a = bridge(&mut owner, Vec3::new(50.0, 5.0, 50.0));
            let mut peer = GameLogic::new();
            let b = bridge(&mut peer, Vec3::new(50.0, 5.0, 50.0));
            assert_eq!(a, b);
            let mut detached = owner.objects[&a.1].clone();
            let mut local_health = crate::game_logic::HostHealthEvents::default();
            detached.take_damage_from_immediate_typed(
                25.0,
                None,
                DamageType::Unresistable,
                &mut local_health,
            );
            assert_eq!(detached.health.current, 75.0);
            assert!(!local_health.snapshot_damage().is_empty());
            // This real production residual used to drain every thread's detached
            // bridge observation and incorrectly apply it to this same-ID peer.
            peer.sync_host_bridge_rubble_and_scaffolds();
            for (world, ids) in [(&owner, a), (&peer, b)] {
                assert_eq!(world.objects[&ids.0].health.current, 200.0);
                assert_eq!(world.objects[&ids.1].health.current, 100.0);
                assert_eq!(world.objects[&ids.2].health.current, 100.0);
            }
            peer.reset();
            owner.sync_host_bridge_rubble_and_scaffolds();
            assert_eq!(owner.objects[&a.0].health.current, 200.0);
            assert_eq!(owner.objects[&a.2].health.current, 100.0);
        },
    );
}
#[test]
fn detached_span_death_cannot_break_peer_or_restored_receiving_bridge() {
    isolated(
        "detached_span_death_cannot_break_peer_or_restored_receiving_bridge",
        || {
            let mut owner = GameLogic::new();
            let a = bridge(&mut owner, Vec3::new(50.0, 5.0, 50.0));
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&owner).unwrap();
            let bytes = bincode_legacy::serialize(&saved).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let mut receiving = GameLogic::new();
            receiving.templates = owner.templates.clone();
            builder
                .restore_from_snapshot(&decoded, &mut receiving)
                .unwrap();
            let mut detached = owner.objects[&a.0].clone();
            let mut local_health = crate::game_logic::HostHealthEvents::default();
            assert!(!detached.take_damage_from_immediate_typed(
                1000.0,
                None,
                DamageType::Unresistable,
                &mut local_health
            ));
            receiving.sync_host_bridge_rubble_and_scaffolds();
            assert_eq!(receiving.objects[&a.0].health.current, 200.0);
            for tower in [a.1, a.2] {
                assert_eq!(receiving.objects[&tower].health.current, 100.0);
                assert!(!receiving.objects[&tower].status.on_die_started);
            }
            assert!(
                !receiving
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .is_bridge_broken(a.0.0)
            );
            owner.sync_host_bridge_rubble_and_scaffolds();
            assert_eq!(owner.objects[&a.0].health.current, 200.0);
        },
    );
}
#[test]
fn receiving_bridge_damage_fx_observes_slot_order_and_restore_healing_is_immediate() {
    isolated(
        "receiving_bridge_damage_fx_observes_slot_order_and_restore_healing_is_immediate",
        || {
            let mut owner = GameLogic::new();
            let (span, first, second, _) = bridge(&mut owner, Vec3::new(50.0, 5.0, 50.0));
            let mut observed = false;
            owner
                .apply_owned_damage_with_killer_team_and_after_fx(
                    first,
                    40.0,
                    None,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                    None,
                    |world, victim| {
                        observed = true;
                        assert_eq!(victim, first);
                        let targets = world
                            .health_events
                            .snapshot_damage()
                            .iter()
                            .map(|event| event.target)
                            .collect::<Vec<_>>();
                        assert_eq!(targets, [first, second, span]);
                        assert_eq!(world.objects[&first].health.current, 60.0);
                        assert_eq!(world.objects[&second].health.current, 60.0);
                        assert_eq!(world.objects[&span].health.current, 120.0);
                    },
                )
                .unwrap();
            assert!(observed);
            owner.apply_owned_healing(first, 10.0, None).unwrap();
            assert_eq!(owner.objects[&second].health.current, 70.0);
            assert_eq!(owner.objects[&span].health.current, 140.0);
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&owner).unwrap();
            let bytes = bincode_legacy::serialize(&saved).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let mut receiving = GameLogic::new();
            receiving.templates = owner.templates.clone();
            builder
                .restore_from_snapshot(&decoded, &mut receiving)
                .unwrap();
            assert_eq!(
                receiving.bridge_behavior.span_id_for(first),
                Some(span),
                "restore retains actual linked bridge topology"
            );
            receiving.apply_owned_healing(first, 10.0, None).unwrap();
            // No sync/drain/tick may be required to expose original immediate repair.
            assert_eq!(receiving.objects[&first].health.current, 80.0);
            assert_eq!(receiving.objects[&second].health.current, 80.0);
            assert_eq!(receiving.objects[&span].health.current, 160.0);
            assert_eq!(owner.objects[&span].health.current, 140.0);
        },
    );
}

// Append to bridge_body_callback_tests.rs. Reuses bridge(), hit(), isolated().
// Callback-phase fixtures exercise the same synchronous visitor as production;
// authored module-order admission remains a separate acceptance obligation.
fn bridge_callback(
    victim: ObjectId,
    kind: DamageType,
) -> crate::game_logic::object::BridgeBodyCallbacks {
    crate::game_logic::object::BridgeBodyCallbacks {
        victim,
        source: None,
        input_amount: 40.0,
        old_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
        health_changed: true,
        damage_type: kind,
        death_type: HostDeathType::Normal,
    }
}
fn live_mirror_delivery_case(kind: DamageType) {
    let mut owner = GameLogic::new();
    let (span, origin, first, _) = bridge(&mut owner, Vec3::new(50.0, 5.0, 50.0));
    let (other_span, _, _, _) = bridge(&mut owner, Vec3::new(150.0, 5.0, 150.0));
    let old = owner
        .create_object("OwnerTower", Team::USA, Vec3::ZERO)
        .unwrap();
    let replacement = owner
        .create_object("OwnerTower", Team::USA, Vec3::ZERO)
        .unwrap();
    owner
        .bridge_behavior
        .bind_towers(span, [origin, first, old, ObjectId(0)]);
    // This is the actual module callback entry, after an earlier bounded body
    // callback changed maximum. Fraction must use 200 now, not admission's100.
    let callbacks = bridge_callback(origin, kind);
    owner.objects.get_mut(&origin).unwrap().health.maximum = 200.0;
    for id in [first, old, replacement, span] {
        let body = owner.objects.get_mut(&id).unwrap();
        body.health.current = if kind == DamageType::Healing {
            body.health.maximum * 0.5
        } else {
            body.health.maximum
        };
    }
    let mut delivered = Vec::new();
    owner.visit_owned_bridge_mirror_recipients(callbacks, |world, target, amount| {
        delivered.push((target, amount));
        if kind == DamageType::Healing {
            world
                .apply_owned_healing(target, amount, Some(origin))
                .unwrap();
        } else {
            world
                .apply_owned_damage(
                    target,
                    amount,
                    Some(origin),
                    kind,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                )
                .unwrap();
        }
        if target == first {
            world.objects.get_mut(&origin).unwrap().health.maximum = 400.0;
            // Change a later real routing slot and its current maximum only
            // after the preceding real damage/heal delivery has completed.
            world.objects.get_mut(&replacement).unwrap().health.maximum = 150.0;
            world.objects.get_mut(&replacement).unwrap().health.current =
                if kind == DamageType::Healing {
                    75.0
                } else {
                    150.0
                };
            world
                .bridge_behavior
                .bind_towers(span, [origin, first, replacement, ObjectId(0)]);
            world
                .bridge_behavior
                .bind_towers(other_span, [origin, ObjectId(0), ObjectId(0), ObjectId(0)]);
        }
    });
    assert_eq!(
        delivered,
        [(first, 20.0), (replacement, 30.0), (span, 40.0)],
        "entry fraction is fixed, later slots/maxima live, original span retained"
    );
    assert_eq!(owner.bridge_behavior.span_id_for(origin), Some(other_span));
    if kind == DamageType::Healing {
        assert_eq!(owner.objects[&first].health.current, 70.0);
        assert_eq!(owner.objects[&replacement].health.current, 105.0);
        assert_eq!(owner.objects[&span].health.current, 140.0);
        assert_eq!(owner.objects[&old].health.current, 50.0);
    } else {
        assert_eq!(owner.objects[&first].health.current, 80.0);
        assert_eq!(owner.objects[&replacement].health.current, 120.0);
        assert_eq!(owner.objects[&span].health.current, 160.0);
        assert_eq!(owner.objects[&old].health.current, 100.0);
    }
    assert_eq!(owner.objects[&other_span].health.current, 200.0);
}
#[test]
fn bridge_damage_callback_reads_live_slots_maxima_and_captured_span() {
    isolated(
        "bridge_damage_callback_reads_live_slots_maxima_and_captured_span",
        || live_mirror_delivery_case(DamageType::Unresistable),
    );
}
#[test]
fn bridge_healing_callback_reads_live_slots_maxima_and_captured_span() {
    isolated(
        "bridge_healing_callback_reads_live_slots_maxima_and_captured_span",
        || live_mirror_delivery_case(DamageType::Healing),
    );
}
#[test]
fn bridge_death_callback_reads_later_slot_after_real_preceding_kill() {
    isolated(
        "bridge_death_callback_reads_later_slot_after_real_preceding_kill",
        || {
            let mut owner = GameLogic::new();
            let (span, first, old, _) = bridge(&mut owner, Vec3::new(50.0, 5.0, 50.0));
            let replacement = owner
                .create_object("OwnerTower", Team::USA, Vec3::ZERO)
                .unwrap();
            // Actual span onDie phase already owns its admission latch. This stops
            // a killed tower's recursive span request from reentering this phase.
            owner.objects.get_mut(&span).unwrap().status.on_die_started = true;
            owner.objects.get_mut(&span).unwrap().health.current = 0.0;
            let mut delivered = Vec::new();
            owner.visit_owned_bridge_death_recipients(span, |world, target| {
                delivered.push(target);
                let _ = world.apply_owned_kill_with_effects(
                    target,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    &mut crate::game_logic::ScriptPresentationEffects::default(),
                );
                if target == first {
                    world
                        .bridge_behavior
                        .bind_towers(span, [first, replacement, ObjectId(0), ObjectId(0)]);
                }
            });
            assert_eq!(delivered, [first, replacement]);
            assert!(owner.objects[&first].status.on_die_started);
            assert!(owner.objects[&replacement].status.on_die_started);
            // The first real kill also runs onDamage, which may already kill
            // the old tower through the bridge. The outer onDie traversal must
            // nevertheless resolve the replacement slot before its next kill.
            assert!(owner.objects[&old].status.on_die_started);
        },
    );
}
#[test]
fn actual_residual_damage_finishes_linked_bridge_callbacks_before_return() {
    isolated(
        "actual_residual_damage_finishes_linked_bridge_callbacks_before_return",
        || {
            let mut owner = GameLogic::new();
            let a = bridge(&mut owner, Vec3::new(50.0, 5.0, 50.0));
            let mut peer = GameLogic::new();
            let b = bridge(&mut peer, Vec3::new(50.0, 5.0, 50.0));
            assert_eq!(a, b);
            let mut effects = crate::game_logic::ScriptPresentationEffects::default();
            assert!(!owner.apply_residual_damage_owned_with_effects(
                a.1,
                25.0,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None,
                &mut effects
            ));
            assert_eq!(owner.objects[&a.1].health.current, 75.0);
            assert_eq!(owner.objects[&a.2].health.current, 75.0);
            assert_eq!(owner.objects[&a.0].health.current, 150.0);
            assert_eq!(peer.objects[&b.0].health.current, 200.0);
            assert_eq!(owner.bridge_behavior.mirrors, 1);
            assert_eq!(peer.bridge_behavior.mirrors, 0);
            assert!(!owner.apply_residual_damage_owned_with_effects(
                a.0,
                1000.0,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None,
                &mut effects
            ));
            for id in [a.0, a.1, a.2] {
                assert!(owner.objects[&id].status.on_die_started);
            }
            assert!(owner.bridge_behavior.death_links > 0);
            assert_eq!(peer.bridge_behavior.death_links, 0);
            assert!(!owner.apply_residual_damage_owned_with_effects(
                ObjectId(u32::MAX),
                1.0,
                None,
                DamageType::Unresistable,
                HostDeathType::Normal,
                None,
                &mut effects
            ));
            peer.sync_host_bridge_rubble_and_scaffolds();
            for id in [b.0, b.1, b.2] {
                assert!(!peer.objects[&id].status.on_die_started);
            }
            assert_eq!(peer.objects[&b.0].health.current, 200.0);
        },
    );
}
