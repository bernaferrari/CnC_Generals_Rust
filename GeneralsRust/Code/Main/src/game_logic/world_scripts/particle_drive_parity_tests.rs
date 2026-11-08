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

#[test]
fn encoded_particle_swath_continues_with_captured_settings() {
    isolated("encoded_particle_swath_continues_with_captured_settings", || {
        let mut source = world(Vec3::new(3., 12., 4.), 10);
        let settings = crate::game_logic::special_power_strikes::ParticleSwathSettings {
            swath_of_death_distance: 250.0,
            swath_of_death_amplitude: 150.0,
        };
        let field = &mut source.special_power_strikes.beam_fields[0];
        field.manual_target_mode = false;
        field.scripted_waypoint_mode = false;
        field.swath_settings = settings;
        next(&mut source);
        source.process_destroy_list();
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let encoded = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
        let mut restored = world(Vec3::new(3., 12., 4.), 10);
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
        assert_eq!(restored.special_power_strikes.beam_fields()[0].swath_settings, settings);
        for _ in 0..3 {
            next(&mut source);
            next(&mut restored);
            assert_eq!(
                source.special_power_strikes.beam_fields()[0].current_target_position,
                restored.special_power_strikes.beam_fields()[0].current_target_position,
            );
        }
    });
}

fn swath_world(raw: u8) -> GameLogic {
    let mut owner = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![raw; 32 * 32];
    assert!(
        owner
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_geometry(map)
            .is_empty()
    );
    owner.add_player(Player::new(0, Team::USA, "USA", true));
    let mut template = ThingTemplate::new("SwathSource");
    template.add_kind_of(KindOf::Structure).set_health(5000.);
    owner.templates.insert("SwathSource".into(), template);
    let ground = raw as f32 * 0.625;
    let source = owner
        .create_object_for_player("SwathSource", 0, Vec3::new(20., ground, 30.))
        .unwrap();
    owner
        .special_power_strikes
        .note_script_fired_special_power(source);
    let strike = owner
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::ParticleCannon,
            source,
            Vec3::new(130., ground, 120.),
        )
        .unwrap();
    owner
        .special_power_strikes
        .record_impact_complete(strike, 0., 0, 0);
    owner.frame = owner.special_power_strikes.beam_fields()[0].spawn_frame;
    assert!(!owner.special_power_strikes.beam_fields()[0].manual_target_mode);
    assert!(!owner.special_power_strikes.beam_fields()[0].scripted_waypoint_mode);
    owner
}

#[test]
fn actual_nonmanual_particle_target_uses_receiving_deformation_and_peer_reset() {
    isolated(
        "actual_nonmanual_particle_target_uses_receiving_deformation_and_peer_reset",
        || {
            let mut first = swath_world(40);
            let mut peer = swath_world(90);
            first.update_particle_beam_fields();
            peer.update_particle_beam_fields();
            let first_target = first.special_power_strikes.beam_fields()[0].current_target_position;
            let peer_target = peer.special_power_strikes.beam_fields()[0].current_target_position;
            assert_eq!(first_target.y, 25.);
            assert_eq!(peer_target.y, 56.25);
            assert_eq!(
                first.special_power_strikes.beam_fields()[0].last_swath_position,
                first_target
            );
            assert_eq!(
                first.special_power_strikes.beam_fields()[0].last_scorch_position,
                first_target
            );
            assert_eq!(
                first.special_power_strikes.remnant_fields()[0].position,
                first_target
            );
            assert_eq!(
                peer.special_power_strikes.beam_fields()[0].last_scorch_position,
                peer_target
            );
            {
                let mut terrain = first.world_services.terrain().write().unwrap();
                for y in 0..32 {
                    for x in 0..32 {
                        terrain.set_raw_map_height_with_effects(x, y, 0, &mut |_| {});
                    }
                }
            }
            // Same-frame effects keep the earlier canonical target even after a
            // terrain callback. The next live frame admits the changed ground.
            first.update_particle_beam_fields();
            assert_eq!(
                first.special_power_strikes.beam_fields()[0].current_target_position,
                first_target
            );
            next(&mut first);
            assert_eq!(
                first.special_power_strikes.beam_fields()[0]
                    .current_target_position
                    .y,
                0.
            );
            assert_eq!(
                peer.special_power_strikes.beam_fields()[0].current_target_position,
                peer_target
            );
            peer.reset();
            next(&mut first);
            assert_eq!(
                first.special_power_strikes.beam_fields()[0]
                    .current_target_position
                    .y,
                0.
            );
        },
    );
}

#[test]
fn encoded_nonmanual_particle_target_continues_canonical_frame_and_ground() {
    isolated(
        "encoded_nonmanual_particle_target_continues_canonical_frame_and_ground",
        || {
            let mut source = swath_world(60);
            source.update_particle_beam_fields();
            next(&mut source);
            source.process_destroy_list();
            assert!(!source.has_pending_object_removals());
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
            let mut restored = swath_world(60);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.special_power_strikes.beam_fields()[0].current_target_position,
                source.special_power_strikes.beam_fields()[0].current_target_position
            );
            assert_eq!(
                restored.special_power_strikes.beam_fields()[0].last_drive_update_frame,
                source.special_power_strikes.beam_fields()[0].last_drive_update_frame
            );
            restored.update_particle_beam_fields();
            assert_eq!(
                restored.special_power_strikes.beam_fields()[0].current_target_position,
                source.special_power_strikes.beam_fields()[0].current_target_position
            );
            for _ in 0..12 {
                next(&mut source);
                next(&mut restored);
                let a = &source.special_power_strikes.beam_fields()[0];
                let b = &restored.special_power_strikes.beam_fields()[0];
                assert_eq!(a.current_target_position, b.current_target_position);
                assert_eq!(a.last_drive_update_frame, b.last_drive_update_frame);
                assert_eq!(a.last_swath_position, b.last_swath_position);
                assert_eq!(a.last_scorch_position, b.last_scorch_position);
                assert_eq!(a.next_tick_frame, b.next_tick_frame);
                assert_eq!(a.next_scorch_frame, b.next_scorch_frame);
                assert_eq!(a.current_target_position.y, 37.5);
                assert_eq!(
                    source
                        .special_power_strikes
                        .remnant_fields()
                        .iter()
                        .map(|field| field.position)
                        .collect::<Vec<_>>(),
                    restored
                        .special_power_strikes
                        .remnant_fields()
                        .iter()
                        .map(|field| field.position)
                        .collect::<Vec<_>>()
                );
            }
        },
    );
}

