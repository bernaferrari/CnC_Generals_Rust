//! Original ParticleUplinkCannon full-vector motion through the native beam route.
use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn world(delta: Vec3, raw: u8) -> GameLogic {
    world_mode(delta, raw, true)
}

fn world_mode(delta: Vec3, raw: u8, scripted: bool) -> GameLogic {
    use gamelogic::system::map_loader::{Coord3D, MapData, MapWaypoint};
    let mut world = GameLogic::new();
    let mut map = MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![raw; 32 * 32];
    for (id, point) in [(1, Vec3::ZERO), (2, delta), (3, Vec3::new(100., 0., 100.))] {
        map.waypoints.push(MapWaypoint {
            id,
            name: format!("Beam{id}"),
            location: Coord3D::new(point.x, point.z, point.y),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    }
    map.waypoint_links = vec![(1, 2), (2, 3)];
    let owner = world.world_services.terrain();
    assert!(owner.write().unwrap().load_map_geometry(map).is_empty());
    world.add_player(Player::new(0, Team::USA, "USA", true));
    let mut template = ThingTemplate::new("BeamSource");
    template.add_kind_of(KindOf::Structure).set_health(5000.);
    world.templates.insert("BeamSource".into(), template);
    let source = world
        .create_object_for_player("BeamSource", 0, Vec3::new(500., 0., 500.))
        .unwrap();
    if scripted {
        world
            .special_power_strikes
            .note_scripted_waypoint_special_power(source, 1);
    }
    let strike = world
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::ParticleCannon,
            source,
            Vec3::ZERO,
        )
        .unwrap();
    world
        .special_power_strikes
        .record_impact_complete(strike, 0., 0, 0);
    world.frame = world.special_power_strikes.beam_fields()[0].spawn_frame;
    // Original component rows begin at the live driving-body boundary, after
    // map admission has legitimately grounded authored waypoint heights.
    // Admit their exact reticle vector through the real public override route.
    // Scripted mode and its next ID survive; manual also being armed does not
    // change the source manual||scripted branch or scripted fast speed.
    let id = world.special_power_strikes.beam_fields()[0].id;
    assert!(
        world
            .special_power_strikes
            .set_beam_override_destination(id, delta, world.frame)
    );
    let field = &world.special_power_strikes.beam_fields()[0];
    assert_eq!(
        field.override_destination.to_array().map(f32::to_bits),
        delta.to_array().map(f32::to_bits),
        "exact original driving-body override input"
    );
    assert_eq!(
        field.current_target_position,
        Vec3::ZERO,
        "exact original driving-body starting input"
    );
    assert!(
        field.manual_target_mode,
        "actual override admission arms manual drive"
    );
    assert_eq!(field.scripted_waypoint_mode, scripted);
    assert_eq!(field.next_dest_waypoint_id, if scripted { 2 } else { 0 });
    world
}

fn next(world: &mut GameLogic) {
    world.frame += 1;
    world.update_particle_beam_fields();
}

#[test]
fn actual_particle_drive_matches_original_zero_tiny_vertical_equal_and_diagonal_cases() {
    isolated(
        "actual_particle_drive_matches_original_zero_tiny_vertical_equal_and_diagonal_cases",
        || {
            let speed = 40.0_f32 / 30.0;
            // Source x/y/height maps to host x/z/y. Next-link admission is STRICT <.
            for (tag, delta, expected, next_id) in [
                ("zero", Vec3::ZERO, Vec3::new(0., 6.25, 0.), 3),
                (
                    "tiny",
                    Vec3::new(0.000001, 0., 0.),
                    Vec3::new(0.000001, 6.25, 0.),
                    3,
                ),
                (
                    "vertical",
                    Vec3::new(0., 4., 0.),
                    Vec3::new(0., 6.25, 0.),
                    2,
                ),
                (
                    "at_speed",
                    Vec3::new(speed, 0., 0.),
                    Vec3::new(speed, 6.25, 0.),
                    2,
                ),
                (
                    "below_speed",
                    Vec3::new(1., 0., 0.),
                    Vec3::new(1., 6.25, 0.),
                    3,
                ),
                (
                    "diagonal",
                    Vec3::new(3., 12., 4.),
                    Vec3::new((3. / 13.) * speed, 6.25, (4. / 13.) * speed),
                    2,
                ),
            ] {
                let mut owner = world(delta, 10);
                next(&mut owner);
                let field = &owner.special_power_strikes.beam_fields()[0];
                assert_eq!(
                    field.next_dest_waypoint_id, next_id,
                    "{tag}: source strict arrival"
                );
                assert_eq!(
                    field.current_target_position.to_array().map(f32::to_bits),
                    expected.to_array().map(f32::to_bits),
                    "{tag}: source divide then scale; horizontal move then ground snap"
                );
                println!(
                    "PARTICLE_DRIVE_CPP {tag} {} {} {} {}",
                    field.current_target_position.x.to_bits(),
                    field.current_target_position.z.to_bits(),
                    field.current_target_position.y.to_bits(),
                    field.next_dest_waypoint_id
                );
            }
        },
    );
}

#[test]
fn actual_manual_particle_drive_uses_full_vector_without_scripted_fast_speed() {
    isolated(
        "actual_manual_particle_drive_uses_full_vector_without_scripted_fast_speed",
        || {
            let mut owner = world_mode(Vec3::new(3., 12., 4.), 10, false);
            next(&mut owner);
            let field = &owner.special_power_strikes.beam_fields()[0];
            let speed = 20.0_f32 / 30.;
            let expected = Vec3::new((3. / 13.) * speed, 6.25, (4. / 13.) * speed);
            assert!(!field.scripted_waypoint_mode);
            assert_eq!(
                field.current_target_position.to_array().map(f32::to_bits),
                expected.to_array().map(f32::to_bits)
            );
            assert_eq!(field.next_dest_waypoint_id, 0);
            println!(
                "PARTICLE_DRIVE_CPP manual_diagonal {} {} {} {}",
                field.current_target_position.x.to_bits(),
                field.current_target_position.z.to_bits(),
                field.current_target_position.y.to_bits(),
                field.next_dest_waypoint_id
            );
        },
    );
}

#[test]
fn actual_particle_ground_snap_uses_receiving_map_and_peer_reset_is_inert() {
    isolated(
        "actual_particle_ground_snap_uses_receiving_map_and_peer_reset_is_inert",
        || {
            let delta = Vec3::new(3., 12., 4.);
            let mut a = world(delta, 10);
            let mut b = world(delta, 30);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let foreign_owner = foreign.services().terrain();
            let held = foreign_owner.write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                next(&mut a);
                next(&mut b);
                assert_eq!(
                    a.special_power_strikes.beam_fields()[0]
                        .current_target_position
                        .y,
                    6.25
                );
                assert_eq!(
                    b.special_power_strikes.beam_fields()[0]
                        .current_target_position
                        .y,
                    18.75
                );
                let before = b.special_power_strikes.beam_fields()[0].current_target_position;
                next(&mut a);
                assert_eq!(
                    b.special_power_strikes.beam_fields()[0].current_target_position,
                    before
                );
                b.reset();
                next(&mut a);
                assert_eq!(
                    a.special_power_strikes.beam_fields()[0]
                        .current_target_position
                        .y,
                    6.25
                );
            });
            drop(held);
        },
    );
}

