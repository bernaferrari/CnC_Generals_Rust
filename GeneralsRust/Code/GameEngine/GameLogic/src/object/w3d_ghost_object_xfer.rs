//! C++ `W3DGhostObject.cpp` Snapshotable xfer. Field order is the wire format.

use super::w3d_ghost_object::{
    INVALID_DRAWABLE_ID, INVALID_OBJECT_ID, Matrix3x4, ParentGeometrySnapshot, RenderObjectClass,
    RenderObjectState, RenderSubObjectSnapshot, W3DDrawableInfo, W3DGhostObject,
    W3DGhostObjectManager, W3DRenderObjectSnapshot,
};
use game_engine::common::game_common::MAX_PLAYER_COUNT;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

const CPP_INVALID_OBJECT_ID: u32 = 0;

fn xfer_err(err: impl ToString) -> String {
    err.to_string()
}

fn xfer_matrix3x4(xfer: &mut dyn Xfer, matrix: &mut Matrix3x4) -> Result<(), String> {
    // C++ `W3DGhostObject.cpp:174` / `:233` `xferUser(&transform, sizeof(Matrix3D))`.
    for row in &mut matrix.rows {
        for value in row {
            xfer.xfer_real(value).map_err(xfer_err)?;
        }
    }
    Ok(())
}

fn xfer_object_id(xfer: &mut dyn Xfer, id: &mut Option<u32>) -> Result<(), String> {
    let mut raw = id.unwrap_or(CPP_INVALID_OBJECT_ID);
    if raw == INVALID_OBJECT_ID {
        raw = CPP_INVALID_OBJECT_ID;
    }
    xfer.xfer_u32(&mut raw).map_err(xfer_err)?;
    *id = (raw != CPP_INVALID_OBJECT_ID).then_some(raw);
    Ok(())
}

impl Snapshotable for W3DRenderObjectSnapshot {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ `W3DGhostObject.cpp:160-256` W3DRenderObjectSnapshot::xfer v1.
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(xfer_err)?;
        xfer_matrix3x4(xfer, &mut self.render_object.transform)?;

