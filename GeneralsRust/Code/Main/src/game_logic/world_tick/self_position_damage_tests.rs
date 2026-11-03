//! Weapon.cpp:1008/1035 — DamageDealtAtSelfPosition recenters splash on the
//! shooter and clears primary-victim skip (intended ObjectId 0).
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn self_position_splash_centers_on_shooter_not_victim() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "self_position_splash_centers_on_shooter_not_victim",
        || {
            let mut world = GameLogic::new();
            let shooter = ObjectId(1);
            let far_victim = ObjectId(2);
            let near_ally = ObjectId(3);

            let mut st = ThingTemplate::new("SelfPosShooter");
            st.set_health(200.0).add_kind_of(KindOf::Attackable);
            let mut so = Object::new(st, shooter, Team::China);
            so.set_position(Vec3::new(0.0, 0.0, 0.0));
            world.objects.insert(shooter, so);
            world.partition_manager.register_object_at(1, 0.0, 0.0);

            // Far from shooter — would be hit if splash centered on victim.
            let mut vt = ThingTemplate::new("FarVictim");
            vt.set_health(100.0)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Infantry);
            let mut vo = Object::new(vt, far_victim, Team::USA);
            vo.set_position(Vec3::new(100.0, 0.0, 0.0));
            world.objects.insert(far_victim, vo);
            world.partition_manager.register_object_at(2, 100.0, 0.0);

            // Near shooter — hit when splash recenters on self position.
            let mut at = ThingTemplate::new("NearAlly");
            at.set_health(100.0)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Infantry);
            let mut ao = Object::new(at, near_ally, Team::USA);
            ao.set_position(Vec3::new(5.0, 0.0, 0.0));
            world.objects.insert(near_ally, ao);
            world.partition_manager.register_object_at(3, 5.0, 0.0);

            // Mirror object_hit at_self path: splash_pos = shooter, intended = 0.
            let hits = world.apply_instant_hit_splash_at(
                Vec3::new(0.0, 0.0, 0.0),
                25.0,
                0.0,
                20.0,
                0.0,
                shooter,
                Team::China,
                ObjectId(0),
                None,
            );
            assert!(hits >= 1, "self-position splash must hit near occupants");
            assert!(
                world.objects[&near_ally].health.current < 100.0,
                "near ally must take self-centered splash"
            );
            assert_eq!(
                world.objects[&far_victim].health.current, 100.0,
                "far victim must not take self-centered splash"
            );
        },
    );
}
