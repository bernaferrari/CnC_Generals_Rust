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
            Self::LoopPingPong => 5,
        };
        xfer.xfer_u8(&mut value)?;
        *self = match value {
            0 => Self::Manual,
            1 => Self::Loop,
            2 => Self::Once,
            3 => Self::LoopBackwards,
            4 => Self::OnceBackwards,
            5 => Self::LoopPingPong,
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

#[cfg(test)]
mod animation_mode_tests {
    use super::*;
    use crate::save_load::{XferLoad, XferSave};
    use std::io::Cursor;

    #[test]
    fn client_drawable_animation_modes_preserve_wire_values_and_append_pingpong() {
        use ClientDrawableAnimationMode::*;
        for (wire, mut mode) in [
            Manual,
            Loop,
            Once,
            LoopBackwards,
            OnceBackwards,
            LoopPingPong,
        ]
        .into_iter()
        .enumerate()
        {
            let expected = mode.clone();
            let mut bytes = Cursor::new(Vec::new());
            mode.xfer(&mut XferSave::new(&mut bytes)).unwrap();
            assert_eq!(bytes.get_ref(), &[wire as u8]);
            let mut restored = Manual;
            restored
                .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
                .unwrap();
            assert_eq!(restored, expected);
        }
        for wire in [6u8, 255] {
            let mut mode = Manual;
            assert!(
                mode.xfer(&mut XferLoad::new(Cursor::new(vec![wire])))
                    .is_err()
            );
            assert_eq!(mode, Manual);
        }
    }
}
