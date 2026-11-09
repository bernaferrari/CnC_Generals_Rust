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

/// Immutable model-selection inputs borrowed for one pristine bone query.
pub(crate) struct PristineBoneSource<'a> {
    pub drawings: &'a crate::game_logic::thing::ReceivingDrawModels,
    pub condition_bits: u128,
    pub mesh_scale: f32,
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
    ) -> Result<Vec<glam::Mat4>, String> {
        self.pristine_bones_for_draws(
            &PristineBoneSource {
                drawings: source.receiving_draw_models.as_ref(),
                condition_bits: source.condition_bits,
                mesh_scale: source.mesh_scale,
            },
            prefix,
            start,
            limit,
        )
    }
    fn pristine_bones_for_draws(
        &self,
        source: &PristineBoneSource<'_>,
        prefix: &str,
        start: i32,
        limit: usize,
    ) -> Result<Vec<glam::Mat4>, String>;
    /// Resolve a particular selected ObjectDraw module rather than searching
    /// the Drawable's whole ordered Draw roster.
    fn pristine_bones_for_selected_draw(
        &self,
        _draw: &AuthoredDrawModel,
        _scale: f32,
        _prefix: &str,
        _start: i32,
        _limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        Err("receiving asset service cannot query a selected Draw".into())
    }
}
impl PristineBoneAssets for crate::assets::AssetManager {
    fn pristine_bones_for_selected_draw(
        &self, draw: &AuthoredDrawModel, scale: f32, prefix: &str, start: i32, limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        if !scale.is_finite() || start < 0 {
            return Err("invalid pristine pose source".into());
        }
        if draw.model_key.is_empty() || prefix.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let prototype = self.cached_w3d_render_object_prototype(&draw.model_key)
            .ok_or_else(|| format!("pristine authored prototype unavailable: {}", draw.model_key))?;
        let model = self.cached_w3d_render_object_source_model(&prototype)
            .ok_or_else(|| "pristine prototype lost its receiving decoded model".to_owned())?;
        let binding = match draw.animations.first() {
            Some(animation) => Some(self.cached_w3d_draw_animation_binding(model, &animation.name)
                .ok_or_else(|| "pristine authored animation binding unavailable".to_owned())?),
            None => None,
        };
        model_pristine_bone_transforms(model, draw, binding.as_ref(), scale, prefix, start, limit)
    }

