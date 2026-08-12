#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

impl RenderPipeline {
    pub(super) fn missing_model_debug_cubes_enabled_from(value: Option<&std::ffi::OsStr>) -> bool {
        // Production must never invent geometry for a missing retail W3D.
        // An explicit developer opt-in keeps the diagnostic cube available
        // without allowing a default game run to look playable for the wrong
        // reason.  This is intentionally distinct from a failed W3D load:
        // the collector records that miss and skips the object.
        value
            .and_then(|value| value.to_str())
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    pub(super) fn missing_model_debug_cubes_enabled() -> bool {
        Self::missing_model_debug_cubes_enabled_from(
            std::env::var_os("GENERALS_RENDER_MISSING_MODEL_CUBES").as_deref(),
        )
    }

    pub(super) fn should_prewarm_startup_map_template(
        asset_manager: &crate::assets::AssetManager,
        template: &str,
    ) -> bool {
        let template = template.trim();
        if template.is_empty() {
            return false;
        }

        if let Some(definition) = asset_manager.get_object_definition(template) {
            return definition.model_name.is_some();
        }

        if asset_manager.get_model_for_object(template).is_some() {
            return true;
        }

        let lower = template.to_ascii_lowercase();
        if lower.starts_with("amb_")
            || lower.starts_with("ambient")
            || lower.starts_with("cin_")
            || lower.starts_with("gc_")
            || lower.starts_with("scorch")
        {
            return false;
        }

        false
    }

    pub fn debug_render_item_count(&self) -> usize {
        self.render_items.len()
    }

    pub fn debug_last_alive_objects(&self) -> usize {
        self.debug_last_alive_objects
    }

    pub fn debug_last_fow_filtered(&self) -> usize {
        self.debug_last_fow_filtered
    }

    pub fn debug_last_frustum_culled(&self) -> usize {
        self.debug_last_frustum_culled
    }

    pub fn debug_last_model_missing(&self) -> usize {
        self.debug_last_model_missing
    }

    pub fn debug_last_deferred_model_loads(&self) -> usize {
        self.debug_last_deferred_model_loads
    }

    pub fn debug_last_deferred_model_load_budget(&self) -> usize {
        self.debug_last_deferred_model_load_budget
    }

    pub fn debug_last_model_budget_skips(&self) -> usize {
        self.debug_last_model_budget_skips
    }

    pub fn debug_last_zero_mesh_models(&self) -> usize {
        self.debug_last_zero_mesh_models
    }

    pub fn debug_last_missing_model_samples(&self) -> &[String] {
        &self.debug_last_missing_model_samples
    }

    pub fn debug_render_pass_counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut shadow = 0usize;
        let mut forward_opaque = 0usize;
        let mut forward_transparent = 0usize;
        let mut water = 0usize;
        let mut ui = 0usize;

        for item in &self.render_items {
            match item.render_pass {
                RenderPass::ShadowPass => shadow += 1,
                RenderPass::ForwardOpaque => forward_opaque += 1,
                RenderPass::ForwardTransparent => forward_transparent += 1,
                RenderPass::WaterPass => water += 1,
                RenderPass::UIPass => ui += 1,
            }
        }

        (shadow, forward_opaque, forward_transparent, water, ui)
    }

