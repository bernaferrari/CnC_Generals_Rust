//! Browser output and already-fetched authored audio files.

use super::*;
use std::collections::HashSet;

/// An application opens one output synchronously from its browser input callback.
#[derive(Clone)]
pub struct BrowserAudioOutput {
    pub(super) device: OutputStreamHandle,
}

impl BrowserAudioOutput {
    /// Call directly from a trusted click or key callback, before yielding.
    pub fn open_from_user_gesture() -> Result<Self, String> {
        let (_stream, device) = OutputStream::try_default()
            .map_err(|error| format!("Cannot open browser audio: {error}"))?;
        // The handle retains the CPAL stream even after the constructor returns.
        Ok(Self { device })
    }
}

/// A receiving manager's immutable, preloaded virtual audio files.
#[derive(Debug)]
pub struct BrowserAudioContent {
    language: String,
    files: HashMap<String, Arc<Vec<u8>>>,
}

impl BrowserAudioContent {
    pub fn new(language: String, files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            language,
            files: files
                .into_iter()
                .map(|(name, bytes)| (virtual_key(&name), Arc::new(bytes)))
                .collect(),
        }
    }

    pub(in crate::common::audio) fn language(&self) -> &str {
        &self.language
    }

    pub(in crate::common::audio) fn file(&self, filename: &str) -> Option<Arc<Vec<u8>>> {
        self.files.get(&virtual_key(filename)).cloned()
    }
}

fn virtual_key(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
        .to_ascii_lowercase()
}

pub(super) struct BrowserPlayback {
    pub(super) output: BrowserAudioOutput,
    pub(super) content: Arc<BrowserAudioContent>,
}

impl AudioManager {
    /// Build an inert receiving manager around a browser application's open output.
    /// The host service submits explicit locality and audio selection, and advances
    /// requests through `update_with_view` with its own view observation.
    pub fn with_browser_audio(
        output: &BrowserAudioOutput,
        content: Arc<BrowserAudioContent>,
    ) -> Self {
        let mut manager =
            Self::with_playback_hook(Box::new(RodioPlaybackHook::with_browser(BrowserPlayback {
                output: output.clone(),
                content: Arc::clone(&content),
            })));
        manager.browser_audio_content = Some(content);
        manager
    }

    pub(super) fn log_browser_play_failure(&mut self, event: &str, reason: &str) {
        if self.browser_play_failures.insert(event.to_owned()) {
            log::warn!("Browser audio play failed {event}: {reason}");
        }
    }
}

pub(super) type FailureLog = HashSet<String>;
