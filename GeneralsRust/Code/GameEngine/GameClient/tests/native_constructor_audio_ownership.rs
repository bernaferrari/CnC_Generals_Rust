//! The production GameClient admission path submits constructor audio to the
//! explicitly bound receiving manager, even for same-ID peer Drawables.
use game_client_rust::core::game_client::{
    ClientInputAuthority, GameClient, NativeDrawableBirthContext, NativeParticleRetirementService,
    PresentationDrawPhysics, PresentationDrawableSync, PresentationModuleAdmission,
    ReceivingDrawablePolicy,
};
use game_engine::common::audio::audio_event_rts::{AC_LOOP, ST_GLOBAL};
use game_engine::common::audio::game_audio::{AudioLocalityRelationship, AudioLocalityResolver};
use game_engine::common::audio::{AudioEventInfo, AudioManager, SoundPlaybackHook};
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module::StaticGameLodLevel;
use game_engine::common::thing::module_factory::ModuleFactory;
use game_engine::common::thing::receiving_module_content::ReceivingModuleContent;
use game_engine::common::thing::thing_template::AdmittedReceivingTemplate;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

struct Players;
impl AudioLocalityResolver for Players {
    fn get_local_player_index(&self) -> Option<i32> {
        Some(1)
    }
    fn is_player_active(&self, _: i32) -> bool {
        true
    }
    fn player_exists(&self, _: i32) -> bool {
        true
    }
    fn has_default_team(&self, _: i32) -> bool {
        true
    }
    fn get_relationship_to_local_team(&self, _: i32, _: i32) -> AudioLocalityRelationship {
        AudioLocalityRelationship::Allies
    }
}

fn admitted() -> (ReceivingModuleContent, Arc<AdmittedReceivingTemplate>) {
    let mut factory = ModuleFactory::new_receiving(NameKeyNamespace::new());
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut factory).unwrap();
    let source = "Object OwnedAmbient\nSoundAmbient = OwnedPristine\nSoundAmbientDamaged = OwnedDamaged\nDraw = W3DModelDraw AmbientTag\nDefaultConditionState\nModel = NONE\nEnd\nEnd\nEnd";
    let mut content = ReceivingModuleContent::new(factory);
    content
        .load_ini_text(source, INILoadType::Overwrite)
        .unwrap();
    let template = content.admitted_template("OwnedAmbient").unwrap();
    (content, template)
}

fn audio() -> AudioManager {
    let mut audio = AudioManager::new();
    audio.init();
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "OwnedPristine".into(),
        sounds: vec!["owned_sample".into()],
        type_field: ST_GLOBAL,
        control: AC_LOOP,
        loop_count: 0,
        ..Default::default()
    });
    audio.register_audio_event_info(AudioEventInfo {
        audio_name: "OwnedDamaged".into(),
        sounds: vec!["owned_damaged_sample".into()],
        type_field: ST_GLOBAL,
        control: AC_LOOP,
        loop_count: 0,
        ..Default::default()
    });
    audio
}

#[derive(Clone)]
struct Playback {
    events: Arc<Mutex<Vec<(char, u32)>>>,
    is_playing: bool,
}
impl SoundPlaybackHook for Playback {
    fn play(&self, event: &game_engine::common::audio::AudioEventRts) -> Result<(), String> {
        self.events
            .lock()
            .unwrap()
            .push(('P', event.get_playing_handle()));
        Ok(())
    }
    fn stop(&self, handle: u32) {
        self.events.lock().unwrap().push(('S', handle));
    }
    fn pause(&self, _: u32) {}
    fn resume(&self, _: u32) {}
    fn is_playing(&self, _: u32) -> bool {
        self.is_playing
    }
}

fn recording_audio(events: Arc<Mutex<Vec<(char, u32)>>>) -> AudioManager {
    recording_audio_with_state(events, true)
}

