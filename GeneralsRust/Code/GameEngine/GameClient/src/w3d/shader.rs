//! W3D Shader Management System

use super::{W3DConfig, W3DError, W3DResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::{Device, ShaderModule, ShaderModuleDescriptor, ShaderSource};

pub struct W3DShaderManager {
    device: Arc<Device>,
    shaders: Mutex<HashMap<String, Arc<ShaderModule>>>,
    config: W3DConfig,
}

impl W3DShaderManager {
    pub fn new(device: &Device, config: &W3DConfig) -> W3DResult<Self> {
        Ok(Self {
            device: Arc::new(device.clone()),
            shaders: Mutex::new(HashMap::new()),
            config: config.clone(),
        })
    }

    pub fn get_or_create_shader(&self, name: &str, source: &str) -> W3DResult<Arc<ShaderModule>> {
        {
            let shaders = self
                .shaders
                .lock()
                .map_err(|_| W3DError::ShaderCompilation("Shader cache lock poisoned".into()))?;
            if let Some(shader) = shaders.get(name) {
                return Ok(Arc::clone(shader));
            }
        }

        let module = Arc::new(self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some(name),
            source: ShaderSource::Wgsl(source.into()),
        }));

        let mut shaders = self
            .shaders
            .lock()
            .map_err(|_| W3DError::ShaderCompilation("Shader cache lock poisoned".into()))?;
        let entry = shaders
            .entry(name.to_string())
            .or_insert_with(|| Arc::clone(&module));
        Ok(Arc::clone(entry))
    }
}
