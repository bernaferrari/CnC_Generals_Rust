#![allow(non_snake_case)]
//! High-level WWAudio wrapper closely mirroring the original C++ `WWAudioClass` API.

use std::io::{Read, Seek, Write};
use std::sync::{Arc, Mutex, OnceLock};

use crate::{
    device::{
        AudioPreferences, EndOfStreamCallback, LogicalEvent, ProviderInfo, TextEventCallback,
    },
    error::Error as AudioError,
    formats::{AudioFormat, SampleRate, SampleWidth},
    logical::list::LogicalSoundRegistry,
    logical_listener::LogicalListener,
    logical_sound::LogicalSound,
    save_load::{DynamicAudioSaveLoad, StaticAudioSaveLoad},
    sound_scene::SoundScene,
    sound_scene_obj::SoundObjectId,
    thread_pool::{global_thread_pool, queue_delayed_release},
    wwaudio_handles::WWHandle,
    AudioResult, AudioSystem, AudioSystemConfig, Driver2DKind, MixerEvent, Priority,
    VoiceStopReason,
};

const PLAYLIST_RELEASE_DELAY_MS: u64 = 2000;

/// Legacy 2D driver identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DriverType2D {
    Error = 0,
    Dsound,
    Waveout,
    Count,
}

impl DriverType2D {
    pub fn from_raw(value: u32) -> Self {
        match value {
            0 => Self::Error,
            1 => Self::Dsound,
            2 => Self::Waveout,
            _ => Self::Error,
        }
    }
}

impl DriverType3D {
    pub fn from_raw(value: u32) -> Self {
        match value {
            0 => Self::Error,
            1 => Self::D3dSound,
            2 => Self::Eax,
            3 => Self::A3d,
            4 => Self::Rsx,
            5 => Self::Pseudo,
            6 => Self::Dolby,
            _ => Self::Error,
        }
    }
}

/// Legacy 3D driver identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DriverType3D {
    Error = 0,
    D3dSound,
    Eax,
    A3d,
    Rsx,
    Pseudo,
    Dolby,
    Count,
}

/// Struct mirroring the C++ `DRIVER_INFO_STRUCT`
#[derive(Debug, Clone)]
pub struct DriverInfo {
    pub provider: Option<ProviderInfo>,
    pub name: String,
    pub driver_type: DriverType3D,
}

impl Default for DriverInfo {
    fn default() -> Self {
        Self {
            provider: None,
            name: String::new(),
            driver_type: DriverType3D::Error,
        }
    }
}

#[derive(Clone)]
struct PlaylistEntry {
    handle: WWHandle,
    priority: f32,
}

/// Rust port of the original `WWAudioClass`
pub struct WWAudioClass {
    audio_system: Option<AudioSystem>,
    sound_scene: SoundScene,
    driver_2d: Option<Driver2DKind>,
    driver_3d_index: u32,
    playlist: Vec<PlaylistEntry>,
    completed_playlist: Vec<WWHandle>,
    static_save: StaticAudioSaveLoad,
    dynamic_save: DynamicAudioSaveLoad,
    music_volume: f32,
    sound_volume: f32,
    reverb_level: f32,
    reverb_room_type: i32,
    is_music_enabled: bool,
    are_sfx_enabled: bool,
    pending_logical_events: Vec<LogicalEvent>,
    logical_registry: LogicalSoundRegistry,
    registered_eos_callbacks: Vec<(EndOfStreamCallback, u32)>,
    registered_text_callbacks: Vec<TextEventCallback>,
}

impl Default for WWAudioClass {
    fn default() -> Self {
        Self {
            audio_system: None,
            sound_scene: SoundScene::new(),
            driver_2d: None,
            driver_3d_index: 0,
            playlist: Vec::new(),
            completed_playlist: Vec::new(),
            static_save: StaticAudioSaveLoad::default(),
            dynamic_save: DynamicAudioSaveLoad::default(),
            music_volume: crate::DEFAULT_VOLUME as f32 / crate::MAX_VOLUME as f32,
            sound_volume: crate::DEFAULT_VOLUME as f32 / crate::MAX_VOLUME as f32,
            reverb_level: 0.0,
            reverb_room_type: 0,
            is_music_enabled: true,
            are_sfx_enabled: true,
            pending_logical_events: Vec::new(),
            logical_registry: LogicalSoundRegistry::new(),
            registered_eos_callbacks: Vec::new(),
            registered_text_callbacks: Vec::new(),
        }
    }
}

static WW_AUDIO_INSTANCE: OnceLock<Mutex<WWAudioClass>> = OnceLock::new();

impl WWAudioClass {
    fn queue_handle_release(handle: WWHandle) {
        queue_delayed_release(PLAYLIST_RELEASE_DELAY_MS, move || {
            drop(handle);
        });
    }

