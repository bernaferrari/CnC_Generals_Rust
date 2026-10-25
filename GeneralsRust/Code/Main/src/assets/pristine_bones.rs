//! Device-independent original logical-condition pristine pose queries.
use crate::assets::{AuthoredDrawModel, W3DModel, W3dAnimationBinding};

/// The same immutable decoded-model kernel serves native and headless owners.
/// No installed HAnim, recoil, world transform or GPU object is consulted.
pub(crate) fn model_pristine_bone_transforms(
    model: &W3DModel,
    selected: &AuthoredDrawModel,
    binding: Option<&W3dAnimationBinding>,
    scale: f32,
    prefix: &str,
    start: i32,
    limit: usize,
) -> Result<Vec<glam::Mat4>, String> {
    if !scale.is_finite() || start < 0 {
        return Err("invalid pristine pose source".into());
    }
    if prefix.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let frame = if selected.flags & (1 << 4) != 0 {
        match binding {
            Some(binding) => model
                .animation_binding_metadata(binding)
                .filter(|(frames, _)| *frames != 0)
                .map(|(frames, _)| (frames - 1) as f32)
                .ok_or_else(|| "pristine final animation frame unavailable".to_owned())?,
            None => 0.,
        }
    } else {
        0.
    };
    let mut out = Vec::new();
    let end = if start == 0 { 0 } else { 99 };
    for index in start..=end {
        if out.len() >= limit.min(64) {
            break;
        }
        let name = if index == 0 {
            prefix.to_owned()
        } else {
            format!("{prefix}{index:02}")
        };
        let Some(local) = model
            .source_named_bone_transform(&name, binding, frame, &[])
            .map_err(|error| format!("pristine source hierarchy unavailable: {error:?}"))?
        else {
            break;
        };
        out.push(glam::Mat4::from_scale(glam::Vec3::splat(scale)) * local);
    }
    Ok(out)
}

/// Device-independent content capability borrowed from the driving session.
/// Missing authored assets are errors; an empty result means a genuine bone miss.
pub(crate) trait PristineBoneAssets {
    fn pristine_bone_transforms(
        &self,
        source: &crate::game_logic::NativeSlaveSource,
        prefix: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<glam::Mat4>, String>;
}
impl PristineBoneAssets for crate::assets::AssetManager {
    fn pristine_bone_transforms(
        &self,
        source: &crate::game_logic::NativeSlaveSource,
        prefix: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        if !source.mesh_scale.is_finite() || start < 0 {
            return Err("invalid pristine pose source".into());
        }
        if prefix.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let models = source
            .receiving_draw_models
            .object_draws(source.condition_bits)
            .map_err(|_| "unsupported receiving pristine Draw state".to_owned())?;
        let mut out = Vec::new();
        for selected in models {
            if out.len() >= limit {
                break;
            }
            if selected.model_key.is_empty() {
                continue;
            }
            let prototype = self
                .cached_w3d_render_object_prototype(&selected.model_key)
                .ok_or_else(|| {
                    format!(
                        "pristine authored prototype unavailable: {}",
                        selected.model_key
                    )
                })?;
            // Content admission/prewarm belongs to the receiving session's
            // setup. This immutable query never creates a GPU resource or
            // selects another manager when source content is unavailable.
            let model = self
                .cached_w3d_render_object_source_model(&prototype)
                .ok_or_else(|| "pristine prototype lost its receiving decoded model".to_owned())?;
            let binding = match selected.animations.first() {
                Some(animation) => Some(
                    self.cached_w3d_draw_animation_binding(model, &animation.name)
                        .ok_or_else(|| {
                            "pristine authored animation binding unavailable".to_owned()
                        })?,
                ),
                None => None,
            };
            out.extend(model_pristine_bone_transforms(
                model,
                &selected,
                binding.as_ref(),
                source.mesh_scale,
                prefix,
                start,
                limit - out.len(),
            )?);
        }
        Ok(out)
    }
}

/// Borrow the application's actual archive/decoded-content service for one
/// synchronous query. Its mutex protects real asset-loader concurrency. The
/// guard ends before returning transforms and before gameplay callbacks run.
impl PristineBoneAssets for std::sync::Mutex<crate::assets::AssetManager> {
    fn pristine_bone_transforms(
        &self,
        source: &crate::game_logic::NativeSlaveSource,
        prefix: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let mut assets = self
            .lock()
            .map_err(|_| "receiving pristine asset service poisoned".to_owned())?;
        let models = source
            .receiving_draw_models
            .object_draws(source.condition_bits)
            .map_err(|_| "unsupported receiving pristine Draw state".to_owned())?;
        for selected in models {
            if selected.model_key.is_empty() {
                continue;
            }
            assets
                .resolve_w3d_render_object_prototype_blocking(&selected.model_key)
                .ok_or_else(|| {
                    format!(
                        "pristine authored prototype unavailable: {}",
                        selected.model_key
                    )
                })?;
            if let Some(animation) = selected.animations.first() {
                if !assets.prewarm_w3d_draw_animation_binding(&selected.model_key, &animation.name)
                {
                    return Err(format!(
                        "pristine authored animation unavailable: {}",
                        animation.name
                    ));
                }
            }
        }
        PristineBoneAssets::pristine_bone_transforms(&*assets, source, prefix, start, limit)
    }
}
