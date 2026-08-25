////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// Asset manager - coordinates all asset loading systems

use crate::assets::ini_parser::{AuthoredConditionModelSelection, AuthoredDrawModel};
use crate::assets::{
    archive::{ArchiveFileSystem, ArchiveStatistics},
    audio::AudioManager,
    models::{
        W3DLoader, W3DMesh, W3DModel, W3dAnimation, W3dAnimationBinding, get_common_cnc_units,
        split_w3d_draw_animation_identity,
    },
    textures::{GPUTexture, RawTexture, TextureManager},
    ww3d_asset_manager::WW3DAssetManager,
};
use crate::localization;
use anyhow::{Result, anyhow};
use log::{debug, error, info, warn};
use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Complete asset management system for C&C Generals
pub struct AssetManager {
    /// Archive file system for reading BIG files
    archive_system: ArchiveFileSystem,
    /// Audio manager for music and sound effects
    audio_manager: AudioManager,
    /// W3D model loader
    model_loader: W3DLoader,
    /// Texture manager
    texture_manager: TextureManager,
    /// WW3D Asset Manager for object definitions and texture lookup
    ww3d_manager: WW3DAssetManager,
    /// Cache of loaded models
    model_cache: HashMap<String, W3DModel>,
    /// C++ `WW3DAssetManager`-style source records for render-object names.
    ///
    /// This deliberately remains separate from `model_cache`: ordinary whole-file
    /// model loads are allowed to use presentation aliases, whereas an external
    /// HLOD child must resolve only a source-authored full prototype identity.
    w3d_render_object_prototypes: W3dRenderObjectPrototypeRegistry,
    /// Validated C++ `Drawable::getBarrelCount` answers keyed by the exact
    /// Object INI identity and active ModelCondition bit bank.  Entries are
    /// populated only from models which are already resident in
    /// [`Self::model_cache`]; combat callers must never turn a barrel lookup
    /// into archive I/O or a renderer-to-GameLogic mutation.
    weapon_barrel_count_cache: HashMap<WeaponBarrelCountCacheKey, [Option<u8>; 3]>,
    /// Exact raw-animation companions keyed by the full frozen Draw identity
    /// and the compatible source hierarchy. `None` memoizes a failed exact
    /// prewarm so render collection neither retries archive I/O nor selects a
    /// local clip by ordinal.
    companion_animation_cache: HashMap<CompanionAnimationCacheKey, Option<Arc<W3dAnimation>>>,
    /// Known-missing model keys to keep repeated lookups O(1) like C++ hash misses.
    missing_model_keys: HashSet<String>,
    /// Initialization status
    initialized: bool,
    /// Active localization language
    language: String,
    /// Active mod root (if any)
    active_mod_path: Option<PathBuf>,
    /// Explicit BIG files to mount after core init
    manual_big_files: Vec<PathBuf>,
}

/// C++ HAnim assets are shared by their fully-qualified `Hierarchy.Animation`
/// identity, but a geometry HTree is only allowed to bind a motion authored
/// for that exact hierarchy. A model basename is deliberately not part of the
/// cache contract: two source models may share a hierarchy, while one basename
/// can appear under several draw modules.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompanionAnimationCacheKey {
    hierarchy_name: String,
    draw_identity: String,
}

/// Stable cache identity for one exact source Draw-state selection.
///
/// A model basename is intentionally insufficient: an Object can select a
/// different set and ordering of Draw modules when its ModelCondition flags
/// change, and C++ [`Drawable::getBarrelCount`] returns the first nonzero
/// answer in that declaration order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct WeaponBarrelCountCacheKey {
    object_name: String,
    condition_bits: u128,
}

/// One source-renderable C++ `PrototypeClass` kind retained by `W3DModel`.
///
/// The indices are immutable source indices into the model returned by
/// [`AssetManager::cached_w3d_render_object_source_model`]. They are source
/// definitions, not renderer submission commands: HMODEL owns an independent
/// HTree and expands only through the aggregate renderer's rigid-node path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3dRenderObjectPrototypeKind {
    /// C++ `MeshLoaderClass` / `PrimitivePrototypeClass` source mesh.
    Mesh { mesh_index: usize },
    /// C++ `HLodLoaderClass` source HLOD definition.
    Hlod { hlod_index: usize },
    /// C++ `HModelLoaderClass` hierarchical model definition.
    Hmodel { hmodel_index: usize },
    /// C++ `ParticleEmitterLoaderClass`.
    Emitter { emitter_index: usize },
    /// C++ `DazzleLoaderClass`.
    Dazzle { dazzle_index: usize },
    /// C++ `BoxLoaderClass` / `CLASSID_OBBOX` or `CLASSID_AABOX`.
    Box { box_index: usize },
    /// C++ `RingLoaderClass`.
    Ring { ring_index: usize },
    /// C++ `SphereLoaderClass`.
    Sphere { sphere_index: usize },
    /// C++ `NullLoaderClass`.
    Null { null_index: usize },
    /// C++ `CollectionLoaderClass`.
    Collection { collection_index: usize },
    /// C++ `AggregateLoaderClass`.
    Aggregate { aggregate_index: usize },
    /// C++ `DistLODLoaderClass`.
    DistLod { dist_lod_index: usize },
}

/// Immutable source metadata for one exact C++ `PrototypeClass` name.
///
/// `WW3DAssetManager::Find_Prototype` compares the complete name
/// case-insensitively. This token preserves that full source spelling and
/// points only at the strict source model that registered it; it cannot select
/// a presentation alias, an aggregate parent, or a placeholder mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dRenderObjectPrototype {
    full_name: String,
    source_file_stem: String,
    source_model_key: String,
    kind: W3dRenderObjectPrototypeKind,
}

impl W3dRenderObjectPrototype {
    /// Original source spelling of the complete prototype identity.
    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    /// Exact W3D file stem that populated this source record.
    pub fn source_file_stem(&self) -> &str {
        &self.source_file_stem
    }

    /// Renderable source representation retained by Main.
    pub fn kind(&self) -> W3dRenderObjectPrototypeKind {
        self.kind
    }
}

/// Strict, source-backed subset of C++ `WW3DAssetManager`'s prototype table.
///
/// The normal `AssetManager::model_cache` intentionally does not feed this
/// table: it can contain presentation-model aliases. Each entry here is loaded
/// through the exact C++ first-dot filename rule and is immutable after its
/// source file has registered, matching C++'s first-name-wins collision rule.
#[derive(Default)]
struct W3dRenderObjectPrototypeRegistry {
    prototypes: HashMap<String, W3dRenderObjectPrototype>,
    source_models: HashMap<String, W3DModel>,
}

impl W3dRenderObjectPrototypeRegistry {
    fn lookup(&self, full_name: &str) -> Option<W3dRenderObjectPrototype> {
        let key = w3d_render_object_identity_key(full_name)?;
        self.prototypes.get(&key).cloned()
    }

    fn source_model(&self, prototype: &W3dRenderObjectPrototype) -> Option<&W3DModel> {
        let key = w3d_render_object_identity_key(&prototype.full_name)?;
        let registered = self.prototypes.get(&key)?;
        (registered == prototype)
            .then(|| self.source_models.get(&prototype.source_model_key))
            .flatten()
    }

    fn source_stem_is_loaded(&self, source_stem: &str) -> bool {
        w3d_render_object_identity_key(source_stem)
            .is_some_and(|source_key| self.source_models.contains_key(&source_key))
    }

