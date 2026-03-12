// Prototype System - Asset Management
// Ported from proto.h

use crate::render_object::{RenderObject, RenderObjectClassId};
use crate::mesh::Mesh;
use crate::mesh_model::MeshModel;
use crate::{Result, W3DError};
use std::sync::Arc;
use std::collections::HashMap;

// Prototype trait - creates render object instances
pub trait Prototype: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_class_id(&self) -> RenderObjectClassId;
    fn create(&self) -> Result<Box<dyn RenderObject>>;
}

// Primitive prototype - clones a stored object
pub struct PrimitivePrototype {
    pub proto: Arc<dyn RenderObject>,
}

impl Prototype for PrimitivePrototype {
    fn get_name(&self) -> &str {
        self.proto.get_name()
    }

    fn get_class_id(&self) -> RenderObjectClassId {
        self.proto.class_id()
    }

    fn create(&self) -> Result<Box<dyn RenderObject>> {
        // For now, create a new mesh instance
        // In a full implementation, this would clone the appropriate type
        Err(W3DError::RenderError("Cloning not fully implemented".to_string()))
    }
}

// Mesh prototype - creates mesh instances from a shared model
pub struct MeshPrototype {
    pub name: String,
    pub model: Arc<MeshModel>,
}

impl MeshPrototype {
    pub fn new(name: String, model: Arc<MeshModel>) -> Self {
        Self { name, model }
    }
}

impl Prototype for MeshPrototype {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_class_id(&self) -> RenderObjectClassId {
        RenderObjectClassId::Mesh
    }

    fn create(&self) -> Result<Box<dyn RenderObject>> {
        Ok(Box::new(Mesh::new(self.name.clone(), Arc::clone(&self.model))))
    }
}

// Asset Manager - manages prototypes and creates instances
pub struct AssetManager {
    pub prototypes: HashMap<String, Arc<dyn Prototype>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            prototypes: HashMap::new(),
        }
    }

    pub fn add_prototype(&mut self, prototype: Arc<dyn Prototype>) {
        let name = prototype.get_name().to_string();
        self.prototypes.insert(name, prototype);
    }

    pub fn get_prototype(&self, name: &str) -> Option<&Arc<dyn Prototype>> {
        self.prototypes.get(name)
    }

    pub fn create_render_object(&self, name: &str) -> Result<Box<dyn RenderObject>> {
        match self.prototypes.get(name) {
            Some(proto) => proto.create(),
            None => Err(W3DError::ResourceNotFound(name.to_string())),
        }
    }

    pub fn load_w3d_mesh(&mut self, name: String, data: &[u8]) -> Result<()> {
        use std::io::Cursor;

        let mut reader = Cursor::new(data);
        let mut model = MeshModel::new();
        model.load_w3d(&mut reader)?;

        let prototype = Arc::new(MeshPrototype::new(name.clone(), Arc::new(model)));
        self.add_prototype(prototype);

        Ok(())
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}
