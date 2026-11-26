//! Borrowed constructor/setter facts, shared by live callbacks and display.
//! This view owns no state and never captures a presentation frame.
use super::*;

/// One Object pose mirrored to GameClient, including facts that can change
/// without an Object transform setter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MirroredDrawablePose {
    revision: u64,
    position: [u32; 3],
    orientation: u32,
    scale: u32,
    construction: Option<(u32, u32)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, Player, Team};

    #[test]
    fn display_observer_borrows_the_same_pose_and_visual_table_as_the_mesh_input() {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
            "display-observer.ini".into(),
            "Object DisplayObserver\nKindOf = STRUCTURE\n\
             Body = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
             Draw = W3DModelDraw MainDraw\nDefaultConditionState\nModel = ObserverMesh\nEnd\nEnd\nEnd\n".into(),
        )]).unwrap()).unwrap();
        let id = world.create_object_for_player("DisplayObserver", 0, glam::Vec3::ZERO).unwrap();
        let mut frame = crate::presentation_frame::PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
        let index = frame.direct_host_drawables.iter().position(|entry| entry.object.id == id).unwrap();
        let models = frame.direct_host_drawables[index].object.receiving_draw_models.clone().unwrap();

        for (template, visual_scale, base_scale) in [
            ("VisualOverride", 0.75, 1.25),
            ("VisualOverride", f32::NAN, 1.25),
            ("", 0.75, f32::NAN),
        ] {
            let entry = &mut frame.direct_host_drawables[index];
            entry.visual_template_name = template.to_owned();
            entry.receiving_visual_draw_models = Some(models.clone());
            entry.visual_mesh_scale = visual_scale;
            entry.object.mesh_scale = base_scale;
            entry.object.under_construction = true;
            entry.object.construction_percent = 0.37;
            entry.object.max_height_above_position = 23.0;
            entry.object.float_yaw = 0.55;
            entry.object.float_pitch = 0.66;

            let old = frame.native_resident_drawable_inputs().into_iter().find(|unit| unit.id == id).unwrap();
            let old = NativeDrawSource::display(&old);
            let new = NativeDrawSource::display_resident(&frame.direct_host_drawables[index]);
            assert_eq!(new.identity, old.identity);
            assert!(std::ptr::eq(new.receiving.unwrap(), old.receiving.unwrap()));
            assert_eq!(new.template, old.template);
            assert_eq!(new.mesh_scale, old.mesh_scale);
            assert_eq!(new.float_yaw, old.float_yaw);
            assert_eq!(new.float_pitch, old.float_pitch);
            assert_eq!(new.under_construction, old.under_construction);
            assert_eq!(new.construction_percent, old.construction_percent);
            assert_eq!(new.max_height_above_position, old.max_height_above_position);
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct NativeDrawSource<'a> {
    pub identity: Option<&'a crate::assets::LiveDrawPlaybackIdentity>,
    pub receiving: Option<&'a crate::game_logic::thing::ReceivingDrawModels>,
    pub template: &'a str,
    pub mesh_scale: f32,
    pub float_yaw: f32,
    pub float_pitch: f32,
    pub under_construction: bool,
    pub construction_percent: f32,
    pub max_height_above_position: f32,
}

impl<'a> NativeDrawSource<'a> {
    pub(super) fn mirrored_pose(
        self,
        sync: &game_client::core::game_client::PresentationDrawableSync,
    ) -> Option<(&'a crate::assets::LiveDrawPlaybackIdentity, MirroredDrawablePose)> {
        let identity = self.identity?;
        let (revision, _) = identity.native_pose()?;
        Some((
            identity,
            MirroredDrawablePose {
                revision,
                position: sync.position.map(f32::to_bits),
                orientation: sync.orientation.to_bits(),
                scale: self.mesh_scale.to_bits(),
                construction: self.under_construction.then_some((
                    self.construction_percent.to_bits(),
                    self.max_height_above_position.to_bits(),
                )),
            },
        ))
    }

    pub(super) fn auxiliary(
        identity: &'a crate::assets::LiveDrawPlaybackIdentity,
        receiving: &'a crate::game_logic::thing::ReceivingDrawModels,
        template: &'a str,
        mesh_scale: f32,
    ) -> Self {
        Self {
            identity: Some(identity),
            receiving: Some(receiving),
            template,
            mesh_scale,
            float_yaw: 0.0,
            float_pitch: 0.0,
            under_construction: false,
            construction_percent: 0.0,
            max_height_above_position: 0.0,
        }
    }

    #[cfg(test)]
    pub(super) fn display(source: &'a crate::presentation_frame::UnitRenderInput) -> Self {
        Self {
            identity: source.draw_playback_identity.as_ref(),
            receiving: source.receiving_draw_models.as_deref(),
            template: &source.template_name,
            mesh_scale: source.mesh_scale,
            float_yaw: source.float_yaw,
            float_pitch: source.float_pitch,
            under_construction: source.under_construction,
            construction_percent: source.construction_percent,
            max_height_above_position: source.max_height_above_position,
        }
    }

    /// Use the completed Drawable record directly for the native setter
    /// observer. Its pose does not require selecting or copying mesh models.
    pub(crate) fn display_resident(
        source: &'a crate::presentation_frame::PresentationDirectHostDrawable,
    ) -> Self {
        let object = &source.object;
        let use_visual_template = !source.visual_template_name.trim().is_empty();
        let (template, receiving, scale) = if use_visual_template {
            (
                &source.visual_template_name,
                source.receiving_visual_draw_models.as_deref(),
                source.visual_mesh_scale,
            )
        } else {
            (&object.template_name, object.receiving_draw_models.as_deref(), object.mesh_scale)
        };
        let mesh_scale = if scale.is_finite() {
            scale
        } else if object.mesh_scale.is_finite() {
            object.mesh_scale
        } else {
            1.0
        };
        Self {
            identity: object.draw_playback_identity.as_ref(),
            receiving,
            template,
            mesh_scale,
            float_yaw: object.float_yaw,
            float_pitch: object.float_pitch,
            under_construction: object.under_construction,
            construction_percent: object.construction_percent,
            max_height_above_position: object.max_height_above_position,
        }
    }

    pub(super) fn current(source: &'a crate::game_logic::NativeSlaveSource) -> Self {
        Self {
            identity: Some(&source.identity),
            receiving: Some(&source.receiving_draw_models),
            template: &source.template_name,
            mesh_scale: source.mesh_scale,
            float_yaw: source.float_yaw,
            float_pitch: source.float_pitch,
            under_construction: source.under_construction,
            construction_percent: source.construction_percent,
            max_height_above_position: source.max_height_above_position,
        }
    }

    pub(super) fn install_observed_transform(
        self,
        state: &mut ObjectVisualState,
        transform: game_client::drawable::Matrix4,
    ) {
        if let Some(identity) = self.identity {
            state.receive_native_installed_transform(
                identity,
                Mat4::from_cols_array_2d(&transform.elements).transpose(),
                self.mesh_scale,
                self.float_yaw,
                self.float_pitch,
                self.under_construction,
                self.construction_percent,
                self.max_height_above_position,
            );
        }
    }
    pub(super) fn install_transform(self, state: &mut ObjectVisualState) {
        if let Some(identity) = self.identity {
            state.receive_native_transform_observations(
                identity,
                self.mesh_scale,
                self.float_yaw,
                self.float_pitch,
                self.under_construction,
                self.construction_percent,
                self.max_height_above_position,
            );
        }
    }
}
