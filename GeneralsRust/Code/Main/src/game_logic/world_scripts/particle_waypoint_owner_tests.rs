//! Actual scripted ParticleCannon admission, field handoff and receiving update.
use super::*;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn map(axis_z: bool, end: (f32, f32)) -> gamelogic::system::map_loader::MapData {
    use gamelogic::system::map_loader::{Coord3D, MapData, MapWaypoint};
    let mut map = MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 32 * 32];
    let points = if axis_z {
        [(0., 0.), (0., 1.), (0., 50.), end]
    } else {
        [(0., 0.), (1., 0.), (50., 0.), end]
    };
    for (index, (x, y)) in points.into_iter().enumerate() {
        map.waypoints.push(MapWaypoint {
            id: index as u32 + 1,
            name: format!("ParticleWaypoint{index}"),
            location: Coord3D::new(x, y, 0.),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    }
    map.waypoint_links = vec![(1, 2), (2, 3), (3, 4)];
    map
}

fn admit(world: &mut GameLogic, axis_z: bool, end: (f32, f32)) {
    let owner = world.world_services.terrain();
    assert!(
        owner
            .write()
            .unwrap()
            .load_map_geometry(map(axis_z, end))
            .is_empty()
    );
}

fn caster(world: &mut GameLogic) -> ObjectId {
    world.add_player(Player::new(0, Team::USA, "USA", true));
    let mut template = ThingTemplate::new("OwnedParticleCannon");
    template.add_kind_of(KindOf::Structure).set_health(5000.);
    world
        .templates
        .insert("OwnedParticleCannon".into(), template);
    world
        .create_object_for_player("OwnedParticleCannon", 0, Vec3::new(500., 0., 500.))
        .unwrap()
}

fn queue(world: &mut GameLogic, source: ObjectId, waypoint: u32) -> u32 {
    world
        .special_power_strikes
        .note_scripted_waypoint_special_power(source, waypoint);
    world
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::ParticleCannon,
            source,
            Vec3::ZERO,
        )
        .unwrap()
}

