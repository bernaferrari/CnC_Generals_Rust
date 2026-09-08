//! Presentation transform / health / team / model residual tests.

pub use super::*;

#[test]
fn presentation_carries_transform_health_team_model() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresFields");
    assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
    let mut t = ThingTemplate::new("SmokeUnit");
    t.set_health(50.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("SmokeUnit".into(), t);
    let id = logic
        .create_object("SmokeUnit", Team::USA, Vec3::new(3.0, 0.0, 4.0))
        .expect("unit");
    // Retail skirmish runs VICTORY_NOBUILDINGS (C++ GameLogic.cpp:1606) and
    // kills a structure-less player's army on the first frame
    // (VictoryConditions.cpp hasSinglePlayerBeenDefeated → Player::killPlayer).
    // Real players start with a victory-counting structure, so seed one to
    // keep the pinned unit under test alive across logic.update().
    let mut hq = ThingTemplate::new("SmokeHQ");
    hq.set_health(100.0);
    hq.add_kind_of(KindOf::Structure);
    hq.add_kind_of(KindOf::MpCountForVictory);
    logic.templates.insert("SmokeHQ".into(), hq);
    let _hq_id = logic
        .create_object("SmokeHQ", Team::USA, Vec3::new(30.0, 0.0, 40.0))
        .expect("hq");
    logic.update();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let obj = frame
        .objects
        .iter()
        .find(|o| o.id == id)
        .expect("object in presentation");
    assert_eq!(obj.team, Team::USA);
    assert!((obj.position.x - 3.0).abs() < 0.01);
    assert!(obj.health_current > 0.0);
    assert_eq!(obj.health_max, 50.0);
    assert_eq!(obj.model_key.as_deref(), Some("SmokeUnit"));
    assert!(!obj.destroyed);
}

