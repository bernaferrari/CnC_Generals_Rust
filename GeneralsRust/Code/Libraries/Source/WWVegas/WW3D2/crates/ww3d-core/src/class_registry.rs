//! Runtime class-ID registry used by WW3D.
//!
//! Owned [`ClassRegistry`] maps numeric class identifiers to RTTI metadata
//! without process-wide statics.

use crate::classid::{ClassID, RenderObjClassId};
use std::{any::TypeId, collections::HashMap, fmt};

#[derive(Clone, Copy, Debug)]
struct ClassRecord {
    id: u32,
    name: &'static str,
    type_id: Option<TypeId>,
}

/// Owned WW3D class-id registry (per Ww3dContext / host).
pub struct ClassRegistry {
    registry_by_id: std::sync::Mutex<HashMap<u32, ClassRecord>>,
    registry_by_type: std::sync::Mutex<HashMap<TypeId, u32>>,
}

impl Default for ClassRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassRegistry {
    pub fn new() -> Self {
        Self {
            registry_by_id: std::sync::Mutex::new(HashMap::new()),
            registry_by_type: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

const CLASSID_NAME_TABLE: &[(u32, &str)] = &[
    (ClassID::IndirectTextureClass as u32, "IndirectTextureClass"),
    (ClassID::VariableTextureClass as u32, "VariableTextureClass"),
    (ClassID::FileListTextureClass as u32, "FileListTextureClass"),
    (
        ClassID::ResizeableTextureInstanceClass as u32,
        "ResizeableTextureInstanceClass",
    ),
    (
        ClassID::AnimTextureInstanceClass as u32,
        "AnimTextureInstanceClass",
    ),
    (
        ClassID::ManualAnimTextureInstanceClass as u32,
        "ManualAnimTextureInstanceClass",
    ),
    (
        ClassID::TimeAnimTextureInstanceClass as u32,
        "TimeAnimTextureInstanceClass",
    ),
    (ClassID::PointGroupClass as u32, "PointGroupClass"),
    (ClassID::MeshModelClass as u32, "MeshModelClass"),
    (
        ClassID::CachedTextureFileClass as u32,
        "CachedTextureFileClass",
    ),
    (
        ClassID::StreamingTextureClass as u32,
        "StreamingTextureClass",
    ),
    (
        ClassID::StreamingTextureInstanceClass as u32,
        "StreamingTextureInstanceClass",
    ),
];

const RENDEROBJ_CLASS_NAME_TABLE: &[(u32, &str)] = &[
    (RenderObjClassId::Unknown as u32, "CLASSID_UNKNOWN"),
    (RenderObjClassId::Mesh as u32, "CLASSID_MESH"),
    (RenderObjClassId::HModel as u32, "CLASSID_HMODEL"),
    (RenderObjClassId::DistLod as u32, "CLASSID_DISTLOD"),
    (
        RenderObjClassId::PredLodGroup as u32,
        "CLASSID_PREDLODGROUP",
    ),
    (RenderObjClassId::TileMap as u32, "CLASSID_TILEMAP"),
    (RenderObjClassId::Image3D as u32, "CLASSID_IMAGE3D"),
    (RenderObjClassId::Line3D as u32, "CLASSID_LINE3D"),
    (RenderObjClassId::Bitmap2D as u32, "CLASSID_BITMAP2D"),
    (RenderObjClassId::Camera as u32, "CLASSID_CAMERA"),
    (RenderObjClassId::DynaMesh as u32, "CLASSID_DYNAMESH"),
    (
        RenderObjClassId::DynaScreenMesh as u32,
        "CLASSID_DYNASCREENMESH",
    ),
    (RenderObjClassId::TextDraw as u32, "CLASSID_TEXTDRAW"),
    (RenderObjClassId::Fog as u32, "CLASSID_FOG"),
    (RenderObjClassId::LayerFog as u32, "CLASSID_LAYERFOG"),
    (RenderObjClassId::Light as u32, "CLASSID_LIGHT"),
    (
        RenderObjClassId::ParticleEmitter as u32,
        "CLASSID_PARTICLEEMITTER",
    ),
    (
        RenderObjClassId::ParticleBuffer as u32,
        "CLASSID_PARTICLEBUFFER",
    ),
    (
        RenderObjClassId::ScreenPointGroup as u32,
        "CLASSID_SCREENPOINTGROUP",
    ),
    (
        RenderObjClassId::ViewPointGroup as u32,
        "CLASSID_VIEWPOINTGROUP",
    ),
    (
        RenderObjClassId::WorldPointGroup as u32,
        "CLASSID_WORLDPOINTGROUP",
    ),
    (RenderObjClassId::Text2D as u32, "CLASSID_TEXT2D"),
    (RenderObjClassId::Text3D as u32, "CLASSID_TEXT3D"),
    (RenderObjClassId::Null as u32, "CLASSID_NULL"),
    (RenderObjClassId::Collection as u32, "CLASSID_COLLECTION"),
    (RenderObjClassId::Flare as u32, "CLASSID_FLARE"),
    (RenderObjClassId::Hlod as u32, "CLASSID_HLOD"),
    (RenderObjClassId::AaBox as u32, "CLASSID_AABOX"),
    (RenderObjClassId::ObBox as u32, "CLASSID_OBBOX"),
    (RenderObjClassId::SegLine as u32, "CLASSID_SEGLINE"),
    (RenderObjClassId::Sphere as u32, "CLASSID_SPHERE"),
    (RenderObjClassId::Ring as u32, "CLASSID_RING"),
    (RenderObjClassId::BoundFog as u32, "CLASSID_BOUNDFOG"),
    (RenderObjClassId::Dazzle as u32, "CLASSID_DAZZLE"),
    (RenderObjClassId::Sound as u32, "CLASSID_SOUND"),
    (
        RenderObjClassId::SegLineTrail as u32,
        "CLASSID_SEGLINETRAIL",
    ),
    (RenderObjClassId::Land as u32, "CLASSID_LAND"),
    (RenderObjClassId::ShdMesh as u32, "CLASSID_SHDMESH"),
];

#[derive(Debug)]
pub enum ClassRegistryError {
    /// Attempted to register an ID that is already known.
    IdAlreadyRegistered {
        id: u32,
        existing_name: &'static str,
    },
    /// Attempted to register a type that already has an ID assigned.
    TypeAlreadyRegistered { id: u32, name: &'static str },
}

impl fmt::Display for ClassRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassRegistryError::IdAlreadyRegistered { id, existing_name } => {
                write!(
                    f,
                    "class id {id:#X} is already registered to '{existing_name}'"
                )
            }
            ClassRegistryError::TypeAlreadyRegistered { id, name } => {
                write!(
                    f,
                    "type already registered as '{name}' with class id {id:#X}"
                )
            }
        }
    }
}

impl std::error::Error for ClassRegistryError {}

impl ClassRegistry {
/// Registers a class identifier with an associated Rust type and legacy name.
pub fn register_class<T: 'static>(&self, id: u32, name: &'static str) -> Result<(), ClassRegistryError> {
    self.register_class_name(id, name)?;

    let type_id = TypeId::of::<T>();
    let duplicated = { let mut by_type = self.registry_by_type.lock().unwrap_or_else(|p| p.into_inner()); 
        let mut by_type = by_type;
        if let Some(existing_id) = by_type.get(&type_id) {
            let existing = { let by_id = self.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); 
                let record = by_id
                    .get(existing_id)
                    .expect("type map and id map out of sync");
                (record.id, record.name)
             };
            Some(existing)
        } else {
            by_type.insert(type_id, id);
            None
        }
     };

