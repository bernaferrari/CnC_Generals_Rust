//! Device-independent original logical-condition pristine pose queries.
use crate::assets::ini_parser::ACBIT_PRISTINE_BONE_POS_IN_FINAL_FRAME;
use crate::assets::{AuthoredDrawModel, W3DModel, W3dAnimationBinding, W3dRenderObjectPrototypeKind};

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
    let frame = if selected.flags & (1 << ACBIT_PRISTINE_BONE_POS_IN_FINAL_FRAME) != 0 {
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
    let mut globals = None;
    let scale = glam::Mat4::from_scale(glam::Vec3::splat(scale));
    let unavailable = |error| format!("pristine source hierarchy unavailable: {error:?}");
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
        let Some(bone_index) = model
            .source_named_bone_index(&name)
            .map_err(unavailable)?
        else {
            break;
        };
        // Sample the exact source pose once, after the first successful name
        // lookup. A mesh with no matching bone needs no hierarchy evaluation.
        let transforms = match globals {
            Some(ref transforms) => transforms,
            None => globals.insert(
                model
                    .source_controlled_hierarchy_transforms(binding, frame, 1.0, &[])
                    .map_err(unavailable)?,
            ),
        };
        let local =
            W3DModel::checked_source_bone_transform(transforms, bone_index).map_err(unavailable)?;
        out.push(scale * local);
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
        // Null3DObjClass inherits the base zero-bone lookup, even if the
        // selected state supplies an animation intended for an HLOD.
        if matches!(prototype.kind(), W3dRenderObjectPrototypeKind::Null { .. }) {
            return Ok(Vec::new());
        }
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
            if matches!(prototype.kind(), W3dRenderObjectPrototypeKind::Null { .. }) {
                continue;
            }
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
            if matches!(prototype.kind(), W3dRenderObjectPrototypeKind::Null { .. }) {
                return Ok(Vec::new());
            }
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
            if matches!(prototype.kind(), W3dRenderObjectPrototypeKind::Null { .. }) {
                continue;
            }
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
    fn builtin_null_is_resident_without_an_archive_and_supplies_no_bones() {
        use crate::assets::models::W3dNullProto;

        let mut manager = crate::assets::AssetManager::new().unwrap();
        manager.archive_file_system_mut().set_local_search_roots(Vec::new());
        let prototype = manager.cached_w3d_render_object_prototype("nUlL").unwrap();
        assert_eq!(prototype.full_name(), "NULL");
        assert_eq!(prototype.source_file_stem(), "");
        assert_eq!(prototype.kind(), W3dRenderObjectPrototypeKind::Null { null_index: 0 });
        let object = manager.cached_w3d_render_object_source_model(&prototype).unwrap();
        assert!(object.meshes.is_empty());
        assert_eq!(object.bounding_box_min, glam::Vec3::splat(-0.1));
        assert_eq!(object.bounding_box_max, glam::Vec3::splat(0.1));
        assert_eq!(manager.load_w3d_model("Null").unwrap().name, "NULL");
        assert_eq!(futures::executor::block_on(manager.load_w3d_model_async("null")).unwrap().name, "NULL");
        assert_eq!(futures::executor::block_on(manager.load_cnc_model("NULL")).unwrap().name, "NULL");

        let mut authored = W3DModel::new("NullArchive".into());
        authored.nulls.extend(["NULL", "NULL.Authored"].map(|name| W3dNullProto { name: name.into() }));
        manager.admit_test_w3d_source("NULL", authored);
        assert_eq!(manager.resolve_w3d_render_object_prototype_blocking("NULL"), Some(prototype.clone()));
        assert_eq!(manager.cached_w3d_render_object_prototype("NULL.Authored").unwrap().source_file_stem(), "NULL");
        manager.clear_caches();
        assert_eq!(manager.cached_w3d_render_object_prototype("NULL"), Some(prototype));

        let assets = std::sync::Mutex::new(manager);
        let draw = AuthoredDrawModel {
            model_key: "NULL".into(),
            animations: vec![AuthoredDrawAnimation {
                name: "Unused.Absent".into(),
                is_idle: false,
                distance_covered_token: None,
            }],
            ..Default::default()
        };
        assert!(assets.pristine_bones_for_selected_draw(&draw, 1.0, "muzzle", 1, 1).unwrap().is_empty());
        let mut missing = draw;
        missing.model_key = "MissingMesh".into();
        assert!(assets.pristine_bones_for_selected_draw(&missing, 1.0, "muzzle", 1, 1).is_err());
    }

    #[test]
    fn retail_patriot_muzzle_loads_from_archive_on_cold_selected_draw_query() {
        let archive = crate::fixture_content::retail_base_w3d_archive();
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
    fn retail_infantry_pristine_pose_resolves_its_separate_named_hierarchy() {
        let mut assets = crate::assets::AssetManager::new().unwrap();
        assets.archive_file_system_mut().set_local_search_roots(Vec::new());
        futures::executor::block_on(assets.archive_file_system_mut().load_big_file(
            crate::fixture_content::retail_base_w3d_archive(),
        )).unwrap();

        let assets = std::sync::Mutex::new(assets);
        for (model_key, hierarchy, animation) in [
            ("UIRGrd_SKN", "UIRGrd_SKL", "UIRGrd_SKL.UIRGrd_RNB"),
            ("UITunF_SKN", "UITunF_SKL", "UITunF_SKL.UITunF_WKA"),
        ] {
            let draw = AuthoredDrawModel {
                model_key: model_key.into(),
                animations: vec![AuthoredDrawAnimation {
                    name: animation.into(),
                    is_idle: false,
                    distance_covered_token: None,
                }],
                ..Default::default()
            };
            let transforms = assets
                .pristine_bones_for_selected_draw(&draw, 1.0, "Muzzle", 1, 1)
                .expect("strict prototype must load its real HTree and HAnim");
            assert_eq!(transforms.len(), 1);
            assert!(transforms[0].w_axis.truncate().is_finite());

            let manager = assets.lock().unwrap();
            let prototype = manager.cached_w3d_render_object_prototype(model_key).unwrap();
            let exact = manager.cached_w3d_render_object_source_model(&prototype).unwrap();
            assert!(exact.hierarchy.as_ref().unwrap().name.eq_ignore_ascii_case(hierarchy));
            assert!(manager.cached_w3d_draw_animation_binding(exact, animation).is_some());
            assert!(manager.get_cached_model_ref(model_key).is_none());
        }
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
                pivot("MUZZLE02", 1, [0.0, 3.0, 0.0]),
                pivot("MUZZLE04", 0, [30.0, 0.0, 0.0]),
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
            flags: 1 << ACBIT_PRISTINE_BONE_POS_IN_FINAL_FRAME,
            ..Default::default()
        };
        let bones = assets.pristine_bones_for_selected_draw(&draw, 1.0, "MUZZLE", 1, 1).unwrap();
        assert_eq!(bones.len(), 1);
        assert!((bones[0].w_axis.x - 17.0).abs() < 0.001);
        let series = assets
            .pristine_bones_for_selected_draw(&draw, 2.0, "muzzle", 1, 4)
            .unwrap();
        assert_eq!(series.len(), 2, "stop at the missing MUZZLE03");
        assert_eq!(series[0].w_axis.truncate(), glam::Vec3::new(34.0, 0.0, 0.0));
        assert_eq!(series[1].w_axis.truncate(), glam::Vec3::new(34.0, 6.0, 0.0));
    }
}
