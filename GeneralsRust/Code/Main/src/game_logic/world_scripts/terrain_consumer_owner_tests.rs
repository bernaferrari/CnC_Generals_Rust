//! Actual OCL planning and WaveGuide update observe their receiving map.
use super::*;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn map(
    boundary: i32,
    first: (f32, f32),
    last: (f32, f32),
) -> gamelogic::system::map_loader::MapData {
    use gamelogic::system::map_loader::{Coord3D, MapData, MapWaypoint};
    let mut map = MapData::new();
    map.width = 128;
    map.height = 128;
    map.heightmap = vec![0; 128 * 128];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(boundary, boundary)];
    for (id, name, xy) in [(1, "WaveGuide1", first), (2, "WaveGuideEnd", last)] {
        map.waypoints.push(MapWaypoint {
            id,
            name: name.into(),
            location: Coord3D::new(xy.0, xy.1, 0.),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    }
    map.waypoint_links.push((1, 2));
    map
}

fn admit(world: &mut GameLogic, boundary: i32, first: (f32, f32), last: (f32, f32)) {
    let owner = world.world_services.terrain();
    assert!(
        owner
            .write()
            .unwrap()
            .load_map_geometry(map(boundary, first, last))
            .is_empty()
    );
}

fn caster(world: &mut GameLogic) -> ObjectId {
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    world.add_player(player);
    let mut template = ThingTemplate::new("ReceivingCommandCenter");
    template.add_kind_of(KindOf::Structure).set_health(5000.);
    world
        .templates
        .insert("ReceivingCommandCenter".into(), template);
    world
        .create_object_for_player("ReceivingCommandCenter", 0, Vec3::new(190., 0., 180.))
        .unwrap()
}

#[test]
fn actual_ocl_plan_uses_receiving_active_extent_with_foreign_terrain_locked() {
    isolated(
        "actual_ocl_plan_uses_receiving_active_extent_with_foreign_terrain_locked",
        || {
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            admit(&mut a, 20, (10., 20.), (1000., 20.));
            admit(&mut b, 40, (30., 40.), (30., 1000.));
            let aid = caster(&mut a);
            let bid = caster(&mut b);
            assert_eq!(aid, bid);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let foreign_owner = foreign.services().terrain();
            let mut held = foreign_owner.write().unwrap();
            held.load_map_geometry(map(80, (70., 80.), (1000., 80.)));
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let target = Vec3::new(100., 0., 100.);
                let ap = a
                    .plan_ocl_special_power("SuperweaponDaisyCutter", aid, target)
                    .unwrap();
                let bp = b
                    .plan_ocl_special_power("SuperweaponDaisyCutter", bid, target)
                    .unwrap();
                assert_eq!(ap.creation_coord, Vec3::new(200., 0., 180.));
                assert_eq!(bp.creation_coord, Vec3::new(190., 0., 0.));
                assert_eq!(ap.target_coord, target);
                assert_eq!(bp.target_coord, target);
                b.reset();
                assert_eq!(
                    a.plan_ocl_special_power("SuperweaponDaisyCutter", aid, target)
                        .unwrap()
                        .creation_coord,
                    ap.creation_coord
                );
            });
            held.reset();
            held.load_map_geometry(map(60, (90., 80.), (90., 1000.)));
            drop(held);
            assert_eq!(a.ocl_map_extents(), (0., 0., 200., 200.));
        },
    );
}

fn wave(world: &mut GameLogic) -> ObjectId {
    let mut template = ThingTemplate::new("WaveGuide1");
    template.add_kind_of(KindOf::WaveGuide).set_health(100.);
    world.templates.insert("WaveGuide1".into(), template);
    let id = world
        .create_object("WaveGuide1", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let obj = world.objects.get_mut(&id).unwrap();
    obj.wave_guide_data = Some(crate::game_logic::host_wave_guide::HostWaveGuideData {
        active_frame: 1,
        ..Default::default()
    });
    obj.status.disabled_default = false;
    world.frame = 10000;
    id
}

#[test]
fn actual_waveguide_update_binds_receiving_complete_chain_and_survives_peer_reset() {
    isolated(
        "actual_waveguide_update_binds_receiving_complete_chain_and_survives_peer_reset",
        || {
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            admit(&mut a, 20, (10., 20.), (1000., 20.));
            admit(&mut b, 40, (30., 40.), (30., 1000.));
            let aid = wave(&mut a);
            let bid = wave(&mut b);
            assert_eq!(aid, bid);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let foreign_owner = foreign.services().terrain();
            foreign_owner
                .write()
                .unwrap()
                .load_map_geometry(map(80, (70., 80.), (1000., 80.)));
            let held = foreign_owner.read().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                a.update_wave_guides();
                b.update_wave_guides();
                let ap = a.objects.get(&aid).unwrap();
                let bp = b.objects.get(&bid).unwrap();
                assert_eq!(
                    ap.wave_guide_data.as_ref().unwrap().final_destination,
                    Some((1000., 20.))
                );
                assert_eq!(
                    bp.wave_guide_data.as_ref().unwrap().final_destination,
                    Some((30., 1000.))
                );
                assert_eq!(ap.get_position().z, 20.);
                assert_eq!(bp.get_position().x, 30.);
                assert!(ap.get_position().x > 10.);
                assert!(bp.get_position().z > 40.);
                b.reset();
                a.frame += 1;
                a.update_wave_guides();
                assert_eq!(
                    a.objects
                        .get(&aid)
                        .unwrap()
                        .wave_guide_data
                        .as_ref()
                        .unwrap()
                        .final_destination,
                    Some((1000., 20.))
                );
            });
            drop(held);
            foreign_owner.write().unwrap().reset();
            a.objects
                .get_mut(&aid)
                .unwrap()
                .wave_guide_data
                .as_mut()
                .unwrap()
                .initialized = false;
            a.frame += 1;
            a.update_wave_guides();
            assert_eq!(
                a.objects
                    .get(&aid)
                    .unwrap()
                    .wave_guide_data
                    .as_ref()
                    .unwrap()
                    .final_destination,
                Some((1000., 20.))
            );
        },
    );
}
