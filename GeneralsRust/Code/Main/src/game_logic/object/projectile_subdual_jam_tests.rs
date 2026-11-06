//! ActiveBody.cpp:1280-1286 — projectileNowJammed on subdual threshold (hq-hskels).
use super::*;
use crate::game_logic::combat::DamageType;

#[test]
fn guided_missile_jams_on_subdual_threshold_dumb_shell_does_not() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();
    let mut audio_events = Vec::new();

    let mut missile_t = ThingTemplate::new("TestGuidedMissile");
    missile_t
        .set_health(1.0)
        .add_kind_of(KindOf::Projectile)
        .add_kind_of(KindOf::SmallMissile);
    let mut missile = Object::new(missile_t, ObjectId(501), Team::China);
    missile.subdual_damage_cap = 50.0;
    missile.tomahawk_missile_projectile = true;
    missile.tomahawk_missile_aim = Some([100.0, 0.0, 0.0]);
    missile.apply_subdual_damage(
        50.0,
        &mut health_events,
        &mut crate::game_logic::ObjectAudio::record(&mut audio_events),
    );
    assert!(missile.is_subdued());
    assert!(
        missile.ecm_missile_jammed,
        "MissileAIUpdate::projectileNowJammed must run on subdued edge"
    );
    assert_ne!(missile.tomahawk_missile_aim, Some([100.0, 0.0, 0.0]));

    let mut shell_t = ThingTemplate::new("TestTankShell");
    shell_t
        .set_health(1.0)
        .add_kind_of(KindOf::Projectile);
    let mut shell = Object::new(shell_t, ObjectId(502), Team::USA);
    shell.subdual_damage_cap = 50.0;
    shell.technical_cannon_shell_projectile = true;
    shell.apply_subdual_damage(
        50.0,
        &mut health_events,
        &mut crate::game_logic::ObjectAudio::record(&mut audio_events),
    );
    assert!(shell.is_subdued());
    assert!(
        !shell.ecm_missile_jammed,
        "DumbProjectileBehavior::projectileNowJammed is empty"
    );
    let _ = DamageType::SubdualMissile;
}
