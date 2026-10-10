//! Existing public setters + the actual owned damage boundary. This fixture
//! also compiles against the old global-gate implementation for a failing control.
use super::*;

fn civilian_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("OwnedRepulsableCivilian");
    template
        .add_kind_of(KindOf::CanBeRepulsed)
        .add_kind_of(KindOf::Infantry);
    let id = ObjectId(731);
    let mut object = Object::new(template, id, Team::Neutral);
    object.health.current = 100.0;
    object.health.maximum = 100.0;
    world.objects.insert(id, object);
    (world, id)
}

#[test]
fn same_id_damage_reads_only_the_driving_world_repulsor_policy() {
    // Native catalogs and compatibility effects are process services. Bound
    // this actual-dispatch fixture without acquiring a shared test mutex.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "same_id_damage_reads_only_the_driving_world_repulsor_policy",
        || {
            let (mut first, id) = civilian_world();
            let (mut second, other_id) = civilian_world();
            assert_eq!(id, other_id);
            first.set_enable_repulsors(true);
            second.set_enable_repulsors(false);
            for world in [&mut first, &mut second] {
                let result = world
                    .apply_owned_damage(
                        id,
                        10.0,
                        None,
                        crate::game_logic::combat::DamageType::Bullet,
                        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                    )
                    .unwrap();
                assert!(!result.destroyed);
            }
            assert!(
                first.objects[&id].status.repulsor,
                "a foreign setter must not disable the first world"
            );
            assert_eq!(first.objects[&id].repulsor_until_frame, 60);
            assert!(!second.objects[&id].status.repulsor);
            assert_eq!(second.objects[&id].repulsor_until_frame, 0);
            second.reset();
            drop(second);
            first
                .objects
                .get_mut(&id)
                .unwrap()
                .set_status_repulsor(false);
            let _ = first
                .apply_owned_damage(
                    id,
                    10.0,
                    None,
                    crate::game_logic::combat::DamageType::Bullet,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                )
                .unwrap();
            assert!(
                first.objects[&id].status.repulsor,
                "foreign reset/drop cannot select a damage policy"
            );
        },
    );
}

#[test]
fn standalone_object_damage_has_no_ambient_match_repulsor_policy() {
    // Native catalogs and compatibility effects are process services. Bound
    // this actual-dispatch fixture without acquiring a shared test mutex.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "standalone_object_damage_has_no_ambient_match_repulsor_policy",
        || {
            let (mut world, id) = civilian_world();
            world.set_enable_repulsors(true);
            let mut standalone = world.objects[&id].clone();
            let mut events = crate::game_logic::HostHealthEvents::default();
            assert!(!standalone.take_damage(10.0, &mut events));
            assert!(
                !standalone.status.repulsor,
                "a standalone wrapper has no driving match policy"
            );
            assert!(!world.objects[&id].status.repulsor);
            assert_eq!(world.objects[&id].health.current, 100.0);
        },
    );
}
