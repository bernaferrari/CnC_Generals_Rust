//! MinefieldBehavior - Rust conversion of C++ MinefieldBehavior.
//!
//! Handles landmine virtual mine counts, regeneration, collision detonation,
//! mine-clearer immunity, scooting after placement, damage synchronization, and
//! save/load state.

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, KindOf, ModuleData, ObjectID, ObjectStatusMaskType,
    ObjectStatusTypes, PathfindLayerEnum, Real, Relationship, TheGameLogic, UnsignedInt,
    XferVersion, INVALID_ID, MODELCONDITION_RUBBLE,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::effects::ObjectCreationList;
use crate::helpers::{TheObjectCreationListStore, TheTerrainLogic, TheWeaponStore};
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, DamageModuleInterface, DieModuleInterface,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{
    xfer_update_module_base_state, BehaviorModuleData, LandMineInterface,
};
use crate::object::Object as GameObject;
use crate::weapon::{with_weapon_store, WeaponTemplate};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType,
};
use std::sync::{Arc, RwLock, Weak};

const MIN_HEALTH: Real = 0.1;
const LOGICFRAMES_PER_SECOND: UnsignedInt = 30;
const MAX_IMMUNITY: usize = 3;
const OBJECT_STATUS_NO_ATTACK_FROM_AI: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::NoAttackFromAi);
const OBJECT_STATUS_MASKED: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::Masked);

#[derive(Clone, Debug)]
pub struct MinefieldBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub detonation_weapon: Option<Arc<WeaponTemplate>>,
    pub detonated_by: Int,
    pub stops_regen_after_creator_dies: Bool,
    pub regenerates: Bool,
    pub workers_detonate: Bool,
    pub creator_death_check_rate: UnsignedInt,
    pub scoot_from_starting_point_time: UnsignedInt,
    pub num_virtual_mines: UnsignedInt,
    pub repeat_detonate_move_thresh: Real,
    pub health_percent_to_drain_per_second: Real,
    pub ocl: Option<Arc<ObjectCreationList>>,
}

impl Default for MinefieldBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            detonation_weapon: None,
            detonated_by: relationship_mask(Relationship::Enemies)
                | relationship_mask(Relationship::Neutral),
            stops_regen_after_creator_dies: true,
            regenerates: false,
            workers_detonate: false,
            creator_death_check_rate: LOGICFRAMES_PER_SECOND,
            scoot_from_starting_point_time: 0,
            num_virtual_mines: 1,
            repeat_detonate_move_thresh: 1.0,
            health_percent_to_drain_per_second: 0.0,
            ocl: None,
        }
    }
}

crate::impl_behavior_module_data_via_base!(MinefieldBehaviorModuleData, base);

impl MinefieldBehaviorModuleData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MINEFIELD_BEHAVIOR_FIELDS)
    }
}

fn token<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn relationship_mask(relationship: Relationship) -> Int {
    1 << (relationship as u8)
}

fn parse_detonation_weapon(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = INI::parse_ascii_string(token(tokens)?)?;
    data.detonation_weapon =
        with_weapon_store(|store| store.find_weapon_template(&name).cloned()).unwrap_or(None);
    Ok(())
}

