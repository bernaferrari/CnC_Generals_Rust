//! Audio event description used by weapon templates.

/// Audio event description
#[derive(Debug, Clone)]
pub struct AudioEventRts {
    event_name: String,
}

impl AudioEventRts {
    pub fn new(event_name: String) -> Self {
        Self { event_name }
    }

    pub fn is_empty(&self) -> bool {
        self.event_name.is_empty()
    }

    pub fn name(&self) -> &str {
        &self.event_name
    }
}
