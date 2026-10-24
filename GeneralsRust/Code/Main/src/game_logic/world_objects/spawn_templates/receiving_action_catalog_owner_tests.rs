use super::*;
fn world(reload: &str) -> GameLogic {
    use game_engine::common::{
        rts::science::{SCIENCE_INVALID, ScienceInfo, ScienceNameFacts, ScienceStore},
        thing::receiving_module_content::ReceivingActionDefinitions,
    };
    let mut store = ScienceStore::new();
    store.add_science(ScienceInfo::new(SCIENCE_INVALID, "SCIENCE_ActionOwner"));
    let sciences = std::sync::Arc::new(ScienceNameFacts::from_store(&store));
    let mut actions = ReceivingActionDefinitions::default();
    actions.apply_source(&format!("SpecialPower ExplicitOwnedPower\nReloadTime = {reload}\nRequiredScience = SCIENCE_ActionOwner\nEnd\n"),&sciences).unwrap();
    let content=crate::assets::ReceivingObjectContent::from_sources_with_catalogs([("owned-action-object.ini".into(),"Object OwnedActionActor\nKindOf = STRUCTURE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = SpecialAbility SourcePower\nSpecialPowerTemplate = ExplicitOwnedPower\nEnd\nEnd\n".into())],sciences,std::sync::Arc::new(actions)).unwrap();
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "ActionOwner", true));
    world.admit_receiving_module_content(content).unwrap();
    world
}
#[test]
fn actual_receiving_metadata_and_encoded_recreation_keep_same_template_peer_policies() {
    let mut source = world("3000");
    let mut peer = world("9000");
    source
        .apply_receiving_module_overrides(&[
            "SpecialPower ExplicitOwnedPower\nReloadTime = 6000\nEnd\n".into(),
        ])
        .unwrap();
    let id = source
        .create_object_for_player("OwnedActionActor", 0, glam::Vec3::ZERO)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("OwnedActionActor", 0, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(id, peer_id);
    let a = &source.objects[&id].thing().template.special_power_modules[0];
    let b = &peer.objects[&peer_id]
        .thing()
        .template
        .special_power_modules[0];
    assert_eq!(a.special_power_template_id, b.special_power_template_id);
    assert_eq!(a.special_power_template, "ExplicitOwnedPower");
    assert_eq!(a.required_science.as_deref(), Some("SCIENCE_ActionOwner"));
    assert_eq!((a.reload_time_frames, b.reload_time_frames), (180, 270));
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut receiving = world("3000");
    receiving
        .apply_receiving_module_overrides(&[
            "SpecialPower ExplicitOwnedPower\nReloadTime = 6000\nEnd\n".into(),
        ])
        .unwrap();
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(
        receiving.objects[&id]
            .thing()
            .template
            .special_power_modules[0]
            .reload_time_frames,
        180
    );
    peer.reset();
    assert_eq!(
        source.objects[&id].thing().template.special_power_modules[0].reload_time_frames,
        180
    );
    source.reset();
    assert_eq!(
        source.templates["OwnedActionActor"].special_power_modules[0].reload_time_frames,
        90
    );
}
