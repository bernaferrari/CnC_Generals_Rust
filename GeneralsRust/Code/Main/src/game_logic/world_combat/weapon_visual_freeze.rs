use super::super::*;
use super::weapon_visual_capture::{
    PendingWeaponVisualDispatchCapture, geometry_center, recoil_dir_from_positions,
};
use crate::presentation_frame::{
    FrozenWeaponVisualDischarge, FrozenWeaponVisualDispatchPlan, FrozenWeaponVisualImpulse,
    FrozenWeaponVisualModuleProbe, freeze_weapon_visual_dispatch_plan,
};

impl GameLogic {
    pub(in super::super) fn stamp_visual_object_generation(&mut self, source: ObjectId) -> u64 {
        let Some(object) = self.objects.get_mut(&source) else {
            return 0;
        };
        if object.visual_object_generation == 0 {
            let generation = self.next_visual_object_generation.max(1);
            object.visual_object_generation = generation;
            self.next_visual_object_generation = generation.saturating_add(1).max(1);
        }
        object.visual_object_generation
    }

    pub(super) fn resolved_visual_target_pos(
        &self,
        capture: &PendingWeaponVisualDispatchCapture,
    ) -> Vec3 {
        if let Some(target_id) = capture.target_id {
            if let Some(victim) = self.objects.get(&target_id) {
                return geometry_center(victim.get_position(), &victim.thing.geometry);
            }
        }
        capture
            .target_pos
            .map(|pos| Vec3::new(pos[0], pos[1], pos[2]))
            .unwrap_or(Vec3::new(
                capture.source_pos[0],
                capture.source_pos[1],
                capture.source_pos[2],
            ))
    }

    pub(in super::super) fn freeze_pending_weapon_visual_plan(
        &self,
        source: ObjectId,
        capture: &PendingWeaponVisualDispatchCapture,
        sequence: u64,
        source_object_generation: u64,
        locally_controlled: bool,
    ) -> Option<FrozenWeaponVisualDispatchPlan> {
        let target_pos = self.resolved_visual_target_pos(capture);
        let source_pos = Vec3::new(
            capture.source_pos[0],
            capture.source_pos[1],
            capture.source_pos[2],
        );
        let impulse = FrozenWeaponVisualImpulse {
            recoil_amount: capture.recoil_amount,
            recoil_dir: recoil_dir_from_positions(source_pos, target_pos, capture.recoil_amount),
            source_orientation: capture.source_orientation,
            target_pos: [target_pos.x, target_pos.y, target_pos.z],
            is_contact_weapon: capture.is_contact_weapon,
        };
        let discharge = FrozenWeaponVisualDischarge {
            world_epoch: self.visual_world_epoch,
            source,
            source_object_generation,
            weapon_slot: capture.weapon_slot,
            fired_barrel: capture.fired_barrel,
            sequence,
            logic_frame: capture.logic_frame,
        };
        let mut plan = freeze_weapon_visual_dispatch_plan(
            discharge,
            capture.source_gate(locally_controlled),
            probe_cached_draw_modules(capture),
        )
        .ok()
        .flatten()?;
        if !plan.is_valid() {
            return None;
        }
        plan.impulse = impulse;
        Some(plan)
    }
}

fn probe_cached_draw_modules(
    capture: &PendingWeaponVisualDispatchCapture,
) -> Vec<FrozenWeaponVisualModuleProbe> {
    let Some(manager_arc) = crate::assets::get_asset_manager() else {
        return Vec::new();
    };
    let Ok(mut manager) = manager_arc.try_lock() else {
        return Vec::new();
    };
    let selected = manager
        .select_draw_models_for_object_conditions(
            &capture.template_name,
            capture.model_condition_bits,
        )
        .unwrap_or_default();
    if let Some(definition) = manager.get_object_definition(&capture.template_name) {
        return definition
            .draw_modules
            .iter()
            .enumerate()
            .filter_map(|(index, module)| {
                let module_index = u32::try_from(index).ok()?;
                Some(probe_declared_draw_module(
                    capture,
                    module_index,
                    &module.declaration,
                    selected
                        .iter()
                        .find(|draw_model| draw_model.module_index == module_index),
                ))
            })
            .collect();
    }
    selected
        .iter()
        .map(|draw_model| probe_one_draw_model(capture, draw_model))
        .collect()
}