fn recording_audio_with_state(
    events: Arc<Mutex<Vec<(char, u32)>>>,
    is_playing: bool,
) -> AudioManager {
    let mut audio = AudioManager::with_playback_hook(Box::new(Playback { events, is_playing }));
    audio.init();
    for (name, sample) in [
        ("OwnedPristine", "owned_sample"),
        ("OwnedDamaged", "owned_damaged_sample"),
    ] {
        audio.register_audio_event_info(AudioEventInfo {
            audio_name: name.into(),
            sounds: vec![sample.into()],
            type_field: ST_GLOBAL,
            control: AC_LOOP,
            loop_count: 0,
            ..Default::default()
        });
    }
    audio
}

#[derive(Clone)]
struct TodPlayback(Arc<Mutex<Vec<game_engine::common::audio::audio_event_rts::TimeOfDay>>>);
impl SoundPlaybackHook for TodPlayback {
    fn play(&self, event: &game_engine::common::audio::AudioEventRts) -> Result<(), String> {
        self.0.lock().unwrap().push(event.get_time_of_day());
        Ok(())
    }
    fn stop(&self, _: u32) {}
    fn pause(&self, _: u32) {}
    fn resume(&self, _: u32) {}
    fn is_playing(&self, _: u32) -> bool {
        true
    }
}

fn tod_recording_audio(
    played: Arc<Mutex<Vec<game_engine::common::audio::audio_event_rts::TimeOfDay>>>,
) -> AudioManager {
    let mut audio = AudioManager::with_playback_hook(Box::new(TodPlayback(played)));
    audio.init();
    for (name, sample) in [
        ("OwnedPristine", "owned_sample"),
        ("OwnedDamaged", "owned_damaged_sample"),
    ] {
        audio.register_audio_event_info(AudioEventInfo {
            audio_name: name.into(),
            sounds: vec![sample.into()],
            type_field: ST_GLOBAL,
            control: AC_LOOP,
            loop_count: 0,
            ..Default::default()
        });
    }
    audio
}

fn entry(plan: Arc<AdmittedReceivingTemplate>) -> PresentationDrawableSync {
    PresentationDrawableSync {
        module_admission: PresentationModuleAdmission::Authored(plan),
        receiving_science_player: None,
        draw_indicator_color: [255; 3],
        draw_physics: PresentationDrawPhysics::default(),
        draw_velocity_magnitude_squared: 0.0,
        draw_logic_frame: 4,
        object_id: 77,
        ambient_audio_locality: Some(
            game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players),
        ),
        host_epoch: 3,
        resident: true,
        visual_template_name: "OwnedAmbient".into(),
        template_name: "OwnedAmbient".into(),
        position: [0.0; 3],
        orientation: 0.0,
        float_yaw: 0.0,
        float_pitch: 0.0,
        destroyed: false,
        model_condition_bits: 1 << 3, // ModelConditionFlags::DAMAGED
        body_damage_state: 1,
        kind_names: Arc::new(Vec::new()),
        team_color: [1.0; 4],
        effectively_stealthed: false,
        scene_hidden_by_stealth: false,
        health_current: 100.0,
        health_max: 100.0,
        selected: false,
        veterancy_level: 0,
        under_construction: false,
        construction_percent: 0.0,
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
        garrisoned_ids: Vec::new(),
        emoticon_name: String::new(),
        emoticon_frames_left: 0,
        formation_id: 0,
        caption: String::new(),
        draw_module_names: Arc::new(Vec::new()),
        health_box_width: 0.0,
        health_box_z_offset: 0.0,
    }
}

fn client(audio: Arc<Mutex<AudioManager>>) -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.particle_systems_mut().finish_definition_admission();
    client.bind_native_audio_device(audio);
    client
}

