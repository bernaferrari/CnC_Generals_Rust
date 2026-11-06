//! Weapon.cpp:1264/1303 — authored DamageStatusType reaches Body doStatusDamage.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;

#[test]
fn authored_weapon_damage_status_type_paints_receiving_body() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_weapon_damage_status_type_paints_receiving_body",
        || {
            let mut world = GameLogic::new();
            world.frame = 50;
            world.add_player(Player::new(1, Team::USA, "Victim", true));
            world.add_player(Player::new(2, Team::China, "Painter", false));
            let mut rule = gamelogic::weapon::WeaponTemplate::new("StatusPaintRule".to_string());
            rule.damage_status_type =
                gamelogic::common::ObjectStatusTypes::FaerieFire.into();
            let mut vt = ThingTemplate::new("StatusPaintVictim");
            vt.set_health(100.0).add_kind_of(KindOf::Vehicle);
            world.templates.insert("StatusPaintVictim".into(), vt);
            let mut st = ThingTemplate::new("StatusPaintSource");
            st.set_health(50.0).add_kind_of(KindOf::Infantry);
            world.templates.insert("StatusPaintSource".into(), st);
            let victim = world
                .create_object_for_player("StatusPaintVictim", 1, Vec3::ZERO)
                .unwrap();
            let source = world
                .create_object_for_player("StatusPaintSource", 2, Vec3::new(10.0, 0.0, 0.0))
                .unwrap();
            let context = DamageHitContext::new_selected(
                world.host_object(source),
                Some("StatusPaintRule"),
                DamageType::Status,
                Some(&rule),
            );
            world
                .apply_owned_damage(
                    victim,
                    1000.0, // duration msec → frames
                    Some(source),
                    DamageType::Status,
                    HostDeathType::Normal,
                    None,
                    &context,
                )
                .unwrap();
            let object = world.host_object(victim).unwrap();
            assert!(
                object.status.faerie_fire,
                "DamageStatusType FAERIE_FIRE must paint receiving Body"
            );
            assert_eq!(object.health.current, 100.0, "STATUS never deals HP");
            assert_eq!(
                object.last_damage_info_type,
                Some(DamageType::Status)
            );
        },
    );
}
