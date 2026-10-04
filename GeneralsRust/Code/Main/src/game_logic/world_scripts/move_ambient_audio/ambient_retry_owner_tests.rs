//! Real Common AudioEvent and Main Object authoring -> admission -> ordinary
//! ambient drain. Only retry pacing and its driving frame are under review.
//! The native AudioManager remains a shared compatibility boundary.
use super::*;

const EVENT: &str = "OwnedAmbientRetryPermanentLoop";
const TEMPLATE: &str = "OwnedAmbientRetryAuthoredBuilding";

#[cfg(not(target_arch = "wasm32"))]
fn isolated(test: &str, run: impl FnOnce()) {
    use std::io::Read;
    use std::process::Stdio;
    const CHILD: &str = "GENERALS_AMBIENT_RETRY_OWNER_CHILD";
    let module = module_path!().split_once("::").expect("crate prefix").1;
    let exact = format!("{module}::{test}");
    if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
        run();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn actual authored ambient owner witness");
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).expect("drain witness output");
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            child.kill().expect("kill stalled witness");
            break child.wait().expect("reap witness");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(
        !timed_out,
        "ambient witness exceeded deadline: {exact}: {output:?}"
    );
    assert!(
        status.success(),
        "ambient witness failed: {exact}: {output:?}"
    );
    assert!(
        output[0].contains("running 1 test") && output[0].contains("1 passed; 0 failed"),
        "exact witness must execute one test: {output:?}"
    );
}

#[cfg(target_arch = "wasm32")]
fn isolated(_: &str, run: impl FnOnce()) {
    run();
}

fn parse_rules() -> ThingTemplate {
    // Actual engine parser publishes actual immutable event info. No manual
    // AudioEventInfo insertion, playback hook, or pretend loaded flag.
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(
        r#"
AudioEvent OwnedAmbientRetryPermanentLoop
  Sounds = OwnedAmbientRetryLoopSample
  Volume = 80
  Control = LOOP
  LoopCount = 0
  Priority = NORMAL
End
"#,
        |ini| ini.parse_current_file(),
    )
    .expect("authored permanent AudioEvent grammar");
    let manager = game_engine::common::audio::game_audio::get_global_audio_manager()
        .expect("real parser initialized THE_AUDIO");
    {
        let guard = manager.lock().expect("real audio definitions");
        let event = guard
            .find_audio_event_info(EVENT)
            .expect("actual parsed AudioEvent");
        assert!(event.is_permanent_sound());
        assert_eq!(event.loop_count, 0);
        assert_ne!(event.control & game_engine::common::audio::AC_LOOP, 0);
        assert_eq!(event.sounds, vec!["OwnedAmbientRetryLoopSample".to_owned()]);
        assert_eq!(
            guard.get_audio_settings().drawable_ambient_frames,
            30,
            "these controls preserve the current configured 30-frame retry policy"
        );
    }
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(
                r#"
Object OwnedAmbientRetryAuthoredBuilding
  KindOf = STRUCTURE SELECTABLE
  SoundAmbient = OwnedAmbientRetryPermanentLoop
  Body = ActiveBody ModuleTag_ActualBody
    MaxHealth = 100
  End
End
"#,
                "owned_ambient_retry.ini",
            )
            .expect("actual Main Object authoring"),
        1
    );
    let definition = parser
        .get_definition(TEMPLATE)
        .expect("parsed actual Object");
    let template = GameLogic::build_template_from_object_definition(TEMPLATE, definition, None);
    assert_eq!(template.sound_ambient.as_deref(), Some(EVENT));
    assert!(template.kind_of.contains(&KindOf::Structure));
    assert_eq!(template.max_health, 100.0);
    template
}

fn create(world: &mut GameLogic, template: &ThingTemplate, position: Vec3) -> ObjectId {
    world
        .templates
        .insert(TEMPLATE.to_owned(), template.clone());
    let id = world
        .create_object(TEMPLATE, Team::USA, position)
        .expect("actual authored ambient admission");
    let object = world.objects.get(&id).expect("actual admitted Object");
    assert_eq!(object.ambient_audio.as_deref(), Some(EVENT));
    assert!(object.ambient_sound_enabled_from_script);
    assert_eq!(object.get_position(), position);
    assert_requests(
        world,
        id,
        position,
        1,
        "constructor-path SoundAmbient starts once",
    );
    id
}

fn assert_requests(world: &mut GameLogic, id: ObjectId, position: Vec3, count: usize, why: &str) {
    // Observe and consume the actual world-owned presentation request queue.
    // No private pacing state, candidate map, or fake playing state is touched.
    let requests = std::mem::take(&mut world.queued_audio_events);
    let ambient: Vec<_> = requests
        .into_iter()
        .filter(|event| {
            event.event_type == EVENT
                && event.object_id == Some(id)
                && event.is_looping
                && !event.stop
        })
        .collect();
    assert_eq!(ambient.len(), count, "{why}: {ambient:?}");
    for event in ambient {
        assert_eq!(event.position, Some(position));
        assert_eq!(event.priority, 80);
    }
}

fn drain(world: &mut GameLogic, id: ObjectId, position: Vec3, frame: u32, count: usize, why: &str) {
    world.set_current_frame(u64::from(frame));
    world.drain_pending_move_ambient_audio();
    assert_requests(world, id, position, count, why);
}