    fn register_source_model(&mut self, source_stem: &str, model: W3DModel) {
        let Some(source_model_key) = w3d_render_object_identity_key(source_stem) else {
            return;
        };

        // C++ holds the first loaded source and refuses later duplicate
        // prototype names. Do not mutate either source records or their model
        // topology on a repeated request.
        if self.source_models.contains_key(&source_model_key) {
            return;
        }

        for (mesh_index, mesh) in model.meshes.iter().enumerate() {
            let Some(full_name) = w3d_mesh_prototype_name(mesh) else {
                continue;
            };
            self.register_prototype(
                full_name,
                source_stem,
                &source_model_key,
                W3dRenderObjectPrototypeKind::Mesh { mesh_index },
            );
        }

        for (hlod_index, hlod) in model.hlods.iter().enumerate() {
            if hlod.name.is_empty() {
                continue;
            }
            self.register_prototype(
                hlod.name.clone(),
                source_stem,
                &source_model_key,
                W3dRenderObjectPrototypeKind::Hlod { hlod_index },
            );
        }

        for (hmodel_index, hmodel) in model.hmodels.iter().enumerate() {
            if hmodel.name.is_empty() {
                continue;
            }
            self.register_prototype(
                hmodel.name.clone(),
                source_stem,
                &source_model_key,
                W3dRenderObjectPrototypeKind::Hmodel { hmodel_index },
            );
        }

        for (full_name, extra) in crate::assets::models::extra_prototypes(&model) {
            let kind = match extra {
                crate::assets::models::W3dExtraPrototypeKind::Emitter { emitter_index } => {
                    W3dRenderObjectPrototypeKind::Emitter { emitter_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Dazzle { dazzle_index } => {
                    W3dRenderObjectPrototypeKind::Dazzle { dazzle_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Box { box_index } => {
                    W3dRenderObjectPrototypeKind::Box { box_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Ring { ring_index } => {
                    W3dRenderObjectPrototypeKind::Ring { ring_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Sphere { sphere_index } => {
                    W3dRenderObjectPrototypeKind::Sphere { sphere_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Null { null_index } => {
                    W3dRenderObjectPrototypeKind::Null { null_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Collection { collection_index } => {
                    W3dRenderObjectPrototypeKind::Collection { collection_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::Aggregate { aggregate_index } => {
                    W3dRenderObjectPrototypeKind::Aggregate { aggregate_index }
                }
                crate::assets::models::W3dExtraPrototypeKind::DistLod { dist_lod_index } => {
                    W3dRenderObjectPrototypeKind::DistLod { dist_lod_index }
                }
            };
            self.register_prototype(full_name, source_stem, &source_model_key, kind);
        }

        self.source_models.insert(source_model_key, model);
    }

    fn register_prototype(
        &mut self,
        full_name: String,
        source_file_stem: &str,
        source_model_key: &str,
        kind: W3dRenderObjectPrototypeKind,
    ) {
        let Some(key) = w3d_render_object_identity_key(&full_name) else {
            return;
        };

        // `Load_Prototype` calls `Render_Obj_Exists` before `Add_Prototype`.
        // Preserve the first exact source record rather than silently replacing
        // it with a later W3D file's conflicting definition.
        self.prototypes
            .entry(key)
            .or_insert(W3dRenderObjectPrototype {
                full_name,
                source_file_stem: source_file_stem.to_string(),
                source_model_key: source_model_key.to_string(),
                kind,
            });
    }
}

impl WeaponBarrelCountCacheKey {
    fn new(object_name: &str, condition_bits: u128) -> Option<Self> {
        let object_name = object_name.trim();
        (!object_name.is_empty()).then(|| Self {
            object_name: object_name.to_ascii_lowercase(),
            condition_bits,
        })
    }
}

impl CompanionAnimationCacheKey {
    fn for_geometry_model(model: &W3DModel, identity: &str) -> Option<Self> {
        let hierarchy_name = model.hierarchy.as_ref()?.name.trim();
        let (requested_hierarchy, _) = split_w3d_draw_animation_identity(identity)?;
        if hierarchy_name.is_empty() || !hierarchy_name.eq_ignore_ascii_case(requested_hierarchy) {
            return None;
        }
        Some(Self {
            hierarchy_name: hierarchy_name.to_ascii_lowercase(),
            draw_identity: identity.trim().to_ascii_lowercase(),
        })
    }
}

/// C++ `stricmp` key for a full source prototype identity.
///
/// W3D names are ASCII records. Deliberately do not trim or strip a suffix:
/// whitespace and every character after a dot participate in `Find_Prototype`.
fn w3d_render_object_identity_key(full_name: &str) -> Option<String> {
    (!full_name.is_empty() && !full_name.as_bytes().contains(&0))
        .then(|| full_name.to_ascii_lowercase())
}

/// C++ `Create_Render_Obj`'s load-on-demand filename rule.
///
/// `strchr(name, '.')` selects the first dot, not the last one. Source render
/// object names are not filesystem paths, so reject path separators before an
/// archive request rather than broadening a missing prototype into arbitrary
/// file access.
fn w3d_render_object_source_stem(full_name: &str) -> Option<&str> {
    w3d_render_object_identity_key(full_name)?;
    let source_stem = full_name
        .split_once('.')
        .map_or(full_name, |(stem, _)| stem);
    (!source_stem.is_empty()
        && !source_stem.contains('/')
        && !source_stem.contains('\\')
        && !source_stem.as_bytes().contains(&0))
    .then_some(source_stem)
}

/// `MeshClass::Load_W3D` builds a prototype name from the exact source
/// `ContainerName`, a dot only when that container is nonempty, and `MeshName`.
fn w3d_mesh_prototype_name(mesh: &W3DMesh) -> Option<String> {
    if mesh.name.is_empty() || mesh.name.as_bytes().contains(&0) {
        return None;
    }

    if mesh.container_name.is_empty() {
        Some(mesh.name.clone())
    } else if mesh.container_name.as_bytes().contains(&0) {
        None
    } else {
        Some(format!("{}.{}", mesh.container_name, mesh.name))
    }
}

/// Archive storage spellings for one exact C++ source filename.
///
/// The stem is never remapped to a presentation alias or a retail filename
/// table. Case-only variants preserve C++'s case-insensitive file lookup on
/// case-sensitive extracted trees; `art/w3d` spellings are storage locations,
/// not alternative asset identities.
fn exact_w3d_render_object_archive_paths(source_stem: &str) -> Vec<String> {
    let mut stems = Vec::new();
    for stem in [
        source_stem.to_string(),
        source_stem.to_ascii_lowercase(),
        source_stem.to_ascii_uppercase(),
    ] {
        if !stems.iter().any(|existing: &String| existing == &stem) {
            stems.push(stem);
        }
    }

    let mut paths = Vec::new();
    let mut push_unique = |path: String| {
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    };
    for stem in stems {
        // Preserve the two C++ Create_Render_Obj lookup locations before
        // Main's archive/extract storage-path spellings.
        push_unique(format!("{stem}.w3d"));
        push_unique(format!("../{stem}.w3d"));
        push_unique(format!("art/w3d/{stem}.w3d"));
        push_unique(format!("Art/W3D/{stem}.w3d"));
        push_unique(format!("Art/W3D/{stem}.W3D"));
        push_unique(format!("art/w3d/{stem}.W3D"));
    }
    paths
}

/// Load one W3D source without the presentation aliases accepted by the
/// ordinary whole-file model loader.
fn load_exact_w3d_render_object_source(
    model_loader: &W3DLoader,
    archive_system: &mut ArchiveFileSystem,
    source_stem: &str,
) -> Result<W3DModel> {
    let mut last_open_error = None;
    for archive_path in exact_w3d_render_object_archive_paths(source_stem) {
        let mut reader = match archive_system.open_reader(&archive_path) {
            Ok(reader) => reader,
            Err(error) => {
                last_open_error = Some(error.to_string());
                continue;
            }
        };

        // Once C++ finds a source file it loads that file, rather than using a
        // later alias/path as a fallback for malformed prototype contents.
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).map_err(|error| {
            anyhow!("failed reading strict W3D source '{archive_path}': {error}")
        })?;
        return model_loader
            .load_model_from_bytes(&bytes, source_stem)
            .map_err(|error| {
                anyhow!("failed parsing strict W3D source '{archive_path}': {error}")
            });
    }

    Err(anyhow!(
        "strict W3D source '{}.w3d' was not found: {}",
        source_stem,
        last_open_error.unwrap_or_else(|| "no archive candidates".to_string())
    ))
}

/// Resolve a full prototype identity, loading only its first-dot source stem
/// and then re-looking up the same full identity exactly as C++ does.
fn resolve_w3d_render_object_prototype_with_loader<F>(
    registry: &mut W3dRenderObjectPrototypeRegistry,
    full_name: &str,
    mut load_source: F,
) -> Option<W3dRenderObjectPrototype>
where
    F: FnMut(&str) -> Option<W3DModel>,
{
    if let Some(prototype) = registry.lookup(full_name) {
        return Some(prototype);
    }

    let source_stem = w3d_render_object_source_stem(full_name)?;
    if !registry.source_stem_is_loaded(source_stem) {
        let source_model = load_source(source_stem)?;
        registry.register_source_model(source_stem, source_model);
    }

    // A loaded file is not itself a render object. The request succeeds only
    // if it registered this original full name.
    registry.lookup(full_name)
}

/// Reconstruct the one answer returned by C++ `Drawable::getBarrelCount` for
/// each WeaponSet slot from an already-selected source Draw-module sequence.
///
/// The caller supplies only models already resident in the AssetManager cache.
/// If an earlier Draw module cannot be validated, the whole lookup is unknown:
/// skipping it and accepting a later module would change C++'s first-nonzero
/// ordering.  Conversely, a validated module with no barrel for a slot is a
/// known zero and permits the next declared module to answer that slot.
fn cached_weapon_barrel_counts_for_draw_models<'a, F>(
    draw_models: &[AuthoredDrawModel],
    mut cached_model: F,
) -> Option<[Option<u8>; 3]>
where
    F: FnMut(&str) -> Option<&'a W3DModel>,
{
    let mut counts = [None; 3];

    for draw_model in draw_models {
        // Once every slot has a first nonzero source answer, later modules
        // cannot affect C++ Drawable::getBarrelCount and need not be resident.
        if counts.iter().all(Option::is_some) {
            break;
        }

        let bindings = &draw_model.weapon_bone_bindings;
        if !bindings.source_fields_valid {
            return None;
        }

        // A source state with no base for an unresolved slot is a proven zero
        // for that Draw module; C++ needs no hierarchy lookup before its
        // `m_weaponBarrelInfoVec[slot]` stays empty. Do not unnecessarily
        // require/cache a model when every remaining slot is in that state.
        let unresolved_slot_has_source_base = counts.iter().enumerate().any(|(slot, count)| {
            if count.is_some() {
                return false;
            }
            let source = &bindings.slots[slot];
            source.fire_fx_bone_base.is_some()
                || source.recoil_bone_base.is_some()
                || source.muzzle_flash_bone_base.is_some()
                || source.launch_bone_base.is_some()
        });
        if !unresolved_slot_has_source_base {
            continue;
        }

        let model_key = draw_model.model_key.trim();
        if model_key.is_empty() {
            return None;
        }
        let model = cached_model(model_key)?;
        let topology = model.weapon_barrel_topology_for_authored_bindings(bindings)?;

        for (slot, count) in counts.iter_mut().enumerate() {
            if count.is_none() {
                *count = topology.barrel_count(slot as u8);
            }
        }
    }

    Some(counts)
}

/// Summary of a model warmup pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ModelPrewarmStats {
    pub requested: usize,
    pub cache_hits: usize,
    pub resolved: usize,
    pub missing: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextureUpdateEntry {
    tga_path: PathBuf,
    dds_path: PathBuf,
    tga_modified: SystemTime,
    dds_modified: Option<SystemTime>,
}

fn texture_update_should_skip(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    lower.contains("caust") || lower.contains("zhca")
}

fn texture_update_entry_needs_rebuild(entry: &TextureUpdateEntry) -> bool {
    match entry.dds_modified {
        None => true,
        Some(dds_modified) => matches!(
            entry.tga_modified.partial_cmp(&dds_modified),
            Some(CmpOrdering::Greater)
        ),
    }
}

fn select_tga_to_dds_entries(entries: Vec<TextureUpdateEntry>) -> Vec<TextureUpdateEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            !texture_update_should_skip(&entry.tga_path)
                && texture_update_entry_needs_rebuild(entry)
        })
        .collect()
}

fn texture_dds_path(tga_path: &Path) -> PathBuf {
    tga_path.with_extension("dds")
}

fn normalize_archive_filename(value: &str) -> String {
    value
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase()
}

fn archive_filename_matches(actual: Option<&str>, expected: &str) -> bool {
    actual
        .map(|actual| normalize_archive_filename(actual) == normalize_archive_filename(expected))
        .unwrap_or(false)
}

impl AssetManager {
    fn should_resolve_object_texture_name(name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
        }

        let has_path = trimmed.contains('/') || trimmed.contains('\\');
        let has_extension = Path::new(trimmed).extension().is_some();
        !has_path && !has_extension
    }

    fn canonical_model_name(model_name: &str) -> String {
        model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D")
            .to_string()
    }

    fn resolve_available_model_name(&self, model_name: &str) -> String {
        // C++ parity: request the model name as-authored instead of fuzzy suffix/alias remaps.
        Self::canonical_model_name(model_name)
    }

    /// Create new asset manager
    pub fn new() -> Result<Self> {
        debug!("Creating AssetManager");

        let (language, active_mod_path) = Self::runtime_overrides();

        Ok(Self {
            archive_system: ArchiveFileSystem::new(),
            audio_manager: AudioManager::new()?,
            model_loader: W3DLoader::new(),
            texture_manager: TextureManager::new(),
            ww3d_manager: WW3DAssetManager::new(),
            model_cache: HashMap::new(),
            w3d_render_object_prototypes: W3dRenderObjectPrototypeRegistry::default(),
            weapon_barrel_count_cache: HashMap::new(),
            companion_animation_cache: HashMap::new(),
            missing_model_keys: HashSet::new(),
            initialized: false,
            language,
            active_mod_path,
            manual_big_files: Vec::new(),
        })
    }

    /// Initialize the asset manager
    pub async fn init(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        debug!("Initializing AssetManager");

        // Add asset search paths before initializing
        // Try to find the assets directory relative to the executable
        let mut asset_paths = vec![
            PathBuf::from("assets"),
            PathBuf::from("Code/Main/assets"),
            PathBuf::from("./Code/Main/assets"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        ];

        // Also try relative to current exe directory
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                asset_paths.push(exe_dir.join("assets"));
                asset_paths.push(exe_dir.join("../Code/Main/assets"));
                asset_paths.push(exe_dir.join("Data"));
            }
        }

        if let Some(mod_path) = &self.active_mod_path {
            asset_paths.insert(0, mod_path.clone());
            asset_paths.insert(1, mod_path.join("Data"));
        }

        for language_path in self.language_specific_paths() {
            asset_paths.insert(0, language_path);
        }

        self.register_search_paths(asset_paths);

        // Initialize archive system (loads BIG files)
        if let Err(e) = self.archive_system.init().await {
            warn!(
                "Failed to initialize archive system: {}. Continuing without archives.",
                e
            );
        }

        if let Err(e) = self.load_manual_archives().await {
            warn!(
                "Failed to load manual archives: {}. Continuing without them.",
                e
            );
        }
        if let Err(e) = self.run_startup_maintenance() {
            warn!("Startup maintenance failed: {}. Continuing.", e);
        }

        // Initialize texture manager with MAGENTA fallback for missing textures
        if let Err(e) = self.texture_manager.init(device, queue) {
            warn!(
                "Failed to initialize texture manager: {}. Continuing without GPU textures.",
                e
            );
        }

        // Initialize WW3D Asset Manager - Load object definitions from INIZH.big
        // This matches C++ WW3DAssetManager initialization
        info!("🎮 Initializing WW3D Asset Manager for object definitions and texture lookup");
        let init_start = SystemTime::now();
        if let Err(e) = self.ww3d_manager.initialize(&mut self.archive_system).await {
            warn!(
                "Failed to initialize WW3D asset manager: {}. Continuing without object definitions.",
                e
            );
        }
        let init_elapsed = init_start.elapsed().unwrap_or_default();
        info!(
            "✅ WW3D Asset Manager initialized in {:.2}s with {} object definitions",
            init_elapsed.as_secs_f64(),
            self.ww3d_manager.object_count()
        );

        // Initialize GameLogic weapon store so templates can be registered
        // This must happen before INI template loading below.
        if let Err(e) = gamelogic::initialize_weapon_store() {
            warn!("Failed to initialize GameLogic weapon store: {}", e);
        }

        // Load weapon, upgrade, and science templates from BIG archives.
        // Matches C++ INI loading order: weapons, upgrades, sciences.
        info!("📋 Loading INI templates (weapons, upgrades, sciences) from BIG archives");
        let template_load_start = SystemTime::now();
        match crate::assets::ini_template_loader::load_all_ini_templates(&mut self.archive_system)
            .await
        {
            Ok(stats) => {
                let elapsed = template_load_start.elapsed().unwrap_or_default();
                info!(
                    "✅ INI templates loaded in {:.2}s: {} weapons, {} upgrades, {} sciences",
                    elapsed.as_secs_f64(),
                    stats.weapons_loaded,
                    stats.upgrades_loaded,
                    stats.sciences_loaded
                );
            }
            Err(e) => {
                warn!("INI template loading failed: {}", e);
            }
        }

        self.initialized = true;

        // Print statistics
        let stats = self.get_statistics();
        info!("AssetManager initialized successfully!");
        info!("  Archives: {}", stats.archive_stats.total_archives);
        info!("  Total files: {}", stats.archive_stats.total_files);
        info!("  Unique files: {}", stats.archive_stats.unique_files);
        info!("  Textures cached: {}", stats.textures_cached);
        info!("  Models cached: {}", stats.models_cached);

        Ok(())
    }

    fn run_startup_maintenance(&self) -> Result<()> {
        self.maybe_update_tga_to_dds();
        self.verify_release_fingerprints()
    }

    fn maybe_update_tga_to_dds(&self) {
        let update_requested = {
            let global = game_engine::common::global_data::read();
            global.writable.should_update_tga_to_dds
        };

        if !update_requested {
            return;
        }

        match self.prepare_tga_to_dds_update() {
            Ok(0) => info!("TGA-to-DDS update requested, but no stale textures were found"),
            Ok(count) => info!("Prepared TGA-to-DDS update for {} textures", count),
            Err(err) => warn!(
                "TGA-to-DDS update requested but could not be prepared: {}",
                err
            ),
        }
    }

    fn prepare_tga_to_dds_update(&self) -> Result<usize> {
        let roots = self.tga_texture_roots();
        let mut entries = Vec::new();

        for root in roots {
            if !root.exists() {
                continue;
            }
            self.collect_tga_update_entries(&root, &mut entries)?;
        }

        let mut seen = HashSet::new();
        let stale_entries = select_tga_to_dds_entries(entries)
            .into_iter()
            .filter(|entry| seen.insert(entry.tga_path.to_string_lossy().to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if stale_entries.is_empty() {
            return Ok(0);
        }

        self.write_build_dds_list(&stale_entries)?;
        self.trigger_tga_to_dds_converter()?;
        Ok(stale_entries.len())
    }

    fn tga_texture_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![PathBuf::from("Art").join("Textures")];

        if let Ok(cwd) = env::current_dir() {
            roots.push(cwd.join("Art").join("Textures"));
        }

        if let Ok(exe) = env::current_exe() {
            if let Some(parent) = exe.parent() {
                roots.push(parent.join("Art").join("Textures"));
                roots.push(parent.join("../Art/Textures"));
            }
        }

        roots.push(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("Art")
                .join("Textures"),
        );

        let mut deduped = Vec::new();
        let mut seen = HashSet::new();
        for root in roots {
            let key = root.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                deduped.push(root);
            }
        }
        deduped
    }

    fn collect_tga_update_entries(
        &self,
        root: &Path,
        entries: &mut Vec<TextureUpdateEntry>,
    ) -> Result<()> {
        for dir_entry in fs::read_dir(root).map_err(|e| {
            anyhow!(
                "Failed to scan texture directory '{}': {}",
                root.display(),
                e
            )
        })? {
            let dir_entry =
                dir_entry.map_err(|e| anyhow!("Failed to read texture directory entry: {}", e))?;
            let path = dir_entry.path();
            if path.is_dir() {
                self.collect_tga_update_entries(&path, entries)?;
                continue;
            }

            if texture_update_should_skip(&path) {
                continue;
            }

            let is_tga = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tga"));
            if !is_tga {
                continue;
            }

            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(err) => {
                    warn!(
                        "Skipping texture '{}' during update scan: {}",
                        path.display(),
                        err
                    );
                    continue;
                }
            };
            let tga_modified = match metadata.modified() {
                Ok(modified) => modified,
                Err(err) => {
                    warn!(
                        "Skipping texture '{}' during update scan: {}",
                        path.display(),
                        err
                    );
                    continue;
                }
            };
            let dds_path = texture_dds_path(&path);
            let dds_modified = fs::metadata(&dds_path)
                .and_then(|metadata| metadata.modified())
                .ok();

            entries.push(TextureUpdateEntry {
                tga_path: path,
                dds_path,
                tga_modified,
                dds_modified,
            });
        }

        Ok(())
    }

    fn write_build_dds_list(&self, entries: &[TextureUpdateEntry]) -> Result<()> {
        let mut file = File::create("buildDDS.txt")
            .map_err(|e| anyhow!("Failed to create buildDDS.txt: {}", e))?;
        for entry in entries {
            let line = entry.tga_path.to_string_lossy().replace('/', "\\");
            writeln!(file, "{line}").map_err(|e| anyhow!("Failed to write buildDDS.txt: {}", e))?;
        }
        Ok(())
    }

    fn trigger_tga_to_dds_converter(&self) -> Result<()> {
        let mut candidates = Vec::new();
        let converter_name = if cfg!(windows) { "nvdxt.exe" } else { "nvdxt" };

        candidates.push(PathBuf::from("Build").join(converter_name));
        candidates.push(PathBuf::from("..").join("Build").join(converter_name));
        candidates.push(PathBuf::from(converter_name));
        if cfg!(windows) {
            candidates.push(PathBuf::from("nvdxt"));
        }

        let stdout_file = File::create("buildDDS.out")
            .map_err(|e| anyhow!("Failed to create buildDDS.out: {}", e))
            .ok();

        for candidate in candidates {
            let mut command = Command::new(&candidate);
            command
                .arg("-list")
                .arg("buildDDS.txt")
                .arg("-dxt5")
                .arg("-full")
                .arg("-outdir")
                .arg("Art/Textures");
            if let Some(file) = stdout_file.as_ref() {
                command.stdout(Stdio::from(file.try_clone().map_err(|e| {
                    anyhow!("Failed to duplicate buildDDS.out handle: {}", e)
                })?));
            }
            match command.status() {
                Ok(status) if status.success() => {
                    info!("TGA-to-DDS converter completed successfully");
                    return Ok(());
                }
                Ok(status) => {
                    warn!(
                        "TGA-to-DDS converter '{}' exited with status {}",
                        candidate.display(),
                        status
                    );
                }
                Err(err) => {
                    warn!(
                        "TGA-to-DDS converter '{}' could not be started: {}",
                        candidate.display(),
                        err
                    );
                }
            }
        }

        Err(anyhow!("no TGA-to-DDS converter executable was available"))
    }

    fn verify_release_fingerprints(&self) -> Result<()> {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            Ok(())
        }

        #[cfg(not(any(debug_assertions, feature = "internal")))]
        {
            self.verify_release_fingerprint("generalsbzh.sec", "genseczh.big")?;
            self.verify_release_fingerprint("generalsazh.sec", "musiczh.big")?;
            Ok(())
        }
    }

    fn verify_release_fingerprint(&self, sec_file: &str, expected_archive: &str) -> Result<()> {
        let archive_name = self.get_archive_filename_for_file(sec_file);
        if archive_filename_matches(archive_name.as_deref(), expected_archive) {
            return Ok(());
        }

        let found = archive_name.as_deref().unwrap_or("<not found>");
        warn!(
            "Release fingerprint mismatch: '{}' resolved to '{}' instead of '{}'",
            sec_file, found, expected_archive
        );
        Err(anyhow!(
            "release fingerprint mismatch for '{}': expected '{}', found '{}'",
            sec_file,
            expected_archive,
            found
        ))
    }

    fn runtime_overrides() -> (String, Option<PathBuf>) {
        let mut language = "English".to_string();
        let mut mod_path = None;

        let global = game_engine::common::global_data::read();
        if let Some(lang) = global.get_override("language").and_then(|v| v.as_str()) {
            if !lang.trim().is_empty() {
                language = lang.to_string();
            }
        }
        if let Some(mod_str) = global.get_override("active_mod").and_then(|v| v.as_str()) {
            if !mod_str.trim().is_empty() {
                let candidate = PathBuf::from(mod_str);
                mod_path = std::fs::canonicalize(&candidate).ok().or(Some(candidate));
            }
        }

        (language, mod_path)
    }

    fn language_specific_paths(&self) -> Vec<PathBuf> {
        let lang = self.language.trim();
        if lang.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        let lang_normalized = lang.replace('\\', "/");

        candidates.push(PathBuf::from("Data").join(&lang_normalized));
        candidates.push(PathBuf::from("Data").join(lang_normalized.to_lowercase()));

        if let Ok(cwd) = env::current_dir() {
            candidates.push(cwd.join("Data").join(&lang_normalized));
            candidates.push(cwd.join("Data").join(lang_normalized.to_lowercase()));
        }

        if let Some(mod_path) = &self.active_mod_path {
            candidates.push(mod_path.join("Data").join(&lang_normalized));
        }

        candidates
    }

    fn register_search_paths(&mut self, paths: Vec<PathBuf>) {
        let mut seen = HashSet::new();
        let mut localization_dirs = Vec::new();
        for path in paths {
            let key = path.to_string_lossy().to_string();
            if !seen.insert(key.clone()) {
                continue;
            }
            if path.is_file() {
                self.add_manual_big_file(&path);
            } else {
                self.add_search_path_if_exists(&path);
                localization_dirs.extend(Self::discover_localization_dirs(&path));
            }
        }

        if localization_dirs.is_empty() {
            localization_dirs.push(PathBuf::from("Data/Localization"));
            localization_dirs.push(PathBuf::from("Localization"));
        }
        localization::set_search_paths(&localization_dirs);
    }

    fn add_search_path_if_exists<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if path.exists() {
            debug!("📂 Adding asset search path: {}", path.display());
            self.archive_system.add_search_path(path);
        } else {
            debug!("Skipping missing asset path: {}", path.display());
        }
    }

    fn add_manual_big_file<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        let ext_is_big = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("big"));

        if !ext_is_big {
            warn!(
                "Ignoring manual archive '{}': unsupported extension",
                path.display()
            );
            return;
        }

        if path.exists() {
            debug!("🗃️ Queuing BIG file for manual load: {}", path.display());
            self.manual_big_files.push(path.to_path_buf());
        } else {
            warn!("Manual BIG file not found, skipping: {}", path.display());
        }
    }

    async fn load_manual_archives(&mut self) -> Result<()> {
        for big in std::mem::take(&mut self.manual_big_files) {
            debug!("🗃️ Loading BIG archive {}", big.display());
            self.archive_system
                .load_big_file(&big)
                .await
                .map_err(|e| anyhow!("Failed to load {}: {}", big.display(), e))?;
        }
        Ok(())
    }

    fn discover_localization_dirs(base: &Path) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let primary = base.join("Localization");
        if primary.exists() && primary.is_dir() {
            dirs.push(primary);
        }

        let data_loc = base.join("Data").join("Localization");
        if data_loc.exists() && data_loc.is_dir() {
            dirs.push(data_loc);
        }

        dirs
    }

    /// Start shell/menu music through TheAudio authored event (C++ Music.ini Shell).
    pub async fn start_background_music(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }

        info!("Starting C&C background music via TheAudio");
        self.audio_manager
            .play_random_cnc_music(&mut self.archive_system)
            .await
    }

