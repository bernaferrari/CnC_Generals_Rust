//! AIUpdate.cpp:4014-4021 — privateGuardPosition rejects !isMobile and PROJECTILE.
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn native_guard_rejects_immobile_and_projectile_actors() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_guard_rejects_immobile_and_projectile_actors",
        || {
            let mut world = GameLogic::new();
            let mobile = ObjectId(1);
            let mut mt = ThingTemplate::new("GuardMobile");
            mt.set_health(100.0).add_kind_of(KindOf::Infantry);
            world.objects.insert(mobile, Object::new(mt, mobile, Team::USA));
            assert!(world.unit_command_guard_position(mobile, Vec3::new(10.0, 0.0, 10.0)));

            let immobile = ObjectId(2);
            let mut it = ThingTemplate::new("GuardImmobile");
            it.set_health(100.0)
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Immobile);
            world
                .objects
                .insert(immobile, Object::new(it, immobile, Team::USA));
            assert!(
                !world.unit_command_guard_position(immobile, Vec3::new(20.0, 0.0, 20.0)),
                "immobile actors cannot enter privateGuardPosition"
            );
            assert!(world.objects[&immobile].guard_position().is_none());

            let projectile = ObjectId(3);
            let mut pt = ThingTemplate::new("GuardProjectile");
            pt.set_health(10.0).add_kind_of(KindOf::Projectile);
            world
                .objects
                .insert(projectile, Object::new(pt, projectile, Team::USA));
            assert!(
                !world.unit_command_guard_position(projectile, Vec3::new(30.0, 0.0, 30.0)),
                "projectile actors cannot enter privateGuardPosition"
            );
            assert!(world.objects[&projectile].guard_position().is_none());
        },
    );
}
