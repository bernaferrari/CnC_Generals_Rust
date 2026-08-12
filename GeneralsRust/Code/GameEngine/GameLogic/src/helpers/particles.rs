// FX list, particle systems, scorch/tree/animation hooks
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

/// FX list bridge to client-side effect manager (matching C++ TheFXList)
pub struct TheFXList;

static FX_LIST_MANAGER: OnceLock<Arc<dyn FXListManagerInterface>> = OnceLock::new();

pub fn register_fx_list_manager(manager: Arc<dyn FXListManagerInterface>) -> bool {
    FX_LIST_MANAGER.set(manager).is_ok()
}

pub fn get_fx_list_manager() -> Option<&'static Arc<dyn FXListManagerInterface>> {
    FX_LIST_MANAGER.get()
}

impl TheFXList {
    pub fn get() -> Option<&'static Self> {
        static FXLIST: OnceLock<TheFXList> = OnceLock::new();
        Some(FXLIST.get_or_init(|| TheFXList))
    }

    pub fn do_fx_at_position(&self, fx_template: &str, pos: &Coord3D) {
        let Some(manager) = FX_LIST_MANAGER.get() else {
            return;
        };
        let fx_id = NameKeyGenerator::name_to_key(fx_template) as FXListId;
        manager.do_fx_pos(fx_id, pos, None);
    }
}

/// Particle system manager bridge to the client-side implementation.
pub struct TheParticleSystemManager;

static PARTICLE_SYSTEM_MANAGER: OnceLock<Arc<dyn ParticleSystemManagerInterface>> = OnceLock::new();

pub fn register_particle_system_manager(manager: Arc<dyn ParticleSystemManagerInterface>) -> bool {
    PARTICLE_SYSTEM_MANAGER.set(manager).is_ok()
}

fn get_particle_system_manager() -> Option<&'static Arc<dyn ParticleSystemManagerInterface>> {
    PARTICLE_SYSTEM_MANAGER.get()
}

pub type ScorchHook = Arc<dyn Fn(&Coord3D, Real, i32) + Send + Sync>;
static SCORCH_HOOK: OnceLock<ScorchHook> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct TerrainTreeRegistration {
    pub drawable_id: u32,
    pub location: Coord3D,
    pub scale: Real,
    pub angle: Real,
    pub random_scale_amount: Real,
    pub module_data: W3DTreeDrawModuleData,
}

#[derive(Clone, Debug)]
pub enum TerrainTreeEvent {
    Add(TerrainTreeRegistration),
    Remove(u32),
}

pub type TerrainTreeHook = Arc<dyn Fn(TerrainTreeEvent) + Send + Sync>;
static TERRAIN_TREE_HOOK: OnceLock<TerrainTreeHook> = OnceLock::new();
pub type AnimationMetadataHook = Arc<dyn Fn(&str) -> Option<Real> + Send + Sync>;
static ANIMATION_METADATA_HOOK: OnceLock<AnimationMetadataHook> = OnceLock::new();

pub fn register_scorch_hook(hook: ScorchHook) -> bool {
    SCORCH_HOOK.set(hook).is_ok()
}

fn get_scorch_hook() -> Option<&'static ScorchHook> {
    SCORCH_HOOK.get()
}

pub fn register_terrain_tree_hook(hook: TerrainTreeHook) -> bool {
    TERRAIN_TREE_HOOK.set(hook).is_ok()
}

fn get_terrain_tree_hook() -> Option<&'static TerrainTreeHook> {
    TERRAIN_TREE_HOOK.get()
}

pub fn register_animation_metadata_hook(hook: AnimationMetadataHook) -> bool {
    ANIMATION_METADATA_HOOK.set(hook).is_ok()
}

fn get_animation_metadata_hook() -> Option<&'static AnimationMetadataHook> {
    ANIMATION_METADATA_HOOK.get()
}