    fn apply_cached_state_to_system(&mut self, system: &mut AudioSystem) {
        for (callback, user_param) in &self.registered_eos_callbacks {
            system.register_eos_callback(callback.clone(), *user_param);
        }
        for callback in &self.registered_text_callbacks {
            system.register_text_callback(callback.clone());
        }
        for registration in self.logical_registry.iter() {
            system.register_logical_sound_factory_entry(
                registration.sound_id,
                registration.type_mask,
                registration.display.clone(),
            );
        }
    }

    /// Retrieve the singleton instance (equivalent to `Get_Instance`)
    pub fn instance() -> &'static Mutex<WWAudioClass> {
        WW_AUDIO_INSTANCE.get_or_init(|| Mutex::new(WWAudioClass::default()))
    }

    /// Alias matching the legacy static accessor name
    pub fn Get_Instance() -> &'static Mutex<WWAudioClass> {
        Self::instance()
    }

    /// Create a new WWAudio wrapper without touching the singleton
    pub fn new() -> Self {
        Self::default()
    }

    /// Port of `WWAudioClass::Initialize(bool stereo, int bits, int hertz)`
    pub async fn Initialize(&mut self, stereo: bool, bits: i32, hertz: i32) -> AudioResult<()> {
        self.static_save = StaticAudioSaveLoad::default();
        self.dynamic_save = DynamicAudioSaveLoad::default();

        let mut config = AudioSystemConfig::default();
        config.default_format = build_format(stereo, bits, hertz);
        config.default_sound_volume = self.sound_volume;
        config.default_music_volume = self.music_volume;
        config.sound_effects_enabled = self.are_sfx_enabled;
        config.music_enabled = self.is_music_enabled;
        config.default_reverb_level = self.reverb_level;
        config.default_reverb_room_type = self.reverb_room_type;

        let mut system = AudioSystem::new_with_config(config).await?;
        let _ = global_thread_pool();
        let default_format = system.configuration().default_format;
        system.open_2d_device_with_format(&default_format)?;
        self.driver_2d = Some(Driver2DKind::Unknown);
        system.build_driver_list();
        self.driver_3d_index = 0;

        self.apply_cached_state_to_system(&mut system);
        self.sound_scene = system.sound_scene().clone();
        self.audio_system = Some(system);
        Ok(())
    }

    /// Port of `WWAudioClass::Initialize(const char *registry_subkey_name)`
    pub async fn Initialize_With_Preferences(&mut self, key: &str) -> AudioResult<()> {
        self.Initialize(true, 16, 44100).await?;
        if let Some(system) = self.audio_system.as_mut() {
            system.load_preferences_with_key(key);
        }
        Ok(())
    }

    /// Port of `WWAudioClass::Shutdown`
    pub async fn Shutdown(&mut self) {
        if let Some(system) = self.audio_system.as_mut() {
            system.shutdown().await;
        }
        self.sound_scene.flush_scene();
        self.audio_system = None;
    }

    /// Port of `Open_2D_Device(bool stereo, int bits, int hertz)`
    pub fn Open_2D_Device(&mut self, stereo: bool, bits: i32, hertz: i32) -> DriverType2D {
        if let Some(system) = self.audio_system.as_mut() {
            let format = build_format(stereo, bits, hertz);
            if system.open_2d_device_with_format(&format).is_ok() {
                return DriverType2D::Dsound;
            }
        }
        DriverType2D::Error
    }

    /// Port of `Close_2D_Device`
    pub fn Close_2D_Device(&mut self) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            system.close_2d_device();
            true
        } else {
            false
        }
    }

    /// Playback sample rate accessor (matches `Get_Playback_Rate`)
    pub fn Get_Playback_Rate(&self) -> i32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.playback_rate() as i32)
            .unwrap_or(0)
    }

    pub fn Get_Playback_Bits(&self) -> i32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.playback_bits() as i32)
            .unwrap_or(0)
    }

    pub fn Get_Playback_Stereo(&self) -> bool {
        self.audio_system
            .as_ref()
            .map(|sys| sys.playback_is_stereo())
            .unwrap_or(false)
    }

    /// Port of `Get_3D_Device_Count`
    pub fn Get_3D_Device_Count(&self) -> usize {
        self.audio_system
            .as_ref()
            .map(|sys| sys.get_provider_count() as usize)
            .unwrap_or(0)
    }

    /// Port of `Get_3D_Device`
    pub fn Get_3D_Device(&self, index: usize) -> Option<DriverInfo> {
        self.audio_system
            .as_ref()
            .and_then(|sys| sys.provider_info(index as u32))
            .cloned()
            .map(|info| DriverInfo {
                name: info.name.clone(),
                driver_type: DriverType3D::from_raw(info.driver_type),
                provider: Some(info),
            })
    }

    /// Port of `Select_3D_Device(int index)`
    pub fn Is_3D_Device_Available(&self, device_type: DriverType3D) -> bool {
        self.Find_3D_Device(device_type).is_some()
    }

    pub fn Find_3D_Device(&self, device_type: DriverType3D) -> Option<u32> {
        self.audio_system
            .as_ref()
            .and_then(|sys| sys.find_3d_provider_by_type(device_type))
    }

    pub fn Get_Current_3D_Device(&self) -> Option<DriverInfo> {
        self.audio_system
            .as_ref()
            .and_then(|sys| sys.provider_info(sys.selected_provider_index()))
            .cloned()
            .map(|info| DriverInfo {
                name: info.name.clone(),
                driver_type: DriverType3D::from_raw(info.driver_type),
                provider: Some(info),
            })
    }

    pub fn Get_Last_3D_Device(&self) -> Option<DriverInfo> {
        self.audio_system
            .as_ref()
            .and_then(|sys| sys.provider_info(sys.last_provider_index()))
            .cloned()
            .map(|info| DriverInfo {
                name: info.name.clone(),
                driver_type: DriverType3D::from_raw(info.driver_type),
                provider: Some(info),
            })
    }

    pub fn Select_3D_Device(&mut self, index: u32) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            system.select_3d_provider(index);
            self.driver_3d_index = index;
            true
        } else {
            false
        }
    }

    pub fn Select_3D_Device_By_Type(&mut self, device_type: DriverType3D) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            if system.select_3d_provider_by_type(device_type) {
                if let Some(index) = system.find_3d_provider_by_type(device_type) {
                    self.driver_3d_index = index;
                }
                return true;
            }
        }
        false
    }

    /// Convenience overload that selects by provider name
    pub fn Select_3D_Device_By_Name(&mut self, name: &str) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            if let Some(idx) = system.find_3d_provider_by_name(name) {
                system.select_3d_provider(idx);
                self.driver_3d_index = idx;
                return true;
            }
        }
        false
    }

    /// Port of `Set_File_Factory`
    pub fn Set_File_Factory(&mut self, factory: Arc<dyn crate::AudioFileFactory>) {
        if let Some(system) = self.audio_system.as_mut() {
            system.set_file_factory(factory);
        }
    }

    /// Port of `Set_Max_2D_Sample_Count`
    pub fn Set_Max_2D_Sample_Count(&mut self, count: u32) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            system.set_max_2d_sample_count(count);
            true
        } else {
            false
        }
    }

    pub fn Get_Max_2D_Sample_Count(&self) -> u32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.max_2d_sample_count())
            .unwrap_or(0)
    }

    pub fn Get_Avail_2D_Sample_Count(&self) -> u32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.available_2d_sample_count())
            .unwrap_or(0)
    }

    pub fn Set_Max_3D_Sample_Count(&mut self, count: u32) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            system.set_max_3d_sample_count(count);
            true
        } else {
            false
        }
    }

    pub fn Get_Max_3D_Sample_Count(&self) -> u32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.max_3d_sample_count())
            .unwrap_or(0)
    }

    pub fn Get_Avail_3D_Sample_Count(&self) -> u32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.available_3d_sample_count())
            .unwrap_or(0)
    }

    pub fn Set_Sound_Effects_Volume(&mut self, volume: f32) {
        self.sound_volume = volume;
        if let Some(system) = self.audio_system.as_mut() {
            system.set_sound_effects_volume(volume);
        }
    }

    pub fn Get_Sound_Effects_Volume(&self) -> f32 {
        self.sound_volume
    }

    pub fn Set_Music_Volume(&mut self, volume: f32) {
        self.music_volume = volume;
        if let Some(system) = self.audio_system.as_mut() {
            system.set_music_volume(volume);
        }
    }

    pub fn Get_Music_Volume(&self) -> f32 {
        self.music_volume
    }

    pub fn Allow_Sound_Effects(&mut self, on: bool) {
        self.are_sfx_enabled = on;
        if let Some(system) = self.audio_system.as_mut() {
            system.allow_sound_effects(on);
        }
    }

    pub fn Are_Sound_Effects_On(&self) -> bool {
        self.are_sfx_enabled
    }

    pub fn Allow_Music(&mut self, on: bool) {
        self.is_music_enabled = on;
        if let Some(system) = self.audio_system.as_mut() {
            system.allow_music(on);
        }
    }

    pub fn Is_Music_On(&self) -> bool {
        self.is_music_enabled
    }

    /// Port of `On_Frame_Update`
    pub async fn On_Frame_Update(&mut self, milliseconds: u32) -> AudioResult<()> {
        if let Some(system) = self.audio_system.as_mut() {
            system.on_frame_update(milliseconds).await?;
            let mixer_events = system.drain_mixer_events();
            let logical_events = system.drain_logical_events();
            let scene_snapshot = system.sound_scene().clone();
            self.handle_mixer_events(mixer_events);
            self.handle_logical_events(logical_events);
            self.sound_scene = scene_snapshot;
        }
        Ok(())
    }

    pub fn Drain_Logical_Events(&mut self) -> Vec<LogicalEvent> {
        self.pending_logical_events.drain(..).collect()
    }

    fn handle_mixer_events(&mut self, events: Vec<MixerEvent>) {
        let mut playlist_dirty = false;

        for event in events {
            if let MixerEvent::VoiceStopped {
                descriptor, reason, ..
            } = event
            {
                if let Some(handle_id) = descriptor.handle_id {
                    playlist_dirty |= self.mark_playlist_completion(handle_id, reason);
                }
            }
        }

        if playlist_dirty {
            self.update_playlist_priorities();
        }
    }

    fn handle_logical_events(&mut self, events: Vec<LogicalEvent>) {
        if !events.is_empty() {
            self.pending_logical_events.extend(events);
        }
    }

    fn mark_playlist_completion(&mut self, handle_id: u32, reason: VoiceStopReason) -> bool {
        let retain = matches!(reason, VoiceStopReason::Completed);

        if let Some(handle) = self.remove_playlist_entry(handle_id) {
            if retain {
                if !self
                    .completed_playlist
                    .iter()
                    .any(|entry| entry.id() == Some(handle_id))
                {
                    self.completed_playlist.push(handle);
                } else {
                    Self::queue_handle_release(handle);
                }
            } else {
                Self::queue_handle_release(handle);
            }
            true
        } else {
            false
        }
    }

    fn remove_playlist_entry(&mut self, handle_id: u32) -> Option<WWHandle> {
        if let Some(index) = self
            .playlist
            .iter()
            .position(|entry| entry.handle.id() == Some(handle_id))
        {
            let entry = self.playlist.remove(index);
            Some(entry.handle)
        } else {
            None
        }
    }

    /// Port of `Register_EOS_Callback`
    pub fn Register_EOS_Callback(&mut self, callback: EndOfStreamCallback, user_param: u32) {
        let mut already_registered = false;
        for (existing, stored_param) in &mut self.registered_eos_callbacks {
            if Arc::ptr_eq(existing, &callback) {
                already_registered = true;
                *stored_param = user_param;
                break;
            }
        }
        if !already_registered {
            self.registered_eos_callbacks
                .push((callback.clone(), user_param));
        }

        if let Some(system) = self.audio_system.as_mut() {
            if already_registered {
                system.unregister_eos_callback(&callback);
            }
            system.register_eos_callback(callback, user_param);
        }
    }

    pub fn UnRegister_EOS_Callback(&mut self, callback: &EndOfStreamCallback) {
        self.registered_eos_callbacks
            .retain(|(existing, _)| !Arc::ptr_eq(existing, callback));

        if let Some(system) = self.audio_system.as_mut() {
            system.unregister_eos_callback(callback);
        }
    }

    pub fn Register_Text_Callback(&mut self, callback: TextEventCallback) {
        let already_registered = self
            .registered_text_callbacks
            .iter()
            .any(|existing| Arc::ptr_eq(existing, &callback));
        if !already_registered {
            self.registered_text_callbacks.push(callback.clone());
        }

        if let Some(system) = self.audio_system.as_mut() {
            if already_registered {
                system.unregister_text_callback(&callback);
            }
            system.register_text_callback(callback);
        }
    }

    pub fn UnRegister_Text_Callback(&mut self, callback: &TextEventCallback) {
        self.registered_text_callbacks
            .retain(|existing| !Arc::ptr_eq(existing, callback));

        if let Some(system) = self.audio_system.as_mut() {
            system.unregister_text_callback(callback);
        }
    }

    pub fn Fire_Text_Callback(&self, text: &str) {
        if let Some(system) = self.audio_system.as_ref() {
            system.fire_text_event(text);
        } else {
            for callback in &self.registered_text_callbacks {
                callback(text);
            }
        }
    }

    /// Port of `Is_Sound_Cached`
    pub async fn Is_Sound_Cached(&self, identifier: &str) -> AudioResult<bool> {
        if let Some(system) = self.audio_system.as_ref() {
            system.is_sound_cached(identifier).await
        } else {
            Ok(false)
        }
    }

    /// Port of `Create_Sound_Effect(const char *filename)`
    pub async fn Create_Sound_Effect(&mut self, filename: &str) -> AudioResult<WWHandle> {
        if let Some(system) = self.audio_system.as_mut() {
            let handle = system.create_audible_sound(filename).await?;
            self.sound_scene = system.sound_scene().clone();
            Ok(handle)
        } else {
            Err(crate::AudioError::Audio(
                "Audio system not initialized".into(),
            ))
        }
    }

    /// Port of `Create_3D_Sound(const char *filename, int classid_hint)`
    pub async fn Create_3D_Sound(&mut self, filename: &str) -> AudioResult<WWHandle> {
        if let Some(system) = self.audio_system.as_mut() {
            let handle = system.create_3d_sound(filename).await?;
            self.sound_scene = system.sound_scene().clone();
            Ok(handle)
        } else {
            Err(crate::AudioError::Audio(
                "Audio system not initialized".into(),
            ))
        }
    }

    pub async fn Create_Pseudo3D_Sound(&mut self, filename: &str) -> AudioResult<WWHandle> {
        if let Some(system) = self.audio_system.as_mut() {
            let handle = system.create_pseudo3d_sound(filename).await?;
            self.sound_scene = system.sound_scene().clone();
            Ok(handle)
        } else {
            Err(crate::AudioError::Audio(
                "Audio system not initialized".into(),
            ))
        }
    }

    /// Port of `Create_Logical_Sound`
    pub fn Create_Logical_Sound(&mut self) -> LogicalSound {
        let id = self.sound_scene.allocate_id();
        let mut sound = LogicalSound::new(id);
        if let Some(system) = self.audio_system.as_ref() {
            if let Some(entry) = system.logical_factory_entry(id) {
                sound.set_type_mask(entry.type_mask);
            }
            if let Some(registration) = system.logical_registration(id) {
                if let Some(name) = registration.display.as_ref() {
                    self.Fire_Text_Callback(name);
                }
            }
        } else if let Some(registration) = self.logical_registry.lookup(id) {
            sound.set_type_mask(registration.type_mask);
            if let Some(name) = registration.display.as_ref() {
                self.Fire_Text_Callback(name);
            }
        }
        self.sound_scene.add_logical_sound(sound.clone());
        sound
    }

    pub fn Create_Logical_Listener(&mut self) -> LogicalListener {
        let id = self.sound_scene.allocate_id();
        let listener = LogicalListener::new(id);
        self.sound_scene.add_logical_listener(listener.clone());
        listener
    }

    pub fn Add_Logical_Type(&mut self, id: i32, display_name: &str) {
        if let Some(system) = self.audio_system.as_mut() {
            system.add_logical_type(id, display_name);
        }
    }

    pub fn Reset_Logical_Types(&mut self) {
        if let Some(system) = self.audio_system.as_mut() {
            system.reset_logical_types();
            system.clear_logical_sound_registry();
        }
        self.logical_registry.clear();
    }

    pub fn Get_Logical_Type_Count(&self) -> usize {
        self.audio_system
            .as_ref()
            .map(|system| system.logical_type_count())
            .unwrap_or(0)
    }

    pub fn Get_Logical_Type(&self, index: usize) -> Option<(i32, String)> {
        self.audio_system
            .as_ref()
            .and_then(|system| system.logical_type(index))
            .map(|record| (record.type_id, record.display_name.clone()))
    }

    pub fn Register_Logical_Sound_Definition(
        &mut self,
        sound_id: SoundObjectId,
        type_mask: u32,
        display_name: Option<&str>,
    ) {
        if let Some(system) = self.audio_system.as_mut() {
            system.register_logical_sound_factory_entry(
                sound_id,
                type_mask,
                display_name.map(|s| s.to_string()),
            );
        }
        self.logical_registry
            .register(sound_id, type_mask, display_name.map(|s| s.to_string()));
    }

    pub fn Clear_Logical_Sound_Definitions(&mut self) {
        if let Some(system) = self.audio_system.as_mut() {
            system.clear_logical_sound_registry();
        }
        self.logical_registry.clear();
    }

    pub fn Get_Sound_Scene(&self) -> &SoundScene {
        &self.sound_scene
    }

    pub fn Get_Sound_Scene_Mut(&mut self) -> &mut SoundScene {
        &mut self.sound_scene
    }

    pub fn Set_Cache_Size(&mut self, kilobytes: usize) {
        if let Some(system) = self.audio_system.as_mut() {
            system.set_cache_size_kb(kilobytes);
        }
    }

    pub fn Get_Cache_Size(&self) -> usize {
        self.audio_system
            .as_ref()
            .map(|system| system.cache_size_kb())
            .unwrap_or(0)
    }

    pub fn Get_Current_Cache_Size(&self) -> usize {
        self.audio_system
            .as_ref()
            .map(|system| system.current_cache_usage())
            .unwrap_or(0)
    }

    pub async fn Flush_Cache(&self) -> AudioResult<()> {
        if let Some(system) = self.audio_system.as_ref() {
            system.flush_cache().await
        } else {
            Ok(())
        }
    }

    pub async fn Simple_Play_2D_Sound_Effect(
        &mut self,
        filename: &str,
        priority: f32,
        volume: f32,
    ) -> AudioResult<()> {
        let priority = priority_to_enum(priority);
        let volume = volume.clamp(0.0, 1.0);
        if let Some(system) = self.audio_system.as_mut() {
            system.set_sound_effects_volume(volume);
            system.simple_play_2d_sound_effect(filename, priority).await
        } else {
            Err(crate::AudioError::Audio(
                "Audio system not initialized".into(),
            ))
        }
    }

    fn playlist_priority(&self, handle: &WWHandle) -> f32 {
        handle
            .id()
            .and_then(|id| self.sound_scene.find_sound(id))
            .map(|sound| sound.priority_value())
            .unwrap_or(0.0)
    }

    fn update_playlist_priorities(&mut self) {
        let priorities: Vec<f32> = self
            .playlist
            .iter()
            .map(|entry| self.playlist_priority(&entry.handle))
            .collect();

        for (entry, priority) in self.playlist.iter_mut().zip(priorities) {
            entry.priority = priority;
        }

        self.playlist.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn Add_To_Playlist(&mut self, handle: &WWHandle) -> bool {
        let Some(id) = handle.id() else {
            return false;
        };

        if self
            .playlist
            .iter()
            .any(|entry| entry.handle.id() == Some(id))
        {
            return false;
        }

        let cloned = handle.clone();
        let priority = self.playlist_priority(&cloned);
        self.playlist.push(PlaylistEntry {
            handle: cloned,
            priority,
        });
        self.update_playlist_priorities();
        true
    }

    pub fn Remove_From_Playlist(&mut self, handle: &WWHandle) -> bool {
        let Some(id) = handle.id() else {
            return false;
        };

        if self
            .playlist
            .iter()
            .any(|entry| entry.handle.id() == Some(id))
        {
            if self
                .completed_playlist
                .iter()
                .any(|entry| entry.id() == Some(id))
            {
                return true;
            }
            self.completed_playlist.push(handle.clone());
            true
        } else {
            false
        }
    }

    pub fn Get_Playlist_Count(&self) -> usize {
        self.playlist.len()
    }

    pub fn Get_Playlist_Entry(&self, index: usize) -> Option<WWHandle> {
        self.playlist.get(index).map(|entry| entry.handle.clone())
    }

    pub fn Peek_Playlist_Entry(&self, index: usize) -> Option<WWHandle> {
        self.Get_Playlist_Entry(index)
    }

    pub fn Flush_Playlist(&mut self) {
        for entry in self.playlist.drain(..) {
            Self::queue_handle_release(entry.handle);
        }
        for handle in self.completed_playlist.drain(..) {
            Self::queue_handle_release(handle);
        }
        if let Some(system) = self.audio_system.as_mut() {
            system.free_completed_sounds();
        }
    }

    pub fn Is_Sound_In_Playlist(&self, handle: &WWHandle) -> bool {
        handle
            .id()
            .map(|id| {
                self.playlist
                    .iter()
                    .any(|entry| entry.handle.id() == Some(id))
            })
            .unwrap_or(false)
    }

    pub fn Reprioritize_Playlist(&mut self) {
        self.update_playlist_priorities();

        let mut best_index: Option<usize> = None;
        let mut highest_priority = f32::MIN;

        for (index, entry) in self.playlist.iter().enumerate() {
            if entry.handle.miles_handle().is_some() {
                continue;
            }

            let is_culled = entry
                .handle
                .id()
                .and_then(|id| self.sound_scene.find_sound(id))
                .map(|sound| sound.is_culled())
                .unwrap_or(false);

            if is_culled {
                continue;
            }

            if entry.priority > highest_priority {
                highest_priority = entry.priority;
                best_index = Some(index);
            }
        }

        if let Some(index) = best_index {
            let handle = self.playlist[index].handle.clone();
            let _ = handle.start();
        }
    }

    pub fn Get_Digital_CPU_Percent(&self) -> f32 {
        self.audio_system
            .as_ref()
            .map(|sys| sys.digital_cpu_percent())
            .unwrap_or(0.0)
    }

    pub fn Is_Disabled(&self) -> bool {
        self.audio_system
            .as_ref()
            .map(|sys| sys.is_disabled())
            .unwrap_or(true)
    }

    pub fn Free_Completed_Sounds(&mut self) {
        if !self.completed_playlist.is_empty() {
            for completed in self.completed_playlist.drain(..) {
                let completed_id = completed.id();

                if let Some(id) = completed_id {
                    if let Some(index) = self
                        .playlist
                        .iter()
                        .position(|entry| entry.handle.id() == Some(id))
                    {
                        let entry = self.playlist.remove(index);
                        Self::queue_handle_release(entry.handle);
                    }
                }

                Self::queue_handle_release(completed);
            }
            self.update_playlist_priorities();
        }

        if let Some(system) = self.audio_system.as_mut() {
            system.free_completed_sounds();
        }
    }

    pub fn Save_To_Registry(&mut self, subkey_name: &str) -> bool {
        if let Some(system) = self.audio_system.as_ref() {
            system.save_preferences_with_key(subkey_name)
        } else {
            let prefs = self.preferences_snapshot();
            AudioSystem::save_preferences_snapshot(subkey_name, &prefs)
        }
    }

    pub fn Save_To_Registry_Detailed(
        &mut self,
        subkey_name: &str,
        device_name: Option<&str>,
        is_stereo: bool,
        bits: u16,
        hertz: u32,
        sound_enabled: bool,
        music_enabled: bool,
        sound_volume: f32,
        music_volume: f32,
        preferred_2d_driver: Option<&str>,
        preferred_3d_provider: Option<&str>,
    ) -> bool {
        let prefs = AudioPreferences {
            device_name: device_name.map(|s| s.to_owned()),
            preferred_2d_driver: preferred_2d_driver.map(|s| s.to_owned()),
            preferred_3d_provider: preferred_3d_provider.map(|s| s.to_owned()),
            stereo: is_stereo,
            bits,
            hertz,
            sound_enabled,
            music_enabled,
            sound_volume,
            music_volume,
        };
        if let Some(system) = self.audio_system.as_ref() {
            system.save_preferences_explicit(subkey_name, &prefs)
        } else {
            AudioSystem::save_preferences_snapshot(subkey_name, &prefs)
        }
    }

    pub fn Load_From_Registry(&mut self, subkey_name: &str) -> bool {
        if let Some(system) = self.audio_system.as_mut() {
            if system.load_preferences_with_key(subkey_name) {
                self.sound_scene = system.sound_scene().clone();
                let prefs = system.preferences_snapshot();
                self.apply_preferences_to_state(&prefs);
                true
            } else {
                false
            }
        } else if let Some(prefs) = AudioSystem::load_preferences_snapshot(subkey_name) {
            self.apply_preferences_to_state(&prefs);
            true
        } else {
            false
        }
    }

    pub fn Load_From_Registry_Detailed(&mut self, subkey_name: &str) -> Option<AudioPreferences> {
        if let Some(system) = self.audio_system.as_mut() {
            if system.load_preferences_with_key(subkey_name) {
                self.sound_scene = system.sound_scene().clone();
                let prefs = system.preferences_snapshot();
                self.apply_preferences_to_state(&prefs);
                Some(prefs)
            } else {
                None
            }
        } else if let Some(prefs) = AudioSystem::load_preferences_snapshot(subkey_name) {
            self.apply_preferences_to_state(&prefs);
            Some(prefs)
        } else {
            None
        }
    }

    pub fn Save_Static<W: Write + Seek>(&mut self, writer: W) -> AudioResult<()> {
        if let Some(system) = self.audio_system.as_mut() {
            system.save_static_state(writer).map_err(AudioError::from)
        } else {
            self.static_save
                .save(&self.sound_scene, writer)
                .map_err(AudioError::from)
        }
    }

    pub fn Save_Dynamic<W: Write + Seek>(&mut self, writer: W) -> AudioResult<()> {
        if let Some(system) = self.audio_system.as_mut() {
            system.save_dynamic_state(writer).map_err(AudioError::from)
        } else {
            self.dynamic_save
                .save(&self.sound_scene, &self.logical_registry, &[], writer)
                .map_err(AudioError::from)
        }
    }

    pub fn Load_Static<R: Read + Seek>(&mut self, reader: R) -> AudioResult<()> {
        if let Some(system) = self.audio_system.as_mut() {
            system.load_static_state(reader).map_err(AudioError::from)?;
            self.sound_scene = system.sound_scene().clone();
            Ok(())
        } else {
            self.static_save.load(reader).map_err(AudioError::from)?;
            self.apply_static_records_from_local();
            Ok(())
        }
    }

    pub fn Load_Dynamic<R: Read + Seek>(&mut self, reader: R) -> AudioResult<()> {
        if let Some(system) = self.audio_system.as_mut() {
            system
                .load_dynamic_state(reader)
                .map_err(AudioError::from)?;
            self.sound_scene = system.sound_scene().clone();
            system.queue_pending_voice_restores();
            Ok(())
        } else {
            self.dynamic_save.load(reader).map_err(AudioError::from)?;
            self.apply_dynamic_records_from_local();
            Ok(())
        }
    }

    fn apply_static_records_from_local(&mut self) {
        self.sound_scene.static_sounds.clear();
        for record in self.static_save.loaded_sounds() {
            let sound = record.instantiate();
            self.sound_scene.add_static_sound(sound);
        }
    }

    fn apply_dynamic_records_from_local(&mut self) {
        self.sound_scene.dynamic_sounds.clear();
        for record in self.dynamic_save.loaded_dynamic_sounds() {
            let sound = record.instantiate();
            self.sound_scene.add_dynamic_sound(sound);
        }
        self.sound_scene.logical_sounds.clear();
        self.logical_registry.clear();
        for record in self.dynamic_save.loaded_logical_sounds() {
            let logical = record.instantiate();
            self.sound_scene.add_logical_sound(logical);
            self.logical_registry.register(
                record.id,
                record.type_mask,
                record.display_name.clone(),
            );
        }
        LogicalListener::set_global_scale(self.dynamic_save.logical_listener_global_scale());
        self.sound_scene.update(0);
        let system = self
            .audio_system
            .as_mut()
            .map(|sys| sys as *mut AudioSystem)
            .and_then(|ptr| unsafe { ptr.as_mut() });
        if let Some(system) = system {
            system.queue_pending_voice_restores();
        }
    }

    pub fn Set_Handle_Miles_Id(&mut self, handle: &mut WWHandle, id: u32) {
        handle.set_miles_handle(id);
    }

    pub fn Handle_Miles_Id(&self, handle: &WWHandle) -> Option<u32> {
        handle.miles_handle()
    }

    pub fn Release_Handle(&mut self, handle: WWHandle) {
        if let Some(id) = handle.id() {
            if let Some(system) = self.audio_system.as_mut() {
                system.sound_scene_mut().remove_sound(id);
                system.recalculate_sample_counts();
                self.sound_scene = system.sound_scene().clone();
            } else {
                self.sound_scene.remove_sound(id);
            }
        }

        handle.stop();
        queue_delayed_release(PLAYLIST_RELEASE_DELAY_MS, move || {
            drop(handle);
        });
    }

    fn preferences_snapshot(&self) -> AudioPreferences {
        if let Some(system) = self.audio_system.as_ref() {
            system.preferences_snapshot()
        } else {
            AudioPreferences {
                device_name: None,
                preferred_3d_provider: None,
                preferred_2d_driver: None,
                stereo: true,
                bits: 16,
                hertz: 44_100,
                sound_enabled: self.are_sfx_enabled,
                music_enabled: self.is_music_enabled,
                sound_volume: self.sound_volume,
                music_volume: self.music_volume,
            }
        }
    }

    fn apply_preferences_to_state(&mut self, prefs: &AudioPreferences) {
        self.sound_volume = prefs.sound_volume;
        self.music_volume = prefs.music_volume;
        self.are_sfx_enabled = prefs.sound_enabled;
        self.is_music_enabled = prefs.music_enabled;
    }

    pub fn Queue_Delayed_Release<F>(&self, delay_ms: u64, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        queue_delayed_release(delay_ms, task);
    }
}

