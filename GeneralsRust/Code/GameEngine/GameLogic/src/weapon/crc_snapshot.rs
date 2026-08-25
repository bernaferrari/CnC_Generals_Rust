//! C++ Weapon::crc / xfer snapshot leftover extracted from weapon/mod.rs.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::Coord3D;
use crate::common::LOGICFRAMES_PER_SECOND;
use crate::common::Relationship;
use crate::common::{INVALID_ID, ObjectID, Real, UnsignedInt, Xfer, XferMode, XferVersion};
use crate::common::{KindOf, PathfindLayerEnum};
use crate::common::{Matrix3D, TurretType};
use crate::damage::{DamageType, DeathType};
use crate::effects::{FXList, ObjectCreationList};
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, TheThingFactory, get_game_logic_random_value,
    get_game_logic_random_value_real,
};
use crate::modules::CountermeasuresBehaviorInterface;
use crate::object::collide::GameObject;
use crate::object::drawable::DrawableArcExt;
use crate::object::update::MissileAIUpdateModuleData;
use crate::system::game_logic::TheObjectFactory;
use crate::weapon::projectile_launch_cast::{
    ProjectileLaunchKindMut, module_projectile_launch_kind,
};
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::ini::ini_particle_sys::ParticleSystemTemplate;
use game_engine::common::system::Snapshotable;

use super::helpers::{
    INVALID_OBJECT_ID, ObjectId, dual_world_registry_unavailable, weapon_slot_from_u32,
    weapon_slot_to_u32, weapon_status_from_u32, weapon_status_to_u32,
};
use super::store::with_weapon_store;
use super::weapon_instance::Weapon;

/// C++ `Weapon::crc` payload (Weapon.cpp). Status is commented out in C++.
#[derive(Debug, Clone)]
pub(crate) struct WeaponCrcSnapshot {
    pub template_name: String,
    pub wslot: i32,
    pub ammo_in_clip: u32,
    pub when_we_can_fire_again: u32,
    pub when_pre_attack_finished: u32,
    pub when_last_reload_started: u32,
    pub last_fire_frame: u32,
    pub projectile_stream_id: ObjectId,
    pub max_shot_count: i32,
    pub cur_barrel: i32,
    pub num_shots_for_cur_barrel: i32,
    pub scatter_targets_unused: Vec<i32>,
    pub pitch_limited: bool,
    pub leech_weapon_range_active: bool,
}

impl Weapon {
    pub(crate) fn crc_snapshot_fields(&self) -> WeaponCrcSnapshot {
        WeaponCrcSnapshot {
            template_name: self.template.get_name().to_string(),
            wslot: self.weapon_slot as i32,
            ammo_in_clip: self.ammo_in_clip,
            when_we_can_fire_again: self.when_we_can_fire_again,
            when_pre_attack_finished: self.when_pre_attack_finished,
            when_last_reload_started: self.when_last_reload_started,
            last_fire_frame: self.last_fire_frame,
            projectile_stream_id: self.projectile_stream_id,
            max_shot_count: self.max_shot_count,
            cur_barrel: self.current_barrel,
            num_shots_for_cur_barrel: self.num_shots_for_current_barrel,
            scatter_targets_unused: self.scatter_targets_unused.clone(),
            pitch_limited: self.pitch_limited,
            leech_weapon_range_active: self.leech_weapon_range_active,
        }
    }
}