fn parse_detonated_by(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut mask = 0;
    for raw in tokens.iter().copied().filter(|token| *token != "=") {
        for part in raw
            .split(['|', ',', '+'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            match part.to_ascii_uppercase().as_str() {
                "ALLIES" | "ALLY" => mask |= relationship_mask(Relationship::Allies),
                "ENEMIES" | "ENEMY" => mask |= relationship_mask(Relationship::Enemies),
                "NEUTRAL" | "NEUTRALS" => mask |= relationship_mask(Relationship::Neutral),
                _ => return Err(INIError::InvalidData),
            }
        }
    }
    data.detonated_by = mask;
    Ok(())
}

fn parse_bool_field(out: &mut Bool, tokens: &[&str]) -> Result<(), INIError> {
    *out = INI::parse_bool(token(tokens)?)?;
    Ok(())
}

fn parse_stops_regen_after_creator_dies(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_bool_field(&mut data.stops_regen_after_creator_dies, tokens)
}

fn parse_regenerates(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_bool_field(&mut data.regenerates, tokens)
}

fn parse_workers_detonate(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_bool_field(&mut data.workers_detonate, tokens)
}

fn parse_creator_death_check_rate(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.creator_death_check_rate = INI::parse_duration_unsigned_int(token(tokens)?)?;
    Ok(())
}

fn parse_scoot_time(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scoot_from_starting_point_time = INI::parse_duration_unsigned_int(token(tokens)?)?;
    Ok(())
}

fn parse_num_virtual_mines(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.num_virtual_mines = INI::parse_unsigned_int(token(tokens)?)?.max(1);
    Ok(())
}

fn parse_repeat_detonate_move_thresh(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.repeat_detonate_move_thresh = INI::parse_real(token(tokens)?)?;
    Ok(())
}

fn parse_degen_percent(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.health_percent_to_drain_per_second = INI::parse_percent_to_real(token(tokens)?)?;
    Ok(())
}

fn parse_creation_list(
    _ini: &mut INI,
    data: &mut MinefieldBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ocl = TheObjectCreationListStore::find_object_creation_list(token(tokens)?);
    Ok(())
}

const MINEFIELD_BEHAVIOR_FIELDS: &[FieldParse<MinefieldBehaviorModuleData>] = &[
    FieldParse {
        token: "DetonationWeapon",
        parse: parse_detonation_weapon,
    },
    FieldParse {
        token: "DetonatedBy",
        parse: parse_detonated_by,
    },
    FieldParse {
        token: "StopsRegenAfterCreatorDies",
        parse: parse_stops_regen_after_creator_dies,
    },
    FieldParse {
        token: "Regenerates",
        parse: parse_regenerates,
    },
    FieldParse {
        token: "WorkersDetonate",
        parse: parse_workers_detonate,
    },
    FieldParse {
        token: "CreatorDeathCheckRate",
        parse: parse_creator_death_check_rate,
    },
    FieldParse {
        token: "ScootFromStartingPointTime",
        parse: parse_scoot_time,
    },
    FieldParse {
        token: "NumVirtualMines",
        parse: parse_num_virtual_mines,
    },
    FieldParse {
        token: "RepeatDetonateMoveThresh",
        parse: parse_repeat_detonate_move_thresh,
    },
    FieldParse {
        token: "DegenPercentPerSecondAfterCreatorDies",
        parse: parse_degen_percent,
    },
    FieldParse {
        token: "CreationList",
        parse: parse_creation_list,
    },
];

#[derive(Clone, Debug)]
struct ImmuneInfo {
    id: ObjectID,
    collide_time: UnsignedInt,
}

impl Default for ImmuneInfo {
    fn default() -> Self {
        Self {
            id: INVALID_ID,
            collide_time: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct DetonatorInfo {
    id: ObjectID,
    where_pos: Coord3D,
}

pub struct MinefieldBehavior {
    object: Weak<RwLock<GameObject>>,
    object_id: ObjectID,
    module_data: Arc<MinefieldBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    next_death_check_frame: UnsignedInt,
    scoot_frames_left: UnsignedInt,
    scoot_vel: Coord3D,
    scoot_accel: Coord3D,
    virtual_mines_remaining: UnsignedInt,
    immunes: [ImmuneInfo; MAX_IMMUNITY],
    detonators: Vec<DetonatorInfo>,
    ignore_damage: Bool,
    regenerates: Bool,
    draining: Bool,
}

impl MinefieldBehavior {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<MinefieldBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let object_id = object.read().map_err(|_| "object lock poisoned")?.get_id();
        if let Ok(mut object) = object.write() {
            object.set_status(OBJECT_STATUS_NO_ATTACK_FROM_AI, true);
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            object_id,
            next_call_frame_and_phase: 0,
            next_death_check_frame: 0,
            scoot_frames_left: 0,
            scoot_vel: Coord3D::new(0.0, 0.0, 0.0),
            scoot_accel: Coord3D::new(0.0, 0.0, 0.0),
            virtual_mines_remaining: module_data.num_virtual_mines.max(1),
            immunes: std::array::from_fn(|_| ImmuneInfo::default()),
            detonators: Vec::new(),
            ignore_damage: false,
            regenerates: module_data.regenerates,
            draining: false,
            module_data,
        })
    }

    fn owner(&self) -> Option<Arc<RwLock<GameObject>>> {
        self.object
            .upgrade()
            .or_else(|| TheGameLogic::find_object_by_id(self.object_id))
    }

    fn current_frame() -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    fn calc_sleep_time(&self) -> UpdateSleepTime {
        if self.draining || self.scoot_frames_left > 0 {
            return UpdateSleepTime::None;
        }
        if self.immunes.iter().any(|immune| immune.id != INVALID_ID) {
            return UpdateSleepTime::None;
        }

        let mut sleep_time = u32::MAX;
        if self.regenerates && self.module_data.stops_regen_after_creator_dies {
            sleep_time = sleep_time.min(
                self.next_death_check_frame
                    .saturating_sub(Self::current_frame()),
            );
        }
        if sleep_time == 0 {
            sleep_time = 1;
        }
        UpdateSleepTime::from_u32(sleep_time)
    }

    fn set_depleted_visuals(&self, depleted: Bool) {
        let Some(owner) = self.owner() else {
            return;
        };
        let Ok(mut object) = owner.write() else {
            return;
        };
        if depleted {
            object.set_model_condition_state(MODELCONDITION_RUBBLE);
            object.set_status(OBJECT_STATUS_MASKED, true);
        } else {
            object.clear_model_condition_state(MODELCONDITION_RUBBLE);
            object.clear_status(OBJECT_STATUS_MASKED);
        }
    }

    fn detonate_once(
        &mut self,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(weapon) = &self.module_data.detonation_weapon {
            if let Some(store) = TheWeaponStore::get() {
                let _ = store.create_and_fire_temp_weapon_at_pos(weapon, self.object_id, position);
            }
        }

        if self.virtual_mines_remaining > 0 {
            self.virtual_mines_remaining -= 1;
        }

        if !self.regenerates && self.virtual_mines_remaining == 0 {
            let _ = TheGameLogic::destroy_object_by_id(self.object_id);
        } else if let Some(owner) = self.owner() {
            let percent =
                self.virtual_mines_remaining as Real / self.module_data.num_virtual_mines as Real;
            let (health, max_health) = owner
                .read()
                .ok()
                .and_then(|object| object.get_body_module())
                .and_then(|body| {
                    body.lock()
                        .ok()
                        .map(|body| (body.get_health(), body.get_max_health()))
                })
                .unwrap_or((0.0, 0.0));
            let desired = (percent * max_health).max(MIN_HEALTH);
            let amount = health - desired;
            if amount > 0.0 {
                self.ignore_damage = true;
                if let Ok(mut object) = owner.write() {
                    let mut damage = DamageInfo::with_simple(
                        amount,
                        self.object_id,
                        DamageType::Unresistable,
                        DeathType::None,
                    );
                    let _ = object.attempt_damage(&mut damage);
                }
                self.ignore_damage = false;
            }
        }

        self.set_depleted_visuals(self.virtual_mines_remaining == 0);

        if let (Some(ocl), Some(owner)) = (&self.module_data.ocl, self.owner()) {
            let _ = ObjectCreationList::create(ocl, &owner, None);
        }

        Ok(())
    }

    pub fn set_scoot_parms(
        &mut self,
        start: &Coord3D,
        end: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut end_on_ground = *end;
        if let Some(terrain) = TheTerrainLogic::get() {
            end_on_ground.z = terrain.get_ground_height(end_on_ground.x, end_on_ground.y, None);
        }

        let mut scoot_time = self.module_data.scoot_from_starting_point_time;
        if start.z > end_on_ground.z {
            let gravity: Real = 1.0;
            let falling_time = (2.0 * (start.z - end_on_ground.z) / gravity).sqrt().ceil() as u32;
            scoot_time = scoot_time.max(falling_time);
        }

        let Some(owner) = self.owner() else {
            return Ok(());
        };

        if scoot_time == 0 {
            if let Ok(mut object) = owner.write() {
                let _ = object.set_position(&end_on_ground);
            }
            self.scoot_frames_left = 0;
            return Ok(());
        }

        let dx = end_on_ground.x - start.x;
        let dy = end_on_ground.y - start.y;
        let dz = end_on_ground.z - start.z;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 0.1 && dz.abs() <= 0.1 {
            if let Ok(mut object) = owner.write() {
                let _ = object.set_position(&end_on_ground);
            }
            self.scoot_frames_left = 0;
            return Ok(());
        }

        let t = scoot_time as Real;
        let speed = dist / t;
        let accel_mag = (2.0 * (dist - speed * t) / (t * t)).abs();
        let dx_norm = if dist <= 0.1 { 0.0 } else { dx / dist };
        let dy_norm = if dist <= 0.1 { 0.0 } else { dy / dist };
        self.scoot_vel = Coord3D::new(dx_norm * speed, dy_norm * speed, 0.0);
        self.scoot_accel = Coord3D::new(-dx_norm * accel_mag, -dy_norm * accel_mag, -1.0);
        self.scoot_frames_left = scoot_time;
        if let Ok(mut object) = owner.write() {
            let _ = object.set_position(start);
        }
        Ok(())
    }

    pub fn disarm(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.regenerates {
            let _ = TheGameLogic::destroy_object_by_id(self.object_id);
            return Ok(());
        }

        if let Some(owner) = self.owner() {
            let amount = owner
                .read()
                .ok()
                .and_then(|object| object.get_body_module())
                .and_then(|body| body.lock().ok().map(|body| body.get_health() - MIN_HEALTH))
                .unwrap_or(0.0);
            if amount > 0.0 {
                self.ignore_damage = true;
                if let Ok(mut object) = owner.write() {
                    let mut damage = DamageInfo::with_simple(
                        amount,
                        self.object_id,
                        DamageType::Unresistable,
                        DeathType::None,
                    );
                    let _ = object.attempt_damage(&mut damage);
                }
                self.ignore_damage = false;
            }
        }

        self.virtual_mines_remaining = 0;
        self.set_depleted_visuals(true);
        Ok(())
    }

    fn update_internal(
        &mut self,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let now = Self::current_frame();

        if self.scoot_frames_left > 0 {
            if let Some(owner) = self.owner() {
                if let Ok(mut object) = owner.write() {
                    let mut pos = *object.get_position();
                    self.scoot_vel.x += self.scoot_accel.x;
                    self.scoot_vel.y += self.scoot_accel.y;
                    self.scoot_vel.z += self.scoot_accel.z;
                    pos.x += self.scoot_vel.x;
                    pos.y += self.scoot_vel.y;
                    pos.z += self.scoot_vel.z;

                    if let Some(terrain) = TheTerrainLogic::get() {
                        let mut tmp = pos;
                        tmp.z = 99999.0;
                        let layer = terrain.get_highest_layer_for_destination(&tmp);
                        object.set_layer(layer);
                        let mut ground = terrain.get_layer_height(pos.x, pos.y, layer);
                        if layer != PathfindLayerEnum::Ground {
                            ground += 1.0;
                        }
                        if pos.z < ground || self.scoot_frames_left <= 1 {
                            pos.z = ground;
                        }
                    }
                    let _ = object.set_position(&pos);
                }
            }
            self.scoot_frames_left = self.scoot_frames_left.saturating_sub(1);
        }

        for immune in &mut self.immunes {
            if immune.id != INVALID_ID
                && (TheGameLogic::find_object_by_id(immune.id).is_none()
                    || now > immune.collide_time + 2)
            {
                *immune = ImmuneInfo::default();
            }
        }

        if now >= self.next_death_check_frame
            && self.regenerates
            && self.module_data.stops_regen_after_creator_dies
        {
            self.next_death_check_frame = now + self.module_data.creator_death_check_rate;
            let producer_dead = self
                .owner()
                .and_then(|owner| owner.read().ok().map(|object| object.get_producer_id()))
                .filter(|id| *id != INVALID_ID)
                .and_then(TheGameLogic::find_object_by_id)
                .and_then(|producer| {
                    producer
                        .read()
                        .ok()
                        .map(|object| object.is_effectively_dead())
                })
                .unwrap_or(true);
            if producer_dead {
                self.regenerates = false;
                self.draining = true;
            }
        }

        if self.draining {
            if let Some(owner) = self.owner() {
                let max_health = owner
                    .read()
                    .ok()
                    .and_then(|object| object.get_body_module())
                    .and_then(|body| body.lock().ok().map(|body| body.get_max_health()))
                    .unwrap_or(0.0);
                let amount = (max_health * self.module_data.health_percent_to_drain_per_second)
                    / LOGICFRAMES_PER_SECOND as Real;
                if amount > 0.0 {
                    if let Ok(mut object) = owner.write() {
                        let mut damage = DamageInfo::with_simple(
                            amount,
                            self.object_id,
                            DamageType::Unresistable,
                            DeathType::Normal,
                        );
                        let _ = object.attempt_damage(&mut damage);
                    }
                }
            }
        }

        Ok(self.calc_sleep_time())
    }

    fn on_collide_internal(&mut self, other_id: ObjectID) {
        if self.virtual_mines_remaining == 0 || self.scoot_frames_left > 0 {
            return;
        }
        let Some(owner) = self.owner() else {
            return;
        };
        let Some(other) = TheGameLogic::find_object_by_id(other_id) else {
            return;
        };
        let now = Self::current_frame();

        if let Ok(other_guard) = other.read() {
            if other_guard.is_effectively_dead() {
                return;
            }
        }

        for immune in &mut self.immunes {
            if immune.id == other_id {
                immune.collide_time = now;
                return;
            }
        }

        let (other_pos, should_ignore_worker, clearing_mines, relationship) = {
            let Ok(object) = owner.read() else {
                return;
            };
            let Ok(other_object) = other.read() else {
                return;
            };
            let worker =
                other_object.is_kind_of(KindOf::Infantry) && other_object.is_kind_of(KindOf::Dozer);
            let clearing = other_object
                .get_ai()
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|ai| ai.is_clearing_mines() && ai.get_goal_object().is_some())
                })
                .unwrap_or(false);
            (
                *other_object.get_position(),
                worker,
                clearing,
                object.relationship_to(&other_object),
            )
        };

        if !self.module_data.workers_detonate && should_ignore_worker {
            return;
        }
        if (self.module_data.detonated_by & relationship_mask(relationship)) == 0 {
            return;
        }
        if clearing_mines {
            if let Some(slot) = self
                .immunes
                .iter_mut()
                .find(|immune| immune.id == INVALID_ID || immune.id == other_id)
            {
                slot.id = other_id;
                slot.collide_time = now;
            }
            return;
        }

        let threshold_sqr = self.module_data.repeat_detonate_move_thresh
            * self.module_data.repeat_detonate_move_thresh;
        if let Some(detonator) = self
            .detonators
            .iter_mut()
            .find(|detonator| detonator.id == other_id)
        {
            if dist_squared(&other_pos, &detonator.where_pos) <= threshold_sqr {
                return;
            }
            detonator.where_pos = other_pos;
        } else {
            self.detonators.push(DetonatorInfo {
                id: other_id,
                where_pos: other_pos,
            });
        }

        let _ = self.detonate_once(&other_pos);
    }

    fn on_damage_internal(&mut self, damage_info: &mut DamageInfo) {
        if self.ignore_damage {
            return;
        }

        loop {
            let Some(owner) = self.owner() else {
                return;
            };
            let (health, max_health, pos) = owner
                .read()
                .ok()
                .and_then(|object| {
                    let pos = *object.get_position();
                    object.get_body_module().and_then(|body| {
                        body.lock()
                            .ok()
                            .map(|body| (body.get_health(), body.get_max_health(), pos))
                    })
                })
                .unwrap_or((0.0, 1.0, Coord3D::new(0.0, 0.0, 0.0)));

            let expected_f = self.module_data.num_virtual_mines as Real * health / max_health;
            let mut expected = if damage_info.input.damage_type == DamageType::Healing {
                expected_f.floor() as UnsignedInt
            } else {
                expected_f.ceil() as UnsignedInt
            };
            expected = expected.min(self.module_data.num_virtual_mines);

            if self.virtual_mines_remaining < expected {
                self.virtual_mines_remaining = expected;
            } else if self.virtual_mines_remaining > expected {
                if self.draining
                    && damage_info.input.source_id == self.object_id
                    && damage_info.input.damage_type == DamageType::Unresistable
                {
                    self.virtual_mines_remaining -= 1;
                } else {
                    let _ = self.detonate_once(&pos);
                    continue;
                }
            }
            break;
        }

        if self.virtual_mines_remaining == 0 && self.regenerates {
            if let Some(owner) = self.owner() {
                if let Some(body) = owner
                    .read()
                    .ok()
                    .and_then(|object| object.get_body_module())
                {
                    if let Ok(mut body) = body.lock() {
                        let health = body.get_health();
                        if health < MIN_HEALTH {
                            let _ = body.internal_change_health(MIN_HEALTH - health);
                        }
                    }
                }
            }
        }
        self.set_depleted_visuals(self.virtual_mines_remaining == 0);
    }
}

