use super::*;
use game_engine::common::ini::INILoadType;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::thing_template::{
    AdmittedReceivingTemplate, ObjectIniFields, ReceivingTemplateKind,
};

fn catalog(prefix: &str) -> ModuleFactory {
    let mut keys = NameKeyNamespace::new();
    if !prefix.is_empty() {
        keys.name_to_key(prefix);
    }
    let mut registered = ModuleFactory::new_receiving(keys);
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut registered).unwrap();
    registered
}

fn admit(factory: &mut ModuleFactory, body: &str) -> Arc<AdmittedReceivingTemplate> {
    let text = format!("{body}\nEnd");
    let lines = text.lines().collect::<Vec<_>>();
    let fields = ObjectIniFields::capture_body(&lines, 0).0;
    Arc::new(
        ThingTemplate::new()
            .admit_named_receiving_module_source(
                "AmericaTankCrusader",
                &fields,
                INILoadType::Overwrite,
                ReceivingTemplateKind::Object,
                factory,
            )
            .unwrap(),
    )
}
fn policy() -> ReceivingDrawablePolicy {
    ReceivingDrawablePolicy {
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    }
}
fn entry(plan: Arc<AdmittedReceivingTemplate>) -> PresentationDrawableSync {
    let mut entry = super::super::tests::presentation_drawable_sync_for_test(
        42,
        1,
        "AmericaTankCrusader",
        true,
        false,
        [0.0; 3],
        0.0,
    );
    entry.module_admission = PresentationModuleAdmission::Authored(plan);
    entry.draw_indicator_color = [12, 34, 56];
    entry
}
fn module_data(client: &mut GameClient) -> (Vec<String>, f32, u32) {
    let id = client.get_drawable_for_object(42).unwrap();
    let drawable = client
        .drawables
        .drawable_map
        .get_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    let modules = drawable.get_draw_modules_mut();
    let tags = modules
        .iter()
        .map(|module| module.snapshot_module_identifier().unwrap().to_owned())
        .collect();
    let module = modules[0].logic_module_mut().unwrap();
    let rate = module
        .get_module_data()
        .as_any()
        .downcast_ref::<W3DTankDrawModuleData>()
        .unwrap()
        .tread_animation_rate;
    (tags, rate, module.get_module_tag_name_key())
}

#[test]
fn actual_authored_native_modules_keep_receiving_data_tags_and_namespace_with_foreign_factory_held()
{
    let mut first_catalog = catalog("");
    let mut peer_catalog = catalog("PeerNamespacePrefix");
    let first = admit(
        &mut first_catalog,
        "Draw = W3DTankDraw TankTag\n  TreadAnimationRate = 2.5\nEnd\nClientUpdate = SwayClientUpdate SwayTag\nEnd",
    );
    let peer = admit(
        &mut peer_catalog,
        "Draw = W3DTankDraw TankTag\n  TreadAnimationRate = 7.5\nEnd\nClientUpdate = SwayClientUpdate SwayTag\nEnd",
    );
    let first_tag = first
        .draw_modules()
        .iter()
        .next()
        .unwrap()
        .data
        .get_module_tag_name_key();
    let peer_tag = peer
        .draw_modules()
        .iter()
        .next()
        .unwrap()
        .data
        .get_module_tag_name_key();
    assert_ne!(first_tag, peer_tag);
    let mut receiver = GameClient::new().unwrap();
    let mut peer_receiver = GameClient::new().unwrap();
    let _foreign = game_engine::common::thing::get_thing_factory().unwrap();
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(first)],
            0,
            &mut first_catalog,
            policy(),
            |_| {},
        )
        .unwrap();
    peer_receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(peer)],
            0,
            &mut peer_catalog,
            policy(),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        module_data(&mut receiver),
        (
            vec!["TankTag".into(), "SwayTag".into()],
            2.5 * (1.0 / 30.0),
            first_tag
        )
    );
    assert_eq!(
        module_data(&mut peer_receiver),
        (
            vec!["TankTag".into(), "SwayTag".into()],
            7.5 * (1.0 / 30.0),
            peer_tag
        )
    );
    assert_eq!(module_data(&mut receiver).1, 2.5 * (1.0 / 30.0));
}

#[test]
fn actual_authored_empty_native_template_does_not_infer_draw_from_tank_identity() {
    let mut content = catalog("");
    let plan = admit(&mut content, "KindOf = VEHICLE");
    let mut receiver = GameClient::new().unwrap();
    let _foreign = game_engine::common::thing::get_thing_factory().unwrap();
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(plan)],
            0,
            &mut content,
            policy(),
            |_| {},
        )
        .unwrap();
    let id = receiver.get_drawable_for_object(42).unwrap();
    let drawable = receiver
        .drawables
        .drawable_map
        .get_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    assert!(drawable.get_draw_modules().is_empty());
    assert!(
        receiver
            .presentation_specialized_draw_snapshot(42)
            .is_none()
    );
}