    /// Load C&C model (with caching)
    pub async fn load_cnc_model(&mut self, unit_name: &str) -> Result<&W3DModel> {
        let unit_key = unit_name.to_lowercase();

        // Return cached model if available
        if self.model_cache.contains_key(&unit_key) {
            return Ok(self
                .model_cache
                .get(&unit_key)
                .expect("model_cache key existed but value disappeared"));
        }

        info!("Loading C&C model: {}", unit_name);

        // Load model using W3D loader
        let model = self
            .model_loader
            .load_cnc_model(&mut self.archive_system, unit_name)
            .await
            .map_err(|e| anyhow!("Failed to load model {}: {}", unit_name, e))?;

        self.model_cache.insert(unit_key.clone(), model);
        self.model_cache
            .get(&unit_key)
            .ok_or_else(|| anyhow!("Model cache insert failed for '{}'", unit_name))
    }

    /// Load texture from BIG archives
    pub async fn load_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> &GPUTexture {
        if !self.initialized {
            error!("AssetManager not initialized, returning default texture");
            return self.texture_manager.get_default_texture();
        }

        let lookup_name = if Self::should_resolve_object_texture_name(texture_name) {
            self.ww3d_manager
                .get_texture_for_object(texture_name)
                .unwrap_or_else(|| texture_name.to_string())
        } else {
            texture_name.to_string()
        };

        self.texture_manager
            .get_texture_or_default(&mut self.archive_system, device, queue, &lookup_name)
            .await
    }

