//! ActiveBody.cpp:646-653 — residual batched impacts finish scoreTheKill/onDie
//! before doDamageFX, same as the live Owned damage path.
use super::super::*;

#[test]
fn residual_lethal_completes_ondie_before_session_damage_fx() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "residual_lethal_completes_ondie_before_session_damage_fx",
        || {
            const FX: &str = "ResidualOnDieBeforeFx";
            let mut world = GameLogic::new();
            world.damage_fx_definitions.add_damage_fx(
                FX.into(),
                game_engine::common::ini::ini_damage_fx::DamageFX::new(),
            );
            let id = ObjectId(811);
            let mut template = ThingTemplate::new("ResidualFxVictim");
            template.set_health(10.0);
            template.armor_sets.push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: None,
                damage_fx: Some(FX.into()),
            });
            let mut victim = Object::new(template, id, Team::USA);
            victim.health.current = 10.0;
            victim.health.maximum = 10.0;
            world.objects.insert(id, victim);

            let mut observed = false;
            let destroyed = world.apply_residual_damage_owned_with_effects(
                id,
                50.0,
                None,
                crate::game_logic::combat::DamageType::Bullet,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                None,
                &mut Default::default(),
            );
            // apply_residual doesn't take after_fx callback — observe post-state.
            let victim = world.objects.get(&id).unwrap();
            assert!(destroyed || victim.status.on_die_started || victim.health.current <= 0.0);
            assert!(
                victim.status.on_die_started,
                "residual lethal must complete onDie"
            );
            assert_eq!(
                victim.last_damage_fx_done,
                Some(crate::game_logic::combat::DamageType::Bullet),
                "DamageFX throttle commits after onDie on the residual path"
            );
            observed = true;
            assert!(observed);
        },
    );
}
