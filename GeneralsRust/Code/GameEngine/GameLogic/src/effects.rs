//! Effects system - FX and OCL helpers wired to game logic stores.

use crate::common::types::FXListManagerInterface;
use crate::common::{
    AsciiString, Coord3D, FXListId, NameKeyGenerator, NameKeyType, ObjectID, Real,
};
use crate::object::Object;
use std::io;
use std::sync::{Arc, RwLock};

pub use crate::object_creation_list::store::ObjectCreationList;
pub use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;

/// Particle system identifier (mirrors C++ ParticleSystemId).
pub type ParticleSystemID = crate::common::ParticleSystemId;

/// Particle system handle (in this port, the runtime ID is the handle).
pub type ParticleSystem = ParticleSystemID;

/// FX list structure
#[derive(Debug, Clone)]
pub struct FXList {
    name: AsciiString,
    name_key: NameKeyType,
}

impl FXList {
    pub fn new(name: &str) -> Self {
        let name_key = NameKeyGenerator::name_to_key(name) as NameKeyType;
        Self {
            name: AsciiString::from(name),
            name_key,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn id(&self) -> FXListId {
        self.name_key as FXListId
    }

    fn resolve_id(&self, optional: Option<&str>) -> FXListId {
        match optional {
            Some(name) => NameKeyGenerator::name_to_key(name) as FXListId,
            None => self.id(),
        }
    }

    fn fx_manager(
    ) -> Result<&'static dyn FXListManagerInterface, Box<dyn std::error::Error + Send + Sync>> {
        crate::helpers::get_fx_list_manager()
            .map(|mgr| mgr.as_ref())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "FXListManager not registered").into()
            })
    }

    /// Execute a visual effect on an object with an optional source object.
    pub fn do_fx_obj_ids(
        &self,
        object_id: ObjectID,
        source_id: Option<ObjectID>,
        optional: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fx_mgr = Self::fx_manager()?;
        fx_mgr.do_fx_obj_with_source(self.resolve_id(optional), object_id, source_id);
        Ok(())
    }

    /// Execute a visual effect on an object with an optional source object.
    pub fn do_fx_obj_with_source(
        &self,
        object: &Arc<RwLock<Object>>,
        source: Option<&Arc<RwLock<Object>>>,
        optional: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = {
            let guard = object
                .read()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "Object lock poisoned"))?;
            guard.get_id()
        };
        let source_id = match source {
            Some(source) => source.read().map(|guard| guard.get_id()).ok(),
            None => None,
        };

        self.do_fx_obj_ids(object_id, source_id, optional)
    }

    /// Execute a visual effect on an object
    pub fn do_fx_obj(
        &self,
        object: &Arc<RwLock<Object>>,
        optional: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.do_fx_obj_with_source(object, None, optional)
    }

    /// Execute a visual effect at a world-space position
    pub fn do_fx_at_position(
        &self,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let fx_mgr = Self::fx_manager()?;
        fx_mgr.do_fx_pos(self.id(), position, None);
        Ok(())
    }

    /// Execute a visual effect at a world-space position with a radius hint.
    ///
    /// C++ FXList::doFXPos accepts a radius for some effects. The current Rust
    /// FX system ignores this hint, but we keep the API for parity.
    pub fn do_fx_at_position_with_radius(
        &self,
        position: &Coord3D,
        _radius: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.do_fx_at_position(position)
    }
}

impl ObjectCreationList {
    /// Create objects from an object creation list.
    ///
    /// Matches the common C++ call pattern `ObjectCreationList::create(ocl, owner, NULL)`.
    pub fn create(
        ocl: &ObjectCreationList,
        owner: &Arc<RwLock<Object>>,
        _optional: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let owner_guard = owner
            .read()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Owner lock poisoned"))?;
        let _primary_pos = owner_guard.get_position();

        let ctx = crate::object_creation_list::live_creation_context();

        let _ = ocl.create_with_objects(&ctx, &owner_guard, None, 0);
        Ok(())
    }

    /// Create objects at a world-space position using an owner for team/producer context when available.
    pub fn create_at_position(
        &self,
        position: &Coord3D,
        owner_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ctx = crate::object_creation_list::live_creation_context();

        let owner = crate::helpers::TheGameLogic::find_object_by_id(owner_id);
        let owner_guard = owner.as_ref().and_then(|h| h.read().ok());
        let primary_obj = owner_guard.as_deref();
        if primary_obj.is_none() {
            return Ok(());
        }

        let primary = *position;
        let secondary = *position;
        let _ = self.create_with_owner_flag(&ctx, primary_obj, &primary, &secondary, true, 0);
        Ok(())
    }

    /// Create objects at a world-space position with an explicit orientation.
    ///
    /// Mirrors `ObjectCreationList::create(ocl, owner, target, NULL, orientation)` usage.
    pub fn create_at_position_with_angle(
        &self,
        position: &Coord3D,
        owner_id: ObjectID,
        angle: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ctx = crate::object_creation_list::live_creation_context();

        let owner = crate::helpers::TheGameLogic::find_object_by_id(owner_id);
        let owner_guard = owner.as_ref().and_then(|h| h.read().ok());
        let primary_obj = owner_guard.as_deref();
        if primary_obj.is_none() {
            return Ok(());
        }

        let primary = *position;
        let secondary = *position;
        let _ = self.create_with_angle(&ctx, primary_obj, &primary, &secondary, angle, 0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::FXListManagerInterface;
    use crate::helpers::{register_fx_list_manager, TheFXListStore};
    use glam::Mat4;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingFxManager {
        object_calls: Arc<Mutex<Vec<(FXListId, ObjectID, Option<ObjectID>)>>>,
    }

    impl FXListManagerInterface for RecordingFxManager {
        fn do_fx_pos(&self, _fx_list: FXListId, _position: &Coord3D, _matrix: Option<&Mat4>) {}

        fn do_fx_obj(&self, fx_list: FXListId, object_id: ObjectID) {
            self.object_calls
                .lock()
                .unwrap()
                .push((fx_list, object_id, None));
        }

        fn do_fx_obj_with_source(
            &self,
            fx_list: FXListId,
            object_id: ObjectID,
            source_id: Option<ObjectID>,
        ) {
            self.object_calls
                .lock()
                .unwrap()
                .push((fx_list, object_id, source_id));
        }
    }

    #[test]
    fn fx_list_object_id_dispatch_preserves_source_orientation() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let _ = register_fx_list_manager(Arc::new(RecordingFxManager {
            object_calls: Arc::clone(&calls),
        }));
        let fx = TheFXListStore::ensure_fx_list("FX_TestDeath");

        fx.do_fx_obj_ids(42, Some(77), None).unwrap();

        assert_eq!(*calls.lock().unwrap(), vec![(fx.id(), 42, Some(77))]);
    }
}