    /// Load texture synchronously - blocks until loaded, returns texture name for cache lookup
    pub fn load_texture_blocking(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> String {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _ = self.load_texture(device, queue, texture_name).await;
                texture_name.to_string()
            })
        })
    }

    /// Prime only raw texture data synchronously (no GPU texture upload).
    pub fn prime_texture_raw_blocking(&mut self, texture_name: &str) -> String {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _ = self
                    .texture_manager
                    .prime_raw_texture(&mut self.archive_system, texture_name)
                    .await;
                texture_name.to_string()
            })
        })
    }

    /// Prime a batch of raw texture payloads synchronously (no GPU upload).
    pub fn prime_textures_raw_blocking<I, S>(&mut self, texture_names: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let unique: Vec<String> = {
            let mut seen = HashSet::new();
            texture_names
                .into_iter()
                .filter_map(|name| {
                    let trimmed = name.as_ref().trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    let key = trimmed.to_ascii_lowercase();
                    if seen.insert(key) {
                        Some(trimmed.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        };

        if unique.is_empty() {
            return;
        }

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                for name in unique {
                    let _ = self
                        .texture_manager
                        .prime_raw_texture(&mut self.archive_system, &name)
                        .await;
                }
            })
        });
    }

    /// Get default texture
    pub fn get_default_texture(&self) -> &GPUTexture {
        self.texture_manager.get_default_texture()
    }

    /// Get raw texture data if it's cached.
    pub fn get_raw_texture(&self, texture_name: &str) -> Option<&RawTexture> {
        self.texture_manager.get_raw_texture(texture_name)
    }

    pub fn is_known_missing_texture(&self, texture_name: &str) -> bool {
        self.texture_manager.is_known_missing_texture(texture_name)
    }

    /// Get colored default texture (for indicating different states)
    pub fn get_colored_default_texture(&self, color_name: &str) -> &GPUTexture {
        self.texture_manager.get_colored_default_texture(color_name)
    }

    /// Get texture for an object from WW3D Asset Manager
    /// Returns the texture filename defined for the object in INI files
    pub fn get_texture_for_object(&self, object_name: &str) -> Option<String> {
        self.ww3d_manager.get_texture_for_object(object_name)
    }

    /// Get model for an object from WW3D Asset Manager
    /// Returns the model filename defined for the object in INI files
    pub fn get_model_for_object(&self, object_name: &str) -> Option<String> {
        self.ww3d_manager.get_model_for_object(object_name)
    }

    /// Select the first source-authored Draw model for this exact Object INI
    /// identity and the frozen ModelConditionFlags bit bank.
    ///
    /// This intentionally does not route through `get_model_for_object`: that
    /// cache holds the pristine/default model and would overwrite a condition
    /// model such as `AVPaladin_D`.
    pub fn select_model_for_object_conditions(
        &self,
        object_name: &str,
        condition_bits: u128,
    ) -> Option<AuthoredConditionModelSelection> {
        self.ww3d_manager
            .select_model_for_object_conditions(object_name, condition_bits)
    }

    /// Select every exact model supplied by the Object's source-authored Draw
    /// modules, preserving declaration order and repeated W3D names.
    pub fn select_draw_models_for_object_conditions(
        &self,
        object_name: &str,
        condition_bits: u128,
    ) -> Option<Vec<AuthoredDrawModel>> {
        self.ww3d_manager
            .select_draw_models_for_object_conditions(object_name, condition_bits)
    }

    /// Apply C++ `findTransitionForSig` playback for one live object.
    pub fn apply_live_draw_transition_playback(
        &self,
        object_id: u32,
        object_name: &str,
        dest_models: Vec<AuthoredDrawModel>,
    ) -> Vec<AuthoredDrawModel> {
        match self
            .ww3d_manager
            .resolve_object_definition(object_name, None)
        {
            Some(definition) => {
                definition.apply_live_draw_transition_playback(object_id, dest_models)
            }
            None => dest_models,
        }
    }

    /// Return cache-only, exact C++ `Drawable::getBarrelCount` candidates for
    /// the currently selected Object INI Draw states.
    ///
    /// Each array entry is the first validated nonzero count contributed by
    /// the source Draw modules in declaration order. `Some(None)` means every
    /// currently validated module reported zero for that slot. `None` means
    /// an earlier module or model is not resident/valid enough to prove the
    /// answer. In that case the gameplay cursor must retain its existing
    /// fail-closed one-barrel or staged-restore state; this method never opens
    /// an archive, blocks on Tokio, or falls back by template/model suffix.
    ///
    /// Positive answers are immutable asset facts for the manager lifetime,
    /// so they are memoized by exact Object identity and ModelCondition bank.
    /// Negative/unknown answers are deliberately not cached: a normal safe
    /// startup/render prewarm may make the exact source model resident later.
    pub fn cached_weapon_barrel_counts_for_object_conditions(
        &mut self,
        object_name: &str,
        condition_bits: u128,
    ) -> Option<[Option<u8>; 3]> {
        let cache_key = WeaponBarrelCountCacheKey::new(object_name, condition_bits)?;
        if let Some(counts) = self.weapon_barrel_count_cache.get(&cache_key) {
            return Some(*counts);
        }

        let draw_models =
            self.select_draw_models_for_object_conditions(object_name, condition_bits)?;
        // Hold the model-cache borrow only for the pure topology reduction.
        // The resulting counts are plain Copy values, so memoization happens
        // after that borrow ends rather than aliasing `self` mutably.
        let counts = {
            let model_cache = &self.model_cache;
            cached_weapon_barrel_counts_for_draw_models(&draw_models, |model_key| {
                model_cache.get(&model_key.to_ascii_lowercase())
            })
        }?;
        self.weapon_barrel_count_cache.insert(cache_key, counts);
        Some(counts)
    }

    /// Warm exact W3D models named by selected Draw states, without routing a
    /// source model key through the Object-template lookup.  This is the safe
    /// map/startup boundary counterpart to the cache-only combat lookup: it
    /// may open archives here, but never from a fixed simulation tick.
    pub fn prewarm_exact_w3d_models_blocking<I, S>(&mut self, model_keys: I) -> ModelPrewarmStats
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut stats = ModelPrewarmStats::default();
        let mut requested = Vec::new();
        let mut seen = HashSet::new();

        for model_key in model_keys {
            let model_key = model_key.as_ref().trim();
            if model_key.is_empty() {
                continue;
            }
            let normalized = model_key.to_ascii_lowercase();
            if !seen.insert(normalized.clone()) {
                continue;
            }
            stats.requested += 1;
            if self.model_cache.contains_key(&normalized) {
                stats.cache_hits += 1;
            } else {
                requested.push(model_key.to_string());
            }
        }

        for model_key in requested {
            match self.load_w3d_model(&model_key) {
                Ok(_) => stats.resolved += 1,
                Err(error) => {
                    self.missing_model_keys
                        .insert(model_key.to_ascii_lowercase());
                    warn!(
                        "Failed to prewarm exact W3D model '{}' for cached barrel topology: {}",
                        model_key, error
                    );
                    stats.missing += 1;
                }
            }
        }

        stats
    }

    /// Prewarm and memoize exact barrel-count facts for a bounded set of
    /// active Object/ModelCondition selections.
    ///
    /// This method is intended for successful map/start boundaries. It loads
    /// only Draw models whose unresolved source slots actually declare one of
    /// C++'s four barrel bases; modules with no bases are already known to
    /// return zero. A failed/malformed topology is left unknown rather than
    /// stored as a one-barrel guess, so a v4 restored cursor remains staged.
    pub fn prewarm_weapon_barrel_topologies_for_object_conditions<I>(
        &mut self,
        selections: I,
    ) -> ModelPrewarmStats
    where
        I: IntoIterator<Item = (String, u128)>,
    {
        let mut selection_keys = Vec::new();
        let mut seen_selections = HashSet::new();
        let mut model_keys = Vec::new();
        let mut seen_models = HashSet::new();

        for (object_name, condition_bits) in selections {
            let Some(cache_key) = WeaponBarrelCountCacheKey::new(&object_name, condition_bits)
            else {
                continue;
            };
            if !seen_selections.insert(cache_key.clone()) {
                continue;
            }
            if self.weapon_barrel_count_cache.contains_key(&cache_key) {
                continue;
            }

            let Some(draw_models) =
                self.select_draw_models_for_object_conditions(&object_name, condition_bits)
            else {
                continue;
            };
            for draw_model in &draw_models {
                let bindings = &draw_model.weapon_bone_bindings;
                if !bindings.source_fields_valid {
                    continue;
                }
                let has_source_base = bindings.slots.iter().any(|slot| {
                    slot.fire_fx_bone_base.is_some()
                        || slot.recoil_bone_base.is_some()
                        || slot.muzzle_flash_bone_base.is_some()
                        || slot.launch_bone_base.is_some()
                });
                let model_key = draw_model.model_key.trim();
                if has_source_base
                    && !model_key.is_empty()
                    && seen_models.insert(model_key.to_ascii_lowercase())
                {
                    model_keys.push(model_key.to_string());
                }
            }
            selection_keys.push((object_name, condition_bits));
        }

        let stats = self.prewarm_exact_w3d_models_blocking(model_keys);
        for (object_name, condition_bits) in selection_keys {
            let _ = self
                .cached_weapon_barrel_counts_for_object_conditions(&object_name, condition_bits);
        }
        stats
    }

    /// Prewarm every model that can carry one of C++'s four barrel bases for
    /// a bounded set of active Object INI identities.
    ///
    /// Unlike [`Self::prewarm_weapon_barrel_topologies_for_object_conditions`],
    /// this scans the finite authored `ConditionState` table instead of only
    /// the state active at map-load time. This matters because a unit can
    /// enter a FIRING/DAMAGED/upgrade state before its first accepted shot;
    /// loading that model from the simulation tick would be the wrong
    /// ownership and latency boundary. The later tick path still performs an
    /// exact current-state selection and accepts only cached W3D models.
    pub fn prewarm_weapon_barrel_topology_models_for_objects<I, S>(
        &mut self,
        object_names: I,
    ) -> ModelPrewarmStats
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut seen_objects = HashSet::new();
        let mut seen_models = HashSet::new();
        let mut model_keys = Vec::new();

        for object_name in object_names {
            let object_name = object_name.as_ref().trim();
            if object_name.is_empty() || !seen_objects.insert(object_name.to_ascii_lowercase()) {
                continue;
            }
            let Some(definition) = self.resolve_object_definition(object_name, None) else {
                continue;
            };

            for module in &definition.draw_modules {
                for state in &module.condition_states {
                    // C++ W3DModelDraw validates its WeaponBarrelInfo against
                    // whichever ModelConditionInfo is current, including a
                    // source-authored TransitionState.  Prewarming a finite
                    // state table is the only safe place to cover that state;
                    // a simulation-tick archive load would be the wrong
                    // ownership boundary.
                    if !state.weapon_bone_bindings.source_fields_valid {
                        continue;
                    }
                    let has_source_base = state.weapon_bone_bindings.slots.iter().any(|slot| {
                        slot.fire_fx_bone_base.is_some()
                            || slot.recoil_bone_base.is_some()
                            || slot.muzzle_flash_bone_base.is_some()
                            || slot.launch_bone_base.is_some()
                    });
                    let crate::assets::AuthoredConditionModel::Named(model_key) = &state.model
                    else {
                        continue;
                    };
                    let model_key = model_key.trim();
                    if has_source_base
                        && !model_key.is_empty()
                        && seen_models.insert(model_key.to_ascii_lowercase())
                    {
                        model_keys.push(model_key.to_string());
                    }
                }
            }
        }

        self.prewarm_exact_w3d_models_blocking(model_keys)
    }

    /// Get full object definition from WW3D Asset Manager
    pub fn get_object_definition(
        &self,
        object_name: &str,
    ) -> Option<&crate::assets::ObjectDefinition> {
        self.ww3d_manager.get_object_definition(object_name)
    }

    /// Resolve object definition using name with optional model hint fallback
    pub fn resolve_object_definition(
        &self,
        object_name: &str,
        model_hint: Option<&str>,
    ) -> Option<&crate::assets::ObjectDefinition> {
        self.ww3d_manager
            .resolve_object_definition(object_name, model_hint)
    }

    /// Check if an object is defined in the WW3D Asset Manager
    pub fn has_object_definition(&self, object_name: &str) -> bool {
        self.ww3d_manager.has_object(object_name)
    }

    /// Get total count of loaded object definitions
    pub fn get_object_definition_count(&self) -> usize {
        self.ww3d_manager.object_count()
    }

    /// Return an owned, deterministically ordered snapshot of the resolved
    /// retail Object INI definitions.
    ///
    /// GameLogic uses this at world initialization to seed exact template
    /// identities without holding the global asset-manager lock while it
    /// constructs gameplay state.
    pub fn object_definitions_snapshot(&self) -> Vec<(String, crate::assets::ObjectDefinition)> {
        self.ww3d_manager.object_definitions_snapshot()
    }

    /// Overlay leftover map.ini Object CREATE_OVERRIDES onto WW3D object definitions.
    pub fn overlay_object_create_overrides(
        &mut self,
        name: &str,
        reskin_from: &str,
        properties: &std::collections::HashMap<String, String>,
    ) {
        self.ww3d_manager
            .overlay_object_create_overrides(name, reskin_from, properties);
    }

    /// Get all texture filenames from WW3D Asset Manager for preloading
    pub fn get_all_texture_filenames(&self) -> Vec<String> {
        self.ww3d_manager.get_all_texture_filenames()
    }

    /// Load a texture (returns a reference to the GPU texture)
    /// This is the actual async method that loads from archives
    pub async fn load_texture_async(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> Result<&GPUTexture> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }

        // This calls the low-level texture manager load_texture with archive system access
        self.texture_manager
            .load_texture(&mut self.archive_system, device, queue, texture_name)
            .await
    }

    /// Play sound effect from archives
    pub async fn play_sound_effect(&mut self, sound_name: &str) -> Result<()> {
        self.play_sound_effect_scaled(sound_name, 1.0).await
    }

    pub async fn play_sound_effect_scaled(
        &mut self,
        sound_name: &str,
        volume_scale: f32,
    ) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }

        self.audio_manager
            .play_sound_effect_scaled(&mut self.archive_system, sound_name, volume_scale)
            .await
    }

    /// Toggle background music
    pub fn toggle_background_music(&self) {
        self.audio_manager.toggle_background_music();
    }

    /// Set music volume
    pub fn set_music_volume(&mut self, volume: f32) {
        self.audio_manager.set_music_volume(volume);
    }

    /// Set sound effects volume
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.audio_manager.set_sfx_volume(volume);
    }

    /// Check if file exists in archives
    pub fn does_file_exist(&self, filename: &str) -> bool {
        if !self.initialized {
            return false;
        }
        self.archive_system.does_file_exist(filename)
    }

    /// Resolve the archive that currently owns the provided file.
    pub fn get_archive_filename_for_file(&self, filename: &str) -> Option<String> {
        self.archive_system.get_archive_filename_for_file(filename)
    }

    /// Check if a virtual archive path can be opened with the active mount set.
    pub fn can_open_file_sync(&mut self, filename: &str) -> bool {
        if !self.initialized {
            return false;
        }
        self.archive_system.open_reader(filename).is_ok()
    }

    /// Extract raw file data from archives
    pub async fn extract_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }
        self.archive_system.open_file(filename).await
    }

    /// List all available files in archives
    pub fn list_all_files(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.archive_system.list_all_files()
    }

    /// List available models
    pub fn list_available_models(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.model_loader
            .list_available_models(&self.archive_system)
    }

    /// List available textures
    pub fn list_available_textures(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.texture_manager
            .list_available_textures(&self.archive_system)
    }

    /// Get loaded archives
    pub fn get_loaded_archives(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.archive_system.get_loaded_archives()
    }

    /// Mount `-mod` BIG/dir into the live extract archive (overwrite).
    pub fn load_user_mods(&mut self, mod_dir: &str, mod_big: &str) -> anyhow::Result<()> {
        self.archive_system.load_user_mods(mod_dir, mod_big)
    }

    /// Get common C&C unit names
    pub fn get_common_cnc_units(&self) -> Vec<&'static str> {
        get_common_cnc_units()
    }

    /// Prewarm a set of object/template names into the internal model cache.
    ///
    /// Each name is resolved through the object-definition map when possible,
    /// then loaded through the normal W3D path and aliased back to the original
    /// request name so later lookups stay cheap.
    pub fn prewarm_object_models_blocking<I, S>(&mut self, object_names: I) -> ModelPrewarmStats
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut stats = ModelPrewarmStats::default();
        let mut seen = HashSet::new();
        let mut requests: Vec<(String, String)> = Vec::new();

        for object_name in object_names {
            let object_name = object_name.as_ref().trim();
            if object_name.is_empty() {
                continue;
            }

            let object_key = object_name.to_ascii_lowercase();
            if !seen.insert(object_key.clone()) {
                continue;
            }

            let resolved_name = self
                .get_model_for_object(object_name)
                .unwrap_or_else(|| object_name.to_string());
            let resolved_key = resolved_name.to_ascii_lowercase();

            stats.requested += 1;
            if self.model_cache.contains_key(&object_key)
                || self.model_cache.contains_key(&resolved_key)
            {
                stats.cache_hits += 1;
                if object_key != resolved_key {
                    if let Some(model) = self.model_cache.get(&resolved_key).cloned() {
                        self.model_cache.insert(object_key, model);
                    }
                }
                continue;
            }

            requests.push((object_name.to_string(), resolved_name));
        }

        for (object_name, resolved_name) in requests {
            match self.load_w3d_model(&resolved_name) {
                Ok(model) => {
                    let object_key = object_name.to_ascii_lowercase();
                    let resolved_key = resolved_name.to_ascii_lowercase();
                    if object_key != resolved_key {
                        self.model_cache.insert(object_key, model.clone());
                    }
                    stats.resolved += 1;
                }
                Err(err) => {
                    self.missing_model_keys
                        .insert(object_name.to_ascii_lowercase());
                    self.missing_model_keys
                        .insert(resolved_name.to_ascii_lowercase());
                    warn!(
                        "Failed to prewarm W3D model '{}' (resolved '{}'): {}",
                        object_name, resolved_name, err
                    );
                    stats.missing += 1;
                }
            }
        }

        stats
    }

    /// Prewarm the common C&C unit set used by shell/menu/world startup paths.
    pub fn prewarm_common_cnc_models_blocking(&mut self) -> ModelPrewarmStats {
        let units = self.get_common_cnc_units();
        self.prewarm_object_models_blocking(units)
    }

    /// Load a specific C&C unit model by name
    pub async fn load_unit_model(&mut self, unit_name: &str) -> Result<&W3DModel> {
        self.load_cnc_model(unit_name).await
    }

    /// Get a cached model by name (synchronous)
    pub fn get_cached_model(&self, unit_name: &str) -> Option<W3DModel> {
        let unit_key = unit_name.to_lowercase();
        self.model_cache.get(&unit_key).cloned()
    }

    pub fn get_cached_model_ref(&self, unit_name: &str) -> Option<&W3DModel> {
        let unit_key = unit_name.to_lowercase();
        self.model_cache.get(&unit_key)
    }

    /// Return a resident exact C++ render-object prototype by full source name.
    ///
    /// This is cache-only and compares the complete identity
    /// case-insensitively. It never treats a W3D filename, a mesh suffix, or a
    /// presentation alias as an equivalent prototype name.
    pub fn cached_w3d_render_object_prototype(
        &self,
        full_name: &str,
    ) -> Option<W3dRenderObjectPrototype> {
        self.w3d_render_object_prototypes.lookup(full_name)
    }

    /// Resolve one exact C++ render-object prototype, loading the source W3D
    /// named by the portion before its first dot on a cache miss.
    ///
    /// This method performs synchronous archive I/O and parsing. Use it during
    /// asset prewarm/construction only; frozen render collection must use
    /// [`Self::cached_w3d_render_object_prototype`] and
    /// [`Self::cached_w3d_render_object_source_model`] exclusively. A file that
    /// lacks the requested complete source identity returns `None` rather than
    /// a whole-file model, alias, or placeholder.
    pub fn resolve_w3d_render_object_prototype_blocking(
        &mut self,
        full_name: &str,
    ) -> Option<W3dRenderObjectPrototype> {
        let (registry, archive_system, model_loader) = (
            &mut self.w3d_render_object_prototypes,
            &mut self.archive_system,
            &self.model_loader,
        );
        resolve_w3d_render_object_prototype_with_loader(registry, full_name, |source_stem| {
            match load_exact_w3d_render_object_source(model_loader, archive_system, source_stem) {
                Ok(model) => Some(model),
                Err(error) => {
                    debug!(
                        "Strict W3D render-object source '{}' did not resolve '{}': {}",
                        source_stem, full_name, error
                    );
                    None
                }
            }
        })
    }

    /// Borrow the exact immutable source model associated with a strict
    /// prototype token.
    ///
    /// The token must still be resident in this manager's strict registry; a
    /// manually fabricated or stale token cannot expose an unrelated model.
    pub fn cached_w3d_render_object_source_model(
        &self,
        prototype: &W3dRenderObjectPrototype,
    ) -> Option<&W3DModel> {
        self.w3d_render_object_prototypes.source_model(prototype)
    }

    pub fn model_animation_names(&self, model_name: &str) -> Vec<String> {
        let model_key = model_name.to_lowercase();
        self.model_cache
            .get(&model_key)
            .map(|m| {
                m.animation_names()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn model_find_animation_index(&self, model_name: &str, anim_name: &str) -> Option<usize> {
        let model_key = model_name.to_lowercase();
        self.model_cache
            .get(&model_key)
            .and_then(|m| m.find_animation_index(anim_name))
    }

    pub fn model_sample_animation(
        &self,
        model_name: &str,
        anim_index: usize,
        frame: f32,
    ) -> Option<Vec<[f32; 16]>> {
        let model_key = model_name.to_lowercase();
        self.model_cache
            .get(&model_key)
            .and_then(|m| m.sample_animation(anim_index, frame))
    }

    /// Return an already-prewarmed exact Draw animation binding for this
    /// geometry model. This is intentionally cache-only: it is called during
    /// frozen-frame collection and must never open an archive, wait on Tokio,
    /// or try a model-name alias.
    pub fn cached_w3d_draw_animation_binding(
        &self,
        geometry_model: &W3DModel,
        identity: &str,
    ) -> Option<W3dAnimationBinding> {
        if let Some(local) = geometry_model.local_animation_binding_for_draw_identity(identity) {
            return Some(local);
        }

        let key = CompanionAnimationCacheKey::for_geometry_model(geometry_model, identity)?;
        let animation = self.companion_animation_cache.get(&key)?.as_ref()?.clone();
        let binding = W3dAnimationBinding::companion(identity.trim(), animation);
        geometry_model
            .animation_binding_is_compatible(&binding)
            .then_some(binding)
    }

    /// Prewarm exactly one frozen `Hierarchy.Animation` request before render
    /// collection. The geometry model is loaded through its existing exact
    /// model path; a companion clip is loaded only through C++'s deterministic
    /// `Animation.w3d` rule and memoized (including misses) by full identity
    /// plus the compatible hierarchy, never by a model basename.
    pub fn prewarm_w3d_draw_animation_binding(&mut self, model_name: &str, identity: &str) -> bool {
        let geometry_model = match self.load_w3d_model(model_name) {
            Ok(model) => model,
            Err(error) => {
                warn!(
                    "W3D Draw animation prewarm skipped: geometry '{}' for '{}' failed: {}",
                    model_name, identity, error
                );
                return false;
            }
        };

        if geometry_model
            .local_animation_binding_for_draw_identity(identity)
            .is_some()
        {
            return true;
        }

        let Some(cache_key) =
            CompanionAnimationCacheKey::for_geometry_model(&geometry_model, identity)
        else {
            // The frozen identity cannot bind this model's HTree. There is no
            // compatible cache key and no reason to touch archive storage.
            return false;
        };

        if let Some(cached) = self.companion_animation_cache.get(&cache_key) {
            return cached.is_some();
        }

        let animation = match self.load_companion_animation_blocking(identity) {
            Ok(animation) => {
                let animation = Arc::new(animation);
                let binding =
                    W3dAnimationBinding::companion(identity.trim(), Arc::clone(&animation));
                if geometry_model.animation_binding_is_compatible(&binding) {
                    Some(animation)
                } else {
                    warn!(
                        "W3D Draw companion '{}' is incompatible with geometry hierarchy for '{}'",
                        identity, model_name
                    );
                    None
                }
            }
            Err(error) => {
                warn!(
                    "W3D Draw companion prewarm failed for '{}': {}",
                    identity, error
                );
                None
            }
        };
        let resolved = animation.is_some();
        self.companion_animation_cache.insert(cache_key, animation);
        resolved
    }

    fn load_companion_animation_blocking(&mut self, identity: &str) -> Result<W3dAnimation> {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                return tokio::task::block_in_place(|| {
                    handle.block_on(
                        self.model_loader
                            .load_companion_animation(&mut self.archive_system, identity),
                    )
                });
            }
            return Err(anyhow!(
                "synchronous W3D companion loading not supported on current-thread runtime"
            ));
        }

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(
            self.model_loader
                .load_companion_animation(&mut self.archive_system, identity),
        )
    }

    /// Load a model asynchronously by cloning from cache or loading fresh
    pub async fn load_w3d_model_async(&mut self, model_name: &str) -> Result<W3DModel> {
        let resolved_name = self.resolve_available_model_name(model_name);
        let model_key = model_name.to_lowercase();
        let resolved_key = resolved_name.to_lowercase();

        // Check cache first
        if let Some(model) = self.model_cache.get(&model_key) {
            return Ok(model.clone());
        }
        if resolved_key != model_key {
            if let Some(model) = self.model_cache.get(&resolved_key).cloned() {
                self.model_cache.insert(model_key.clone(), model.clone());
                return Ok(model);
            }
        }

        if self.missing_model_keys.contains(&model_key)
            || self.missing_model_keys.contains(&resolved_key)
        {
            return Err(anyhow!(
                "W3D load skipped for known-missing model '{}'",
                resolved_name
            ));
        }

        info!("Loading W3D model: {}", resolved_name);

        // Use the actual W3D loader to parse the model.
        let w3d_loader = W3DLoader::new();
        let model = match w3d_loader
            .load_model(&mut self.archive_system, &resolved_name)
            .await
        {
            Ok(model) => model,
            Err(e) => {
                self.missing_model_keys.insert(model_key.clone());
                self.missing_model_keys.insert(resolved_key.clone());
                crate::release_candidate::note_missing_w3d_model(model_name);
                warn!("W3D loader failed for '{}': {}", model_name, e);
                return Err(anyhow!("W3D load failed for '{}': {e}", model_name));
            }
        };

        info!(
            "✅ Successfully loaded W3D model '{}' with {} meshes, {} total vertices",
            resolved_name,
            model.meshes.len(),
            model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
        );

        // Cache the model
        self.model_cache.insert(resolved_key, model.clone());
        if resolved_name != model_name {
            self.model_cache.insert(model_key, model.clone());
        }
        self.missing_model_keys.remove(&model_name.to_lowercase());
        self.missing_model_keys
            .remove(&resolved_name.to_lowercase());
        Ok(model)
    }

    pub fn load_w3d_model_blocking(&mut self, model_name: &str) -> Result<W3DModel> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.load_w3d_model_async(model_name))
        })
    }

    /// Load a model synchronously by cloning from cache or loading through the W3D parser path.
    pub fn load_w3d_model(&mut self, model_name: &str) -> Result<W3DModel> {
        let resolved_name = self.resolve_available_model_name(model_name);
        let model_key = model_name.to_lowercase();
        let resolved_key = resolved_name.to_lowercase();

        // Check cache first
        if let Some(model) = self.model_cache.get(&model_key) {
            return Ok(model.clone());
        }
        if resolved_key != model_key {
            if let Some(model) = self.model_cache.get(&resolved_key).cloned() {
                self.model_cache.insert(model_key.clone(), model.clone());
                return Ok(model);
            }
        }

        if self.missing_model_keys.contains(&model_key)
            || self.missing_model_keys.contains(&resolved_key)
        {
            return Err(anyhow!(
                "Synchronous W3D load skipped for known-missing model '{}'",
                resolved_name
            ));
        }

        let model = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                tokio::task::block_in_place(|| {
                    handle.block_on(
                        self.model_loader
                            .load_model(&mut self.archive_system, &resolved_name),
                    )
                })
            } else {
                Err(anyhow!(
                    "Synchronous W3D loading not supported on current-thread runtime"
                ))
            }
        } else {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(
                self.model_loader
                    .load_model(&mut self.archive_system, &resolved_name),
            )
        };

        let model = match model {
            Ok(model) => model,
            Err(err) => {
                self.missing_model_keys.insert(model_key.clone());
                self.missing_model_keys.insert(resolved_key.clone());
                crate::release_candidate::note_missing_w3d_model(&resolved_name);
                warn!(
                    "Synchronous W3D load failed for '{}': {}",
                    resolved_name, err
                );
                return Err(anyhow!(
                    "Synchronous W3D load failed for '{}': {}",
                    resolved_name,
                    err
                ));
            }
        };

        // Cache the model
        self.model_cache.insert(resolved_key, model.clone());
        if resolved_name != model_name {
            self.model_cache.insert(model_key, model.clone());
        }
        self.missing_model_keys.remove(&model_name.to_lowercase());
        self.missing_model_keys
            .remove(&resolved_name.to_lowercase());
        Ok(model)
    }

    /// Play faction-specific music
    pub async fn play_faction_music(&mut self, faction: &str) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }
        self.audio_manager
            .play_faction_music(&mut self.archive_system, faction)
            .await
    }

    /// Update asset manager (cleanup, etc.)
    pub fn update(&mut self) {
        if self.initialized {
            self.audio_manager.update();
        }
    }

    /// Get asset statistics
    pub fn get_statistics(&self) -> AssetStatistics {
        let archive_stats = if self.initialized {
            self.archive_system.get_statistics()
        } else {
            ArchiveStatistics {
                total_archives: 0,
                total_files: 0,
                unique_files: 0,
            }
        };

        let (textures_raw, textures_gpu) = if self.initialized {
            self.texture_manager.get_cache_stats()
        } else {
            (0, 0)
        };

        AssetStatistics {
            archive_stats,
            models_cached: self.model_cache.len(),
            textures_cached: textures_gpu,
            textures_raw_cached: textures_raw,
            initialized: self.initialized,
        }
    }

    /// Clear caches to free memory
    pub fn clear_caches(&mut self) {
        info!("Clearing asset caches");
        self.model_cache.clear();
        self.w3d_render_object_prototypes = W3dRenderObjectPrototypeRegistry::default();
        self.companion_animation_cache.clear();
        self.missing_model_keys.clear();
        self.texture_manager.clear_cache();
    }

    /// Check if a texture is already loaded in cache
    pub fn get_cached_texture(&self, texture_name: &str) -> Option<&GPUTexture> {
        if !self.initialized {
            return None;
        }
        self.texture_manager.get_cached_texture(texture_name)
    }

    /// Search for specific assets
    pub fn search_assets(&self, pattern: &str) -> AssetSearchResults {
        if !self.initialized {
            return AssetSearchResults::default();
        }

        let pattern_lower = pattern.to_lowercase();
        let all_files = self.archive_system.list_all_files();

        let mut models = Vec::new();
        let mut textures = Vec::new();
        let mut audio = Vec::new();
        let mut other = Vec::new();

        for file in all_files {
            let file_lower = file.to_lowercase();
            if !file_lower.contains(&pattern_lower) {
                continue;
            }

            if file_lower.ends_with(".w3d") {
                models.push(file);
            } else if file_lower.ends_with(".tga")
                || file_lower.ends_with(".dds")
                || file_lower.ends_with(".bmp")
                || file_lower.ends_with(".jpg")
                || file_lower.ends_with(".png")
            {
                textures.push(file);
            } else if file_lower.ends_with(".mp3")
                || file_lower.ends_with(".ogg")
                || file_lower.ends_with(".wav")
            {
                audio.push(file);
            } else {
                other.push(file);
            }
        }

        AssetSearchResults {
            models,
            textures,
            audio,
            other,
        }
    }

    /// Is the asset manager initialized?
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Asset manager statistics
#[derive(Debug)]
pub struct AssetStatistics {
    pub archive_stats: ArchiveStatistics,
    pub models_cached: usize,
    pub textures_cached: usize,
    pub textures_raw_cached: usize,
    pub initialized: bool,
}