        let mut sub_object_count =
            i32::try_from(self.render_object.sub_objects.len()).unwrap_or(i32::MAX);
        xfer.xfer_int(&mut sub_object_count).map_err(xfer_err)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.render_object.sub_objects.clear();
            self.render_object
                .sub_objects
                .reserve(sub_object_count.max(0) as usize);
        }
        for index in 0..sub_object_count.max(0) as usize {
            if xfer.get_xfer_mode() == XferMode::Load {
                self.render_object
                    .sub_objects
                    .push(RenderSubObjectSnapshot {
                        name: String::new(),
                        visible: true,
                        transform: Matrix3x4::IDENTITY,
                    });
            }
            let child = &mut self.render_object.sub_objects[index];
            xfer.xfer_ascii_string(&mut child.name).map_err(xfer_err)?;
            xfer.xfer_bool(&mut child.visible).map_err(xfer_err)?;
            xfer_matrix3x4(xfer, &mut child.transform)?;
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            // C++ recovers class via Create_Render_Obj. Host reconstructs
            // HLod vs Mesh from the persisted child list so UV disable and
            // GhostRenderState materialization stay exact.
            self.render_object.class_id = if self.render_object.sub_objects.is_empty() {
                RenderObjectClass::Mesh
            } else {
                RenderObjectClass::HLod
            };
            self.update(self.render_object.clone());
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for W3DGhostObject {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ `W3DGhostObject.cpp:588-805` then base `GhostObject.cpp:51-103`.
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(xfer_err)?;

        // C++ `GhostObject::xfer` v1 immediately after the W3D version byte.
        let mut base_version: XferVersion = 1;
        xfer.xfer_version(&mut base_version, 1).map_err(xfer_err)?;
        let mut parent_id = self.parent_object_id();
        xfer_object_id(xfer, &mut parent_id)?;
        let mut geometry = self.parent_geometry().unwrap_or(ParentGeometrySnapshot {
            geometry_type: 0,
            is_small: false,
            major_radius: 0.0,
            minor_radius: 0.0,
            position: [0.0; 3],
            angle: 0.0,
        });
        xfer.xfer_u32(&mut geometry.geometry_type)
            .map_err(xfer_err)?;
        xfer.xfer_bool(&mut geometry.is_small).map_err(xfer_err)?;
        xfer.xfer_real(&mut geometry.major_radius)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut geometry.minor_radius)
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut geometry.angle).map_err(xfer_err)?;
        xfer.xfer_real(&mut geometry.position[0])
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut geometry.position[1])
            .map_err(xfer_err)?;
        xfer.xfer_real(&mut geometry.position[2])
            .map_err(xfer_err)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.update_parent_object(parent_id, true);
            self.set_parent_geometry(geometry);
        }

        let mut info = self.drawable_info();
        let mut shroud_id = if info.shroud_status_object_id == INVALID_OBJECT_ID {
            CPP_INVALID_OBJECT_ID
        } else {
            info.shroud_status_object_id
        };
        xfer.xfer_u32(&mut shroud_id).map_err(xfer_err)?;
        xfer.xfer_int(&mut info.flags).map_err(xfer_err)?;
        xfer.xfer_u32(&mut info.drawable_id).map_err(xfer_err)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            info.shroud_status_object_id = if shroud_id == CPP_INVALID_OBJECT_ID {
                INVALID_OBJECT_ID
            } else {
                shroud_id
            };
            if info.drawable_id == CPP_INVALID_OBJECT_ID {
                info.drawable_id = INVALID_DRAWABLE_ID;
            }
            self.set_drawable_info(info);
        }

        for player in 0..MAX_PLAYER_COUNT {
            let mut count = u8::try_from(self.snapshots(player).len()).unwrap_or(u8::MAX);
            xfer.xfer_unsigned_byte(&mut count).map_err(xfer_err)?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for snapshot in self.snapshots(player).iter().take(count as usize) {
                    let mut name = snapshot.render_object.name.clone();
                    let mut scale = snapshot.render_object.scale;
                    let mut color = snapshot.render_object.color;
                    xfer.xfer_ascii_string(&mut name).map_err(xfer_err)?;
                    xfer.xfer_real(&mut scale).map_err(xfer_err)?;
                    xfer.xfer_unsigned_int(&mut color).map_err(xfer_err)?;
                    let mut nested = snapshot.clone();
                    nested.xfer(xfer)?;
                }
            } else {
                let mut loaded = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let mut name = String::new();
                    let mut scale = 1.0f32;
                    let mut color = 0u32;
                    xfer.xfer_ascii_string(&mut name).map_err(xfer_err)?;
                    xfer.xfer_real(&mut scale).map_err(xfer_err)?;
                    xfer.xfer_u32(&mut color).map_err(xfer_err)?;
                    let mut snapshot = W3DRenderObjectSnapshot::new(RenderObjectState {
                        name,
                        scale,
                        color,
                        transform: Matrix3x4::IDENTITY,
                        sub_objects: Vec::new(),
                        class_id: RenderObjectClass::Other,
                    });
                    snapshot.xfer(xfer)?;
                    loaded.push(snapshot);
                }
                self.replace_player_snapshots(player, loaded);
            }
        }

        let mut shroudedness_count = 0u8;
        for player in 0..MAX_PLAYER_COUNT {
            if self.has_snapshot(player) {
                shroudedness_count = shroudedness_count.saturating_add(1);
            }
        }
        xfer.xfer_unsigned_byte(&mut shroudedness_count)
            .map_err(xfer_err)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for player in 0..MAX_PLAYER_COUNT {
                if !self.has_snapshot(player) {
                    continue;
                }
                let mut player_index = u8::try_from(player).unwrap_or(u8::MAX);
                let mut status = i32::from(self.previous_shroudedness(player).unwrap_or(0));
                xfer.xfer_unsigned_byte(&mut player_index)
                    .map_err(xfer_err)?;
                xfer.xfer_int(&mut status).map_err(xfer_err)?;
            }
        } else {
            for _ in 0..shroudedness_count {
                let mut player_index = 0u8;
                let mut status = 0i32;
                xfer.xfer_unsigned_byte(&mut player_index)
                    .map_err(xfer_err)?;
                xfer.xfer_int(&mut status).map_err(xfer_err)?;
                self.set_previous_shroudedness(player_index as usize, status as u8);
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for W3DGhostObjectManager {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ `W3DGhostObject.cpp:1120-1223` + `GhostObject.cpp:181-192`.
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1).map_err(xfer_err)?;
        let mut base_version: XferVersion = 1;
        xfer.xfer_version(&mut base_version, 1).map_err(xfer_err)?;
        let mut local_player = i32::try_from(self.local_player_index()).unwrap_or(0);
        xfer.xfer_int(&mut local_player).map_err(xfer_err)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.set_local_player_index(local_player.max(0) as usize);
        }

        let mut count = u16::try_from(self.used_count()).unwrap_or(u16::MAX);
        xfer.xfer_unsigned_short(&mut count).map_err(xfer_err)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for ghost in self.used() {
                let mut parent = ghost.parent_object_id();
                xfer_object_id(xfer, &mut parent)?;
                let mut nested = ghost.clone();
                nested.xfer(xfer)?;
            }
            return Ok(());
        }

        if self.used_count() != 0 {
            return Err("W3DGhostObjectManager::xfer used list must be empty on load".to_string());
        }
        // C++ `W3DGhostObject.cpp:1172` unlocks so addGhostObject can allocate.
        self.set_save_lock_ghost_objects(false);
        let local_player = self.local_player_index();
        for _ in 0..count {
            let mut parent = None;
            xfer_object_id(xfer, &mut parent)?;
            self.add_ghost_object(parent, parent.is_some())
                .ok_or_else(|| "W3DGhostObjectManager::xfer could not create ghost".to_string())?;
            self.used_mut()[0].xfer(xfer)?;
            self.used_mut()[0].emit_xfer_load_scene_events(local_player);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
