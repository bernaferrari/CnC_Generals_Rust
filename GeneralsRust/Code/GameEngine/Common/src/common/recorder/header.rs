//! One GENREP header decoder for active Recorder admission and immutable file listing.
use super::*;

/// Inspect exactly this file. The returned header is metadata, never a simulation
/// owner: its temporary GameInfo is cleaned for the original non-playback path.
pub fn inspect_replay_header(path: &std::path::Path) -> std::io::Result<ReplayHeader> {
    let mut file = File::open(path)?;
    let mut header = ReplayHeader::default();
    header.filename = path
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Replay path has no filename",
            )
        })?
        .to_string_lossy()
        .into_owned();
    let mut metadata = ReplayGameInfo::new();
    if !decode_replay_header(&mut file, &mut header, &mut metadata)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid replay header",
        ));
    }
    Ok(header)
}

pub(super) fn decode_replay_header<R: Read>(
    mut file: &mut R,
    header: &mut ReplayHeader,
    game_info: &mut ReplayGameInfo,
) -> std::io::Result<bool> {
    // Read GENREP magic (Recorder.cpp:804-812)
    let mut magic = [0u8; 6];
    file.read_exact(&mut magic)?;
    if &magic != b"GENREP" {
        log::error!("Replay file did not have GENREP at start");
        return Ok(false);
    }

    // Read stats (Recorder.cpp:814-825)
    let mut buf4 = [0u8; 4];
    let mut buf1 = [0u8; 1];

    header.start_time = read_fixed_width_time_t(&mut file)? as u64;
    header.end_time = read_fixed_width_time_t(&mut file)? as u64;

    file.read_exact(&mut buf4)?;
    header.frame_duration = u32::from_le_bytes(buf4);

    file.read_exact(&mut buf1)?;
    header.desync_game = buf1[0] != 0;

    file.read_exact(&mut buf1)?;
    header.quit_early = buf1[0] != 0;

    for i in 0..8 {
        file.read_exact(&mut buf1)?;
        header.player_discons[i] = buf1[0] != 0;
    }

    // Read replay name (Recorder.cpp:828)
    header.replay_name = Recorder::read_unicode_string(&mut file)?;

    // Read date/time (Recorder.cpp:831)
    header.time_val = read_system_time_from_file(&mut file)?;

    // Read version info (Recorder.cpp:833-838)
    header.version_string = Recorder::read_unicode_string(&mut file)?;
    header.version_time_string = Recorder::read_unicode_string(&mut file)?;

    file.read_exact(&mut buf4)?;
    header.version_number = u32::from_le_bytes(buf4);

    file.read_exact(&mut buf4)?;
    header.exe_crc = u32::from_le_bytes(buf4);

    file.read_exact(&mut buf4)?;
    header.ini_crc = u32::from_le_bytes(buf4);

    // Read game info (Recorder.cpp:841-852)
    header.game_options = Recorder::read_ascii_string(&mut file)?;
    game_info.reset();
    game_info.enter_game();

    if header.game_options.is_empty() {
        // Legacy rust files wrote an empty stub string; keep SP empty slots.
    } else if !parse_replay_game_info(&header.game_options, game_info) {
        log::error!("Failed to parse replay game options");
        game_info.end_game();
        game_info.reset();
        return Ok(false);
    }
    game_info.start_game(0);

    // Read player index (Recorder.cpp:854-869)
    let player_index_str = Recorder::read_ascii_string(&mut file)?;
    header.local_player_index = player_index_str.parse().unwrap_or(-1);

    if header.local_player_index < -1 || header.local_player_index >= 8 {
        log::error!("Invalid local slot number");
        game_info.end_game();
        game_info.reset();
        return Ok(false);
    }

    if header.local_player_index >= 0 {
        if let Some(slot) = game_info.get_slot(header.local_player_index as usize) {
            let local_ip = slot.get_ip();
            game_info.set_local_ip(local_ip);
        }
    }

    // If not for playback, cleanup and close
    if !header.for_playback {
        game_info.end_game();
        game_info.reset();
        return Ok(true);
    }

    Ok(true)
}
