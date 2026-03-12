//! Control Bar Scheme System
//!
//! Rust conversion of ControlBarScheme.cpp - manages control bar visual themes and layouts

use super::{ControlBarAnimation, ControlBarLayout, ControlBarScheme};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Default control bar scheme manager implementation
#[derive(Default)]
pub struct DefaultControlBarSchemeManager {
    current_scheme: Arc<RwLock<Option<Arc<ControlBarScheme>>>>,
    loaded_schemes: Arc<RwLock<HashMap<String, Arc<ControlBarScheme>>>>,
}

impl DefaultControlBarSchemeManager {
    pub fn new() -> Self {
        Self::default()
    }
}

impl super::ControlBarSchemeManager for DefaultControlBarSchemeManager {
    fn load_scheme(&self, scheme_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Loading control bar scheme: {}", scheme_name);

        // Create default scheme for now
        let scheme = Arc::new(ControlBarScheme {
            name: scheme_name.to_string(),
            images: HashMap::new(),
            animations: HashMap::new(),
            layout: ControlBarLayout {
                command_buttons: Vec::new(),
                info_panels: Vec::new(),
                construction_queue: super::QueueLayout {
                    x: 0,
                    y: 0,
                    width: 200,
                    height: 100,
                    max_visible_items: 5,
                },
            },
        });

        self.loaded_schemes
            .write()
            .unwrap()
            .insert(scheme_name.to_string(), scheme.clone());
        *self.current_scheme.write().unwrap() = Some(scheme);

        Ok(())
    }

    fn get_scheme(&self) -> Option<Arc<ControlBarScheme>> {
        self.current_scheme.read().ok()?.clone()
    }

    fn set_scheme(&mut self, scheme: Arc<ControlBarScheme>) {
        *self.current_scheme.write().unwrap() = Some(scheme);
    }
}

/// Default control bar resizer implementation
#[derive(Default)]
pub struct DefaultControlBarResizer;

impl super::ControlBarResizer for DefaultControlBarResizer {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!("Resizing control bar to {}x{}", width, height);
        Ok(())
    }

    fn get_optimal_size(&self) -> (u32, u32) {
        (800, 150) // Default control bar size
    }
}
