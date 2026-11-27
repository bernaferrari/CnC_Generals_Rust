// FILE: store.rs - Object Creation List Store
// Author: Steven Johnson, December 2001 (C++)
// Rust Port: 2025
// Desc: ObjectCreationListStore - manages all OCL definitions
//
// Ported from GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectCreationList.h
//
// The store holds all ObjectCreationList definitions loaded from INI files.
// ObjectCreationLists are immutable and shared between multiple users.

use super::nuggets::ObjectCreationNugget;
use super::{CreationContext, CreationResult};
use crate::common::NameKeyGenerator;
use crate::common::*;
use crate::object::Object;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Object Creation List ID type (index into store)
pub type ObjectCreationListId = usize;

/// ObjectCreationList - a collection of nuggets that create objects together
/// Matches C++ class ObjectCreationList (ObjectCreationList.h:102-157)
///
/// ObjectCreationLists are:
/// - Specified solely by name
/// - Shared between multiple units (immutable after creation)
/// - No inheritance or overriding
/// - Can't be modified by subsequent INI loads
#[derive(Clone)]
pub struct ObjectCreationList {
    /// Nuggets to execute (in order)
    /// Note: nuggets are owned by the store, this just holds references
    nuggets: Vec<Arc<dyn ObjectCreationNugget>>,
}

impl std::fmt::Debug for ObjectCreationList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectCreationList")
            .field("nugget_count", &self.nuggets.len())
            .finish()
    }
}

impl ObjectCreationList {
    /// Create empty OCL
    pub fn new() -> Self {
        Self {
            nuggets: Vec::new(),
        }
    }

    /// Add a nugget to this OCL
    /// Matches C++ ObjectCreationList::addObjectCreationNugget (ObjectCreationList.cpp:1502-1506)
    pub fn add_nugget(&mut self, nugget: Arc<dyn ObjectCreationNugget>) {
        self.nuggets.push(nugget);
    }

    /// Clear all nuggets
    /// Matches C++ ObjectCreationList::clear (ObjectCreationList.cpp:1495-1499)
    pub fn clear(&mut self) {
        self.nuggets.clear();
    }

