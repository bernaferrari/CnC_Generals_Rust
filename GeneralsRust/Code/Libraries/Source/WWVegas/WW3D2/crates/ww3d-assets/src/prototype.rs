// Copyright 2025 - Rust port of C&C Generals Zero Hour W3D Asset Management System
//
// This module implements the Prototype system from proto.h/proto.cpp
// Original C++ files:
// - proto.h (lines 85-126): PrototypeClass interface
// - proto.cpp (lines 60-86): PrimitivePrototypeClass implementation
//
// The prototype system is a factory pattern for efficient render object instancing.
// It allows thousands of game objects to share underlying geometry/data while having
// independent transforms and state.

use std::any::Any;
use std::sync::Arc;
use ww3d_core::RenderObjClassId;

use crate::assets::RenderObj;

/// Core Prototype trait - mirrors C++ PrototypeClass (proto.h:85-109)
///
/// This trait defines the interface for render object prototypes. Prototypes are
/// abstract factories that create render object instances. The asset manager stores
/// these and uses them whenever the user wants to create an instance of a named
/// render object.
///
/// # C++ Reference
/// From proto.h lines 74-83:
/// ```cpp
/// /*
/// ** PrototypeClass
/// ** This class is a generic interface to a render object prototype.
/// ** The asset manager will store a these and use them whenever the
/// ** user wants to create an instance of a named render object.
/// ** Some simple render objects will be created through cloning. In
/// ** that case, their associated prototype simply stores an object and
/// ** clones it whenever the Create method is called.
/// */
/// ```
///
/// # Design Philosophy
/// - Simple objects (meshes): Clone from template
/// - Complex objects (HModels): Construct from blueprint
/// - Associates a name with a render object creation function
pub trait Prototype: Send + Sync + std::fmt::Debug {
    /// Get the name of this prototype
    /// C++ equivalent: `Get_Name()` (proto.h:92)
    fn get_name(&self) -> &str;

    /// Get the render object class ID
    /// C++ equivalent: `Get_Class_ID()` (proto.h:93)
    fn get_class_id(&self) -> RenderObjClassId;

    /// Create a new instance of the render object
    /// C++ equivalent: `Create()` (proto.h:94)
    ///
    /// For simple objects, this clones a template. For complex objects,
    /// this constructs from a blueprint.
    fn create(&self) -> Box<dyn RenderObj>;

    /// Get the source asset file path if available
    fn get_asset_file(&self) -> Option<&str> {
        None
    }

    /// Downcast to Any for type introspection
    fn as_any(&self) -> &dyn Any;
}

/// Primitive prototype for simple render objects that can be cloned
/// Mirrors C++ PrimitivePrototypeClass (proto.h:111-126, proto.cpp:60-86)
///
/// This prototype type stores a template render object and clones it whenever
/// an instance is requested. This is the most common prototype type for simple
/// meshes and geometry.
///
/// # C++ Implementation Details
/// From proto.cpp:60-86:
/// ```cpp
/// PrimitivePrototypeClass::PrimitivePrototypeClass(RenderObjClass * proto)
/// {
///     Proto = proto;
///     assert(Proto);
///     Proto->Add_Ref();
/// }
///
/// RenderObjClass * PrimitivePrototypeClass::Create(void)
/// {
///     return (RenderObjClass *)( SET_REF_OWNER( Proto->Clone() ) );
/// }
/// ```
///
/// # Rust Adaptation
/// - Uses Arc<dyn RenderObj> instead of reference counting pointers
/// - Template is shared immutably across all instances
/// - Cloning creates a new instance with independent state
#[derive(Debug)]
pub struct PrimitivePrototype {
    /// Name of the prototype
    name: String,

    /// Render object class ID
    class_id: RenderObjClassId,

    /// Template object to clone from
    /// Uses Arc for efficient sharing across multiple prototype holders
    template: Arc<dyn RenderObj>,

    /// Optional source asset file path
    asset_file: Option<String>,
}

impl PrimitivePrototype {
    /// Create a new primitive prototype with a template object
    ///
    /// # Arguments
    /// * `template` - The template render object to clone from
    ///
    /// # C++ Reference
    /// Mirrors proto.cpp:60-65
    pub fn new(template: Arc<dyn RenderObj>) -> Self {
        let name = template.get_name().to_string();
        let class_id = Self::infer_class_id(&*template);

        Self {
            name,
            class_id,
            template,
            asset_file: None,
        }
    }

    /// Create a new primitive prototype with an explicit name and class ID
    pub fn new_with_name(
        name: String,
        class_id: RenderObjClassId,
        template: Arc<dyn RenderObj>,
    ) -> Self {
        Self {
            name,
            class_id,
            template,
            asset_file: None,
        }
    }

    /// Set the asset file path
    pub fn with_asset_file(mut self, asset_file: String) -> Self {
        self.asset_file = Some(asset_file);
        self
    }

    /// Infer the class ID from the render object type
    fn infer_class_id(_obj: &dyn RenderObj) -> RenderObjClassId {
        // Try to downcast to known types to determine class ID
        // This is a simplified version; the full implementation would check all types

        // Default to Unknown if we can't determine the type
        RenderObjClassId::Unknown
    }

    /// Get a reference to the template object
    pub fn template(&self) -> &Arc<dyn RenderObj> {
        &self.template
    }
}

