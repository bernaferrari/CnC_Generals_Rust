// Subsystem manager lifecycle.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Manages subsystem lifecycle and dependencies
pub struct SubsystemManager {
    display: Option<Arc<Mutex<GraphicsDisplay>>>,
    audio: Option<Arc<Mutex<AudioSubsystem>>>,
    input_keyboard: Option<KeyboardHandle>,
    input_mouse: Option<MouseHandle>,
    terrain_visual: Option<Arc<Mutex<TerrainVisualStub>>>,
    window_manager: Option<Arc<Mutex<WindowManagerSubsystem>>>,
    font_library: Option<Arc<Mutex<FontLibrarySubsystem>>>,
    header_templates: Option<Arc<Mutex<HeaderTemplateManagerSubsystem>>>,
    display_strings: Option<Arc<Mutex<DisplayStringManagerSubsystem>>>,
    hot_key_manager: Option<Arc<Mutex<HotKeyManagerSubsystem>>>,
    in_game_ui: Option<Arc<Mutex<InGameUISubsystem>>>,
    video_player: Option<Arc<Mutex<VideoPlayerSubsystem>>>,
    decal_manager: Option<Arc<Mutex<DecalManager>>>,
    asset_manager: Option<Arc<AssetManager>>,
    platform_context: Option<PlatformContext>,
}

// Subsystem manager implementation

impl SubsystemManager {
    fn new() -> Self {
        Self {
            display: None,
            audio: None,
            input_keyboard: None,
            input_mouse: None,
            terrain_visual: None,
            window_manager: None,
            font_library: None,
            header_templates: None,
            display_strings: None,
            hot_key_manager: None,
            in_game_ui: None,
            video_player: None,
            decal_manager: None,
            asset_manager: None,
            platform_context: None,
        }
    }

    fn reset_all(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.display {
            display.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref audio) = self.audio {
            audio.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref keyboard) = self.input_keyboard {
            keyboard.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref mouse) = self.input_mouse {
            mouse.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref terrain) = self.terrain_visual {
            terrain.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref window_manager) = self.window_manager {
            window_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reset()?;
        }

        if let Some(ref font_library) = self.font_library {
            font_library
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reset()?;
        }

        if let Some(ref header_templates) = self.header_templates {
            header_templates
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reset()?;
        }

        if let Some(ref display_strings) = self.display_strings {
            display_strings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .reset()?;
        }

        if let Some(ref hot_keys) = self.hot_key_manager {
            hot_keys.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref ui) = self.in_game_ui {
            ui.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref video) = self.video_player {
            video.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(ref decals) = self.decal_manager {
            if let Ok(mut guard) = decals.lock() {
                guard.clear_all();
            }
        }

        crate::eva::reset_eva_system();

        Ok(())
    }
}