fn build_format(stereo: bool, bits: i32, hertz: i32) -> AudioFormat {
    let channels = if stereo { 2 } else { 1 };
    let sample_width = match bits {
        8 => SampleWidth::U8,
        24 => SampleWidth::S24,
        32 => SampleWidth::S32,
        _ => SampleWidth::S16,
    };
    let sample_rate = match hertz {
        8000 => SampleRate::Hz8000,
        11025 => SampleRate::Hz11025,
        16000 => SampleRate::Hz16000,
        22050 => SampleRate::Hz22050,
        44100 => SampleRate::Hz44100,
        48000 => SampleRate::Hz48000,
        96000 => SampleRate::Hz96000,
        192000 => SampleRate::Hz192000,
        _ => SampleRate::Hz44100,
    };
    AudioFormat {
        channels,
        sample_rate,
        sample_width,
        channel_layout: if channels == 1 {
            crate::formats::ChannelLayout::Mono
        } else {
            crate::formats::ChannelLayout::Stereo
        },
    }
}

fn priority_to_enum(priority: f32) -> Priority {
    if priority >= 1.0 {
        Priority::Critical
    } else if priority >= 0.75 {
        Priority::High
    } else if priority >= 0.25 {
        Priority::Normal
    } else {
        Priority::Low
    }
}