/// Asset search results
#[derive(Debug, Default)]
pub struct AssetSearchResults {
    pub models: Vec<String>,
    pub textures: Vec<String>,
    pub audio: Vec<String>,
    pub other: Vec<String>,
}

impl AssetSearchResults {
    pub fn total_results(&self) -> usize {
        self.models.len() + self.textures.len() + self.audio.len() + self.other.len()
    }
}

/// Global asset manager instance
static ASSET_MANAGER: OnceLock<Arc<Mutex<AssetManager>>> = OnceLock::new();
static CAUSTIC_WARMUP_STARTED: AtomicBool = AtomicBool::new(false);
static TEXTURE_PRIME_QUEUE: OnceLock<Sender<String>> = OnceLock::new();

/// Initialize the global asset manager
pub async fn init_asset_manager(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
    let manager_create_start = SystemTime::now();
    info!("📂 Creating asset manager and loading BIG archives...");

    let mut manager = AssetManager::new()?;
    let manager_create_duration = manager_create_start.elapsed().unwrap_or_default();
    info!(
        "📂 BIG archives loaded in {:.2}s",
        manager_create_duration.as_secs_f32()
    );

    let wgpu_init_start = SystemTime::now();
    info!("🖥️ Initializing WGPU asset resources...");
    manager.init(device, queue).await?;
    let wgpu_init_duration = wgpu_init_start.elapsed().unwrap_or_default();
    info!(
        "🖥️ WGPU asset resources initialized in {:.2}s",
        wgpu_init_duration.as_secs_f32()
    );

    ASSET_MANAGER
        .set(Arc::new(Mutex::new(manager)))
        .map_err(|_| anyhow!("Asset manager already initialized"))?;

    begin_background_music_startup();

    info!(
        "Global asset manager initialized (Total: {:.2}s)",
        (manager_create_duration + wgpu_init_duration).as_secs_f32()
    );
    Ok(())
}