#[test]
fn actual_model_parser_allocates_turret_transition_then_tag_in_receiving_namespace_and_rolls_back_rejection()
 {
    for index in 0..12 {
        NameKeyGenerator::name_to_key(&format!("UnrelatedCompatibility{index}"));
    }
    let mut content = catalog("ReceivingPrefix");
    let plan = admit(
        &mut content,
        "Draw = W3DModelDraw ModelTag\n  DefaultConditionState\n    Model = None\n    Turret = TurretBone\n    TransitionKey = OpenState\n  End\nEnd",
    );
    let entry = plan.draw_modules().iter().next().unwrap();
    let data = entry
        .data
        .as_any()
        .downcast_ref::<W3DModelDrawModuleData>()
        .unwrap();
    let turret = data.condition_states[0].turrets[0].turret_angle_name_key;
    let transition = data.condition_states[0].transition_key;
    let tag = entry.data.get_module_tag_name_key();
    let keys = content.receiving_name_keys_mut().unwrap();
    assert_eq!(keys.key_to_name(turret).as_deref(), Some("turretbone"));
    assert_eq!(keys.key_to_name(transition).as_deref(), Some("openstate"));
    assert_eq!(keys.key_to_name(tag).as_deref(), Some("ModelTag"));
    assert_eq!(transition, turret + 1);
    assert_eq!(tag, transition + 1);
    let mut expected = keys.clone();
    let text = "Draw = W3DModelDraw RejectedTag\n  TransitionState = NewFrom NewTo\n    RejectedField = Yes\n  End\nEnd\nEnd";
    let lines = text.lines().collect::<Vec<_>>();
    let fields = ObjectIniFields::capture_body(&lines, 0).0;
    assert!(
        ThingTemplate::new()
            .admit_named_receiving_module_source(
                "Rejected",
                &fields,
                INILoadType::Overwrite,
                ReceivingTemplateKind::Object,
                &mut content
            )
            .is_err()
    );
    assert_eq!(
        content
            .receiving_name_keys_mut()
            .unwrap()
            .name_to_key("AfterRejected"),
        expected.name_to_key("AfterRejected")
    );
    assert_eq!(entry.data.get_module_tag_name_key(), tag);
}

#[test]
fn actual_authored_native_lod_and_shrub_sway_filter_use_receiving_policy() {
    let mut content = catalog("");
    let plan = admit(
        &mut content,
        "KindOf = SHRUBBERY\nDraw = W3DTankDraw TankTag\n  MinLODRequired = HIGH\nEnd\nClientUpdate = SwayClientUpdate SwayTag\nEnd",
    );
    let mut low = GameClient::new().unwrap();
    low.sync_presentation_drawables_observed_with_content(
        [entry(Arc::clone(&plan))],
        0,
        &mut content,
        ReceivingDrawablePolicy {
            use_draw_module_lod: true,
            static_game_lod: StaticGameLodLevel::Low,
            use_tree_sway: false,
        },
        |_| {},
    )
    .unwrap();
    let id = low.get_drawable_for_object(42).unwrap();
    assert!(
        low.drawables
            .drawable_map
            .get_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .get_draw_modules()
            .is_empty()
    );
    let mut high = GameClient::new().unwrap();
    high.sync_presentation_drawables_observed_with_content(
        [entry(plan)],
        0,
        &mut content,
        policy(),
        |_| {},
    )
    .unwrap();
    assert_eq!(module_data(&mut high).0, ["TankTag", "SwayTag"]);
}