    /// Create objects with position and angle
    /// Matches C++ static ObjectCreationList::create with angle parameter (ObjectCreationList.h:126-131)
    ///
    /// Returns the first object created (or None if no objects created)
    pub fn create_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_angle(
            ctx,
            primary_obj,
            primary,
            secondary,
            angle,
            lifetime_frames,
        )
    }
    /// C++ create with a null secondary position.
    pub fn create_without_secondary(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let mut first_object: Option<Arc<RwLock<Object>>> = None;
        for nugget in &self.nuggets {
            if let Some(obj) =
                nugget.create_without_secondary(ctx, primary_obj, primary, angle, lifetime_frames)
            {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }
        first_object
    }

    /// Create objects with object parameters
    /// Matches C++ static ObjectCreationList::create with objects (ObjectCreationList.h:136-141)
    ///
    /// Returns the first object created (or None if no objects created)
    pub fn create_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        secondary: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_objects(ctx, primary, secondary, lifetime_frames)
    }

    /// CPP ObjectCreationList.cpp:1539–1551 still runs every nugget in authored
    /// order. Generic births can update the newest dying parent's backlink
    /// before the child enters its original onCreate callbacks.
    pub(crate) fn create_with_borrowed_primary(
        &self,
        ctx: &CreationContext<'_>,
        primary: &mut Object,
        secondary_id: ObjectID,
    ) -> CreationResult {
        let mut first = None;
        for nugget in &self.nuggets {
            let created = nugget.create_with_borrowed_primary(ctx, primary, secondary_id);
            if first.is_none() {
                first = created;
            }
        }
        first
    }

    /// Create objects with owner flag (for DeliverPayload)
    /// Matches C++ static ObjectCreationList::create with createOwner (ObjectCreationList.h:116-121)
    ///
    /// Returns the first object created (or None if no objects created)
    pub fn create_with_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_owner_flag(
            ctx,
            primary_obj,
            primary,
            secondary,
            create_owner,
            lifetime_frames,
        )
    }

    /// Execute in list order with the exact source handle, without holding its
    /// object lock across nuggets that mutate that same source.
    pub fn create_with_owner_handle(
        &self,
        ctx: &CreationContext<'_>,
        owner: &Arc<RwLock<Object>>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Option<Real>,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let mut first = None;
        for nugget in &self.nuggets {
            let created = nugget.create_with_owner_handle(
                ctx, owner, primary, secondary, angle, create_owner, lifetime_frames,
            );
            if first.is_none() {
                first = created;
            }
        }
        first
    }

    /// Create objects with angle and create-owner flag.
    /// Matches C++ call sites that pass both angle and createOwner.
    pub fn create_with_angle_and_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        self.create_internal_with_angle_and_owner_flag(
            ctx,
            primary_obj,
            primary,
            secondary,
            angle,
            create_owner,
            lifetime_frames,
        )
    }

    /// Internal implementation - create with angle
    /// Matches C++ ObjectCreationList::createInternal (ObjectCreationList.cpp:1524-1536)
    fn create_internal_with_angle(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        debug_assert!(
            primary_obj.is_some(),
            "ObjectCreationList expects a non-null primary object for ownership context"
        );
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_angle(
                ctx,
                primary_obj,
                primary,
                secondary,
                angle,
                lifetime_frames,
            ) {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    /// Internal implementation - create with objects
    /// Matches C++ ObjectCreationList::createInternal (ObjectCreationList.cpp:1539-1551)
    fn create_internal_with_objects(
        &self,
        ctx: &CreationContext<'_>,
        primary: &Object,
        secondary: Option<&Object>,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_objects(ctx, primary, secondary, lifetime_frames)
            {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    /// Internal implementation - create with owner flag
    /// Matches C++ ObjectCreationList::createInternal (ObjectCreationList.cpp:1509-1521)
    fn create_internal_with_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        debug_assert!(
            primary_obj.is_some(),
            "ObjectCreationList expects a non-null primary object for ownership context"
        );
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_owner_flag(
                ctx,
                primary_obj,
                primary,
                secondary,
                create_owner,
                lifetime_frames,
            ) {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    fn create_internal_with_angle_and_owner_flag(
        &self,
        ctx: &CreationContext<'_>,
        primary_obj: Option<&Object>,
        primary: &Coord3D,
        secondary: &Coord3D,
        angle: Real,
        create_owner: Bool,
        lifetime_frames: UnsignedInt,
    ) -> CreationResult {
        debug_assert!(
            primary_obj.is_some(),
            "ObjectCreationList expects a non-null primary object for ownership context"
        );
        let mut first_object: Option<Arc<RwLock<Object>>> = None;

        for nugget in &self.nuggets {
            if let Some(obj) = nugget.create_with_angle_and_owner_flag(
                ctx,
                primary_obj,
                primary,
                secondary,
                angle,
                create_owner,
                lifetime_frames,
            ) {
                if first_object.is_none() {
                    first_object = Some(obj);
                }
            }
        }

        first_object
    }

    /// Get number of nuggets
    pub fn get_nugget_count(&self) -> usize {
        self.nuggets.len()
    }

    /// Nuggets in definition order (parse/create inspection).
    pub fn nuggets(&self) -> &[Arc<dyn ObjectCreationNugget>] {
        &self.nuggets
    }
}

impl Default for ObjectCreationList {
    fn default() -> Self {
        Self::new()
    }
}

/// ObjectCreationListStore - manages all OCL definitions
/// Matches C++ class ObjectCreationListStore (ObjectCreationList.h:163-194)
///
/// The store:
/// - Holds all OCL definitions loaded from INI
/// - Owns all nuggets (OCLs just reference them)
/// - Provides lookup by name
/// - Is a singleton (TheObjectCreationListStore)
pub struct ObjectCreationListStore {
    /// Exact authored names own catalog identity. Numeric name keys are allocated
    /// per thread in this port and cannot identify entries in a moved catalog.
    ocls: HashMap<String, Arc<ObjectCreationList>>,

    /// All nuggets owned by the store
    /// OCLs hold Arc references to these
    nuggets: Vec<Arc<dyn ObjectCreationNugget>>,
}

impl ObjectCreationListStore {
    /// Create new empty store
    /// Matches C++ ObjectCreationListStore::ObjectCreationListStore (ObjectCreationList.cpp:1558-1560)
    pub fn new() -> Self {
        Self {
            ocls: HashMap::new(),
            nuggets: Vec::new(),
        }
    }

    /// Find OCL by name
    /// Matches C++ ObjectCreationListStore::findObjectCreationList (ObjectCreationList.cpp:1574-1588)
    ///
    /// Returns None if "None" or not found
    pub fn find_object_creation_list(&self, name: &str) -> Option<Arc<ObjectCreationList>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }

        // Preserve C++ NAMEKEY allocation order, including unsuccessful lookups.
        let _ = NameKeyGenerator::name_to_key(name);
        self.ocls.get(name).cloned()
    }

    /// Add a nugget to the store
    /// Matches C++ ObjectCreationListStore::addObjectCreationNugget (ObjectCreationList.cpp:1591-1594)
    pub fn add_nugget(&mut self, nugget: Arc<dyn ObjectCreationNugget>) {
        self.nuggets.push(nugget);
    }

    /// Register an OCL by name
    /// Used during INI parsing
    pub fn register_ocl(
        &mut self,
        name: String,
        ocl: ObjectCreationList,
    ) -> Arc<ObjectCreationList> {
        let _ = NameKeyGenerator::name_to_key(name.as_str());
        let handle = Arc::new(ocl);
        self.ocls.insert(name, handle.clone());
        handle
    }

    /// Get mutable reference to OCL (for building during parse)
    pub fn get_ocl_mut(&mut self, name: &str) -> Option<&mut ObjectCreationList> {
        let _ = NameKeyGenerator::name_to_key(name);
        self.ocls.get_mut(name).map(Arc::make_mut)
    }

    /// Get or create OCL by name
    pub fn get_or_create_ocl(&mut self, name: String) -> &mut ObjectCreationList {
        let _ = NameKeyGenerator::name_to_key(name.as_str());
        let entry = self
            .ocls
            .entry(name)
            .or_insert_with(|| Arc::new(ObjectCreationList::new()));
        Arc::make_mut(entry)
    }

    /// Get number of registered OCLs
    pub fn get_ocl_count(&self) -> usize {
        self.ocls.len()
    }

    /// Get number of nuggets
    pub fn get_nugget_count(&self) -> usize {
        self.nuggets.len()
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.ocls.clear();
        self.nuggets.clear();
    }
}

impl Default for ObjectCreationListStore {
    fn default() -> Self {
        Self::new()
    }
}

fn ocl_store_cell() -> std::sync::Arc<std::sync::RwLock<Option<ObjectCreationListStore>>> {
    std::sync::Arc::clone(crate::system::engine_stores::active_services().ocl_store())
}

/// Initialize the per-world store
pub fn init_object_creation_list_store() {
    let cell = ocl_store_cell();
    let mut store = cell.write().unwrap();
    *store = Some(ObjectCreationListStore::new());
}

/// Get Arc to the active world's OCL store cell.
pub fn get_object_creation_list_store_arc()
-> std::sync::Arc<std::sync::RwLock<Option<ObjectCreationListStore>>> {
    ocl_store_cell()
}

/// Compatibility: callers that expected a 'static guard should switch to the Arc API.
pub fn get_object_creation_list_store()
-> std::sync::Arc<std::sync::RwLock<Option<ObjectCreationListStore>>> {
    ocl_store_cell()
}

pub fn get_object_creation_list_store_mut()
-> std::sync::Arc<std::sync::RwLock<Option<ObjectCreationListStore>>> {
    ocl_store_cell()
}

/// Adapt the authored data once for standalone object callbacks. The native
/// simulation reads the definitions directly from generals-sim.
fn bind_legacy_nuggets(
    authored: generals_sim::object_creation_list::ObjectCreationList,
) -> ObjectCreationList {
    use generals_sim::object_creation_list::ObjectCreationNugget as Authored;
    let mut list = ObjectCreationList::new();
    for nugget in authored.nuggets().iter().cloned() {
        let bound: Arc<dyn ObjectCreationNugget> = match nugget {
            Authored::Generic(data) => Arc::new(data),
            Authored::Attack(data) => Arc::new(data),
            Authored::FireWeapon(data) => Arc::new(data),
            Authored::ApplyRandomForce(data) => Arc::new(data),
            Authored::DeliverPayload(data) => Arc::new(data.map_data(|data| {
                data.bind_assets(
                    |name| crate::helpers::TheFXListStore::find_fx_list(&name),
                    |name| {
                        crate::weapon::with_weapon_store(|store| {
                            store.find_weapon_template(&name).cloned()
                        })
                        .ok()
                        .flatten()
                    },
                )
            })),
        };
        list.add_nugget(bound);
    }
    list
}

pub fn parse_object_creation_lists_from_str(
    content: &str,
) -> Result<Vec<(String, ObjectCreationList)>, String> {
    bind_legacy_lists(generals_sim::object_creation_list::parse_object_creation_lists_from_str(content))
}

fn bind_legacy_lists(
    parsed: Result<Vec<(String, generals_sim::object_creation_list::ObjectCreationList)>, String>,
) -> Result<Vec<(String, ObjectCreationList)>, String> {
    parsed.map(|lists| {
        lists
            .into_iter()
            .map(|(name, list)| (name, bind_legacy_nuggets(list)))
            .collect()
    })
}

/// Legacy publisher keeps its original global store adapter; private receivers
/// consume the exact same typed parser without publishing any active catalog.
pub fn load_object_creation_lists_from_str(content: &str) -> Result<usize, String> {
    let parsed = parse_object_creation_lists_from_str(content)?;
    if parsed.is_empty() {
        return Ok(0);
    }
    let services = crate::system::engine_stores::active_services();
    Ok(admit_object_creation_lists(&services, parsed, false))
}

fn load_object_creation_lists_into(
    services: &crate::system::engine_stores::WorldServices,
    content: &str,
    only_if_empty: bool,
) -> Result<usize, String> {
    let parsed = parse_object_creation_lists_from_str(content)?;
    if parsed.is_empty() {
        return Ok(0);
    }
    Ok(admit_object_creation_lists(services, parsed, only_if_empty))
}

fn admit_object_creation_lists(
    services: &crate::system::engine_stores::WorldServices,
    parsed: Vec<(String, ObjectCreationList)>,
    only_if_empty: bool,
) -> usize {

    let count_loaded = parsed.len();

    let mut store_guard = services.ocl_store().write().expect("ocl store write");
    if only_if_empty && store_guard.as_ref().is_some_and(|store| store.get_ocl_count() > 0) {
        return count_loaded;
    }
    let store = store_guard.get_or_insert_with(ObjectCreationListStore::new);

    for (name, ocl) in parsed {
        let nugget_clones: Vec<Arc<dyn ObjectCreationNugget>> = ocl.nuggets.to_vec();
        {
            let target_ocl = store.get_or_create_ocl(name);
            target_ocl.clear();
            for nugget in &nugget_clones {
                target_ocl.add_nugget(Arc::clone(nugget));
            }
        }
        for nugget in nugget_clones {
            store.nuggets.push(nugget);
        }
    }

    count_loaded
}

pub fn load_object_creation_lists_from_path<P: AsRef<Path>>(path: P) -> Result<usize, String> {
    let content =
        fs::read_to_string(&path).map_err(|err| format!("Failed to read OCL file: {err}"))?;
    load_object_creation_lists_from_str(&content)
}

fn default_ocl_paths() -> Vec<PathBuf> {
    let workspace_retail = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        // GameLogic lives at GeneralsRust/Code/GameEngine/GameLogic.
        // Keep the retail source-data lookup independent of the executable's
        // current working directory so a Cargo launch from GeneralsRust does
        // not silently leave the global OCL store empty.
        .join("../../../..")
        .join("windows_game/extracted_big_files/INIZH/Data/INI/ObjectCreationList.ini");
    vec![
        PathBuf::from("Data/INI/ObjectCreationList.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INI/ObjectCreationList.ini"),
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/ObjectCreationList.ini"),
        workspace_retail,
    ]
}

fn ensure_default_object_creation_lists_loaded_in(
    services: &crate::system::engine_stores::WorldServices,
    paths: &[PathBuf],
) {
    services.ocl_defaults_initialized().get_or_init(|| {
        if services.ocl_store()
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|store| store.get_ocl_count() > 0))
            .unwrap_or(false)
        {
            return;
        }

        for path in paths {
            if !path.exists() {
                continue;
            }

            let loaded = fs::read_to_string(path)
                .map_err(|err| format!("Failed to read OCL file: {err}"))
                .and_then(|content| load_object_creation_lists_into(services, &content, true));
            match loaded {
                Ok(count) if count > 0 => {
                    log::info!(
                        "Loaded {} ObjectCreationList definitions from {}",
                        count,
                        path.display()
                    );
                    return;
                }
                Ok(_) => {}
                Err(err) => {
                    log::warn!(
                        "Failed to load OCL definitions from {}: {}",
                        path.display(),
                        err
                    );
                }
            }
        }

        log::warn!("ObjectCreationList definitions could not be loaded from default paths");
    });
}

