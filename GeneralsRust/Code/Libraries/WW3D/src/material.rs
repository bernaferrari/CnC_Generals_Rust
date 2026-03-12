// Material System
// Ported from vertmaterial.h, matinfo.h

use crate::w3d_file::*;
use crate::texture::Texture;
use crate::shader::Shader;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct VertexMaterial {
    pub name: String,
    pub attributes: u32,
    pub ambient: [u8; 3],
    pub diffuse: [u8; 3],
    pub specular: [u8; 3],
    pub emissive: [u8; 3],
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

impl Default for VertexMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            attributes: 0,
            ambient: [255, 255, 255],
            diffuse: [255, 255, 255],
            specular: [0, 0, 0],
            emissive: [0, 0, 0],
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }
}

#[derive(Clone)]
pub struct MaterialPass {
    pub vertex_materials: Vec<Option<Arc<VertexMaterial>>>,
    pub shaders: Vec<Shader>,
    pub textures: Vec<Vec<Option<Arc<Texture>>>>, // [stage][texture]
    pub uv_coords: Vec<Vec<[f32; 2]>>, // [stage][vertex]
    pub vertex_colors: Option<Vec<[u8; 4]>>,
}

impl MaterialPass {
    pub fn new() -> Self {
        Self {
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
            textures: Vec::new(),
            uv_coords: Vec::new(),
            vertex_colors: None,
        }
    }
}

impl Default for MaterialPass {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MaterialInfo {
    pub passes: Vec<MaterialPass>,
}

impl MaterialInfo {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
        }
    }

    pub fn get_pass_count(&self) -> usize {
        self.passes.len()
    }

    pub fn add_pass(&mut self, pass: MaterialPass) {
        self.passes.push(pass);
    }
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self::new()
    }
}