#[test]
fn gameclient_constructor_routes_same_id_ambient_to_each_bound_manager() {
    let device_a = Arc::new(Mutex::new(audio()));
    let device_b = Arc::new(Mutex::new(audio()));
    let mut a = client(Arc::clone(&device_a));
    let mut b = client(Arc::clone(&device_b));
    let (mut content_a, plan_a) = admitted();
    let (mut content_b, plan_b) = admitted();
    let policy = ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Night),
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    a.sync_presentation_drawables_observed_with_content(
        [entry(plan_a)],
        0,
        content_a.module_factory_mut(),
        policy,
        |_| {},
    )
    .unwrap();
    b.sync_presentation_drawables_observed_with_content(
        [entry(plan_b)],
        0,
        content_b.module_factory_mut(),
        policy,
        |_| {},
    )
    .unwrap();
    assert_eq!(a.get_drawable_for_object(77), b.get_drawable_for_object(77));
    assert_eq!(
        device_a
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedPristine"),
        1
    );
    assert_eq!(
        device_b
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedPristine"),
        1
    );
    assert_eq!(
        device_a
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedDamaged"),
        0
    );
    assert_eq!(
        device_b
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedDamaged"),
        0
    );
}

#[test]
fn auxiliary_retirement_stops_only_on_its_bound_manager() {
    let events_a = Arc::new(Mutex::new(Vec::new()));
    let events_b = Arc::new(Mutex::new(Vec::new()));
    let device_a = Arc::new(Mutex::new(recording_audio(Arc::clone(&events_a))));
    let device_b = Arc::new(Mutex::new(recording_audio(Arc::clone(&events_b))));
    let mut a = client(Arc::clone(&device_a));
    let mut b = client(Arc::clone(&device_b));
    let (mut content_a, plan_a) = admitted();
    let (mut content_b, plan_b) = admitted();
    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    let policy = ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Night),
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    let make_aux = |client: &mut GameClient,
                    content: &mut ModuleFactory,
                    plan: Arc<AdmittedReceivingTemplate>| {
        let (_, mut owner, ..) = client.split_render_bridge_drawables_and_ui_renderer();
        owner
            .create_auxiliary_drawable_with_locality(
                plan,
                content,
                1.0,
                policy,
                game_client_rust::core::game_client::NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                9,
                0,
                Some(&locality),
                |_| Ok(()),
            )
            .unwrap()
    };
    let receipt_a = make_aux(&mut a, content_a.module_factory_mut(), plan_a);
    let receipt_b = make_aux(&mut b, content_b.module_factory_mut(), plan_b);
    assert_eq!(receipt_a.drawable_id(), receipt_b.drawable_id());
    device_a.lock().unwrap().process_request_list();
    device_b.lock().unwrap().process_request_list();
    let handle_a = a
        .find_drawable_by_id(receipt_a.drawable_id())
        .unwrap()
        .as_any()
        .downcast_ref::<game_client_rust::drawable::BasicDrawable>()
        .unwrap()
        .owned_ambient_audio_handle()
        .unwrap();
    let handle_b = b
        .find_drawable_by_id(receipt_b.drawable_id())
        .unwrap()
        .as_any()
        .downcast_ref::<game_client_rust::drawable::BasicDrawable>()
        .unwrap()
        .owned_ambient_audio_handle()
        .unwrap();
    {
        let (_, mut owner, ..) = a.split_render_bridge_drawables_and_ui_renderer();
        owner
            .retire_auxiliary_drawable(&receipt_a, |_| Ok(()))
            .unwrap();
    }
    device_a.lock().unwrap().process_request_list();
    assert_eq!(
        events_a.lock().unwrap().as_slice(),
        &[('P', handle_a), ('S', handle_a)]
    );
    assert_eq!(events_b.lock().unwrap().as_slice(), &[('P', handle_b)]);
}