#[test]
fn authored_same_id_ambient_retries_are_owned_and_constructor_is_inactive() {
    isolated(
        "authored_same_id_ambient_retries_are_owned_and_constructor_is_inactive",
        || {
            let template = parse_rules();
            let mut first = GameLogic::new();
            let first_pos = Vec3::new(10.0, 0.0, 20.0);
            let first_id = create(&mut first, &template, first_pos);
            crate::game_logic::host_historic_bonus::set_logic_frame(100);
            drain(
                &mut first,
                first_id,
                first_pos,
                100,
                1,
                "first owner initial retry",
            );

            let mut second = GameLogic::new();
            assert!(
                second.queued_audio_events.is_empty(),
                "world construction emits no ambient request"
            );
            // Constructing another world must neither reset nor consume first's pace.
            drain(
                &mut first,
                first_id,
                first_pos,
                101,
                0,
                "constructor does not clear first retry",
            );
            let second_pos = Vec3::new(40.0, 0.0, 50.0);
            let second_id = create(&mut second, &template, second_pos);
            assert_eq!(
                first_id, second_id,
                "independent actual ID counters intentionally collide"
            );
            drain(
                &mut second,
                second_id,
                second_pos,
                100,
                1,
                "identical event and ID in a second owner still gets its initial retry",
            );
            drain(
                &mut first,
                first_id,
                first_pos,
                129,
                0,
                "first backoff remains its own",
            );
            drain(
                &mut second,
                second_id,
                second_pos,
                129,
                0,
                "second backoff remains its own",
            );
            drain(&mut first, first_id, first_pos, 130, 1, "first deadline");
            drain(
                &mut second,
                second_id,
                second_pos,
                130,
                1,
                "second independent deadline",
            );
        },
    );
}

#[test]
fn authored_ambient_retry_uses_driving_frame_not_foreign_historic_clock() {
    isolated(
        "authored_ambient_retry_uses_driving_frame_not_foreign_historic_clock",
        || {
            let template = parse_rules();
            let mut world = GameLogic::new();
            let pos = Vec3::new(20.0, 0.0, 30.0);
            let id = create(&mut world, &template, pos);
            crate::game_logic::host_historic_bonus::set_logic_frame(9000);
            drain(&mut world, id, pos, 100, 1, "first attempt");
            crate::game_logic::host_historic_bonus::set_logic_frame(9100);
            drain(
                &mut world,
                id,
                pos,
                129,
                0,
                "foreign advance cannot bypass owner backoff",
            );
            crate::game_logic::host_historic_bonus::set_logic_frame(0);
            drain(
                &mut world,
                id,
                pos,
                130,
                1,
                "foreign rewind cannot postpone owner deadline",
            );
        },
    );
}

#[test]
fn authored_ambient_reset_clears_only_driving_owner_with_reused_ids() {
    isolated(
        "authored_ambient_reset_clears_only_driving_owner_with_reused_ids",
        || {
            let template = parse_rules();
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            let a = Vec3::new(10.0, 0.0, 20.0);
            let b = Vec3::new(40.0, 0.0, 50.0);
            let aid = create(&mut first, &template, a);
            let bid = create(&mut second, &template, b);
            assert_eq!(aid, bid);
            // Matching old ambient clocks makes setup succeed before exposing
            // reset inheritance, independently of the explicit-frame regression.
            crate::game_logic::host_historic_bonus::set_logic_frame(100);
            drain(&mut first, aid, a, 100, 1, "first initial retry");
            crate::game_logic::host_historic_bonus::set_logic_frame(200);
            drain(&mut second, bid, b, 200, 1, "second initial retry");
            first.reset();
            first.set_current_frame(201);
            let recreated = create(&mut first, &template, a);
            assert_eq!(
                recreated, aid,
                "actual driving reset reuses its own ID counter"
            );
            crate::game_logic::host_historic_bonus::set_logic_frame(201);
            drain(
                &mut first,
                recreated,
                a,
                201,
                1,
                "new roster gets a fresh initial retry",
            );
            crate::game_logic::host_historic_bonus::set_logic_frame(229);
            drain(
                &mut second,
                bid,
                b,
                229,
                0,
                "other owner deadline stays 200+30",
            );
            crate::game_logic::host_historic_bonus::set_logic_frame(230);
            drain(
                &mut second,
                bid,
                b,
                230,
                1,
                "other owner is due exactly at its retained deadline",
            );
        },
    );
}

#[test]
fn authored_ambient_in_place_snapshot_restore_discards_only_transient_retry_deadline() {
    isolated(
        "authored_ambient_in_place_snapshot_restore_discards_only_transient_retry_deadline",
        || {
            let template = parse_rules();
            let mut world = GameLogic::new();
            let pos = Vec3::new(20.0, 0.0, 30.0);
            let id = create(&mut world, &template, pos);
            crate::game_logic::host_historic_bonus::set_logic_frame(100);
            drain(&mut world, id, pos, 100, 1, "pre-save initial retry");
            world.set_current_frame(101);
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder
                .create_world_snapshot(&world)
                .expect("actual world capture");
            builder
                .restore_from_snapshot(&snapshot, &mut world)
                .expect("actual in-place world restore");
            assert!(
                world.objects.contains_key(&id),
                "exact saved ObjectId restored"
            );
            // Current Main world snapshots do not automatically replay a host
            // ambient start. Exercise the real explicit start after restore;
            // this control does not claim complete CPP Drawable postload replay.
            world.start_ambient_sound(id);
            assert_requests(
                &mut world,
                id,
                pos,
                1,
                "real post-restore explicit ambient start",
            );
            crate::game_logic::host_historic_bonus::set_logic_frame(101);
            drain(
                &mut world,
                id,
                pos,
                101,
                1,
                "recreated saved ID must not inherit a pre-load presentation retry deadline",
            );
        },
    );
}