    pub fn debug_render_item_breakdown_for_objects(&self, object_ids: &[ObjectID]) -> String {
        let focus_ids: HashSet<ObjectID> = object_ids.iter().copied().collect();
        if focus_ids.is_empty() {
            return "none".to_string();
        }

        let mut counts: HashMap<ObjectID, (usize, usize, usize, String)> = HashMap::new();
        for item in &self.render_items {
            if !focus_ids.contains(&item.object_id) {
                continue;
            }

            let entry = counts.entry(item.object_id).or_insert_with(|| {
                (
                    0,
                    0,
                    0,
                    format!("{}::{}", item.model_name, item.material.name),
                )
            });
            match item.render_pass {
                RenderPass::ForwardOpaque => entry.0 += 1,
                RenderPass::ForwardTransparent => entry.1 += 1,
                _ => entry.2 += 1,
            }
        }

        let mut ordered = object_ids.to_vec();
        ordered.sort_unstable();
        ordered
            .into_iter()
            .map(|id| {
                if let Some((opaque, transparent, other, sample)) = counts.get(&id) {
                    format!(
                        "{}:opaque={} transparent={} other={} sample={}",
                        id, opaque, transparent, other, sample
                    )
                } else {
                    format!("{}:opaque=0 transparent=0 other=0 sample=none", id)
                }
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }

    pub fn prewarm_textures_blocking<I, S>(
        &mut self,
        texture_names: I,
    ) -> Result<TexturePrewarmStats>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.forward_pass.prewarm_textures_blocking(texture_names)
    }

    pub fn debug_forward_renderer_stats(&self) -> (u32, u32, u32) {
        let stats = self.forward_pass.renderer.stats();
        (
            stats.draw_calls,
            stats.meshes_rendered,
            stats.triangles_rendered,
        )
    }

    pub(super) fn render_pass_for_material(material: &W3DMaterial) -> RenderPass {
        match material.blend_mode {
            crate::assets::models::BlendMode::Opaque => {
                if material.opacity < 0.999 {
                    RenderPass::ForwardTransparent
                } else {
                    RenderPass::ForwardOpaque
                }
            }
            crate::assets::models::BlendMode::Alpha
            | crate::assets::models::BlendMode::Additive
            | crate::assets::models::BlendMode::Modulate => RenderPass::ForwardTransparent,
        }
    }

    pub(super) fn compare_render_items(a: &RenderItem, b: &RenderItem) -> std::cmp::Ordering {
        let pass_cmp = (a.render_pass as u32).cmp(&(b.render_pass as u32));
        if pass_cmp != std::cmp::Ordering::Equal {
            return pass_cmp;
        }

        if a.render_pass == RenderPass::ForwardTransparent {
            let distance_cmp = b
                .distance
                .partial_cmp(&a.distance)
                .unwrap_or(std::cmp::Ordering::Equal);
            if distance_cmp != std::cmp::Ordering::Equal {
                return distance_cmp;
            }
            let material_cmp = a.material_key.cmp(&b.material_key);
            if material_cmp != std::cmp::Ordering::Equal {
                return material_cmp;
            }
            return a
                .object_id
                .cmp(&b.object_id)
                .then_with(|| a.model_name.cmp(&b.model_name))
                .then_with(|| a.mesh_index.cmp(&b.mesh_index));
        }

        let material_cmp = a.material_key.cmp(&b.material_key);
        if material_cmp != std::cmp::Ordering::Equal {
            return material_cmp;
        }

        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.object_id.cmp(&b.object_id))
            .then_with(|| a.model_name.cmp(&b.model_name))
            .then_with(|| a.mesh_index.cmp(&b.mesh_index))
    }

    pub(super) fn paint_minimap_circle(
        texture: &mut [u8],
        width: u32,
        height: u32,
        center_x: i32,
        center_y: i32,
        radius: i32,
        tint_rgb: [u8; 3],
        blend: f32,
    ) {
        if radius <= 0 {
            return;
        }

        let blend = blend.clamp(0.0, 1.0);
        let px_width = width as i32;
        let px_height = height as i32;
        let radius_sq = radius * radius;

        for oy in -radius..=radius {
            for ox in -radius..=radius {
                if ox * ox + oy * oy > radius_sq {
                    continue;
                }

                let x = center_x + ox;
                let y = center_y + oy;
                if x < 0 || y < 0 || x >= px_width || y >= px_height {
                    continue;
                }

                let base = ((y as u32 * width + x as u32) * 4) as usize;
                texture[base] = (texture[base] as f32 * (1.0 - blend) + tint_rgb[0] as f32 * blend)
                    .clamp(0.0, 255.0) as u8;
                texture[base + 1] = (texture[base + 1] as f32 * (1.0 - blend)
                    + tint_rgb[1] as f32 * blend)
                    .clamp(0.0, 255.0) as u8;
                texture[base + 2] = (texture[base + 2] as f32 * (1.0 - blend)
                    + tint_rgb[2] as f32 * blend)
                    .clamp(0.0, 255.0) as u8;
                texture[base + 3] = 255;
            }
        }
    }
}
