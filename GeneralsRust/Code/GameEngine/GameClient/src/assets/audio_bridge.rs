//! Audio bridge between the legacy GameAudio system and the Rust AssetManager.

use crate::assets::{AssetManager, AssetPriority};
use game_engine::common::audio::{
    register_sound_playback_hook, AudioEventRts, AudioHandle, AudioType, SoundPlaybackHook,
};
use nalgebra::Vector3;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::runtime::{Builder, Handle, Runtime};

pub fn register_audio_playback_bridge(asset_manager: Arc<AssetManager>) {
    let hook = Arc::new(AssetAudioPlaybackHook::new(asset_manager));
    if !register_sound_playback_hook(hook) {
        log::warn!("Audio playback hook already registered; ignoring duplicate");
    }
}

struct AssetAudioPlaybackHook {
    asset_manager: Arc<AssetManager>,
    handle_map: Arc<Mutex<HashMap<AudioHandle, Option<u64>>>>,
    runtime: Arc<Runtime>,
}

impl AssetAudioPlaybackHook {
    fn new(asset_manager: Arc<AssetManager>) -> Self {
        let runtime = Arc::new(
            Builder::new_multi_thread()
                .enable_all()
                .thread_name("audio-playback")
                .build()
                .expect("Failed to create audio playback runtime"),
        );
        Self {
            asset_manager,
            handle_map: Arc::new(Mutex::new(HashMap::new())),
            runtime,
        }
    }

    fn spawn(&self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        if let Ok(handle) = Handle::try_current() {
            handle.spawn(task);
        } else {
            self.runtime.spawn(task);
        }
    }

    fn resolve_paths(&self, event: &AudioEventRts) -> Vec<PathBuf> {
        let audio_type = event
            .get_audio_event_info()
            .map(|info| info.sound_type)
            .unwrap_or(AudioType::SoundEffect);
        let prefix = event.generate_filename_prefix(audio_type, false);
        let extension = event.generate_filename_extension(audio_type);

        let mut paths = Vec::new();
        for candidate in event.sound_candidates() {
            let trimmed = candidate.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains('/') || trimmed.contains('\\') {
                paths.push(self.normalize_audio_path(trimmed));
            } else if trimmed.contains('.') {
                paths.push(self.normalize_audio_path(&format!("{prefix}{trimmed}")));
            } else {
                paths.push(self.normalize_audio_path(&format!("{prefix}{trimmed}{extension}")));
            }
        }

        if paths.is_empty() {
            if let Some(filename) = event.resolve_filename() {
                paths.push(self.normalize_audio_path(&filename));
            }
        }

        paths
    }

    fn normalize_audio_path(&self, raw: &str) -> PathBuf {
        let trimmed = raw.trim().trim_matches('"');
        let normalized = trimmed.replace('\\', "/");
        let mut path = PathBuf::from(normalized);

        if path.is_absolute() {
            return path;
        }

        let base = self.asset_manager.base_path();
        if let Some(base_name) = base.file_name().and_then(|name| name.to_str()) {
            if base_name.eq_ignore_ascii_case("data") {
                let mut iter = path.iter();
                if let Some(first) = iter.next() {
                    if first.to_string_lossy().eq_ignore_ascii_case("data") {
                        path = iter.collect();
                    }
                }
            }
        }

        base.join(path)
    }

    fn resolve_position(event: &AudioEventRts) -> Option<Vector3<f32>> {
        if event.is_positional() || event.object_id != 0 || event.drawable_id != 0 {
            let pos = event.get_position();
            Some(Vector3::new(pos.x, pos.y, pos.z))
        } else {
            None
        }
    }
}

impl SoundPlaybackHook for AssetAudioPlaybackHook {
    fn play(&self, event: &AudioEventRts) -> Result<(), String> {
        let handle = event.get_playing_handle();
        if let Ok(mut map) = self.handle_map.lock() {
            map.insert(handle, None);
        }

        let paths = self.resolve_paths(event);
        if paths.is_empty() {
            if let Ok(mut map) = self.handle_map.lock() {
                map.remove(&handle);
            }
            return Err("No audio paths resolved".to_string());
        }
        let volume = if event.get_volume() >= 0.0 {
            Some(event.get_volume())
        } else {
            None
        };
        let pitch = if event.pitch_shift > 0.0 {
            Some(event.pitch_shift)
        } else {
            None
        };
        let position = Self::resolve_position(event);

        let asset_manager = Arc::clone(&self.asset_manager);
        let handle_map = Arc::clone(&self.handle_map);

        self.spawn(async move {
            for path in paths {
                let handle_result = asset_manager.load_asset(&path, AssetPriority::Normal).await;
                let asset_handle = match handle_result {
                    Ok(handle) => handle,
                    Err(err) => {
                        log::debug!("Audio asset load failed for {}: {err}", path.display());
                        continue;
                    }
                };

                match asset_manager
                    .play_audio_asset(asset_handle, volume, pitch, position)
                    .await
                {
                    Ok(instance_id) => {
                        if let Ok(mut map) = handle_map.lock() {
                            map.insert(handle, Some(instance_id));
                        }
                        return;
                    }
                    Err(err) => {
                        log::debug!("Audio playback failed for {}: {err}", path.display());
                    }
                }
            }

            if let Ok(mut map) = handle_map.lock() {
                map.remove(&handle);
            }
        });

        Ok(())
    }

    fn stop(&self, handle: AudioHandle) {
        let instance_id = {
            let Ok(mut map) = self.handle_map.lock() else {
                return;
            };
            map.remove(&handle)
        };

        let Some(instance_id) = instance_id.flatten() else {
            return;
        };

        let asset_manager = Arc::clone(&self.asset_manager);
        self.spawn(async move {
            let _ = asset_manager.stop_audio_instance(instance_id);
        });
    }

    fn pause(&self, handle: AudioHandle) {
        // AssetManager does not yet expose per-instance pause; stop as a compatibility fallback.
        self.stop(handle);
    }

    fn is_playing(&self, handle: AudioHandle) -> bool {
        let state = {
            let Ok(map) = self.handle_map.lock() else {
                return false;
            };
            map.get(&handle).cloned()
        };

        let Some(state) = state else {
            return false;
        };

        let Some(instance_id) = state else {
            return true;
        };

        let playing = self.asset_manager.is_audio_instance_playing(instance_id);
        if !playing {
            if let Ok(mut map) = self.handle_map.lock() {
                map.remove(&handle);
            }
        }
        playing
    }
}
