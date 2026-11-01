use generals_main::ai::{AIDifficulty, AIPlayer};
use generals_main::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use glam::Vec3;

fn world(
    enemy_pos: Vec3,
) -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    ThingTemplate,
) {
    let mut world = GameLogic::new();
    let site = ThingTemplate::new("BuildSite");
    let mut enemy = ThingTemplate::new("Threat");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert(enemy.name.clone(), enemy);
    let id = world
        .create_object("Threat", Team::China, enemy_pos)
        .unwrap();
    (world, id, site)
}

#[test]
fn range_pruning_preserves_threats_visibility_nan_and_world_ownership() {
    let ai = AIPlayer::new(0, Team::USA, AIDifficulty::Medium);
    let (mut owner, id, site) = world(Vec3::ZERO);
    let (peer, peer_id, peer_site) = world(Vec3::new(1_000_000.0, 0.0, 0.0));
    assert_eq!(id, peer_id);
    assert!(!ai.is_location_safe(&owner, Vec3::ZERO, Some(&site)));
    assert!(ai.is_location_safe(&peer, Vec3::ZERO, Some(&peer_site)));
    owner.host_object_mut(id).unwrap().status.stealthed = true;
    assert!(ai.is_location_safe(&owner, Vec3::ZERO, Some(&site)));
    owner.host_object_mut(id).unwrap().status.detected = true;
    assert!(!ai.is_location_safe(&owner, Vec3::ZERO, Some(&site)));
    let radius = game_engine::common::ini::AIData::default().supply_center_safe_radius;
    let (boundary, _, boundary_site) = world(Vec3::new(radius, 0.0, 0.0));
    let (outside, _, outside_site) = world(Vec3::new(radius + 1.0, 0.0, 0.0));
    assert!(
        !ai.is_location_safe(&boundary, Vec3::ZERO, Some(&boundary_site)),
        "boundary remains inside the safety radius"
    );
    assert!(
        ai.is_location_safe(&outside, Vec3::ZERO, Some(&outside_site)),
        "outside the safety radius stays safe"
    );
    assert!(
        !ai.is_location_safe(&owner, Vec3::new(f32::NAN, 0.0, 0.0), Some(&site)),
        "NaN uses the original ordered comparison"
    );
}