#[test]
fn actual_same_name_receiving_ambient_rebinds_exact_plan_and_never_falls_back_on_absence() {
    use gamelogic::common::types::BodyDamageType;
    fn event(client: &mut GameClient, damage: BodyDamageType) -> Option<String> {
        let id = client.get_drawable_for_object(42).unwrap();
        let drawable = client
            .drawables
            .drawable_map
            .get_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        drawable
            .get_ambient_sound_by_damage(damage)
            .map(|event| event.get_event_name().to_string())
    }
    let mut first = catalog("First");
    let mut peer = catalog("Peer");
    let a = admit(
        &mut first,
        "SoundAmbient = ReceivingA\nSoundAmbientDamaged = ReceivingDamagedA",
    );
    let b = admit(
        &mut peer,
        "SoundAmbient = ReceivingB\nSoundAmbientRubble = ReceivingRubbleB",
    );
    let empty = admit(&mut peer, "KindOf = VEHICLE");
    let mut receiver = GameClient::new().unwrap();
    let mut other = GameClient::new().unwrap();
    let _foreign = game_engine::common::thing::get_thing_factory().unwrap();
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(a.clone())],
            0,
            &mut first,
            policy(),
            |_| {},
        )
        .unwrap();
    other
        .sync_presentation_drawables_observed_with_content(
            [entry(b.clone())],
            0,
            &mut peer,
            policy(),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        event(&mut receiver, BodyDamageType::Pristine).as_deref(),
        Some("ReceivingA")
    );
    assert_eq!(
        event(&mut receiver, BodyDamageType::Damaged).as_deref(),
        Some("ReceivingDamagedA")
    );
    assert_eq!(
        event(&mut other, BodyDamageType::Pristine).as_deref(),
        Some("ReceivingB")
    );
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(b)],
            0,
            &mut peer,
            policy(),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        event(&mut receiver, BodyDamageType::Pristine).as_deref(),
        Some("ReceivingB"),
        "same name retains the new immutable definition identity"
    );
    assert_eq!(
        event(&mut receiver, BodyDamageType::Rubble).as_deref(),
        Some("ReceivingRubbleB")
    );
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(empty)],
            0,
            &mut peer,
            policy(),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        event(&mut receiver, BodyDamageType::Pristine),
        None,
        "authored absence never selects a foreign template"
    );
    assert_eq!(
        event(&mut other, BodyDamageType::Pristine).as_deref(),
        Some("ReceivingB"),
        "peer retains its own admitted plan"
    );
}