    if let Some((existing_id, existing_name)) = duplicated {
        return Err(ClassRegistryError::TypeAlreadyRegistered {
            id: existing_id,
            name: existing_name,
        });
    }

    { let mut by_id = self.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); 
        if let Some(record) = by_id.get_mut(&id) {
            record.type_id = Some(type_id);
        }
     };
    Ok(())
}

/// Registers a class identifier without binding it to a Rust type.
pub fn register_class_name(&self, id: u32, name: &'static str) -> Result<(), ClassRegistryError> {
    { let mut by_id = self.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); 
        if let Some(existing) = by_id.get(&id) {
            if existing.name == name {
                return Ok(());
            }
            return Err(ClassRegistryError::IdAlreadyRegistered {
                id,
                existing_name: existing.name,
            });
        }
        by_id.insert(
            id,
            ClassRecord {
                id,
                name,
                type_id: None,
            },
        );
        Ok(())
     }
}

/// Registers all builtin WW3D class names so lookups succeed even before Rust
/// types are bound to the identifiers.
pub fn register_builtin_class_names(&self) {
    for &(id, name) in CLASSID_NAME_TABLE {
        let _ = self.register_class_name(id, name);
    }
    for &(id, name) in RENDEROBJ_CLASS_NAME_TABLE {
        let _ = self.register_class_name(id, name);
    }
}

