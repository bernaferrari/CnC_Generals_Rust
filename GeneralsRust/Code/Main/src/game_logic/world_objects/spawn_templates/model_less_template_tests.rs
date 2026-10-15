//! C++ ThingFactory admits authored Objects independently of render art.
use super::*;
use crate::presentation_frame::PresentationFrame;

fn definition(name: &str, modules: &str) -> ObjectDefinition {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object {name}\nKindOf = IMMOBILE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\n{modules}\nEnd"), "model-less.ini").unwrap();
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
