//! Real receiving producer -> host factory -> frozen frame -> typed Client admission.
use super::*;
use crate::presentation_frame::PresentationFrame;
use std::sync::Arc;
// W3DTankDraw.cpp58 + INI.cpp1709: authored dist/sec becomes dist/frame.
const FIRST_RATE_PER_FRAME: f32 = 2.5 * (1.0 / 30.0);
const PEER_RATE_PER_FRAME: f32 = 7.5 * (1.0 / 30.0);

const NAME: &str = "AmericaTankReceivingOwner";
fn source(rate: Option<f32>) -> String {
    let draw = rate.map_or(String::new(), |rate| {
        format!("Draw = W3DTankDraw TankTag\nTreadAnimationRate = {rate}\nEnd\n")
    });
    format!(
        "Object {NAME}\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\n{draw}End"
    )
}
fn producer(text: &str) -> crate::assets::ReceivingObjectContent {
    crate::assets::ReceivingObjectContent::from_sources([(
        "receiving-owned-source.ini".to_owned(),
        text.to_owned(),
    )])
    .unwrap()
}

fn seeded(text: &str) -> GameLogic {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(text, "receiving-owned-source.ini")
        .unwrap();
    let mut world = GameLogic::new();
    assert_eq!(
        world.seed_asset_definition_templates_from_snapshot(
            parser
                .get_all_definitions()
                .iter()
                .map(|(name, row)| (name.clone(), row.clone()))
        ),
        1
    );
    assert!(
        world.templates[NAME].receiving_object_definition.is_some(),
        "actual initial seed must retain captured authored identity before admission"
    );
    world
}
fn receiving(rate: Option<f32>) -> GameLogic {
    let text = source(rate);
    let mut world = seeded(&text);
    world
        .admit_receiving_module_content(producer(&text))
        .unwrap();
    assert!(world.receiving_template_baseline.is_some());
    world
}
fn frame(world: &mut GameLogic, id: ObjectId) -> PresentationFrame {
    let frame = PresentationFrame::build_from_logic(world, 0);
    let direct = frame
        .direct_host_drawables
        .iter()
        .find(|row| row.object.id == id)
        .unwrap();
    assert!(direct.authored_client_template_required);
    assert!(direct.receiving_client_template.is_some());
    frame
}
fn rate(world: &GameLogic, name: &str) -> f32 {
    world
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template(name)
        .unwrap()
        .draw_modules()
        .iter()
        .next()
        .unwrap()
        .data
        .as_any()
        .downcast_ref::<gamelogic::object::draw::W3DTankDrawModuleData>()
        .unwrap()
        .tread_animation_rate
}

#[test]
fn receiving_factory_and_frozen_frame_keep_same_name_same_id_peer_data_after_foreign_reset() {
    let mut first = receiving(Some(2.5));
    let mut peer = receiving(Some(7.5));
    let a = first.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    let b = peer.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(a, b);
    assert_eq!(first.host_object(a).unwrap().health.current, 40.0);
    let plan = first
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template(NAME)
        .unwrap();
    let frozen = frame(&mut first, a);
    assert_eq!(rate(&first, NAME), FIRST_RATE_PER_FRAME);
    assert_eq!(rate(&peer, NAME), PEER_RATE_PER_FRAME);
    peer.reset();
    assert!(Arc::ptr_eq(
        &plan,
        &first
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template(NAME)
            .unwrap()
    ));
    assert_eq!(rate(&first, NAME), FIRST_RATE_PER_FRAME);
    assert_eq!(rate(&peer, NAME), PEER_RATE_PER_FRAME);
    let later = frame(&mut first, a);
    assert_eq!(
        frozen.direct_host_drawables[0].receiving_client_template,
        later.direct_host_drawables[0].receiving_client_template,
        "freezing and peer reset must not parse or replace the receiving capability"
    );
}

#[test]
fn captured_missing_admission_rejects_while_uncaptured_programmatic_template_is_compatibility() {
    let mut captured = seeded(&source(Some(2.5)));
    let error = captured
        .admit_receiving_module_content(producer(""))
        .unwrap_err();
    assert!(
        error.contains("absent from its originating module content"),
        "{error}"
    );
    assert!(captured.receiving_module_content.is_none());
    assert!(captured.objects.registered_is_empty());
    assert!(
        captured
            .create_object(NAME, Team::USA, Vec3::ZERO)
            .is_none(),
        "captured source is not a Compatibility fallback"
    );
    let mut programmatic = GameLogic::new();
    programmatic.templates.insert(
        "UncapturedProgrammatic".into(),
        ThingTemplate::new("UncapturedProgrammatic"),
    );
    let id = programmatic
        .create_object("UncapturedProgrammatic", Team::USA, Vec3::ZERO)
        .unwrap();
    let frame = PresentationFrame::build_from_logic(&mut programmatic, 0);
    let direct = frame
        .direct_host_drawables
        .iter()
        .find(|row| row.object.id == id)
        .unwrap();
    assert!(!direct.authored_client_template_required);
    assert!(direct.receiving_client_template.is_none());
}

