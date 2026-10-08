// CPP ScriptActions.cpp2743-2764 and ScriptEngine.cpp7268-7300.
use super::*;
use gamelogic::scripting::engine::ScriptEngine;

pub(super) fn isolated(module: &str, test: &str, run: impl FnOnce()) {
    // Legacy script engine, tracker, and terrain are process-owned. Bound a
    // fresh exact-test child without clearing another test's registry/world.
    const CHILD: &str = "GENERALS_MAIN_SPEECH_TIMER_CHILD";
    let exact = format!("{}::{test}", module.split_once("::").unwrap().1);
    if std::env::var(CHILD).ok().as_deref() != Some(exact.as_str()) {
        use std::io::Read;
        use std::process::Stdio;
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
            .env(CHILD, &exact)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pipes: [Box<dyn Read + Send>; 2] = [
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        let readers = pipes.map(|mut pipe| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).unwrap();
                bytes
            })
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break (status, false);
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                break (child.wait().unwrap(), true);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let output =
            readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
        assert!(!timed_out, "Main actor child timed out: {output:?}");
        assert!(
            status.success(),
            "Main actor child failed: {status}: {output:?}"
        );
        assert!(
            output[0].contains("1 passed; 0 failed"),
            "exact child ran no test: {output:?}"
        );
        return;
    }

    run();
}

fn unknown_speech(hooks: &Arc<MissionScriptHooks>, name: &str) -> MissionScriptActionHandler {
    let event = gamelogic::common::audio::AudioEventRts::new(name);
    assert_eq!(
        gamelogic::helpers::TheAudio::get()
            .unwrap()
            .get_audio_length_ms(&event),
        0.0,
        "fresh child uses a missing sound with zero C++ length"
    );
    MissionScriptActionHandler::new(hooks.clone())
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn speech_playback_does_not_start_or_restart_the_completion_query_timer() {
    isolated(
        module_path!(),
        "speech_playback_does_not_start_or_restart_the_completion_query_timer",
        || {
            let hooks = MissionScriptHooks::new();
            let engine = ScriptEngine::new().unwrap();
            let name = "CompletionProbe_Unregistered_Speech";
            let handler = unknown_speech(&hooks, name);
            let frame = 10;
            handler.speech_play(name, false).unwrap();
            assert!(
                engine.snapshot_xfer_tail().testing_speech.is_empty(),
                "CPP speech playback never creates testingSpeech timer rows"
            );
            let frame = 90;
            assert!(engine.is_speech_complete_at_frame(name, false, frame));
            assert_eq!(
                engine
                    .snapshot_xfer_tail()
                    .testing_speech
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, f)| *f),
                Some(90)
            );
            let frame = 100;
            handler.speech_play(name, true).unwrap();
            assert_eq!(
                engine
                    .snapshot_xfer_tail()
                    .testing_speech
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, f)| *f),
                Some(90),
                "repeated playback must not restart an existing query timer"
            );
            assert!(engine.is_speech_complete_at_frame(name, true, frame));
            assert!(engine.snapshot_xfer_tail().testing_speech.is_empty());
            assert!(engine.is_speech_complete_at_frame(name, false, frame));
            assert_eq!(
                engine
                    .snapshot_xfer_tail()
                    .testing_speech
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, f)| *f),
                Some(100),
                "first query after flushing creates a fresh row at the current frame"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn speech_playback_preserves_an_existing_not_yet_complete_query_deadline() {
    isolated(
        module_path!(),
        "speech_playback_preserves_an_existing_not_yet_complete_query_deadline",
        || {
            let hooks = MissionScriptHooks::new();
            let engine = ScriptEngine::new().unwrap();
            let name = "CompletionProbe_Existing_Timer";
            let handler = unknown_speech(&hooks, name);
            let mut tail = engine.snapshot_xfer_tail();
            tail.testing_speech = vec![(name.into(), 160)];
            engine.restore_xfer_tail(&tail);
            for (frame, overlap) in [(10, false), (40, true)] {
                handler.speech_play(name, overlap).unwrap();
                assert!(
                    !engine.is_speech_complete_at_frame(name, true, frame),
                    "CPP keeps pending query until its original deadline"
                );
                assert_eq!(
                    engine
                        .snapshot_xfer_tail()
                        .testing_speech
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, f)| *f),
                    Some(160)
                );
            }
            let frame = 159;
            assert!(!engine.is_speech_complete_at_frame(name, false, frame));
            let frame = 160;
            assert!(engine.is_speech_complete_at_frame(name, true, frame));
            assert!(engine.snapshot_xfer_tail().testing_speech.is_empty());
        },
    );
}
