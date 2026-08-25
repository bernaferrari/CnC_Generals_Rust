//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_format::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh::*;
use super::w3d_mesh_build::*;
use super::w3d_model::*;
use super::*;

pub struct W3DLoader;

impl Default for W3DLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip directory + extension from a W3D model request (keeps original casing).
pub(super) fn w3d_model_basename(model_name: &str) -> &str {
    model_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(model_name)
        .trim()
        .trim_end_matches(".w3d")
        .trim_end_matches(".W3D")
}

/// Archive path candidates for `W3DLoader::load_model`.
///
/// Retail ZH BIG archives store mixed-case paths such as
/// `Art/W3D/ABBtCmdHQ.W3D`. Linux extract trees are case-sensitive, so
/// both `art/w3d/*.w3d` and `Art/W3D/*.W3D` (plus remapped / retail
/// basenames) must be tried. BIG lookup is already case-insensitive;
/// variants exist so a case-sensitive overlay / extract tree still hits.
pub fn w3d_archive_path_variants(model_name: &str) -> Vec<String> {
    use crate::assets::mesh_asset_resolve::{
        remap_model_key_alias, retail_w3d_basename_for_key, w3d_filename_variants,
    };

    let base = w3d_model_basename(model_name);
    let remapped = remap_model_key_alias(base);
    let retail = retail_w3d_basename_for_key(base);

    let mut stems = Vec::new();
    let mut push_stem = |stem: &str| {
        if stem.is_empty() {
            return;
        }
        if !stems.iter().any(|existing: &String| existing == stem) {
            stems.push(stem.to_string());
        }
    };
    // Requested basename first (preserves the original two load_model paths).
    push_stem(base);
    // Retail archive casing (AIRanger_S, ABBtCmdHQ, AvHummer, …).
    push_stem(&retail);
    // Remapped alias (AmericaCommandCenter → abbtcmdhq).
    push_stem(&remapped);

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push_path = |path: String| {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    };

    for stem in &stems {
        // Existing load_model candidates, then retail BIG / extract mixed-case.
        push_path(format!("art/w3d/{stem}.w3d"));
        push_path(format!("{stem}.w3d"));
        push_path(format!("Art/W3D/{stem}.w3d"));
        push_path(format!("Art/W3D/{stem}.W3D"));
        push_path(format!("art/w3d/{stem}.W3D"));

        // Extra filename casings from mesh residual tables.
        for name in w3d_filename_variants(stem) {
            push_path(format!("art/w3d/{name}"));
            push_path(name.clone());
            push_path(format!("Art/W3D/{name}"));
        }
    }

    out
}

/// Exact C++ companion-file spelling for a raw HAnim selected by
/// `W3DModelDraw`.
///
/// `WW3DAssetManager::Get_HAnim` resolves an HAnim by name first and, on a
/// miss, takes the substring after the first dot in `Hierarchy.Animation`,
/// then loads `Animation.w3d`. This intentionally does not use any model-key
/// alias or filename-family table.
pub fn w3d_companion_animation_filename(identity: &str) -> Option<String> {
    let (_, animation) = split_w3d_draw_animation_identity(identity)?;
    Some(format!("{animation}.w3d"))
}

/// Case-only archive/extract variants for one exact companion HAnim file.
///
/// BIG lookups are case-insensitive, while retail extracted trees may not be.
/// These paths vary only storage/path casing; they never remap a Draw identity
/// to a different model or scan an archive for similarly named motions.
pub fn w3d_companion_animation_archive_path_variants(identity: &str) -> Option<Vec<String>> {
    let (_, animation) = split_w3d_draw_animation_identity(identity)?;
    let mut stems = Vec::new();
    for stem in [
        animation.to_string(),
        animation.to_ascii_lowercase(),
        animation.to_ascii_uppercase(),
    ] {
        if !stems.iter().any(|existing: &String| existing == &stem) {
            stems.push(stem);
        }
    }

    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |path: String| {
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    };
    for stem in stems {
        push(format!("art/w3d/{stem}.w3d"));
        push(format!("{stem}.w3d"));
        push(format!("Art/W3D/{stem}.w3d"));
        push(format!("Art/W3D/{stem}.W3D"));
        push(format!("art/w3d/{stem}.W3D"));
    }
    Some(paths)
}

/// C++ `WW3DAssetManager::Get_HTree` load-on-demand filenames.
///
/// On a miss it loads `{HierarchyName}.w3d`, then `..\{HierarchyName}.w3d`.
/// Extra `art/w3d` casings are storage locations for extracted trees, not a
/// new asset identity. Do not remap the HLOD name through model aliases.
pub(super) fn w3d_companion_hierarchy_archive_path_variants(hierarchy_name: &str) -> Vec<String> {
    let name = hierarchy_name.trim();
    if name.is_empty() {
        return Vec::new();
    }
    let mut stems = Vec::new();
    for stem in [
        name.to_string(),
        name.to_ascii_lowercase(),
        name.to_ascii_uppercase(),
    ] {
        if !stems.iter().any(|existing: &String| existing == &stem) {
            stems.push(stem);
        }
    }

    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |path: String| {
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    };
    for stem in stems {
        // C++ Get_HTree order first, then extracted-tree storage spellings.
        push(format!("{stem}.w3d"));
        push(format!("../{stem}.w3d"));
        push(format!("art/w3d/{stem}.w3d"));
        push(format!("Art/W3D/{stem}.w3d"));
        push(format!("Art/W3D/{stem}.W3D"));
        push(format!("art/w3d/{stem}.W3D"));
    }
    paths
}

impl W3DLoader {
    /// Create new W3D loader
    pub fn new() -> Self {
        Self
    }