#[test]
fn drawable_xfer_restore_stops_and_restarts_only_on_receiving_world_audio() {
    let events_a = Arc::new(Mutex::new(Vec::new()));
    let events_b = Arc::new(Mutex::new(Vec::new()));
    let device_a = Arc::new(Mutex::new(recording_audio(Arc::clone(&events_a))));
    let device_b = Arc::new(Mutex::new(recording_audio(Arc::clone(&events_b))));
    let mut a = client(Arc::clone(&device_a));
    let mut b = client(Arc::clone(&device_b));
    let (mut content_a, plan_a) = admitted();
    let (mut content_b, plan_b) = admitted();
    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    let policy = ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Night),
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    let make_aux = |client: &mut GameClient,
                    content: &mut ModuleFactory,
                    plan: Arc<AdmittedReceivingTemplate>| {
        let (_, mut owner, ..) = client.split_render_bridge_drawables_and_ui_renderer();
        owner
            .create_auxiliary_drawable_with_locality(
                plan,
                content,
                1.0,
                policy,
                game_client_rust::core::game_client::NativeDrawableBirthContext {
                    loading_map: false,
                    in_load_game: false,
                },
                9,
                0,
                Some(&locality),
                |_| Ok(()),
            )
            .unwrap()
    };
    let receipt_a = make_aux(&mut a, content_a.module_factory_mut(), plan_a);
    let receipt_b = make_aux(&mut b, content_b.module_factory_mut(), plan_b);
    assert_eq!(receipt_a.drawable_id(), receipt_b.drawable_id());
    for (client, receipt, x) in [(&mut a, &receipt_a, 7.0), (&mut b, &receipt_b, 2.0)] {
        let drawable = client
            .find_drawable_by_id_mut(receipt.drawable_id())
            .unwrap();
        drawable.set_object_id(Some(444));
        drawable.set_position(game_client_rust::drawable::Vector3::new(x, 0.0, 0.0));
    }
    device_a.lock().unwrap().process_request_list();
    device_b.lock().unwrap().process_request_list();
    assert_eq!(events_a.lock().unwrap().len(), 1);
    assert_eq!(events_b.lock().unwrap().len(), 1);

    // The serialized Drawable state is restored into the same-ID peer through
    // the public production Drawable Xfer hook used by GameClient's roster.
    let bytes = {
        let drawable = a.find_drawable_by_id_mut(receipt_a.drawable_id()).unwrap();
        let mut bytes = Vec::new();
        let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
        drawable.xfer_snapshot(&mut save).unwrap();
        bytes
    };
    {
        let drawable = b.find_drawable_by_id_mut(receipt_b.drawable_id()).unwrap();
        let mut load = XferLoad::new(Cursor::new(bytes.as_slice()), 1);
        drawable
            .xfer_snapshot_for_native_load(&mut load, Some(&device_b))
            .unwrap();
    }
    device_b.lock().unwrap().process_request_list();
    assert_eq!(
        events_a.lock().unwrap().len(),
        1,
        "source world remains live"
    );
    assert_eq!(
        events_b.lock().unwrap().len(),
        2,
        "restore stops peer's old loop"
    );

    b.load_drawable_post_process_with_native_audio_in_order(
        &[receipt_b.drawable_id()],
        Some(game_client_rust::system::TimeOfDay::Night),
        |_| locality,
        |object_id| {
            (object_id == Some(444))
                .then(|| glam::Mat4::from_translation(glam::Vec3::new(20.0, 3.0, 0.0)))
        },
    )
    .unwrap();
    device_b.lock().unwrap().process_request_list();
    assert_eq!(
        a.find_drawable_by_id(receipt_a.drawable_id())
            .unwrap()
            .get_position()
            .x,
        7.0
    );
    assert_eq!(
        b.find_drawable_by_id(receipt_b.drawable_id())
            .unwrap()
            .get_position()
            .x,
        20.0
    );
    assert_eq!(events_a.lock().unwrap().len(), 1);
    assert_eq!(
        events_b.lock().unwrap().len(),
        3,
        "post-load restarts on peer manager"
    );
}

