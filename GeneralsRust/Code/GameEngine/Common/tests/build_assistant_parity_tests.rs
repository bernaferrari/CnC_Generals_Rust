use game_engine::common::system::build_assistant::{
    clear_build_assistant_backend, BuildAssistant, CanMakeType, Coord3D, LegalBuildCode,
    LocalLegalToBuildOptions, Object, Player, ThingTemplate,
};

#[test]
fn build_assistant_fails_closed_without_gamelogic_backend() {
    clear_build_assistant_backend();

    let assistant = BuildAssistant::new();
    let builder = Object {
        id: 7,
        position: Coord3D::new(0.0, 0.0, 0.0),
        orientation: 0.0,
    };
    let player = Player { player_index: 0 };
    let template = ThingTemplate::new("AmericaPowerPlant");
    let pos = Coord3D::new(64.0, 128.0, 0.0);

    assert_eq!(
        assistant.is_location_legal_to_build(
            &pos,
            &template,
            0.0,
            LocalLegalToBuildOptions::TERRAIN_RESTRICTIONS
                | LocalLegalToBuildOptions::CLEAR_PATH
                | LocalLegalToBuildOptions::NO_OBJECT_OVERLAP,
            Some(&builder),
            Some(&player),
        ),
        LegalBuildCode::GenericFailure
    );
    assert!(assistant
        .build_object_now(Some(&builder), &template, &pos, 0.0, &player)
        .is_none());
    assert!(!assistant.is_possible_to_make_unit(&builder, &template));
    assert_eq!(
        assistant.can_make_unit(&builder, &template),
        CanMakeType::NoPrereq
    );
}
