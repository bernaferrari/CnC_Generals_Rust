//! Lookup through the active template catalog, including its legacy startup fallback.

use std::sync::Arc;

use crate::common::ascii_string::AsciiString;

use super::thing_factory::{get_thing_factory, init_thing_factory};
use super::thing_template::ThingTemplate;

/// Load the startup catalog on a miss only if the active catalog has no templates.
pub fn find_template_or_initialize(name: &str) -> Option<Arc<ThingTemplate>> {
    let key = AsciiString::from(name);
    let needs_initialization = {
        let guard = get_thing_factory().ok();
        let factory = guard.as_ref().and_then(|guard| guard.as_ref());
        if let Some(template) =
            factory.and_then(|factory| factory.find_template(key.as_str(), false))
        {
            return Some(template);
        }
        factory.is_none_or(|factory| factory.first_template().is_none())
    };

    if needs_initialization && init_thing_factory().is_ok() {
        let guard = get_thing_factory().ok()?;
        return guard.as_ref()?.find_template(key.as_str(), false);
    }
    None
}
