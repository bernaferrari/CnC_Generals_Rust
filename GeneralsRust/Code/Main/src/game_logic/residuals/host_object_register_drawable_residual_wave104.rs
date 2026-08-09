//! Wave 104 residual peels: Object status-mask residual state machine /
//! onCreate module residual order / ActiveBody MaxHealth residual pack apply /
//! Drawable create residual bookkeeping / GameLogic::registerObject m_objList
//! doubly-linked residual insert (host-testable toward live ThingFactory Objects).
//!
//! Orthogonal to Waves 82 (ObjectStatus bit-name table), Wave 92 (body MaxHealth
//! table), Waves 100/101 (ThingFactory pipeline / ModuleFactory / Partition
//! register), and Wave 103 (KindOf packs). Host residual only — shell
//! `playable_claim` stays false; network deferred; not full GPU / W3D draw claim.
//!
//! Sources (retail ZH C++):
//! - Object.cpp ctor: m_status = objectStatusMask; helpers first; behaviors;
//!   onObjectCreated(); m_modulesReady; TheGameLogic->registerObject
//! - Object.cpp setStatus/clear path (set/clear bits on ObjectStatusMaskType)
//! - ThingFactory.cpp newObject: friend_createObject → CreateModule::onCreate
//!   loop → PartitionManager::registerObject → initObject
//! - ActiveBody.cpp MaxHealth/InitialHealth ctor apply; MaxHealthChangeType;
//!   calcDamageState (UnitDamagedThreshold 0.5 / UnitReallyDamagedThreshold 0.1);
//!   YELLOW_DAMAGE_PERCENT 0.25
//! - BodyModule.h BodyDamageType / MaxHealthChangeType name tables
//! - GameLogic.cpp registerObject: prependToList(&m_objList) + lookup + sleepy wake
//! - Object.cpp prependToList / removeFromList / isInList doubly-linked residual
//! - Drawable.cpp ctor statusBits + registerDrawable; GameClient::registerDrawable
//!   allocDrawableID + prependToList(&m_drawableList)
//!
//! Fail-closed:
//! - Not full live BehaviorModule createProc / exclusive module graph residual
//! - Not full ActiveBody attemptDamage / ArmorSet / particle damage FX residual
//! - Not full W3D Drawable draw / DrawModule residual
//! - Not full sleepy-update heap residual
//! - Shell `playable_claim` stays false; network deferred

use crate::game_logic::host_combat_sim_residual::{
    body_max_health_residual, honesty_body_max_health_residual_table_wave92,
};
use crate::game_logic::host_enum_table_residual::{
    honesty_object_status_enum_table_wave82, object_status_bit_name_index,
    OBJECT_STATUS_BIT_NAME_LIST, OBJECT_STATUS_COUNT, OBJECT_STATUS_STEALTHED,
};
use crate::game_logic::host_thing_factory_module_xfer_residual::{
    residual_name_index, DRAWABLE_STATUS_NONE, DRAWABLE_STATUS_SHADOWS, MODULE_INTERFACE_BODY,
    MODULE_INTERFACE_CREATE, MODULE_TYPE_BEHAVIOR, THING_FACTORY_OBJECT_STATUS_MASK_NONE,
    THING_FACTORY_POST_CREATE_STEPS_WAVE101,
};

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Convert ObjectStatus ordinal (0..OBJECT_STATUS_COUNT-1) to residual bit mask.
/// C++ ObjectStatusMaskType bit N corresponds to enum ordinal N (NONE=0 has no bits).
#[inline]
pub fn object_status_bit_mask_residual(ordinal: u32) -> u64 {
    if ordinal == 0 || ordinal >= OBJECT_STATUS_COUNT {
        return 0;
    }
    1u64 << ordinal
}

/// Residual: make single-status mask from bit-name (Wave 82 table).
#[inline]
pub fn make_object_status_mask_residual(name: &str) -> Option<u64> {
    let idx = object_status_bit_name_index(name)?;
    Some(object_status_bit_mask_residual(idx as u32))
}

// ---------------------------------------------------------------------------
// 1. Object residual status-mask state machine (set/clear/test)
// ---------------------------------------------------------------------------

/// Host residual ObjectStatusMaskType bookkeeping (u64 bitset).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ObjectStatusMaskResidual {
    pub bits: u64,
}

impl ObjectStatusMaskResidual {
    pub fn none() -> Self {
        Self {
            bits: THING_FACTORY_OBJECT_STATUS_MASK_NONE,
        }
    }

    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// C++ `m_status.set(objectStatus)` residual (OR bits).
    pub fn set_mask(&mut self, mask: u64) {
        self.bits |= mask;
    }

    /// C++ `m_status.clear(objectStatus)` residual (AND NOT bits).
    pub fn clear_mask(&mut self, mask: u64) {
        self.bits &= !mask;
    }

    /// C++ `m_status.test(bit)` residual.
    pub fn test_mask(&self, mask: u64) -> bool {
        (self.bits & mask) != 0
    }

    /// C++ `testStatus(OBJECT_STATUS_*)` residual via ordinal.
    pub fn test_ordinal(&self, ordinal: u32) -> bool {
        self.test_mask(object_status_bit_mask_residual(ordinal))
    }

    /// C++ Object::setStatus(mask, set) residual; returns true if changed.
    pub fn apply_set_status(&mut self, mask: u64, set: bool) -> bool {
        let old = self.bits;
        if set {
            self.set_mask(mask);
        } else {
            self.clear_mask(mask);
        }
        self.bits != old
    }
}

/// Host residual Object status state machine (ctor apply + set/clear path).
#[derive(Debug, Clone, Default)]
pub struct ObjectStatusStateMachineResidual {
    pub status: ObjectStatusMaskResidual,
    /// C++ m_modulesReady residual (false until modules fully created).
    pub modules_ready: bool,
    /// Count of setStatus residual applications that changed bits.
    pub status_change_applications: u32,
    /// Count of setStatus residual applications that were no-ops.
    pub status_noop_applications: u32,
    /// Count of onCreate residual bit sets (pre-initObject flags).
    pub on_create_status_set_applications: u32,
}

impl ObjectStatusStateMachineResidual {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ Object ctor: `m_status = objectStatusMask` (applied pre-onCreate).
    pub fn ctor_apply_status_mask_residual(&mut self, initial: u64) {
        self.status = ObjectStatusMaskResidual::from_bits(initial);
        self.modules_ready = false;
    }