#[test]
fn encoded_particle_drive_continues_full_vector_and_receiving_ground_after_restore() {
    isolated(
        "encoded_particle_drive_continues_full_vector_and_receiving_ground_after_restore",
        || {
            let delta = Vec3::new(3., 12., 4.);
            let mut source = world(delta, 10);
            next(&mut source);
            // `next` executes only the beam module, not the frame's physical
            // removal phase. The synthetic view currently has an active body;
            // retain that separately tracked content gap instead of snapshotting
            // a dead object whose pending removal is deliberately not serialized.
            let view = source.special_power_strikes.view_objects()[0]
                .object_id
                .expect("actual special-power view object");
            assert_eq!(
                source.objects[&view].template_name,
                "SpecialPowerViewObject"
            );
            assert!(!source.objects[&view].is_alive());
            assert_eq!(
                source
                    .objects_to_destroy
                    .iter()
                    .map(|event| event.id)
                    .collect::<Vec<_>>(),
                vec![view],
                "only the synthetic target view awaits this frame's removal phase"
            );
            let beam = &source.special_power_strikes.beam_fields()[0];
            let caster = beam.source_object;
            assert!(source.objects[&caster].is_alive());
            assert_eq!(beam.current_target_position.y, 6.25);
            source.process_destroy_list();
            assert!(!source.has_pending_object_removals());
            assert!(!source.objects.contains_key(&view));
            assert!(source.objects[&caster].is_alive());
            assert_eq!(source.special_power_strikes.beam_fields().len(), 1);
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
            // Same independently admitted definitions/map and actual team foundation.
            let mut restored = world(delta, 10);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.special_power_strikes.beam_fields()[0].current_target_position,
                source.special_power_strikes.beam_fields()[0].current_target_position
            );
            for _ in 0..3 {
                next(&mut source);
                next(&mut restored);
                let a = &source.special_power_strikes.beam_fields()[0];
                let b = &restored.special_power_strikes.beam_fields()[0];
                assert_eq!(a.current_target_position, b.current_target_position);
                assert_eq!(a.override_destination, b.override_destination);
                assert_eq!(a.next_dest_waypoint_id, b.next_dest_waypoint_id);
                assert_eq!(a.last_drive_update_frame, b.last_drive_update_frame);
                assert_eq!(
                    a.manual_drive_distance_total.to_bits(),
                    b.manual_drive_distance_total.to_bits()
                );
            }
        },
    );
}
