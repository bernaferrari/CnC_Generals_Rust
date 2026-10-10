//! Actual residual impacts keep their driving policy after DamageFX.
//! C++ ActiveBody.cpp:653-662 sets repulsor for damaged CAN_BE_REPULSED objects.
use super::super::*;

fn civilian_world(enabled: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.set_enable_repulsors(enabled);
    let mut template = ThingTemplate::new("ResidualRepulsableCivilian");
    template
        .add_kind_of(KindOf::CanBeRepulsed)
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(10_000.0);
    let id = ObjectId(733);
    let mut object = Object::new(template, id, Team::Neutral);
    object.set_position(Vec3::ZERO);
    world.objects.insert(id, object);
    (world, id)
}

fn assert_policy_and_damage(world: &GameLogic, id: ObjectId, enabled: bool) {
    let victim = world
        .host_object(id)
        .expect("residual victim remains installed");
    assert!(
        victim.health.current < victim.health.maximum,
        "the real residual applied HP damage"
    );
    assert_eq!(
        victim.status.repulsor, enabled,
        "only the driving world admits repulsor status after DamageFX"
    );
    assert_eq!(victim.repulsor_until_frame, if enabled { 60 } else { 0 });
    assert!(
        victim.is_alive(),
        "this fixture exercises the ordinary nonlethal ActiveBody tail"
    );
}

#[test]
fn scud_area_damage_reads_the_driving_world_repulsor_policy() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "scud_area_damage_reads_the_driving_world_repulsor_policy",
        || {
            let (mut first, id) = civilian_world(true);
            let (mut second, other) = civilian_world(false);
            assert_eq!(id, other);
            assert_eq!(
                first.apply_scud_area_at(Vec3::ZERO, None, Team::GLA, false),
                (1, false)
            );
            assert_eq!(
                second.apply_scud_area_at(Vec3::ZERO, None, Team::GLA, false),
                (1, false)
            );
            assert_policy_and_damage(&first, id, true);
            assert_policy_and_damage(&second, id, false);
            // Neither a foreign definition reset nor an opposite flag change
            // can choose the next impact's owner. Preserve the same IDs.
            second.reset();
            second.set_enable_repulsors(true);
            first.set_enable_repulsors(false);
            first
                .host_object_mut(id)
                .unwrap()
                .set_status_repulsor(false);
            assert_eq!(
                first.apply_scud_area_at(Vec3::ZERO, None, Team::GLA, false),
                (1, false)
            );
            assert!(!first.host_object(id).unwrap().status.repulsor);
        },
    );
}

#[test]
fn fuel_air_radius_damage_reads_the_driving_world_repulsor_policy() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "fuel_air_radius_damage_reads_the_driving_world_repulsor_policy",
        || {
            let (mut first, id) = civilian_world(true);
            let (mut second, other) = civilian_world(false);
            assert_eq!(id, other);
            for world in [&mut first, &mut second] {
                let result = world.apply_fuel_air_radius_damage(
                    ObjectId(734),
                    None,
                    Team::USA,
                    Vec3::ZERO,
                    10.0,
                    50.0,
                    crate::game_logic::combat::DamageType::Explosive,
                );
                assert_eq!(
                    result,
                    (10.0, 1, 0),
                    "the actual payload splash hit exactly one live victim"
                );
            }
            assert_policy_and_damage(&first, id, true);
            assert_policy_and_damage(&second, id, false);
            // A queried Object is still a standalone value. Match policy is
            // passed by the real world callers rather than copied into objects.
            let mut standalone = first.host_object(id).unwrap().clone();
            standalone.set_status_repulsor(false);
            let mut events = crate::game_logic::HostHealthEvents::default();
            assert!(!standalone.take_damage_from_immediate(10.0, None, &mut events));
            assert!(!standalone.status.repulsor);
            assert!(first.host_object(id).unwrap().status.repulsor);
        },
    );
}