#[test]
fn receiving_map_solo_order_and_reset_restore_absent_base_fields_and_remove_map_only_templates() {
    let mut world = receiving(Some(2.5));
    let baseline = world.templates[NAME].asset_scale;
    let peer = world
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template(NAME)
        .unwrap();
    world
        .apply_receiving_module_overrides(&[
            format!("Object {NAME}\nScale = 3\nEnd\nObject ReceivingMapOnly\nScale = 5\nEnd"),
            format!("Object {NAME}\nScale = 4\nEnd"),
        ])
        .unwrap();
    assert_eq!(world.templates[NAME].asset_scale, 4.0);
    assert_eq!(world.templates["ReceivingMapOnly"].asset_scale, 5.0);
    assert!(
        world
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template("ReceivingMapOnly")
            .is_some()
    );
    assert!(!Arc::ptr_eq(
        &peer,
        &world
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template(NAME)
            .unwrap()
    ));
    world.apply_receiving_module_overrides(&[]).unwrap();
    assert_eq!(
        world.templates[NAME].asset_scale, baseline,
        "absent base Scale must erase previous map residue"
    );
    assert!(!world.templates.contains_key("ReceivingMapOnly"));
    assert!(
        world
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template("ReceivingMapOnly")
            .is_none()
    );
    assert!(Arc::ptr_eq(
        &peer,
        &world
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template(NAME)
            .unwrap()
    ));
}

#[test]
fn rejected_receiving_map_tail_keeps_both_catalogs_and_host_templates_unchanged() {
    let mut world = receiving(Some(2.5));
    world
        .apply_receiving_module_overrides(&[format!("Object {NAME}\nScale = 3\nEnd")])
        .unwrap();
    let plan = world
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template(NAME)
        .unwrap();
    let definition = Arc::clone(
        world
            .receiving_object_definitions
            .as_ref()
            .unwrap()
            .get(NAME)
            .unwrap(),
    );
    let error = world.apply_receiving_module_overrides(&[
        format!("Object {NAME}\nScale = 9\nEnd\nObject InvalidTail\nUnrecognizedReceivingField = Yes\nEnd")]).unwrap_err();
    assert!(error.contains("Unknown Object field"), "{error}");
    assert_eq!(world.templates[NAME].asset_scale, 3.0);
    assert!(Arc::ptr_eq(
        &plan,
        &world
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template(NAME)
            .unwrap()
    ));
    assert!(Arc::ptr_eq(
        &definition,
        world
            .receiving_object_definitions
            .as_ref()
            .unwrap()
            .get(NAME)
            .unwrap()
    ));
    assert!(!world.templates.contains_key("InvalidTail"));
}

#[test]
fn encoded_snapshot_rebinds_to_receiving_catalog_without_reparsing_or_peer_substitution() {
    let mut world = receiving(Some(2.5));
    let id = world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = receiving(Some(2.5));
    let expected = restored
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template(NAME)
        .unwrap();
    let mut foreign = receiving(Some(7.5));
    foreign.reset();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(Arc::ptr_eq(
        &expected,
        &restored
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template(NAME)
            .unwrap()
    ));
    assert_eq!(restored.host_object(id).unwrap().health.current, 40.0);
    let _ = frame(&mut restored, id);
    assert_eq!(rate(&restored, NAME), FIRST_RATE_PER_FRAME);
    assert_eq!(rate(&foreign, NAME), PEER_RATE_PER_FRAME);
}

