//! Exact original position witnesses and the local common-grounding phase.
use super::*;
use crate::game_logic::{ObjectId, Team};
use glam::Vec3;

fn near(actual: Vec3, expected: Vec3, case: &str) {
    for (axis, (actual, expected)) in actual
        .to_array()
        .into_iter()
        .zip(expected.to_array())
        .enumerate()
    {
        assert!(
            (actual - expected).abs() <= 0.0002,
            "{case} axis{axis}: original={expected}, Rust={actual}"
        );
    }
}

#[test]
fn swath_position_matches_executed_original_full_vector_frame_and_acos_cases() {
    // Original ParticleUplinkCannonUpdate.cpp480-579 plus exact Coord3D,
    // Vector2 and Matrix3D bodies; DEFAULT_TRIG acosf, portable Inv_Sqrt.
    // Executed independently in527/particle-target-cpp: Config250/150,
    // birth100/death200; ground=.125*x-.25*y+13. Not an x86 bit-identity claim.
    let building = Vec3::new(10., 30., 20.);
    for (case, now, target, expected) in [
        (
            "positive100",
            100,
            Vec3::new(110., 70., 80.),
            Vec3::new(8.53221607, 8.84635544, 20.8806858),
        ),
        (
            "positive125",
            125,
            Vec3::new(110., 70., 80.),
            Vec3::new(-15.0488472, -33.8559227, 179.899261),
        ),
        (
            "positive150",
            150,
            Vec3::new(110., 70., 80.),
            Vec3::new(115.718819, 6.6070261, 83.4313049),
        ),
        (
            "positive175",
            175,
            Vec3::new(110., 70., 80.),
            Vec3::new(246.486481, 30.5516472, 53.0366554),
        ),
        (
            "positive200",
            200,
            Vec3::new(110., 70., 80.),
            Vec3::new(222.905441, 3.92736244, 147.743271),
        ),
        (
            "q2",
            125,
            Vec3::new(-90., -10., 80.),
            Vec3::new(-119.299881, -31.249649, 117.348648),
        ),
        (
            "q3",
            175,
            Vec3::new(-90., 70., -40.),
            Vec3::new(-226.486481, -12.0516472, -13.0366554),
        ),
        (
            "q4",
            125,
            Vec3::new(110., 70., -40.),
            Vec3::new(139.299881, 49.749649, -77.3486481),
        ),
        ("degenerate", 150, building, Vec3::new(10., 9.25, 20.)),
        (
            "vertical_axis",
            125,
            Vec3::new(10., 130., 20.),
            Vec3::new(9.99999332, -29.4041138, 174.616455),
        ),
    ] {
        let mut actual = particle_swath_position(building, target, now, 100, 200, 250., 150.);
        actual.y = (0.125 * actual.x + -0.25 * actual.z) + 13.;
        near(actual, expected, case);
        println!(
            "PARTICLE_TARGET_CPP {case} {} {} {}",
            actual.x, actual.z, actual.y
        );
    }
    // Parent separately executed the unchanged original body with the actual
    // reached retail constants and lifetime; these are not Rust-generated values.
    for (case, now, expected) in [
        (
            "retail_swath_positive_100",
            100,
            Vec3::new(29.9695358, 8.75076294, 31.9817162),
        ),
        (
            "retail_swath_positive_125",
            125,
            Vec3::new(34.999382, -3.24936676, 82.4971619),
        ),
        (
            "retail_swath_positive_150",
            150,
            Vec3::new(57.6288605, -5.71635437, 103.679848),
        ),
        (
            "retail_swath_positive_200",
            200,
            Vec3::new(149.810074, 14.7658806, 67.8415146),
        ),
        (
            "retail_swath_positive_264",
            264,
            Vec3::new(201.408096, 5.01977348, 132.624954),
        ),
    ] {
        let mut actual = particle_swath_position(
            building,
            Vec3::new(110., 70., 80.),
            now,
            100,
            265,
            PARTICLE_SWATH_OF_DEATH_DISTANCE,
            PARTICLE_SWATH_OF_DEATH_AMPLITUDE,
        );
        actual.y = (0.125 * actual.x + -0.25 * actual.z) + 13.;
        near(actual, expected, case);
        println!(
            "PARTICLE_TARGET_CPP {case} {} {} {}",
            actual.x, actual.z, actual.y
        );
    }
}

fn terrain() -> gamelogic::terrain::TerrainLogic {
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = (0..32)
        .flat_map(|y| (0..32).map(move |x| (80 + x + y) as u8))
        .collect();
    let mut terrain = gamelogic::terrain::TerrainLogic::new();
    assert!(terrain.load_map_geometry(map).is_empty());
    terrain
}