fn authored_swath_world(distance: u32, amplitude: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![10; 32 * 32];
    assert!(world.world_services.terrain().write().unwrap().load_map_geometry(map).is_empty());
    let ini = format!(
        "Object AuthoredSwathCannon\nKindOf = STRUCTURE\nGeometry = BOX\nGeometryMajorRadius = 20\nGeometryMinorRadius = 10\nGeometryHeight = 30\nBody = ActiveBody BodyTag\nMaxHealth = 5000\nInitialHealth = 5000\nEnd\nBehavior = ParticleUplinkCannonUpdate PUCTag\nSwathOfDeathDistance = {distance}\nSwathOfDeathAmplitude = {amplitude}\nEnd\nEnd\n\
         Object SwathVictim\nKindOf = VEHICLE\nGeometry = BOX\nGeometryMajorRadius = 4\nGeometryMinorRadius = 4\nGeometryHeight = 4\nBody = ActiveBody BodyTag\nMaxHealth = 5000\nInitialHealth = 5000\nEnd\nEnd"
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([
        ("authored-swath-gameplay.ini".to_owned(), ini),
    ]).unwrap();
    world.admit_receiving_module_content(content).unwrap();
    world.add_player(Player::new(0, Team::USA, "USA", true));
    let source = world.create_object_for_player(
        "AuthoredSwathCannon", 0, Vec3::new(20., 6.25, 30.),
    ).unwrap();
    (world, source)
}

#[test]
fn authored_swath_fires_damages_and_survives_save_without_peer_leak() {
    isolated("authored_swath_fires_damages_and_survives_save_without_peer_leak", || {
        let (mut first, first_source) = authored_swath_world(250, 150);
        let (mut peer, peer_source) = authored_swath_world(80, 10);
        assert_eq!(first_source, peer_source);
        for (world, source) in [(&mut first, first_source), (&mut peer, peer_source)] {
            world.special_power_strikes.note_script_fired_special_power(source);
            let id = world.queue_special_power_strike(
                &crate::command_system::SpecialPowerType::ParticleCannon,
                source,
                Vec3::new(130., 6.25, 120.),
            ).unwrap();
            world.frame = world.special_power_strikes.get(id).unwrap().impact_frame;
            world.update_special_power_strikes().unwrap();
            assert_eq!(world.special_power_strikes.beam_fields().len(), 1);
            for _ in 0..20 {
                world.frame += 1;
                world.update_particle_beam_fields().unwrap();
            }
        }
        let first_pos = first.special_power_strikes.beam_fields()[0].current_target_position;
        let peer_pos = peer.special_power_strikes.beam_fields()[0].current_target_position;
        assert_ne!(first_pos, peer_pos);
        let first_victim = first.create_object("SwathVictim", Team::GLA, first_pos).unwrap();
        let peer_victim = peer.create_object("SwathVictim", Team::GLA, peer_pos).unwrap();
        assert_eq!(first_victim, peer_victim);
        for (world, victim) in [(&mut first, first_victim), (&mut peer, peer_victim)] {
            let before = world.objects[&victim].health.current;
            for _ in 0..4 {
                world.frame += 1;
                world.update_particle_beam_fields().unwrap();
            }
            assert!(world.objects[&victim].health.current < before);
            world.process_destroy_list();
        }
        let builder = SnapshotBuilder::new();
        let restore = |source: &GameLogic, distance, amplitude| {
            let snapshot = builder.create_world_snapshot(source).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
            let (mut result, _) = authored_swath_world(distance, amplitude);
            builder.restore_from_snapshot(&decoded, &mut result).unwrap();
            result
        };
        let mut first_restored = restore(&first, 250, 150);
        let mut peer_restored = restore(&peer, 80, 10);
        for _ in 0..6 {
            for world in [&mut first, &mut peer, &mut first_restored, &mut peer_restored] {
                world.frame += 1;
                world.update_particle_beam_fields().unwrap();
            }
            for (original, restored, victim) in [
                (&first, &first_restored, first_victim),
                (&peer, &peer_restored, peer_victim),
            ] {
                let a = &original.special_power_strikes.beam_fields()[0];
                let b = &restored.special_power_strikes.beam_fields()[0];
                assert_eq!(a.swath_settings, b.swath_settings);
                assert_eq!(a.current_target_position, b.current_target_position);
                assert_eq!(a.last_swath_position, b.last_swath_position);
                assert_eq!(original.objects[&victim].health.current, restored.objects[&victim].health.current);
            }
            assert_ne!(
                first_restored.special_power_strikes.beam_fields()[0].current_target_position,
                peer_restored.special_power_strikes.beam_fields()[0].current_target_position,
            );
        }
    });
}