#[cfg(feature = "game_client")]
fn client_entry(
    frame: &PresentationFrame,
    id: ObjectId,
) -> game_client::core::game_client::PresentationDrawableSync {
    let direct = frame
        .direct_host_drawables
        .iter()
        .find(|row| row.object.id == id)
        .unwrap();
    let object = &direct.object;
    game_client::core::game_client::PresentationDrawableSync {
        receiving_science_player: frame.receiving_science_player.clone(),
        module_admission: direct
            .receiving_client_template
            .as_ref()
            .unwrap()
            .client_admission(),
        object_id: id.0,
        ambient_audio_locality: None,
        host_epoch: 1,
        resident: direct.resident,
        visual_template_name: direct.visual_template_name.clone(),
        template_name: object.template_name.clone(),
        position: [object.position.x, object.position.y, object.position.z],
        orientation: object.orientation,
        draw_indicator_color: direct.draw_indicator_color,
        draw_physics: direct.draw_physics,
        draw_velocity_magnitude_squared: direct.draw_velocity_magnitude_squared,
        draw_logic_frame: frame.frame.0,
        float_yaw: 0.0,
        float_pitch: 0.0,
        destroyed: false,
        model_condition_bits: 0,
        body_damage_state: 0,
        kind_names: Arc::new(vec!["VEHICLE".into()]),
        team_color: [1.0; 4],
        effectively_stealthed: false,
        scene_hidden_by_stealth: false,
        health_current: 40.0,
        health_max: 40.0,
        selected: false,
        veterancy_level: 0,
        under_construction: false,
        construction_percent: 1.0,
        sold: false,
        ammo_pip_total: 0,
        ammo_pip_full: 0,
        occupant_count: 0,
        max_garrison: 0,
        disabled: false,
        is_carbomb: false,
        bomb_type: 0,
        bomb_timer_seconds: 0,
        weapon_bonus_enthusiastic: false,
        show_healing: false,
        healing_icon_type: 0,
        garrisoned_ids: vec![],
        emoticon_name: String::new(),
        emoticon_frames_left: 0,
        formation_id: 0,
        caption: String::new(),
        draw_module_names: Arc::new(vec![]),
        health_box_width: 0.0,
        health_box_z_offset: 0.0,
    }
}
#[cfg(feature = "game_client")]
fn installed_rate(
    client: &mut game_client::core::game_client::GameClient,
    id: ObjectId,
) -> Option<f32> {
    let id = client.get_drawable_for_object(id.0).unwrap();
    let drawable = client
        .find_drawable_by_id_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<game_client::drawable::BasicDrawable>()
        .unwrap();
    let modules = drawable.get_draw_modules_mut();
    if modules.is_empty() {
        return None;
    }
    assert_eq!(modules.len(), 1);
    let module = modules[0].logic_module_mut().unwrap();
    Some(
        module
            .get_module_data()
            .as_any()
            .downcast_ref::<gamelogic::object::draw::W3DTankDrawModuleData>()
            .unwrap()
            .tread_animation_rate,
    )
}
#[cfg(feature = "game_client")]
#[test]
fn actual_receiving_factory_frame_installs_typed_client_data_and_authored_empty_without_inference()
{
    let mut first = receiving(Some(2.5));
    let mut peer = receiving(Some(7.5));
    let mut empty = receiving(None);
    let a = first.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    let b = peer.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    let c = empty.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    assert_eq!((a, b), (b, c));
    let first_frame = frame(&mut first, a);
    let peer_frame = frame(&mut peer, b);
    let empty_frame = frame(&mut empty, c);
    let mut receiver = game_client::core::game_client::GameClient::new().unwrap();
    let mut peer_receiver = game_client::core::game_client::GameClient::new().unwrap();
    let mut empty_receiver = game_client::core::game_client::GameClient::new().unwrap();
    let policy = game_client::core::game_client::ReceivingDrawablePolicy {
        time_of_day: Some(game_client::system::TimeOfDay::Afternoon),
        use_draw_module_lod: true,
        static_game_lod: game_engine::common::thing::module::StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    for (world, frame, id, client) in [
        (&mut first, &first_frame, a, &mut receiver),
        (&mut peer, &peer_frame, b, &mut peer_receiver),
        (&mut empty, &empty_frame, c, &mut empty_receiver),
    ] {
        let entry = client_entry(frame, id);
        assert!(matches!(
            &entry.module_admission,
            game_client::core::game_client::PresentationModuleAdmission::Authored(_)
        ));
        client
            .sync_presentation_drawables_observed_with_content(
                [entry],
                0,
                world
                    .receiving_module_content
                    .as_mut()
                    .unwrap()
                    .module_factory_mut(),
                policy,
                |_| {},
            )
            .unwrap();
    }
    assert_eq!(installed_rate(&mut receiver, a), Some(FIRST_RATE_PER_FRAME));
    assert_eq!(
        installed_rate(&mut peer_receiver, b),
        Some(PEER_RATE_PER_FRAME)
    );
    assert_eq!(
        installed_rate(&mut empty_receiver, c),
        None,
        "authoritative empty Draw list must not infer W3DTankDraw from tank identity"
    );
    peer.reset();
    assert_eq!(installed_rate(&mut receiver, a), Some(FIRST_RATE_PER_FRAME));
}

#[test]
fn receiving_snapshot_rejects_unknown_authored_name_before_clearing_live_roster() {
    let mut world = receiving(Some(2.5));
    let id = world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&world).unwrap();
    snapshot.objects.get_mut(&id).unwrap().template_name = "AbsentReceivingObject".into();
    let before = world.host_object(id).unwrap().health.current;
    let result = builder.restore_from_snapshot(&snapshot, &mut world);
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Saved Object") && error.contains("AbsentReceivingObject"),
        "intended receiving definition rejection: {error}"
    );
    assert_eq!(world.host_object(id).unwrap().get_template().name, NAME);
    assert_eq!(world.host_object(id).unwrap().health.current, before);
    assert!(!world.templates.contains_key("AbsentReceivingObject"));
}