fn dist_squared(a: &Coord3D, b: &Coord3D) -> Real {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

impl BehaviorModuleInterface for MinefieldBehavior {
    fn get_module_name(&self) -> &str {
        "MinefieldBehavior"
    }

    fn get_land_mine_interface(&mut self) -> Option<&mut dyn LandMineInterface> {
        Some(self)
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_collide(&mut self) -> Option<&mut dyn CollideModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl UpdateModuleInterface for MinefieldBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.update_internal()
    }
}

impl CollideModuleInterface for MinefieldBehavior {
    fn on_collision(&mut self, _object_id: ObjectID, other_id: ObjectID) {
        self.on_collide_internal(other_id);
    }
}

impl DamageModuleInterface for MinefieldBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, damage: &DamageInfo) -> Real {
        damage.input.amount
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_damage_internal(damage_info);
        Ok(())
    }

    fn on_healing(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_damage_internal(damage_info);
        Ok(())
    }
}

impl DieModuleInterface for MinefieldBehavior {
    fn on_die(
        &mut self,
        _damage: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = TheGameLogic::destroy_object_by_id(self.object_id);
        Ok(())
    }
}

impl LandMineInterface for MinefieldBehavior {
    fn set_scoot_parms(&mut self, start: &Coord3D, end: &Coord3D) {
        let _ = MinefieldBehavior::set_scoot_parms(self, start, end);
    }