#[test]
fn actual_receiving_ambient_damage_ignores_same_id_foreign_core_body() {
    const CHILD: &str = "GENERALS_RECEIVING_AMBIENT_DAMAGE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        use std::io::Read;
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let exact = format!(
            "{}::actual_receiving_ambient_damage_ignores_same_id_foreign_core_body",
            module_path!()
        );
        let exact = exact
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(&exact);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([exact, "--exact", "--test-threads=1", "--nocapture"])
            .env(CHILD, "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let read = |mut stream: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).unwrap();
                bytes
            })
        };
        let readers = [read(Box::new(stdout)), read(Box::new(stderr))];
        let deadline = Instant::now() + Duration::from_secs(25);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("receiving ambient child exceeded 25 seconds");
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let [stdout, stderr] =
            readers.map(|reader| String::from_utf8(reader.join().unwrap()).unwrap());
        assert!(
            status.success(),
            "receiving ambient child failed: {stdout}{stderr}"
        );
        assert!(
            stdout.contains("running 1 test"),
            "exact child did not run one test: {stdout}{stderr}"
        );
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed"),
            "exact child did not pass one test: {stdout}{stderr}"
        );
        return;
    }
    use game_engine::common::bit_flags::{ModelConditionFlags, create_model_condition_flags};
    use gamelogic::common::types::BodyDamageType;
    use gamelogic::object::{Object, registry::OBJECT_REGISTRY};
    use std::sync::RwLock;

    // A fresh child isolates the actual process-wide compatibility registry and
    // audio service without publishing another native world's definitions.
    assert!(OBJECT_REGISTRY.is_empty());
    assert!(game_engine::common::audio::game_audio::get_global_audio_manager().is_none());
    let foreign = Arc::new(RwLock::new(Object::new_for_xfer_load(42, 100.0)));
    assert_eq!(
        foreign
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_damage_state(),
        BodyDamageType::Pristine
    );
    OBJECT_REGISTRY.register_object(42, &foreign);
    let mut factory = catalog("ReceivingDamage");
    let damaged = admit(&mut factory, "SoundAmbientDamaged = ReceivingDamageOnly");
    let pristine = admit(&mut factory, "SoundAmbient = ReceivingPristineOnly");
    let mut receiver = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(damaged)],
            0,
            &mut factory,
            policy(),
            |_| {},
        )
        .unwrap();
    peer.sync_presentation_drawables_observed_with_content(
        [entry(pristine)],
        0,
        &mut factory,
        policy(),
        |_| {},
    )
    .unwrap();
    let drawable = |client: &mut GameClient| client.get_drawable_for_object(42).unwrap();
    let receiver_id = drawable(&mut receiver);
    let peer_id = drawable(&mut peer);
    {
        let received = receiver
            .drawables
            .drawable_map
            .get_mut(&receiver_id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        let mut flags = create_model_condition_flags();
        flags.set(ModelConditionFlags::DAMAGED, true);
        received.replace_model_condition_flags(flags, false);
        received.start_ambient_sound(false);
        assert!(
            received.ambient_sound_is_active(),
            "receiving DAMAGED state selects its own ambient despite same-ID pristine Core body"
        );
    }
    {
        let other = peer
            .drawables
            .drawable_map
            .get_mut(&peer_id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        let mut flags = create_model_condition_flags();
        flags.set(ModelConditionFlags::RUBBLE, true);
        other.replace_model_condition_flags(flags, false);
        other.start_ambient_sound(false);
        assert!(
            !other.ambient_sound_is_active(),
            "receiving RUBBLE absence cannot select foreign pristine ambient"
        );
    }
    assert!(
        receiver
            .drawables
            .drawable_map
            .get_mut(&receiver_id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap()
            .ambient_sound_is_active(),
        "peer restart preserves receiving ambient state"
    );
    assert_eq!(
        foreign
            .read()
            .unwrap()
            .get_body_module()
            .unwrap()
            .lock()
            .unwrap()
            .get_damage_state(),
        BodyDamageType::Pristine
    );
    OBJECT_REGISTRY.unregister_object(42);
}

#[test]
fn authored_science_uses_received_player_and_definition_identity_with_foreign_factory_held() {
    use game_engine::common::rts::science::{ScienceInfo, ScienceNameFacts, ScienceStore};
    use gamelogic::object::draw::w3d_science_model_draw::ReceivingSciencePlayer;
    fn required_science(client: &mut GameClient) -> (i32, Option<bool>) {
        let id = client.get_drawable_for_object(42).unwrap();
        let drawable = client
            .drawables
            .drawable_map
            .get_mut(&id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BasicDrawable>()
            .unwrap();
        let module = drawable.get_draw_modules_mut()[0]
            .logic_module_mut()
            .unwrap();
        let science = game_engine::common::thing::module::Module::as_any_mut(module)
            .downcast_mut::<W3DScienceModelDraw>()
            .unwrap();
        // Call the actual typed native draw phase while the foreign factory is held.
        gamelogic::object::draw::draw_module::DrawModule::do_draw_module(
            science,
            &gamelogic::common::Matrix3D::IDENTITY,
            &gamelogic::helpers::DrawTimeObservation::new(None, false, false, true),
        );
        let required = science
            .get_module_data()
            .as_any()
            .downcast_ref::<W3DScienceModelDrawModuleData>()
            .unwrap()
            .required_science;
        (required, science.receiving_science_hidden())
    }
    let mut sciences = ScienceStore::new();
    sciences.add_science(ScienceInfo::new(7, "SCIENCE_Receiving"));
    let mut content = ModuleFactory::new_receiving_with_sciences(
        NameKeyNamespace::new(),
        Arc::new(ScienceNameFacts::from_store(&sciences)),
    );
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut content).unwrap();
    // C++ INI accepts attached and spaced '=' at this registered reader.
    let mut parsed = Vec::new();
    for field in [
        "RequiredScience=SCIENCE_Receiving",
        "RequiredScience = SCIENCE_Receiving",
        "RequiredScience= SCIENCE_Receiving",
        "RequiredScience =SCIENCE_Receiving",
    ] {
        let body = format!("Draw = W3DScienceModelDraw ScienceTag\n{field}\nEnd");
        let plan = admit(&mut content, &body);
        assert_eq!(
            plan.draw_modules()
                .iter()
                .next()
                .unwrap()
                .data
                .as_any()
                .downcast_ref::<W3DScienceModelDrawModuleData>()
                .unwrap()
                .required_science,
            7,
            "{field}"
        );
        parsed.push(plan);
    }
    let plan = parsed.pop().unwrap();
    let empty = admit(&mut content, "KindOf = VEHICLE");
    let invalid = admit(
        &mut content,
        "Draw = W3DScienceModelDraw InvalidScienceTag\nEnd",
    );
    let high_lod = admit(
        &mut content,
        "Draw = W3DScienceModelDraw HighScienceTag\nMinLODRequired = HIGH\nRequiredScience = SCIENCE_Receiving\nEnd",
    );
    let low_policy = ReceivingDrawablePolicy {
        static_game_lod: StaticGameLodLevel::Low,
        ..policy()
    };
    let mut receiver = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    let mut filtered_receiver = GameClient::new().unwrap();
    let mut retained_receiver = GameClient::new().unwrap();
    let _foreign_factory = game_engine::common::thing::get_thing_factory().unwrap();
    let players = gamelogic::player::ThePlayerList();
    let _foreign_players = players.write().unwrap();
    assert!(
        receiver
            .sync_presentation_drawables_observed_with_content(
                [entry(plan.clone())],
                0,
                &mut content,
                policy(),
                |_| {}
            )
            .is_err(),
        "valid science cannot silently borrow an unavailable player observation"
    );
    for tick in 0..2 {
        filtered_receiver
            .sync_presentation_drawables_observed_with_content(
                [entry(high_lod.clone())],
                tick,
                &mut content,
                low_policy,
                |_| {},
            )
            .unwrap();
        let id = filtered_receiver.get_drawable_for_object(42).unwrap();
        let drawable = filtered_receiver.drawables.drawable_map[&id]
            .as_any()
            .downcast_ref::<BasicDrawable>()
            .unwrap();
        assert!(
            drawable.get_draw_modules().is_empty(),
            "Low constructor policy filters Science; repeated sync needs no player facts"
        );
    }
    let mut retained = entry(high_lod.clone());
    retained.receiving_science_player = Some(Arc::new(ReceivingSciencePlayer::new(true, [7])));
    retained_receiver
        .sync_presentation_drawables_observed_with_content(
            [retained],
            0,
            &mut content,
            policy(),
            |_| {},
        )
        .unwrap();
    let retained_id = retained_receiver.get_drawable_for_object(42).unwrap();
    assert!(
        matches!(
            retained_receiver.sync_presentation_drawables_observed_with_content(
                [entry(high_lod)],
                1,
                &mut content,
                low_policy,
                |_| panic!("missing retained Science facts must fail before mutation"),
            ),
            Err(GameClientError::InvalidOperation(_))
        ),
        "changing current LOD does not remove an already instantiated Science module"
    );
    assert_eq!(
        retained_receiver.get_drawable_for_object(42),
        Some(retained_id)
    );
    assert_eq!(required_science(&mut retained_receiver), (7, Some(false)));
    for (client, owns, active, expected) in [
        (&mut receiver, true, true, false),
        (&mut peer, false, true, true),
    ] {
        let mut row = entry(plan.clone());
        row.receiving_science_player = Some(Arc::new(ReceivingSciencePlayer::new(
            active,
            owns.then_some(7),
        )));
        client
            .sync_presentation_drawables_observed_with_content(
                [row],
                0,
                &mut content,
                policy(),
                |_| {},
            )
            .unwrap();
        assert_eq!(required_science(client), (7, Some(expected)));
    }
    let first_id = receiver.get_drawable_for_object(42).unwrap();
    let mut observer = entry(plan);
    observer.receiving_science_player = Some(Arc::new(ReceivingSciencePlayer::new(false, [])));
    receiver
        .sync_presentation_drawables_observed_with_content(
            [observer],
            1,
            &mut content,
            policy(),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        receiver.get_drawable_for_object(42),
        Some(first_id),
        "facts alone do not replace the definition lifetime"
    );
    assert_eq!(required_science(&mut receiver), (7, Some(false)));
    assert_eq!(required_science(&mut peer), (7, Some(true)));
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(empty)],
            2,
            &mut content,
            policy(),
            |_| {},
        )
        .unwrap();
    let id = receiver.get_drawable_for_object(42).unwrap();
    assert_ne!(
        id, first_id,
        "a different immutable admitted definition replaces typed modules"
    );
    let drawable = receiver
        .drawables
        .drawable_map
        .get_mut(&id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BasicDrawable>()
        .unwrap();
    assert!(
        drawable.get_draw_modules().is_empty(),
        "authored empty never falls back by name"
    );
    receiver
        .sync_presentation_drawables_observed_with_content(
            [entry(invalid)],
            3,
            &mut content,
            policy(),
            |_| {},
        )
        .unwrap();
    assert_eq!(
        required_science(&mut receiver),
        (game_engine::common::rts::SCIENCE_INVALID, Some(true)),
        "invalid science hides before querying a local player"
    );
    // Some(empty) Science facts reject a name even if a global store knows it.
    let mut empty_facts = catalog("EmptyScienceFacts");
    let text =
        "Draw = W3DScienceModelDraw UnknownScience\nRequiredScience = SCIENCE_Receiving\nEnd\nEnd";
    let lines = text.lines().collect::<Vec<_>>();
    let fields = ObjectIniFields::capture_body(&lines, 0).0;
    assert!(
        ThingTemplate::new()
            .admit_named_receiving_module_source(
                "Unknown",
                &fields,
                INILoadType::Overwrite,
                ReceivingTemplateKind::Object,
                &mut empty_facts
            )
            .is_err()
    );
}
