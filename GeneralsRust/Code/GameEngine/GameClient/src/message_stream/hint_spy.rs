pub use crate::message_stream::translators::HintSpy;

pub fn create_hint_spy() -> HintSpy {
    HintSpy::new()
}