#[test]
fn actual_scripted_particle_start_and_arrival_use_current_receiving_waypoints() {
    isolated(
        "actual_scripted_particle_start_and_arrival_use_current_receiving_waypoints",
        || {
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            admit(&mut a, false, (100., 0.));
            admit(&mut b, true, (0., 100.));
            let aid = caster(&mut a);
            let bid = caster(&mut b);
            assert_eq!(aid, bid);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let foreign_owner = foreign.services().terrain();
            let mut held = foreign_owner.write().unwrap();
            held.load_map_geometry(map(false, (250., 250.)));
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let astrike = queue(&mut a, aid, 1);
                let bstrike = queue(&mut b, bid, 1);
                assert_eq!(
                    a.special_power_strikes
                        .get(astrike)
                        .unwrap()
                        .next_dest_waypoint_id,
                    2
                );
                assert_eq!(
                    a.special_power_strikes
                        .get(astrike)
                        .unwrap()
                        .waypoint_override,
                    Vec3::new(1., 0., 0.)
                );
                assert_eq!(
                    b.special_power_strikes
                        .get(bstrike)
                        .unwrap()
                        .waypoint_override,
                    Vec3::new(0., 0., 1.)
                );
                a.special_power_strikes
                    .record_impact_complete(astrike, 0., 0, 0);
                b.special_power_strikes
                    .record_impact_complete(bstrike, 0., 0, 0);
                let spawn = a.special_power_strikes.beam_fields()[0].spawn_frame;
                assert_eq!(spawn, b.special_power_strikes.beam_fields()[0].spawn_frame);
                a.frame = spawn + 1;
                b.frame = spawn + 1;
                a.update_particle_beam_fields();
                b.update_particle_beam_fields();
                let af = &a.special_power_strikes.beam_fields()[0];
                let bf = &b.special_power_strikes.beam_fields()[0];
                assert_eq!(af.next_dest_waypoint_id, 3);
                assert_eq!(bf.next_dest_waypoint_id, 3);
                assert_eq!(af.override_destination, Vec3::new(50., 0., 0.));
                assert_eq!(bf.override_destination, Vec3::new(0., 0., 50.));
                // The old destination vector completes this frame; newly chosen
                // next-link destination only affects the following frame.
                assert_eq!(af.current_target_position, Vec3::new(1., 0., 0.));
                assert_eq!(bf.current_target_position, Vec3::new(0., 0., 1.));
                let peer = bf.clone();
                // Read the receiving chain again on arrival, not a start-time copy.
                admit(&mut a, false, (100., 25.));
                a.frame += 40;
                a.update_particle_beam_fields();
                // ParticleUplinkCannonUpdate.cpp:532–578 converts authored
                // speed to one logic step. A frame-label jump is one update,
                // never forty unexecuted simulation steps.
                let jumped = &a.special_power_strikes.beam_fields()[0];
                assert_eq!(jumped.next_dest_waypoint_id, 3);
                assert_eq!(jumped.override_destination, Vec3::new(50., 0., 0.));
                assert_eq!(jumped.manual_drive_applications, 2);
                assert!((jumped.current_target_position.x - (1. + 40. / 30.)).abs() < 0.00001);
                let after_jump = jumped.current_target_position;
                a.update_particle_beam_fields();
                assert_eq!(
                    a.special_power_strikes.beam_fields()[0].current_target_position,
                    after_jump,
                    "observing the same frame cannot apply another movement step"
                );
                // Execute the remaining real steps to the first arrival,
                // retaining the old-vector/new-destination observation below.
                for step in 1..=36 {
                    a.frame += 1;
                    a.update_particle_beam_fields();
                    if step < 36 {
                        assert_eq!(
                            a.special_power_strikes.beam_fields()[0].next_dest_waypoint_id,
                            3
                        );
                    }
                }
                let af = &a.special_power_strikes.beam_fields()[0];
                assert_eq!(af.manual_drive_applications, 38);
                assert_eq!(af.next_dest_waypoint_id, 4);
                assert_eq!(af.override_destination, Vec3::new(100., 0., 25.));
                assert_eq!(af.current_target_position, Vec3::new(50., 0., 0.));
                assert_eq!(
                    b.special_power_strikes.beam_fields()[0].current_target_position,
                    peer.current_target_position
                );
                assert_eq!(
                    b.special_power_strikes.beam_fields()[0].override_destination,
                    peer.override_destination
                );
                b.reset();
                a.frame += 1;
                a.update_particle_beam_fields();
                assert_eq!(
                    a.special_power_strikes.beam_fields()[0].override_destination,
                    Vec3::new(100., 0., 25.)
                );
            });
            held.reset();
            drop(held);
            a.frame += 1;
            a.update_particle_beam_fields();
            assert_eq!(
                a.special_power_strikes.beam_fields()[0].next_dest_waypoint_id,
                4
            );
        },
    );
}

#[test]
fn actual_scripted_particle_start_keeps_receiving_terminal_or_missing_waypoint_policy() {
    isolated(
        "actual_scripted_particle_start_keeps_receiving_terminal_or_missing_waypoint_policy",
        || {
            let mut world = GameLogic::new();
            admit(&mut world, false, (100., 25.));
            let source = caster(&mut world);
            let terminal = queue(&mut world, source, 4);
            let terminal = world.special_power_strikes.get(terminal).unwrap();
            assert!(terminal.scripted_waypoint_mode);
            assert_eq!(terminal.next_dest_waypoint_id, 4);
            assert_eq!(terminal.waypoint_override, Vec3::new(100., 0., 25.));
            let missing = queue(&mut world, source, 999);
            let missing = world.special_power_strikes.get(missing).unwrap();
            assert!(missing.scripted_waypoint_mode);
            assert_eq!(missing.next_dest_waypoint_id, 999);
            assert_eq!(missing.waypoint_override, Vec3::ZERO);
        },
    );
}