    /// Load W3D model from BIG archive
    pub async fn load_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        model_name: &str,
    ) -> Result<W3DModel> {
        debug!("Loading W3D model: {}", model_name);

        // C++ parity: deterministic model lookup (requested file, remapped alias,
        // and retail Art/W3D mixed-case archive location).
        let base_name = w3d_model_basename(model_name);
        let w3d_filename = format!("{base_name}.w3d");
        let path_variations = w3d_archive_path_variants(model_name);

        let mut last_error = None;
        for path_variant in path_variations {
            debug!("Trying W3D path: {}", path_variant);
            match archive_system.open_file(&path_variant).await {
                Ok(model_data) => {
                    debug!("Found W3D file at path: {}", path_variant);
                    debug!("Loaded W3D file data: {} bytes", model_data.len());
                    let mut model = self.parse_w3d_data(&model_data, base_name.to_string())?;
                    self.attach_missing_named_hlod_hierarchies(archive_system, &mut model)
                        .await;
                    return Ok(model);
                }
                Err(e) => {
                    debug!("Failed to find W3D at {}: {}", path_variant, e);
                    last_error = Some(e);
                }
            }
        }

        Err(anyhow!(
            "Failed to load W3D file {}: {}",
            w3d_filename,
            last_error.unwrap_or_else(|| anyhow!("file not found"))
        ))
    }

    /// Load one exact raw-animation companion according to the C++ HAnim
    /// fallback rule. The returned clip remains separate from a geometry
    /// model; callers must still validate its hierarchy before binding it.
    pub async fn load_companion_animation(
        &self,
        archive_system: &mut ArchiveFileSystem,
        identity: &str,
    ) -> Result<W3dAnimation> {
        let filename = w3d_companion_animation_filename(identity)
            .ok_or_else(|| anyhow!("invalid W3D Draw animation identity '{identity}'"))?;
        let candidates = w3d_companion_animation_archive_path_variants(identity)
            .expect("validated companion identity must yield paths");
        let mut last_error = None;

        for candidate in candidates {
            match archive_system.open_file(&candidate).await {
                Ok(data) => match self.load_companion_animation_from_bytes(&data, identity) {
                    Ok(animation) => return Ok(animation),
                    Err(error) => {
                        last_error = Some(error);
                        debug!(
                            "W3D companion '{}' at '{}' did not contain its exact HAnim: {}",
                            identity,
                            candidate,
                            last_error.as_ref().expect("just assigned error")
                        );
                    }
                },
                Err(error) => last_error = Some(error),
            }
        }

        Err(anyhow!(
            "failed to load exact W3D companion '{}' for Draw animation '{}': {}",
            filename,
            identity,
            last_error.unwrap_or_else(|| anyhow!("file not found"))
        ))
    }

    /// Parse `{HierarchyName}.w3d` bytes and retain only that named HTree.
    /// Returns true when the HLOD can now resolve `source_hierarchy_for_hlod`.
    pub(super) fn import_named_hlod_hierarchy_from_bytes(
        &self,
        model: &mut W3DModel,
        hierarchy_name: &str,
        data: &[u8],
    ) -> bool {
        match self.parse_w3d_animation_data(data, hierarchy_name.to_string()) {
            Ok(companion) => {
                model.import_named_hierarchy_from(&companion, hierarchy_name);
                model.hlods.iter().any(|hlod| {
                    hlod.hierarchy_name.eq_ignore_ascii_case(hierarchy_name)
                        && model.source_hierarchy_for_hlod(hlod).is_some()
                })
            }
            Err(error) => {
                debug!(
                    "HLOD companion HTree '{}' did not parse: {}",
                    hierarchy_name, error
                );
                false
            }
        }
    }

    /// C++ `Get_HTree` load-on-demand: when an HLOD names a tree that is not
    /// in this file, open `{HierarchyName}.w3d` through the existing archive
    /// path set and retain only that named HTree. Missing companions stay
    /// fail-closed (`Init_Default`); they must not abort geometry load.
    async fn attach_missing_named_hlod_hierarchies(
        &self,
        archive_system: &mut ArchiveFileSystem,
        model: &mut W3DModel,
    ) {
        let missing = model.missing_named_hlod_hierarchy_names();
        for hierarchy_name in missing {
            let current_stem = w3d_model_basename(&model.name);
            if current_stem.eq_ignore_ascii_case(hierarchy_name.trim()) {
                continue;
            }
            let mut attached = false;
            for candidate in w3d_companion_hierarchy_archive_path_variants(&hierarchy_name) {
                match archive_system.open_file(&candidate).await {
                    Ok(data) => {
                        if self.import_named_hlod_hierarchy_from_bytes(
                            model,
                            &hierarchy_name,
                            &data,
                        ) {
                            attached = true;
                            break;
                        }
                    }
                    Err(error) => {
                        debug!(
                            "HLOD companion HTree '{}' not at '{}': {}",
                            hierarchy_name, candidate, error
                        );
                    }
                }
            }
            if !attached {
                debug!(
                    "HLOD named HTree '{}' was not found for '{}'; using C++ Init_Default",
                    hierarchy_name, model.name
                );
            }
        }
    }

    /// Filesystem analog of C++ `Get_HTree` load-on-demand for residual
    /// `load_model_from_path`. Looks next to the geometry file, then the
    /// parent directory, then extracted `art/w3d` spellings.
    fn attach_missing_named_hlod_hierarchies_from_dir(
        &self,
        model: &mut W3DModel,
        dir: &std::path::Path,
    ) {
        let missing = model.missing_named_hlod_hierarchy_names();
        for hierarchy_name in missing {
            let current_stem = w3d_model_basename(&model.name);
            if current_stem.eq_ignore_ascii_case(hierarchy_name.trim()) {
                continue;
            }
            let mut attached = false;
            for candidate in w3d_companion_hierarchy_archive_path_variants(&hierarchy_name) {
                let path = dir.join(&candidate);
                match std::fs::read(&path) {
                    Ok(data) => {
                        if self.import_named_hlod_hierarchy_from_bytes(
                            model,
                            &hierarchy_name,
                            &data,
                        ) {
                            attached = true;
                            break;
                        }
                    }
                    Err(error) => {
                        debug!(
                            "HLOD companion HTree '{}' not at '{}': {}",
                            hierarchy_name,
                            path.display(),
                            error
                        );
                    }
                }
            }
            if !attached {
                debug!(
                    "HLOD named HTree '{}' was not found beside '{}'; using C++ Init_Default",
                    hierarchy_name, model.name
                );
            }
        }
    }

    /// Parse W3D binary data using the legacy chunk parser path for strict C++ parity.
    pub(super) fn parse_w3d_data(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        self.parse_w3d_data_legacy(data, model_name, false)
    }

    /// Parse an HAnim companion stream. Retail raw-animation W3Ds commonly
    /// contain no mesh chunks at all, unlike a geometry model, so the normal
    /// model parser's no-mesh rejection is not applicable here.
    pub(super) fn parse_w3d_animation_data(
        &self,
        data: &[u8],
        model_name: String,
    ) -> Result<W3DModel> {
        self.parse_w3d_data_legacy(data, model_name, true)
    }

    /// Parse a W3D model from already-loaded bytes (filesystem / archive residual path).
    ///
    /// Used by mesh asset resolve when assets are present without a full GPU/AssetManager
    /// init. Fail-closed: not full material/animation retail parity.
    pub fn load_model_from_bytes(&self, data: &[u8], model_name: &str) -> Result<W3DModel> {
        let base_name = model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D");
        if data.is_empty() {
            return Err(anyhow!("empty W3D payload for '{}'", base_name));
        }
        self.parse_w3d_data(data, base_name.to_string())
    }

    /// Parse a raw-animation W3D payload and select only the exact full
    /// `Hierarchy.Animation` record requested by the frozen Draw state.
    pub fn load_companion_animation_from_bytes(
        &self,
        data: &[u8],
        identity: &str,
    ) -> Result<W3dAnimation> {
        let filename = w3d_companion_animation_filename(identity)
            .ok_or_else(|| anyhow!("invalid W3D Draw animation identity '{identity}'"))?;
        if data.is_empty() {
            return Err(anyhow!("empty W3D companion payload for '{filename}'"));
        }
        let model = self.parse_w3d_animation_data(data, filename.clone())?;
        model
            .animations
            .into_iter()
            .find(|animation| animation.matches_draw_identity(identity))
            .ok_or_else(|| {
                anyhow!(
                    "W3D companion '{}' contains no exact HAnim '{}'",
                    filename,
                    identity
                )
            })
    }

    /// Load a W3D model from a filesystem path when present (tests / residual resolve).
    pub fn load_model_from_path(&self, path: &std::path::Path) -> Result<W3DModel> {
        let data = std::fs::read(path)
            .map_err(|e| anyhow!("failed to read W3D '{}': {e}", path.display()))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
        let mut model = self.load_model_from_bytes(&data, name)?;
        if let Some(dir) = path.parent() {
            self.attach_missing_named_hlod_hierarchies_from_dir(&mut model, dir);
        }
        Ok(model)
    }

    // Non-parity companion/heuristic model-family merge path removed.
    // The active parser path is strict legacy chunk parsing (`parse_w3d_data_legacy`).

    // Non-parity companion source loading and alternate ww3d-assets parsing entrypoints removed.

    pub(super) fn stage_channel_to_uv_source(channel: u8) -> UVSource {
        match channel {
            0 => UVSource::UV0,
            1 => UVSource::UV1,
            2 => UVSource::UV2,
            _ => UVSource::UV3,
        }
    }

    pub(super) fn stage_mapping_mut(
        material: &mut W3DMaterial,
        stage: usize,
        create: bool,
    ) -> Option<&mut TextureStageMapping> {
        match stage {
            0 => Some(&mut material.stage0_mapping),
            1 => {
                if material.stage1_mapping.is_none() && create {
                    material.stage1_mapping = Some(TextureStageMapping::default());
                }
                material.stage1_mapping.as_mut()
            }
            2 => {
                if material.stage2_mapping.is_none() && create {
                    material.stage2_mapping = Some(TextureStageMapping::default());
                }
                material.stage2_mapping.as_mut()
            }
            3 => {
                if material.stage3_mapping.is_none() && create {
                    material.stage3_mapping = Some(TextureStageMapping::default());
                }
                material.stage3_mapping.as_mut()
            }
            _ => None,
        }
    }

    pub(super) fn apply_material_stage_mappings(material: &mut W3DMaterial, mesh: &W3DMesh) {
        for stage_idx in 0..4 {
            let create = stage_idx == 0
                || mesh.stage_uv_channels.get(stage_idx).is_some()
                || Self::stage_texture_from_mesh(mesh, 0, stage_idx).is_some();

            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, create) {
                if mapping.texture_name.is_none() {
                    if let Some(name) = Self::stage_texture_from_mesh(mesh, 0, stage_idx) {
                        mapping.texture_name = Some(name);
                    }
                }
            }
        }

        for (stage_idx, &channel) in mesh.stage_uv_channels.iter().enumerate().take(4) {
            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, true) {
                mapping.uv_source = Self::stage_channel_to_uv_source(channel);
            }
        }

        // Match the common C++ material path more closely: once stage 0 resolves to a texture,
        // expose that as the primary material texture too so caches/debugging/legacy consumers
        // don't diverge from the active pass state.
        if material.texture_name.is_none() {
            material.texture_name = material.stage0_mapping.texture_name.clone();
        }
    }

    pub(super) fn stage_texture_from_mesh(
        mesh: &W3DMesh,
        pass_index: usize,
        stage_index: usize,
    ) -> Option<String> {
        if let Some(stage_sets) = mesh.per_pass_stage_texture_names.get(pass_index) {
            if let Some(names) = stage_sets.get(stage_index) {
                if let Some(name) = names.iter().find(|n| !n.is_empty()) {
                    return Some(name.clone());
                }
            }
        }

        mesh.stage_texture_names_from_ids(pass_index, stage_index)
            .into_iter()
            .find(|name| !name.is_empty())
    }

    /// Parse W3D binary data using the legacy chunk parser (fallback path)
    pub(super) fn parse_w3d_data_legacy(
        &self,
        data: &[u8],
        model_name: String,
        allow_animation_only: bool,
    ) -> Result<W3DModel> {
        if data.len() < 8 {
            return Err(anyhow!("W3D file too small: {} bytes", data.len()));
        }

        let mut model = W3DModel::new(model_name);
        let mut offset = 0usize;

        // Parse W3D chunks with safety counter to prevent infinite loops
        let mut chunk_counter = 0;
        pub(super) const MAX_CHUNKS: usize = 10000; // Safety limit to prevent infinite loops

        while offset + 8 <= data.len() && chunk_counter < MAX_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Handle W3D chunk size format: MSB indicates container chunk
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize; // Clear MSB to get actual size

            debug!(
                "W3D chunk: type=0x{:08X}, raw_size=0x{:08X}, size={}, container={}",
                chunk_type, raw_chunk_size, chunk_size, is_container_chunk
            );

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "W3D chunk extends beyond file: type 0x{:08X}, size {} (raw: 0x{:08X})",
                    chunk_type, chunk_size, raw_chunk_size
                );
                break;
            }

            // Additional safety checks to prevent infinite loops
            if chunk_size == 0 {
                warn!(
                    "Zero-sized chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8; // Skip just the header
                continue;
            }

            if chunk_size > data.len() {
                warn!(
                    "Chunk size {} exceeds total file size {} - aborting parsing",
                    chunk_size,
                    data.len()
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Parsing W3D mesh chunk, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse W3D mesh chunk");
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!("Parsing W3D hierarchy chunk, size: {}", chunk_size);
                    match self.parse_hierarchy_chunk(chunk_data) {
                        Ok(hierarchy) => {
                            debug!(
                                "Parsed hierarchy '{}' with {} pivots",
                                hierarchy.name,
                                hierarchy.pivots.len()
                            );
                            model.retain_source_hierarchy(hierarchy);
                        }
                        Err(e) => warn!("Failed to parse hierarchy chunk: {}", e),
                    }
                    if is_container_chunk {
                        self.parse_container_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_MATERIALS3 => {
                    debug!("Parsing W3D materials3 container, size: {}", chunk_size);
                    // Parse materials3 container - contains material definitions with texture names
                    if is_container_chunk {
                        self.parse_materials3_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Parsing W3D textures container, size: {}", chunk_size);
                    // Parse textures container - contains individual texture definitions
                    if is_container_chunk {
                        self.parse_textures_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Parsing W3D animation chunk, size: {}", chunk_size);
                    match self.parse_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            debug!(
                                "Parsed animation '{}' ({} frames @ {}fps)",
                                animation.name, animation.num_frames, animation.frame_rate
                            );
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse animation chunk: {}", e),
                    }
                }
                W3D_CHUNK_COMPRESSED_ANIMATION => {
                    debug!(
                        "Parsing W3D compressed animation chunk (timecoded/adaptive delta), size: {}",
                        chunk_size
                    );
                    match self.parse_compressed_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            debug!(
                                "Parsed compressed animation '{}' ({} frames @ {}fps)",
                                animation.name, animation.num_frames, animation.frame_rate
                            );
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse compressed animation chunk: {}", e),
                    }
                }
                W3D_CHUNK_HMODEL => {
                    debug!("Found W3D hierarchical model chunk, size: {}", chunk_size);
                    if !is_container_chunk {
                        warn!("HMODEL chunk is not a container; skipping unsafe prototype");
                    } else {
                        match self.parse_hmodel_chunk(chunk_data) {
                            Ok(hmodel) => model.hmodels.push(hmodel),
                            Err(error) => {
                                warn!("Failed to parse HMODEL definition: {}", error);
                            }
                        }
                    }
                }
                W3D_CHUNK_LODMODEL => {
                    debug!("Found W3D LOD model chunk, size: {}", chunk_size);
                    if let Some(dist_lod) =
                        super::w3d_collection_aggregate::parse_dist_lod_chunk(chunk_data)
                    {
                        model.dist_lods.push(dist_lod);
                    }
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse LOD model container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_HLOD => {
                    debug!(
                        "Found W3D HLOD (Hierarchical LOD) chunk, size: {}",
                        chunk_size
                    );
                    if !is_container_chunk {
                        model.hlod_parse_failed = true;
                        warn!(
                            "HLOD chunk is not marked as a container; suppressing unsafe mesh fallback"
                        );
                    } else {
                        match self.parse_hlod_chunk(chunk_data) {
                            Ok(hlod) => model.hlods.push(hlod),
                            Err(e) => {
                                model.hlod_parse_failed = true;
                                warn!("Failed to parse HLOD container: {}", e);
                            }
                        }
                    }
                }
                W3D_CHUNK_EMITTER => {
                    if let Some(emitter) =
                        super::w3d_emitter_loader::parse_emitter_chunk(chunk_data)
                    {
                        model.emitters.push(emitter);
                    }
                }
                W3D_CHUNK_DAZZLE => {
                    if let Some(dazzle) = super::w3d_dazzle_loader::parse_dazzle_chunk(chunk_data) {
                        model.dazzles.push(dazzle);
                    }
                }
                W3D_CHUNK_BOX => {
                    if let Some(box_proto) =
                        super::w3d_primitive_protos::parse_box_chunk(chunk_data)
                    {
                        model.boxes.push(box_proto);
                    }
                }
                W3D_CHUNK_RING => {
                    if let Some(ring) = super::w3d_primitive_protos::parse_ring_chunk(chunk_data) {
                        model.rings.push(ring);
                    }
                }
                W3D_CHUNK_SPHERE => {
                    if let Some(sphere) =
                        super::w3d_primitive_protos::parse_sphere_chunk(chunk_data)
                    {
                        model.spheres.push(sphere);
                    }
                }
                W3D_CHUNK_NULL_OBJECT => {
                    if let Some(null) = super::w3d_primitive_protos::parse_null_chunk(chunk_data) {
                        model.nulls.push(null);
                    }
                }
                W3D_CHUNK_COLLECTION => {
                    if let Some(collection) =
                        super::w3d_collection_aggregate::parse_collection_chunk(chunk_data)
                    {
                        model.collections.push(collection);
                    }
                }
                W3D_CHUNK_AGGREGATE => {
                    if let Some(aggregate) =
                        super::w3d_collection_aggregate::parse_aggregate_chunk(chunk_data)
                    {
                        model.aggregates.push(aggregate);
                    }
                }
                _ => {
                    debug!("Unknown W3D chunk type: 0x{:08X}", chunk_type);
                    // If it's a container chunk, try to parse it recursively
                    if is_container_chunk && chunk_size > 0 {
                        debug!("  -> Container chunk, parsing recursively");
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!(
                                "Failed to parse container chunk 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CHUNKS {
            warn!(
                "⚠️  W3D chunk parsing hit safety limit ({} chunks) - possible malformed file",
                MAX_CHUNKS
            );
        }

        if model.meshes.is_empty()
            && model.hlods.is_empty()
            && model.hmodels.is_empty()
            && model.emitters.is_empty()
            && model.dazzles.is_empty()
            && model.boxes.is_empty()
            && model.rings.is_empty()
            && model.spheres.is_empty()
            && model.nulls.is_empty()
            && model.collections.is_empty()
            && model.aggregates.is_empty()
            && model.dist_lods.is_empty()
            && !allow_animation_only
        {
            return Err(anyhow!(
                "legacy parser: no valid meshes found in '{}'",
                model.name
            ));
        }

        // Post-process: Resolve texture indices to actual texture names from W3D_CHUNK_TEXTURES
        // This matches C++ behavior where W3D_CHUNK_MAP3_FILENAME contains texture indices
        // that need to be resolved against the texture_names array
        self.resolve_texture_indices(&mut model);

        // HMODEL has separate source binding metadata that is not part of this HLOD
        // correction.  Preserve its existing narrow pivot-name residual, but apply
        // the resulting matrix only at render time.  In particular, never bake it
        // into already converted vertices and then retain it for a second render
        // transform.  HLOD uses its own exact `HLOD.Name.MeshName -> BoneIndex`
        // records below and must never take this generic name path.
        if model.hlods.is_empty() && !model.hlod_parse_failed {
            if let Some(ref hierarchy) = model.hierarchy {
                if !hierarchy.pivots.is_empty() {
                    if let Some(globals) = Self::compute_global_transforms(hierarchy) {
                        for mesh in &mut model.meshes {
                            if mesh.transform != Mat4::IDENTITY {
                                continue;
                            }
                            if let Some(pivot_idx) =
                                hierarchy.pivots.iter().position(|p| p.name == mesh.name)
                            {
                                let global = &globals[pivot_idx];
                                mesh.transform = W3DModel::w3d_transform_to_render_basis(
                                    Mat4::from_cols_array(global),
                                );
                            }
                        }
                    }
                }
            }
        }

        model.calculate_bounding_box();
        Ok(model)
    }

    /// Parse a W3D container chunk recursively
    pub(super) fn parse_container_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut chunk_counter = 0;
        pub(super) const MAX_CONTAINER_CHUNKS: usize = 5000; // Safety limit for container chunks

        while offset + 8 <= data.len() && chunk_counter < MAX_CONTAINER_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Container sub-chunk extends beyond container: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for container chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized container chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Found mesh chunk in container, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse mesh chunk in container");
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Found textures chunk in container, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_textures_chunk(chunk_data, model) {
                            warn!("Failed to parse textures chunk: {}", e);
                        }
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Found animation chunk in container, size: {}", chunk_size);
                    match self.parse_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse animation chunk in container: {}", e),
                    }
                }
                W3D_CHUNK_COMPRESSED_ANIMATION => {
                    debug!(
                        "Found compressed animation chunk in container, size: {}",
                        chunk_size
                    );
                    match self.parse_compressed_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            model.animations.push(animation);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse compressed animation chunk in container: {}",
                                e
                            )
                        }
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!("Found hierarchy chunk in container, size: {}", chunk_size);
                    match self.parse_hierarchy_chunk(chunk_data) {
                        Ok(hierarchy) => {
                            model.retain_source_hierarchy(hierarchy);
                        }
                        Err(e) => {
                            warn!("Failed to parse hierarchy chunk in container: {}", e)
                        }
                    }
                }
                W3D_CHUNK_HMODEL => {
                    if !is_container_chunk {
                        warn!("Nested HMODEL chunk is not a container; skipping unsafe prototype");
                    } else {
                        match self.parse_hmodel_chunk(chunk_data) {
                            Ok(hmodel) => model.hmodels.push(hmodel),
                            Err(error) => {
                                warn!("Failed to parse nested HMODEL definition: {}", error)
                            }
                        }
                    }
                }
                W3D_CHUNK_EMITTER => {
                    if let Some(emitter) =
                        super::w3d_emitter_loader::parse_emitter_chunk(chunk_data)
                    {
                        model.emitters.push(emitter);
                    }
                }
                W3D_CHUNK_DAZZLE => {
                    if let Some(dazzle) = super::w3d_dazzle_loader::parse_dazzle_chunk(chunk_data) {
                        model.dazzles.push(dazzle);
                    }
                }
                W3D_CHUNK_BOX => {
                    if let Some(box_proto) =
                        super::w3d_primitive_protos::parse_box_chunk(chunk_data)
                    {
                        model.boxes.push(box_proto);
                    }
                }
                W3D_CHUNK_RING => {
                    if let Some(ring) = super::w3d_primitive_protos::parse_ring_chunk(chunk_data) {
                        model.rings.push(ring);
                    }
                }
                W3D_CHUNK_SPHERE => {
                    if let Some(sphere) =
                        super::w3d_primitive_protos::parse_sphere_chunk(chunk_data)
                    {
                        model.spheres.push(sphere);
                    }
                }
                W3D_CHUNK_NULL_OBJECT => {
                    if let Some(null) = super::w3d_primitive_protos::parse_null_chunk(chunk_data) {
                        model.nulls.push(null);
                    }
                }
                W3D_CHUNK_COLLECTION => {
                    if let Some(collection) =
                        super::w3d_collection_aggregate::parse_collection_chunk(chunk_data)
                    {
                        model.collections.push(collection);
                    }
                }
                W3D_CHUNK_AGGREGATE => {
                    if let Some(aggregate) =
                        super::w3d_collection_aggregate::parse_aggregate_chunk(chunk_data)
                    {
                        model.aggregates.push(aggregate);
                    }
                }
                W3D_CHUNK_LODMODEL => {
                    if let Some(dist_lod) =
                        super::w3d_collection_aggregate::parse_dist_lod_chunk(chunk_data)
                    {
                        model.dist_lods.push(dist_lod);
                    }
                    if is_container_chunk && chunk_size > 0 {
                        if let Err(e) = self.parse_container_chunk(chunk_data, model) {
                            warn!("Failed to parse nested LODMODEL container: {}", e);
                        }
                    }
                }
                _ => {
                    debug!(
                        "Container sub-chunk: type 0x{:08X}, size {}, container: {}",
                        chunk_type, chunk_size, is_container_chunk
                    );
                    // Recursively parse nested containers
                    if is_container_chunk && chunk_size > 0 {
                        if let Err(e) = self.parse_container_chunk(chunk_data, model) {
                            warn!(
                                "Failed to parse nested container 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CONTAINER_CHUNKS {
            warn!(
                "⚠️  Container chunk parsing hit safety limit ({} chunks)",
                MAX_CONTAINER_CHUNKS
            );
        }

        Ok(())
    }

    /// Parse one C++ `HModelDefClass` source record.
    ///
    /// `HModelDefClass::Load_W3D` requires a header first, allocates exactly
    /// `NumConnections` node records, and reads NODE, COLLISION_NODE, and
    /// SKIN_NODE in source order. Under the original 32-bit MSVC ABI,
    /// `sizeof(W3dHModelHeaderStruct)` is 40 (four-byte tail alignment) while
    /// `sizeof(W3dHModelNodeStruct)` is exactly 18 (its largest member has
    /// two-byte alignment). Preserve that asymmetrical wire shape instead of
    /// accepting the unrelated u32 helper structs from other compatibility
    /// crates.
    pub(super) fn parse_hmodel_chunk(&self, data: &[u8]) -> Result<W3dHmodel> {
        pub(super) const HMODEL_HEADER_SIZE: usize = 40;
        pub(super) const HMODEL_NODE_SIZE: usize = 18;
        pub(super) const MAX_HMODEL_CONNECTIONS: usize = 4096;

        let mut offset = 0usize;
        let header = next_w3d_chunk(data, &mut offset, "HMODEL header")?
            .ok_or_else(|| anyhow!("HMODEL has no header"))?;
        if header.chunk_type != W3D_CHUNK_HMODEL_HEADER || header.is_container {
            return Err(anyhow!(
                "HMODEL expected raw header 0x{W3D_CHUNK_HMODEL_HEADER:08X}, got 0x{:08X}, container={}",
                header.chunk_type,
                header.is_container
            ));
        }
        if header.data.len() != HMODEL_HEADER_SIZE {
            return Err(anyhow!(
                "HMODEL header has wrong size: {} != {}",
                header.data.len(),
                HMODEL_HEADER_SIZE
            ));
        }

        let version = u32::from_le_bytes(header.data[0..4].try_into().unwrap());
        let name = hmodel_cxx_header_name(&header.data[4..4 + W3D_NAME_LEN]);
        let hierarchy_name = hmodel_cxx_header_name(&header.data[4 + W3D_NAME_LEN..36]);
        let declared_connection_count =
            usize::from(u16::from_le_bytes(header.data[36..38].try_into().unwrap()));
        if declared_connection_count > MAX_HMODEL_CONNECTIONS {
            return Err(anyhow!(
                "HMODEL '{}' declares {} connections, exceeding safe limit {}",
                name,
                declared_connection_count,
                MAX_HMODEL_CONNECTIONS
            ));
        }

        let mut nodes = Vec::with_capacity(declared_connection_count);
        let mut source_snap_points = Vec::new();
        let mut has_invalid_records = name.is_empty()
            || name.as_bytes().contains(&0)
            || hierarchy_name.as_bytes().contains(&0);

        while let Some(record) = next_w3d_chunk(data, &mut offset, "HMODEL connection")? {
            if record.chunk_type == W3D_CHUNK_POINTS {
                // `SnapPointsClass::Load_W3D` uses `Cur_Chunk_Length() / 12`
                // and reads that many raw W3dVectorStruct values. It does
                // not consume a declared HMODEL connection, rejects no
                // trailing byte remainder, and a later POINTS record replaces
                // the definition-owned pointer. Keep all three properties.
                source_snap_points = Self::parse_hmodel_source_snap_points(record.data);
                continue;
            }
            let kind = match record.chunk_type {
                W3D_CHUNK_HMODEL_NODE => W3dHmodelNodeKind::Node,
                W3D_CHUNK_HMODEL_COLLISION_NODE => W3dHmodelNodeKind::CollisionNode,
                W3D_CHUNK_HMODEL_SKIN_NODE => W3dHmodelNodeKind::SkinNode,
                // Obsolete extension chunks do not define a child render
                // object and must never become aliases.
                _ => continue,
            };
            if record.is_container || record.data.len() != HMODEL_NODE_SIZE {
                has_invalid_records = true;
                continue;
            }
            if nodes.len() >= declared_connection_count {
                has_invalid_records = true;
                continue;
            }

            let Some(render_object_name_end) = record.data[..W3D_NAME_LEN]
                .iter()
                .position(|byte| *byte == 0)
            else {
                // `read_connection` uses `strcat` on this source field. A
                // non-terminated sixteen-byte leaf is undefined behavior in
                // C++; safe Main must not manufacture a wider alias from it.
                has_invalid_records = true;
                continue;
            };
            let render_object_name = w3d_string_from_bytes(&record.data[..render_object_name_end]);
            let bone_index = u32::from(u16::from_le_bytes(
                record.data[W3D_NAME_LEN..W3D_NAME_LEN + 2]
                    .try_into()
                    .unwrap(),
            ));
            if render_object_name.is_empty() || render_object_name.as_bytes().contains(&0) {
                has_invalid_records = true;
                continue;
            }

            // C++ `read_connection` always constructs the complete identity
            // from the HMODEL header name and the node leaf, even for a
            // collision or skin connection.
            nodes.push(W3dHmodelNode {
                name: format!("{}.{}", name, render_object_name),
                bone_index: if version < W3D_HTREE_ROOT_VERSION {
                    // Pre-3.0 source uses 0xffff for the newly synthesized
                    // external root; every other source pivot shifts by one.
                    if bone_index == u32::from(u16::MAX) {
                        0
                    } else {
                        bone_index.checked_add(1).ok_or_else(|| {
                            anyhow!("HMODEL '{}' pre-3.0 pivot index overflow", name)
                        })?
                    }
                } else {
                    if bone_index == u32::from(u16::MAX) {
                        has_invalid_records = true;
                    }
                    bone_index
                },
                kind,
            });
        }

        if nodes.len() != declared_connection_count {
            has_invalid_records = true;
        }

        Ok(W3dHmodel {
            version,
            name,
            hierarchy_name,
            nodes,
            source_snap_points,
            has_invalid_records,
        })
    }

    /// Read the source payload exactly as C++ `SnapPointsClass::Load_W3D`.
    /// A non-vector remainder is skipped by the C++ integer division, and
    /// points remain in source W3D coordinates rather than the active render
    /// basis.
    pub(super) fn parse_hmodel_source_snap_points(data: &[u8]) -> Vec<W3dHmodelSnapPoint> {
        pub(super) const W3D_VECTOR_SIZE: usize = 12;

        data.chunks_exact(W3D_VECTOR_SIZE)
            .map(|record| W3dHmodelSnapPoint {
                source_position: [
                    f32::from_le_bytes(record[0..4].try_into().unwrap()),
                    f32::from_le_bytes(record[4..8].try_into().unwrap()),
                    f32::from_le_bytes(record[8..12].try_into().unwrap()),
                ],
            })
            .collect()
    }

    /// Parse one C++ `W3dHLodArrayHeaderStruct` plus its exact ordered
    /// `W3dHLodSubObjectStruct` records. The same wire shape is used for an
    /// ordinary LOD, aggregate render objects, and application proxies.
    pub(super) fn parse_hlod_subobject_array(
        &self,
        data: &[u8],
        hlod_name: &str,
        array_label: &str,
    ) -> Result<W3dHlodAttachmentArray> {
        pub(super) const HLOD_ARRAY_HEADER_SIZE: usize = 8;
        pub(super) const HLOD_SUBOBJECT_SIZE: usize = 36;
        pub(super) const MAX_HLOD_SUBOBJECTS: usize = 4096;

        let mut offset = 0usize;
        let array_header = next_w3d_chunk(data, &mut offset, "HLOD subobject array header")?
            .ok_or_else(|| {
                anyhow!(
                    "HLOD '{}' {} has no subobject array header",
                    hlod_name,
                    array_label
                )
            })?;
        if array_header.chunk_type != W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER
            || array_header.is_container
        {
            return Err(anyhow!(
                "HLOD '{}' {} expected raw subobject array header 0x{W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER:08X}, got 0x{:08X}, container={}",
                hlod_name,
                array_label,
                array_header.chunk_type,
                array_header.is_container
            ));
        }
        if array_header.data.len() != HLOD_ARRAY_HEADER_SIZE {
            return Err(anyhow!(
                "HLOD '{}' {} array header has wrong size: {} != {}",
                hlod_name,
                array_label,
                array_header.data.len(),
                HLOD_ARRAY_HEADER_SIZE
            ));
        }

        let model_count = u32::from_le_bytes(array_header.data[0..4].try_into().unwrap()) as usize;
        if model_count > MAX_HLOD_SUBOBJECTS {
            return Err(anyhow!(
                "HLOD '{}' {} declares {} subobjects, exceeding safe limit {}",
                hlod_name,
                array_label,
                model_count,
                MAX_HLOD_SUBOBJECTS
            ));
        }
        let max_screen_size = f32::from_le_bytes(array_header.data[4..8].try_into().unwrap());

        let mut subobjects = Vec::with_capacity(model_count);
        for subobject_index in 0..model_count {
            let subobject =
                next_w3d_chunk(data, &mut offset, "HLOD subobject")?.ok_or_else(|| {
                    anyhow!(
                        "HLOD '{}' {} is missing subobject {}",
                        hlod_name,
                        array_label,
                        subobject_index
                    )
                })?;
            if subobject.chunk_type != W3D_CHUNK_HLOD_SUB_OBJECT || subobject.is_container {
                return Err(anyhow!(
                    "HLOD '{}' {} expected raw subobject 0x{W3D_CHUNK_HLOD_SUB_OBJECT:08X}, got 0x{:08X}, container={}",
                    hlod_name,
                    array_label,
                    subobject.chunk_type,
                    subobject.is_container
                ));
            }
            if subobject.data.len() != HLOD_SUBOBJECT_SIZE {
                return Err(anyhow!(
                    "HLOD '{}' {} subobject {} has wrong size: {} != {}",
                    hlod_name,
                    array_label,
                    subobject_index,
                    subobject.data.len(),
                    HLOD_SUBOBJECT_SIZE
                ));
            }
            subobjects.push(W3dHlodSubObject {
                bone_index: u32::from_le_bytes(subobject.data[0..4].try_into().unwrap()),
                name: w3d_string_from_bytes(&subobject.data[4..HLOD_SUBOBJECT_SIZE]),
            });
        }
        if next_w3d_chunk(data, &mut offset, "HLOD subobject array trailing data")?.is_some() {
            return Err(anyhow!(
                "HLOD '{}' {} has trailing chunks after its declared subobjects",
                hlod_name,
                array_label
            ));
        }

        Ok(W3dHlodAttachmentArray {
            max_screen_size,
            subobjects,
        })
    }

    /// Parse the exact rigid-child metadata from a `W3D_CHUNK_HLOD` container.
    ///
    /// C++ `HLodDefClass::Load_W3D` reads the HLOD header, then exactly
    /// `LodCount` `W3D_CHUNK_HLOD_LOD_ARRAY` records.  Each array contains an
    /// array header followed by exact `W3dHLodSubObjectStruct` records.  Keep that
    /// structure instead of recursively flattening it into anonymous meshes.
    pub(super) fn parse_hlod_chunk(&self, data: &[u8]) -> Result<W3dHlod> {
        pub(super) const HLOD_HEADER_SIZE: usize = 40;
        pub(super) const MAX_HLOD_LODS: usize = 64;

        let mut offset = 0usize;
        let header = next_w3d_chunk(data, &mut offset, "HLOD header")?
            .ok_or_else(|| anyhow!("HLOD has no header"))?;
        if header.chunk_type != W3D_CHUNK_HLOD_HEADER {
            return Err(anyhow!(
                "HLOD expected header 0x{W3D_CHUNK_HLOD_HEADER:08X}, got 0x{:08X}",
                header.chunk_type
            ));
        }
        if header.data.len() < HLOD_HEADER_SIZE {
            return Err(anyhow!(
                "HLOD header too small: {} < {}",
                header.data.len(),
                HLOD_HEADER_SIZE
            ));
        }

        let version = u32::from_le_bytes(header.data[0..4].try_into().unwrap());
        let lod_count = u32::from_le_bytes(header.data[4..8].try_into().unwrap()) as usize;
        if lod_count > MAX_HLOD_LODS {
            return Err(anyhow!(
                "HLOD declares {} LODs, exceeding safe limit {}",
                lod_count,
                MAX_HLOD_LODS
            ));
        }
        let name = w3d_string_from_bytes(&header.data[8..8 + W3D_NAME_LEN]);
        let hierarchy_name =
            w3d_string_from_bytes(&header.data[8 + W3D_NAME_LEN..HLOD_HEADER_SIZE]);

        let mut lods = Vec::with_capacity(lod_count);
        for lod_index in 0..lod_count {
            let lod_chunk = next_w3d_chunk(data, &mut offset, "HLOD LOD array")?
                .ok_or_else(|| anyhow!("HLOD '{}' is missing LOD {}", name, lod_index))?;
            if lod_chunk.chunk_type != W3D_CHUNK_HLOD_LOD_ARRAY || !lod_chunk.is_container {
                return Err(anyhow!(
                    "HLOD '{}' expected container LOD array 0x{W3D_CHUNK_HLOD_LOD_ARRAY:08X}, got type 0x{:08X}, container={}",
                    name,
                    lod_chunk.chunk_type,
                    lod_chunk.is_container
                ));
            }

            let array = self.parse_hlod_subobject_array(
                lod_chunk.data,
                &name,
                &format!("LOD {lod_index}"),
            )?;

            lods.push(W3dHlodLod {
                max_screen_size: array.max_screen_size,
                subobjects: array.subobjects,
            });
        }

        // Retain the two C++ trailing arrays even before Main can recursively
        // render aggregates. Only aggregates affect model output; proxies are
        // intentionally non-rendering application metadata in C++.
        let mut aggregates = None;
        let mut proxies = None;
        let mut has_invalid_trailing_records = false;
        while let Some(remaining) = next_w3d_chunk(data, &mut offset, "HLOD trailing record")? {
            match remaining.chunk_type {
                W3D_CHUNK_HLOD_AGGREGATE_ARRAY => {
                    if !remaining.is_container || aggregates.is_some() {
                        has_invalid_trailing_records = true;
                        warn!(
                            "HLOD '{}' has a non-container or repeated aggregate array; suppressing ambiguous render",
                            name
                        );
                        continue;
                    }
                    match self.parse_hlod_subobject_array(remaining.data, &name, "aggregate array")
                    {
                        Ok(array) => aggregates = Some(array),
                        Err(error) => {
                            has_invalid_trailing_records = true;
                            warn!(
                                "HLOD '{}' has malformed aggregate data; suppressing ambiguous render: {}",
                                name, error
                            );
                        }
                    }
                }
                W3D_CHUNK_HLOD_PROXY_ARRAY => {
                    if !remaining.is_container || proxies.is_some() {
                        has_invalid_trailing_records = true;
                        warn!(
                            "HLOD '{}' has a non-container or repeated proxy array; suppressing ambiguous render",
                            name
                        );
                        continue;
                    }
                    match self.parse_hlod_subobject_array(remaining.data, &name, "proxy array") {
                        Ok(array) => proxies = Some(array),
                        Err(error) => {
                            has_invalid_trailing_records = true;
                            warn!(
                                "HLOD '{}' has malformed proxy data; suppressing ambiguous render: {}",
                                name, error
                            );
                        }
                    }
                }
                unknown => {
                    has_invalid_trailing_records = true;
                    warn!(
                        "HLOD '{}' has unsupported trailing chunk 0x{:08X}; suppressing ambiguous render",
                        name, unknown
                    );
                }
            }
        }

        let has_unrendered_aggregates = aggregates
            .as_ref()
            .is_some_and(|array| !array.subobjects.is_empty());

        Ok(W3dHlod {
            version,
            name,
            hierarchy_name,
            lods,
            aggregates,
            proxies,
            has_unrendered_aggregates,
            has_invalid_trailing_records,
        })
    }

    /// Parse W3D hierarchy chunk (0x100) — ported from standalone parser's parse_hierarchy_chunk
    pub(super) fn parse_hierarchy_chunk(&self, data: &[u8]) -> Result<W3dHierarchy> {
        let mut name = String::new();
        let mut hierarchy_version = None;
        let mut num_pivots: u32 = 0;
        let mut pivots: Vec<W3dPivot> = Vec::new();
        let mut pivot_fixups: Vec<[[f32; 3]; 4]> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_HIERARCHY_HEADER => {
                    // W3dHierarchyHeader: version(u32) + name[16] + num_pivots(u32) + center[3]
                    if chunk_data.len() < 32 {
                        return Err(anyhow!("hierarchy header too small: {}", chunk_data.len()));
                    }
                    let version = u32::from_le_bytes([
                        chunk_data[0],
                        chunk_data[1],
                        chunk_data[2],
                        chunk_data[3],
                    ]);
                    hierarchy_version = Some(version);
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    num_pivots = u32::from_le_bytes([
                        chunk_data[20],
                        chunk_data[21],
                        chunk_data[22],
                        chunk_data[23],
                    ]);
                    debug!(
                        "Hierarchy header: name='{}', version=0x{:08X}, num_pivots={}",
                        name, version, num_pivots
                    );
                }
                W3D_CHUNK_PIVOTS => {
                    // Each pivot: name[16] + parent_idx(u32) + translation[3] + euler[3] + quat[4]
                    // = 16 + 4 + 12 + 12 + 16 = 60 bytes
                    pub(super) const PIVOT_SIZE: usize = 60;
                    let count = if num_pivots > 0 {
                        num_pivots as usize
                    } else {
                        chunk_size / PIVOT_SIZE
                    };
                    pivots.reserve(count);
                    for i in 0..count {
                        let base = i * PIVOT_SIZE;
                        if base + PIVOT_SIZE > chunk_data.len() {
                            break;
                        }
                        let d = &chunk_data[base..base + PIVOT_SIZE];
                        let mut pname = String::new();
                        for j in 0..W3D_NAME_LEN {
                            if d[j] == 0 {
                                break;
                            }
                            pname.push(d[j] as char);
                        }
                        let parent_idx = u32::from_le_bytes([d[16], d[17], d[18], d[19]]);
                        let tx = f32::from_le_bytes([d[20], d[21], d[22], d[23]]);
                        let ty = f32::from_le_bytes([d[24], d[25], d[26], d[27]]);
                        let tz = f32::from_le_bytes([d[28], d[29], d[30], d[31]]);
                        let ex = f32::from_le_bytes([d[32], d[33], d[34], d[35]]);
                        let ey = f32::from_le_bytes([d[36], d[37], d[38], d[39]]);
                        let ez = f32::from_le_bytes([d[40], d[41], d[42], d[43]]);
                        let qx = f32::from_le_bytes([d[44], d[45], d[46], d[47]]);
                        let qy = f32::from_le_bytes([d[48], d[49], d[50], d[51]]);
                        let qz = f32::from_le_bytes([d[52], d[53], d[54], d[55]]);
                        let qw = f32::from_le_bytes([d[56], d[57], d[58], d[59]]);
                        pivots.push(W3dPivot {
                            name: pname,
                            parent_idx,
                            translation: [tx, ty, tz],
                            euler_angles: [ex, ey, ez],
                            rotation: [qx, qy, qz, qw],
                        });
                    }
                }
                W3D_CHUNK_PIVOT_FIXUPS => {
                    // Each fixup: [[f32;3];4] = 48 bytes
                    pub(super) const FIXUP_SIZE: usize = 48;
                    let count = if num_pivots > 0 {
                        num_pivots as usize
                    } else {
                        chunk_size / FIXUP_SIZE
                    };
                    pivot_fixups.reserve(count);
                    for i in 0..count {
                        let base = i * FIXUP_SIZE;
                        if base + FIXUP_SIZE > chunk_data.len() {
                            break;
                        }
                        let d = &chunk_data[base..base + FIXUP_SIZE];
                        let mut tm = [[0.0f32; 3]; 4];
                        for row in 0..4 {
                            for col in 0..3 {
                                let off = (row * 3 + col) * 4;
                                tm[row][col] = f32::from_le_bytes([
                                    d[off],
                                    d[off + 1],
                                    d[off + 2],
                                    d[off + 3],
                                ]);
                            }
                        }
                        pivot_fixups.push(tm);
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        if hierarchy_version.is_some_and(|version| version < W3D_HTREE_ROOT_VERSION) {
            // C++ `HTreeClass::read_pivots(pre30)` injects this exact node,
            // then increments every source parent index (including
            // `0xFFFF_FFFF`, which wraps to the synthetic root).  Preserve
            // that normalized index space so the shared runtime evaluator can
            // always reserve pivot zero for the external object transform.
            for pivot in &mut pivots {
                pivot.parent_idx = pivot.parent_idx.wrapping_add(1);
            }
            pivots.insert(
                0,
                W3dPivot {
                    name: "RootTransform".to_string(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            );
            if !pivot_fixups.is_empty() {
                // The Main loader retains fixups as source metadata. Keep the
                // same pivot indexing even though no active runtime consumer
                // currently evaluates them.
                pivot_fixups.insert(
                    0,
                    [
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                        [0.0, 0.0, 0.0],
                    ],
                );
            }
        }

        Ok(W3dHierarchy {
            name,
            pivots,
            pivot_fixups,
        })
    }

    /// Parse W3D animation chunk (0x200) — ported from standalone parser's parse_animation_chunk
    pub(super) fn parse_animation_chunk(&self, data: &[u8]) -> Result<W3dAnimation> {
        let mut name = String::new();
        let mut hierarchy_name = String::new();
        let mut animation_version = None;
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut channels: Vec<W3dAnimChannel> = Vec::new();
        let mut raw_visibility_channels: Vec<W3dRawVisibilityChannel> = Vec::new();
        let mut unsupported_visibility_pivots: Vec<Option<u16>> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_ANIMATION_HEADER => {
                    // version(u32) + name[16] + hierarchy_name[16] + num_frames(u32) + frame_rate(u32)
                    // = 4 + 16 + 16 + 4 + 4 = 44 bytes
                    if chunk_data.len() < 44 {
                        return Err(anyhow!("animation header too small: {}", chunk_data.len()));
                    }
                    let version = u32::from_le_bytes([
                        chunk_data[0],
                        chunk_data[1],
                        chunk_data[2],
                        chunk_data[3],
                    ]);
                    animation_version = Some(version);
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    let mut hn = String::new();
                    for i in 20..20 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        hn.push(chunk_data[i] as char);
                    }
                    hierarchy_name = hn;
                    num_frames = u32::from_le_bytes([
                        chunk_data[36],
                        chunk_data[37],
                        chunk_data[38],
                        chunk_data[39],
                    ]);
                    frame_rate = u32::from_le_bytes([
                        chunk_data[40],
                        chunk_data[41],
                        chunk_data[42],
                        chunk_data[43],
                    ]);
                    debug!(
                        "Animation header: name='{}', hierarchy='{}', version=0x{:08X}, frames={}, fps={}",
                        name, hierarchy_name, version, num_frames, frame_rate
                    );
                }
                W3D_CHUNK_ANIMATION_CHANNEL => {
                    // first_frame(u16) + last_frame(u16) + vector_len(u16) + flags(u16) + pivot(u16) + pad(u16)
                    // = 12 bytes header, rest is f32 data
                    if chunk_data.len() < 12 {
                        offset += 8 + chunk_size;
                        continue;
                    }
                    let first_frame = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
                    let last_frame = u16::from_le_bytes([chunk_data[2], chunk_data[3]]);
                    let vector_len = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                    let flags = u16::from_le_bytes([chunk_data[6], chunk_data[7]]);
                    let pivot = u16::from_le_bytes([chunk_data[8], chunk_data[9]]);
                    let remaining = chunk_size.saturating_sub(12);
                    let count_f32 = remaining / 4;
                    let mut chan_data = Vec::with_capacity(count_f32);
                    for i in 0..count_f32 {
                        let off = 12 + i * 4;
                        if off + 4 > chunk_data.len() {
                            break;
                        }
                        chan_data.push(f32::from_le_bytes([
                            chunk_data[off],
                            chunk_data[off + 1],
                            chunk_data[off + 2],
                            chunk_data[off + 3],
                        ]));
                    }
                    channels.push(W3dAnimChannel {
                        first_frame,
                        last_frame,
                        vector_len,
                        flags,
                        pivot,
                        data: chan_data,
                    });
                }
                W3D_CHUNK_BIT_CHANNEL => {
                    // W3dBitChannelStruct has nine fixed bytes before the
                    // packed bit stream: first/last/flags/pivot/default. C++
                    // accepts this channel only for BIT_CHANNEL_VIS (0).
                    if chunk_data.len() < 9 {
                        unsupported_visibility_pivots.push(None);
                        debug!(
                            "Malformed raw W3D bit channel has no recoverable pivot, size: {}",
                            chunk_size
                        );
                        offset += 8 + chunk_size;
                        continue;
                    }
                    let first_frame = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
                    let last_frame = u16::from_le_bytes([chunk_data[2], chunk_data[3]]);
                    let flags = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                    let pivot = u16::from_le_bytes([chunk_data[6], chunk_data[7]]);
                    if flags != 0 {
                        // `HRawAnimClass::add_bit_channel` ignores types other
                        // than BIT_CHANNEL_VIS, so they are not an HLOD
                        // visibility fallback to guess at.
                        debug!(
                            "Ignoring non-visibility raw W3D bit channel type {} for pivot {}",
                            flags, pivot
                        );
                    } else if last_frame < first_frame {
                        unsupported_visibility_pivots.push(Some(pivot));
                        debug!(
                            "Malformed raw W3D visibility channel has inverted range {}..{} for pivot {}",
                            first_frame, last_frame, pivot
                        );
                    } else {
                        let bit_count = usize::from(last_frame - first_frame) + 1;
                        let byte_count = (bit_count + 7) / 8;
                        let expected_len = 9 + byte_count;
                        if chunk_data.len() != expected_len {
                            unsupported_visibility_pivots.push(Some(pivot));
                            debug!(
                                "Malformed raw W3D visibility channel for pivot {}: expected {} bytes, got {}",
                                pivot,
                                expected_len,
                                chunk_data.len()
                            );
                        } else {
                            raw_visibility_channels.push(W3dRawVisibilityChannel {
                                first_frame,
                                last_frame,
                                flags,
                                pivot,
                                default_visible: chunk_data[8] != 0,
                                bits: chunk_data[9..].to_vec(),
                            });
                        }
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        if animation_version.is_some_and(|version| version < W3D_HTREE_ROOT_VERSION) {
            // C++ `HRawAnimClass::{read_channel,read_bit_channel}` shifts
            // every pre-3.0 source pivot after `HTreeClass` inserts its
            // synthetic root.  `u16` wrapping matches the raw C++ field
            // operation; a resulting out-of-range channel is safely ignored
            // later by the same bounded pose sampler.
            for channel in &mut channels {
                channel.pivot = channel.pivot.wrapping_add(1);
            }
            for channel in &mut raw_visibility_channels {
                channel.pivot = channel.pivot.wrapping_add(1);
            }
            for pivot in &mut unsupported_visibility_pivots {
                if let Some(pivot) = pivot {
                    *pivot = pivot.wrapping_add(1);
                }
            }
        }

        Ok(W3dAnimation {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            source_is_compressed: false,
            channels,
            raw_visibility_channels,
            unsupported_visibility_pivots,
        })
    }

    /// Parse W3D compressed animation chunk (0x280) — handles both timecoded (flavor 0)
    /// and adaptive delta (flavor 1) sub-formats.
    /// C++ parity: W3D_CHUNK_COMPRESSED_ANIMATION container with CompressedAnimationHeader,
    /// CompressedAnimationChannel, and CompressedBitChannel sub-chunks.
    pub(super) fn parse_compressed_animation_chunk(&self, data: &[u8]) -> Result<W3dAnimation> {
        let mut name = String::new();
        let mut hierarchy_name = String::new();
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut flavor: u16 = ANIM_FLAVOR_TIMECODED;
        let mut channels: Vec<W3dAnimChannel> = Vec::new();
        let raw_visibility_channels: Vec<W3dRawVisibilityChannel> = Vec::new();
        let mut unsupported_visibility_pivots: Vec<Option<u16>> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_COMPRESSED_ANIMATION_HEADER => {
                    if chunk_data.len() < 44 {
                        return Err(anyhow!(
                            "compressed animation header too small: {}",
                            chunk_data.len()
                        ));
                    }
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    let mut hn = String::new();
                    for i in 20..20 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        hn.push(chunk_data[i] as char);
                    }
                    hierarchy_name = hn;
                    num_frames = u32::from_le_bytes([
                        chunk_data[36],
                        chunk_data[37],
                        chunk_data[38],
                        chunk_data[39],
                    ]);
                    frame_rate = u16::from_le_bytes([chunk_data[40], chunk_data[41]]) as u32;
                    flavor = u16::from_le_bytes([chunk_data[42], chunk_data[43]]);
                    debug!(
                        "Compressed animation header: name='{}', hierarchy='{}', frames={}, fps={}, flavor={}",
                        name, hierarchy_name, num_frames, frame_rate, flavor
                    );
                }
                W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL => {
                    if chunk_data.len() < 8 {
                        offset += 8 + chunk_size;
                        continue;
                    }
                    if flavor == ANIM_FLAVOR_TIMECODED {
                        if let Some(ch) = Self::parse_timecoded_channel(chunk_data, num_frames) {
                            channels.push(ch);
                        }
                    } else {
                        let chs = Self::parse_adaptive_delta_channel(chunk_data, num_frames);
                        channels.extend(chs);
                    }
                }
                W3D_CHUNK_COMPRESSED_BIT_CHANNEL => {
                    // W3dTimeCodedBitChannelStruct starts with
                    // NumTimeCodes(u32), Pivot(u16), Flags(u8), Default(u8).
                    // The C++ HCompressedAnim class installs flags==0 as an
                    // HTree visibility source. Keep its pivot and make that
                    // child non-rendering until time-coded sampling is ported.
                    if chunk_data.len() < 8 {
                        unsupported_visibility_pivots.push(None);
                        debug!(
                            "Malformed compressed W3D bit channel has no recoverable pivot, size: {}",
                            chunk_size
                        );
                    } else {
                        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                        let flags = chunk_data[6];
                        if flags == 0 {
                            unsupported_visibility_pivots.push(Some(pivot));
                            debug!(
                                "Compressed W3D visibility channel for pivot {} retained as unsupported",
                                pivot
                            );
                        }
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(W3dAnimation {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            source_is_compressed: true,
            channels,
            raw_visibility_channels,
            unsupported_visibility_pivots,
        })
    }

    /// Parse a single timecoded animation channel (ANIM_FLAVOR_TIMECODED, flavor=0).
    /// Format: num_timecodes(u32) + pivot(u16) + vector_len(u8) + flags(u8)
    ///         then [timecode(u32) + vector_len * f32] per timecode.
    /// Timecode MSB = binary (step) flag; lower 31 bits = frame index.
    /// Produces a densified per-frame W3dAnimChannel matching the uncompressed layout.
    pub(super) fn parse_timecoded_channel(
        chunk_data: &[u8],
        num_frames: u32,
    ) -> Option<W3dAnimChannel> {
        if chunk_data.len() < 8 {
            return None;
        }
        let num_timecodes =
            u32::from_le_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]])
                as usize;
        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
        let vector_len = chunk_data[6] as u16;
        let flags_tc = chunk_data[7] as u16;

        if vector_len == 0 {
            return None;
        }

        let vl = vector_len as usize;
        let entry_size = 4 + vl * 4;
        if chunk_data.len() < 8 + num_timecodes * entry_size {
            return None;
        }

        let mut timecodes: Vec<(u32, bool, Vec<f32>)> = Vec::with_capacity(num_timecodes);
        for i in 0..num_timecodes {
            let base = 8 + i * entry_size;
            let tc = u32::from_le_bytes([
                chunk_data[base],
                chunk_data[base + 1],
                chunk_data[base + 2],
                chunk_data[base + 3],
            ]);
            let binary = (tc & 0x8000_0000) != 0;
            let frame = tc & 0x7FFF_FFFF;
            let mut vals = Vec::with_capacity(vl);
            for c in 0..vl {
                let off = base + 4 + c * 4;
                vals.push(f32::from_le_bytes([
                    chunk_data[off],
                    chunk_data[off + 1],
                    chunk_data[off + 2],
                    chunk_data[off + 3],
                ]));
            }
            timecodes.push((frame, binary, vals));
        }

        let total_frames = num_frames as usize;
        if total_frames == 0 {
            return Some(W3dAnimChannel {
                first_frame: 0,
                last_frame: 0,
                vector_len,
                flags: Self::map_timecoded_flag(flags_tc),
                pivot,
                data: Vec::new(),
            });
        }

        let mut data = vec![0.0f32; total_frames * vl];

        if timecodes.is_empty() {
            return Some(W3dAnimChannel {
                first_frame: 0,
                last_frame: (num_frames.max(1) - 1) as u16,
                vector_len,
                flags: Self::map_timecoded_flag(flags_tc),
                pivot,
                data,
            });
        }

        timecodes.sort_by_key(|t| t.0);

        let (first_tc, _, ref v0) = timecodes[0];
        for f in 0..(first_tc.min(num_frames)) as usize {
            let base = f * vl;
            for c in 0..vl {
                data[base + c] = v0[c];
            }
        }

        for seg in 0..timecodes.len() {
            let (f0, bin0, ref v0_vals) = timecodes[seg];
            let f1 = if seg + 1 < timecodes.len() {
                timecodes[seg + 1].0
            } else {
                num_frames - 1
            };
            let v1_vals = if seg + 1 < timecodes.len() {
                &timecodes[seg + 1].2
            } else {
                v0_vals
            };
            let start_f = f0 as usize;
            let end_f = f1 as usize;
            if start_f > end_f || start_f as u32 >= num_frames {
                continue;
            }
            let clamped_end = end_f.min(total_frames - 1);
            for f in start_f..=clamped_end {
                let t = if end_f == start_f {
                    0.0
                } else {
                    (f as f32 - start_f as f32) / (end_f as f32 - start_f as f32)
                };
                let base = f * vl;
                for c in 0..vl {
                    data[base + c] = if bin0 {
                        v0_vals[c]
                    } else {
                        v0_vals[c] * (1.0 - t) + v1_vals[c] * t
                    };
                }
            }
        }

        Some(W3dAnimChannel {
            first_frame: 0,
            last_frame: (num_frames.max(1) - 1) as u16,
            vector_len,
            flags: Self::map_timecoded_flag(flags_tc),
            pivot,
            data,
        })
    }

    /// Parse a single adaptive delta animation channel (ANIM_FLAVOR_ADAPTIVE_DELTA, flavor=1).
    /// Format: num_frames(u32) + pivot(u16) + vector_len(u8) + flags(u8) + scale(f32)
    ///         then blocks of 16 frames each, where each block contains vector_len packets.
    ///         Each packet: 1 byte (filter_idx in lower 7 bits) + 8 bytes (16 4-bit nibbles).
    ///         Delta decoding with adaptive filter table.
    pub(super) fn parse_adaptive_delta_channel(
        chunk_data: &[u8],
        hdr_num_frames: u32,
    ) -> Vec<W3dAnimChannel> {
        if chunk_data.len() < 12 {
            return Vec::new();
        }
        let num_frames =
            u32::from_le_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]])
                as usize;
        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
        let vector_len = chunk_data[6] as usize;
        let _flags_raw = chunk_data[7] as u16;
        let scale =
            f32::from_le_bytes([chunk_data[8], chunk_data[9], chunk_data[10], chunk_data[11]]);

        if vector_len == 0 || num_frames == 0 {
            return Vec::new();
        }

        let blocks = num_frames.div_ceil(16);
        let packet_count = blocks * vector_len;
        let bytes_needed = 8 + packet_count * 9;
        if chunk_data.len() < bytes_needed {
            return Vec::new();
        }

        let filter_table = Self::build_adaptive_delta_filter_table();
        let mut data = vec![0.0f32; num_frames * vector_len];
        let mut last_vals = vec![0.0f32; vector_len];
        if vector_len == 4 {
            last_vals[3] = 1.0;
        }

        let mut read_pos = 12usize;
        for b in 0..blocks {
            for vi in 0..vector_len {
                let b0 = chunk_data[read_pos];
                read_pos += 1;
                let filter_idx = (b0 & 0x7F) as usize;
                let mut nibbles = [0u8; 16];
                for byte_i in 0..8 {
                    let byte = chunk_data[read_pos];
                    read_pos += 1;
                    nibbles[byte_i * 2] = byte & 0x0F;
                    nibbles[byte_i * 2 + 1] = (byte >> 4) & 0x0F;
                }
                let filter = filter_table.get(filter_idx).copied().unwrap_or(1.0) * scale;
                for fi in 0..16 {
                    let frame = b * 16 + fi;
                    if frame >= num_frames {
                        break;
                    }
                    let raw = nibbles[fi] as i32;
                    let factor = (raw - 8) as f32;
                    let value = last_vals[vi] + factor * filter;
                    data[frame * vector_len + vi] = value;
                    last_vals[vi] = value;
                }
            }
        }

        let mut out = Vec::new();
        if vector_len == 3 {
            for axis in 0..3 {
                let mut axis_data = Vec::with_capacity(num_frames);
                for f in 0..num_frames {
                    axis_data.push(data[f * 3 + axis]);
                }
                out.push(W3dAnimChannel {
                    first_frame: 0,
                    last_frame: (hdr_num_frames.max(1) - 1) as u16,
                    vector_len: 1,
                    flags: axis as u16,
                    pivot,
                    data: axis_data,
                });
            }
        } else if vector_len == 4 {
            for f in 0..num_frames {
                let i = f * 4;
                let x = data[i];
                let y = data[i + 1];
                let z = data[i + 2];
                let w = data[i + 3];
                let len = (x * x + y * y + z * z + w * w).sqrt();
                if len > 1e-5 {
                    data[i] = x / len;
                    data[i + 1] = y / len;
                    data[i + 2] = z / len;
                    data[i + 3] = w / len;
                }
            }
            out.push(W3dAnimChannel {
                first_frame: 0,
                last_frame: (hdr_num_frames.max(1) - 1) as u16,
                vector_len: 4,
                flags: 6,
                pivot,
                data,
            });
        } else {
            let mut axis_data = Vec::with_capacity(num_frames);
            for f in 0..num_frames {
                axis_data.push(data[f * vector_len]);
            }
            out.push(W3dAnimChannel {
                first_frame: 0,
                last_frame: (hdr_num_frames.max(1) - 1) as u16,
                vector_len: 1,
                flags: 0,
                pivot,
                data: axis_data,
            });
        }
        out
    }

    pub(super) fn map_timecoded_flag(flag: u16) -> u16 {
        match flag {
            8 => 0,
            9 => 1,
            10 => 2,
            11 => 6,
            _ => flag,
        }
    }

    pub(super) fn build_adaptive_delta_filter_table() -> [f32; 256] {
        let mut table = [0.0f32; 256];
        let base: [f32; 16] = [
            1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1000.0, 10000.0,
            100000.0, 1000000.0, 10000000.0,
        ];
        table[..16].copy_from_slice(&base);
        let gen_start = 16usize;
        let gen_size = 256 - gen_start;
        for i in 0..gen_size {
            let ratio = i as f32 / gen_size as f32;
            table[gen_start + i] = 1.0 - (std::f32::consts::FRAC_PI_2 * ratio).sin();
        }
        table
    }

    /// Compute global bone transforms from hierarchy pivots.
    /// Ported from standalone writer.rs compute_global_transforms + mat4_from_tr_quat.
    pub(super) fn compute_global_transforms(hierarchy: &W3dHierarchy) -> Option<Vec<[f32; 16]>> {
        let locals = hierarchy
            .pivots
            .iter()
            .map(Self::mat4_from_tr_quat)
            .collect::<Vec<_>>();
        compute_htree_global_transforms_from_locals(hierarchy, &locals)
    }

    /// Build the exact source-space affine matrix used by the shared HTree
    /// evaluator.  Keep the legacy HMODEL residual on the same representation
    /// as rigid HLOD/palette paths so parent-child order cannot diverge.
    pub(super) fn mat4_from_tr_quat(pivot: &W3dPivot) -> [f32; 16] {
        mat4_from_pivot(pivot)
    }
}

#[cfg(test)]
mod companion_htree_tests {
    use super::super::W3DModel;
    use super::super::w3d_format::{
        W3D_CHUNK_HIERARCHY, W3D_CHUNK_HIERARCHY_HEADER, W3D_CHUNK_PIVOTS,
        W3D_CURRENT_HTREE_VERSION, W3D_NAME_LEN, W3dHlod, W3dHlodLod, W3dHlodSubObject,
    };
    use super::*;
    fn chunk(chunk_type: u32, payload: Vec<u8>, container: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len());
        out.extend_from_slice(&chunk_type.to_le_bytes());
        let raw_size = (payload.len() as u32) | if container { 0x8000_0000 } else { 0 };
        out.extend_from_slice(&raw_size.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    fn fixed_name(name: &str, len: usize) -> Vec<u8> {
        let mut out = vec![0; len];
        let bytes = name.as_bytes();
        let copy_len = bytes.len().min(len);
        out[..copy_len].copy_from_slice(&bytes[..copy_len]);
        out
    }

    fn pivot(name: &str, parent: u32, translation: [f32; 3]) -> Vec<u8> {
        let mut out = fixed_name(name, W3D_NAME_LEN);
        out.extend_from_slice(&parent.to_le_bytes());
        for value in translation {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 12]);
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&1.0f32.to_le_bytes());
        out
    }

    fn companion_hierarchy_bytes(name: &str) -> Vec<u8> {
        let mut hierarchy_header = Vec::with_capacity(36);
        hierarchy_header.extend_from_slice(&W3D_CURRENT_HTREE_VERSION.to_le_bytes());
        hierarchy_header.extend_from_slice(&fixed_name(name, W3D_NAME_LEN));
        hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
        hierarchy_header.extend_from_slice(&[0u8; 12]);
        let mut pivots = pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]);
        pivots.extend_from_slice(&pivot("BONE", 0, [4.0, 5.0, 6.0]));
        chunk(
            W3D_CHUNK_HIERARCHY,
            [
                chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
                chunk(W3D_CHUNK_PIVOTS, pivots, false),
            ]
            .concat(),
            true,
        )
    }

    #[test]
    fn get_htree_companion_paths_follow_cxx_load_on_demand_order() {
        // C++ `WW3DAssetManager::Get_HTree` (`assetmgr.cpp:964-971`) loads
        // `{name}.w3d` then `..\{name}.w3d`.
        let paths = w3d_companion_hierarchy_archive_path_variants("CompTree");
        assert_eq!(paths[0], "CompTree.w3d");
        assert_eq!(paths[1], "../CompTree.w3d");
        assert!(paths.iter().any(|path| path == "Art/W3D/CompTree.W3D"));
    }

    #[test]
    fn import_named_hlod_hierarchy_from_bytes_is_case_insensitive() {
        let mut model = W3DModel::new("geometry".to_string());
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: "HLODROOT".to_string(),
            hierarchy_name: "COMPTREE".to_string(),
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![W3dHlodSubObject {
                    name: "HLODROOT.RIGID".to_string(),
                    bone_index: 1,
                }],
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        assert_eq!(
            model.missing_named_hlod_hierarchy_names(),
            vec!["COMPTREE".to_string()]
        );

        let loader = W3DLoader::new();
        assert!(loader.import_named_hlod_hierarchy_from_bytes(
            &mut model,
            "COMPTREE",
            &companion_hierarchy_bytes("comptree"),
        ));
        assert!(model.missing_named_hlod_hierarchy_names().is_empty());
        assert!(
            model
                .source_hierarchy_for_hlod(&model.hlods[0])
                .is_some_and(|hierarchy| hierarchy.name.eq_ignore_ascii_case("comptree"))
        );
    }
}