fn begin_background_music_startup() {
    let Some(manager_arc) = get_asset_manager() else {
        return;
    };

    tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        let mut manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(err) = handle.block_on(async { manager.start_background_music().await }) {
            warn!("Failed to start background music: {}", err);
        }
    });
}

/// Get reference to global asset manager
pub fn get_asset_manager() -> Option<Arc<Mutex<AssetManager>>> {
    ASSET_MANAGER.get().cloned()
}

/// Resolve the source-authored W3D model key for a frozen presentation object.
///
/// A separately authored fallback is valid only when no catalogue is active or
/// the exact Object identity has no retained ConditionState table.  Once an
/// Object INI state table was found, `Model = None` and an unsupported source
/// token fail closed rather than rendering a pristine/suffix-guessed mesh.
pub fn resolve_presentation_model_key_for_conditions(
    object_name: &str,
    fallback_model_key: &str,
    condition_bits: u128,
) -> Option<String> {
    let fallback = || {
        let value = fallback_model_key.trim();
        (!value.is_empty()).then(|| value.to_string())
    };

    let Some(asset_manager) = get_asset_manager() else {
        return fallback();
    };
    let Ok(asset_manager) = asset_manager.lock() else {
        // The catalogue exists but cannot be queried reliably.  Rendering a
        // potentially pristine fallback here would be a condition-model
        // substitution, so leave this object without a mesh for this frame.
        return None;
    };

    match asset_manager.select_model_for_object_conditions(object_name, condition_bits) {
        Some(AuthoredConditionModelSelection::Model(model)) => Some(model),
        Some(
            AuthoredConditionModelSelection::Suppressed
            | AuthoredConditionModelSelection::Unresolved,
        ) => None,
        Some(AuthoredConditionModelSelection::NoAuthoredState) | None => fallback(),
    }
}

