//! ActiveBody.cpp:341-346 — freeze DamageInfo.sourceTemplate at impact so
//! AttackedByType still resolves after the shooter is gone (hq-e8k5u).
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;

#[test]
fn owned_damage_freezes_shooter_template_after_source_removed() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "owned_damage_freezes_shooter_template_after_source_removed",
        || {
            let mut world = GameLogic::new();
            world.add_player(Player::new(1, Team::USA, "Victim", true));
            world.add_player(Player::new(2, Team::China, "Shooter", false));
            let mut vt = ThingTemplate::new("TemplateFreezeVictim");
            vt.set_health(100.0).add_kind_of(KindOf::Vehicle);
            world.templates.insert("TemplateFreezeVictim".into(), vt);
            let mut st = ThingTemplate::new("AmericaTankCrusader");
            st.set_health(100.0).add_kind_of(KindOf::Vehicle);
            world.templates.insert("AmericaTankCrusader".into(), st);
            let victim = world
                .create_object_for_player("TemplateFreezeVictim", 1, Vec3::ZERO)
                .unwrap();
            let source = world
                .create_object_for_player(
                    "AmericaTankCrusader",
                    2,
                    Vec3::new(15.0, 0.0, 0.0),
                )
                .unwrap();
            let context =
                DamageHitContext::new(world.host_object(source), None, DamageType::Bullet);
            world
                .apply_owned_damage(
                    victim,
                    10.0,
                    Some(source),
                    DamageType::Bullet,
                    HostDeathType::Normal,
                    None,
                    &context,
                )
                .unwrap();
            assert_eq!(
                world.host_object(victim).unwrap().last_damage_source_template,
                "AmericaTankCrusader"
            );
            // Shooter dies / is removed — frozen template must remain.
            world.objects.remove(&source);
            let object = world.host_object(victim).unwrap();
            assert_eq!(object.last_damage_source, Some(source));
            assert_eq!(
                object.last_damage_source_template, "AmericaTankCrusader",
                "frozen sourceTemplate must survive shooter removal"
            );
        },
    );
}