    /// C++ Object::setStatus residual path.
    pub fn set_status_residual(&mut self, mask: u64, set: bool) -> bool {
        let changed = self.status.apply_set_status(mask, set);
        if changed {
            self.status_change_applications = self.status_change_applications.saturating_add(1);
        } else {
            self.status_noop_applications = self.status_noop_applications.saturating_add(1);
        }
        changed
    }

    /// Residual: CreateModule onCreate may set status bits (UNDER_CONSTRUCTION etc.)
    /// without clearing the initial mask — Wave 104 deepen onCreate residual.
    pub fn on_create_set_status_residual(&mut self, mask: u64) {
        self.status.set_mask(mask);
        self.on_create_status_set_applications =
            self.on_create_status_set_applications.saturating_add(1);
    }

    /// C++ m_modulesReady = true after modules fully created.
    pub fn mark_modules_ready_residual(&mut self) {
        self.modules_ready = true;
    }
}

/// Wave 104 honesty: Object status-mask residual state machine pack.
pub fn honesty_object_status_state_machine_residual_wave104() -> bool {
    // Wave 82 table still holds.
    honesty_object_status_enum_table_wave82()
        && THING_FACTORY_OBJECT_STATUS_MASK_NONE == 0
        && object_status_bit_mask_residual(0) == 0
        && object_status_bit_mask_residual(1) == 1 << 1 // DESTROYED
        && object_status_bit_mask_residual(3) == 1 << 3 // UNDER_CONSTRUCTION
        && object_status_bit_mask_residual(OBJECT_STATUS_STEALTHED) == 1 << 16
        && object_status_bit_mask_residual(OBJECT_STATUS_COUNT) == 0
        && make_object_status_mask_residual("UNDER_CONSTRUCTION") == Some(1 << 3)
        && make_object_status_mask_residual("STEALTHED") == Some(1 << 16)
        && make_object_status_mask_residual("UNSELECTABLE") == Some(1 << 4)
        && make_object_status_mask_residual("NOT_A_STATUS").is_none()
        // State machine residual
        && {
            let mut sm = ObjectStatusStateMachineResidual::new();
            // Ctor applies UNDER_CONSTRUCTION pre-onCreate (constructing unit residual).
            let under = make_object_status_mask_residual("UNDER_CONSTRUCTION").unwrap();
            sm.ctor_apply_status_mask_residual(under);
            sm.status.test_mask(under)
                && !sm.modules_ready
                && sm.status.bits == under
                // setStatus STEALTHED set
                && {
                    let stealth = make_object_status_mask_residual("STEALTHED").unwrap();
                    sm.set_status_residual(stealth, true)
                        && sm.status.test_mask(stealth)
                        && sm.status.test_mask(under)
                        && sm.status_change_applications == 1
                        // clear STEALTHED
                        && sm.set_status_residual(stealth, false)
                        && !sm.status.test_mask(stealth)
                        && sm.status.test_mask(under)
                        && sm.status_change_applications == 2
                        // no-op clear already-clear
                        && !sm.set_status_residual(stealth, false)
                        && sm.status_noop_applications == 1
                }
                // onCreate may OR additional bits without blowing ctor mask
                && {
                    let unsel = make_object_status_mask_residual("UNSELECTABLE").unwrap();
                    sm.on_create_set_status_residual(unsel);
                    sm.status.test_mask(under)
                        && sm.status.test_mask(unsel)
                        && sm.on_create_status_set_applications == 1
                }
                && {
                    sm.mark_modules_ready_residual();
                    sm.modules_ready
                }
        }
        // NONE residual stays empty
        && ObjectStatusMaskResidual::none().bits == 0
        && OBJECT_STATUS_BIT_NAME_LIST[3] == "UNDER_CONSTRUCTION"
}

// ---------------------------------------------------------------------------
// 2. Object create residual order (helpers → behaviors → onObjectCreated →
//    registerObject; then ThingFactory CreateModule::onCreate → partition → init)
// ---------------------------------------------------------------------------

/// C++ Object ctor + ThingFactory::newObject residual order step names.
///
/// Deepens Wave 101 post-create steps with Object-ctor-internal order residual
/// (helpers first, behaviors, onObjectCreated, GameLogic registerObject).
pub const OBJECT_CREATE_ORDER_STEPS_WAVE104: &[&str] = &[
    "CTOR_STATUS_MASK",    // m_status = objectStatusMask
    "ALLOCATE_OBJECT_ID",  // TheGameLogic->allocateObjectID
    "HELPERS_FIRST",       // SMC/StatusDamage/Subdual/Repulsor/Defection/WS/Fire/TempBonus
    "BEHAVIOR_MODULES",    // ModuleFactory newModule MODULETYPE_BEHAVIOR loop
    "ON_OBJECT_CREATED",   // BehaviorModule::onObjectCreated inter-module resolution
    "MODULES_READY",       // m_modulesReady = true
    "RADAR_ADD",           // TheRadar->addObject
    "GAMELOGIC_REGISTER",  // TheGameLogic->registerObject (m_objList prepend)
    "TF_CREATE_ON_CREATE", // ThingFactory CreateModuleInterface::onCreate loop
    "PARTITION_REGISTER",  // ThePartitionManager->registerObject
    "INIT_OBJECT",         // obj->initObject (sendObjectCreated / upgrades / battle plans)
];

/// Host residual counters for Object create residual order.
#[derive(Debug, Clone, Default)]
pub struct ObjectCreateOrderResidualCounters {
    pub ctor_status_mask_applications: u32,
    pub allocate_object_id_applications: u32,
    pub helper_module_applications: u32,
    pub behavior_module_applications: u32,
    pub on_object_created_applications: u32,
    pub modules_ready_applications: u32,
    pub radar_add_applications: u32,
    pub gamelogic_register_applications: u32,
    pub create_module_on_create_applications: u32,
    pub partition_register_applications: u32,
    pub init_object_applications: u32,
    /// Last completed step index residual (0-based into OBJECT_CREATE_ORDER_STEPS_WAVE104).
    pub last_step_index: i32,
    pub next_object_id: u32,
    pub last_allocated_object_id: u32,
}

impl ObjectCreateOrderResidualCounters {
    pub fn new() -> Self {
        Self {
            next_object_id: 1, // C++ m_nextObjID starts at 1 after reset
            last_step_index: -1,
            ..Default::default()
        }
    }

    /// C++ GameLogic::allocateObjectID residual: assign then post-increment.
    pub fn allocate_object_id_residual(&mut self) -> u32 {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.saturating_add(1);
        self.last_allocated_object_id = id;
        self.allocate_object_id_applications =
            self.allocate_object_id_applications.saturating_add(1);
        id
    }