impl TheParticleSystemManager {
    pub fn get() -> Option<&'static Self> {
        static MGR: OnceLock<TheParticleSystemManager> = OnceLock::new();
        Some(MGR.get_or_init(|| TheParticleSystemManager))
    }

    pub fn create_particle_system(&self, template: Option<&str>) -> Option<u32> {
        let manager = get_particle_system_manager()?;
        let name = template?;
        let template_id = manager.find_template(name)?;
        manager.create_particle_system(template_id)
    }

    pub fn find_template(&self, template: &str) -> Option<u32> {
        let manager = get_particle_system_manager()?;
        manager.find_template(template)
    }

    pub fn create_attached_particle_system_id(
        &self,
        template: Option<&str>,
        object_id: ObjectID,
    ) -> Option<u32> {
        let manager = get_particle_system_manager()?;
        let name = template?;
        let template_id = manager.find_template(name)?;
        manager.create_attached_particle_system_id(template_id, object_id)
    }

    pub fn set_particle_system_position(&self, id: u32, position: &Coord3D) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_position(id, position);
        }
    }

    pub fn get_particle_system_position(&self, id: u32) -> Option<Coord3D> {
        let manager = get_particle_system_manager()?;
        manager.get_particle_system_position(id)
    }

    pub fn attach_particle_system_to_object(&self, id: u32, object_id: ObjectID) {
        if let Some(manager) = get_particle_system_manager() {
            manager.attach_particle_system_to_object(id, object_id);
        }
    }

    pub fn attach_particle_system_to_drawable(&self, id: u32, drawable_id: ObjectID) {
        if let Some(manager) = get_particle_system_manager() {
            manager.attach_particle_system_to_drawable(id, drawable_id);
        }
    }

    pub fn set_particle_system_transform(&self, id: u32, transform: &Matrix3D) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_transform(id, transform);
        }
    }

    pub fn destroy_particle_system(&self, id: u32) {
        if let Some(manager) = get_particle_system_manager() {
            manager.destroy_particle_system(id);
        }
    }

    pub fn destroy_attached_systems(&self, object_id: ObjectID) {
        if let Some(manager) = get_particle_system_manager() {
            manager.destroy_attached_systems(object_id);
        }
    }

    pub fn start_particle_system(&self, id: u32) {
        if let Some(manager) = get_particle_system_manager() {
            manager.start_particle_system(id);
        }
    }

    pub fn stop_particle_system(&self, id: u32) {
        if let Some(manager) = get_particle_system_manager() {
            manager.stop_particle_system(id);
        }
    }

    pub fn set_particle_system_saveable(&self, id: u32, saveable: bool) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_saveable(id, saveable);
        }
    }

    pub fn rotate_particle_system_local_transform_z(&self, id: u32, angle: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.rotate_particle_system_local_transform_z(id, angle);
        }
    }

    pub fn set_particle_system_skip_parent_xfrm(&self, id: u32, enable: bool) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_skip_parent_xfrm(id, enable);
        }
    }

    pub fn tint_particle_system_all_colors(&self, id: u32, color: Color) {
        if let Some(manager) = get_particle_system_manager() {
            manager.tint_particle_system_all_colors(id, color);
        }
    }

    pub fn set_particle_system_velocity_multiplier(&self, id: u32, multiplier: &Coord3D) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_velocity_multiplier(id, multiplier);
        }
    }

    pub fn set_particle_system_burst_count_multiplier(&self, id: u32, multiplier: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_burst_count_multiplier(id, multiplier);
        }
    }

    pub fn find_particle_system(&self, id: u32) -> Option<Box<dyn std::any::Any>> {
        let manager = get_particle_system_manager()?;
        manager.find_particle_system(id)
    }

    pub fn get_particle_system_emission_volume_type(&self, id: u32) -> Option<EmissionVolumeType> {
        let manager = get_particle_system_manager()?;
        manager.get_particle_system_emission_volume_type(id)
    }

    pub fn set_particle_system_emission_volume_sphere_radius(&self, id: u32, radius: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_emission_volume_sphere_radius(id, radius);
        }
    }

    pub fn set_particle_system_emission_volume_cylinder_radius(&self, id: u32, radius: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_emission_volume_cylinder_radius(id, radius);
        }
    }

    /// Name-based OCL attach: findTemplate + createParticleSystem + attachToObject.
    ///
    /// Distinct from [`Self::attach_particle_system_to_object`], which takes an
    /// already-created system id. Fail-closed if manager/template is missing.
    pub fn attach_named_particle_system_to_object(
        &self,
        name: &str,
        object_id: ObjectID,
    ) -> Option<u32> {
        attach_particle_system_to_object(name, object_id)
    }
}