impl Snapshotable for Weapon {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Weapon::crc: name, slot blob, ammo, fire/reload frames, projectile +
        // unused laser IDs, shot/barrel, scatter list, pitch/leech. No version/status.
        let snap = self.crc_snapshot_fields();
        let mut name = snap.template_name;
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| e.to_string())?;

        let mut wslot = snap.wslot;
        unsafe {
            xfer.xfer_user(
                (&mut wslot as *mut i32).cast::<u8>(),
                std::mem::size_of::<i32>(),
            )
        }
        .map_err(|e| e.to_string())?;

        let mut ammo = snap.ammo_in_clip;
        xfer.xfer_unsigned_int(&mut ammo)
            .map_err(|e| e.to_string())?;
        let mut when_fire = snap.when_we_can_fire_again;
        xfer.xfer_unsigned_int(&mut when_fire)
            .map_err(|e| e.to_string())?;
        let mut when_pre = snap.when_pre_attack_finished;
        xfer.xfer_unsigned_int(&mut when_pre)
            .map_err(|e| e.to_string())?;
        let mut when_reload = snap.when_last_reload_started;
        xfer.xfer_unsigned_int(&mut when_reload)
            .map_err(|e| e.to_string())?;
        let mut last_fire = snap.last_fire_frame;
        xfer.xfer_unsigned_int(&mut last_fire)
            .map_err(|e| e.to_string())?;

        let mut stream_id = snap.projectile_stream_id;
        xfer.xfer_object_id(&mut stream_id)
            .map_err(|e| e.to_string())?;
        let mut laser_id_unused = INVALID_OBJECT_ID;
        xfer.xfer_object_id(&mut laser_id_unused)
            .map_err(|e| e.to_string())?;

        let mut max_shots = snap.max_shot_count;
        xfer.xfer_int(&mut max_shots).map_err(|e| e.to_string())?;
        let mut cur_barrel = snap.cur_barrel;
        xfer.xfer_int(&mut cur_barrel).map_err(|e| e.to_string())?;
        let mut shots_for_barrel = snap.num_shots_for_cur_barrel;
        xfer.xfer_int(&mut shots_for_barrel)
            .map_err(|e| e.to_string())?;

        let mut scatter_count = snap.scatter_targets_unused.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut scatter_count)
            .map_err(|e| e.to_string())?;
        for target in &snap.scatter_targets_unused {
            let mut int_data = *target;
            xfer.xfer_int(&mut int_data).map_err(|e| e.to_string())?;
        }

        let mut pitch_limited = snap.pitch_limited;
        xfer.xfer_bool(&mut pitch_limited)
            .map_err(|e| e.to_string())?;
        let mut leech = snap.leech_weapon_range_active;
        xfer.xfer_bool(&mut leech).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            let mut template_name = self.template.get_name().to_string();
            xfer.xfer_ascii_string(&mut template_name)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                let template =
                    with_weapon_store(|store| store.find_weapon_template(&template_name).cloned())
                        .map_err(|e| e.to_string())?;
                let template = template
                    .ok_or_else(|| format!("Weapon::xfer missing template {}", template_name))?;
                self.template = template;
            }
        }

        let mut slot = weapon_slot_to_u32(self.weapon_slot);
        xfer.xfer_unsigned_int(&mut slot)
            .map_err(|e| e.to_string())?;
        self.weapon_slot = weapon_slot_from_u32(slot);

        let mut status = weapon_status_to_u32(self.status);
        xfer.xfer_unsigned_int(&mut status)
            .map_err(|e| e.to_string())?;
        self.status = weapon_status_from_u32(status);

        xfer.xfer_unsigned_int(&mut self.ammo_in_clip)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.when_we_can_fire_again)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.when_pre_attack_finished)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.when_last_reload_started)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.last_fire_frame)
            .map_err(|e| e.to_string())?;

        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.suspend_fx_frame)
                .map_err(|e| e.to_string())?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.suspend_fx_frame = 0;
        }

        xfer.xfer_object_id(&mut self.projectile_stream_id)
            .map_err(|e| e.to_string())?;

        let mut unused_laser_id = INVALID_OBJECT_ID;
        xfer.xfer_object_id(&mut unused_laser_id)
            .map_err(|e| e.to_string())?;

        xfer.xfer_int(&mut self.max_shot_count)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.current_barrel)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.num_shots_for_current_barrel)
            .map_err(|e| e.to_string())?;

        let mut scatter_count = self.scatter_targets_unused.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut scatter_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.scatter_targets_unused.clear();
            for _ in 0..scatter_count {
                let mut value = 0;
                xfer.xfer_int(&mut value).map_err(|e| e.to_string())?;
                self.scatter_targets_unused.push(value);
            }
        } else {
            for &entry in self
                .scatter_targets_unused
                .iter()
                .take(scatter_count as usize)
            {
                let mut value = entry;
                xfer.xfer_int(&mut value).map_err(|e| e.to_string())?;
            }
        }

        xfer.xfer_bool(&mut self.pitch_limited)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.leech_weapon_range_active)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Wave 265: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if self.projectile_stream_id != INVALID_OBJECT_ID {
            // Existence probe via borrow-first helper (no Arc kept).
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(self.projectile_stream_id, |_| ())
                .is_none()
            {
                self.projectile_stream_id = INVALID_OBJECT_ID;
            }
        }
        Ok(())
    }
}
