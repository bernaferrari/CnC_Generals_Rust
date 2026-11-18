//! Executable command and lifecycle regressions.

fn command_order_target_log() -> bool {
    use crate::game_logic::{AIState, Team};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame; units are authored content.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("OrderUnit205")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker(),
        crate::fixture_content::FixtureObject::new("OrderBldg205")
            .kind_of("STRUCTURE")
            .health(500.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("OrderTgt205"),
    );
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let Some(u) = logic.create_object("OrderUnit205", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(b) = logic.create_object("OrderBldg205", Team::China, Vec3::new(40.0, 0.0, 0.0))
    else {
        return false;
    };

    let frame = logic.frame;
    if let Some((unit, attack_events, audio_events)) = logic.host_object_attack_audio_mut(u) {
        unit.set_ai_state(
            AIState::Idle,
            frame,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.set_order_target(
            Some(b),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        if unit.target != Some(b) {
            return false;
        }
        // Must not force Attacking AI state.
        if unit.ai_state() == AIState::Attacking {
            return false;
        }
        if unit.status.attacking {
            return false;
        }
        if unit.force_attack {
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
    let Some(target) = observer.entity_for_host(b) else {
        return false;
    };
    observer
        .world()
        .entity(actor)
        .is_some_and(|object| object.attack_target == Some(target))
}

#[test]
fn command_order_target_log_runs() {
    assert!(command_order_target_log());
}
