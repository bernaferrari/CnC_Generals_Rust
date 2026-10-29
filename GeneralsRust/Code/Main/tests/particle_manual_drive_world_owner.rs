use generals_main::command_system::SpecialPowerType;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::{Snapshot, SnapshotBuilder, WorldSnapshot, XferLoad, XferSave};
use glam::Vec3;
use std::io::Cursor;

const SOURCE: &str = "ParticleSettingsSource";

fn world(speed: u32, fast: u32, delay: &str) -> (GameLogic, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "ParticleSettings.ini".into(),
        format!(
            "Object {SOURCE}\n KindOf = STRUCTURE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = ParticleUplinkCannonUpdate Puc\n  ManualDrivingSpeed = {speed}\n  ManualFastDrivingSpeed = {fast}\n  DoubleClickToFastDriveDelay = {delay}\n End\nEnd\n"
        ),
    )])
    .expect("typed ParticleUplinkCannonUpdate module admitted");
    game.start_new_game(GameMode::SinglePlayer);
    game.add_player(Player::new(0, Team::China, "CannonOwner", true));
    let source = game
        .create_object_for_player(SOURCE, 0, Vec3::ZERO)
        .expect("authored cannon source");
    (game, source)
}

fn assert_strike(game: &GameLogic, strike_id: u32, speed: f32, fast: f32, delay: u32) {
    let strike = game
        .special_power_strikes()
        .get(strike_id)
        .expect("queued cannon strike");
    assert_eq!(strike.manual_drive_settings.manual_driving_speed, speed);
    assert_eq!(strike.manual_drive_settings.manual_fast_driving_speed, fast);
    assert_eq!(strike.manual_drive_settings.double_click_to_fast_drive_delay, delay);
}

fn queue(game: &mut GameLogic, source: ObjectId) -> u32 {
    game.queue_special_power_strike(
        &SpecialPowerType::ParticleCannon,
        source,
        Vec3::ZERO,
    )
    .expect("Particle Cannon queued")
}

fn bincode_roundtrip(snapshot: &WorldSnapshot) -> WorldSnapshot {
    let bytes = bincode::serde::encode_to_vec(snapshot, bincode::config::legacy()).unwrap();
    let (decoded, consumed): (WorldSnapshot, usize) =
        bincode::serde::decode_from_slice(&bytes, bincode::config::legacy()).unwrap();
    assert_eq!(consumed, bytes.len());
    decoded
}

fn xfer_roundtrip(snapshot: &mut WorldSnapshot) -> WorldSnapshot {
    let mut bytes = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();
    decoded
}

fn rewrite_oxfr_v10_as_v9(bytes: &mut Vec<u8>, particle_strikes: usize, beams: usize) -> usize {
    let record = bytes
        .windows(4)
        .enumerate()
        .find_map(|(offset, window)| {
            if window != b"OXFR" || bytes.get(offset + 4..offset + 8)? != 10_u32.to_le_bytes() {
                return None;
            }
            let len = u32::from_le_bytes(bytes.get(offset + 8..offset + 12)?.try_into().ok()?) as usize;
            let payload_end = offset.checked_add(12)?.checked_add(len)?;
            let record_end = payload_end.checked_add(8)?;
            let locator_len = bytes.get(payload_end..payload_end + 4)?;
            let footer = bytes.get(payload_end + 4..record_end)?;
            (locator_len == (len as u32).to_le_bytes() && footer == b"OXFE")
                .then_some((offset, payload_end, record_end, len))
        })
        .expect("framed OXFR v10 record");
    let (start, payload_end, end, payload_len) = record;
    // The v10 additions are the final two Vecs plus their presence bool.
    let removed = 8 + 16 * particle_strikes + 8 + 16 * beams + 1;
    let new_len = payload_len.checked_sub(removed).expect("v10 sidecar suffix");
    let payload = bytes[start + 12..payload_end - removed].to_vec();
    let mut replacement = Vec::with_capacity(20 + payload.len());
    replacement.extend_from_slice(b"OXFR");
    replacement.extend_from_slice(&9_u32.to_le_bytes());
    replacement.extend_from_slice(&(new_len as u32).to_le_bytes());
    replacement.extend_from_slice(&payload);
    replacement.extend_from_slice(&(new_len as u32).to_le_bytes());
    replacement.extend_from_slice(b"OXFE");
    bytes.splice(start..end, replacement);
    removed
}

