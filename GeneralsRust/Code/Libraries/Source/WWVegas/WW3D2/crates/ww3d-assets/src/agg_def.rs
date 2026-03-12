//! Aggregate definition support mirroring `agg_def.cpp/h` from the C++ WW3D engine.
//!
//! The aggregate system allows complex render objects to be composed from a base model plus
//! additional attachments that follow specific bones. This module keeps the behaviour and
//! public surface compatible with the original implementation while delegating low level
//! rendering to the modern Rust renderer.

use crate::assets::{AssetManager, Prototype, RenderObj};
use glam::Mat4;
use std::any::Any;
use std::fmt::{self, Debug};
use ww3d_core::{W3dAggregateMiscInfo, W3dTextureReplacerStruct};

/// Runtime attachment used by aggregate definitions.
pub struct AggregateAttachment {
    pub name: String,
    pub bone_name: String,
    pub object: Option<Box<dyn RenderObj>>,
}

impl Debug for AggregateAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AggregateAttachment")
            .field("name", &self.name)
            .field("bone_name", &self.bone_name)
            .field("has_object", &self.object.is_some())
            .finish()
    }
}

impl AggregateAttachment {
    /// Attachment logical name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Bone name this attachment follows.
    pub fn bone_name(&self) -> &str {
        &self.bone_name
    }

    /// Borrow the instantiated render object, if available.
    pub fn object(&self) -> Option<&dyn RenderObj> {
        self.object.as_deref()
    }

    /// Render the attachment if an instantiated render object is available.
    pub fn render(&self) {
        if let Some(obj) = self.object.as_ref() {
            obj.render();
        }
    }

    /// Propagate a transform to the underlying render object.
    pub fn set_transform(&mut self, transform: Mat4) {
        if let Some(obj) = self.object.as_mut() {
            obj.set_transform(transform);
        }
    }

    /// Query the render object instance mutably for advanced integration scenarios.
    pub fn object_mut(&mut self) -> Option<&mut Box<dyn RenderObj>> {
        self.object.as_mut()
    }
}

/// Serialized sub-object binding captured from the W3D aggregate definition.
#[derive(Debug, Clone)]
pub struct AggregateSubobject {
    pub subobject_name: String,
    pub bone_name: String,
}

/// Prototype mirroring the C++ `AggregateDefClass`.
#[derive(Debug)]
pub struct AggregatePrototype {
    pub name: String,
    pub version: u32,
    pub base_model_name: String,
    pub subobjects: Vec<AggregateSubobject>,
    pub misc_info: W3dAggregateMiscInfo,
    pub texture_replacers: Vec<W3dTextureReplacerStruct>,
}

/// Runtime instance built from an [`AggregatePrototype`].
pub struct AggregateInstance {
    name: String,
    transform: Mat4,
    base_name: Option<String>,
    base_object: Option<Box<dyn RenderObj>>,
    attachments: Vec<AggregateAttachment>,
    _misc_info: W3dAggregateMiscInfo,
    _texture_replacers: Vec<W3dTextureReplacerStruct>,
}

impl AggregateInstance {
    fn new(
        name: String,
        base_name: Option<String>,
        base_object: Option<Box<dyn RenderObj>>,
        attachments: Vec<AggregateAttachment>,
        misc_info: W3dAggregateMiscInfo,
        texture_replacers: Vec<W3dTextureReplacerStruct>,
    ) -> Self {
        let mut instance = Self {
            name,
            transform: Mat4::IDENTITY,
            base_name,
            base_object,
            attachments,
            _misc_info: misc_info,
            _texture_replacers: texture_replacers,
        };
        instance.propagate_transform();
        instance
    }

    fn propagate_transform(&mut self) {
        let transform = self.transform;
        if let Some(base) = self.base_object.as_mut() {
            base.set_transform(transform);
        }
        for attachment in &mut self.attachments {
            attachment.set_transform(transform);
        }
    }

    /// Access the optional base render object label.
    pub fn base_name(&self) -> Option<&str> {
        self.base_name.as_deref()
    }

    /// Borrow the instantiated base render object.
    pub fn base_object(&self) -> Option<&dyn RenderObj> {
        self.base_object.as_deref()
    }