    /// Residual Object ctor + ThingFactory newObject pipeline (host counters).
    ///
    /// `helper_count` / `behavior_count` model module array population.
    /// `create_module_count` models CreateModuleInterface::onCreate loop hits.
    /// Returns allocated ObjectID residual (0 = failed / no template).
    pub fn object_create_order_residual(
        &mut self,
        template_present: bool,
        initial_status_bits: u64,
        helper_count: u32,
        behavior_count: u32,
        create_module_count: u32,
    ) -> u32 {
        if !template_present {
            return 0;
        }
        // 0 CTOR_STATUS_MASK
        let _ = initial_status_bits;
        self.ctor_status_mask_applications = self.ctor_status_mask_applications.saturating_add(1);
        self.last_step_index = 0;
        // 1 ALLOCATE_OBJECT_ID
        let id = self.allocate_object_id_residual();
        self.last_step_index = 1;
        // 2 HELPERS_FIRST
        self.helper_module_applications =
            self.helper_module_applications.saturating_add(helper_count);
        self.last_step_index = 2;
        // 3 BEHAVIOR_MODULES
        self.behavior_module_applications = self
            .behavior_module_applications
            .saturating_add(behavior_count);
        self.last_step_index = 3;
        // 4 ON_OBJECT_CREATED (one call per behavior residual)
        self.on_object_created_applications = self
            .on_object_created_applications
            .saturating_add(behavior_count);
        self.last_step_index = 4;
        // 5 MODULES_READY
        self.modules_ready_applications = self.modules_ready_applications.saturating_add(1);
        self.last_step_index = 5;
        // 6 RADAR_ADD
        self.radar_add_applications = self.radar_add_applications.saturating_add(1);
        self.last_step_index = 6;
        // 7 GAMELOGIC_REGISTER (inside Object ctor)
        self.gamelogic_register_applications =
            self.gamelogic_register_applications.saturating_add(1);
        self.last_step_index = 7;
        // 8 TF_CREATE_ON_CREATE (ThingFactory after friend_createObject returns)
        self.create_module_on_create_applications = self
            .create_module_on_create_applications
            .saturating_add(create_module_count);
        self.last_step_index = 8;
        // 9 PARTITION_REGISTER
        self.partition_register_applications =
            self.partition_register_applications.saturating_add(1);
        self.last_step_index = 9;
        // 10 INIT_OBJECT
        self.init_object_applications = self.init_object_applications.saturating_add(1);
        self.last_step_index = 10;
        id
    }
}

/// Wave 104 honesty: Object create residual order pack.
pub fn honesty_object_create_order_residual_wave104() -> bool {
    OBJECT_CREATE_ORDER_STEPS_WAVE104.len() == 11
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "CTOR_STATUS_MASK")
            == Some(0)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "HELPERS_FIRST")
            == Some(2)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "ON_OBJECT_CREATED")
            == Some(4)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "GAMELOGIC_REGISTER")
            == Some(7)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "TF_CREATE_ON_CREATE")
            == Some(8)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "PARTITION_REGISTER")
            == Some(9)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "INIT_OBJECT")
            == Some(10)
        // GAMELOGIC_REGISTER happens before TF onCreate (inside Object ctor)
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "GAMELOGIC_REGISTER")
            < residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "TF_CREATE_ON_CREATE")
        // Cross-link Wave 101 post-create: ON_CREATE / PARTITION / INIT still hold
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "ON_CREATE_MODULES")
            == Some(2)
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "PARTITION_REGISTER")
            == Some(3)
        && residual_name_index(THING_FACTORY_POST_CREATE_STEPS_WAVE101, "INIT_OBJECT")
            == Some(4)
        // Order counters residual
        && {
            let mut c = ObjectCreateOrderResidualCounters::new();
            c.object_create_order_residual(false, 0, 0, 0, 0) == 0
                && c.allocate_object_id_applications == 0
                && {
                    let id = c.object_create_order_residual(true, 1 << 3, 3, 5, 2);
                    id == 1
                        && c.last_allocated_object_id == 1
                        && c.next_object_id == 2
                        && c.ctor_status_mask_applications == 1
                        && c.helper_module_applications == 3
                        && c.behavior_module_applications == 5
                        && c.on_object_created_applications == 5
                        && c.modules_ready_applications == 1
                        && c.radar_add_applications == 1
                        && c.gamelogic_register_applications == 1
                        && c.create_module_on_create_applications == 2
                        && c.partition_register_applications == 1
                        && c.init_object_applications == 1
                        && c.last_step_index == 10
                }
                && {
                    let id2 = c.object_create_order_residual(true, 0, 1, 1, 0);
                    id2 == 2 && c.gamelogic_register_applications == 2
                }
        }
}

// ---------------------------------------------------------------------------
// 3. ActiveBody MaxHealth residual pack application on Object residual spawn
// ---------------------------------------------------------------------------

/// C++ BodyDamageType residual ordinals (BodyModule.h).
pub const BODY_DAMAGE_PRISTINE: u32 = 0;
pub const BODY_DAMAGE_DAMAGED: u32 = 1;
pub const BODY_DAMAGE_REALLYDAMAGED: u32 = 2;
pub const BODY_DAMAGE_RUBBLE: u32 = 3;
pub const BODY_DAMAGE_TYPE_COUNT: u32 = 4;

/// Ordered BodyDamageType residual names.
pub const BODY_DAMAGE_TYPE_NAME_TABLE_RESIDUAL: &[&str] =
    &["PRISTINE", "DAMAGED", "REALLYDAMAGED", "RUBBLE"];

/// C++ MaxHealthChangeType residual ordinals (BodyModule.h).
pub const MAX_HEALTH_CHANGE_SAME_CURRENTHEALTH: u32 = 0;
pub const MAX_HEALTH_CHANGE_PRESERVE_RATIO: u32 = 1;
pub const MAX_HEALTH_CHANGE_ADD_CURRENT_HEALTH_TOO: u32 = 2;
pub const MAX_HEALTH_CHANGE_FULLY_HEAL: u32 = 3;

/// Ordered MaxHealthChangeType residual names (INI table omits FULLY_HEAL).
pub const MAX_HEALTH_CHANGE_TYPE_NAME_TABLE_RESIDUAL: &[&str] = &[
    "SAME_CURRENTHEALTH",
    "PRESERVE_RATIO",
    "ADD_CURRENT_HEALTH_TOO",
    "FULLY_HEAL",
];