#[test]
fn all_beam_modes_share_one_grounded_target_before_scorch_damage_and_remnant() {
    for (manual, scripted) in [(false, false), (true, false), (false, true)] {
        let mut terrain = terrain();
        let random = game_engine::common::random_value::SessionRandom::default();
        let mut registry = HostSpecialPowerStrikeRegistry::new();
        let birth = 10;
        let id = registry.spawn_beam_field_with_manual(
            ObjectId(1),
            Team::USA,
            Vec3::new(120., 91., 110.),
            birth,
            0,
            manual,
        );
        let field = &mut registry.beam_fields[0];
        field.bind_source_axis(Vec3::new(20., 17., 30.));
        field.scripted_waypoint_mode = scripted;
        registry.advance_beam_targets(birth, &terrain, &random);
        let target = registry.beam_fields()[0].current_target_position;
        assert_eq!(
            target.y.to_bits(),
            terrain
                .get_ground_height(target.x, target.z, None)
                .to_bits()
        );
        assert!(target.y > 0. && target.y != 91.);

        // A callback may deform terrain after positioning. Neither a later
        // consumer nor a repeated same-frame admission may resnap this target.
        for y in 0..32 {
            for x in 0..32 {
                terrain.set_raw_map_height_with_effects(x, y, 0, &mut |_| {});
            }
        }
        assert_eq!(terrain.get_ground_height(target.x, target.z, None), 0.);
        registry.advance_beam_targets(birth, &terrain, &random);
        assert_eq!(registry.beam_fields()[0].current_target_position, target);
        let scorch = registry.apply_due_beam_scorch_reveals(birth);
        assert_eq!(scorch.len(), 1);
        assert_eq!(scorch[0].position, target);
        let plans = registry.plan_due_beam_ticks(birth, &[(ObjectId(2), target, Team::GLA, true)]);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].position, target);
        assert_eq!(plans[0].hits.len(), 1);
        registry.record_beam_tick_complete(id, plans[0].hits[0].damage, 1, 0, birth);
        assert_eq!(registry.remnant_fields()[0].position, target);
        assert_eq!(registry.beam_fields()[0].last_swath_position, target);
        assert_eq!(registry.beam_fields()[0].last_scorch_position, target);
        registry.advance_beam_targets(birth + 1, &terrain, &random);
        assert_eq!(registry.beam_fields()[0].current_target_position.y, 0.);
    }
}

#[test]
fn swath_retarget_uses_latest_live_frame_and_encoded_frame_is_not_replayed() {
    let terrain = terrain();
    let random = game_engine::common::random_value::SessionRandom::default();
    let mut registry = HostSpecialPowerStrikeRegistry::new();
    let id = registry.spawn_beam_field(ObjectId(1), Team::USA, Vec3::new(120., 91., 110.), 10, 0);
    registry.beam_fields[0].bind_source_axis(Vec3::new(20., 17., 30.));
    registry.advance_beam_targets(10, &terrain, &random);
    registry.record_beam_tick_complete(id, 0., 0, 0, 10);
    let last_damage_target = registry.beam_fields()[0].last_swath_position;
    registry.advance_beam_targets(11, &terrain, &random);
    let current = registry.beam_fields()[0].current_target_position;
    assert_ne!(
        current, last_damage_target,
        "position advances between damage pulses"
    );
    assert!(registry.set_beam_override_destination(id, current + Vec3::X * 100., 11));
    assert_eq!(registry.beam_fields()[0].current_target_position, current);
    let bytes = bincode_legacy::serialize(&registry.beam_fields()[0]).unwrap();
    let restored: HostParticleBeamField = bincode_legacy::deserialize(&bytes).unwrap();
    assert_eq!(restored.last_drive_update_frame, 11);
    assert_eq!(restored.current_target_position, current);
    let mut peer = HostSpecialPowerStrikeRegistry::new();
    peer.beam_fields.push(restored);
    peer.advance_beam_targets(11, &terrain, &random);
    assert_eq!(peer.beam_fields()[0].current_target_position, current);
    for frame in 12..15 {
        registry.advance_beam_targets(frame, &terrain, &random);
        peer.advance_beam_targets(frame, &terrain, &random);
        assert_eq!(
            peer.beam_fields()[0].current_target_position,
            registry.beam_fields()[0].current_target_position
        );
        assert_eq!(
            peer.beam_fields()[0].last_drive_update_frame,
            registry.beam_fields()[0].last_drive_update_frame
        );
    }
}