/// C++ ObjectCreationList.cpp GenericObjectCreationNugget::doStuffToObj:
/// `TheParticleSystemManager->findTemplate(name)` + `createParticleSystem` +
/// `sys->attachToObject(obj)`.
///
/// Stable GameLogic-callable entry for OCL. Fail-closed: empty name, missing
/// manager, or unknown template returns `None` (never panics).
pub fn attach_particle_system_to_object(name: &str, object_id: ObjectID) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    let manager = get_particle_system_manager()?;
    let template_id = manager.find_template(name)?;
    let system_id = manager.create_particle_system(template_id)?;
    manager.attach_particle_system_to_object(system_id, object_id);
    Some(system_id)
}

#[cfg(test)]
mod particle_attach_support {
    use crate::common::types::{
        EmissionVolumeType, ParticleSystemManagerInterface,
    };
    use crate::common::{Coord3D, Matrix3D, ObjectID, Real};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    #[derive(Debug)]
    pub struct RecordingParticleManager {
        templates: Mutex<HashMap<String, u32>>,
        next_template_id: AtomicU32,
        next_system_id: AtomicU32,
        /// system_id -> attached object_id
        attached: Mutex<HashMap<u32, ObjectID>>,
        attach_count: AtomicU32,
        last_attached_object: AtomicU32,
    }

    impl RecordingParticleManager {
        pub fn new() -> Self {
            Self {
                templates: Mutex::new(HashMap::new()),
                next_template_id: AtomicU32::new(1),
                next_system_id: AtomicU32::new(1),
                attached: Mutex::new(HashMap::new()),
                attach_count: AtomicU32::new(0),
                last_attached_object: AtomicU32::new(0),
            }
        }

        pub fn register_template(&self, name: &str) -> u32 {
            let mut map = self.templates.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(&id) = map.get(name) {
                return id;
            }
            let id = self.next_template_id.fetch_add(1, Ordering::Relaxed);
            map.insert(name.to_string(), id);
            id
        }

        pub fn attach_count(&self) -> u32 {
            self.attach_count.load(Ordering::Relaxed)
        }

        pub fn attached_object(&self, system_id: u32) -> Option<ObjectID> {
            self.attached.lock().ok()?.get(&system_id).copied()
        }

        pub fn last_attached_object(&self) -> Option<ObjectID> {
            let id = self.last_attached_object.load(Ordering::Relaxed);
            if id == 0 {
                None
            } else {
                Some(id)
            }
        }
    }

    impl ParticleSystemManagerInterface for RecordingParticleManager {
        fn find_template(&self, name: &str) -> Option<u32> {
            self.templates.lock().ok()?.get(name).copied()
        }

        fn create_particle_system(&self, template_id: u32) -> Option<u32> {
            let known = self
                .templates
                .lock()
                .ok()?
                .values()
                .any(|&id| id == template_id);
            if !known {
                return None;
            }
            Some(self.next_system_id.fetch_add(1, Ordering::Relaxed))
        }

        fn create_attached_particle_system_id(
            &self,
            template_id: u32,
            object_id: ObjectID,
        ) -> Option<u32> {
            let id = self.create_particle_system(template_id)?;
            self.attach_particle_system_to_object(id, object_id);
            Some(id)
        }

        fn find_particle_system(&self, _system_id: u32) -> Option<Box<dyn std::any::Any>> {
            None
        }

        fn set_particle_system_position(&self, _system_id: u32, _position: &Coord3D) {}

        fn get_particle_system_position(&self, _system_id: u32) -> Option<Coord3D> {
            None
        }

