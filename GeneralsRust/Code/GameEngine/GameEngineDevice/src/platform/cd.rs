//! C++ `FileSystem::unloadMusicFilesFromCD` / Win32CDDrive eject path.

use game_engine::common::audio::game_audio::get_global_audio_manager;
use game_engine::common::system::archive_file_system::{MUSIC_BIG, get_archive_file_system};

/// If music is playing from the CD archive, close `Music.big`.
pub fn unload_music_files_from_cd() {
    if let Some(audio) = get_global_audio_manager() {
        if let Ok(guard) = audio.lock() {
            if !guard.is_music_playing_from_cd() {
                return;
            }
        }
    }
    if let Some(mut archives) = get_archive_file_system() {
        archives.close_archive_file(MUSIC_BIG);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unload_is_safe_without_cd_music() {
        unload_music_files_from_cd();
    }
}