    fn disarm(&mut self) {
        let _ = MinefieldBehavior::disarm(self);
    }
}

impl Snapshotable for MinefieldBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.virtual_mines_remaining)
            .map_err(|err| err.to_string())?;
        xfer.xfer_unsigned_int(&mut self.next_death_check_frame)
            .map_err(|err| err.to_string())?;
        xfer.xfer_unsigned_int(&mut self.scoot_frames_left)
            .map_err(|err| err.to_string())?;
        xfer.xfer_coord3d(&mut self.scoot_vel);
        xfer.xfer_coord3d(&mut self.scoot_accel);
        xfer.xfer_bool(&mut self.ignore_damage)
            .map_err(|err| err.to_string())?;
        xfer.xfer_bool(&mut self.regenerates)
            .map_err(|err| err.to_string())?;
        xfer.xfer_bool(&mut self.draining)
            .map_err(|err| err.to_string())?;

        let mut max_immunity = MAX_IMMUNITY as u8;
        xfer.xfer_unsigned_byte(&mut max_immunity)
            .map_err(|err| err.to_string())?;
        if max_immunity as usize != MAX_IMMUNITY {
            return Err(format!(
                "MinefieldBehavior::xfer expected MAX_IMMUNITY {}, got {}",
                MAX_IMMUNITY, max_immunity
            ));
        }
        for immune in &mut self.immunes {
            xfer.xfer_object_id(&mut immune.id)
                .map_err(|err| err.to_string())?;
            xfer.xfer_unsigned_int(&mut immune.collide_time)
                .map_err(|err| err.to_string())?;
        }
        self.detonators.clear();
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct MinefieldBehaviorModule {
    behavior: MinefieldBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<MinefieldBehaviorModuleData>,
}

impl MinefieldBehaviorModule {
    pub fn new(
        behavior: MinefieldBehavior,
        module_name: &AsciiString,
        module_data: Arc<MinefieldBehaviorModuleData>,
    ) -> Self {
        Self {
            behavior,
            module_name_key: NameKeyGenerator::name_to_key(module_name.as_str()),
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut MinefieldBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for MinefieldBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl EngineModule for MinefieldBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

/// Retained for legacy callers that still construct behavior modules directly.
pub struct MinefieldBehaviorFactory;

impl MinefieldBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let _ = module_data;
        Ok(Box::new(MinefieldBehavior::new(
            thing,
            Arc::new(MinefieldBehaviorModuleData::default()),
        )?))
    }
}