/// C++ GlobalData default UnitDamagedThreshold residual.
pub const UNIT_DAMAGED_THRESH_RESIDUAL: f32 = 0.5;
/// C++ GlobalData default UnitReallyDamagedThreshold residual.
pub const UNIT_REALLY_DAMAGED_THRESH_RESIDUAL: f32 = 0.1;
/// C++ ActiveBody YELLOW_DAMAGE_PERCENT residual.
pub const YELLOW_DAMAGE_PERCENT_RESIDUAL: f32 = 0.25;
/// ActiveBody module name residual.
pub const ACTIVE_BODY_MODULE_NAME: &str = "ActiveBody";
/// ModuleFactory BODY interface residual (cross-link Wave 100/101).
pub const ACTIVE_BODY_INTERFACE_MASK: u32 = MODULE_INTERFACE_BODY;

/// C++ calcDamageState residual (ActiveBody.cpp).
#[inline]
pub fn calc_body_damage_state_residual(health: f32, max_health: f32) -> u32 {
    if max_health <= 0.0 {
        return BODY_DAMAGE_RUBBLE;
    }
    let ratio = health / max_health;
    if ratio > UNIT_DAMAGED_THRESH_RESIDUAL {
        BODY_DAMAGE_PRISTINE
    } else if ratio > UNIT_REALLY_DAMAGED_THRESH_RESIDUAL {
        BODY_DAMAGE_DAMAGED
    } else if ratio > 0.0 {
        BODY_DAMAGE_REALLYDAMAGED
    } else {
        BODY_DAMAGE_RUBBLE
    }
}

/// Host residual ActiveBody state applied when Object residual is spawned.
#[derive(Debug, Clone)]
pub struct ActiveBodyResidual {
    pub max_health: f32,
    pub initial_health: f32,
    pub current_health: f32,
    pub prev_health: f32,
    pub damage_state: u32,
    pub module_name: &'static str,
}

impl ActiveBodyResidual {
    /// C++ ActiveBody ctor residual: current/prev = InitialHealth; max = MaxHealth.
    pub fn from_module_data_residual(max_health: f32, initial_health: f32) -> Self {
        let initial = if initial_health > 0.0 {
            initial_health
        } else {
            max_health
        };
        Self {
            max_health,
            initial_health: initial,
            current_health: initial,
            prev_health: initial,
            damage_state: calc_body_damage_state_residual(initial, max_health),
            module_name: ACTIVE_BODY_MODULE_NAME,
        }
    }

    /// Spawn residual: look up Wave 92 MaxHealth table; InitialHealth defaults to Max.
    pub fn spawn_from_template_residual(template_name: &str) -> Option<Self> {
        let max = body_max_health_residual(template_name)?;
        Some(Self::from_module_data_residual(max, max))
    }

    /// C++ ActiveBody::setMaxHealth residual (subset of change types).
    pub fn set_max_health_residual(&mut self, max_health: f32, change_type: u32) {
        let prev_max = self.max_health;
        if prev_max <= 0.0 {
            self.max_health = max_health;
            return;
        }
        match change_type {
            MAX_HEALTH_CHANGE_PRESERVE_RATIO => {
                let ratio = self.current_health / prev_max;
                self.max_health = max_health;
                self.current_health = max_health * ratio;
            }
            MAX_HEALTH_CHANGE_ADD_CURRENT_HEALTH_TOO => {
                let delta = max_health - prev_max;
                self.max_health = max_health;
                self.current_health = (self.current_health + delta).max(0.0);
            }
            MAX_HEALTH_CHANGE_FULLY_HEAL => {
                self.max_health = max_health;
                self.current_health = max_health;
            }
            // SAME_CURRENTHEALTH (default): keep current_health, clamp to new max
            _ => {
                self.max_health = max_health;
                if self.current_health > max_health {
                    self.current_health = max_health;
                }
            }
        }
        self.damage_state = calc_body_damage_state_residual(self.current_health, self.max_health);
    }

    /// Residual health ratio (current / max).
    pub fn health_ratio_residual(&self) -> f32 {
        if self.max_health <= 0.0 {
            0.0
        } else {
            self.current_health / self.max_health
        }
    }

    /// Residual: crossed yellow damage threshold (0.25) this change?
    pub fn crossed_yellow_damage_residual(&self, prev_health: f32) -> bool {
        if self.max_health <= 0.0 {
            return false;
        }
        let prev_ratio = prev_health / self.max_health;
        let cur_ratio = self.current_health / self.max_health;
        prev_ratio > YELLOW_DAMAGE_PERCENT_RESIDUAL && cur_ratio < YELLOW_DAMAGE_PERCENT_RESIDUAL
    }
}

/// Host residual counters for ActiveBody MaxHealth pack application on spawn.
#[derive(Debug, Clone, Default)]
pub struct ActiveBodySpawnResidualCounters {
    pub spawn_applications: u32,
    pub spawn_unknown_template_rejects: u32,
    pub max_health_change_applications: u32,
    pub last_max_health: f32,
    pub last_current_health: f32,
}

impl ActiveBodySpawnResidualCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply Wave 92 MaxHealth residual pack when Object residual is spawned.
    pub fn apply_body_on_spawn_residual(
        &mut self,
        template_name: &str,
    ) -> Option<ActiveBodyResidual> {
        match ActiveBodyResidual::spawn_from_template_residual(template_name) {
            Some(body) => {
                self.spawn_applications = self.spawn_applications.saturating_add(1);
                self.last_max_health = body.max_health;
                self.last_current_health = body.current_health;
                Some(body)
            }
            None => {
                self.spawn_unknown_template_rejects =
                    self.spawn_unknown_template_rejects.saturating_add(1);
                None
            }
        }
    }
}

