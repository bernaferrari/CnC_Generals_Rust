//! MinefieldBehavior - Rust conversion of C++ MinefieldBehavior.
//!
//! Handles landmine virtual mine counts, regeneration, collision detonation,
//! mine-clearer immunity, scooting after placement, damage synchronization, and
//! save/load state.

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, INVALID_ID, Int, KindOf, MODELCONDITION_RUBBLE, ModuleData,
    ObjectID, ObjectStatusMaskType, ObjectStatusTypes, PathfindLayerEnum, Real, Relationship,
    TheGameLogic, UnsignedInt, XferVersion,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::effects::ObjectCreationList;
use crate::helpers::{TheObjectCreationListStore, TheTerrainLogic, TheWeaponStore};
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, DamageModuleInterface, DieModuleInterface,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::{BehaviorModuleHandle, Object as GameObject};
use crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule;
use crate::object::behavior::behavior_module::{
    BehaviorModuleData, LandMineInterface, xfer_update_module_base_state,
};
use crate::weapon::{WeaponTemplate, with_weapon_store};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::update_module::UpdateScheduleContext;
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType,
};
use game_engine::system::geometry::GeometryType;
use std::sync::{Arc, RwLock, Weak};

/// Wave 410: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

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
    object_id: ObjectID,
    owner: Weak<RwLock<GameObject>>,
    module_data: game_engine::common::thing::module::ModuleDataView<MinefieldBehaviorModuleData>,
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
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_birth_frame(object, module_data, None)
    }

    pub(crate) fn new_with_birth_frame(
        object: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
        module_birth_frame: Option<u32>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !module_data.is::<MinefieldBehaviorModuleData>() {
            return Err("Invalid module data".into());
        }
        let module_data = game_engine::common::thing::module::ModuleDataView::new(module_data, |data| data.as_any().downcast_ref::<MinefieldBehaviorModuleData>().expect("MinefieldBehavior module data type"));
        let (virtual_mines_remaining, regenerates) = {
            let data = module_data.read();
            (data.num_virtual_mines.max(1), data.regenerates)
        };
        let object_id = object.read().map_err(|_| "object lock poisoned")?.get_id();
        if let Ok(mut object) = object.write() {
            object.set_status(OBJECT_STATUS_NO_ATTACK_FROM_AI, true);
        }

        Ok(Self {
            object_id,
            owner: Arc::downgrade(&object),
            // MinefieldBehavior.cpp:102 uses UPDATE_SLEEP_NONE (one frame).
            next_call_frame_and_phase: module_birth_frame
                .unwrap_or_else(TheGameLogic::get_frame)
                .saturating_add(1),
            next_death_check_frame: 0,
            scoot_frames_left: 0,
            scoot_vel: Coord3D::new(0.0, 0.0, 0.0),
            scoot_accel: Coord3D::new(0.0, 0.0, 0.0),
            virtual_mines_remaining,
            immunes: std::array::from_fn(|_| ImmuneInfo::default()),
            detonators: Vec::new(),
            ignore_damage: false,
            regenerates,
            draining: false,
            module_data,
        })
    }

    fn owner(&self) -> Option<Arc<RwLock<GameObject>>> {
        self.owner.upgrade()
    }

    fn current_frame() -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    fn calc_sleep_time(&self, now: UnsignedInt) -> UpdateSleepTime {
        if self.draining || self.scoot_frames_left > 0 {
            return UpdateSleepTime::None;
        }
        if self.immunes.iter().any(|immune| immune.id != INVALID_ID) {
            return UpdateSleepTime::None;
        }

        let mut sleep_time = u32::MAX;
        if self.regenerates && self.module_data.read().stops_regen_after_creator_dies {
            sleep_time = sleep_time.min(
                self.next_death_check_frame
                    .saturating_sub(now),
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
        Self::apply_depleted_visuals(&mut object, depleted);
    }

    fn apply_depleted_visuals(object: &mut GameObject, depleted: Bool) {
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
        if let Some(weapon) = &self.module_data.read().detonation_weapon {
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
                self.virtual_mines_remaining as Real / self.module_data.read().num_virtual_mines as Real;
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

        if let (Some(ocl), Some(owner)) = (&self.module_data.read().ocl, self.owner()) {
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

        let mut scoot_time = self.module_data.read().scoot_from_starting_point_time;
        let gravity = crate::object::die::eject_pilot_die::current_gravity();
        if start.z > end_on_ground.z {
            let falling_time = (2.0 * (start.z - end_on_ground.z) / gravity.abs())
                .sqrt()
                .ceil() as u32;
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
        self.scoot_accel = Coord3D::new(-dx_norm * accel_mag, -dy_norm * accel_mag, gravity);
        self.scoot_frames_left = scoot_time;
        if let Ok(mut object) = owner.write() {
            let _ = object.set_position(start);
        }
        let now = Self::current_frame();
        let sleep = self.calc_sleep_time(now);
        self.next_call_frame_and_phase = match sleep {
            UpdateSleepTime::None => now.saturating_add(1),
            UpdateSleepTime::Forever => UpdateSleepTime::Forever.to_u32(),
            UpdateSleepTime::Frames(frames) => now.saturating_add(frames),
        };
        if let Some(object) = crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        {
            if let Ok(guard) = object.read() {
                guard.reschedule_named_update("MinefieldBehavior", self.next_call_frame_and_phase);
            }
        }
        Ok(())
    }

    /// MinefieldBehavior.cpp:515–534 uses the normal body damage path so
    /// AutoHeal sees the loss. The mine's own callback ignores this damage.
    fn apply_disarm_damage(object: &mut GameObject) -> bool {
        let prior_health = object.get_health();
        let mut damage = DamageInfo::with_simple(
            prior_health - MIN_HEALTH,
            object.get_id(),
            DamageType::Unresistable,
            DeathType::None,
        );
        let _ = object.attempt_damage(&mut damage);
        object.get_health() < prior_health
    }

    pub fn disarm(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.regenerates {
            let _ = TheGameLogic::destroy_object_by_id(self.object_id);
            return Ok(());
        }

        self.ignore_damage = true;
        if let Some(owner) = self.owner() {
            if let Ok(mut object) = owner.write() {
                Self::apply_disarm_damage(&mut object);
            }
        }
        self.ignore_damage = false;
        self.virtual_mines_remaining = 0;
        self.set_depleted_visuals(true);
        Ok(())
    }

    fn update_internal(
        &mut self,
        mut schedule: Option<&mut dyn UpdateScheduleContext>,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 410: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::None);
        }

        let now = schedule.as_ref().map_or_else(Self::current_frame, |schedule| schedule.frame());

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
            && self.module_data.read().stops_regen_after_creator_dies
        {
            self.next_death_check_frame = now + self.module_data.read().creator_death_check_rate;
            let creator = self.owner().and_then(|owner| {
                let actor = owner.read().ok()?;
                let id = actor.get_producer_id();
                (id != INVALID_ID).then(|| (id, actor.owning_world_services()))
            });
            // MinefieldBehavior.cpp:219–230 skips detached fields with INVALID_ID.
            if let Some((creator_id, world)) = creator {
                let producer = match world {
                    Some(world) => world.object_registry().get_object(creator_id),
                    None => TheGameLogic::find_object_by_id(creator_id),
                };
                let producer_dead = producer
                    .and_then(|producer| producer.read().ok().map(|object| object.is_effectively_dead()))
                    .unwrap_or(true);
                if producer_dead {
                    self.regenerates = false;
                    self.draining = true;
                    match &mut schedule {
                        Some(schedule) => self.stop_owner_auto_heal(Some(&mut **schedule)),
                        None => self.stop_owner_auto_heal(None),
                    }
                }
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
                let amount = (max_health * self.module_data.read().health_percent_to_drain_per_second)
                    / LOGICFRAMES_PER_SECOND as Real;
                if amount > 0.0 {
                    if schedule
                        .as_mut()
                        .is_some_and(|schedule| schedule.stage_minefield_drain(amount))
                    {
                        return Ok(self.calc_sleep_time(now));
                    }
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

        Ok(self.calc_sleep_time(now))
    }

    fn on_collide_internal(&mut self, other_id: ObjectID) {
        // Wave 410: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.virtual_mines_remaining == 0 {
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

        let (
            other_pos,
            owner_pos,
            geom_type,
            major_radius,
            minor_radius,
            should_ignore_worker,
            clearing_mines,
            relationship,
        ) = {
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
                    ai.lock().ok().map(|ai| {
                        ai.is_clearing_mines()
                            && ai.get_goal_object_id() != crate::common::INVALID_ID
                    })
                })
                .unwrap_or(false);
            let geom = object.get_geometry_info();
            (
                *other_object.get_position(),
                *object.get_position(),
                geom.get_geometry_type(),
                (geom.bounds().max.x - geom.bounds().min.x).abs() * 0.5,
                (geom.bounds().max.y - geom.bounds().min.y).abs() * 0.5,
                worker,
                clearing,
                object.relationship_to(&other_object),
            )
        };

        if !self.module_data.read().workers_detonate && should_ignore_worker {
            return;
        }
        if (self.module_data.read().detonated_by & relationship_mask(relationship)) == 0 {
            return;
        }
        if self.scoot_frames_left > 0 {
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
                if let Ok(object) = owner.read() {
                    object.reschedule_named_update("MinefieldBehavior", now.saturating_add(1));
                }
            }
            return;
        }

        let threshold = self.module_data.read().repeat_detonate_move_thresh;
        let threshold_sqr = threshold * threshold;
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

        let mut det_pt = other_pos;
        clip_point_to_footprint(
            geom_type,
            major_radius,
            minor_radius,
            &owner_pos,
            &mut det_pt,
        );
        let _ = self.detonate_once(&det_pt);
    }

    fn expected_virtual_mines(
        &self,
        health: Real,
        max_health: Real,
        damage_type: DamageType,
    ) -> UnsignedInt {
        let mine_count = self.module_data.read().num_virtual_mines;
        let ratio = mine_count as Real * health / max_health;
        // MinefieldBehavior.cpp:443–448 uses the original signed x87 conversions.
        let expected = if damage_type == DamageType::Healing {
            game_engine::base_type::real_to_int_floor(ratio)
        } else {
            game_engine::base_type::real_to_int_ceil(ratio)
        };
        (expected.max(0) as UnsignedInt).min(mine_count)
    }

    /// MinefieldBehavior.cpp:446–469. A self-sourced draining hit changes
    /// the count to the Body's expected value without detonating a mine.
    pub(crate) fn apply_drain_damage(&mut self, owner: &mut GameObject) {
        if self.ignore_damage {
            return;
        }
        let Some(body) = owner.get_body_module() else {
            return;
        };
        let expected = {
            let Ok(body) = body.lock() else {
                return;
            };
            self.expected_virtual_mines(
                body.get_health(),
                body.get_max_health(),
                DamageType::Unresistable,
            )
        };
        self.virtual_mines_remaining = expected;
        Self::apply_depleted_visuals(owner, expected == 0);
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

            let expected =
                self.expected_virtual_mines(health, max_health, damage_info.input.damage_type);

            if self.virtual_mines_remaining <= expected {
                self.virtual_mines_remaining = expected;
                break;
            }
            if self.draining
                && damage_info.input.source_id == self.object_id
                && damage_info.input.damage_type == DamageType::Unresistable
            {
                self.virtual_mines_remaining -= 1;
            } else {
                let _ = self.detonate_once(&pos);
            }
        }

        if self.virtual_mines_remaining == 0
            && self.regenerates
            && let Some(owner) = self.owner()
            && let Ok(mut object) = owner.write()
        {
            // MinefieldBehavior.cpp:473–481 revives this Body and Object together.
            let _ = object.restore_minimum_body_health_with_owner(MIN_HEALTH);
        }
        self.set_depleted_visuals(self.virtual_mines_remaining == 0);
    }

    fn stop_owner_auto_heal(&self, schedule: Option<&mut dyn UpdateScheduleContext>) {
        let Some(owner) = self.owner() else {
            return;
        };
        if let Some(schedule) = schedule {
            let registration = owner.read().ok().and_then(|actor| actor.auto_heal_update_registrations().next());
            if let Some((entry, proxy)) = registration
                && entry.with_module_downcast::<AutoHealBehaviorModule, _, _>(|module| {
                    module.behavior_mut().stop_healing_owned();
                }).is_some()
            {
                schedule.awaken(&proxy, UpdateSleepTime::Forever.to_u32());
            }
            return;
        }
        let Some(module) = owner
            .read()
            .ok()
            .and_then(|object| object.find_update_module("AutoHealBehavior"))
        else {
            return;
        };
        module.with_module_downcast::<AutoHealBehaviorModule, _, _>(|ahb| {
            ahb.behavior_mut().stop_healing();
        });
    }
}

fn dist_squared(a: &Coord3D, b: &Coord3D) -> Real {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

/// C++ GeometryInfo::clipPointToFootprint — keep the blast on the minefield pad.
fn clip_point_to_footprint(
    geom_type: GeometryType,
    major_radius: Real,
    minor_radius: Real,
    geom_center: &Coord3D,
    pt: &mut Coord3D,
) {
    match geom_type {
        GeometryType::Sphere | GeometryType::Cylinder => {
            let dx = pt.x - geom_center.x;
            let dy = pt.y - geom_center.y;
            let radius = (dx * dx + dy * dy).sqrt();
            if radius > major_radius && radius > 0.0 {
                let ratio = major_radius / radius;
                pt.x = geom_center.x + dx * ratio;
                pt.y = geom_center.y + dy * ratio;
            }
        }
        GeometryType::Box => {
            pt.x =
                pt.x.clamp(geom_center.x - major_radius, geom_center.x + major_radius);
            pt.y =
                pt.y.clamp(geom_center.y - minor_radius, geom_center.y + minor_radius);
        }
    }
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
        self.update_internal(None)
    }

    fn update_scheduled(
        &mut self,
        schedule: &mut dyn UpdateScheduleContext,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.update_internal(Some(schedule))
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;
        let mut virtual_mines_remaining = self.virtual_mines_remaining;
        xfer.xfer_unsigned_int(&mut virtual_mines_remaining)
            .map_err(|err| err.to_string())?;
        let mut next_death_check_frame = self.next_death_check_frame;
        xfer.xfer_unsigned_int(&mut next_death_check_frame)
            .map_err(|err| err.to_string())?;
        let mut scoot_frames_left = self.scoot_frames_left;
        xfer.xfer_unsigned_int(&mut scoot_frames_left)
            .map_err(|err| err.to_string())?;
        let mut scoot_vel = self.scoot_vel;
        xfer.xfer_coord3d(&mut scoot_vel);
        let mut scoot_accel = self.scoot_accel;
        xfer.xfer_coord3d(&mut scoot_accel);
        let mut ignore_damage = self.ignore_damage;
        xfer.xfer_bool(&mut ignore_damage)
            .map_err(|err| err.to_string())?;
        let mut regenerates = self.regenerates;
        xfer.xfer_bool(&mut regenerates)
            .map_err(|err| err.to_string())?;
        let mut draining = self.draining;
        xfer.xfer_bool(&mut draining)
            .map_err(|err| err.to_string())?;
        let mut max_immunity = MAX_IMMUNITY as u8;
        xfer.xfer_unsigned_byte(&mut max_immunity)
            .map_err(|err| err.to_string())?;
        for immune in &self.immunes {
            let mut id = immune.id;
            xfer.xfer_object_id(&mut id)
                .map_err(|err| err.to_string())?;
            let mut collide_time = immune.collide_time;
            xfer.xfer_unsigned_int(&mut collide_time)
                .map_err(|err| err.to_string())?;
        }
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
    module_data: game_engine::common::thing::module::ModuleDataHandle,
}

impl MinefieldBehaviorModule {
    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.behavior.next_call_frame_and_phase
    }

    pub fn new(
        behavior: MinefieldBehavior,
        module_name: &AsciiString,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
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

    /// Run an installed LandMineInterface disarm with no module guard crossing
    /// normal body damage or GameLogic's synchronous deletion callbacks.
    pub(crate) fn disarm_installed(
        entry: &BehaviorModuleHandle,
        owner: &Arc<RwLock<GameObject>>,
        on_damage: Option<&mut crate::weapon::DisarmDamageReaction<'_>>,
    ) {
        let Some(regenerates) = entry.with_module_downcast::<Self, _, _>(|module| {
            let regenerates = module.behavior.regenerates;
            if regenerates {
                module.behavior.ignore_damage = true;
            }
            regenerates
        }) else {
            return;
        };

        if !regenerates {
            let Ok(mine) = owner.read() else {
                return;
            };
            let id = mine.get_id();
            let world = mine.owning_world_services();
            drop(mine);
            let _ = match world {
                Some(world) => world.destroy_logic_object(id),
                None => TheGameLogic::destroy_object_by_id(id),
            };
            return;
        }

        let damaged = owner
            .write()
            .is_ok_and(|mut mine| MinefieldBehavior::apply_disarm_damage(&mut mine));
        // C++ MinefieldBehavior.cpp:515–534 keeps the mine's own damage
        // callback suppressed while AutoHeal reacts to actual Body loss.
        if damaged && let Some(on_damage) = on_damage {
            on_damage(owner);
        }
        entry.with_module_downcast::<Self, _, _>(|module| {
            module.behavior.ignore_damage = false;
            module.behavior.virtual_mines_remaining = 0;
        });
        if let Ok(mut mine) = owner.write() {
            MinefieldBehavior::apply_depleted_visuals(&mut mine, true);
        }
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
        self.module_data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.module_data)
    }

    /// C++ `UpdateModule : Module` interface query
    /// (`Module::DynamicInterfaceCast(ModuleInterfaceType::UPDATE)`): forwards
    /// to the wrapped behavior's per-frame hooks. Replaces the former
    /// `as_any().downcast_mut::<Self>()` branch in `ModuleUpdateProxy`.
    fn get_update_module_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        Some(self.behavior_mut())
    }

    /// C++ `UpdateModule` initial wake frame, reached through the typed
    /// `Module::get_initial_wake_frame()` accessor instead of the former
    /// `as_any().downcast_ref::<Self>()` chain in `initial_update_wake_frame`.
    fn get_initial_wake_frame(&mut self) -> Option<u32> {
        Some(self.initial_wake_frame())
    }
}

