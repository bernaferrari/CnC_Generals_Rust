//! Template access helpers bridging the GameLogic layer with the Common Thing system.

use crate::common::AsciiString;
use crate::helpers::TheThingFactory;
use std::sync::Arc;

/// GameLogic-facing alias for the shared engine thing template trait.
pub type ObjectTemplate = dyn crate::common::ThingTemplate;

/// Convenience alias for object template handles.
pub type ObjectTemplateHandle = Arc<ObjectTemplate>;

/// Find a template by name using the shared thing factory.
pub fn find_template(name: &AsciiString) -> Option<ObjectTemplateHandle> {
    TheThingFactory::find_template(name)
}

/// Try to find a template by its string name.
pub fn find_template_str(name: &str) -> Option<ObjectTemplateHandle> {
    let key = AsciiString::from(name);
    find_template(&key)
}

/// Resolve a template or return an error describing the missing asset.
pub fn require_template(name: &AsciiString) -> Result<ObjectTemplateHandle, String> {
    find_template(name).ok_or_else(|| format!("Missing object template: {}", name))
}