/// Looks up the human-readable name for a class identifier.
pub fn class_name_from_id(&self, id: u32) -> Option<&'static str> {
    { let map = self.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); map.get(&id).map(|record| record.name) }
}

/// Returns the class identifier that was registered for the supplied type.
pub fn class_id_for_type<T: 'static>(&self) -> Option<u32> {
    let type_id = TypeId::of::<T>();
    { let map = self.registry_by_type.lock().unwrap_or_else(|p| p.into_inner()); map.get(&type_id).copied() }
}

/// Returns the `TypeId` associated with a class identifier, if one is known.
pub fn type_id_from_class(&self, id: u32) -> Option<TypeId> {
    { let map = self.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); map.get(&id).and_then(|record| record.type_id) }
}

/// Convenience helper to check whether a class identifier has been registered.
pub fn is_class_registered(&self, id: u32) -> bool {
    { let map = self.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); map.contains_key(&id) }
}

}

#[cfg(test)]
pub(crate) fn clear_registry_for_test() {
    { let mut map = registry.registry_by_id.lock().unwrap_or_else(|p| p.into_inner()); map.clear() };
    { let mut map = registry.registry_by_type.lock().unwrap_or_else(|p| p.into_inner()); map.clear() };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;
    struct Other;

    #[test]
    fn registers_and_queries_class() {
        let registry = ClassRegistry::new();
        clear_registry_for_test();
        register_class::<Dummy>(0x1234, "DummyClass").expect("registration succeeds");
        assert_eq!(registry.class_name_from_id(0x1234), Some("DummyClass"));
        assert_eq!(class_id_for_type::<Dummy>(), Some(0x1234));
        assert!(registry.is_class_registered(0x1234));
    }

    #[test]
    fn duplicate_id_is_rejected() {
        clear_registry_for_test();
        register_class::<Dummy>(0x2000, "DummyClass").expect("first registration");
        let err = register_class::<Other>(0x2000, "OtherClass").unwrap_err();
        match err {
            ClassRegistryError::IdAlreadyRegistered { id, existing_name } => {
                assert_eq!(id, 0x2000);
                assert_eq!(existing_name, "DummyClass");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn duplicate_type_is_rejected() {
        clear_registry_for_test();
        register_class::<Dummy>(0x2000, "DummyClass").expect("first registration");
        let err = register_class::<Dummy>(0x2001, "DummyAgain").unwrap_err();
        match err {
            ClassRegistryError::TypeAlreadyRegistered { id, name } => {
                assert_eq!(id, 0x2000);
                assert_eq!(name, "DummyClass");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn builtin_names_register_without_error() {
        let registry = ClassRegistry::new();
        clear_registry_for_test();
        registry.register_builtin_class_names();
        assert_eq!(
            registry.class_name_from_id(ClassID::MeshModelClass as u32),
            Some("MeshModelClass")
        );
        assert_eq!(
            registry.class_name_from_id(RenderObjClassId::SegLine as u32),
            Some("CLASSID_SEGLINE")
        );
    }
}
