//! Retail `m_iniCRC` spanning the GameEngine::init subsystem table.
//!
//! C++ `GameEngine.cpp:314-530` wraps every `initSubsystem` INI load in one
//! `XferCRC`. GameData-only hashing is not the MP/replay identity.

use game_engine::common::ini::{INI, INILoadType};
use game_engine::common::system::Xfer;
use game_engine::common::system::xfer_crc::XferCRC;
use game_engine::common::system::xfer_load::XferLoad;
use log::debug;
use std::io::Cursor;
use std::path::Path;

/// C++ GameEngine.cpp initSubsystem / ini.load order that feeds XferCRC.
pub const GAME_ENGINE_INI_CRC_PATHS: &[&str] = &[
    "Data/INI/Default/GameData.ini",
    "Data/INI/GameData.ini",
    "Data/INI/Default/Water.ini",
    "Data/INI/Water.ini",
    "Data/INI/Default/Weather.ini",
    "Data/INI/Weather.ini",
    "Data/INI/Default/Science.ini",
    "Data/INI/Science.ini",
    "Data/INI/Default/Multiplayer.ini",
    "Data/INI/Multiplayer.ini",
    "Data/INI/Default/Terrain.ini",
    "Data/INI/Terrain.ini",
    "Data/INI/Default/Roads.ini",
    "Data/INI/Roads.ini",
    "Data/INI/Rank.ini",
    "Data/INI/Default/PlayerTemplate.ini",
    "Data/INI/PlayerTemplate.ini",
    "Data/INI/Default/FXList.ini",
    "Data/INI/FXList.ini",
    "Data/INI/Weapon.ini",
    "Data/INI/Default/ObjectCreationList.ini",
    "Data/INI/ObjectCreationList.ini",
    "Data/INI/Locomotor.ini",
    "Data/INI/Default/SpecialPower.ini",
    "Data/INI/SpecialPower.ini",
    "Data/INI/DamageFX.ini",
    "Data/INI/Armor.ini",
    "Data/INI/Default/Object.ini",
    "Data/INI/Default/Upgrade.ini",
    "Data/INI/Upgrade.ini",
    "Data/INI/Default/AIData.ini",
    "Data/INI/AIData.ini",
    "Data/INI/Default/Crate.ini",
    "Data/INI/Crate.ini",
];

/// Hash the retail init table (plus Object directory after Default/Object.ini).
pub fn calculate_game_engine_ini_crc(load_text: impl Fn(&str) -> Option<String>) -> u32 {
    let inner = XferLoad::new(Cursor::new(Vec::new()), 1);
    let xfer_crc = XferCRC::new(inner);
    let mut ini = INI::new();
    ini.set_xfer(xfer_crc);

    for path in GAME_ENGINE_INI_CRC_PATHS {
        feed_ini_path(&mut ini, path, &load_text);
        if *path == "Data/INI/Default/Object.ini" {
            for object_path in super::object_ini_boot::collect_object_ini_virtual_paths() {
                if object_path.eq_ignore_ascii_case("Data/INI/Default/Object.ini") {
                    continue;
                }
                feed_ini_path(&mut ini, &object_path, &load_text);
            }
        }
    }

    #[cfg(any(debug_assertions, feature = "internal"))]
    {
        feed_ini_path(&mut ini, "Data/INI/GameDataDebug.ini", &load_text);
    }

    let crc = ini.take_xfer().and_then(|mutex| {
        let mut xfer_crc = mutex.into_inner().ok()?;
        xfer_crc.close().ok()?;
        Some(xfer_crc.get_crc())
    });
    ini.clear_xfer();
    crc.unwrap_or(0)
}

fn feed_ini_path(ini: &mut INI, path: &str, load_text: &impl Fn(&str) -> Option<String>) {
    if Path::new(path).exists() {
        match ini.load(path, INILoadType::Overwrite) {
            Ok(()) => debug!("XferCRC INI loaded: {path}"),
            Err(err) => debug!("XferCRC INI skipped '{path}': {err}"),
        }
        return;
    }
    let Some(text) = load_text(path) else {
        return;
    };
    match ini.with_inline_source(&text, |ini| ini.parse_current_file()) {
        Ok(()) => debug!("XferCRC INI loaded from extract: {path}"),
        Err(err) => debug!("XferCRC extract skipped '{path}': {err}"),
    }
}

/// Store the completed CRC on the runtime GlobalData view.
pub fn publish_ini_crc(crc: u32) {
    let mut data = game_engine::common::global_data::write();
    data.ini_crc = crc;
}