pub(super) fn classify_draw_module_declaration(declaration: &str) -> DrawModuleFireFxClass {
    let class_name = declaration.split_whitespace().next().unwrap_or(declaration);
    if W3D_MODEL_DRAW_CLASSES
        .iter()
        .any(|name| class_name.eq_ignore_ascii_case(name))
    {
        DrawModuleFireFxClass::W3dModelDraw
    } else if KNOWN_NO_OBJECT_DRAW_INTERFACE
        .iter()
        .any(|name| class_name.eq_ignore_ascii_case(name))
    {
        DrawModuleFireFxClass::KnownNoWeaponFireFx
    } else {
        DrawModuleFireFxClass::OpaqueObjectDrawInterface
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DrawModuleFireFxClass {
    W3dModelDraw,
    KnownNoWeaponFireFx,
    OpaqueObjectDrawInterface,
}

const W3D_MODEL_DRAW_CLASSES: &[&str] = &[
    "W3DModelDraw",
    "W3DTankDraw",
    "W3DTruckDraw",
    "W3DTankTruckDraw",
    "W3DOverlordTankDraw",
    "W3DOverlordTruckDraw",
    "W3DOverlordAircraftDraw",
    "W3DScienceModelDraw",
    "W3DDependencyModelDraw",
    "W3DPoliceCarDraw",
];

const KNOWN_NO_OBJECT_DRAW_INTERFACE: &[&str] = &[
    "W3DDefaultDraw",
    "W3DLaserDraw",
    "W3DTracerDraw",
    "W3DDebrisDraw",
    "W3DRopeDraw",
    "W3DTreeDraw",
    "W3DPropDraw",
    "W3DProjectileStreamDraw",
];

fn probe_declared_draw_module(
    capture: &PendingWeaponVisualDispatchCapture,
    module_index: u32,
    declaration: &str,
    selected: Option<&crate::assets::AuthoredDrawModel>,
) -> FrozenWeaponVisualModuleProbe {
    match classify_draw_module_declaration(declaration) {
        DrawModuleFireFxClass::KnownNoWeaponFireFx => {
            FrozenWeaponVisualModuleProbe::KnownNoWeaponFireFx {
                draw_module_index: module_index,
            }
        }
        DrawModuleFireFxClass::OpaqueObjectDrawInterface => {
            FrozenWeaponVisualModuleProbe::OpaqueObjectDrawInterface {
                draw_module_index: module_index,
            }
        }
        DrawModuleFireFxClass::W3dModelDraw => match selected {
            Some(draw_model) => probe_one_draw_model(capture, draw_model),
            None => FrozenWeaponVisualModuleProbe::KnownNoWeaponFireFx {
                draw_module_index: module_index,
            },
        },
    }
}

fn probe_one_draw_model(
    capture: &PendingWeaponVisualDispatchCapture,
    draw_model: &crate::assets::AuthoredDrawModel,
) -> FrozenWeaponVisualModuleProbe {
    let unknown = || FrozenWeaponVisualModuleProbe::KnownNoWeaponFireFx {
        draw_module_index: draw_model.module_index,
    };
    if !draw_model.weapon_bone_bindings.source_fields_valid {
        return unknown();
    }
    let Some(slot) = draw_model.weapon_bone_bindings.slot(capture.weapon_slot) else {
        return unknown();
    };
    let named = |bone: &Option<String>| bone.as_deref().is_some_and(|name| !name.trim().is_empty());
    let has_fx = named(&slot.fire_fx_bone_base);
    let has_recoil = named(&slot.recoil_bone_base) || named(&slot.muzzle_flash_bone_base);
    if !has_fx && !has_recoil {
        return unknown();
    }
    FrozenWeaponVisualModuleProbe::W3dModelDraw {
        state: crate::presentation_frame::FrozenWeaponVisualDrawState {
            draw_module_index: draw_model.module_index,
            source_template_name: capture.template_name.clone(),
            model_key: draw_model.model_key.clone(),
            selected_condition_state_index: draw_model.selected_condition_state_index,
            draw_state_revision: capture.draw_state_revision,
        },
        barrel: crate::presentation_frame::FrozenW3dWeaponVisualBarrel {
            fire_fx_will_handle: has_fx,
            starts_recoil_or_muzzle: has_recoil,
        },
    }
}