#[test]
fn drawable_load_post_process_applies_receiving_transform_without_audio_device() {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.particle_systems_mut().finish_definition_admission();
    let (mut content, plan) = admitted();
    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    let policy = ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Night),
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    let receipt = {
        let (_, mut owner, ..) = client.split_render_bridge_drawables_and_ui_renderer();
        owner
            .create_auxiliary_drawable_with_locality(
                plan,
                content.module_factory_mut(),
                1.0,
                policy,
                game_client_rust::core::game_client::NativeDrawableBirthContext {
                    loading_map: true,
                    in_load_game: false,
                },
                9,
                0,
                Some(&locality),
                |_| Ok(()),
            )
            .unwrap()
    };
    let drawable = client
        .find_drawable_by_id_mut(receipt.drawable_id())
        .unwrap();
    drawable.set_object_id(Some(444));
    drawable.set_position(game_client_rust::drawable::Vector3::new(2.0, 0.0, 0.0));

    client
        .load_drawable_post_process_with_native_audio_in_order(
            &[receipt.drawable_id()],
            None,
            |_| locality,
            |object_id| {
                (object_id == Some(444))
                    .then(|| glam::Mat4::from_translation(glam::Vec3::new(30.0, 4.0, 0.0)))
            },
        )
        .unwrap();
    assert_eq!(
        client
            .find_drawable_by_id(receipt.drawable_id())
            .unwrap()
            .get_position()
            .x,
        30.0,
        "device-less post-load still uses the receiving transform"
    );
}

