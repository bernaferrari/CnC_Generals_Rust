//! Runtime class-ID registry used by WW3D.
//!
//! The legacy engine exposes a global registry that maps numeric class
//! identifiers to RTTI metadata and construction helpers.  The Rust port keeps
//! the data in a thread-safe registry so render objects and prototypes can look
//! up their class information without relying on `static mut`.

use crate::classid::{ClassID, RenderObjClassId};
use once_cell::sync::Lazy;
use std::{any::TypeId, collections::HashMap, fmt, sync::RwLock};

#[derive(Clone, Copy, Debug)]
struct ClassRecord {
    id: u32,
    name: &'static str,
    type_id: Option<TypeId>,
}

static REGISTRY_BY_ID: Lazy<RwLock<HashMap<u32, ClassRecord>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
static REGISTRY_BY_TYPE: Lazy<RwLock<HashMap<TypeId, u32>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

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

/// Errors that may occur when registering class information.
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

/// Registers a class identifier with an associated Rust type and legacy name.
pub fn register_class<T: 'static>(id: u32, name: &'static str) -> Result<(), ClassRegistryError> {
    register_class_name(id, name)?;

    let type_id = TypeId::of::<T>();
    let mut by_type = REGISTRY_BY_TYPE
        .write()
        .expect("class registry poisoned (type)");
    if let Some(existing_id) = by_type.get(&type_id) {
        let existing = REGISTRY_BY_ID
            .read()
            .expect("class registry poisoned (id)")
            .get(existing_id)
            .expect("type map and id map out of sync")
            .clone();
        return Err(ClassRegistryError::TypeAlreadyRegistered {
            id: existing.id,
            name: existing.name,
        });
    }
    by_type.insert(type_id, id);

    if let Some(record) = REGISTRY_BY_ID
        .write()
        .expect("class registry poisoned (id)")
        .get_mut(&id)
    {
        record.type_id = Some(type_id);
    }
    Ok(())
}

/// Registers a class identifier without binding it to a Rust type.
pub fn register_class_name(id: u32, name: &'static str) -> Result<(), ClassRegistryError> {
    let mut by_id = REGISTRY_BY_ID
        .write()
        .expect("class registry poisoned (id)");
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

/// Registers all builtin WW3D class names so lookups succeed even before Rust
/// types are bound to the identifiers.
pub fn register_builtin_class_names() {
    for &(id, name) in CLASSID_NAME_TABLE {
        let _ = register_class_name(id, name);
    }
    for &(id, name) in RENDEROBJ_CLASS_NAME_TABLE {
        let _ = register_class_name(id, name);
    }
}

/// Looks up the human-readable name for a class identifier.
pub fn class_name_from_id(id: u32) -> Option<&'static str> {
    REGISTRY_BY_ID
        .read()
        .ok()
        .and_then(|map| map.get(&id).map(|record| record.name))
}

/// Returns the class identifier that was registered for the supplied type.
pub fn class_id_for_type<T: 'static>() -> Option<u32> {
    let type_id = TypeId::of::<T>();
    REGISTRY_BY_TYPE
        .read()
        .ok()
        .and_then(|map| map.get(&type_id).copied())
}

/// Returns the `TypeId` associated with a class identifier, if one is known.
pub fn type_id_from_class(id: u32) -> Option<TypeId> {
    REGISTRY_BY_ID
        .read()
        .ok()
        .and_then(|map| map.get(&id).and_then(|record| record.type_id))
}

/// Convenience helper to check whether a class identifier has been registered.
pub fn is_class_registered(id: u32) -> bool {
    REGISTRY_BY_ID
        .read()
        .map_or(false, |map| map.contains_key(&id))
}

#[cfg(test)]
pub(crate) fn clear_registry_for_test() {
    REGISTRY_BY_ID
        .write()
        .expect("class registry poisoned (id)")
        .clear();
    REGISTRY_BY_TYPE
        .write()
        .expect("class registry poisoned (type)")
        .clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    // Tests mutate the global registry; serialize them so concurrent execution
    // does not wipe entries created by a sibling test.
    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct Dummy;
    struct Other;

    #[test]
    fn registers_and_queries_class() {
        let _guard = TEST_GUARD.lock().expect("test guard poisoned");
        clear_registry_for_test();
        register_class::<Dummy>(0x1234, "DummyClass").expect("registration succeeds");
        assert_eq!(class_name_from_id(0x1234), Some("DummyClass"));
        assert_eq!(class_id_for_type::<Dummy>(), Some(0x1234));
        assert!(is_class_registered(0x1234));
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let _guard = TEST_GUARD.lock().expect("test guard poisoned");
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
        let _guard = TEST_GUARD.lock().expect("test guard poisoned");
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
        let _guard = TEST_GUARD.lock().expect("test guard poisoned");
        clear_registry_for_test();
        register_builtin_class_names();
        assert_eq!(
            class_name_from_id(ClassID::MeshModelClass as u32),
            Some("MeshModelClass")
        );
        assert_eq!(
            class_name_from_id(RenderObjClassId::SegLine as u32),
            Some("CLASSID_SEGLINE")
        );
    }
}
