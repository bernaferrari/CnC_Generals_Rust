//! Executable command and lifecycle regressions.

#[test]
fn docking_order_publishes_the_pad_without_starting_an_attack() {
    use crate::game_logic::{AIState, Team};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("DockUnit207")
            .kind_of("VEHICLE SELECTABLE")
            .health(100.0)
            .walker(),
        crate::fixture_content::FixtureObject::new("DockPad207")
            .kind_of("STRUCTURE")
            .health(500.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("NonAtk207"),
    );
    apply_skirmish_config(&mut logic, &cfg).unwrap();
    let u = logic
        .create_object("DockUnit207", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let pad = logic
        .create_object("DockPad207", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();

    let frame = logic.frame;
    let (unit, attack_events, audio_events) = logic.host_object_attack_audio_mut(u).unwrap();
    unit.set_ai_state(
        AIState::Idle,
        frame,
        &mut crate::game_logic::ObjectAudio::queue(audio_events),
    );
    unit.set_order_target(
        Some(pad),
        attack_events,
        &mut crate::game_logic::ObjectAudio::queue(audio_events),
    );
    unit.set_ai_state(
        AIState::Docking,
        frame,
        &mut crate::game_logic::ObjectAudio::queue(audio_events),
    );
    assert_eq!(unit.target, Some(pad));
    assert_eq!(unit.ai_state(), AIState::Docking);
    assert!(!unit.status.attacking);

    let presentation = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let row = presentation.objects.iter().find(|row| row.id == u).unwrap();
    assert_eq!(row.attack_target, Some(pad));
}
