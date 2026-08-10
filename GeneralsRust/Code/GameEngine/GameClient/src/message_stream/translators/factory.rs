use super::*;

pub struct TranslatorFactory {}

impl TranslatorFactory {
    pub fn new() -> Self {
        Self {}
    }

    /// Create a command translator
    pub fn create_command_translator() -> Arc<RwLock<CommandTranslator>> {
        Arc::new(RwLock::new(CommandTranslator::new()))
    }

    /// Create a selection translator  
    pub fn create_selection_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(SelectionTranslatorXlat::new()))
    }

    /// Create a window translator
    pub fn create_window_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(WindowTranslator::new()))
    }

    /// Create a meta event translator
    pub fn create_meta_event_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(MetaEventTranslator::new()))
    }

    /// Create a look-at translator
    pub fn create_look_at_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(LookAtTranslator::new()))
    }

    /// Create a hot key translator
    pub fn create_hot_key_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(HotKeyTranslator::new()))
    }

    /// Create a placement translator
    pub fn create_place_event_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(PlaceEventTranslator::new()))
    }

    /// Create a GUI command translator
    pub fn create_gui_command_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(GUICommandTranslator::new()))
    }

    /// Create a hint spy translator
    pub fn create_hint_spy() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(HintSpy::new()))
    }

    /// Create the standard set of translators with appropriate priorities
    pub fn create_standard_translator_set() -> Vec<(Arc<RwLock<dyn GameMessageTranslator>>, u32)> {
        let command_translator: Arc<RwLock<dyn GameMessageTranslator>> =
            Self::create_command_translator();

        vec![
            (Self::create_window_translator(), 10), // Window input handling
            (Self::create_meta_event_translator(), 20), // Meta key remapping
            (Self::create_hot_key_translator(), 25), // UI hotkeys
            (Self::create_place_event_translator(), 30), // Placement handling
            (Self::create_gui_command_translator(), 40), // UI commands
            (Self::create_selection_translator(), 50), // Selection handling
            (Self::create_look_at_translator(), 60), // Camera movement
            (command_translator, 70),               // Command processing
            (Self::create_hint_spy(), 100),         // Hints and feedback
        ]
    }
}

impl Default for TranslatorFactory {
    fn default() -> Self {
        Self::new()
    }
}

