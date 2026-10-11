//! Record actual line command events through the current replay file codec.
use super::*;

struct ReplayFixtureFile(PathBuf);

impl Drop for ReplayFixtureFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn facing_line_command_survives_current_replay_event_stream() {
    let command = GameCommand {
        command_type: CommandType::DozerConstructLineFacing {
            template_name: "ChinaWall".into(),
            start: glam::Vec3::new(10.0, 2.0, 20.0),
            end: glam::Vec3::new(10.0, 2.0, 40.0),
            orientation: -0.75,
        },
        player_id: 7,
        command_id: 81,
        timestamp: SystemTime::UNIX_EPOCH,
        selected_units: vec![ObjectId(31), ObjectId(42)],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    };
    let mut recorder = ReplayManager::new();
    // Exercise record_command and the real file codec without invoking the
    // native recording-lobby publication unrelated to command payloads.
    recorder.mode = RecorderMode::Recording;
    recorder.current_frame = 12;
    recorder
        .record_command(&command)
        .expect("record line command");
    assert_eq!(recorder.recorded_events.len(), 1);
    assert_eq!(
        recorder.recorded_events[0].event_type,
        ReplayEventType::BuildCommand
    );

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture = ReplayFixtureFile(std::env::temp_dir().join(format!(
        "generals-line-input-{}-{nonce}.rep",
        std::process::id()
    )));
    {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&fixture.0)
            .unwrap();
        let mut writer = BufWriter::new(file);
        recorder
            .write_replay_file(&mut writer)
            .expect("write actual replay stream");
    }
    let mut reader = BufReader::new(File::open(&fixture.0).unwrap());
    let _header = recorder
        .read_replay_header(&mut reader)
        .expect("read current header");
    let events = recorder
        .read_replay_events(&mut reader)
        .expect("read current replay events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].frame, 12);
    assert_eq!(events[0].player_id, 7);
    assert_eq!(events[0].event_type, ReplayEventType::BuildCommand);
    let restored: GameCommand = bincode_legacy::deserialize(&events[0].data)
        .expect("same command decoder as playback event admission");
    assert_eq!(restored.command_type, command.command_type);
    assert_eq!(restored.player_id, command.player_id);
    assert_eq!(restored.selected_units, command.selected_units);
    assert_eq!(restored.command_id, command.command_id);
    assert_eq!(restored.timestamp, command.timestamp);
}
