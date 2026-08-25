//! C++ `GameState` `CHUNK_Radar` over `TheRadar` (Radar.cpp:1455-1524).

use super::{MAX_RADAR_EVENTS, RadarEventType, RadarObject, RadarSystem};
use crate::System::xfer::{
    RGBAColorInt as XferRgba, Snapshot, Xfer, XferMode, XferStatus, XferVersion,
};
use crate::System::{SnapshotType, get_game_state};

/// C++ `sizeof(RadarEventType)` — MSVC enum is 4 bytes.
const RADAR_EVENT_TYPE_SIZE: usize = 4;

pub struct RadarSnapshot;

impl RadarSnapshot {
    fn with_radar<R>(f: impl FnOnce(&mut RadarSystem) -> R) -> Result<R, XferStatus> {
        let radar = super::get_radar_system();
        let mut guard = radar.write().map_err(|_| XferStatus::InvalidData)?;
        Ok(f(&mut guard))
    }
}

impl Snapshot for RadarSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        Self::with_radar(|radar| radar.xfer_cpp_chunk(xfer))?
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Self::with_radar(|radar| {
            radar.terrain_dirty = true;
            radar.refresh_terrain();
            Ok(())
        })?
    }
}

impl RadarSystem {
    /// C++ `Radar::xfer` version 1 layout.
    pub fn xfer_cpp_chunk(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;

        xfer.xfer_bool(&mut self.radar_hidden)?;
        xfer.xfer_bool(&mut self.radar_force_on)?;

        xfer_object_list(xfer, &mut self.local_object_list)?;
        xfer_object_list(xfer, &mut self.object_list)?;

        let mut event_count = MAX_RADAR_EVENTS as u16;
        xfer.xfer_unsigned_short(&mut event_count)?;
        if event_count != MAX_RADAR_EVENTS as u16 {
            return Err(XferStatus::InvalidData);
        }

        for event in &mut self.events {
            let mut type_bits = event.event_type as i32;
            // SAFETY: C++ writes `sizeof(RadarEventType)` raw enum bytes.
            unsafe {
                xfer.xfer_user(
                    (&mut type_bits as *mut i32).cast::<u8>(),
                    RADAR_EVENT_TYPE_SIZE,
                )?;
            }
            event.event_type = match type_bits {
                1 => RadarEventType::Construction,
                2 => RadarEventType::Upgrade,
                3 => RadarEventType::UnderAttack,
                4 => RadarEventType::Information,
                5 => RadarEventType::BeaconPulse,
                6 => RadarEventType::Infiltration,
                7 => RadarEventType::BattlePlan,
                8 => RadarEventType::StealthDiscovered,
                9 => RadarEventType::StealthNeutralized,
                10 => RadarEventType::Fake,
                _ => RadarEventType::Invalid,
            };

            xfer.xfer_bool(&mut event.active)?;
            xfer.xfer_unsigned_int(&mut event.create_frame)?;
            xfer.xfer_unsigned_int(&mut event.die_frame)?;
            xfer.xfer_unsigned_int(&mut event.fade_frame)?;

            let mut c1 = XferRgba {
                red: event.color1.r as u32,
                green: event.color1.g as u32,
                blue: event.color1.b as u32,
                alpha: event.color1.a as u32,
            };
            xfer.xfer_rgba_color_int(&mut c1)?;
            event.color1 = super::RGBAColorInt::new(
                c1.red.min(255) as u8,
                c1.green.min(255) as u8,
                c1.blue.min(255) as u8,
                c1.alpha.min(255) as u8,
            );

            let mut c2 = XferRgba {
                red: event.color2.r as u32,
                green: event.color2.g as u32,
                blue: event.color2.b as u32,
                alpha: event.color2.a as u32,
            };
            xfer.xfer_rgba_color_int(&mut c2)?;
            event.color2 = super::RGBAColorInt::new(
                c2.red.min(255) as u8,
                c2.green.min(255) as u8,
                c2.blue.min(255) as u8,
                c2.alpha.min(255) as u8,
            );

            let mut world = crate::System::xfer::Coord3D {
                x: event.world_loc.x,
                y: event.world_loc.y,
                z: event.world_loc.z,
            };
            xfer.xfer_coord_3d(&mut world)?;
            event.world_loc = super::Coord3D {
                x: world.x,
                y: world.y,
                z: world.z,
            };

            let mut radar_loc = crate::System::xfer::ICoord2D {
                x: event.radar_loc.x,
                y: event.radar_loc.y,
            };
            xfer.xfer_icoord_2d(&mut radar_loc)?;
            event.radar_loc = super::ICoord2D {
                x: radar_loc.x,
                y: radar_loc.y,
            };

            xfer.xfer_bool(&mut event.sound_played)?;
        }

        let mut next_free = self.next_free_event as i32;
        xfer.xfer_int(&mut next_free)?;
        self.next_free_event = next_free.max(0) as usize;

        let mut last_event = self.last_event.map(|i| i as i32).unwrap_or(-1);
        xfer.xfer_int(&mut last_event)?;
        self.last_event = if last_event >= 0 {
            Some(last_event as usize)
        } else {
            None
        };

        Ok(())
    }
}

fn xfer_object_list(
    xfer: &mut dyn Xfer,
    object_list: &mut Vec<RadarObject>,
) -> Result<(), XferStatus> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)?;

    let mut count = object_list.len() as u16;
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for obj in object_list.iter_mut() {
                xfer_radar_object(xfer, obj)?;
            }
        }
        XferMode::Load => {
            object_list.clear();
            for _ in 0..count {
                let mut obj = RadarObject::new(0);
                xfer_radar_object(xfer, &mut obj)?;
                object_list.push(obj);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }
    Ok(())
}

fn xfer_radar_object(xfer: &mut dyn Xfer, obj: &mut RadarObject) -> Result<(), XferStatus> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)?;
    xfer.xfer_object_id(&mut obj.object_id)?;
    xfer.xfer_unsigned_int(&mut obj.color)?;
    Ok(())
}

/// Replace any InGameUI ping block with `TheRadar`. Safe to call every frame.
pub fn register_the_radar_snapshot_block() {
    let mut state = get_game_state();
    state.add_snapshot_block(
        "CHUNK_Radar".to_string(),
        Box::new(RadarSnapshot),
        SnapshotType::SaveLoad,
    );
}

pub fn ensure_the_radar_snapshot_block() {
    register_the_radar_snapshot_block();
}