/// Wave 104 honesty: ActiveBody MaxHealth residual pack application.
pub fn honesty_active_body_max_health_apply_residual_wave104() -> bool {
    // Thresholds + type tables
    UNIT_DAMAGED_THRESH_RESIDUAL == 0.5
        && UNIT_REALLY_DAMAGED_THRESH_RESIDUAL == 0.1
        && YELLOW_DAMAGE_PERCENT_RESIDUAL == 0.25
        && BODY_DAMAGE_TYPE_COUNT == 4
        && BODY_DAMAGE_TYPE_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(BODY_DAMAGE_TYPE_NAME_TABLE_RESIDUAL, "PRISTINE") == Some(0)
        && residual_name_index(BODY_DAMAGE_TYPE_NAME_TABLE_RESIDUAL, "RUBBLE") == Some(3)
        && MAX_HEALTH_CHANGE_TYPE_NAME_TABLE_RESIDUAL.len() == 4
        && residual_name_index(
            MAX_HEALTH_CHANGE_TYPE_NAME_TABLE_RESIDUAL,
            "PRESERVE_RATIO",
        ) == Some(1)
        && residual_name_index(
            MAX_HEALTH_CHANGE_TYPE_NAME_TABLE_RESIDUAL,
            "FULLY_HEAL",
        ) == Some(3)
        && ACTIVE_BODY_MODULE_NAME == "ActiveBody"
        && ACTIVE_BODY_INTERFACE_MASK == MODULE_INTERFACE_BODY
        // Wave 92 table still holds
        && honesty_body_max_health_residual_table_wave92()
        // calcDamageState residual
        && calc_body_damage_state_residual(480.0, 480.0) == BODY_DAMAGE_PRISTINE
        && calc_body_damage_state_residual(200.0, 480.0) == BODY_DAMAGE_DAMAGED // ~0.42
        && calc_body_damage_state_residual(40.0, 480.0) == BODY_DAMAGE_REALLYDAMAGED // ~0.083
        && calc_body_damage_state_residual(0.0, 480.0) == BODY_DAMAGE_RUBBLE
        // Spawn from template residual (Wave 92 anchors)
        && {
            let body = ActiveBodyResidual::spawn_from_template_residual("AmericaTankCrusader")
                .expect("crusader");
            body.max_health == 480.0
                && body.current_health == 480.0
                && body.initial_health == 480.0
                && body.damage_state == BODY_DAMAGE_PRISTINE
                && body.module_name == "ActiveBody"
        }
        && {
            let body =
                ActiveBodyResidual::spawn_from_template_residual("AmericaInfantryRanger")
                    .expect("ranger");
            body.max_health == 180.0 && body.health_ratio_residual() == 1.0
        }
        && ActiveBodyResidual::spawn_from_template_residual("NotAUnit").is_none()
        // setMaxHealth residual change types
        && {
            let mut body = ActiveBodyResidual::from_module_data_residual(100.0, 50.0);
            body.current_health == 50.0
                && {
                    body.set_max_health_residual(200.0, MAX_HEALTH_CHANGE_PRESERVE_RATIO);
                    // 50/100 = 0.5 → 100/200
                    (body.current_health - 100.0).abs() < 0.001 && body.max_health == 200.0
                }
                && {
                    body.set_max_health_residual(300.0, MAX_HEALTH_CHANGE_ADD_CURRENT_HEALTH_TOO);
                    // delta +100 → current 200
                    (body.current_health - 200.0).abs() < 0.001 && body.max_health == 300.0
                }
                && {
                    body.set_max_health_residual(400.0, MAX_HEALTH_CHANGE_FULLY_HEAL);
                    body.current_health == 400.0 && body.max_health == 400.0
                }
                && {
                    body.current_health = 100.0;
                    body.set_max_health_residual(80.0, MAX_HEALTH_CHANGE_SAME_CURRENTHEALTH);
                    body.current_health == 80.0 && body.max_health == 80.0
                }
        }
        // Spawn counters residual
        && {
            let mut c = ActiveBodySpawnResidualCounters::new();
            c.apply_body_on_spawn_residual("GLATankScorpion").is_some()
                && c.spawn_applications == 1
                && c.last_max_health == 370.0
                && c.apply_body_on_spawn_residual("Missing").is_none()
                && c.spawn_unknown_template_rejects == 1
        }
        // Yellow damage residual edge
        && {
            let body = ActiveBodyResidual::from_module_data_residual(100.0, 100.0);
            let mut b = body;
            b.current_health = 20.0; // 0.20 < 0.25
            b.crossed_yellow_damage_residual(30.0) // 0.30 > 0.25 → 0.20
                && !b.crossed_yellow_damage_residual(20.0)
        }
}

// ---------------------------------------------------------------------------
// 4. Drawable residual create bookkeeping (not full W3D draw)
// ---------------------------------------------------------------------------

/// C++ INVALID_DRAWABLE_ID residual (0).
pub const INVALID_DRAWABLE_ID_RESIDUAL: u32 = 0;
/// C++ m_nextDrawableID initial residual (starts at 1 after client reset).
pub const DRAWABLE_NEXT_ID_INITIAL_RESIDUAL: u32 = 1;
/// C++ m_explicitOpacity / m_stealthOpacity / m_effectiveStealthOpacity ctor residual.
pub const DRAWABLE_DEFAULT_OPACITY_RESIDUAL: f32 = 1.0;
/// C++ m_expirationDate == 0 means never expires.
pub const DRAWABLE_NEVER_EXPIRES_RESIDUAL: u32 = 0;

/// C++ Drawable ctor + GameClient::registerDrawable residual step names.
pub const DRAWABLE_CREATE_STEPS_WAVE104: &[&str] = &[
    "ASSIGN_STATUS_BITS", // m_status = statusBits (before complex init)
    "INIT_LIST_LINKS",    // m_nextDrawable = m_prevDrawable = NULL
    "REGISTER_DRAWABLE",  // TheGameClient->registerDrawable
    "ALLOC_DRAWABLE_ID",  // allocDrawableID / setID (inside registerDrawable)
    "PREPEND_DRAW_LIST",  // prependToList(&m_drawableList)
    "INIT_OPACITY",       // explicit/stealth/effective = 1.0
    "UNBOUND_OBJECT",     // m_object = NULL initially
];

/// Host residual Drawable create bookkeeping node.
#[derive(Debug, Clone)]
pub struct DrawableCreateResidual {
    pub id: u32,
    pub status_bits: u32,
    pub explicit_opacity: f32,
    pub stealth_opacity: f32,
    pub effective_stealth_opacity: f32,
    pub expiration_date: u32,
    pub object_bound: bool,
    pub next_index: Option<usize>,
    pub prev_index: Option<usize>,
    pub in_list: bool,
}

impl DrawableCreateResidual {
    pub fn new_unregistered(status_bits: u32) -> Self {
        Self {
            id: INVALID_DRAWABLE_ID_RESIDUAL,
            status_bits,
            explicit_opacity: DRAWABLE_DEFAULT_OPACITY_RESIDUAL,
            stealth_opacity: DRAWABLE_DEFAULT_OPACITY_RESIDUAL,
            effective_stealth_opacity: DRAWABLE_DEFAULT_OPACITY_RESIDUAL,
            expiration_date: DRAWABLE_NEVER_EXPIRES_RESIDUAL,
            object_bound: false,
            next_index: None,
            prev_index: None,
            in_list: false,
        }
    }
}