#[test]
fn map_loading_suppresses_constructor_audio_without_tod_or_locality() {
    let device = Arc::new(Mutex::new(audio()));
    let mut client = client(Arc::clone(&device));
    let (mut content, plan) = admitted();
    let mut source = entry(plan);
    source.ambient_audio_locality = None;
    let policy = ReceivingDrawablePolicy {
        time_of_day: None,
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    client
        .borrow_native_drawable_owner()
        .admit_semantic_drawable_with_content(
            source,
            NativeDrawableBirthContext {
                loading_map: true,
                in_load_game: false,
            },
            0,
            content.module_factory_mut(),
            policy,
            &mut NativeParticleRetirementService,
            |_| Ok(()),
        )
        .expect("map construction does not require suppressed ambient audio");
    assert_eq!(
        device
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedPristine"),
        0
    );
}

#[test]
fn map_loading_owner_starts_at_level_start_and_retires_on_same_manager() {
    let events_a = Arc::new(Mutex::new(Vec::new()));
    let events_b = Arc::new(Mutex::new(Vec::new()));
    let device_a = Arc::new(Mutex::new(recording_audio(Arc::clone(&events_a))));
    let device_b = Arc::new(Mutex::new(recording_audio(Arc::clone(&events_b))));
    let mut a = client(Arc::clone(&device_a));
    let mut b = client(Arc::clone(&device_b));
    let (mut content_a, plan_a) = admitted();
    let (mut content_b, plan_b) = admitted();
    let policy = ReceivingDrawablePolicy {
        time_of_day: None,
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    let mut retirement = NativeParticleRetirementService;
    for (client, content, plan) in [
        (&mut a, &mut content_a, plan_a),
        (&mut b, &mut content_b, plan_b),
    ] {
        let mut owner = client.borrow_native_drawable_owner();
        owner
            .admit_semantic_drawable_with_content(
                entry(plan),
                NativeDrawableBirthContext {
                    loading_map: true,
                    in_load_game: false,
                },
                0,
                content.module_factory_mut(),
                policy,
                &mut retirement,
                |_| Ok(()),
            )
            .unwrap();
    }
    assert_eq!(a.get_drawable_for_object(77), b.get_drawable_for_object(77));
    assert_eq!(device_a.lock().unwrap().pending_play_request_count(), 0);
    assert_eq!(device_b.lock().unwrap().pending_play_request_count(), 0);
    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    for client in [&mut a, &mut b] {
        client
            .borrow_native_drawable_owner()
            .on_level_start_with_audio_at(game_client_rust::system::TimeOfDay::Night, |_| locality)
            .unwrap();
    }
    assert_eq!(
        device_a
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedDamaged"),
        1
    );
    assert_eq!(
        device_b
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedDamaged"),
        1
    );
    assert_eq!(
        device_a
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedPristine"),
        0
    );
    assert_eq!(
        device_b
            .lock()
            .unwrap()
            .pending_play_request_count_for("OwnedPristine"),
        0
    );
    device_a.lock().unwrap().process_request_list();
    device_b.lock().unwrap().process_request_list();
    let handle_a = a
        .find_drawable_by_id(a.get_drawable_for_object(77).unwrap())
        .unwrap()
        .as_any()
        .downcast_ref::<game_client_rust::drawable::BasicDrawable>()
        .unwrap()
        .owned_ambient_audio_handle()
        .unwrap();
    let handle_b = b
        .find_drawable_by_id(b.get_drawable_for_object(77).unwrap())
        .unwrap()
        .as_any()
        .downcast_ref::<game_client_rust::drawable::BasicDrawable>()
        .unwrap()
        .owned_ambient_audio_handle()
        .unwrap();
    a.retire_native_drawables_for_application_shutdown()
        .unwrap();
    device_a.lock().unwrap().process_request_list();
    assert_eq!(
        events_a.lock().unwrap().as_slice(),
        &[('P', handle_a), ('S', handle_a)]
    );
    assert_eq!(events_b.lock().unwrap().as_slice(), &[('P', handle_b)]);
}

#[test]
fn native_update_restarts_dropped_ambient_on_each_receiving_manager() {
    let events_a = Arc::new(Mutex::new(Vec::new()));
    let events_b = Arc::new(Mutex::new(Vec::new()));
    let device_a = Arc::new(Mutex::new(recording_audio_with_state(
        Arc::clone(&events_a),
        false,
    )));
    let device_b = Arc::new(Mutex::new(recording_audio_with_state(
        Arc::clone(&events_b),
        false,
    )));
    let mut a = client(Arc::clone(&device_a));
    let mut b = client(Arc::clone(&device_b));
    let (mut content_a, plan_a) = admitted();
    let (mut content_b, plan_b) = admitted();
    let policy = ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Night),
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    for (client, content, plan) in [
        (&mut a, &mut content_a, Arc::clone(&plan_a)),
        (&mut b, &mut content_b, Arc::clone(&plan_b)),
    ] {
        client
            .sync_presentation_drawables_observed_with_content(
                [entry(plan)],
                0,
                content.module_factory_mut(),
                policy,
                |_| {},
            )
            .unwrap();
    }
    assert_eq!(a.get_drawable_for_object(77), b.get_drawable_for_object(77));
    device_a.lock().unwrap().process_request_list();
    device_b.lock().unwrap().process_request_list();

    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    let mut locality_a = |_| locality;
    let mut locality_b = |_| locality;
    let mut ambient_a = game_client_rust::drawable::drawable::NativeAmbientUpdate::new(
        Some(game_client_rust::system::TimeOfDay::Night),
        &mut locality_a,
    );
    let mut ambient_b = game_client_rust::drawable::drawable::NativeAmbientUpdate::new(
        Some(game_client_rust::system::TimeOfDay::Night),
        &mut locality_b,
    );
    let frames = game_client_rust::core::game_client::NativeParticleFrames {
        logic_frame: 1,
        client_frame: 0,
    };
    a.update_drawables_local_with_ambient(0.0, frames, &mut ambient_a)
        .unwrap();
    b.update_drawables_local_with_ambient(0.0, frames, &mut ambient_b)
        .unwrap();
    device_a.lock().unwrap().process_request_list();
    device_b.lock().unwrap().process_request_list();

    assert_eq!(
        events_a
            .lock()
            .unwrap()
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        vec!['P', 'S', 'P']
    );
    assert_eq!(
        events_b
            .lock()
            .unwrap()
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        vec!['P', 'S', 'P']
    );

    let evening = ReceivingDrawablePolicy {
        time_of_day: Some(game_client_rust::system::TimeOfDay::Evening),
        ..policy
    };
    for (client, content, plan) in [
        (&mut a, &mut content_a, Arc::clone(&plan_a)),
        (&mut b, &mut content_b, Arc::clone(&plan_b)),
    ] {
        client
            .sync_presentation_drawables_observed_with_content(
                [entry(plan)],
                0,
                content.module_factory_mut(),
                evening,
                |_| {},
            )
            .unwrap();
    }
    let mut locality_a = |_| locality;
    let mut locality_b = |_| locality;
    let mut ambient_a = game_client_rust::drawable::drawable::NativeAmbientUpdate::new(
        evening.time_of_day,
        &mut locality_a,
    );
    let mut ambient_b = game_client_rust::drawable::drawable::NativeAmbientUpdate::new(
        evening.time_of_day,
        &mut locality_b,
    );
    a.update_drawables_local_with_ambient(0.0, frames, &mut ambient_a)
        .unwrap();
    b.update_drawables_local_with_ambient(0.0, frames, &mut ambient_b)
        .unwrap();
    device_a.lock().unwrap().process_request_list();
    device_b.lock().unwrap().process_request_list();
    assert_eq!(
        events_a
            .lock()
            .unwrap()
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        vec!['P', 'S', 'P', 'S', 'P']
    );
    assert_eq!(
        events_b
            .lock()
            .unwrap()
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<Vec<_>>(),
        vec!['P', 'S', 'P', 'S', 'P']
    );
}

#[test]
fn public_tick_retry_keeps_queued_tod_when_receiving_context_changes() {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.particle_systems_mut().finish_definition_admission();
    let (mut content, plan) = admitted();
    let mut source = entry(plan);
    source.ambient_audio_locality = None;
    let policy = ReceivingDrawablePolicy {
        time_of_day: None,
        use_draw_module_lod: true,
        static_game_lod: StaticGameLodLevel::High,
        use_tree_sway: true,
    };
    client
        .borrow_native_drawable_owner()
        .admit_semantic_drawable_with_content(
            source,
            NativeDrawableBirthContext {
                loading_map: true,
                in_load_game: false,
            },
            0,
            content.module_factory_mut(),
            policy,
            &mut NativeParticleRetirementService,
            |_| Ok(()),
        )
        .unwrap();
    let drawable_id = client.get_drawable_for_object(77).unwrap();
    client
        .find_drawable_by_id(drawable_id)
        .unwrap()
        .set_time_of_day(game_client_rust::system::TimeOfDay::Night)
        .unwrap();

    let frames = game_client_rust::core::game_client::NativeParticleFrames {
        logic_frame: 1,
        client_frame: client.get_frame(),
    };
    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    let mut locality_for_object = |_| locality;
    let mut no_device = game_client_rust::drawable::drawable::NativeAmbientUpdate::new(
        Some(game_client_rust::system::TimeOfDay::Night),
        &mut locality_for_object,
    );
    client
        .update_drawables_local_with_ambient(0.0, frames, &mut no_device)
        .unwrap();

    let played = Arc::new(Mutex::new(Vec::new()));
    let device = Arc::new(Mutex::new(tod_recording_audio(Arc::clone(&played))));
    client.bind_native_audio_device(Arc::clone(&device));
    let mut locality_for_object = |_| locality;
    let mut retry = game_client_rust::drawable::drawable::NativeAmbientUpdate::new(
        Some(game_client_rust::system::TimeOfDay::Morning),
        &mut locality_for_object,
    );
    client
        .update_drawables_local_with_ambient(0.0, frames, &mut retry)
        .unwrap();
    device.lock().unwrap().process_request_list();

    assert_eq!(
        played.lock().unwrap().as_slice(),
        &[game_engine::common::audio::audio_event_rts::TimeOfDay::Night]
    );
}

#[test]
fn compatibility_visual_can_be_admitted_without_an_ambient_template() {
    let device = Arc::new(Mutex::new(audio()));
    let mut client = client(Arc::clone(&device));
    let (_, plan) = admitted();
    let mut visual = entry(plan);
    visual.module_admission = PresentationModuleAdmission::Compatibility;
    visual.ambient_audio_locality = None;

    let (created, _, _) = client
        .sync_presentation_drawables([visual])
        .expect("a compatibility visual without an audio template remains drawable");
    assert_eq!(created, 1);
    let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(1, &Players);
    client
        .borrow_native_drawable_owner()
        .on_level_start_with_audio_at(game_client_rust::system::TimeOfDay::Night, |_| locality)
        .unwrap();
    assert_eq!(device.lock().unwrap().pending_play_request_count(), 0);
}
