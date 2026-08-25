//! Host GameLogic tests — `strategy_and_stealth`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

fn attach_command_special_power(
    tpl: &mut ThingTemplate,
    power: crate::command_system::SpecialPowerType,
    template_name: &str,
    kind: SpecialPowerModuleKind,
) {
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_SpecialPower".into()),
        module_kind: kind,
        special_power_template: template_name.into(),
        special_power_template_id: 1,
        command_power: Some(power),
        reload_time_frames: 0,
        required_science: None,
        public_timer: true,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    });
}

fn arm_attacker_then_attack(attacker: &mut Object, target_id: ObjectId, damage: f32) {
    attacker.weapon = Some(Weapon {
        damage,
        range: 200.0,
        reload_time: 0.0,
        last_fire_time: 0.0,
        projectile_speed: 0.0,
        ..Weapon::default()
    });
    attacker.attack_target(target_id);
}

fn spawn_live_black_market(logic: &mut GameLogic, team: Team, pos: Vec3) -> ObjectId {
    if !logic.templates.contains_key("GLABlackMarket") {
        let mut market = ThingTemplate::new("GLABlackMarket");
        market
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBlackMarket)
            .set_health(1000.0);
        logic.templates.insert("GLABlackMarket".into(), market);
    }
    logic
        .create_object("GLABlackMarket", team, pos)
        .expect("black market")
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod stealth_and_detectors;
mod strategy_and_artillery;