/// Host residual GameClient drawable list + ID allocator.
#[derive(Debug, Clone, Default)]
pub struct DrawableCreateResidualRegistry {
    pub nodes: Vec<DrawableCreateResidual>,
    /// Head index into nodes (None = empty list residual).
    pub list_head: Option<usize>,
    pub next_drawable_id: u32,
    pub register_applications: u32,
    pub reject_null_template_applications: u32,
}

impl DrawableCreateResidualRegistry {
    pub fn new() -> Self {
        Self {
            next_drawable_id: DRAWABLE_NEXT_ID_INITIAL_RESIDUAL,
            ..Default::default()
        }
    }

    /// C++ allocDrawableID residual.
    pub fn alloc_drawable_id_residual(&mut self) -> u32 {
        let id = self.next_drawable_id;
        self.next_drawable_id = self.next_drawable_id.saturating_add(1);
        id
    }

    /// C++ Drawable::prependToList residual (doubly-linked insert at head).
    pub fn prepend_to_list_residual(&mut self, index: usize) {
        let old_head = self.list_head;
        {
            let node = &mut self.nodes[index];
            node.prev_index = None;
            node.next_index = old_head;
            node.in_list = true;
        }
        if let Some(h) = old_head {
            self.nodes[h].prev_index = Some(index);
        }
        self.list_head = Some(index);
    }

    /// C++ Drawable::removeFromList residual.
    pub fn remove_from_list_residual(&mut self, index: usize) {
        let (prev, next) = {
            let n = &self.nodes[index];
            (n.prev_index, n.next_index)
        };
        if let Some(n) = next {
            self.nodes[n].prev_index = prev;
        }
        if let Some(p) = prev {
            self.nodes[p].next_index = next;
        } else {
            // was head
            self.list_head = next;
        }
        let node = &mut self.nodes[index];
        node.prev_index = None;
        node.next_index = None;
        node.in_list = false;
    }

    /// C++ Drawable ctor + GameClient::registerDrawable residual.
    /// Returns node index, or None when template missing.
    pub fn new_drawable_residual(
        &mut self,
        template_present: bool,
        status_bits: u32,
    ) -> Option<usize> {
        if !template_present {
            self.reject_null_template_applications =
                self.reject_null_template_applications.saturating_add(1);
            return None;
        }
        let mut node = DrawableCreateResidual::new_unregistered(status_bits);
        // registerDrawable: alloc ID then prepend
        node.id = self.alloc_drawable_id_residual();
        let index = self.nodes.len();
        self.nodes.push(node);
        self.prepend_to_list_residual(index);
        self.register_applications = self.register_applications.saturating_add(1);
        Some(index)
    }

    /// Residual live drawable count (in_list).
    pub fn live_count_residual(&self) -> usize {
        self.nodes.iter().filter(|n| n.in_list).count()
    }

    /// Walk residual list from head; returns ids in list order.
    pub fn list_ids_residual(&self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = self.list_head;
        let mut guard = 0usize;
        while let Some(i) = cur {
            if guard > self.nodes.len() {
                break; // cycle guard
            }
            out.push(self.nodes[i].id);
            cur = self.nodes[i].next_index;
            guard += 1;
        }
        out
    }
}

/// Wave 104 honesty: Drawable create residual bookkeeping pack.
pub fn honesty_drawable_create_residual_wave104() -> bool {
    DRAWABLE_CREATE_STEPS_WAVE104.len() == 7
        && residual_name_index(DRAWABLE_CREATE_STEPS_WAVE104, "ASSIGN_STATUS_BITS")
            == Some(0)
        && residual_name_index(DRAWABLE_CREATE_STEPS_WAVE104, "REGISTER_DRAWABLE")
            == Some(2)
        && residual_name_index(DRAWABLE_CREATE_STEPS_WAVE104, "PREPEND_DRAW_LIST")
            == Some(4)
        && residual_name_index(DRAWABLE_CREATE_STEPS_WAVE104, "UNBOUND_OBJECT")
            == Some(6)
        && INVALID_DRAWABLE_ID_RESIDUAL == 0
        && DRAWABLE_NEXT_ID_INITIAL_RESIDUAL == 1
        && DRAWABLE_DEFAULT_OPACITY_RESIDUAL == 1.0
        && DRAWABLE_NEVER_EXPIRES_RESIDUAL == 0
        && DRAWABLE_STATUS_NONE == 0
        && DRAWABLE_STATUS_SHADOWS == 0x2
        // Registry residual
        && {
            let mut reg = DrawableCreateResidualRegistry::new();
            reg.new_drawable_residual(false, DRAWABLE_STATUS_NONE).is_none()
                && reg.reject_null_template_applications == 1
                && {
                    let i0 = reg
                        .new_drawable_residual(true, DRAWABLE_STATUS_SHADOWS)
                        .expect("d0");
                    let n0 = &reg.nodes[i0];
                    n0.id == 1
                        && n0.status_bits == DRAWABLE_STATUS_SHADOWS
                        && n0.explicit_opacity == 1.0
                        && n0.stealth_opacity == 1.0
                        && n0.effective_stealth_opacity == 1.0
                        && n0.expiration_date == 0
                        && !n0.object_bound
                        && n0.in_list
                        && n0.prev_index.is_none()
                        && n0.next_index.is_none()
                        && reg.list_head == Some(i0)
                }
                && {
                    let i1 = reg
                        .new_drawable_residual(true, DRAWABLE_STATUS_NONE)
                        .expect("d1");
                    // Prepend: i1 is new head; i0 follows
                    reg.list_head == Some(i1)
                        && reg.nodes[i1].id == 2
                        && reg.nodes[i1].next_index == Some(0)
                        && reg.nodes[0].prev_index == Some(i1)
                        && reg.list_ids_residual() == vec![2, 1]
                        && reg.live_count_residual() == 2
                        && reg.register_applications == 2
                }
                && {
                    // remove head (id 2)
                    reg.remove_from_list_residual(1);
                    reg.list_head == Some(0)
                        && reg.list_ids_residual() == vec![1]
                        && reg.live_count_residual() == 1
                        && !reg.nodes[1].in_list
                }
        }
}

// ---------------------------------------------------------------------------
// 5. GameLogic::registerObject m_objList doubly-linked residual insert
// ---------------------------------------------------------------------------

/// C++ GameLogic::registerObject residual step names.
pub const GAMELOGIC_REGISTER_OBJECT_STEPS_WAVE104: &[&str] = &[
    "PREPEND_OBJ_LIST",  // obj->prependToList(&m_objList)
    "LOOKUP_TABLE_ADD",  // addObjectToLookupTable(obj)
    "SLEEPY_WAKE_FRAME", // when==0 → friend_setNextCallFrame(now); pushSleepyUpdate
];

