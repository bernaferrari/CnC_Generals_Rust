//! Receiving attachment resolution at the original particle preparation phase.
use super::*;

impl ParticleSystem {
    /// C++ ParticleSys.cpp:1852-1905 — follow drawable/object or self-destroy.
    #[cfg(test)]
    pub(super) fn resolve_attached_parent(&mut self, local_player_index: i32) {
        self.resolve_attached_parent_with_drawable_poses(local_player_index, None);
    }

    pub(super) fn resolve_attached_parent_with_drawable_poses(
        &mut self,
        local_player_index: i32,
        drawable_pose: Option<&dyn Fn(DrawableId) -> Option<DrawableParticlePose>>,
    ) {
        self.resolve_attached_parent_with_object_liveness(local_player_index, drawable_pose, None);
    }
    pub(super) fn resolve_attached_parent_with_object_liveness(
        &mut self,
        local_player_index: i32,
        drawable_pose: Option<&dyn Fn(DrawableId) -> Option<DrawableParticlePose>>,
        object_is_live: Option<&dyn Fn(u32) -> bool>,
    ) {
        if self.attached_drawable_id.is_valid() {
            if let Some(resolve) = drawable_pose {
                if let Some(pose) = resolve(self.attached_drawable_id) {
                    self.last_position = self.position;
                    let (rotation, translation) =
                        affine_from_glam_cols(pose.transform.to_cols_array());
                    self.parent_transform = Some(rotation);
                    self.position = Vec3::from(translation);
                    self.is_shrouded = pose.is_shrouded;
                } else {
                    self.attached_drawable_id = DrawableId::INVALID;
                    self.destroy();
                }
                return;
            }
            if let Some(client) = gamelogic::helpers::TheGameClient::get() {
                if let Some(state) = client.find_drawable_by_id(self.attached_drawable_id.0) {
                    self.last_position = self.position;
                    if let Some(draw) = state.drawable.as_ref() {
                        if let Ok(guard) = draw.read() {
                            let (rot, trans) =
                                affine_from_glam_cols(guard.get_transform_matrix().to_cols_array());
                            self.parent_transform = Some(rot);
                            self.position = Vec3::from(trans);
                        } else {
                            self.apply_yaw_parent_pose(
                                state.position.x,
                                state.position.y,
                                state.position.z,
                                state.orientation,
                            );
                        }
                    } else {
                        self.apply_yaw_parent_pose(
                            state.position.x,
                            state.position.y,
                            state.position.z,
                            state.orientation,
                        );
                    }
                    self.is_shrouded = drawable_state_is_shrouded(&state, local_player_index);
                    return;
                }
            }
            self.attached_drawable_id = DrawableId::INVALID;
            self.destroy();
            return;
        }

        if self.attached_object_id != 0 {
            // Native existence comes from the driving World, not a peer's
            // published pose or compatibility registry. Delay/wind already ran.
            if object_is_live.is_some_and(|exists| !exists(self.attached_object_id)) {
                self.attached_object_id = 0;
                self.destroy();
                return;
            }
            // Live host IDs are not leftover OBJECT_REGISTRY entries.
            // C++ ParticleSys.cpp:1880-1905 follows TheGameLogic object; on
            // the production host path that is the published host pose / live
            // drawable, not leftover find_object.
            if let Some(pose) = gamelogic::helpers::host_fx_object_pose(self.attached_object_id)
                .or_else(|| {
                    crate::core::game_client::query_live_drawable_fx_pose(self.attached_object_id)
                })
            {
                self.apply_host_fx_object_pose(&pose);
                return;
            }
            if let Some(object) =
                gamelogic::helpers::TheGameLogic::find_object_by_id(self.attached_object_id)
            {
                if let Ok(guard) = object.read() {
                    use gamelogic::common::types::ObjectShroudStatus;
                    let status = guard.get_shrouded_status(local_player_index);
                    self.is_shrouded = (status as u8) >= (ObjectShroudStatus::Fogged as u8);
                    self.last_position = self.position;
                    if let Some(draw) = guard.get_drawable() {
                        if let Ok(draw_guard) = draw.read() {
                            let (rot, trans) = affine_from_glam_cols(
                                draw_guard.get_transform_matrix().to_cols_array(),
                            );
                            self.parent_transform = Some(rot);
                            self.position = Vec3::from(trans);
                        } else {
                            let (rot, trans) =
                                affine_from_glam_cols(guard.get_transform_matrix().to_cols_array());
                            self.parent_transform = Some(rot);
                            self.position = Vec3::from(trans);
                        }
                    } else {
                        let (rot, trans) =
                            affine_from_glam_cols(guard.get_transform_matrix().to_cols_array());
                        self.parent_transform = Some(rot);
                        self.position = Vec3::from(trans);
                    }
                }
                return;
            }
            self.attached_object_id = 0;
            self.destroy();
        }
    }

    fn apply_yaw_parent_pose(&mut self, x: f32, y: f32, z: f32, yaw: f32) {
        self.position = Vec3::new(x, y, z);
        let (s, c) = yaw.sin_cos();
        self.parent_transform = Some(Mat3::from_cols_array(&[
            c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0,
        ]));
    }

    fn apply_host_fx_object_pose(&mut self, pose: &gamelogic::helpers::HostFxObjectPose) {
        self.last_position = self.position;
        let (rot, trans) = affine_from_glam_cols(pose.transform.to_cols_array());
        self.parent_transform = Some(rot);
        self.position = Vec3::from(trans);
        // C++ ParticleSys.cpp:1880-1905 object-attach Fogged+ emit gate.
        self.is_shrouded = pose.is_shrouded;
    }
}