#[test]
fn authoritative_empty_content_cannot_spawn_a_synthetic_bootstrap_template() {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources(std::iter::empty()).unwrap(),
        )
        .unwrap();
    assert!(
        world.templates.contains_key("USA_CrusaderTank"),
        "fixture observes actual curated bootstrap presence"
    );
    assert!(
        world
            .create_object("USA_CrusaderTank", Team::USA, Vec3::ZERO)
            .is_none(),
        "native Some(empty) must not create a compatibility substitute"
    );
    assert!(world.host_objects().registered_is_empty());
}

#[test]
fn native_science_frame_uses_uncapped_receiving_membership_and_current_local_viewer() {
    use game_engine::common::rts::science::{ScienceInfo, ScienceNameFacts, ScienceStore};
    fn world(owns_last: bool, observer: bool) -> GameLogic {
        let mut store = ScienceStore::new();
        for id in 1..=40 {
            store.add_science(ScienceInfo::new(id, format!("SCIENCE_Receiving{id:02}")));
        }
        let source = "Object NativeScienceProbe\nKindOf = IMMOBILE\nBody = ActiveBody Body\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = W3DScienceModelDraw ScienceTag\nRequiredScience = SCIENCE_Receiving40\nEnd\nEnd";
        let content = crate::assets::ReceivingObjectContent::from_sources_with_sciences(
            [("native-science.ini".into(), source.into())],
            Arc::new(ScienceNameFacts::from_store(&store)),
        )
        .unwrap();
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(content).unwrap();
        let mut player = Player::new(0, Team::USA, "Receiving", true);
        player.is_observer = observer;
        for id in 1..=if owns_last { 40 } else { 39 } {
            player
                .unlocked_sciences
                .insert(format!("SCIENCE_Receiving{id:02}"));
        }
        world.add_player(player);
        if observer {
            // Object::setTeam redirects inactive controllers to the actual
            // receiving neutral Player; the local science viewer stays observer.
            let mut neutral = Player::new(2, Team::Neutral, "Neutral", false);
            neutral.map_side.role = crate::game_logic::PlayerSideRole::Neutral;
            neutral.map_side.map_player_name.clear();
            world.add_player(neutral);
        }
        world
            .create_object_for_player("NativeScienceProbe", 0, Vec3::ZERO)
            .unwrap();
        world
    }
    let mut first = world(true, false);
    let mut peer = world(false, false);
    let mut observer = world(false, true);
    let a = PresentationFrame::build_from_logic(&mut first, 0);
    let b = PresentationFrame::build_from_logic(&mut peer, 0);
    let o = PresentationFrame::build_from_logic(&mut observer, 0);
    assert_eq!(
        a.local_unlocked_sciences.len(),
        32,
        "the UI cap stays unchanged"
    );
    assert!(
        !a.receiving_science_player.as_ref().unwrap().should_hide(40),
        "native science uses the full receiving set"
    );
    assert!(b.receiving_science_player.as_ref().unwrap().should_hide(40));
    assert!(
        !o.receiving_science_player.as_ref().unwrap().should_hide(40),
        "inactive observer draws without owning the science"
    );
    assert!(
        o.receiving_science_player
            .as_ref()
            .unwrap()
            .should_hide(game_engine::common::rts::SCIENCE_INVALID)
    );
    first.add_player(Player::new(1, Team::China, "SelectedViewer", true));
    let selected_viewer = PresentationFrame::build_from_logic(&mut first, 1);
    assert!(
        selected_viewer
            .receiving_science_player
            .as_ref()
            .unwrap()
            .should_hide(40),
        "changing the local viewer freezes that player's membership"
    );
    assert!(
        !PresentationFrame::build_from_logic(&mut first, 0)
            .receiving_science_player
            .as_ref()
            .unwrap()
            .should_hide(40),
        "a prior selected viewer does not leak into this player"
    );
    let before = a.presentation_hash();
    first
        .get_player_mut(0)
        .unwrap()
        .unlocked_sciences
        .remove("SCIENCE_Receiving40");
    let next = PresentationFrame::build_from_logic(&mut first, 0);
    assert_ne!(
        before,
        next.presentation_hash(),
        "science-only changes affect the frozen hash"
    );
    assert!(
        !a.receiving_science_player.as_ref().unwrap().should_hide(40),
        "prior frame remains immutable"
    );
    assert!(
        PresentationFrame::build_from_logic(&mut peer, 0)
            .receiving_science_player
            .as_ref()
            .unwrap()
            .should_hide(40)
    );
}