/// Host residual Object list node for m_objList.
#[derive(Debug, Clone)]
pub struct ObjectListNodeResidual {
    pub object_id: u32,
    pub next_index: Option<usize>,
    pub prev_index: Option<usize>,
    pub in_list: bool,
    /// Simulated update next-call-frame residual (0 = unset / never set in ctor).
    pub update_next_call_frame: u32,
}

/// Host residual GameLogic object list + lookup + register bookkeeping.
#[derive(Debug, Clone, Default)]
pub struct GameLogicRegisterObjectResidual {
    pub nodes: Vec<ObjectListNodeResidual>,
    pub list_head: Option<usize>,
    /// Lookup table residual: object_id → node index (sparse via Option map simulated as vec).
    pub lookup: Vec<Option<usize>>,
    pub register_applications: u32,
    pub unregister_applications: u32,
    pub lookup_add_applications: u32,
    pub lookup_remove_applications: u32,
    pub sleepy_wake_applications: u32,
    pub logic_frame: u32,
}

impl GameLogicRegisterObjectResidual {
    pub fn new() -> Self {
        Self {
            logic_frame: 0,
            ..Default::default()
        }
    }

    /// C++ now = getFrame(); if now==0 then now=1 residual.
    pub fn sleepy_now_residual(&self) -> u32 {
        if self.logic_frame == 0 {
            1
        } else {
            self.logic_frame
        }
    }

    /// C++ Object::isInList residual.
    pub fn is_in_list_residual(&self, index: usize) -> bool {
        let n = &self.nodes[index];
        n.prev_index.is_some() || n.next_index.is_some() || self.list_head == Some(index)
    }

    /// C++ Object::prependToList residual.
    pub fn prepend_to_list_residual(&mut self, index: usize) {
        debug_assert!(!self.is_in_list_residual(index));
        let old_head = self.list_head;
        {
            let node = &mut self.nodes[index];
            node.prev_index = None;
            node.next_index = old_head;
            node.in_list = true;
        }
        if let Some(h) = old_head {
            self.nodes[h].prev_index = Some(index);
        }
        self.list_head = Some(index);
    }

    /// C++ Object::removeFromList residual.
    pub fn remove_from_list_residual(&mut self, index: usize) {
        let (prev, next) = {
            let n = &self.nodes[index];
            (n.prev_index, n.next_index)
        };
        if let Some(n) = next {
            self.nodes[n].prev_index = prev;
        }
        if let Some(p) = prev {
            self.nodes[p].next_index = next;
        } else {
            self.list_head = next;
        }
        let node = &mut self.nodes[index];
        node.prev_index = None;
        node.next_index = None;
        node.in_list = false;
    }

    /// C++ addObjectToLookupTable residual (grow power-of-two style residual).
    pub fn add_to_lookup_residual(&mut self, object_id: u32, index: usize) {
        let id = object_id as usize;
        while self.lookup.len() <= id {
            let new_len = if self.lookup.is_empty() {
                2
            } else {
                self.lookup.len() * 2
            };
            self.lookup.resize(new_len, None);
        }
        self.lookup[id] = Some(index);
        self.lookup_add_applications = self.lookup_add_applications.saturating_add(1);
    }

    /// C++ removeObjectFromLookupTable residual.
    pub fn remove_from_lookup_residual(&mut self, object_id: u32) {
        let id = object_id as usize;
        if id < self.lookup.len() {
            self.lookup[id] = None;
        }
        self.lookup_remove_applications = self.lookup_remove_applications.saturating_add(1);
    }

    /// Find by object_id residual.
    pub fn find_by_id_residual(&self, object_id: u32) -> Option<usize> {
        let id = object_id as usize;
        if id < self.lookup.len() {
            self.lookup[id]
        } else {
            None
        }
    }

    /// C++ GameLogic::registerObject residual (list + lookup + sleepy wake).
    ///
    /// `update_next_call_frame`: 0 models modules that never called setWakeFrame.
    pub fn register_object_residual(
        &mut self,
        object_id: u32,
        update_next_call_frame: u32,
    ) -> usize {
        let index = self.nodes.len();
        self.nodes.push(ObjectListNodeResidual {
            object_id,
            next_index: None,
            prev_index: None,
            in_list: false,
            update_next_call_frame,
        });
        // PREPEND_OBJ_LIST
        self.prepend_to_list_residual(index);
        // LOOKUP_TABLE_ADD
        self.add_to_lookup_residual(object_id, index);
        // SLEEPY_WAKE_FRAME residual
        if self.nodes[index].update_next_call_frame == 0 {
            let now = self.sleepy_now_residual();
            self.nodes[index].update_next_call_frame = now;
            self.sleepy_wake_applications = self.sleepy_wake_applications.saturating_add(1);
        }
        self.register_applications = self.register_applications.saturating_add(1);
        index
    }

    /// Residual unregister: remove from list + clear lookup.
    pub fn unregister_object_residual(&mut self, index: usize) {
        let object_id = self.nodes[index].object_id;
        self.remove_from_list_residual(index);
        self.remove_from_lookup_residual(object_id);
        self.unregister_applications = self.unregister_applications.saturating_add(1);
    }

    /// Walk m_objList residual; returns object ids head→tail.
    pub fn list_ids_residual(&self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = self.list_head;
        let mut guard = 0usize;
        while let Some(i) = cur {
            if guard > self.nodes.len() {
                break;
            }
            out.push(self.nodes[i].object_id);
            cur = self.nodes[i].next_index;
            guard += 1;
        }
        out
    }

    pub fn live_count_residual(&self) -> usize {
        self.nodes.iter().filter(|n| n.in_list).count()
    }
}

