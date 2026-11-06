use super::*;
use crate::command_system::SpecialPowerType;

fn test_module(
    power: SpecialPowerType,
    template: &str,
    kind: SpecialPowerModuleKind,
) -> SpecialPowerModuleMetadata {
    SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_SpecialPower".into()),
        module_kind: kind,
        special_power_template: template.into(),
        special_power_template_id: 1,
        command_power: Some(power),
        reload_time_frames: 0,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    }
}

/// C++ `SpecialPowerStore::canUseSpecialPower` (`SpecialPower.cpp:308`)
/// requires `getSpecialPowerModule`. A tank with no Frenzy/CashHack
/// module must not be ready; a command center that carries the modules is.
#[test]
fn frenzy_and_cash_hack_require_special_power_module() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::China, "China", true));

    let mut tank = ThingTemplate::new("Hq4vpduTank");
    tank.set_health(100.0);
    logic.templates.insert("Hq4vpduTank".into(), tank);
    let tank_id = logic
        .create_object("Hq4vpduTank", Team::China, glam::Vec3::ZERO)
        .expect("tank");

    assert!(
        !logic.is_special_power_ready_for(tank_id, &SpecialPowerType::Frenzy),
        "any-unit Frenzy without SpecialPowerModule must fail closed"
    );
    assert!(
        !logic.is_special_power_ready_for(tank_id, &SpecialPowerType::CashHack),
        "any-unit CashHack without SpecialPowerModule must fail closed"
    );
    assert!(
        !logic.is_special_power_ready_for(tank_id, &SpecialPowerType::EarlyFrenzy),
        "EarlyFrenzy without module must fail closed"
    );
    assert!(!logic.consume_special_power_charge_for(tank_id, &SpecialPowerType::Frenzy));
    assert!(!logic.consume_special_power_charge_for(tank_id, &SpecialPowerType::CashHack));

    let mut cc = ThingTemplate::new("Hq4vpduCC");
    cc.set_health(5000.0);
    cc.special_power_modules.push(test_module(
        SpecialPowerType::Frenzy,
        "SuperweaponFrenzy",
        SpecialPowerModuleKind::OclSpecialPower,
    ));
    cc.special_power_modules.push(test_module(
        SpecialPowerType::CashHack,
        "SuperweaponCashHack",
        SpecialPowerModuleKind::CashHackSpecialPower,
    ));
    logic.templates.insert("Hq4vpduCC".into(), cc);
    let cc_id = logic
        .create_object("Hq4vpduCC", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("cc");

    assert!(
        logic.is_special_power_ready_for(cc_id, &SpecialPowerType::Frenzy),
        "object that carries Frenzy SpecialPowerModule must be ready"
    );
    assert!(
        logic.is_special_power_ready_for(cc_id, &SpecialPowerType::CashHack),
        "object that carries CashHack SpecialPowerModule must be ready"
    );
    assert!(
        !logic.is_special_power_ready_for(cc_id, &SpecialPowerType::EarlyFrenzy),
        "CC without EarlyFrenzy module must not inherit Frenzy"
    );
}

/// C++ Player.cpp:1240-1304 sold / under-construction sources cannot fire.
#[test]
fn sold_and_under_construction_cannot_fire_special_power() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut puc = ThingTemplate::new("Hq6mi3iPuc");
    puc.set_health(4000.0);
    puc.special_power_modules.push(test_module(
        SpecialPowerType::ParticleCannon,
        "SuperweaponParticleUplinkCannon",
        SpecialPowerModuleKind::OclSpecialPower,
    ));
    logic.templates.insert("Hq6mi3iPuc".into(), puc);
    let id = logic
        .create_object("Hq6mi3iPuc", Team::USA, glam::Vec3::ZERO)
        .expect("puc");
    if let Some(o) = logic.host_object_mut(id) {
        o.status.under_construction = false;
        o.status.sold = false;
        o.construction_percent = 1.0;
    }
    assert!(
        logic.is_special_power_ready_for(id, &SpecialPowerType::ParticleCannon),
        "constructed unsold PUC must be ready"
    );

    if let Some(o) = logic.host_object_mut(id) {
        o.status.sold = true;
        o.construction_percent = 0.999;
        o.status.under_construction = false;
    }
    assert!(
        !logic.is_special_power_ready_for(id, &SpecialPowerType::ParticleCannon),
        "C++ Player iterators skip OBJECT_STATUS_SOLD"
    );
    assert!(
        !logic.consume_special_power_charge_for(id, &SpecialPowerType::ParticleCannon),
        "sold PUC must not consume/recharge"
    );

    if let Some(o) = logic.host_object_mut(id) {
        o.status.sold = false;
        o.status.under_construction = true;
        o.construction_percent = 0.4;
    }
    assert!(
        !logic.is_special_power_ready_for(id, &SpecialPowerType::ParticleCannon),
        "C++ Player iterators skip OBJECT_STATUS_UNDER_CONSTRUCTION"
    );
}
