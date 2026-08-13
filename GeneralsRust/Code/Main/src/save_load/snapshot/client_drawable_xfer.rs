//! Direct-Xfer wire layout for renderer-owned client Drawable snapshot DTOs.
//!
//! The DTO definitions deliberately remain renderer-facing serde records in
//! `client_drawable.rs`.  This sibling owns only the positional Common Xfer
//! layout so graphics never needs to depend on the save/Xfer API.

use super::xfer_helpers::{xfer_option, xfer_vec_default};
use super::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData};

impl XferData for ClientDrawableWorldSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ClientDrawableWorldSnapshot")?;
        xfer.xfer_marker_label("Drawables")?;
        xfer_vec_default(
            xfer,
            &mut self.drawables,
            ClientDrawableStateSnapshot::default(),
        )
    }
}

impl XferData for ClientDrawableStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ClientDrawableStateSnapshot")?;
        xfer.xfer_marker_label("ObjectId")?;
        xfer.xfer_u32(&mut self.object_id)?;
        xfer.xfer_marker_label("DrawModuleIndex")?;
        xfer.xfer_u32(&mut self.draw_module_index)?;
        xfer.xfer_marker_label("SourceTemplateName")?;
        self.source_template_name.xfer(xfer)?;
        xfer.xfer_marker_label("ModelKey")?;
        self.model_key.xfer(xfer)?;
        xfer.xfer_marker_label("SelectedConditionStateIndex")?;
        xfer.xfer_u32(&mut self.selected_condition_state_index)?;
        xfer.xfer_marker_label("Animation")?;
        xfer_option(
            xfer,
            &mut self.animation,
            ClientDrawableAnimationSnapshot::default(),
        )?;
        xfer.xfer_marker_label("LastSeenWeaponDischargeSequence")?;
        xfer.xfer_u64(&mut self.last_seen_weapon_discharge_sequence)?;
        for (slot, recoil) in self.recoil_slots.iter_mut().enumerate() {
            xfer.xfer_marker_label(match slot {
                0 => "PrimaryRecoil",
                1 => "SecondaryRecoil",
                _ => "TertiaryRecoil",
            })?;
            xfer_vec_default(xfer, recoil, ClientDrawableRecoilSnapshot::default())?;
        }
        Ok(())
    }
}

impl XferData for ClientDrawableAnimationSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ClientDrawableAnimationSnapshot")?;
        xfer.xfer_marker_label("HierarchyAnimation")?;
        self.hierarchy_animation.xfer(xfer)?;
        xfer.xfer_marker_label("Frame")?;
        xfer.xfer_f32(&mut self.frame)?;
        xfer.xfer_marker_label("Mode")?;
        self.mode.xfer(xfer)
    }
}

impl XferData for ClientDrawableAnimationMode {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            Self::Manual => 0u8,
            Self::Loop => 1,
            Self::Once => 2,
            Self::LoopBackwards => 3,
            Self::OnceBackwards => 4,
        };
        xfer.xfer_u8(&mut value)?;
        *self = match value {
            0 => Self::Manual,
            1 => Self::Loop,
            2 => Self::Once,
            3 => Self::LoopBackwards,
            4 => Self::OnceBackwards,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid ClientDrawableAnimationMode value in snapshot: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for ClientDrawableRecoilSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ClientDrawableRecoilSnapshot")?;
        xfer.xfer_marker_label("Phase")?;
        self.phase.xfer(xfer)?;
        xfer.xfer_marker_label("Shift")?;
        xfer.xfer_f32(&mut self.shift)?;
        xfer.xfer_marker_label("RecoilRate")?;
        xfer.xfer_f32(&mut self.recoil_rate)?;
        Ok(())
    }
}

impl XferData for ClientDrawableRecoilPhase {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            Self::Idle => 0u8,
            Self::RecoilStart => 1,
            Self::Recoil => 2,
            Self::Settle => 3,
        };
        xfer.xfer_u8(&mut value)?;
        *self = match value {
            0 => Self::Idle,
            1 => Self::RecoilStart,
            2 => Self::Recoil,
            3 => Self::Settle,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid ClientDrawableRecoilPhase value in snapshot: {other}"
                )));
            }
        };
        Ok(())
    }
}
