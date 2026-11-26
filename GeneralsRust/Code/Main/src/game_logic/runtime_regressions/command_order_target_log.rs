//! Executable command and lifecycle regressions.

#[test]
fn idle_order_target_publishes_without_starting_an_attack() {
    use crate::game_logic::{AIState, Team};
    use crate::presentation_frame::PresentationFrame;
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
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let u = logic
        .create_object("OrderUnit205", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("OrderBldg205", Team::China, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();

    let frame = logic.frame;
    let (unit, attack_events, audio_events) = logic.host_object_attack_audio_mut(u).unwrap();
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
    assert_eq!(unit.target, Some(b));
    assert_eq!(unit.ai_state(), AIState::Idle);
    assert!(!unit.status.attacking);
    assert!(!unit.force_attack);

    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = presentation.objects.iter().find(|row| row.id == u).unwrap();
    assert_eq!(row.attack_target, Some(b));
}
