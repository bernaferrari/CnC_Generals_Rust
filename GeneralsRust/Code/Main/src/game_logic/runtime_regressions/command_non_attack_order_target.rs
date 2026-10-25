//! Executable command and lifecycle regressions.

fn command_non_attack_order_target() -> bool {
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("NonAtk207");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("DockUnit207") {
        let mut t = ThingTemplate::new("DockUnit207");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DockUnit207".into(), t);
    }
    if !logic.templates.contains_key("DockPad207") {
        let mut t = ThingTemplate::new("DockPad207");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("DockPad207".into(), t);
    }
    let Some(u) = logic.create_object("DockUnit207", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(pad) = logic.create_object("DockPad207", Team::USA, Vec3::new(30.0, 0.0, 0.0)) else {
        return false;
    };

    if let Some((unit, attack_events, audio_events)) = logic.host_object_attack_audio_mut(u) {
        unit.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.set_order_target(
            Some(pad),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.set_ai_state(
            AIState::Docking,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        if unit.target != Some(pad) {
            return false;
        }
        if unit.ai_state != AIState::Docking {
            return false;
        }
        if unit.status.attacking {
            return false;
        }
    } else {
        return false;
    }
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(actor) = observer.entity_for_host(u) else {
        return false;
    };
    let Some(target) = observer.entity_for_host(pad) else {
        return false;
    };
    observer
        .world()
        .entity(actor)
        .is_some_and(|object| object.attack_target == Some(target))
}

#[test]
fn command_non_attack_order_target_runs() {
    assert!(command_non_attack_order_target());
}
