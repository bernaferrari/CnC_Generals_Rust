//! W3DSupplyDraw's stock status update on the receiving Drawable lifetime.
use super::*;
use crate::assets::{W3DModel, W3dRenderObjectPrototypeKind};
use crate::game_logic::NativeSlaveSource;
use game_client::core::PresentationDirectDrawableBindingKey;
use game_engine::common::thing::thing_template::ModuleInfo;
use game_engine::common::model_condition::ModelConditionFlags;
use gamelogic::object::draw::{W3DModelDrawModuleData, W3DSupplyDrawModuleData};
use installed_draw_pose::InstalledDrawObject;
use native_pose_loan::NativeInstalledPoseLoan;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SupplyBoneLatch {
    prefix: String,
    total: i32,
    shown: i32,
}

impl NativeInstalledPoseLoan<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn receive_supply_status(
        &mut self,
        source: &NativeSlaveSource,
        binding: PresentationDirectDrawableBindingKey,
        declarations: &ModuleInfo,
        global_data: &parking_lot::RwLock<game_engine::common::global_data::GlobalData>,
        max: i32,
        current: i32,
        condition_bits: u128,
        graphics: &mut GraphicsSystem,
        mut flush_first_drawable_bone_query: impl FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        self.validate_object_retirement(&source.identity, binding)
            .map_err(str::to_owned)?;
        let owner = NativeDrawOwnerId::Actor(source.identity.object_id());
        let condition = ModelConditionFlags::from_bits_retain(condition_bits);
        for (index, entry) in declarations.iter().enumerate() {
            if !entry.name.as_str().eq_ignore_ascii_case("W3DSupplyDraw") {
                continue;
            }
            let index = u32::try_from(index).map_err(|_| "invalid supply module index")?;
            self.validate_binding(&source.identity, binding, index)
                .map_err(|error| format!("receiving supply binding unavailable: {error:?}"))?;
            let Some(module) = self.drawable_visual_states.get(&(owner, index)) else {
                return Err("receiving supply Draw module has no installed lifetime".into());
            };
            if module.supply_bones.is_none() {
                // W3DSupplyDraw.cpp:55 and Drawable.cpp:747. The first
                // pristine query consumes the just-written CARRYING row.
                // A later range still targets the current RenderObj.
                flush_first_drawable_bone_query()?;
                self.consume_native_camera_conditions(
                    crate::game_logic::ObjectId(source.identity.object_id()),
                    graphics,
                );
                let prefix = {
                    let data = entry.data.read();
                    let supply = data
                        .downcast_ref::<W3DSupplyDrawModuleData>()
                        .ok_or("receiving supply Draw module type differs from its declaration")?;
                    supply.supply_bone_prefix.as_str().to_owned()
                };
                let total = if prefix.is_empty() {
                    0
                } else {
                    let global_data = global_data.read();
                    self.count_public_supply_bones(
                        declarations,
                        &condition,
                        &global_data.standard_public_bones,
                        &prefix,
                    )?
                };
                self.drawable_visual_states
                    .get_mut(&(owner, index))
                    .expect("validated supply Draw owner")
                    .supply_bones = Some(SupplyBoneLatch {
                    prefix,
                    total,
                    shown: total,
                });
            }
            let module = self
                .drawable_visual_states
                .get_mut(&(owner, index))
                .expect("validated supply Draw owner");
            let latch = module
                .supply_bones
                .as_mut()
                .expect("first supply call latches");
            let total = latch.total;
            // W3DSupplyDraw.cpp:60–80. Retail authored positive denominators
            // use this signed conversion. Leave an undefined conversion alone.
            let next = (total as f32 * (current as f32 / max as f32)).ceil();
            if !(-2_147_483_648.0..2_147_483_648.0).contains(&next) {
                continue;
            }
            let next = (next as i32).min(total);
            if next == latch.shown {
                continue;
            }
            let low = latch.shown.min(next);
            let high = latch.shown.max(next);
            let hidden = next < latch.shown;
            if let InstalledDrawObject::Ready(pose) = &mut module.installed_draw_pose {
                for (ordinal, name) in pose.model.supply_child_indices(&latch.prefix) {
                    if ordinal > low && ordinal <= high {
                        pose.write_supply_child_visibility(&name, hidden);
                    }
                }
            }
            latch.shown = next;
        }
        Ok(())
    }

    fn count_public_supply_bones(
        &self,
        declarations: &ModuleInfo,
        condition: &ModelConditionFlags,
        standard_public: &[String],
        prefix: &str,
    ) -> Result<i32, String> {
        if prefix.is_empty() {
            return Ok(0);
        }
        let assets = self
            .application_assets
            .as_ref()
            .ok_or("receiving supply asset owner missing")?;
        let mut assets = assets
            .lock()
            .map_err(|_| "receiving supply asset owner poisoned")?;
        let mut count = 0;
        for entry in declarations.iter() {
            let data = entry.data.read();
            let model_draw = if let Some(supply) = data.downcast_ref::<W3DSupplyDrawModuleData>() {
                &supply.base
            } else if let Some(model) = data.downcast_ref::<W3DModelDrawModuleData>() {
                model
            } else if matches!(entry.name.as_str(), "W3DDefaultDraw" | "W3DLaserDraw") {
                // These ObjectDraw interfaces have no pristine ModelDraw palette.
                continue;
            } else {
                return Err(format!(
                    "unsupported pristine supply Draw type '{}'",
                    entry.name
                ));
            };
            let Some(state) = model_draw.find_best_info(condition) else {
                continue;
            };
            if state.model_name.is_empty() || state.model_name.is_none() {
                continue;
            }
            let public = standard_public
                .iter()
                .map(String::as_str)
                .chain(
                    model_draw
                        .extra_public_bones
                        .iter()
                        .map(|name| name.as_str()),
                )
                .chain(state.public_bones.iter().map(|name| name.as_str()));
            if !public.clone().any(|name| admits_supply_names(name, prefix)) {
                continue;
            }
            let key = state.model_name.as_str();
            // W3DModelDraw.cpp:583–609 validates pristine bones against
            // the exact authored RenderObj, creating it if needed. The
            // presentation model cache does not admit that prototype.
            let prototype = assets
                .resolve_w3d_render_object_prototype_blocking(key)
                .ok_or_else(|| format!("receiving supply prototype {key} could not be resolved"))?;
            if matches!(prototype.kind(), W3dRenderObjectPrototypeKind::Null { .. }) {
                continue;
            }
            let model = assets
                .cached_w3d_render_object_source_model(&prototype)
                .ok_or_else(|| format!("receiving supply prototype {key} has no decoded model"))?;
            count += count_public_prefix(model, public, prefix)?;
        }
        Ok(count)
    }
}