pub fn ensure_default_object_creation_lists_loaded() {
    let services = crate::system::engine_stores::active_services();
    if services.ocl_defaults_initialized().get().is_some() {
        return;
    }
    ensure_default_object_creation_lists_loaded_in(&services, &default_ocl_paths());
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "name_lookup_tests.rs"]
mod name_lookup_tests;

#[cfg(test)]
#[path = "live_creation_test_fixture.rs"]
mod live_creation_test_fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_creation_list::advanced_nuggets::{
        ApplyRandomForceNugget, AttackNugget, DeliverPayloadNugget, FireWeaponNugget,
    };
    use crate::object_creation_list::nuggets::{DebrisDisposition, GenericObjectCreationNugget};
    use std::path::PathBuf;

    #[test]
    fn default_ocls_load_once_per_world_and_preserve_existing_content() {
        use crate::system::engine_stores::WorldServices;
        let dir = std::env::temp_dir().join(format!(
            "generals-ocl-defaults-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ObjectCreationList.ini");
        let authored = "ObjectCreationList OCL_AuthoredWorld\n CreateObject\n ObjectNames = RealTank\n End\n End\n";
        fs::write(&path, authored).unwrap();
        let paths = [path];
        let first = WorldServices::new_for_world();
        let second = WorldServices::new_for_world();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| ensure_default_object_creation_lists_loaded_in(&first, &paths));
            }
        });
        let first_list = first.ocl_store().read().unwrap().as_ref().unwrap()
            .find_object_creation_list("OCL_AuthoredWorld").unwrap();
        assert_eq!(first_list.get_nugget_count(), 1);
        let nugget = ObjectCreationNugget::as_any(first_list.nuggets()[0].as_ref())
            .downcast_ref::<GenericObjectCreationNugget>().unwrap();
        assert_eq!(nugget.names, ["RealTank"]);
        assert_eq!(first.ocl_store().read().unwrap().as_ref().unwrap().get_nugget_count(), 1);
        first.ocl_store().write().unwrap().as_mut().unwrap().clear();
        ensure_default_object_creation_lists_loaded_in(&first, &paths);
        assert_eq!(first.ocl_store().read().unwrap().as_ref().unwrap().get_ocl_count(), 0);
        ensure_default_object_creation_lists_loaded_in(&second, &paths);
        assert!(second.ocl_store().read().unwrap().as_ref().unwrap()
            .find_object_creation_list("OCL_AuthoredWorld").is_some());
        drop(first);
        let third = WorldServices::new_for_world();
        ensure_default_object_creation_lists_loaded_in(&third, &paths);
        assert!(third.ocl_store().read().unwrap().as_ref().unwrap()
            .find_object_creation_list("OCL_AuthoredWorld").is_some());
        let existing = WorldServices::new_for_world();
        let mut store = ObjectCreationListStore::new();
        let parsed = parse_object_creation_lists_from_str(
            "ObjectCreationList OCL_AuthoredWorld\n CreateObject\n ObjectNames = ExistingTank\n End\n CreateObject\n ObjectNames = ExistingInfantry\n End\n End\n"
        ).unwrap();
        for (name, list) in parsed { store.register_ocl(name, list); }
        *existing.ocl_store().write().unwrap() = Some(store);
        ensure_default_object_creation_lists_loaded_in(&existing, &paths);
        assert_eq!(existing.ocl_store().read().unwrap().as_ref().unwrap()
            .find_object_creation_list("OCL_AuthoredWorld").unwrap().get_nugget_count(), 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_default_ocl_attempt_is_not_retried_in_same_world() {
        use crate::system::engine_stores::WorldServices;
        let dir = std::env::temp_dir().join(format!(
            "generals-ocl-failure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let paths = [dir.join("ObjectCreationList.ini")];
        let first = WorldServices::new_for_world();
        ensure_default_object_creation_lists_loaded_in(&first, &paths);
        fs::write(&paths[0], "ObjectCreationList OCL_AfterFailure\n CreateObject\n ObjectNames = Tank\n End\n End\n").unwrap();
        ensure_default_object_creation_lists_loaded_in(&first, &paths);
        assert!(first.ocl_store().read().unwrap().is_none());
        let second = WorldServices::new_for_world();
        ensure_default_object_creation_lists_loaded_in(&second, &paths);
        assert!(second.ocl_store().read().unwrap().as_ref().unwrap()
            .find_object_creation_list("OCL_AfterFailure").is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn test_ocl_creation() {
        let mut ocl = ObjectCreationList::new();
        assert_eq!(ocl.get_nugget_count(), 0);

        let nugget = Arc::new(GenericObjectCreationNugget::default());
        ocl.add_nugget(nugget);
        assert_eq!(ocl.get_nugget_count(), 1);
    }

    #[test]
    fn test_ocl_clear() {
        let mut ocl = ObjectCreationList::new();
        let nugget = Arc::new(GenericObjectCreationNugget::default());
        ocl.add_nugget(nugget);
        assert_eq!(ocl.get_nugget_count(), 1);

        ocl.clear();
        assert_eq!(ocl.get_nugget_count(), 0);
    }

    #[test]
    fn test_store_basic() {
        let mut store = ObjectCreationListStore::new();
        assert_eq!(store.get_ocl_count(), 0);

        let ocl = ObjectCreationList::new();
        store.register_ocl("TestOCL".to_string(), ocl);
        assert_eq!(store.get_ocl_count(), 1);

        assert!(store.find_object_creation_list("TestOCL").is_some());
        assert!(store.find_object_creation_list("None").is_none());
        assert!(store.find_object_creation_list("NotFound").is_none());
    }

    #[test]
    fn test_store_add_nugget() {
        let mut store = ObjectCreationListStore::new();
        let nugget = Arc::new(GenericObjectCreationNugget::default());

        store.add_nugget(nugget);
        assert_eq!(store.get_nugget_count(), 1);
    }

    #[test]
    fn test_global_store_init() {
        init_object_creation_list_store();
        let store_cell = get_object_creation_list_store();
        let store = store_cell.read().unwrap();
        assert!(store.is_some());
    }

    #[test]
    fn test_parse_object_creation_list_from_str() {
        let data = r#"
ObjectCreationList OCL_Test
  CreateObject
    ObjectNames = UnitA UnitB
    Count = 2
    Disposition = ON_GROUND_ALIGNED INHERIT_VELOCITY
  End
End
"#;

        let count = load_object_creation_lists_from_str(data).expect("failed to parse OCL");
        assert_eq!(count, 1);

        let store_cell = get_object_creation_list_store();
        let store = store_cell.read().unwrap();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_Test"))
            .expect("OCL_Test missing after parse");
        assert_eq!(ocl.get_nugget_count(), 1);
    }

    #[test]
    fn explicit_zero_count_is_not_replaced_by_the_default() {
        let parsed = parse_object_creation_lists_from_ini(
            "ObjectCreationList Zero\nCreateObject\nObjectNames = UnitA\nCount = 0\nEnd\nEnd\nObjectCreationList Default\nCreateObject\nObjectNames = UnitB\nEnd\nEnd\n",
        )
        .unwrap();
        let count_for = |name: &str| {
            let (_, list) = parsed
                .iter()
                .find(|(candidate, _)| candidate == name)
                .unwrap();
            // Dispatch through the trait object, not the Box's own Any.
            let nugget = ObjectCreationNugget::as_any(list.nuggets()[0].as_ref())
                .downcast_ref::<GenericObjectCreationNugget>()
                .unwrap();
            nugget.debris_to_generate
        };
        assert_eq!(count_for("Zero"), 0);
        assert_eq!(count_for("Default"), 1);
    }

    #[test]
    fn test_parse_advanced_nuggets_and_nested_delivery_decal() {
        let data = r#"
ObjectCreationList OCL_Advanced
  DeliverPayload
    DeliveryDecal
      Texture = SCCNukeLaunchDecal
      Style = SHADOW_ALPHA_DECAL
    End
    Transport = AmericaJetAurora
    Payload = AmericaJetAuroraBomb 1
  End
  FireWeapon
    Weapon = DemoTrapDetonationWeapon
  End
  Attack
    NumberOfShots = 1
    WeaponSlot = PRIMARY
    DeliveryDecal
      Texture = SCCNukeLaunchDecal
      Style = SHADOW_ALPHA_DECAL
    End
  End
  ApplyRandomForce
    SpinRate = 1.0
    MinForceMagnitude = 10.0
    MaxForceMagnitude = 20.0
  End
End
"#;

        let count =
            load_object_creation_lists_from_str(data).expect("failed to parse advanced OCL");
        assert_eq!(count, 1);

        let store_cell = get_object_creation_list_store();
        let store = store_cell.read().unwrap();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_Advanced"))
            .expect("OCL_Advanced missing after parse");
        assert_eq!(ocl.get_nugget_count(), 4);
    }

    #[test]
    fn test_parse_create_object_fire_weapon_attack_force_and_payload_fields() {
        let _guard = crate::test_sync::lock();
        let data = r#"
ObjectCreationList OCL_AllNuggetHeaders
  CreateObject
    ObjectNames = AmericaInfantryRanger
    Count = 2
    Disposition = ON_GROUND_ALIGNED
  End
  FireWeapon
    Weapon = NeutronMissileWeapon
  End
  Attack
    NumberOfShots = 9
    WeaponSlot = PRIMARY
    DeliveryDecalRadius = 200
    DeliveryDecal
      Texture = SCCScudStorm_GLA
      Style = SHADOW_ALPHA_DECAL
    End
  End
  ApplyRandomForce
    SpinRate = 120
    MinForceMagnitude = 60
    MaxForceMagnitude = 100
    MinForcePitch = 70
    MaxForcePitch = 90
  End
  DeliverPayload
    Transport = AmericaJetAurora
    StartAtPreferredHeight = Yes
    StartAtMaxSpeed = No
    FormationSize = 3
    FormationSpacing = 25
    WeaponConvergenceFactor = 0.5
    WeaponErrorRadius = 12
    DelayDeliveryMax = 1000
    Payload = AmericaJetAuroraBomb 2
    PutInContainer = AmericaParachute
    DeliveryDistance = 250
    DeliveryDecalRadius = 150
    DeliveryDecal
      Texture = SCCA10Strike_USA
      Style = SHADOW_ALPHA_DECAL
    End
  End
End
"#;

        let count =
            load_object_creation_lists_from_str(data).expect("failed to parse mixed OCL headers");
        assert_eq!(count, 1);

        let store_cell = get_object_creation_list_store();
        let store = store_cell.read().unwrap();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_AllNuggetHeaders"))
            .expect("OCL_AllNuggetHeaders missing after parse");
        assert_eq!(ocl.get_nugget_count(), 5);

        let create = ObjectCreationNugget::as_any(ocl.nuggets()[0].as_ref())
            .downcast_ref::<GenericObjectCreationNugget>()
            .expect("CreateObject nugget");
        assert_eq!(create.names, vec!["AmericaInfantryRanger".to_string()]);
        assert_eq!(create.debris_to_generate, 2);
        assert!(create.disposition.has(DebrisDisposition::ON_GROUND_ALIGNED));

        let fire = ObjectCreationNugget::as_any(ocl.nuggets()[1].as_ref())
            .downcast_ref::<FireWeaponNugget>()
            .expect("FireWeapon nugget");
        assert_eq!(fire.weapon.as_deref(), Some("NeutronMissileWeapon"));

        let attack = ObjectCreationNugget::as_any(ocl.nuggets()[2].as_ref())
            .downcast_ref::<AttackNugget>()
            .expect("Attack nugget");
        assert_eq!(attack.number_of_shots, 9);
        assert_eq!(attack.weapon_slot, crate::weapon::WeaponSlotType::Primary);
        assert_eq!(attack.delivery_decal_radius, 200.0);
        assert_eq!(
            attack.delivery_decal_template.texture_name.as_str(),
            "SCCScudStorm_GLA"
        );
        assert_eq!(
            attack.delivery_decal_template.shadow_type,
            SHADOW_ALPHA_DECAL
        );

        let force = ObjectCreationNugget::as_any(ocl.nuggets()[3].as_ref())
            .downcast_ref::<ApplyRandomForceNugget>()
            .expect("ApplyRandomForce nugget");
        assert!((force.min_mag - 60.0).abs() < f32::EPSILON);
        assert!((force.max_mag - 100.0).abs() < f32::EPSILON);
        let expected_spin =
            game_engine::common::ini::INI::parse_angular_velocity_real("120").unwrap();
        let expected_min_pitch = game_engine::common::ini::INI::parse_angle_real("70").unwrap();
        let expected_max_pitch = game_engine::common::ini::INI::parse_angle_real("90").unwrap();
        assert!((force.spin_rate - expected_spin).abs() < 1e-6);
        assert!((force.min_pitch - expected_min_pitch).abs() < 1e-6);
        assert!((force.max_pitch - expected_max_pitch).abs() < 1e-6);

        let payload = ObjectCreationNugget::as_any(ocl.nuggets()[4].as_ref())
            .downcast_ref::<DeliverPayloadNugget>()
            .expect("DeliverPayload nugget");
        assert_eq!(payload.transport_name, "AmericaJetAurora");
        assert!(payload.start_at_preferred_height);
        assert!(!payload.start_at_max_speed);
        assert_eq!(payload.formation_size, 3);
        assert!((payload.formation_spacing - 25.0).abs() < f32::EPSILON);
        assert!((payload.convergence_factor - 0.5).abs() < f32::EPSILON);
        assert!((payload.error_radius - 12.0).abs() < f32::EPSILON);
        assert_eq!(payload.delay_delivery_frames_max, 30);
        assert_eq!(payload.payload.len(), 1);
        assert_eq!(payload.payload[0].payload_name, "AmericaJetAuroraBomb");
        assert_eq!(payload.payload[0].payload_count, 2);
        assert_eq!(payload.put_in_container_name, "AmericaParachute");
        assert!((payload.data.dist_to_target - 250.0).abs() < f32::EPSILON);
        assert!((payload.data.delivery_decal_radius - 150.0).abs() < f32::EPSILON);
        assert_eq!(
            payload.data.delivery_decal_template.texture_name.as_str(),
            "SCCA10Strike_USA"
        );

        let fixture = super::live_creation_test_fixture::LiveCreationFixture::new()
            .expect("scoped live OCL fixture");
        let live = crate::object_creation_list::live_creation_context();
        let ctx = crate::object_creation_list::CreationContext {
            game_logic: live.game_logic,
            thing_factory: &fixture,
            terrain_logic: live.terrain_logic,
        };
        let obj = crate::object::Object::new_test(501, 100.0);
        let primary = Coord3D::new(0.0, 0.0, 0.0);
        let secondary = Coord3D::new(10.0, 20.0, 0.0);
        // Live factory may lack templates; Neutral fallback means we no longer
        // skip solely because the source has no controlling player.
        let _ = ocl.create_with_angle(&ctx, Some(&obj), &primary, &secondary, 0.0, 0);
        let _ = ocl.create_with_objects(&ctx, &obj, None, 0);
        fixture.retire().expect("exact live OCL fixture retirement");
    }

    #[test]
    fn test_parse_stock_object_creation_list_file_when_present() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..");
        let stock =
            root.join("windows_game/extracted_big_files/INIZH/Data/INI/ObjectCreationList.ini");
        if !stock.exists() {
            return;
        }

        let count = load_object_creation_lists_from_path(&stock)
            .expect("failed to parse stock ObjectCreationList.ini");
        assert!(
            count >= 200,
            "expected at least 200 OCLs from stock data, got {}",
            count
        );
    }

    #[test]
    fn test_incremental_load_keeps_existing_ocls() {
        let a = r#"
ObjectCreationList OCL_A
  CreateObject
    ObjectNames = UnitA
  End
End
"#;
        let b = r#"
ObjectCreationList OCL_B
  CreateObject
    ObjectNames = UnitB
  End
End
"#;

        let count_a = load_object_creation_lists_from_str(a).expect("load A failed");
        assert_eq!(count_a, 1);
        let count_b = load_object_creation_lists_from_str(b).expect("load B failed");
        assert_eq!(count_b, 1);

        let store_cell = get_object_creation_list_store();
        let store = store_cell.read().unwrap();
        let store = store.as_ref().expect("store missing");
        assert!(store.find_object_creation_list("OCL_A").is_some());
        assert!(store.find_object_creation_list("OCL_B").is_some());
    }

    #[test]
    fn test_reload_same_ocl_replaces_nuggets() {
        let first = r#"
ObjectCreationList OCL_Reload
  CreateObject
    ObjectNames = UnitA
  End
End
"#;
        let second = r#"
ObjectCreationList OCL_Reload
  CreateObject
    ObjectNames = UnitA
  End
  CreateObject
    ObjectNames = UnitB
  End
End
"#;

        load_object_creation_lists_from_str(first).expect("first load failed");
        load_object_creation_lists_from_str(second).expect("second load failed");

        let store_cell = get_object_creation_list_store();
        let store = store_cell.read().unwrap();
        let ocl = store
            .as_ref()
            .and_then(|s| s.find_object_creation_list("OCL_Reload"))
            .expect("OCL_Reload missing after reload");
        assert_eq!(ocl.get_nugget_count(), 2);
    }

    #[test]
    fn test_helper_ensure_ocl_lookup_does_not_create_placeholder() {
        let missing = "OCL_Missing_ParityTest_20260302_Batch83";
        assert!(
            crate::helpers::TheObjectCreationListStore::find_object_creation_list(missing)
                .is_none()
        );
        assert!(
            crate::helpers::TheObjectCreationListStore::ensure_object_creation_list(missing)
                .is_none()
        );
        assert!(
            crate::helpers::TheObjectCreationListStore::find_object_creation_list(missing)
                .is_none()
        );
    }
}

pub fn parse_object_creation_lists_from_ini(
    content: &str,
) -> Result<Vec<(String, ObjectCreationList)>, String> {
    bind_legacy_lists(generals_sim::object_creation_list::parse_object_creation_lists_from_ini(content))
}

#[cfg(test)]
#[path = "owned_content_tests.rs"]
mod owned_content_tests;
