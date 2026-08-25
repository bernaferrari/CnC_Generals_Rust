//! Phase 6: command queue view → GameWorld mutations same tick.

use super::*;
use crate::command_system::{CommandType, GameCommand, ModifierKeys};
use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
use crate::gameworld_shadow::shadow_session_after_host_tick;
use std::time::SystemTime;

#[test]
fn attack_command_lands_set_attack_target_same_tick() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("P6Atk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["P6AtkA", "P6AtkB"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.set_health(120.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let attacker = logic
        .create_object("P6AtkA", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("a");
    let target = logic
        .create_object("P6AtkB", Team::China, glam::Vec3::new(30.0, 0.0, 30.0))
        .expect("t");
    {
        let o = logic.host_object_mut(attacker).expect("a");
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    assert!(shadow.ingest_command_queue_view(&logic) >= 1);
    let ea = shadow.entity_for_host(attacker).expect("ea");
    let et = shadow.entity_for_host(target).expect("et");
    assert_eq!(
        shadow.world().entity(ea).expect("e").attack_target,
        Some(et)
    );
    logic.process_commands();
    let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
    assert_eq!(
        shadow.world().entity(ea).expect("e").attack_target,
        Some(et)
    );
}