    /// Borrow the instantiated base render object mutably.
    pub fn base_object_mut(&mut self) -> Option<&mut Box<dyn RenderObj>> {
        self.base_object.as_mut()
    }

    /// Slice accessor for aggregate attachments.
    pub fn attachments(&self) -> &[AggregateAttachment] {
        &self.attachments
    }

    /// Mutable slice accessor for aggregate attachments.
    pub fn attachments_mut(&mut self) -> &mut [AggregateAttachment] {
        &mut self.attachments
    }

    /// Find an attachment by its logical name.
    pub fn attachment_by_name(&self, name: &str) -> Option<&AggregateAttachment> {
        self.attachments.iter().find(|att| att.name == name)
    }

    /// Find an attachment by its bound bone name.
    pub fn attachment_by_bone(&self, bone_name: &str) -> Option<&AggregateAttachment> {
        self.attachments
            .iter()
            .find(|att| att.bone_name == bone_name)
    }

    /// Mutable lookup by logical name.
    pub fn attachment_by_name_mut(&mut self, name: &str) -> Option<&mut AggregateAttachment> {
        self.attachments.iter_mut().find(|att| att.name == name)
    }

    /// Mutable lookup by bone name.
    pub fn attachment_by_bone_mut(&mut self, bone_name: &str) -> Option<&mut AggregateAttachment> {
        self.attachments
            .iter_mut()
            .find(|att| att.bone_name == bone_name)
    }

    /// Number of attachments carried by this instance.
    pub fn attachment_count(&self) -> usize {
        self.attachments.len()
    }

    /// Read the world transform.
    pub fn transform(&self) -> &Mat4 {
        &self.transform
    }

    /// Update the world transform and propagate it to all child render objects.
    pub fn set_world_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.propagate_transform();
    }
}

impl Debug for AggregateInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AggregateInstance")
            .field("name", &self.name)
            .field("base_name", &self.base_name)
            .field("attachment_count", &self.attachments.len())
            .finish()
    }
}

impl RenderObj for AggregateInstance {
    fn render(&self) {
        if let Some(base) = self.base_object.as_ref() {
            base.render();
        }
        for attachment in &self.attachments {
            attachment.render();
        }
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.set_world_transform(transform);
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(AggregateInstance {
            name: self.name.clone(),
            transform: self.transform,
            base_name: self.base_name.clone(),
            base_object: self.base_object.as_ref().map(|obj| obj.clone_box()),
            attachments: self
                .attachments
                .iter()
                .map(|att| AggregateAttachment {
                    name: att.name.clone(),
                    bone_name: att.bone_name.clone(),
                    object: att.object.as_ref().map(|obj| obj.clone_box()),
                })
                .collect(),
            _misc_info: self._misc_info.clone(),
            _texture_replacers: self._texture_replacers.clone(),
        })
    }
}

impl Prototype for AggregatePrototype {
    fn create_instance(&self, assets: &AssetManager) -> Option<Box<dyn RenderObj>> {
        let base_object = if self.base_model_name.is_empty() {
            None
        } else {
            let instance = assets.create_instance(&self.base_model_name);
            if instance.is_none() {
                println!(
                    "Aggregate '{}' missing base model '{}'",
                    self.name, self.base_model_name
                );
            }
            instance
        };

        let base_name = if self.base_model_name.is_empty() {
            None
        } else {
            Some(self.base_model_name.clone())
        };

        let mut attachments = Vec::with_capacity(self.subobjects.len());
        for sub in &self.subobjects {
            let object = assets.create_instance(&sub.subobject_name);
            if object.is_none() {
                println!(
                    "Aggregate '{}' missing attachment '{}'",
                    self.name, sub.subobject_name
                );
            }
            attachments.push(AggregateAttachment {
                name: sub.subobject_name.clone(),
                bone_name: sub.bone_name.clone(),
                object,
            });
        }

        let instance = AggregateInstance::new(
            self.name.clone(),
            base_name,
            base_object,
            attachments,
            self.misc_info,
            self.texture_replacers.clone(),
        );
        Some(Box::new(instance))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