impl Prototype for PrimitivePrototype {
    /// Returns the name of the prototype
    /// C++: `const char * PrimitivePrototypeClass::Get_Name(void) const` (proto.cpp:73-76)
    fn get_name(&self) -> &str {
        &self.name
    }

    /// Returns the render object class ID
    /// C++: `int PrimitivePrototypeClass::Get_Class_ID(void) const` (proto.cpp:78-81)
    fn get_class_id(&self) -> RenderObjClassId {
        self.class_id
    }

    /// Creates a new instance by cloning the template
    /// C++: `RenderObjClass * PrimitivePrototypeClass::Create(void)` (proto.cpp:83-86)
    ///
    /// # Implementation Notes
    /// The C++ version calls `Proto->Clone()` and wraps it with SET_REF_OWNER.
    /// In Rust, we achieve the same effect by calling clone_box() which creates
    /// a new Box<dyn RenderObj> with independent state but potentially shared
    /// underlying geometry data.
    fn create(&self) -> Box<dyn RenderObj> {
        self.template.clone_box()
    }

    fn get_asset_file(&self) -> Option<&str> {
        self.asset_file.as_deref()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Extension trait for RenderObj to support cloning
///
/// This trait must be implemented by all RenderObj types to enable
/// prototype-based instancing.
pub trait CloneRenderObj {
    /// Clone this render object into a new boxed instance
    fn clone_box(&self) -> Box<dyn RenderObj>;
}

// Blanket implementation for all RenderObj that are Clone
impl<T> CloneRenderObj for T
where
    T: RenderObj + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn RenderObj> {
        Box::new(self.clone())
    }
}

/// Builder for creating prototypes with various configurations
pub struct PrototypeBuilder {
    name: Option<String>,
    class_id: Option<RenderObjClassId>,
    asset_file: Option<String>,
}

impl PrototypeBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            class_id: None,
            asset_file: None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn class_id(mut self, class_id: RenderObjClassId) -> Self {
        self.class_id = Some(class_id);
        self
    }

    pub fn asset_file(mut self, asset_file: String) -> Self {
        self.asset_file = Some(asset_file);
        self
    }

    pub fn build_primitive(self, template: Arc<dyn RenderObj>) -> PrimitivePrototype {
        let name = self.name.unwrap_or_else(|| template.get_name().to_string());
        let class_id = self
            .class_id
            .unwrap_or_else(|| PrimitivePrototype::infer_class_id(&*template));

        let mut proto = PrimitivePrototype {
            name,
            class_id,
            template,
            asset_file: None,
        };

        if let Some(asset_file) = self.asset_file {
            proto.asset_file = Some(asset_file);
        }

        proto
    }
}

impl Default for PrototypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;
    use std::sync::Arc;

    // Mock RenderObj for testing
    #[derive(Debug, Clone)]
    struct MockRenderObj {
        name: String,
        transform: Mat4,
    }

    impl MockRenderObj {
        fn new(name: String) -> Self {
            Self {
                name,
                transform: Mat4::IDENTITY,
            }
        }
    }

    impl RenderObj for MockRenderObj {
        fn render(&self) {
            // Mock implementation
        }

        fn get_name(&self) -> &str {
            &self.name
        }

        fn set_transform(&mut self, transform: Mat4) {
            self.transform = transform;
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
            Box::new(MockRenderObj {
                name: self.name.clone(),
                transform: self.transform,
            })
        }
    }

    #[test]
    fn test_primitive_prototype_creation() {
        let template = Arc::new(MockRenderObj::new("TestMesh".to_string()));
        let prototype = PrimitivePrototype::new(template);

        assert_eq!(prototype.get_name(), "TestMesh");
    }

    #[test]
    fn test_primitive_prototype_create_instance() {
        let template = Arc::new(MockRenderObj::new("TestMesh".to_string()));
        let prototype = PrimitivePrototype::new(template);

        let instance = prototype.create();
        assert_eq!(instance.get_name(), "TestMesh");
    }

    #[test]
    fn test_multiple_instances_independent() {
        let template = Arc::new(MockRenderObj::new("TestMesh".to_string()));
        let prototype = PrimitivePrototype::new(template);

        let mut instance1 = prototype.create();
        let instance2 = prototype.create();

        // Modify instance1's transform
        let transform1 = Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        instance1.set_transform(transform1);

        // instance2 should still have identity transform
        assert_eq!(*instance2.get_transform(), Mat4::IDENTITY);
        assert_eq!(*instance1.get_transform(), transform1);
    }

    #[test]
    fn test_prototype_builder() {
        let template = Arc::new(MockRenderObj::new("TestMesh".to_string()));
        let prototype = PrototypeBuilder::new()
            .name("CustomName".to_string())
            .class_id(RenderObjClassId::Mesh)
            .asset_file("test.w3d".to_string())
            .build_primitive(template);

        assert_eq!(prototype.get_name(), "CustomName");
        assert_eq!(prototype.get_class_id(), RenderObjClassId::Mesh);
        assert_eq!(prototype.get_asset_file(), Some("test.w3d"));
    }

    #[test]
    fn test_arc_sharing() {
        let template = Arc::new(MockRenderObj::new("TestMesh".to_string()));
        let template_clone = Arc::clone(&template);

        let prototype = PrimitivePrototype::new(template);

        // Verify Arc is actually sharing the same data
        assert_eq!(Arc::strong_count(prototype.template()), 2);

        // Drop the clone
        drop(template_clone);
        assert_eq!(Arc::strong_count(prototype.template()), 1);
    }
}
