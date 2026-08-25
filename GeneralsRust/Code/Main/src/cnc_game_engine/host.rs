#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
//! Extra `CnCGameEngine` impl chunk (texture preload) extracted from the god file.
//!
//! Mechanical split: keeps the second impl block out of `cnc_game_engine.rs`.

use super::*;
use crate::assets::W3DModel;
use crate::graphics::{GraphicsSystem, graphics_system::MAX_STAGE_TEXTURES};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::sync::Arc;

impl CnCGameEngine {
    /// Preload textures from all cached models using C++ approach - material names as texture files
    pub(super) async fn preload_model_textures(
        graphics_system: &mut GraphicsSystem,
    ) -> anyhow::Result<()> {
        use std::collections::HashSet;

        log::info!(
            "🎨 TEXTURE: Loading textures using C++ approach - material names as texture filenames"
        );

        // Get all models from graphics system cache and collect material names as texture names
        let mut texture_names: HashSet<String> = HashSet::new();

        // Get all cached models from graphics system
        for (model_name, model) in graphics_system.get_all_models() {
            log::debug!(
                "🔍 TEXTURE: Scanning model '{}' for referenced stage textures...",
                model_name
            );

            Self::collect_material_textures(model, &mut texture_names);

            for mesh in &model.meshes {
                // Direct material reference on mesh (fallback path)
                if let Some(ref tex_name) = mesh.material.texture_name {
                    if Self::is_valid_texture_name(tex_name) {
                        texture_names.insert(tex_name.clone());
                        log::debug!("  📄 Found mesh embedded texture: {}", tex_name);
                    }
                }

                // Authoritative per-pass stage texture names (preferred)
                for (pass_idx, stage_sets) in mesh.per_pass_stage_texture_names.iter().enumerate() {
                    for (stage_idx, names) in stage_sets.iter().enumerate() {
                        let mut stage_populated = false;
                        for texture_name in names {
                            if Self::is_valid_texture_name(texture_name) {
                                texture_names.insert(texture_name.clone());
                                stage_populated = true;
                                log::debug!(
                                    "  📄 Pass {} Stage {} texture: {}",
                                    pass_idx,
                                    stage_idx,
                                    texture_name
                                );
                            }
                        }

                        if !stage_populated {
                            for fallback in mesh.stage_texture_names_from_ids(pass_idx, stage_idx) {
                                if Self::is_valid_texture_name(&fallback) {
                                    texture_names.insert(fallback.clone());
                                    log::debug!(
                                        "  📄 Pass {} Stage {} texture (from IDs): {}",
                                        pass_idx,
                                        stage_idx,
                                        fallback
                                    );
                                }
                            }
                        }
                    }
                }

                if mesh.per_pass_stage_texture_names.is_empty()
                    && !mesh.per_pass_stage_texture_ids.is_empty()
                {
                    for (pass_idx, stages) in mesh.per_pass_stage_texture_ids.iter().enumerate() {
                        for stage_idx in 0..stages.len() {
                            for fallback in mesh.stage_texture_names_from_ids(pass_idx, stage_idx) {
                                if Self::is_valid_texture_name(&fallback) {
                                    texture_names.insert(fallback.clone());
                                    log::debug!(
                                        "  📄 Pass {} Stage {} texture (from IDs): {}",
                                        pass_idx,
                                        stage_idx,
                                        fallback
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!(
            "🎨 TEXTURE: Found {} unique material-based textures to load",
            texture_names.len()
        );
        log::info!(
            "🎨 TEXTURE: First 10 texture names: {:?}",
            texture_names.iter().take(10).collect::<Vec<_>>()
        );

        if texture_names.is_empty() {
            log::warn!("⚠️  TEXTURE: No material names found - skipping preload");
            return Ok(());
        }

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            let mut loaded_count = 0;
            let mut failed_count = 0;
            let total_textures = texture_names.len();
            let texture_names: Vec<_> = texture_names.iter().collect();

            log::info!(
                "🎨 TEXTURE: Starting preload of {} textures",
                total_textures
            );

            for (index, texture_name) in texture_names.iter().enumerate() {
                log::debug!(
                    "🎯 Loading texture {}/{}: {}",
                    index + 1,
                    total_textures,
                    texture_name
                );

                let load_result = async {
                    match asset_manager_arc.lock() {
                        Ok(mut asset_manager) => {
                            asset_manager
                                .load_texture(
                                    graphics_system.device(),
                                    graphics_system.queue(),
                                    texture_name,
                                )
                                .await;
                            true
                        }
                        Err(_) => {
                            log::warn!(
                                "Could not acquire asset manager lock for texture: {}",
                                texture_name
                            );
                            false
                        }
                    }
                }
                .await;

                if load_result {
                    loaded_count += 1;
                } else {
                    failed_count += 1;
                }
            }

            log::info!(
                "✅ TEXTURE PRELOAD: Loaded {} textures ({} failed/timeout)",
                loaded_count,
                failed_count
            );
        } else {
            log::error!("❌ TEXTURE PRELOAD: Asset manager not available");
        }

        Ok(())
    }

    pub(super) fn collect_material_textures(
        model: &Arc<W3DModel>,
        texture_names: &mut HashSet<String>,
    ) {
        for (material_name, material) in &model.materials {
            if Self::is_valid_texture_name(material_name) {
                texture_names.insert(material_name.clone());
                log::debug!("  📄 Found material-as-texture: {}", material_name);
            }

            if let Some(ref texture_name) = material.texture_name {
                if Self::is_valid_texture_name(texture_name) {
                    texture_names.insert(texture_name.clone());
                    log::debug!("  📄 Found explicit material texture: {}", texture_name);
                }
            }

            for stage_idx in 0..MAX_STAGE_TEXTURES {
                if let Some(stage_texture) = GraphicsSystem::stage_texture_name(material, stage_idx)
                {
                    if Self::is_valid_texture_name(stage_texture) {
                        texture_names.insert(stage_texture.clone());
                        log::debug!(
                            "  📄 Material stage{} texture: {}",
                            stage_idx,
                            stage_texture
                        );
                    }
                }
            }
        }
    }

    pub(super) fn is_valid_texture_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.eq_ignore_ascii_case("default") {
            return false;
        }
        name.parse::<usize>().is_err()
    }

    /// Preload textures using WW3D Asset Manager definitions
    /// This loads textures defined in INI object definitions from INIZH.big
    pub(super) async fn preload_ww3d_textures(
        graphics_system: &mut GraphicsSystem,
    ) -> anyhow::Result<()> {
        info!("🎨 TEXTURE: Preloading textures from WW3D Asset Manager definitions...");

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            // First, get the list of texture filenames
            let texture_filenames = {
                let asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
                asset_manager.get_all_texture_filenames()
            };

            info!(
                "🎨 TEXTURE: WW3D Asset Manager has {} unique texture filenames to load",
                texture_filenames.len()
            );

            // Show first 20 texture names for debugging
            for (index, name) in texture_filenames.iter().take(20).enumerate() {
                debug!("  📄 Texture {}: {}", index + 1, name);
            }

            if texture_filenames.len() > 20 {
                info!("  ... and {} more textures", texture_filenames.len() - 20);
            }

            // Load ALL textures (matching C++ behavior - no artificial limit)
            let mut loaded_count = 0;
            let mut failed_count = 0;
            let total_to_load = texture_filenames.len(); // Load all textures upfront like C++

            info!(
                "🎨 TEXTURE: Loading ALL {} textures from BIG archives (matching C++ behavior)...",
                total_to_load
            );

            for (index, texture_name) in texture_filenames.iter().enumerate() {
                debug!(
                    "🎯 Loading WW3D texture {}/{}: {}",
                    index + 1,
                    total_to_load,
                    texture_name
                );

                // Try to load the texture with timeout
                let load_future = async {
                    match asset_manager_arc.lock() {
                        Ok(mut asset_manager) => {
                            // Load the texture asynchronously
                            match asset_manager
                                .load_texture_async(
                                    graphics_system.device(),
                                    graphics_system.queue(),
                                    texture_name,
                                )
                                .await
                            {
                                Ok(_) => {
                                    debug!("✅ Loaded texture: {}", texture_name);
                                    true
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to load texture {}: {}", texture_name, e);
                                    false
                                }
                            }
                        }
                        Err(_) => {
                            warn!("Could not lock asset manager for texture: {}", texture_name);
                            false
                        }
                    }
                };

                match tokio::time::timeout(tokio::time::Duration::from_millis(500), load_future)
                    .await
                {
                    Ok(true) => loaded_count += 1,
                    Ok(false) => failed_count += 1,
                    Err(_) => {
                        failed_count += 1;
                        warn!("⏰ Texture '{}' timeout (500ms)", texture_name);
                    }
                }

                // Small delay between textures
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }

            info!(
                "✅ WW3D TEXTURE PRELOAD: Loaded {} textures ({} failed/timeout) from {} available",
                loaded_count,
                failed_count,
                texture_filenames.len()
            );
        } else {
            warn!("⚠️ WW3D TEXTURE PRELOAD: Asset manager not available");
        }

        Ok(())
    }
}