/// Resolve all source-authored W3D Draw modules for a frozen presentation
/// object. The returned vector is intentionally not deduplicated: two C++
/// Draw modules may use the same basename but own separate animation state.
///
/// A fallback list is permitted only when no active catalogue can find a
/// selectable Draw-state table for the exact Object identity. If the catalogue
/// reports source state but no safely selected model, this returns an empty
/// vector rather than borrowing a pristine or suffix-derived mesh.
pub fn resolve_presentation_draw_models_for_conditions(
    object_name: &str,
    fallback_draw_models: &[AuthoredDrawModel],
    condition_bits: u128,
) -> Vec<AuthoredDrawModel> {
    let fallback = || {
        fallback_draw_models
            .iter()
            .filter(|model| !model.model_key.trim().is_empty())
            .cloned()
            .collect()
    };

    let Some(asset_manager) = get_asset_manager() else {
        return fallback();
    };
    let Ok(asset_manager) = asset_manager.lock() else {
        // The catalogue exists but cannot be queried reliably. Rendering a
        // fallback could turn a damaged/construction state into pristine art.
        return Vec::new();
    };

    asset_manager
        .select_draw_models_for_object_conditions(object_name, condition_bits)
        .unwrap_or_else(fallback)
}

/// Apply C++ `setModelState` TransitionState playback to already-selected dest
/// models for one live object. Missing catalogue identity leaves dest as-is.
pub fn apply_live_draw_transition_playback_for_object(
    object_id: u32,
    object_name: &str,
    dest_models: Vec<AuthoredDrawModel>,
) -> Vec<AuthoredDrawModel> {
    let Some(asset_manager) = get_asset_manager() else {
        return dest_models;
    };
    let Ok(asset_manager) = asset_manager.lock() else {
        return dest_models;
    };
    let Some(definition) = asset_manager.resolve_object_definition(object_name, None) else {
        return dest_models;
    };
    definition.apply_live_draw_transition_playback(object_id, dest_models)
}

/// Resolve Draw models then play any authored `TransitionState` for this live
/// object (C++ `setModelState` / `findTransitionForSig`).
pub fn resolve_presentation_draw_models_for_live_object(
    object_id: u32,
    object_name: &str,
    fallback_draw_models: &[AuthoredDrawModel],
    condition_bits: u128,
) -> Vec<AuthoredDrawModel> {
    let dest = resolve_presentation_draw_models_for_conditions(
        object_name,
        fallback_draw_models,
        condition_bits,
    );
    let Some(asset_manager) = get_asset_manager() else {
        return dest;
    };
    let Ok(asset_manager) = asset_manager.lock() else {
        return dest;
    };
    asset_manager.apply_live_draw_transition_playback(object_id, object_name, dest)
}

/// Warm up optional caustic animation textures outside startup critical path.
pub fn warmup_caustic_textures_async(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> bool {
    let _ = device;
    let _ = queue;
    false
}

/// Queue a background task to prime raw texture data without blocking the caller.
pub fn queue_prime_texture_raw(texture_name: &str) -> bool {
    let Some(manager_arc) = get_asset_manager() else {
        return false;
    };
    let name = texture_name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return false;
    }

    texture_prime_sender(manager_arc)
        .send(name.to_string())
        .is_ok()
}

fn texture_prime_sender(manager_arc: Arc<Mutex<AssetManager>>) -> &'static Sender<String> {
    TEXTURE_PRIME_QUEUE.get_or_init(move || {
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    error!("Failed to initialize texture prime runtime: {}", err);
                    return;
                }
            };

            while let Ok(name) = rx.recv() {
                let Ok(mut manager) = manager_arc.lock() else {
                    continue;
                };
                let (texture_manager, archive_system) = {
                    let manager_ref: &mut AssetManager = &mut manager;
                    (
                        &mut manager_ref.texture_manager,
                        &mut manager_ref.archive_system,
                    )
                };

                let _ = runtime.block_on(async {
                    texture_manager
                        .prime_raw_texture(archive_system, &name)
                        .await
                });
            }
        });
        tx
    })
}

/// Convenience functions for common operations
pub async fn load_cnc_unit_model(unit_name: &str) -> Result<()> {
    let manager_arc =
        get_asset_manager().ok_or_else(|| anyhow!("Asset manager not initialized"))?;
    let handle = tokio::runtime::Handle::current();
    let unit_name = unit_name.to_string();
    let unit_name_for_task = unit_name.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
        handle.block_on(async { manager.load_cnc_model(&unit_name_for_task).await })?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("model preload task join failed: {e}"))??;

    info!("Loaded C&C unit model: {}", unit_name);
    Ok(())
}

pub async fn play_cnc_sound_effect(sound_name: &str) -> Result<()> {
    play_cnc_sound_effect_scaled(sound_name, 1.0).await
}

pub async fn play_cnc_sound_effect_scaled(sound_name: &str, volume_scale: f32) -> Result<()> {
    let manager_arc =
        get_asset_manager().ok_or_else(|| anyhow!("Asset manager not initialized"))?;
    let handle = tokio::runtime::Handle::current();
    let sound_name = sound_name.to_string();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
        handle.block_on(async {
            manager
                .play_sound_effect_scaled(&sound_name, volume_scale)
                .await
        })?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("sound task join failed: {e}"))?
}