/// Retained for legacy callers that still construct behavior modules directly.
pub struct MinefieldBehaviorFactory;

impl MinefieldBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(MinefieldBehavior::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ThingTemplate;
    use crate::helpers::TheThingFactory;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use crate::team::Team;
    use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
    use game_engine::common::thing::thing_factory::{
        ensure_thing_factory_exists, get_thing_factory,
    };

    fn install_mine_template() -> Arc<dyn ThingTemplate> {
        assert!(ensure_thing_factory_exists());
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        assert_eq!(
            get_thing_factory()
                .unwrap()
                .as_mut()
                .unwrap()
                .load_ini_text(
                    "Object CountedMine
                        KindOf = MINE IMMOBILE
                        Body = ActiveBody Health
                            MaxHealth = 100
                            InitialHealth = 100
                        End
                        Behavior = MinefieldBehavior Mines
                            NumVirtualMines = 8
                            Regenerates = Yes
                        End
                    End",
                ),
            1,
        );
        TheThingFactory::find_template("CountedMine").unwrap()
    }

    #[test]
    fn installed_minefield_matches_cpp_damage_healing_and_drain_counts() {
        let world = new_for_world();
        with_active_stores(&world, || {
            let template = install_mine_template();
            let team = Arc::new(RwLock::new(Team::new("MineTeam".into(), 1)));
            let mine = TheThingFactory::get()
                .unwrap()
                .new_object_with_team_handle(template, team)
                .unwrap();
            let (mine_id, body, field) = {
                let mine = mine.read().unwrap();
                (
                    mine.get_id(),
                    mine.get_body_module().unwrap(),
                    mine.module_by_name(&"MinefieldBehavior".into()).unwrap(),
                )
            };
            // ActiveBody commits health before notifying damage behaviors.
            let set_health = |health| {
                let mut body = body.lock().unwrap();
                let delta = health - body.get_health();
                body.change_health_for_borrowed_owner(delta, false).unwrap();
            };
            field
                .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                    let behavior = module.behavior_mut();
                    assert_eq!(behavior.virtual_mines_remaining, 8);

                    set_health(50.0);
                    let mut hit = DamageInfo::with_simple(
                        50.0,
                        INVALID_ID,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    behavior.on_damage(&mut hit).unwrap();
                    assert_eq!(behavior.virtual_mines_remaining, 5);

                    set_health(25.0);
                    let mut hit = DamageInfo::with_simple(
                        25.0,
                        INVALID_ID,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    behavior.on_damage(&mut hit).unwrap();
                    assert_eq!(behavior.virtual_mines_remaining, 3);

                    set_health(50.0);
                    let mut heal = DamageInfo::with_simple(
                        25.0,
                        INVALID_ID,
                        DamageType::Healing,
                        DeathType::None,
                    );
                    behavior.on_healing(&mut heal).unwrap();
                    assert_eq!(behavior.virtual_mines_remaining, 4);

                    behavior.regenerates = false;
                    behavior.draining = true;
                    set_health(12.5);
                    let mut drain = DamageInfo::with_simple(
                        37.5,
                        mine_id,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    behavior.on_damage(&mut drain).unwrap();
                    assert_eq!(behavior.virtual_mines_remaining, 2);
                })
                .unwrap();
            let mine = mine.read().unwrap();
            assert_eq!(mine.get_health(), 12.5);
            assert!(!mine.test_status(ObjectStatusTypes::Masked));
            assert!(!mine.is_destroyed());
        });
    }

    #[test]
    fn native_mine_assembly_and_damage_use_the_supplied_owner_with_a_same_id_peer() {
        use crate::object::body::active_body::BodyDefinitionCatalog;
        use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
        use crate::system::game_logic::GameLogic;
        use game_engine::common::global_data::GlobalData;

        let receiving = new_for_world();
        let foreign = new_for_world();
        with_active_stores(&foreign, || {
            foreign
                .game_logic_mutex()
                .lock()
                .unwrap()
                .set_current_frame(17);
            let template = install_mine_template();
            let team = Arc::new(RwLock::new(Team::new("ForeignMineTeam".into(), 1)));
            let peer = TheThingFactory::get()
                .unwrap()
                .new_object_with_team_handle(Arc::clone(&template), team)
                .unwrap();
            peer.write()
                .unwrap()
                .set_status(OBJECT_STATUS_NO_ATTACK_FROM_AI, false);

            let mut logic = GameLogic::new_with_engine_stores(Arc::clone(&receiving));
            logic.set_current_frame(384);
            let definitions = Arc::new(
                BodyDefinitionCatalog::new(
                    Some(&GlobalData::default()),
                    Default::default(),
                    Default::default(),
                )
                .unwrap(),
            );
            let mut factory = ObjectFactory::new();
            let id = factory
                .prepare_object_owned(
                    &mut logic,
                    template,
                    Coord3D::ZERO,
                    None,
                    ObjectCreationFlags::NO_AI,
                    &mut |_: &str| Ok(None),
                    definitions,
                    get_module_factory().unwrap().as_ref().unwrap(),
                )
                .unwrap();
            assert_eq!(id, peer.read().unwrap().get_id());
            let mine = factory.get_object(id).unwrap().get_base_object().unwrap();
            let (body, field) = {
                let mine = mine.read().unwrap();
                assert!(mine.test_status(ObjectStatusTypes::NoAttackFromAi));
                (
                    mine.get_body_module().unwrap(),
                    mine.module_by_name(&"MinefieldBehavior".into()).unwrap(),
                )
            };
            body.lock()
                .unwrap()
                .change_health_for_borrowed_owner(-55.0, false)
                .unwrap();
            field
                .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                    assert_eq!(module.initial_wake_frame(), 385);
                    let behavior = module.behavior_mut();
                    let mut damage = DamageInfo::with_simple(
                        55.0,
                        INVALID_ID,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    behavior.on_damage(&mut damage).unwrap();
                    assert_eq!(behavior.virtual_mines_remaining, 4);
                })
                .unwrap();
            assert_eq!(mine.read().unwrap().get_health(), 45.0);
            let peer = peer.read().unwrap();
            assert_eq!(peer.get_health(), 100.0);
            assert!(!peer.test_status(ObjectStatusTypes::NoAttackFromAi));
            assert!(!peer.test_status(ObjectStatusTypes::Masked));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn mounted_mine_birth_uses_the_locked_driver_frame_and_restores_the_saved_wake() {
        use crate::object::body::active_body::BodyDefinitionCatalog;
        use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
        use crate::test_process::{TestProcess, run_bounded};
        use game_engine::common::global_data::GlobalData;
        use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
        use std::io::Cursor;

        const NAME: &str = concat!(
            module_path!(),
            "::mounted_mine_birth_uses_the_locked_driver_frame_and_restores_the_saved_wake"
        );
        if let TestProcess::ParentVerified = run_bounded(
            NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
            "GENERALS_CORE_MINE_BIRTH_FRAME_CHILD",
        ) {
            return;
        }

        let world = new_for_world();
        with_active_stores(&world, || {
            let template = install_mine_template();
            let modules = get_module_factory().unwrap();
            let modules = modules.as_ref().unwrap();
            let definitions = Arc::new(
                BodyDefinitionCatalog::new(
                    Some(&GlobalData::default()),
                    Default::default(),
                    Default::default(),
                )
                .unwrap(),
            );
            let mut factory = ObjectFactory::new();
            let mut logic = world.game_logic_mutex().lock().unwrap();
            logic.set_current_frame(384);
            let id = factory
                .prepare_object_owned(
                    &mut logic,
                    Arc::clone(&template),
                    Coord3D::ZERO,
                    None,
                    ObjectCreationFlags::NO_AI,
                    &mut |_: &str| Ok(None),
                    Arc::clone(&definitions),
                    modules,
                )
                .unwrap();
            let field = logic
                .find_object_by_id(id)
                .unwrap()
                .read()
                .unwrap()
                .module_by_name(&"MinefieldBehavior".into())
                .unwrap();
            let mut saved = Vec::new();
            field
                .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                    assert_eq!(module.initial_wake_frame(), 385);
                    module
                        .xfer(&mut XferSave::new(Cursor::new(&mut saved), 1))
                        .unwrap();
                })
                .unwrap();

            logic.set_current_frame(717);
            let mut restored = ObjectFactory::new();
            restored
                .prepare_saved_object(template, id, &mut |_: &str| Ok(None), definitions, modules)
                .unwrap();
            restored
                .get_object(id)
                .unwrap()
                .get_base_object()
                .unwrap()
                .read()
                .unwrap()
                .module_by_name(&"MinefieldBehavior".into())
                .unwrap()
                .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                    module
                        .xfer(&mut XferLoad::new(saved.as_slice(), 1))
                        .unwrap();
                    assert_eq!(module.initial_wake_frame(), 385);
                })
                .unwrap();
            assert_eq!(logic.get_frame(), 717);
        });
    }

    fn mount_regrowing_mine(
        world: &Arc<crate::system::engine_stores::EngineStores>,
    ) -> [Arc<RwLock<GameObject>>; 3] {
        use crate::system::game_logic::get_game_logic;
        with_active_stores(world, || {
            assert!(ensure_thing_factory_exists());
            if get_module_factory().unwrap().is_none() {
                init_module_factory().unwrap();
            }
            crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
            crate::initialize_weapon_store().unwrap();
            crate::weapon::with_weapon_store_mut(|store| {
                let mut blast = WeaponTemplate::new("RegrowthMineDetonation".into());
                blast.primary_damage = 99.0;
                blast.primary_damage_radius = 20.0;
                blast.attack_range = 20.0;
                blast.weapon_speed = 99999.0;
                blast.min_weapon_speed = 99999.0;
                let _ = store.add_weapon_template(blast);
            })
            .unwrap();
            get_game_logic().lock().unwrap().set_current_frame(37);
            assert_eq!(
                get_thing_factory()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .load_ini_text(
                        r#"
        Object RegrowthDisarmingDozer
            KindOf = VEHICLE DOZER
            Geometry = SPHERE
            GeometryMajorRadius = 4
            Body = ActiveBody Health
                InitialHealth = 100
                MaxHealth = 100
            End
            Behavior = DozerAIUpdate Dozer
            End
        End
        Object RegrowthLinkedMine
            KindOf = MINE IMMOBILE
            Geometry = CYLINDER
            GeometryMajorRadius = 2
            Body = ActiveBody Health
                InitialHealth = 100
                MaxHealth = 100
            End
            Behavior = MinefieldBehavior Mines
                DetonationWeapon = RegrowthMineDetonation
                NumVirtualMines = 8
                Regenerates = Yes
                StopsRegenAfterCreatorDies = Yes
                DegenPercentPerSecondAfterCreatorDies = 3.33%
            End
            Behavior = AutoHealBehavior Heal
                StartsActive = Yes
                HealingAmount = 2
                HealingDelay = 5000
                StartHealingDelay = 15000
            End
        End
        Object RegrowthMineCreator
            KindOf = STRUCTURE IMMOBILE
            Geometry = BOX
            GeometryMajorRadius = 10
            GeometryMinorRadius = 10
            Body = ActiveBody Health
                InitialHealth = 100
                MaxHealth = 100
            End
        End
        "#
                    ),
                3
            );
            let spawn = |name, x| {
                let actor = TheThingFactory::get()
                    .unwrap()
                    .new_object_optional_team(TheThingFactory::find_template(name).unwrap(), None)
                    .unwrap();
                actor
                    .write()
                    .unwrap()
                    .set_position(&Coord3D::new(x, 105.0, 0.0))
                    .unwrap();
                actor
            };
            let dozer = spawn("RegrowthDisarmingDozer", 105.0);
            let mine = spawn("RegrowthLinkedMine", 108.0);
            let producer = spawn("RegrowthMineCreator", 165.0);
            mine.write()
                .unwrap()
                .set_producer(Some(&producer.read().unwrap()));
            [dozer, mine, producer]
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn mounted_regrowth_floor_revives_only_its_receiving_owner() {
        use crate::object::body::BodyDamageContext;
        use crate::system::engine_stores::EngineStores;
        use crate::system::game_logic::get_game_logic;
        use crate::test_process::{TestProcess, run_bounded};

        const NAME: &str = concat!(module_path!(), "::mounted_regrowth_floor_revives_only_its_receiving_owner");
        if let TestProcess::ParentVerified = run_bounded(
            NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
            "GENERALS_MINEFIELD_FLOOR_OWNER_CHILD",
        ) {
            return;
        }

        let mount = |world: &Arc<EngineStores>| {
            with_active_stores(world, || {
                get_game_logic().lock().unwrap().set_current_frame(38);
                let template = install_mine_template();
                let mine = TheThingFactory::get()
                    .unwrap()
                    .new_object_optional_team(template, None)
                    .unwrap();
                let mut actor = mine.write().unwrap();
                let body = actor.get_body_module().unwrap();
                let mut damage = DamageInfo::with_simple(
                    actor.get_max_health(),
                    INVALID_ID,
                    DamageType::Unresistable,
                    DeathType::Normal,
                );
                damage.input.kill = true;
                body.lock()
                    .unwrap()
                    .attempt_damage_with_context(
                        &mut damage,
                        &BodyDamageContext {
                            owner_is_preferred_source: false,
                        },
                    )
                    .unwrap();
                // ActiveBody.cpp:1226 sets the dead flag before onDamage.
                assert_eq!(actor.get_health(), 0.0);
                actor.set_effectively_dead(true);
                drop(actor);
                (mine, damage)
            })
        };
        let driving = new_for_world();
        let peer = new_for_world();
        let (mine, mut damage) = mount(&driving);
        let (peer_mine, _) = mount(&peer);
        let id = mine.read().unwrap().get_id();
        assert_eq!(peer_mine.read().unwrap().get_id(), id);
        let callback = mine
            .read()
            .unwrap()
            .module_by_name(&"MinefieldBehavior".into())
            .unwrap();
        let deliver = |info: &mut DamageInfo| {
            callback
                .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                    module.behavior_mut().on_damage(info).unwrap();
                })
                .unwrap();
        };
        let check_peer = || {
            let peer_mine = peer_mine.read().unwrap();
            assert_eq!(peer_mine.get_health(), 0.0);
            assert!(peer_mine.is_effectively_dead());
            assert!(!peer_mine.test_status(ObjectStatusTypes::Masked));
        };

        with_active_stores(&peer, || deliver(&mut damage));
        {
            let mine = mine.read().unwrap();
            assert_eq!(mine.get_health(), MIN_HEALTH);
            assert!(!mine.is_effectively_dead());
            assert!(mine.test_status(ObjectStatusTypes::Masked));
        }
        check_peer();

        with_active_stores(&driving, || {
            let logic = get_game_logic();
            let mut logic = logic.lock().unwrap();
            logic.destroy_object(id);
            logic.cleanup_dead_objects().unwrap();
            assert!(logic.find_object_by_id(id).is_none());
        });
        assert_eq!(mine.read().unwrap().get_id(), INVALID_ID);
        assert!(mine.read().unwrap().get_body_module().is_none());
        with_active_stores(&peer, || deliver(&mut damage));
        check_peer();
    }

    #[test]
    fn mounted_disarm_restarts_auto_heal_and_regrows_first_virtual_mine() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            "object::behavior::minefield_behavior::tests::mounted_disarm_restarts_auto_heal_and_regrows_first_virtual_mine",
            "GENERALS_MINEFIELD_REGROWTH_CHILD",
        ) {
            return;
        }

        use crate::system::game_logic::get_game_logic;
        use crate::weapon::WeaponStore;
        let world = new_for_world();
        let peer = new_for_world();
        let [dozer, mine, producer] = mount_regrowing_mine(&world);
        let [peer_dozer, peer_mine, peer_producer] = mount_regrowing_mine(&peer);
        assert_eq!(
            dozer.read().unwrap().get_id(),
            peer_dozer.read().unwrap().get_id()
        );
        assert_eq!(
            mine.read().unwrap().get_id(),
            peer_mine.read().unwrap().get_id()
        );
        assert_eq!(
            producer.read().unwrap().get_id(),
            peer_producer.read().unwrap().get_id()
        );

        let observe = |mine: &Arc<RwLock<GameObject>>| {
            let mine = mine.read().unwrap();
            let count = mine
                .module_by_name(&"MinefieldBehavior".into())
                .unwrap()
                .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                    module.behavior.virtual_mines_remaining
                })
                .unwrap();
            let drawable = mine.get_drawable().unwrap();
            let rubble = drawable
                .read()
                .unwrap()
                .get_model_conditions()
                .intersects(MODELCONDITION_RUBBLE);
            let body = mine.get_body_module().unwrap();
            let body = body.lock().unwrap();
            (
                body.get_health(),
                body.get_last_healing_timestamp(),
                count,
                mine.test_status(ObjectStatusTypes::Masked),
                rubble,
                mine.is_destroyed(),
            )
        };
        with_active_stores(&world, || {
            let mut disarm = WeaponTemplate::new("RegrowthDozerDisarm".into());
            disarm.primary_damage = 1.0;
            disarm.weapon_speed = 99999.0;
            disarm.min_weapon_speed = 99999.0;
            disarm.attack_range = 5.0;
            disarm.damage_type = DamageType::Disarm;
            disarm.clip_size = 1;
            disarm.clip_reload_time = 120;
            let source = dozer.read().unwrap().get_id();
            let victim = mine.read().unwrap().get_id();
            let disarm = Arc::new(disarm);
            WeaponStore::create_and_fire_temp_weapon_at_target(&disarm, source, victim).unwrap();
            let (floor, _, count, masked, rubble, destroyed) = observe(&mine);
            assert!((floor - 0.1).abs() < 0.0001);
            assert_eq!((count, masked, rubble, destroyed), (0, true, true, false));

            let auto_heal = mine
                .read()
                .unwrap()
                .module_by_name(&"AutoHealBehavior".into())
                .unwrap();
            let observe_wake = || {
                auto_heal
                    .with_module_downcast::<AutoHealBehaviorModule, _, _>(|heal| {
                        let heal = heal.behavior();
                        (heal.last_wake_sleep(), heal.next_call_frame_and_phase >> 2)
                    })
                    .unwrap()
            };
            assert_eq!(observe_wake(), (Some(UpdateSleepTime::Frames(450)), 487));
            // At MIN_HEALTH a repeated disarm cannot lower Body health and
            // must leave the previous onDamage deadline intact.
            get_game_logic().lock().unwrap().set_current_frame(38);
            WeaponStore::create_and_fire_temp_weapon_at_target(&disarm, source, victim).unwrap();
            assert_eq!(observe_wake(), (Some(UpdateSleepTime::Frames(450)), 487));
            dozer
                .write()
                .unwrap()
                .set_position(&Coord3D::new(305.0, 105.0, 0.0))
                .unwrap();
            let mut logic = get_game_logic().lock().unwrap();
            for frame in 38..=1388 {
                logic.update(frame, || None).unwrap();
                let (expected_health, last_heal, count, masked) = match frame {
                    486 => (0.1, u32::MAX, 0, true),
                    487 | 488 => (2.1, 487, 0, true),
                    1237 | 1386 => (12.1, 1237, 0, true),
                    1387 | 1388 => (14.1, 1387, 1, false),
                    _ => continue,
                };
                let (health, timestamp, mines, is_masked, rubble, destroyed) = observe(&mine);
                assert!(
                    (health - expected_health).abs() < 0.0001,
                    "f{frame}: HP {health}"
                );
                assert_eq!(
                    (timestamp, mines, is_masked, rubble, destroyed),
                    (last_heal, count, masked, masked, false),
                    "f{frame}"
                );
            }
        });
        with_active_stores(&peer, || {
            assert_eq!(observe(&peer_mine), (100.0, u32::MAX, 8, false, false, false));
        });
    }

    #[test]
    fn mounted_creator_death_drains_and_retires_mine_on_original_frames() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            "object::behavior::minefield_behavior::tests::mounted_creator_death_drains_and_retires_mine_on_original_frames",
            "GENERALS_MINE_CREATOR_DEATH_CHILD",
        ) {
            return;
        }

        use crate::system::game_logic::get_game_logic;
        use crate::weapon::WeaponStore;
        let world = new_for_world();
        let peer = new_for_world();
        let [dozer, mine, creator] = mount_regrowing_mine(&world);
        let [peer_dozer, peer_mine, peer_creator] = mount_regrowing_mine(&peer);
        let creator_id = creator.read().unwrap().get_id();
        assert_eq!(creator_id, peer_creator.read().unwrap().get_id());
        assert_eq!(
            mine.read().unwrap().get_id(),
            peer_mine.read().unwrap().get_id()
        );
        let observe_heal = |mine: &Arc<RwLock<GameObject>>| {
            let module = mine
                .read()
                .unwrap()
                .module_by_name(&"AutoHealBehavior".into())
                .unwrap();
            module
                .with_module_downcast::<AutoHealBehaviorModule, _, _>(|module| {
                    let heal = module.behavior();
                    (
                        heal.stopped,
                        heal.soonest_heal_frame,
                        heal.next_call_frame_and_phase >> 2,
                    )
                })
                .unwrap()
        };
        let peer_heal = observe_heal(&peer_mine);

        let mut weapon = WeaponTemplate::new("CreatorDeathDozerDisarm".into());
        weapon.primary_damage = 1.0;
        weapon.weapon_speed = 99999.0;
        weapon.min_weapon_speed = 99999.0;
        weapon.attack_range = 5.0;
        weapon.damage_type = DamageType::Disarm;
        weapon.clip_size = 1;
        let weapon = Arc::new(weapon);
        let disarm = |dozer: &Arc<RwLock<GameObject>>, mine: &Arc<RwLock<GameObject>>| {
            let source = dozer.read().unwrap().get_id();
            let victim = mine.read().unwrap().get_id();
            WeaponStore::create_and_fire_temp_weapon_at_target(&weapon, source, victim).unwrap();
            dozer
                .write()
                .unwrap()
                .set_position(&Coord3D::new(305.0, 105.0, 0.0))
                .unwrap();
            victim
        };
        with_active_stores(&world, || {
            let blast_witness = TheThingFactory::get()
                .unwrap()
                .new_object_optional_team(
                    TheThingFactory::find_template("RegrowthMineCreator").unwrap(),
                    None,
                )
                .unwrap();
            blast_witness
                .write()
                .unwrap()
                .set_position(&Coord3D::new(125.0, 105.0, 0.0))
                .unwrap();
            let victim = disarm(&dozer, &mine);
            assert_eq!(observe_heal(&mine).2, 487);
            let mut logic = get_game_logic().lock().unwrap();
            for frame in 38..=517 {
                if frame == 500 {
                    logic.destroy_object(creator_id);
                }
                logic.update(frame, || None).unwrap();
            }
            assert!(logic.find_object_by_id(creator_id).is_none());
            let (minefield, proxy, body) = {
                let mine = mine.read().unwrap();
                (
                    mine.module_by_name(&"MinefieldBehavior".into()).unwrap(),
                    mine.named_update_registrations("MinefieldBehavior")
                        .pop()
                        .unwrap(),
                    mine.get_body_module().unwrap(),
                )
            };
            let mine_state = || {
                minefield
                    .with_module_downcast::<MinefieldBehaviorModule, _, _>(|module| {
                        let behavior = module.behavior_mut();
                        assert!(behavior.module_data.read().detonation_weapon.is_some());
                        (
                            behavior.regenerates,
                            behavior.draining,
                            behavior.virtual_mines_remaining,
                        )
                    })
                    .unwrap()
            };
            let health = || body.lock().unwrap().get_health();
            assert_eq!(logic.sleepy_entry_for(&proxy), Some((victim, 518)));
            assert_eq!(observe_heal(&mine), (false, 637, 637));
            assert_eq!(mine_state(), (true, false, 0));
            assert!((health() - 2.1).abs() < 0.0001);

            // The f38 creator poll repeats every 30 frames. f518 sees the exact
            // retired creator, stops healing, and applies one native drain tick.
            logic.update(518, || None).unwrap();
            assert_eq!(observe_heal(&mine), (true, u32::MAX, 0x3fff_ffff));
            assert_eq!(mine_state(), (false, true, 1));
            assert!(!mine.read().unwrap().test_status(ObjectStatusTypes::Masked));
            assert_eq!(logic.sleepy_entry_for(&proxy), Some((victim, 519)));
            assert!((health() - 1.989).abs() < 0.0001);
            logic.update(519, || None).unwrap();
            assert!((health() - 1.878).abs() < 0.0001);
            assert_eq!(body.lock().unwrap().get_last_healing_timestamp(), 487);
            assert_eq!(observe_heal(&mine), (true, u32::MAX, 0x3fff_ffff));
            for frame in 520..=535 {
                logic.update(frame, || None).unwrap();
            }
            assert!((health() - 0.102).abs() < 0.0001);
            assert!(logic.find_object_by_id(victim).is_some());
            assert_eq!(mine_state(), (false, true, 1));
            logic.update(536, || None).unwrap();
            assert_eq!(health(), 0.0);
            assert_eq!(mine_state(), (false, true, 0));
            assert!(logic.find_object_by_id(victim).is_none());
            assert!(logic.sleepy_entry_for(&proxy).is_none());
            assert_eq!(blast_witness.read().unwrap().get_health(), 100.0);
            let death = body.lock().unwrap().get_last_damage_info().unwrap();
            assert_eq!(death.input.source_id, victim);
            assert_eq!(death.input.damage_type, DamageType::Unresistable);
            assert_eq!(death.input.death_type, DeathType::Normal);
        });
        with_active_stores(&peer, || {
            assert_eq!(peer_mine.read().unwrap().get_health(), 100.0);
            assert_eq!(observe_heal(&peer_mine), peer_heal);
            assert!(
                get_game_logic()
                    .lock()
                    .unwrap()
                    .find_object_by_id(creator_id)
                    .is_some_and(|actor| Arc::ptr_eq(&actor, &peer_creator))
            );
            peer_mine.write().unwrap().set_producer(None);
            let peer_victim = disarm(&peer_dozer, &peer_mine);
            assert_eq!(peer_mine.read().unwrap().get_producer_id(), INVALID_ID);
            let mut logic = get_game_logic().lock().unwrap();
            for frame in 38..=536 {
                logic.update(frame, || None).unwrap();
            }
            assert!(logic.find_object_by_id(peer_victim).is_some());
            assert!((peer_mine.read().unwrap().get_health() - 2.1).abs() < 0.0001);
            assert_eq!(observe_heal(&peer_mine), (false, 637, 637));
        });
    }
}