fn admits_supply_names(name: &str, prefix: &str) -> bool {
    if name.eq_ignore_ascii_case(prefix) {
        return true;
    }
    let Some((base, suffix)) = name.split_at_checked(prefix.len()) else {
        return false;
    };
    base.eq_ignore_ascii_case(prefix)
        && suffix.len() == 2
        && suffix
            .parse::<u8>()
            .is_ok_and(|ordinal| (1..=64).contains(&ordinal))
}

fn count_public_prefix<'a>(
    model: &W3DModel,
    public: impl Iterator<Item = &'a str>,
    prefix: &str,
) -> Result<i32, String> {
    let prefix = prefix.to_ascii_lowercase();
    let candidates: [String; 64] = std::array::from_fn(|index| format!("{prefix}{:02}", index + 1));
    let mut present = [false; 64];
    for public in public {
        if public.is_empty() || public.eq_ignore_ascii_case("none") {
            continue;
        }
        let public = public.to_ascii_lowercase();
        // Exact names and numbered families are the only ways this
        // declaration can admit any of the requested public names.
        if public != prefix && !candidates.contains(&public) {
            continue;
        }
        let bone = |name: &str| {
            model
                .source_named_bone_index(name)
                .map(|index| index.is_some())
                .map_err(|error| format!("unsupported receiving supply hierarchy: {error:?}"))
        };
        let has_literal = bone(&public)?;
        let first_number = format!("{public}01");
        let has_numbered = bone(&first_number)?;
        let named = |name: &str| {
            if has_literal || has_numbered {
                bone(name)
            } else {
                Ok(model.has_rigid_render_child_named(name))
            }
        };
        if public == prefix {
            for (index, candidate) in candidates.iter().enumerate() {
                if !named(candidate)? {
                    break;
                }
                present[index] = true;
            }
        } else if named(&public)? {
            let index = candidates
                .iter()
                .position(|name| *name == public)
                .expect("public name is one of the matched numbered candidates");
            present[index] = true;
        }
    }
    Ok(present.iter().take_while(|present| **present).count() as i32)
}

#[cfg(test)]
impl RenderPipeline {
    pub(crate) fn observe_native_supply_draw(
        &self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
    ) -> Option<(Option<(i32, i32)>, Vec<String>, Vec<String>)> {
        let state = self
            .native_draw_world
            .drawable_visual_states
            .get(&(NativeDrawOwnerId::Actor(id.0), module_index))?;
        let InstalledDrawObject::Ready(pose) = &state.installed_draw_pose else {
            return None;
        };
        let latch = state
            .supply_bones
            .as_ref()
            .map(|latch| (latch.total, latch.shown));
        let writes = pose
            .supply_child_visibility
            .iter()
            .map(|op| op.name.clone())
            .collect();
        // Source warehouse and dock use SUP. Count actual retained mesh
        // children through the production HLOD visibility operation.
        let prefix = state
            .supply_bones
            .as_ref()
            .map_or("SUP", |latch| latch.prefix.as_str());
        let mut visible: Vec<_> = pose
            .model
            .meshes
            .iter()
            .enumerate()
            .filter(|(_, mesh)| {
                mesh.name
                    .split_at_checked(prefix.len())
                    .is_some_and(|(base, suffix)| {
                        base.eq_ignore_ascii_case(prefix)
                            && suffix.len() == 2
                            && suffix.parse::<u8>().is_ok()
                    })
            })
            .filter(|(index, _)| {
                pose.model
                    .mesh_visible_for_authored_subobject_directives(*index, &pose.supply_child_visibility)
            })
            .map(|(_, mesh)| mesh.name.to_ascii_uppercase())
            .collect();
        visible.sort();
        Some((latch, visible, writes))
    }
}