        fn attach_particle_system_to_object(&self, system_id: u32, object_id: ObjectID) {
            if let Ok(mut map) = self.attached.lock() {
                map.insert(system_id, object_id);
                self.attach_count.fetch_add(1, Ordering::Relaxed);
                self.last_attached_object.store(object_id, Ordering::Relaxed);
            }
        }

        fn attach_particle_system_to_drawable(&self, _system_id: u32, _drawable_id: ObjectID) {}

        fn set_particle_system_transform(&self, _system_id: u32, _transform: &Matrix3D) {}

        fn destroy_particle_system(&self, _system_id: u32) {}

        fn get_particle_system_emission_volume_type(
            &self,
            _system_id: u32,
        ) -> Option<EmissionVolumeType> {
            None
        }

        fn set_particle_system_emission_volume_sphere_radius(
            &self,
            _system_id: u32,
            _radius: Real,
        ) {
        }

        fn set_particle_system_emission_volume_cylinder_radius(
            &self,
            _system_id: u32,
            _radius: Real,
        ) {
        }
    }

    static TEST_MGR: OnceLock<Arc<RecordingParticleManager>> = OnceLock::new();

    pub fn ensure_test_manager() -> Option<Arc<RecordingParticleManager>> {
        let mgr = TEST_MGR.get_or_init(|| {
            let m = Arc::new(RecordingParticleManager::new());
            let _ = super::register_particle_system_manager(m.clone());
            m
        });
        mgr.register_template("__ocl_particle_test_sentinel__");
        if super::get_particle_system_manager()?
            .find_template("__ocl_particle_test_sentinel__")
            .is_some()
        {
            Some(Arc::clone(mgr))
        } else {
            None
        }
    }
}

/// Register a dummy particle template on the in-process test recorder.
/// Returns `false` if a different particle manager is already installed.
#[cfg(test)]
pub fn register_test_particle_template(name: &str) -> bool {
    match particle_attach_support::ensure_test_manager() {
        Some(mgr) => {
            mgr.register_template(name);
            true
        }
        None => false,
    }
}

#[cfg(test)]
pub fn test_particle_attach_count() -> u32 {
    particle_attach_support::ensure_test_manager()
        .map(|m| m.attach_count())
        .unwrap_or(0)
}

#[cfg(test)]
pub fn test_particle_attached_object_id(system_id: u32) -> Option<ObjectID> {
    particle_attach_support::ensure_test_manager()?.attached_object(system_id)
}

#[cfg(test)]
pub fn test_last_attached_object_id() -> Option<ObjectID> {
    particle_attach_support::ensure_test_manager()?.last_attached_object()
}

#[cfg(test)]
mod particle_tests {
    #[test]
    fn attach_particle_system_to_object_dummy_name_does_not_panic() {
        let result = std::panic::catch_unwind(|| {
            super::attach_particle_system_to_object("OclDummyNoSuchParticleTemplate", 12345)
        });
        assert!(result.is_ok(), "dummy particle template must fail-closed");
        assert_eq!(result.unwrap(), None);

        let empty = std::panic::catch_unwind(|| super::attach_particle_system_to_object("", 1));
        assert!(empty.is_ok());
        assert_eq!(empty.unwrap(), None);
    }

    #[test]
    fn attach_particle_system_to_object_records_parent_object_id() {
        let Some(_) = super::particle_attach_support::ensure_test_manager() else {
            // Another manager already owns the OnceLock; dummy path still fail-closed.
            assert!(super::attach_particle_system_to_object("MissingOclParticle", 7).is_none());
            return;
        };
        assert!(super::register_test_particle_template("OclTestSmoke"));
        let before = super::test_particle_attach_count();
        let object_id = 42_424u32;
        let system_id = super::attach_particle_system_to_object("OclTestSmoke", object_id)
            .expect("registered test template should create+attach");
        assert_eq!(super::test_particle_attach_count(), before + 1);
        assert_eq!(
            super::test_particle_attached_object_id(system_id),
            Some(object_id)
        );
        assert!(super::attach_particle_system_to_object("StillMissingOclParticle", object_id)
            .is_none());
    }
}