fn advance_until_beam(game: &mut GameLogic) {
    for _ in 0..120 {
        game.update();
        if !game.special_power_strikes().beam_fields().is_empty() {
            return;
        }
    }
    panic!("production fixed-step update did not resolve the queued Particle Cannon");
}

fn drive_one_step(game: &mut GameLogic, beam_id: u32) -> f32 {
    let frame = game.get_frame();
    assert!(game.special_power_strikes_mut().set_beam_override_destination(
        beam_id,
        Vec3::new(100.0, 0.0, 0.0),
        frame,
    ));
    let before = game
        .special_power_strikes()
        .beam_fields()
        .iter()
        .find(|beam| beam.id == beam_id)
        .unwrap()
        .current_target_position;
    game.update();
    let after = game
        .special_power_strikes()
        .beam_fields()
        .iter()
        .find(|beam| beam.id == beam_id)
        .unwrap()
        .current_target_position;
    after.distance(before)
}

#[test]
fn authored_settings_are_instance_owned_and_survive_queued_and_active_save_paths() {
    let (mut first, first_source) = world(90, 240, "100ms");
    let (mut peer, peer_source) = world(180, 600, "200ms");
    assert_eq!(first_source, peer_source, "worlds deliberately reuse ObjectIds");

    let first_strike = queue(&mut first, first_source);
    let peer_strike = queue(&mut peer, peer_source);
    assert_strike(&first, first_strike, 90.0, 240.0, 3);
    assert_strike(&peer, peer_strike, 180.0, 600.0, 6);

    let builder = SnapshotBuilder::new();
    let mut queued = builder.create_world_snapshot(&first).unwrap();
    assert_eq!(queued.version, 38);

    // Save before impact. Both codecs must restore the queued record's exact
    // source settings; the fixed-step route below then resolves the real strike.
    // Recreate a genuine v37/OXFR9 stream: v10's final two vectors and marker
    // are absent in the prior schema, while the fixed positional body is same.
    let mut v37_bincode = builder.create_world_snapshot(&first).unwrap();
    v37_bincode.version = 37;
    rewrite_oxfr_v10_as_v9(&mut v37_bincode.lifecycle_tail, 1, 0);
    let v37_bytes = bincode::serde::encode_to_vec(&v37_bincode, bincode::config::legacy()).unwrap();
    let (v37_decoded, v37_used): (WorldSnapshot, usize) =
        bincode::serde::decode_from_slice(&v37_bytes, bincode::config::legacy()).unwrap();
    assert_eq!(v37_used, v37_bytes.len());
    assert_eq!(v37_decoded.version, 37);
    let mut v37_world = world(90, 240, "100ms").0;
    builder.restore_from_snapshot(&v37_decoded, &mut v37_world).unwrap();
    // Historical v37 did not persist these settings and used Main's previous
    // fallback values when recreating its source/beam.
    assert_strike(&v37_world, first_strike, 20.0, 40.0, 15);

    let queued_bincode = bincode_roundtrip(&queued);
    let mut queued_bincode_world = world(90, 240, "100ms").0;
    builder
        .restore_from_snapshot(&queued_bincode, &mut queued_bincode_world)
        .unwrap();
    assert_strike(&queued_bincode_world, first_strike, 90.0, 240.0, 3);

    let queued_direct = xfer_roundtrip(&mut queued);
    assert_eq!(queued_direct.version, 38);
    let mut queued_direct_world = world(90, 240, "100ms").0;
    builder
        .restore_from_snapshot(&queued_direct, &mut queued_direct_world)
        .unwrap();
    assert_strike(&queued_direct_world, first_strike, 90.0, 240.0, 3);

    // Direct Xfer readers retain v37 support. A v37/OXFR9 stream re-saved by
    // the current writer becomes v38/OXFR10 with explicit compatibility rows.
    let mut v37_direct_bytes = Cursor::new(Vec::new());
    queued.xfer(&mut XferSave::new(&mut v37_direct_bytes)).unwrap();
    let mut v37_direct_wire = v37_direct_bytes.into_inner();
    v37_direct_wire[..4].copy_from_slice(&37_u32.to_le_bytes());
    let tail_offset = v37_direct_wire
        .windows(queued.lifecycle_tail.len())
        .position(|window| window == queued.lifecycle_tail)
        .expect("direct Xfer lifecycle tail bytes");
    let tail_length_offset = tail_offset - 4;
    let removed = rewrite_oxfr_v10_as_v9(&mut v37_direct_wire, 1, 0);
    let legacy_tail_len = (queued.lifecycle_tail.len() - removed) as u32;
    v37_direct_wire[tail_length_offset..tail_length_offset + 4]
        .copy_from_slice(&legacy_tail_len.to_le_bytes());
    let mut v37_direct = WorldSnapshot::default();
    v37_direct.xfer(&mut XferLoad::new(Cursor::new(v37_direct_wire))).unwrap();
    let mut v37_direct_world = world(90, 240, "100ms").0;
    builder.restore_from_snapshot(&v37_direct, &mut v37_direct_world).unwrap();
    assert_strike(&v37_direct_world, first_strike, 20.0, 40.0, 15);
    let mut upgraded_wire = Cursor::new(Vec::new());
    v37_direct.xfer(&mut XferSave::new(&mut upgraded_wire)).unwrap();
    assert_eq!(v37_direct.version, 38);
    let mut upgraded = WorldSnapshot::default();
    upgraded.xfer(&mut XferLoad::new(Cursor::new(upgraded_wire.into_inner()))).unwrap();
    let mut upgraded_world = world(90, 240, "100ms").0;
    builder.restore_from_snapshot(&upgraded, &mut upgraded_world).unwrap();
    assert_strike(&upgraded_world, first_strike, 20.0, 40.0, 15);

    // This is the production GameLogic fixed-step impact route, rather than a
    // test-only registry completion call.
    for game in [
        &mut first,
        &mut peer,
        &mut queued_bincode_world,
        &mut queued_direct_world,
    ] {
        advance_until_beam(game);
    }
    let first_beam = first.special_power_strikes().beam_fields()[0].id;
    let peer_beam = peer.special_power_strikes().beam_fields()[0].id;
    assert_eq!(
        first.special_power_strikes().beam_fields()[0].manual_drive_settings.manual_driving_speed,
        90.0
    );
    assert_eq!(
        peer.special_power_strikes().beam_fields()[0].manual_drive_settings.manual_driving_speed,
        180.0
    );

    // The active beam's actual manual movement uses its own captured speed.
    let first_step = drive_one_step(&mut first, first_beam);
    let peer_step = drive_one_step(&mut peer, peer_beam);
    assert!((first_step - 3.0).abs() < 0.001, "first world step: {first_step}");
    assert!((peer_step - 6.0).abs() < 0.001, "peer world step: {peer_step}");

    // A second save after impact carries the active beam sidecar via bincode
    // and direct Xfer; restored beams continue to use their captured settings.
    let mut active = builder.create_world_snapshot(&first).unwrap();
    let active_bincode = bincode_roundtrip(&active);
    let mut active_bincode_world = world(90, 240, "100ms").0;
    builder
        .restore_from_snapshot(&active_bincode, &mut active_bincode_world)
        .unwrap();
    let active_direct = xfer_roundtrip(&mut active);
    let mut active_direct_world = world(90, 240, "100ms").0;
    builder
        .restore_from_snapshot(&active_direct, &mut active_direct_world)
        .unwrap();
    assert_eq!(drive_one_step(&mut first, first_beam), drive_one_step(&mut active_bincode_world, first_beam));
    assert_eq!(drive_one_step(&mut first, first_beam), drive_one_step(&mut active_direct_world, first_beam));

    assert_eq!(peer.special_power_strikes().beam_fields()[0].manual_drive_settings.double_click_to_fast_drive_delay, 6);
}
