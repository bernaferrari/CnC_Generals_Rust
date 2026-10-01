// Subsystem manager lifecycle.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Manages subsystem lifecycle and dependencies
pub struct SubsystemManager {
    display: Option<Arc<Mutex<GraphicsDisplay>>>,
    audio: Option<Arc<Mutex<AudioSubsystem>>>,
    input_keyboard: Option<crate::input::Keyboard>,
    // Mouse lives in the shared `THE_MOUSE` singleton (crate::input::mouse),
    // not a per-GameClient handle: Main's OS intake and the client tick share it.
    terrain_visual: Option<Arc<Mutex<TerrainVisualStub>>>,
    window_manager: Option<WindowManagerSubsystem>,
    font_library: Option<FontLibrarySubsystem>,
    header_templates: Option<HeaderTemplateManagerSubsystem>,
    display_strings: Option<DisplayStringManagerSubsystem>,
    hot_key_manager: Option<HotKeyManagerSubsystem>,
    in_game_ui: Option<Arc<Mutex<InGameUISubsystem>>>,
    video_player: Option<VideoPlayerSubsystem>,
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

        if let Some(ref mut keyboard) = self.input_keyboard {
            keyboard.reset()?;
        }

        // Shared THE_MOUSE (Main OS inject + client tick); rule-d boundary.
        crate::input::mouse::the_mouse()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .reset()?;

        if let Some(ref terrain) = self.terrain_visual {
            terrain.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }
        // C++ GameClient::reset does not reset TheWindowManager (GameClient.cpp:426-457).
        // Destroying root windows here wipes MainMenu.wnd on GAME_SHELL apply (hq-3vo4).

        if let Some(font_library) = &mut self.font_library {
            font_library.reset()?;
        }

        if let Some(header_templates) = &mut self.header_templates {
            header_templates.reset()?;
        }

        if let Some(display_strings) = &mut self.display_strings {
            display_strings.reset()?;
        }

        if let Some(hot_keys) = &mut self.hot_key_manager {
            hot_keys.reset()?;
        }

        if let Some(ref ui) = self.in_game_ui {
            ui.lock().unwrap_or_else(|e| e.into_inner()).reset()?;
        }

        if let Some(video) = &mut self.video_player {
            video.reset()?;
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
