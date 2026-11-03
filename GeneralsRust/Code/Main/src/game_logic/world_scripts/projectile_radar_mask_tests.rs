//! Weapon.cpp:1434-1468 + Object.cpp:1851 — radius-hit radar uses the projectile
//! controller mask, not a launcher rewrite of m_sourceID.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn under_attack_uses_captured_projectile_mask_not_rewritten_launcher_owner() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "under_attack_uses_captured_projectile_mask_not_rewritten_launcher_owner",
        || {
            let mut world = GameLogic::new();
            world.players.insert(0, Player::new(0, Team::USA, "Local", true));
            world.players.insert(1, Player::new(1, Team::China, "Enemy", false));
            assert!(world.select_local_player(Some(0)));

            let victim = ObjectId(10);
            let launcher = ObjectId(20);
            let mut vt = ThingTemplate::new("LocalVictim");
            vt.add_kind_of(KindOf::Infantry).set_health(100.0);
            let mut v = Object::new(vt, victim, Team::USA);
            v.owner_player_id = Some(0);
            v.set_position(Vec3::ZERO);
            world.objects.insert(victim, v);

            // Launcher currently owned by the local player (friendly after capture),
            // but the projectile controller mask is still the enemy.
            let mut lt = ThingTemplate::new("CapturedLauncher");
            lt.set_health(100.0);
            let mut l = Object::new(lt, launcher, Team::USA);
            l.owner_player_id = Some(0);
            l.set_position(Vec3::new(5.0, 0.0, 0.0));
            world.objects.insert(launcher, l);

            let enemy_mask = 1u32 << 1;
            let context = DamageHitContext::new(
                world.objects.get(&launcher),
                None,
                DamageType::Explosive,
            )
            .with_source_player_mask(enemy_mask);
            world
                .apply_owned_damage(
                    victim,
                    5.0,
                    Some(launcher), // rewritten launcher sourceID
                    DamageType::Explosive,
                    HostDeathType::Normal,
                    None,
                    &context,
                )
                .unwrap();

            let obj = world.objects.get(&victim).unwrap();
            assert_eq!(obj.last_damage_source, Some(launcher));
            assert_eq!(obj.last_damage_source_player_mask, enemy_mask);
            assert!(
                world.with_own_services_mut(|world| world.try_under_attack_from_damage(victim)),
                "enemy projectile mask must still raise under-attack despite friendly launcher sourceID"
            );

            // Friendly controller mask must suppress the ping even when sourceID
            // points at a foreign object.
            let mut world = GameLogic::new();
            world.players.insert(0, Player::new(0, Team::USA, "Local", true));
            world.players.insert(1, Player::new(1, Team::China, "Enemy", false));
            assert!(world.select_local_player(Some(0)));
            let mut vt = ThingTemplate::new("LocalVictim");
            vt.add_kind_of(KindOf::Infantry).set_health(100.0);
            let mut v = Object::new(vt, victim, Team::USA);
            v.owner_player_id = Some(0);
            v.set_position(Vec3::ZERO);
            world.objects.insert(victim, v);
            let foreign = ObjectId(30);
            let mut f = Object::new(ThingTemplate::new("Foreign"), foreign, Team::China);
            f.owner_player_id = Some(1);
            world.objects.insert(foreign, f);
            let friendly_mask = 1u32 << 0;
            let context = DamageHitContext::new(
                world.objects.get(&foreign),
                None,
                DamageType::Explosive,
            )
            .with_source_player_mask(friendly_mask);
            world
                .apply_owned_damage(
                    victim,
                    5.0,
                    Some(foreign),
                    DamageType::Explosive,
                    HostDeathType::Normal,
                    None,
                    &context,
                )
                .unwrap();
            assert!(
                !world.with_own_services_mut(|world| world.try_under_attack_from_damage(victim)),
                "friendly projectile mask must suppress under-attack even if sourceID is foreign"
            );
        },
    );
}
