//! C++ ThingFactory admits authored Objects independently of render art.
use super::*;
use crate::presentation_frame::PresentationFrame;

fn source(name: &str, modules: &str) -> String {
    format!(
        "Object {name}\nKindOf = IMMOBILE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\n{modules}\nEnd"
    )
}

fn definition(name: &str, modules: &str) -> ObjectDefinition {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&source(name, modules), "model-less.ini")
        .unwrap();
    parser.get_definition(name).unwrap().clone()
}

fn receiving(name: &str, modules: &str) -> GameLogic {
    let mut world = GameLogic::new();
    assert_eq!(
        world.seed_asset_definition_templates_from_snapshot([(
            name.into(),
            definition(name, modules)
        )]),
        1
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "model-less.ini".into(),
        source(name, modules),
    )])
    .unwrap();
    world.admit_receiving_module_content(content).unwrap();
    world
}

#[test]
fn model_less_template_admits_real_client_only_and_behavior_only_objects_without_name_mesh() {
    for (name, modules) in [
        (
            "USA_Ranger",
            "ClientUpdate = SwayClientUpdate ClientTag\nEnd",
        ),
        (
            "InvisibleLogicAnchor",
            "Behavior = LifetimeUpdate LifetimeTag\nMinLifetime = 10000\nMaxLifetime = 10000\nEnd",
        ),
    ] {
        let mut world = receiving(name, modules);
        let id = world.create_object(name, Team::USA, Vec3::ZERO).unwrap();
        let object = world.host_object(id).unwrap();
        assert_eq!(object.health.current, 40.0);
        assert!(object.get_template().has_authored_no_model());
        assert!(
            object
                .get_template()
                .model_name_for_conditions(0)
                .is_empty()
        );
        let frame = PresentationFrame::build_from_logic(&mut world, 0);
        let record = frame.objects.iter().find(|object| object.id == id).unwrap();
        assert!(record.model_key.is_none());
        assert!(record.draw_models.is_empty());
        let input = frame
            .unit_render_inputs()
            .into_iter()
            .find(|object| object.id == id)
            .unwrap();
        assert!(input.model_key.is_empty());
        assert!(input.draw_models.is_empty());
        world.destroy_object(id);
        world.process_destroy_list();
        assert!(world.host_object(id).is_none());
    }
}

#[test]
fn model_less_template_same_id_peer_and_restore_use_their_receiving_data() {
    let name = "USA_Ranger";
    let mut world = receiving(name, "ClientUpdate = SwayClientUpdate ClientTag\nEnd");
    let mut peer = receiving(
        name,
        "Draw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = PeerExact\nEnd\nEnd",
    );
    let first = world.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    let second = peer.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(first, second);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = receiving(name, "ClientUpdate = SwayClientUpdate ClientTag\nEnd");
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    for owner in [&mut world, &mut restored] {
        let record = PresentationFrame::build_from_logic(owner, 0);
        assert!(
            record
                .objects
                .iter()
                .find(|o| o.id == first)
                .unwrap()
                .draw_models
                .is_empty()
        );
        assert!(
            owner
                .host_object(first)
                .unwrap()
                .get_template()
                .has_authored_no_model()
        );
    }
    let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
    assert_eq!(
        peer_frame
            .objects
            .iter()
            .find(|o| o.id == second)
            .unwrap()
            .model_key
            .as_deref(),
        Some("PeerExact")
    );
}

#[test]
fn model_less_template_refresh_removes_curated_alias_and_keeps_explicit_none_state() {
    let name = "USA_Ranger";
    let mut world = GameLogic::new();
    let mut curated = ThingTemplate::new(name);
    curated.set_model("OldCuratedProxy");
    world.templates.insert(name.into(), curated);
    assert_eq!(world.seed_asset_definition_templates_from_snapshot([(name.into(), definition(name, "Draw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = None\nEnd\nConditionState = DAMAGED\nModel = ExactDamaged\nEnd\nEnd"))]), 0);
    let template = &world.templates[name];
    assert!(template.has_authored_no_model());
    assert!(template.model_name_for_conditions(0).is_empty());
    assert_eq!(
        template.model_name_for_conditions(
            1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_DAMAGED
        ),
        "ExactDamaged"
    );
}

#[test]
fn curated_receiving_body_uses_own_maximum_initial_and_saved_reset_baseline() {
    fn body(maximum: f32, initial: f32) -> GameLogic {
        let text = format!(
            "Object USA_Ranger\nKindOf = IMMOBILE\nBody = ActiveBody AuthoredBody\nMaxHealth = {maximum}f\nInitialHealth = {initial}F\nEnd\nClientUpdate = SwayClientUpdate ClientTag\nEnd\nEnd\n"
        );
        let content =
            crate::assets::ReceivingObjectContent::from_sources([("body-owner.ini".into(), text)])
                .unwrap();
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(content).unwrap();
        world
    }
    let mut first = body(40.0, 25.0);
    let mut peer = body(90.0, 60.0);
    let id = first
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    let peer_id = peer
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(id, peer_id);
    assert_eq!(
        (
            first.host_object(id).unwrap().health.current,
            first.host_object(id).unwrap().health.maximum
        ),
        (25.0, 40.0)
    );
    assert_eq!(
        (
            peer.host_object(peer_id).unwrap().health.current,
            peer.host_object(peer_id).unwrap().health.maximum
        ),
        (60.0, 90.0)
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&first).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = body(40.0, 25.0);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        (
            restored.host_object(id).unwrap().health.current,
            restored.host_object(id).unwrap().health.maximum
        ),
        (25.0, 40.0)
    );
    first.apply_receiving_module_overrides(&["Object USA_Ranger\nReplaceModule AuthoredBody\nBody = ActiveBody MapAuthoredBody\nMaxHealth = 55f\nInitialHealth = 31F\nEnd\nEnd\nEnd\n".into()]).unwrap();
    let map_definition = first
        .receiving_object_definitions
        .as_ref()
        .unwrap()
        .get("USA_Ranger")
        .unwrap();
    let map_body = map_definition
        .behavior_modules
        .iter()
        .find(|module| module.module_tag.as_deref() == Some("MapAuthoredBody"))
        .expect("actual replacement Body definition");
    assert_eq!(map_body.attribute("MaxHealth"), Some("55f"));
    assert_eq!(map_body.attribute("InitialHealth"), Some("31F"));
    assert!(
        !map_definition
            .behavior_modules
            .iter()
            .any(|module| module.module_tag.as_deref() == Some("AuthoredBody"))
    );
    let map_id = first
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        (
            first.host_object(map_id).unwrap().health.current,
            first.host_object(map_id).unwrap().health.maximum
        ),
        (31.0, 55.0)
    );
    first.apply_receiving_module_overrides(&[]).unwrap();
    let baseline = first
        .create_object("USA_Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(
        (
            first.host_object(baseline).unwrap().health.current,
            first.host_object(baseline).unwrap().health.maximum
        ),
        (25.0, 40.0)
    );
    peer.reset();
    assert_eq!(
        (
            restored.host_object(id).unwrap().health.current,
            restored.host_object(id).unwrap().health.maximum
        ),
        (25.0, 40.0)
    );
}
