//! Immediate menu sound operations on the receiving native service.
//! Handles retain their service lifetime; queued audio requests are a separate API.
use std::sync::{Arc, Weak};

#[derive(Debug, Clone)]
pub struct MenuAudioHandle {
    raw: u32,
    owner: Weak<()>,
}

/// Identity for one actual menu-audio service generation, without mutable sharing.
#[derive(Debug, Default)]
pub struct MenuAudioLifetime(Arc<()>);
impl MenuAudioLifetime {
    pub fn handle(&self, raw: u32) -> MenuAudioHandle {
        MenuAudioHandle {
            raw,
            owner: Arc::downgrade(&self.0),
        }
    }
    pub fn resolve(&self, handle: &MenuAudioHandle) -> Result<u32, String> {
        if !handle.owner.ptr_eq(&Arc::downgrade(&self.0)) {
            return Err("menu audio handle belongs to another service lifetime".into());
        }
        Ok(handle.raw)
    }
}

impl PartialEq for MenuAudioHandle {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.owner.ptr_eq(&other.owner)
    }
}
impl Eq for MenuAudioHandle {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAudioStop {
    Stopped,
    AlreadyEnded,
    NotOwned,
}

pub trait MenuAudioEffects {
    /// Returns the actual device handle; muted/NoSound returns None.
    fn is_bound(&self) -> bool;
    fn play(&mut self, name: &str) -> Option<MenuAudioHandle>;
    fn play_fading(&mut self, name: &str) -> Option<MenuAudioHandle>;
    fn stop(&mut self, handle: MenuAudioHandle) -> MenuAudioStop;
    fn pump(&mut self);
    fn fade_out_music(&mut self);
}

/// Deliberate device-less service for actual headless clients and fixtures.
/// Missing service is a checked admission error, never implicitly Silent.
#[derive(Debug, Default)]
pub struct SilentMenuAudio;
impl SilentMenuAudio {
    pub fn new() -> Self {
        Self
    }
}
impl MenuAudioEffects for SilentMenuAudio {
    fn is_bound(&self) -> bool {
        true
    }
    fn play(&mut self, _: &str) -> Option<MenuAudioHandle> {
        None
    }
    fn play_fading(&mut self, _: &str) -> Option<MenuAudioHandle> {
        None
    }
    fn stop(&mut self, _: MenuAudioHandle) -> MenuAudioStop {
        MenuAudioStop::NotOwned
    }
    fn pump(&mut self) {}
    fn fade_out_music(&mut self) {}
}
