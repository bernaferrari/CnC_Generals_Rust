pub use crate::message_stream::translators::GUICommandTranslator;

pub fn create_gui_command_translator() -> GUICommandTranslator {
    GUICommandTranslator::new()
}