pub fn toggle_cnc_music() {
    if let Some(manager_arc) = get_asset_manager() {
        // We need to spawn a task for the async lock
        tokio::spawn(async move {
            let Ok(manager) = manager_arc.lock() else {
                log::warn!("Skipping toggle_cnc_music: asset manager lock poisoned");
                return;
            };
            manager.toggle_background_music();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{
        AuthoredDrawWeaponBoneBindings, AuthoredDrawWeaponBoneSlot, W3dHierarchy, W3dHlod,
        W3dHlodLod, W3dHmodel, W3dHmodelNode, W3dHmodelNodeKind, W3dPivot,
    };
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn weapon_barrel_topology_cache_model(name: &str, barrel_count: u8) -> W3DModel {
        let mut model = W3DModel::new(name.to_string());
        let mut pivots = vec![W3dPivot {
            name: "ROOT".to_string(),
            parent_idx: u32::MAX,
            translation: [0.0; 3],
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }];
        pivots.extend((1..=barrel_count).map(|index| W3dPivot {
            name: format!("Recoil{index:02}"),
            parent_idx: 0,
            translation: [0.0; 3],
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }));
        model.hierarchy = Some(W3dHierarchy {
            name: "BarrelHierarchy".to_string(),
            pivots,
            pivot_fixups: Vec::new(),
        });
        model.hlods = vec![W3dHlod {
            version: 0,
            name: "BarrelHlod".to_string(),
            hierarchy_name: "BarrelHierarchy".to_string(),
            lods: vec![W3dHlodLod {
                max_screen_size: 1.0,
                subobjects: Vec::new(),
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        }];
        model
    }

    fn weapon_barrel_topology_cache_draw_model(
        model_key: &str,
        source_fields_valid: bool,
        recoil_base: Option<&str>,
    ) -> AuthoredDrawModel {
        AuthoredDrawModel {
            model_key: model_key.to_string(),
            weapon_bone_bindings: AuthoredDrawWeaponBoneBindings {
                slots: [
                    AuthoredDrawWeaponBoneSlot {
                        recoil_bone_base: recoil_base.map(str::to_string),
                        ..Default::default()
                    },
                    AuthoredDrawWeaponBoneSlot::default(),
                    AuthoredDrawWeaponBoneSlot::default(),
                ],
                source_fields_valid,
            },
            ..Default::default()
        }
    }

    fn prototype_source_model(mesh_container: &str, mesh_name: &str, hlod_name: &str) -> W3DModel {
        let mut model = W3DModel::new("strict_source".to_string());
        let mut mesh = W3DMesh::new(mesh_name.to_string());
        mesh.container_name = mesh_container.to_string();
        model.meshes.push(mesh);
        if !hlod_name.is_empty() {
            model.hlods.push(W3dHlod {
                version: 0,
                name: hlod_name.to_string(),
                hierarchy_name: String::new(),
                lods: Vec::new(),
                aggregates: None,
                proxies: None,
                has_unrendered_aggregates: false,
                has_invalid_trailing_records: false,
            });
        }
        model
    }

    #[test]
    fn strict_render_object_registry_indexes_only_exact_mesh_and_hlod_names() {
        let mut registry = W3dRenderObjectPrototypeRegistry::default();
        registry.register_source_model(
            "ATTACHED_MODEL",
            prototype_source_model("ATTACHED_MODEL", "Body", "ATTACHED_MODEL"),
        );

        let mesh = registry
            .lookup("attached_model.body")
            .expect("C++ Find_Prototype is case-insensitive for the complete mesh identity");
        assert_eq!(mesh.full_name(), "ATTACHED_MODEL.Body");
        assert_eq!(mesh.source_file_stem(), "ATTACHED_MODEL");
        assert_eq!(
            mesh.kind(),
            W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 }
        );
        assert!(registry.source_model(&mesh).is_some());

        let hlod = registry
            .lookup("attached_model")
            .expect("source HLOD header name is also an exact prototype identity");
        assert_eq!(
            hlod.kind(),
            W3dRenderObjectPrototypeKind::Hlod { hlod_index: 0 }
        );

        assert!(registry.lookup("Body").is_none(), "no bare mesh alias");
        assert!(
            registry.lookup("ATTACHED_MODEL.Body.extra").is_none(),
            "no suffix/whole-file fallback"
        );
    }

    #[test]
    fn strict_render_object_registry_retains_each_hlod_chunk_index() {
        let mut source = prototype_source_model("MULTI_HLOD", "Body", "");
        source.hlods = vec![
            W3dHlod {
                version: 0x0001_0000,
                name: "FIRST_HLOD".to_string(),
                hierarchy_name: "FIRST_TREE".to_string(),
                lods: vec![W3dHlodLod {
                    max_screen_size: f32::MAX,
                    subobjects: Vec::new(),
                }],
                aggregates: None,
                proxies: None,
                has_unrendered_aggregates: false,
                has_invalid_trailing_records: false,
            },
            W3dHlod {
                version: 0x0001_0000,
                name: "SECOND_HLOD".to_string(),
                hierarchy_name: "SECOND_TREE".to_string(),
                lods: vec![W3dHlodLod {
                    max_screen_size: f32::MAX,
                    subobjects: Vec::new(),
                }],
                aggregates: None,
                proxies: None,
                has_unrendered_aggregates: false,
                has_invalid_trailing_records: false,
            },
        ];

        let mut registry = W3dRenderObjectPrototypeRegistry::default();
        registry.register_source_model("MULTI_HLOD", source);

        assert_eq!(
            registry
                .lookup("first_hlod")
                .map(|prototype| prototype.kind()),
            Some(W3dRenderObjectPrototypeKind::Hlod { hlod_index: 0 })
        );
        assert_eq!(
            registry
                .lookup("SECOND_HLOD")
                .map(|prototype| prototype.kind()),
            Some(W3dRenderObjectPrototypeKind::Hlod { hlod_index: 1 }),
            "each C++ HLOD loader record must retain its immutable source index"
        );
    }

    #[test]
    fn strict_render_object_registry_indexes_hmodel_by_its_exact_header_name() {
        let mut model = W3DModel::new("hmodel_source".to_string());
        model.hmodels.push(W3dHmodel {
            version: 0x0004_0002,
            name: "ATTACHED_HMODEL".to_string(),
            hierarchy_name: "ATTACHED_TREE".to_string(),
            nodes: vec![W3dHmodelNode {
                name: "ATTACHED_HMODEL.Body".to_string(),
                bone_index: 0,
                kind: W3dHmodelNodeKind::Node,
            }],
            source_snap_points: Vec::new(),
            has_invalid_records: false,
        });

        let mut registry = W3dRenderObjectPrototypeRegistry::default();
        registry.register_source_model("ATTACHED_HMODEL", model);

        let hmodel = registry
            .lookup("attached_hmodel")
            .expect("C++ Find_Prototype finds HMODEL by its complete header name");
        assert_eq!(hmodel.full_name(), "ATTACHED_HMODEL");
        assert_eq!(
            hmodel.kind(),
            W3dRenderObjectPrototypeKind::Hmodel { hmodel_index: 0 }
        );
        assert!(registry.source_model(&hmodel).is_some());
        assert!(
            registry.lookup("ATTACHED_HMODEL.Body").is_none(),
            "an HMODEL connection is a later exact prototype lookup, not an alias registered from its leaf"
        );
    }

    #[test]
    fn strict_render_object_registry_preserves_the_first_duplicate_name() {
        let mut registry = W3dRenderObjectPrototypeRegistry::default();
        registry.register_source_model("FIRST", prototype_source_model("DUPLICATE", "Mesh", ""));
        registry.register_source_model("SECOND", prototype_source_model("DUPLICATE", "Mesh", ""));

        let prototype = registry
            .lookup("duplicate.mesh")
            .expect("the first source registered the prototype");
        assert_eq!(prototype.source_file_stem(), "FIRST");
        assert!(
            registry.source_model(&prototype).is_some(),
            "the duplicate must not replace the original source model"
        );
    }

    #[test]
    fn strict_render_object_on_demand_uses_first_dot_stem_then_relooks_up_full_name() {
        let mut registry = W3dRenderObjectPrototypeRegistry::default();
        let mut requested_stems = Vec::new();
        let prototype = resolve_w3d_render_object_prototype_with_loader(
            &mut registry,
            "ATTACHED_MODEL.Body.extra",
            |source_stem| {
                requested_stems.push(source_stem.to_string());
                Some(prototype_source_model("ATTACHED_MODEL", "Body.extra", ""))
            },
        )
        .expect("the full name must be re-looked up after the exact stem source loads");

        assert_eq!(requested_stems, ["ATTACHED_MODEL"]);
        assert_eq!(prototype.full_name(), "ATTACHED_MODEL.Body.extra");
        assert_eq!(
            prototype.kind(),
            W3dRenderObjectPrototypeKind::Mesh { mesh_index: 0 }
        );
    }

    #[test]
    fn strict_render_object_on_demand_never_returns_an_unmatched_whole_file() {
        let mut registry = W3dRenderObjectPrototypeRegistry::default();
        let unresolved = resolve_w3d_render_object_prototype_with_loader(
            &mut registry,
            "ATTACHED_MODEL.Body",
            |_| Some(prototype_source_model("ATTACHED_MODEL", "Other", "")),
        );

        assert!(
            unresolved.is_none(),
            "a loaded source file is not an acceptable substitute for its missing exact prototype"
        );
        assert!(registry.lookup("ATTACHED_MODEL.Other").is_some());
    }

    #[test]
    fn strict_render_object_stem_keeps_cxx_first_dot_rule_and_rejects_paths() {
        assert_eq!(
            w3d_render_object_source_stem("ATTACHED_MODEL.Body.extra"),
            Some("ATTACHED_MODEL")
        );
        assert_eq!(
            w3d_render_object_source_stem("ATTACHED_MODEL"),
            Some("ATTACHED_MODEL")
        );
        assert_eq!(w3d_render_object_source_stem(""), None);
        assert_eq!(w3d_render_object_source_stem("../ATTACHED.Body"), None);
    }

    #[test]
    fn weapon_barrel_topology_cache_uses_the_first_nonzero_draw_module_answer() {
        let first = weapon_barrel_topology_cache_model("first", 2);
        let later = weapon_barrel_topology_cache_model("later", 3);
        let result = cached_weapon_barrel_counts_for_draw_models(
            &[
                weapon_barrel_topology_cache_draw_model("first", true, Some("Recoil")),
                weapon_barrel_topology_cache_draw_model("later", true, Some("Recoil")),
            ],
            |model_key| match model_key {
                "first" => Some(&first),
                "later" => Some(&later),
                other => panic!("unexpected source Draw model {other}"),
            },
        )
        .expect("both selected exact Draw models are resident and valid");

        assert_eq!(result, [Some(2), None, None]);
    }

    #[test]
    fn weapon_barrel_topology_cache_allows_a_later_module_after_a_known_zero() {
        let later = weapon_barrel_topology_cache_model("later", 2);
        let result = cached_weapon_barrel_counts_for_draw_models(
            &[
                weapon_barrel_topology_cache_draw_model("zero", true, None),
                weapon_barrel_topology_cache_draw_model("later", true, Some("Recoil")),
            ],
            |model_key| {
                assert_eq!(
                    model_key, "later",
                    "a source state with no bases is a known zero"
                );
                Some(&later)
            },
        )
        .expect("known zero followed by a valid exact source is resolved");

        assert_eq!(result, [Some(2), None, None]);
    }

    #[test]
    fn weapon_barrel_topology_cache_rejects_an_unloaded_or_malformed_earlier_source() {
        let unknown = cached_weapon_barrel_counts_for_draw_models(
            &[
                weapon_barrel_topology_cache_draw_model("missing", true, Some("Recoil")),
                weapon_barrel_topology_cache_draw_model("later", true, Some("Recoil")),
            ],
            |model_key| {
                assert_eq!(
                    model_key, "missing",
                    "later modules must not bypass an unknown source"
                );
                None
            },
        );
        assert!(unknown.is_none());

        let malformed = cached_weapon_barrel_counts_for_draw_models(
            &[weapon_barrel_topology_cache_draw_model(
                "ignored",
                false,
                Some("Recoil"),
            )],
            |_| panic!("malformed retained source must fail before any model lookup"),
        );
        assert!(malformed.is_none());
    }

    #[test]
    fn test_asset_statistics() {
        let stats = AssetStatistics {
            archive_stats: ArchiveStatistics {
                total_archives: 10,
                total_files: 5000,
                unique_files: 4500,
            },
            models_cached: 25,
            textures_cached: 100,
            textures_raw_cached: 150,
            initialized: true,
        };

        assert!(stats.initialized);
        assert_eq!(stats.models_cached, 25);
        assert_eq!(stats.archive_stats.total_archives, 10);
    }

    #[test]
    fn test_asset_search_results() {
        let mut results = AssetSearchResults::default();
        results.models.push("tank.w3d".to_string());
        results.textures.push("tank_diffuse.tga".to_string());
        results.audio.push("engine.wav".to_string());

        assert_eq!(results.total_results(), 3);
        assert_eq!(results.models.len(), 1);
        assert_eq!(results.textures.len(), 1);
        assert_eq!(results.audio.len(), 1);
    }

    #[test]
    fn tga_update_selection_skips_caustic_and_prefers_stale_files() {
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let stale = TextureUpdateEntry {
            tga_path: PathBuf::from("Art/Textures/Units/tank.tga"),
            dds_path: PathBuf::from("Art/Textures/Units/tank.dds"),
            tga_modified: base,
            dds_modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(90)),
        };
        let fresh = TextureUpdateEntry {
            tga_path: PathBuf::from("Art/Textures/Units/jeep.tga"),
            dds_path: PathBuf::from("Art/Textures/Units/jeep.dds"),
            tga_modified: base,
            dds_modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(110)),
        };
        let missing_dds = TextureUpdateEntry {
            tga_path: PathBuf::from("Art/Textures/Units/apc.tga"),
            dds_path: PathBuf::from("Art/Textures/Units/apc.dds"),
            tga_modified: base,
            dds_modified: None,
        };
        let skipped = TextureUpdateEntry {
            tga_path: PathBuf::from("Art/Textures/Water/caustic_sheet.tga"),
            dds_path: PathBuf::from("Art/Textures/Water/caustic_sheet.dds"),
            tga_modified: base,
            dds_modified: None,
        };

        let selected =
            select_tga_to_dds_entries(vec![stale.clone(), fresh, missing_dds.clone(), skipped]);

        assert_eq!(selected, vec![stale, missing_dds]);
    }

    #[test]
    fn archive_fingerprint_matching_is_case_insensitive_and_path_agnostic() {
        assert!(archive_filename_matches(
            Some(r"C:\Games\Generals\genseczh.big"),
            "genseczh.big"
        ));
        assert!(archive_filename_matches(
            Some("/opt/generals/MUSICZH.BIG"),
            r"C:\Temp\musiczh.big"
        ));
        assert!(!archive_filename_matches(
            Some(r"C:\Games\Generals\other.big"),
            "genseczh.big"
        ));
        assert!(!archive_filename_matches(None, "musiczh.big"));
    }
}