/// Wave 104 honesty: GameLogic registerObject m_objList residual pack.
pub fn honesty_gamelogic_register_object_residual_wave104() -> bool {
    GAMELOGIC_REGISTER_OBJECT_STEPS_WAVE104.len() == 3
        && residual_name_index(
            GAMELOGIC_REGISTER_OBJECT_STEPS_WAVE104,
            "PREPEND_OBJ_LIST",
        ) == Some(0)
        && residual_name_index(
            GAMELOGIC_REGISTER_OBJECT_STEPS_WAVE104,
            "LOOKUP_TABLE_ADD",
        ) == Some(1)
        && residual_name_index(
            GAMELOGIC_REGISTER_OBJECT_STEPS_WAVE104,
            "SLEEPY_WAKE_FRAME",
        ) == Some(2)
        // Doubly-linked prepend residual + lookup + sleepy
        && {
            let mut gl = GameLogicRegisterObjectResidual::new();
            // frame 0 → sleepy now becomes 1
            gl.logic_frame = 0;
            let i0 = gl.register_object_residual(1, 0);
            gl.nodes[i0].object_id == 1
                && gl.nodes[i0].update_next_call_frame == 1 // woken to now=1
                && gl.sleepy_wake_applications == 1
                && gl.list_head == Some(i0)
                && gl.list_ids_residual() == vec![1]
                && gl.find_by_id_residual(1) == Some(i0)
                && gl.is_in_list_residual(i0)
                // Second object prepends to head; already-set wake frame not re-woken
                && {
                    gl.logic_frame = 10;
                    let i1 = gl.register_object_residual(2, 42);
                    gl.nodes[i1].update_next_call_frame == 42
                        && gl.sleepy_wake_applications == 1 // unchanged
                        && gl.list_head == Some(i1)
                        && gl.list_ids_residual() == vec![2, 1]
                        && gl.nodes[i1].next_index == Some(i0)
                        && gl.nodes[i0].prev_index == Some(i1)
                        && gl.nodes[i1].prev_index.is_none()
                        && gl.live_count_residual() == 2
                        && gl.register_applications == 2
                }
                // Third with when==0 at frame 10 → wake to 10
                && {
                    let i2 = gl.register_object_residual(3, 0);
                    gl.nodes[i2].update_next_call_frame == 10
                        && gl.sleepy_wake_applications == 2
                        && gl.list_ids_residual() == vec![3, 2, 1]
                }
                // Unregister middle (id 2)
                && {
                    gl.unregister_object_residual(1);
                    gl.list_ids_residual() == vec![3, 1]
                        && gl.find_by_id_residual(2).is_none()
                        && gl.find_by_id_residual(3) == Some(2)
                        && gl.live_count_residual() == 2
                        && gl.unregister_applications == 1
                        && gl.nodes[1].prev_index.is_none()
                        && gl.nodes[1].next_index.is_none()
                        // links: 3 → 1
                        && gl.nodes[2].next_index == Some(0)
                        && gl.nodes[0].prev_index == Some(2)
                }
        }
        // Cross-link: Object create order GAMELOGIC_REGISTER is step 7
        && residual_name_index(OBJECT_CREATE_ORDER_STEPS_WAVE104, "GAMELOGIC_REGISTER")
            == Some(7)
}

// ---------------------------------------------------------------------------
// Combined Wave 104 residual pack + cross-link
// ---------------------------------------------------------------------------

/// Cross-link: Object create order ↔ ActiveBody spawn ↔ Drawable create ↔ registerObject.
pub fn honesty_object_register_drawable_crosslink_wave104() -> bool {
    // CREATE interface residual still used by TF onCreate modules
    MODULE_INTERFACE_CREATE == 0x8
        && MODULE_TYPE_BEHAVIOR == 0
        // ActiveBody is BODY interface residual (Module.h bit 0x20)
        && MODULE_INTERFACE_BODY == 0x20
        && ACTIVE_BODY_INTERFACE_MASK == MODULE_INTERFACE_BODY
        // Status mask none residual shared with ThingFactory
        && THING_FACTORY_OBJECT_STATUS_MASK_NONE == 0
        // Combined host residual path: spawn object + body + register + drawable
        && {
            let mut order = ObjectCreateOrderResidualCounters::new();
            let mut body_c = ActiveBodySpawnResidualCounters::new();
            let mut gl = GameLogicRegisterObjectResidual::new();
            let mut draw = DrawableCreateResidualRegistry::new();

            let under = make_object_status_mask_residual("UNDER_CONSTRUCTION").unwrap();
            let mut sm = ObjectStatusStateMachineResidual::new();
            sm.ctor_apply_status_mask_residual(under);

            let oid = order.object_create_order_residual(true, under, 2, 3, 1);
            oid == 1
                && order.gamelogic_register_applications == 1
                && {
                    let body = body_c
                        .apply_body_on_spawn_residual("AmericaTankCrusader")
                        .expect("body");
                    body.max_health == 480.0
                }
                && {
                    gl.logic_frame = 0;
                    gl.register_object_residual(oid, 0);
                    gl.live_count_residual() == 1 && gl.list_ids_residual() == vec![1]
                }
                && {
                    let di = draw
                        .new_drawable_residual(true, DRAWABLE_STATUS_NONE)
                        .expect("draw");
                    draw.nodes[di].id == 1 && !draw.nodes[di].object_bound
                }
                && sm.status.test_mask(under)
                && !sm.modules_ready
                && {
                    sm.mark_modules_ready_residual();
                    sm.modules_ready
                }
        }
}

/// Wave 104 combined residual pack honesty.
pub fn honesty_object_register_drawable_residual_pack_wave104() -> bool {
    honesty_object_status_state_machine_residual_wave104()
        && honesty_object_create_order_residual_wave104()
        && honesty_active_body_max_health_apply_residual_wave104()
        && honesty_drawable_create_residual_wave104()
        && honesty_gamelogic_register_object_residual_wave104()
        && honesty_object_register_drawable_crosslink_wave104()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_status_state_machine_residual_wave104_honesty() {
        assert!(honesty_object_status_state_machine_residual_wave104());
        let under = make_object_status_mask_residual("UNDER_CONSTRUCTION").unwrap();
        assert_eq!(under, 1 << 3);
    }

    #[test]
    fn object_create_order_residual_wave104_honesty() {
        assert!(honesty_object_create_order_residual_wave104());
        assert_eq!(OBJECT_CREATE_ORDER_STEPS_WAVE104.len(), 11);
    }

    #[test]
    fn active_body_max_health_apply_residual_wave104_honesty() {
        assert!(honesty_active_body_max_health_apply_residual_wave104());
        assert_eq!(body_max_health_residual("AmericaTankCrusader"), Some(480.0));
    }

    #[test]
    fn drawable_create_residual_wave104_honesty() {
        assert!(honesty_drawable_create_residual_wave104());
    }

    #[test]
    fn gamelogic_register_object_residual_wave104_honesty() {
        assert!(honesty_gamelogic_register_object_residual_wave104());
    }

    #[test]
    fn object_register_drawable_crosslink_wave104_honesty() {
        assert!(honesty_object_register_drawable_crosslink_wave104());
    }

    #[test]
    fn residual_pack_honesty_wave104() {
        assert!(honesty_object_register_drawable_residual_pack_wave104());
    }
}