    fn pristine_bones_for_draws(
        &self,
        source: &PristineBoneSource<'_>,
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
        let models = source.drawings.object_draws(source.condition_bits)
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
    fn pristine_bones_for_selected_draw(
        &self, draw: &AuthoredDrawModel, scale: f32, prefix: &str, start: i32, limit: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        if !scale.is_finite() || start < 0 {
            return Err("invalid pristine pose source".into());
        }
        let mut assets = self.lock().map_err(|_| "receiving pristine asset service poisoned".to_owned())?;
        if !draw.model_key.is_empty() && !prefix.is_empty() && limit != 0 {
            let prototype = assets.resolve_w3d_render_object_prototype_blocking(&draw.model_key)
                .ok_or_else(|| format!("pristine authored prototype unavailable: {}", draw.model_key))?;
            if let Some(animation) = draw.animations.first() {
                if !assets.prewarm_w3d_prototype_animation_binding(&prototype, &animation.name) {
                    return Err(format!("pristine authored animation unavailable: {}", animation.name));
                }
            }
        }
        PristineBoneAssets::pristine_bones_for_selected_draw(&*assets, draw, scale, prefix, start, limit)
    }

    fn pristine_bones_for_draws(
        &self,
        source: &PristineBoneSource<'_>,
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
        let mut assets = self
            .lock()
            .map_err(|_| "receiving pristine asset service poisoned".to_owned())?;
        let models = source.drawings.object_draws(source.condition_bits)
            .map_err(|_| "unsupported receiving pristine Draw state".to_owned())?;
        for selected in models {
            if selected.model_key.is_empty() {
                continue;
            }
            let prototype = assets
                .resolve_w3d_render_object_prototype_blocking(&selected.model_key)
                .ok_or_else(|| {
                    format!(
                        "pristine authored prototype unavailable: {}",
                        selected.model_key
                    )
                })?;
            if let Some(animation) = selected.animations.first() {
                if !assets.prewarm_w3d_prototype_animation_binding(&prototype, &animation.name)
                {
                    return Err(format!(
                        "pristine authored animation unavailable: {}",
                        animation.name
                    ));
                }
            }
        }
        PristineBoneAssets::pristine_bones_for_draws(&*assets, source, prefix, start, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::ini_parser::AuthoredDrawAnimation;
    use crate::assets::models::test_animation::{motion, raw_animation};
    use crate::assets::{W3dHierarchy, W3dHlod, W3dHlodLod, W3dPivot};

    #[test]
    fn retail_patriot_muzzle_loads_from_archive_on_cold_selected_draw_query() {
        let archive = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../windows_game/Command & Conquer Generals/W3D.big");
        let mut manager = crate::assets::AssetManager::new().unwrap();
        manager.archive_file_system_mut().set_local_search_roots(Vec::new());
        futures::executor::block_on(manager.archive_file_system_mut().load_big_file(archive))
            .expect("retail base W3D archive");
        assert!(manager.cached_w3d_render_object_prototype("ABPatriot").is_none());
        assert!(manager.get_cached_model_ref("ABPatriot").is_none());

        let assets = std::sync::Mutex::new(manager);
        // FactionBuilding.ini's pristine Patriot Draw selects ABPatriot and
        // names WeaponA as its PRIMARY launch-bone base.
        let draw = AuthoredDrawModel {
            model_key: "ABPatriot".into(),
            ..Default::default()
        };
        let bones = assets
            .pristine_bones_for_selected_draw(&draw, 1.0, "WeaponA", 1, 1)
            .expect("retail muzzle query must load its own source");
        assert_eq!(bones.len(), 1);
        // Sum of the retail ROOTTRANSFORM/CHASSIS/TURRET01/TURRETEL/
        // WEAPONA01 translations, in source C++ Z-up coordinates.
        let expected = glam::Vec3::new(0.193_172, 1.700_902, 14.466_129);
        assert!((bones[0].w_axis.truncate() - expected).length() < 0.001);
        let manager = assets.lock().unwrap();
        assert!(manager.cached_w3d_render_object_prototype("ABPatriot").is_some());
        assert!(manager.get_cached_model_ref("ABPatriot").is_none());
    }

    #[test]
    fn selected_draw_uses_prototype_source_for_animation_without_presentation_cache() {
        let mut model = W3DModel::new("MISSILE_TEST".into());
        let pivot = |name: &str, parent_idx, translation| W3dPivot {
            name: name.into(),
            parent_idx,
            translation,
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        model.hierarchy = Some(W3dHierarchy {
            name: "MISSILE_RIG".into(),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0.0; 3]),
                pivot("MUZZLE01", 0, [0.0; 3]),
            ],
            pivot_fixups: Vec::new(),
        });
        model.hlods.push(W3dHlod {
            version: 0,
            name: "MISSILE_TEST".into(),
            hierarchy_name: "MISSILE_RIG".into(),
            lods: vec![W3dHlodLod { max_screen_size: 1.0, subobjects: Vec::new() }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        model.animations.push(raw_animation(
            "FIRE", "MISSILE_RIG", 2, 30,
            vec![motion(1, 0, 0, 1, vec![0.0, 17.0])], Vec::new(),
        ));
        let mut manager = crate::assets::AssetManager::new().unwrap();
        manager.admit_test_w3d_source("MISSILE_TEST", model);
        assert!(manager.get_cached_model_ref("MISSILE_TEST").is_none());
        let assets = std::sync::Mutex::new(manager);
        let draw = AuthoredDrawModel {
            model_key: "MISSILE_TEST".into(),
            animations: vec![AuthoredDrawAnimation {
                name: "MISSILE_RIG.FIRE".into(),
                is_idle: false,
                distance_covered_token: None,
            }],
            flags: 1 << 4,
            ..Default::default()
        };
        let bones = assets.pristine_bones_for_selected_draw(&draw, 1.0, "MUZZLE", 1, 1).unwrap();
        assert_eq!(bones.len(), 1);
        assert!((bones[0].w_axis.x - 17.0).abs() < 0.001);
    }
}
