use super::*;
use crate::command_system::SpecialPowerType;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Object type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectType {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Supply,
    Projectile,
    Neutral,
}

/// Game Object - the main entity class for all game units, buildings, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Base Thing functionality
    pub thing: Thing,

    /// Unique identifier
    pub id: ObjectId,

    /// Link to the GameEngine crate's full Object (ObjectFactory-created).
    /// When Some, this object has a full module system (AI, weapons, physics, drawables).
    /// When None, this is a lightweight visual-only object.
    pub engine_object_id: Option<u32>,

    /// Team ownership
    pub team: Team,

    /// Object name
    pub name: String,

    /// Object status
    pub status: ObjectStatus,

    /// Health system
    pub health: Health,

    /// Movement system
    pub movement: Movement,

    /// Experience system
    pub experience: Experience,

    /// Primary weapon
    pub weapon: Option<Weapon>,

    /// Secondary weapon slot (C++ WeaponSet SECONDARY). Optional residual bind.
    pub secondary_weapon: Option<Weapon>,

    /// Current target
    pub target: Option<ObjectId>,

    /// Construction progress (0.0 to 1.0)
    pub construction_percent: f32,

    /// Building-specific data (present for structures)
    pub building_data: Option<BuildingData>,

    /// Resource storage for buildings
    pub stored_resources: Resources,

    /// Power provided/consumed
    pub power_provided: i32,
    pub power_consumed: i32,

    /// Selection state
    pub selected: bool,

    /// AI state for autonomous behavior
    pub ai_state: AIState,

    // Command system compatibility fields
    /// Object type identifier
    pub object_type: ObjectType,

    /// Template name for identification
    pub template_name: String,

    /// Current position (shadow of thing.position for compatibility)
    pub position: Vec3,

    /// Maximum health
    pub max_health: f32,

    /// Target location for ground attacks
    pub target_location: Option<Vec3>,

    /// Guard position
    pub guard_position: Option<Vec3>,

    /// Guard target
    pub guard_target: Option<ObjectId>,

    /// Force attack mode
    pub force_attack: bool,

    /// Visual properties for rendering
    pub show_health_bar: bool,
    pub selection_radius: f32,
    pub team_color: [f32; 4],

    /// Tracked occupants for transports/garrisons
    pub occupants: Vec<ObjectId>,

    /// Residual transport slot capacity (vehicles).
    /// `0` = use footprint heuristic (existing host residual default).
    /// Explicit value (e.g. Humvee/Chinook slots) hard-caps occupants.
    /// Fail-closed: not multi-door / air-transport path parity.
    pub max_transport: usize,

    /// Host residual: China Overlord / BattleBunker infantry capacity.
    ///
    /// C++ OverlordContain holds one PORTABLE_STRUCTURE (BattleBunker), then
    /// redirects infantry contain queries into the bunker's TransportContain
    /// (INI `Slots = 5`). Host residual collapses that redirect into a single
    /// capacity on the tank:
    /// - `None` — not an overlord-style container (normal vehicle residual)
    /// - `Some(0)` — overlord-style without BattleBunker residual (reject enter)
    /// - `Some(n)` — BattleBunker residual active with `n` infantry slots
    ///
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn /
    /// GattlingCannon / PropagandaTower payload matrix.
    pub overlord_bunker_capacity: Option<usize>,

    /// C++ parity (Object::m_containedBy): when this unit is inside a
    /// transport/garrison, stores the container's ID.  None when free.
    pub contained_by: Option<ObjectId>,

    /// Optional short-lived cheer/animation timer
    pub cheer_timer: f32,

    /// Toggleable weapon/overcharge state flags
    pub overcharge_enabled: bool,
    pub active_weapon_slot: u8,

    /// Stored guard radius for pathing/AI persistence
    pub guard_radius: f32,

    /// Applied upgrades keyed by upgrade template/tag name.
    pub applied_upgrades: HashSet<String>,

    /// Special power availability/cooldown state.
    pub special_power_ready: bool,
    pub special_power_cooldown: f32,
    pub special_power_cooldown_remaining: f32,

    /// Host residual mine / demo-trap / timed demo-charge state.
    /// `None` for ordinary units/structures. Fail-closed: not full C++
    /// MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate modules.
    pub mine_data: Option<crate::game_logic::host_mines::HostMineData>,

    /// Host residual: unit can detect stealthed enemies (C++ StealthDetectorUpdate).
    /// Fail-closed: not full IR FX / kindof filters / garrisoned-detect rules.
    pub is_detector: bool,
    /// Detection range in world units. `0` => use template `sight_range`
    /// (matches C++ when DetectionRange is unset/0).
    pub detection_range: f32,
    /// Logic frame when OBJECT_STATUS_DETECTED expires (0 = no timer).
    /// C++ StealthUpdate::m_detectionExpiresFrame residual.
    pub detection_expires_frame: u32,
    /// C++ STEALTH_NOT_WHILE_ATTACKING residual: firing breaks stealth.
    /// Default true for host residual honesty.
    pub stealth_breaks_on_attack: bool,

    /// Host residual: bitmask of player indices currently vision-spying this unit
    /// (C++ Object::m_visionSpiedBy / setVisionSpied for CIA Intelligence SpyVision).
    /// Fail-closed: not full looking_mask partition maintenance.
    pub vision_spied_mask: u32,

    /// Host residual weapon-bonus flags from PropagandaTowerBehavior.
    /// C++ WEAPONBONUSCONDITION_ENTHUSIASTIC / SUBLIMINAL (rate-of-fire buff near speaker tower).
    /// Fail-closed: not full WeaponBonusConditionFlags matrix / ROF multiplier application.
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,
}

/// AI behavior states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AIState {
    Idle,
    Moving,
    Attacking,
    AttackMoving,
    AttackingGround,
    Gathering,
    ReturningResources,
    Constructing,
    Repairing,
    GuardingArea,
    GuardingObject,
    Patrolling,
    Docked,
    Garrisoned,
    SpecialAbility,
    SeekingRepair,
    SeekingHealing,
    Entering,
    Docking,
    Capturing,
}

impl Object {
    pub fn new(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let max_health = template.max_health;
        let position = Vec3::ZERO; // Default position
        let template_name = template.name.clone();

        // Determine object type from template
        let object_type = if template.is_kind_of(KindOf::Infantry) {
            ObjectType::Infantry
        } else if template.is_kind_of(KindOf::Vehicle) {
            ObjectType::Vehicle
        } else if template.is_kind_of(KindOf::Aircraft) {
            ObjectType::Aircraft
        } else if template.is_kind_of(KindOf::Structure) {
            ObjectType::Building
        } else {
            ObjectType::Neutral
        };

        // Calculate selection radius based on object type
        let selection_radius = match object_type {
            ObjectType::Infantry => 8.0,
            ObjectType::Vehicle => 15.0,
            ObjectType::Aircraft => 20.0,
            ObjectType::Building => 25.0,
            ObjectType::Neutral => 10.0,
            _ => 10.0,
        };

        let building_data = if object_type == ObjectType::Building {
            let building_type = BuildingType::from_template_name(&template_name);
            Some(BuildingData::new(building_type))
        } else {
            None
        };

        let special_power_cooldown = template.special_power_cooldown;

        let (power_provided, power_consumed) = building_data
            .as_ref()
            .map(|data| (data.power_output, data.power_requirement))
            .unwrap_or((0, 0));

        Self {
            thing: Thing::new(template),
            id,
            engine_object_id: None,
            team,
            name: String::new(),
            status: ObjectStatus::default(),
            health: Health::new(max_health),
            movement: Movement::default(),
            experience: Experience::default(),
            weapon: None,
            secondary_weapon: None,
            target: None,
            construction_percent: 1.0, // Fully constructed by default
            building_data,
            stored_resources: Resources::default(),
            power_provided,
            power_consumed,
            selected: false,
            ai_state: AIState::Idle,
            object_type,
            template_name,
            position,
            max_health,
            target_location: None,
            guard_position: None,
            guard_target: None,
            force_attack: false,
            show_health_bar: true, // Show health bars by default
            selection_radius,
            team_color: team.get_color(),
            occupants: Vec::new(),
            max_transport: 0,
            overlord_bunker_capacity: None,
            contained_by: None,
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            guard_radius: 0.0,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown,
            special_power_cooldown_remaining: 0.0,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
        }
    }

    /// Alternative constructor for command system compatibility
    pub fn new_simple(id: ObjectId, object_type: ObjectType, template_name: String) -> Self {
        let template = ThingTemplate::new(&template_name);
        let team = Team::Neutral;
        let selection_radius = match object_type {
            ObjectType::Infantry => 8.0,
            ObjectType::Vehicle => 15.0,
            ObjectType::Aircraft => 20.0,
            ObjectType::Building => 25.0,
            ObjectType::Neutral => 10.0,
            _ => 10.0,
        };

        Self {
            thing: Thing::new(template),
            id,
            engine_object_id: None,
            team,
            name: String::new(),
            status: ObjectStatus::default(),
            health: Health::new(100.0),
            movement: Movement::default(),
            experience: Experience::default(),
            weapon: None,
            secondary_weapon: None,
            target: None,
            construction_percent: 1.0,
            building_data: None,
            stored_resources: Resources::default(),
            power_provided: 0,
            power_consumed: 0,
            selected: false,
            ai_state: AIState::Idle,
            object_type,
            template_name,
            position: Vec3::ZERO,
            max_health: 100.0,
            target_location: None,
            guard_position: None,
            guard_target: None,
            force_attack: false,
            show_health_bar: true,
            selection_radius,
            team_color: team.get_color(),
            occupants: Vec::new(),
            max_transport: 0,
            overlord_bunker_capacity: None,
            contained_by: None,
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            guard_radius: 0.0,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown: 10.0,
            special_power_cooldown_remaining: 0.0,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
        }
    }

    pub fn new_under_construction(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let mut obj = Self::new(template, id, team);
        obj.construction_percent = 0.0;
        obj.status.under_construction = true;
        obj.health.current = 0.1; // Very low health during construction
        obj
    }

    pub fn get_template(&self) -> &ThingTemplate {
        self.thing.get_template()
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.thing.is_kind_of(kind)
    }

    pub fn is_alive(&self) -> bool {
        if let Some(engine_id) = self.engine_object_id {
            if let Some(alive) = Self::read_engine_is_alive(engine_id) {
                return alive;
            }
        }
        !self.status.destroyed && self.health.is_alive()
    }

    fn read_engine_is_alive(engine_id: u32) -> Option<bool> {
        let obj = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id)?;
        let guard = obj.read().ok()?;
        Some(guard.is_alive())
    }

    pub fn get_health_percentage(&self) -> f32 {
        if let Some(engine_id) = self.engine_object_id {
            if let Some(pct) = Self::read_engine_health_percentage(engine_id) {
                return pct;
            }
        }
        self.health.percentage()
    }

    fn read_engine_health_percentage(engine_id: u32) -> Option<f32> {
        let obj = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id)?;
        let guard = obj.read().ok()?;
        Some(guard.get_health_percentage())
    }

    pub fn is_constructed(&self) -> bool {
        !self.status.under_construction && self.construction_percent >= 1.0
    }

    pub fn is_mobile(&self) -> bool {
        self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
    }

    pub fn is_selectable(&self) -> bool {
        self.is_alive()
            && self.is_kind_of(KindOf::Selectable)
            && !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
    }

    pub fn is_worker(&self) -> bool {
        self.is_kind_of(KindOf::Worker)
            || self.template_name.contains("Dozer")
            || self.template_name.contains("Worker")
            || self.template_name.contains("Harvester")
            || self.template_name.contains("Collector")
    }

    pub fn is_hero(&self) -> bool {
        self.is_kind_of(KindOf::Hero) || self.template_name.contains("Hero")
    }

    pub fn is_command_center(&self) -> bool {
        self.is_kind_of(KindOf::CommandCenter)
            || self.template_name.contains("CommandCenter")
            || self.template_name.contains("Headquarters")
    }

    pub fn is_faction_structure(&self) -> bool {
        self.is_kind_of(KindOf::FSBarracks)
            || self.is_kind_of(KindOf::FSWarFactory)
            || self.is_kind_of(KindOf::FSAirfield)
            || self.is_kind_of(KindOf::FSInternetCenter)
            || self.is_kind_of(KindOf::FSPower)
            || self.is_kind_of(KindOf::FSBaseDefense)
            || self.is_kind_of(KindOf::FSSupplyDropzone)
            || self.is_kind_of(KindOf::FSSupplyCenter)
            || self.is_kind_of(KindOf::FSSuperweapon)
            || self.is_kind_of(KindOf::FSStrategyCenter)
            || self.is_kind_of(KindOf::FSFake)
            || self.is_kind_of(KindOf::FSTechnology)
            || self.is_kind_of(KindOf::FSBlackMarket)
            || self.is_kind_of(KindOf::FSAdvancedTech)
            || self.is_command_center()
            || self.is_kind_of(KindOf::SupplyCenter)
            || self.is_kind_of(KindOf::PowerPlant)
            || self.template_name.contains("Barracks")
            || self.template_name.contains("WarFactory")
            || self.template_name.contains("Airfield")
            || self.template_name.contains("InternetCenter")
            || self.template_name.contains("PowerPlant")
            || self.template_name.contains("SupplyDropzone")
            || self.template_name.contains("SupplyCenter")
            || self.template_name.contains("Superweapon")
            || self.template_name.contains("StrategyCenter")
            || self.template_name.contains("BlackMarket")
            || self.template_name.contains("TechCenter")
    }

    pub fn is_non_faction_structure(&self) -> bool {
        self.is_kind_of(KindOf::Structure) && !self.is_faction_structure()
    }

    /// C++ parity (Object::isDisabled): returns true if the object is in any
    /// disabled state that prevents it from acting (attacking, producing, etc.)
    ///
    /// Note: `weapons_jammed` (ECM residual) is intentionally **not** full
    /// disabled — C++ DISABLED_SUBDUED only blocks `canFireWeapon`; residual
    /// keeps movement. Check `is_weapons_jammed()` / `can_attack()` for fire.
    pub fn is_disabled(&self) -> bool {
        self.status.disabled_underpowered
            || self.status.disabled_unmanned
            || self.status.disabled_hacked
            || self.status.disabled_emp
            || self.status.under_construction
    }

    /// C++ DISABLED_UNMANNED residual (Jarmen Kell kill-pilot snipe).
    pub fn is_unmanned(&self) -> bool {
        self.status.disabled_unmanned
    }

    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack).
    pub fn is_hacked_disabled(&self) -> bool {
        self.status.disabled_hacked
    }

    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse).
    pub fn is_emp_disabled(&self) -> bool {
        self.status.disabled_emp
    }

    /// Host ECM / jammer residual: weapons cannot fire while in jam radius.
    /// C++ DISABLED_SUBDUED / canFireWeapon residual (Microwave/ECM disabler).
    pub fn is_weapons_jammed(&self) -> bool {
        self.status.weapons_jammed
    }

    /// Apply / clear weapons-jam residual (ECM field coverage).
    pub fn set_weapons_jammed(&mut self, jammed: bool) {
        if jammed {
            self.status.weapons_jammed = true;
            // C++ canFireWeapon false while subdued: drop in-progress attack fire
            // but do not freeze movement (jam residual is weapons-only).
            self.status.attacking = false;
            self.force_attack = false;
        } else {
            self.status.weapons_jammed = false;
        }
    }

    /// Apply kill-pilot residual: vehicle becomes unmanned (no HP change).
    /// Caller is responsible for team transfer (typically Neutral).
    pub fn apply_kill_pilot_unmanned(&mut self) {
        self.status.disabled_unmanned = true;
        self.status.disabled_hacked = false;
        self.status.disabled_hacked_until_frame = 0;
        self.status.disabled_emp = false;
        self.status.disabled_emp_until_frame = 0;
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    /// Apply DISABLED_HACKED residual until `until_frame` (absolute host logic frame).
    /// C++ SpecialAbilityUpdate: setDisabledUntil(DISABLED_HACKED, now + EffectDuration).
    pub fn apply_disabled_hacked(&mut self, until_frame: u32) {
        self.status.disabled_hacked = true;
        self.status.disabled_hacked_until_frame = until_frame;
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    /// Expire DISABLED_HACKED when the host frame passes the residual timer.
    pub fn tick_disabled_hacked(&mut self, current_frame: u32) {
        if self.status.disabled_hacked
            && self.status.disabled_hacked_until_frame > 0
            && current_frame >= self.status.disabled_hacked_until_frame
        {
            self.status.disabled_hacked = false;
            self.status.disabled_hacked_until_frame = 0;
        }
    }

    /// Apply DISABLED_EMP residual until `until_frame` (absolute host logic frame).
    /// C++ EMPUpdate::doDisableAttack: setDisabledUntil(DISABLED_EMP, now + DisabledDuration).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_disabled_emp(&mut self, until_frame: u32) {
        self.status.disabled_emp = true;
        if until_frame > self.status.disabled_emp_until_frame {
            self.status.disabled_emp_until_frame = until_frame;
        }
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    /// Expire DISABLED_EMP when the host frame passes the residual timer.
    pub fn tick_disabled_emp(&mut self, current_frame: u32) {
        if self.status.disabled_emp
            && self.status.disabled_emp_until_frame > 0
            && current_frame >= self.status.disabled_emp_until_frame
        {
            self.status.disabled_emp = false;
            self.status.disabled_emp_until_frame = 0;
        }
    }

    /// C++ OBJECT_STATUS_IS_CARBOMB residual.
    pub fn is_car_bomb(&self) -> bool {
        self.status.is_carbomb
    }

    /// C++ OBJECT_STATUS_HIJACKED residual.
    pub fn is_hijacked(&self) -> bool {
        self.status.hijacked
    }

    /// Apply ConvertToCarBomb residual onto this vehicle (caller sets team).
    /// Binds SuicideCarBomb residual weapon and marks IS_CARBOMB.
    pub fn apply_convert_to_car_bomb(&mut self) {
        self.status.is_carbomb = true;
        self.status.disabled_unmanned = false;
        self.status.disabled_hacked = false;
        self.status.disabled_hacked_until_frame = 0;
        self.status.disabled_emp = false;
        self.status.disabled_emp_until_frame = 0;
        self.status.hijacked = false;
        self.weapon = Some(crate::game_logic::host_car_bomb::suicide_car_bomb_weapon());
        self.secondary_weapon = None;
        self.active_weapon_slot = 0;
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    /// Apply Hijack residual ownership mark (caller sets team).
    /// C++ ConvertToHijackedVehicleCrateCollide: OBJECT_STATUS_HIJACKED + idle AI.
    pub fn apply_hijacked(&mut self) {
        self.status.hijacked = true;
        self.status.disabled_unmanned = false;
        self.status.disabled_hacked = false;
        self.status.disabled_hacked_until_frame = 0;
        self.status.disabled_emp = false;
        self.status.disabled_emp_until_frame = 0;
        self.status.is_carbomb = false;
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    pub fn can_attack(&self) -> bool {
        // Garrisoned units may still fire from the structure (residual
        // fire-from-garrison). Docked transport cargo and units mid-enter cannot.
        // weapons_jammed: C++ canFireWeapon DISABLED_SUBDUED residual (ECM field).
        self.is_alive()
            && self.weapon.is_some()
            && !self.is_disabled()
            && !self.status.weapons_jammed
            && !matches!(self.ai_state, AIState::Docked | AIState::Entering)
    }

    /// Authoritative container for docked/garrisoned units.
    /// Prefer `contained_by`; fall back to `target` for legacy enter paths.
    pub fn container_id(&self) -> Option<ObjectId> {
        if let Some(id) = self.contained_by {
            return Some(id);
        }
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.target
        } else {
            None
        }
    }

    /// True when this unit is currently inside a transport or garrison.
    pub fn is_contained(&self) -> bool {
        matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
            || self.contained_by.is_some()
    }

    pub fn is_attackable(&self) -> bool {
        self.is_alive() && self.is_kind_of(KindOf::Attackable)
    }

    pub fn get_position(&self) -> Vec3 {
        if let Some(engine_id) = self.engine_object_id {
            if let Some(pos) = Self::read_engine_position(engine_id) {
                return pos;
            }
        }
        self.thing.get_position()
    }

    fn read_engine_position(engine_id: u32) -> Option<Vec3> {
        let obj = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id)?;
        let guard = obj.read().ok()?;
        let pos = guard.get_position(); // Coord3D is glam::Vec3
        Some(Vec3::new(pos.x, pos.y, pos.z))
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.thing.set_position(position);
        // Propagate position to GameEngine ObjectFactory object so both
        // the lightweight and full engine representations stay in sync.
        if let Some(engine_id) = self.engine_object_id {
            Self::write_engine_position(engine_id, position);
        }
    }

    fn write_engine_position(engine_id: u32, position: Vec3) {
        if let Some(obj) = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id) {
            if let Ok(mut guard) = obj.write() {
                // Convert glam 0.24 Vec3 -> gamelogic Coord3D (glam 0.28)
                let coord = gamelogic::common::Coord3D::new(position.x, position.y, position.z);
                if let Err(err) = guard.set_position(&coord) {
                    log::warn!("failed to synchronize bridge object {engine_id} position: {err}");
                }
            }
        }
    }

    pub fn get_orientation(&self) -> f32 {
        if let Some(engine_id) = self.engine_object_id {
            if let Some(angle) = Self::read_engine_orientation(engine_id) {
                return angle;
            }
        }
        self.thing.get_orientation()
    }

    fn read_engine_orientation(engine_id: u32) -> Option<f32> {
        let obj = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id)?;
        let guard = obj.read().ok()?;
        Some(guard.get_orientation())
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.thing.set_orientation(angle);
        if let Some(engine_id) = self.engine_object_id {
            if let Some(obj) = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id) {
                if let Ok(mut guard) = obj.write() {
                    if let Err(err) = guard.set_orientation(angle) {
                        log::warn!(
                            "failed to synchronize bridge object {engine_id} orientation: {err}"
                        );
                    }
                }
            }
        }
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.thing.get_transform_matrix()
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        if self.status.destroyed {
            return false;
        }

        // Apply armor reduction
        let armor_factor = 1.0 - (self.thing.template.armor / (self.thing.template.armor + 100.0));
        let actual_damage = damage * armor_factor;

        self.health.damage(actual_damage);

        // Check if object is destroyed
        if !self.health.is_alive() {
            self.status.destroyed = true;
            self.ai_state = AIState::Idle;
            self.target = None;
            true // Object was destroyed
        } else {
            false
        }
    }

    pub fn heal(&mut self, amount: f32) {
        if !self.status.destroyed {
            self.health.heal(amount);
        }
    }

    /// C++ residual: STEALTHED && !DETECTED && !DISGUISED (disguise not residual).
    /// Stealthed-and-undetected units are not legal auto/manual attack targets.
    pub fn is_effectively_stealthed(&self) -> bool {
        self.status.stealthed && !self.status.detected
    }

    /// Effective detection radius for this unit when `is_detector`.
    /// C++: DetectionRange if > 0 else vision range.
    pub fn effective_detection_range(&self) -> f32 {
        if self.detection_range > 0.0 {
            self.detection_range
        } else {
            self.get_template().sight_range
        }
    }

    /// Mark this object as detected until `expires_frame` (logic frame exclusive).
    /// C++ StealthUpdate::markAsDetected residual.
    pub fn mark_detected(&mut self, expires_frame: u32) {
        self.status.detected = true;
        // Keep the later expiry if already detected by another scanner.
        if expires_frame > self.detection_expires_frame {
            self.detection_expires_frame = expires_frame;
        }
    }

    /// Clear DETECTED status (stealth may remain active).
    pub fn clear_detected(&mut self) {
        self.status.detected = false;
        self.detection_expires_frame = 0;
    }

    /// Break stealth entirely (fire / script residual).
    pub fn break_stealth(&mut self) {
        self.status.stealthed = false;
        self.status.detected = false;
        self.detection_expires_frame = 0;
    }

    /// C++ Object::setVisionSpied residual (refcounted mask simplified to bitmask).
    /// When on, spying player treats this unit as a temporary looker / revealed target.
    pub fn set_vision_spied_by_player(&mut self, player_id: u32, on: bool) {
        let bit = 1u32 << player_id.min(31);
        if on {
            self.vision_spied_mask |= bit;
        } else {
            self.vision_spied_mask &= !bit;
        }
    }

    /// True if `player_id` currently has vision-spied residual on this unit.
    pub fn is_vision_spied_by_player(&self, player_id: u32) -> bool {
        let bit = 1u32 << player_id.min(31);
        (self.vision_spied_mask & bit) != 0
    }

    /// Whether an enemy of `attacker_team` may target this object.
    /// C++ WeaponSet::getCanAttackObject stealth gate residual.
    pub fn is_targetable_by_enemy_of(&self, attacker_team: Team) -> bool {
        if !self.is_alive() || !self.is_attackable() {
            return false;
        }
        if self.team == attacker_team {
            return false;
        }
        // Stealthed and not detected: not a valid target.
        !self.is_effectively_stealthed()
    }

    /// Whether `weapon` can legally hit `target` (air/ground + range + stealth).
    pub fn can_target_with(&self, target: &Object, weapon: &Weapon) -> bool {
        // C++ WeaponSet: stealthed + undetected cannot be attacked
        // (including force-fire against pure stealth; disguise exception not residual).
        if target.is_effectively_stealthed() && target.team != self.team {
            return false;
        }

        let target_is_air =
            target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

        if target_is_air && !weapon.can_target_air {
            return false;
        }

        if !target_is_air && !weapon.can_target_ground {
            return false;
        }

        // C++ parity (Weapon::isWithinAttackRange): check both minimum
        // and maximum attack range. Ground targets use horizontal (XZ)
        // distance so terrain height does not permanently block fire after
        // a successful march into range.
        let distance = if target_is_air {
            self.thing.get_distance_to(&target.thing)
        } else {
            let a = self.get_position();
            let b = target.get_position();
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            (dx * dx + dz * dz).sqrt()
        };
        if weapon.min_range > 0.0 && distance < weapon.min_range {
            return false;
        }
        distance <= weapon.range
    }

    /// True if primary **or** secondary can currently hit the target.
    pub fn can_target(&self, target: &Object) -> bool {
        if target.is_effectively_stealthed() && target.team != self.team {
            return false;
        }
        if let Some(weapon) = &self.weapon {
            if self.can_target_with(target, weapon) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            if self.can_target_with(target, weapon) {
                return true;
            }
        }
        false
    }

    /// Weapon ready on reload timer (not range).
    pub fn weapon_ready(weapon: &Weapon, current_time: f32) -> bool {
        current_time - weapon.last_fire_time >= weapon.reload_time
    }

    pub fn can_fire(&self, current_time: f32) -> bool {
        // C++ Object::canFireWeapon: DISABLED_SUBDUED / weapons_jammed residual.
        if self.status.weapons_jammed || self.is_disabled() {
            return false;
        }
        if let Some(weapon) = &self.weapon {
            if Self::weapon_ready(weapon, current_time) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            if Self::weapon_ready(weapon, current_time) {
                return true;
            }
        }
        false
    }

    /// Fail-closed residual combat weapon choice (not full AutoChoose/PreferredAgainst).
    ///
    /// Slot: `0` = primary, `1` = secondary.
    /// Rules:
    /// - Player lock (`active_weapon_slot == 1`): prefer secondary when ready + in range.
    /// - PreferredAgainst residual (damage + kind heuristic, not full INI matrix):
    ///   - Structures: prefer secondary when damage ≥ primary (or primary cannot fire).
    ///   - Infantry: prefer secondary when damage > primary (FlashBang residual).
    ///   - Vehicles: prefer secondary when damage > primary (TOW residual).
    ///   - Neutron residual: active secondary with neutron upgrade vs infantry/vehicle
    ///     prefers secondary when player locked or secondary is the only ready slot;
    ///     also when primary cannot fire and secondary is ready.
    /// - Else primary when ready + in range; else secondary (alternate fire residual).
    pub fn select_combat_weapon_slot(
        &self,
        target: &Object,
        current_time: f32,
    ) -> Option<u8> {
        let primary_ok = self.weapon.as_ref().is_some_and(|w| {
            Self::weapon_ready(w, current_time) && self.can_target_with(target, w)
        });
        let secondary_ok = self.secondary_weapon.as_ref().is_some_and(|w| {
            Self::weapon_ready(w, current_time) && self.can_target_with(target, w)
        });

        if !primary_ok && !secondary_ok {
            return None;
        }

        // Manual weapon-slot toggle (command residual).
        if self.active_weapon_slot == 1 {
            if secondary_ok {
                return Some(1);
            }
            if primary_ok {
                return Some(0);
            }
            return None;
        }

        let target_is_structure = target.object_type == ObjectType::Building
            || target.is_kind_of(KindOf::Structure);
        let target_is_infantry = target.is_kind_of(KindOf::Infantry);
        let target_is_vehicle = target.is_kind_of(KindOf::Vehicle)
            && !target.is_kind_of(KindOf::Aircraft);

        let primary_damage = self.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0);
        let secondary_damage = self
            .secondary_weapon
            .as_ref()
            .map(|w| w.damage)
            .unwrap_or(0.0);

        if secondary_ok {
            // PreferredAgainst residual by target kind + relative damage.
            if target_is_structure && (secondary_damage >= primary_damage || !primary_ok) {
                return Some(1);
            }
            if target_is_infantry && (secondary_damage > primary_damage || !primary_ok) {
                // FlashBang residual (35 > 5). Neutron secondary damage is 1.0 so
                // only wins here when primary cannot fire unless slot-locked.
                return Some(1);
            }
            if target_is_vehicle && (secondary_damage > primary_damage || !primary_ok) {
                // TOW residual (30 > 10 Humvee gun).
                return Some(1);
            }
        }

        // Default / alternate: primary first, then secondary if only it is ready.
        if primary_ok {
            Some(0)
        } else if secondary_ok {
            Some(1)
        } else {
            None
        }
    }

    pub fn weapon_slot(&self, slot: u8) -> Option<&Weapon> {
        match slot {
            1 => self.secondary_weapon.as_ref(),
            _ => self.weapon.as_ref(),
        }
    }

    pub fn weapon_slot_mut(&mut self, slot: u8) -> Option<&mut Weapon> {
        match slot {
            1 => self.secondary_weapon.as_mut(),
            _ => self.weapon.as_mut(),
        }
    }

    pub fn fire_at(&mut self, target_id: ObjectId, current_time: f32) -> bool {
        // C++ canFireWeapon residual: jammed / disabled units cannot discharge.
        if self.status.weapons_jammed || self.is_disabled() {
            return false;
        }
        // Prefer the locked/active slot when ready; else primary; else secondary.
        let slot = {
            let prefer_secondary = self.active_weapon_slot == 1;
            let primary_ready = self
                .weapon
                .as_ref()
                .is_some_and(|w| Self::weapon_ready(w, current_time));
            let secondary_ready = self
                .secondary_weapon
                .as_ref()
                .is_some_and(|w| Self::weapon_ready(w, current_time));
            if prefer_secondary && secondary_ready {
                1u8
            } else if primary_ready {
                0u8
            } else if secondary_ready {
                1u8
            } else {
                return false;
            }
        };

        if let Some(weapon) = self.weapon_slot_mut(slot) {
            weapon.last_fire_time = current_time;
            let weapon_damage = weapon.damage;
            let weapon_speed = weapon.projectile_speed;
            let shooter_id = self.id;
            let shooter_pos = self.get_position();
            self.target = Some(target_id);

            super::combat::queue_projectile(super::combat::PendingProjectile {
                shooter_id,
                shooter_pos,
                target_id: Some(target_id),
                target_pos: None,
                damage: weapon_damage,
                speed: weapon_speed,
            });

            // C++ STEALTH_NOT_WHILE_ATTACKING / IS_FIRING_WEAPON residual:
            // firing breaks stealth (default host residual).
            if self.stealth_breaks_on_attack && self.status.stealthed {
                self.break_stealth();
            }
            true
        } else {
            false
        }
    }

    pub fn move_to(&mut self, position: Vec3) {
        if self.is_mobile() && self.is_alive() {
            self.movement.target_position = Some(position);
            self.ai_state = AIState::Moving;
            self.status.moving = true;
        }
    }

    pub fn stop_moving(&mut self) {
        self.movement.target_position = None;
        self.movement.velocity = Vec3::ZERO;
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.status.moving = false;
        // Only pure locomotion returns to Idle when the destination is reached.
        // Interaction states (Capturing, Repairing, SpecialAbility, Entering, …)
        // set a destination while remaining in-state; clobbering them to Idle
        // aborted capture/repair on arrival before support-state resolution.
        if matches!(self.ai_state, AIState::Moving | AIState::AttackMoving) {
            self.ai_state = AIState::Idle;
        }
    }

    pub fn attack_target(&mut self, target_id: ObjectId) {
        if self.can_attack() && self.is_alive() {
            self.target = Some(target_id);
            self.target_location = None;
            self.force_attack = false;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
        }
    }

    pub fn stop_attack(&mut self) {
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.status.attacking = false;
        // C++ parity: guard units return to their guard state after a kill
        // rather than going fully idle. The guard anchor/radius are preserved
        // so the support-states update loop will re-engage nearby enemies.
        if self.guard_target.is_some() {
            self.ai_state = AIState::GuardingObject;
        } else if self.guard_position.is_some() {
            self.ai_state = AIState::GuardingArea;
        } else {
            self.ai_state = AIState::Idle;
        }
    }

    pub fn clear_all_occupants(&mut self) {
        if let Some(building) = self.building_data.as_mut() {
            building.garrisoned_units.clear();
        }
        self.occupants.clear();
    }

    // Command system compatibility methods
    pub fn can_move(&self) -> bool {
        // weapons_jammed intentionally does NOT block movement (weapons-only residual).
        self.is_mobile()
            && self.is_alive()
            && !self.status.disabled_unmanned
            && !self.status.disabled_hacked
            && !self.status.disabled_emp
            && !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
    }

    pub fn set_destination(&mut self, destination: Vec3) {
        self.move_to(destination);
    }

    pub fn set_target(&mut self, target: Option<ObjectId>) {
        self.target = target;
        if target.is_some() {
            self.target_location = None;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
        } else {
            self.target_location = None;
            self.force_attack = false;
            self.ai_state = AIState::Idle;
            self.status.attacking = false;
        }
    }

    /// Check whether this object can fire the requested special power.
    pub fn is_special_power_ready(&self, _power: &SpecialPowerType) -> bool {
        self.is_alive() && self.special_power_ready && self.special_power_cooldown_remaining <= 0.0
    }

    /// Consume a charge for the special power and start cooldown.
    pub fn consume_special_power_charge(&mut self, power: &SpecialPowerType) {
        if !self.is_special_power_ready(power) {
            return;
        }
        self.special_power_ready = false;
        self.special_power_cooldown_remaining = self.special_power_cooldown;
        self.ai_state = AIState::Idle;
    }

    pub fn apply_upgrade_tag(&mut self, upgrade: &str) {
        if !upgrade.is_empty() {
            self.applied_upgrades.insert(upgrade.to_string());
        }
    }

    pub fn has_upgrade_tag(&self, upgrade: &str) -> bool {
        self.applied_upgrades.contains(upgrade)
    }

    pub fn set_target_location(&mut self, location: Option<Vec3>) {
        self.target_location = location;
        if location.is_some() {
            self.target = None;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
        } else {
            self.force_attack = false;
        }
    }

    pub fn set_force_attack(&mut self, force: bool) {
        self.force_attack = force;
    }

    pub fn stop(&mut self) {
        // Stop all current actions
        self.stop_moving();
        self.stop_attack();
    }

    pub fn set_guard_position(&mut self, position: Option<Vec3>) {
        self.guard_position = position;
        if position.is_some() {
            self.ai_state = AIState::GuardingArea;
        }
    }

    pub fn set_guard_target(&mut self, target: Option<ObjectId>) {
        self.guard_target = target;
        if target.is_some() {
            self.ai_state = AIState::GuardingObject;
        }
    }

    pub fn can_repair(&self) -> bool {
        // Repair/build authority should be limited to worker/dozer-style units.
        self.can_move() && self.is_worker()
    }

    pub fn can_construct(&self) -> bool {
        // Construction should be limited to worker/dozer-style units.
        self.can_move() && self.is_worker()
    }

    pub fn can_contain(&self) -> bool {
        if !self.is_alive() {
            return false;
        }
        // China Overlord residual: only containable once BattleBunker residual
        // capacity is installed (Some(n>0)). Without bunker (Some(0)) reject.
        if self.is_overlord_style_container() {
            return self.overlord_bunker_slot_capacity() > 0;
        }
        // Transports: any vehicle may act as a container (host residual).
        // Explicit max_transport=0 still allows footprint residual capacity.
        if self.is_kind_of(KindOf::Vehicle) {
            return true;
        }
        // Structures: only garrisonable buildings with residual capacity > 0.
        // Fail-closed: faction producers / non-bunker structures reject Enter.
        if self.is_kind_of(KindOf::Structure) {
            return self
                .building_data
                .as_ref()
                .map(|b| b.max_garrison > 0)
                .unwrap_or(false);
        }
        false
    }

    pub fn has_capacity_for(&self, count: usize) -> bool {
        if let Some(building) = &self.building_data {
            if building.max_garrison == 0 {
                return false;
            }
            building.garrisoned_units.len() + count <= building.max_garrison
        } else if self.is_kind_of(KindOf::Vehicle) {
            let cap = self.transport_capacity();
            if cap == 0 {
                return false;
            }
            self.occupants.len() + count <= cap
        } else {
            false
        }
    }

    /// Residual garrison capacity (structures only). 0 = not garrisonable.
    pub fn garrison_capacity(&self) -> usize {
        self.building_data
            .as_ref()
            .map(|b| b.max_garrison)
            .unwrap_or(0)
    }

    /// True when this vehicle uses OverlordContain residual semantics
    /// (`overlord_bunker_capacity` is `Some(...)`).
    pub fn is_overlord_style_container(&self) -> bool {
        self.overlord_bunker_capacity.is_some()
    }

    /// Residual BattleBunker infantry slots on an Overlord-style vehicle.
    /// `0` when not overlord-style or bunker residual not installed.
    pub fn overlord_bunker_slot_capacity(&self) -> usize {
        self.overlord_bunker_capacity.unwrap_or(0)
    }

    /// Install residual BattleBunker capacity (C++ OCL_OverlordBattleBunker →
    /// ChinaTankOverlordBattleBunker TransportContain Slots=5).
    /// Fail-closed: does not spawn a real portable-structure passenger object.
    pub fn install_overlord_battle_bunker(&mut self, slots: usize) {
        self.overlord_bunker_capacity = Some(slots);
    }

    /// Residual transport capacity (vehicles). Overlord bunker residual wins,
    /// then explicit `max_transport`, else footprint heuristic. Structures return 0.
    pub fn transport_capacity(&self) -> usize {
        if self.is_kind_of(KindOf::Structure) {
            return 0;
        }
        if !self.is_kind_of(KindOf::Vehicle) {
            return 0;
        }
        // Overlord BattleBunker residual: bunker slots only (0 without bunker).
        if let Some(cap) = self.overlord_bunker_capacity {
            return cap;
        }
        if self.max_transport > 0 {
            return self.max_transport;
        }
        // Transport heuristic based on footprint: larger selection radius holds more.
        let base_cap = (self.selection_radius / 8.0).ceil() as usize + 2;
        base_cap.clamp(2, 12)
    }

    /// Current transport occupant count (vehicles only; structures use garrison).
    pub fn transport_count(&self) -> usize {
        if self.is_kind_of(KindOf::Structure) {
            0
        } else {
            self.occupants.len()
        }
    }

    /// Current garrison/transport occupant count.
    pub fn garrison_count(&self) -> usize {
        self.contained_units().len()
    }

    pub fn add_occupant(&mut self, unit_id: ObjectId) -> bool {
        if !self.can_contain() || !self.has_capacity_for(1) {
            return false;
        }
        if let Some(building) = self.building_data.as_mut() {
            if building.garrisoned_units.contains(&unit_id) {
                return true;
            }
            building.garrisoned_units.push(unit_id);
            true
        } else {
            if self.occupants.contains(&unit_id) {
                return true;
            }
            self.occupants.push(unit_id);
            true
        }
    }

    pub fn contained_units(&self) -> Vec<ObjectId> {
        if let Some(building) = &self.building_data {
            building.garrisoned_units.clone()
        } else {
            self.occupants.clone()
        }
    }

    pub fn remove_occupant(&mut self, unit_id: ObjectId) -> bool {
        if let Some(building) = self.building_data.as_mut() {
            if let Some(pos) = building
                .garrisoned_units
                .iter()
                .position(|&id| id == unit_id)
            {
                building.garrisoned_units.remove(pos);
                return true;
            }
        }
        if let Some(pos) = self.occupants.iter().position(|&id| id == unit_id) {
            self.occupants.remove(pos);
            return true;
        }
        false
    }

    /// Begin containing an occupant (transport/garrison bookkeeping).
    pub fn enter_transport(&mut self, unit_id: ObjectId) -> bool {
        self.add_occupant(unit_id)
    }

    /// Remove an occupant from this transport/garrison.
    pub fn exit_transport(&mut self, unit_id: ObjectId) -> bool {
        self.remove_occupant(unit_id)
    }

    pub fn tick_timers(&mut self, dt: f32) {
        if self.cheer_timer > 0.0 {
            self.cheer_timer -= dt;
            if self.cheer_timer <= 0.0 && self.ai_state == AIState::SpecialAbility {
                self.ai_state = AIState::Idle;
                self.cheer_timer = 0.0;
            }
        }

        if self.special_power_cooldown_remaining > 0.0 {
            self.special_power_cooldown_remaining =
                (self.special_power_cooldown_remaining - dt).max(0.0);
            if self.special_power_cooldown_remaining <= 0.0 {
                self.special_power_ready = true;
            }
        }
    }

    pub fn update_construction(&mut self, dt: f32) {
        if self.status.under_construction {
            let build_rate = 1.0 / self.thing.template.build_time;
            self.construction_percent += build_rate * dt;

            if self.construction_percent >= 1.0 {
                self.construction_percent = 1.0;
                self.status.under_construction = false;
                self.health.current = self.health.maximum;
            } else {
                // Health scales with construction progress
                self.health.current = self.health.maximum * (0.1 + 0.9 * self.construction_percent);
            }
        }
    }

    pub fn update_movement(&mut self, dt: f32) {
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.movement.target_position = None;
            self.movement.velocity = Vec3::ZERO;
            return;
        }

        if let Some(target_pos) = self.movement.target_position {
            let current_pos = self.get_position();
            let direction = (target_pos - current_pos).normalize_or_zero();

            if direction.length() > 0.0 {
                // Update velocity
                let target_velocity = direction * self.movement.max_speed;
                let velocity_diff = target_velocity - self.movement.velocity;
                let max_accel = self.movement.acceleration * dt;

                if velocity_diff.length() <= max_accel {
                    self.movement.velocity = target_velocity;
                } else {
                    self.movement.velocity += velocity_diff.normalize() * max_accel;
                }

                // Update position
                let new_position = current_pos + self.movement.velocity * dt;
                self.set_position(new_position);

                // Update orientation to face movement direction
                if self.movement.velocity.length() > 0.1 {
                    let desired_angle = (-self.movement.velocity.z).atan2(self.movement.velocity.x);
                    let current_angle = self.get_orientation();
                    let angle_diff = desired_angle - current_angle;

                    // Normalize angle difference
                    let angle_diff = ((angle_diff + std::f32::consts::PI)
                        % (2.0 * std::f32::consts::PI))
                        - std::f32::consts::PI;

                    let max_turn = self.movement.turn_rate * dt;
                    let new_angle = if angle_diff.abs() <= max_turn {
                        desired_angle
                    } else {
                        current_angle + max_turn * angle_diff.signum()
                    };

                    self.set_orientation(new_angle);
                }

                // Check if we've reached the target
                let distance_to_target = current_pos.distance(target_pos);
                if distance_to_target < 2.0 {
                    // C++ parity: advance to the next waypoint in the path if one
                    // exists, otherwise stop moving.
                    let next_waypoint =
                        if self.movement.current_path_index + 1 < self.movement.path.len() {
                            self.movement.current_path_index += 1;
                            Some(self.movement.path[self.movement.current_path_index])
                        } else {
                            None
                        };

                    if let Some(waypoint) = next_waypoint {
                        self.movement.target_position = Some(waypoint);
                    } else {
                        self.stop_moving();
                    }
                }
            } else {
                self.stop_moving();
            }
        }
    }

    pub fn gain_experience(&mut self, amount: f32) {
        self.experience.current += amount;

        // C++ parity: veterancy thresholds are per-template (Object::ExperienceValues
        // in INI).  Use template-defined thresholds, falling back to defaults.
        let thresholds = self.thing.template.veterancy_xp_thresholds;

        // Check for level up
        let previous_level = self.experience.level;
        let new_level = if self.experience.current >= thresholds[2] {
            VeterancyLevel::Heroic
        } else if self.experience.current >= thresholds[1] {
            VeterancyLevel::Elite
        } else if self.experience.current >= thresholds[0] {
            VeterancyLevel::Veteran
        } else {
            VeterancyLevel::Rookie
        };

        if new_level != previous_level {
            self.experience.level = new_level;
            // Apply veterancy bonuses
            self.apply_veterancy_bonuses(previous_level, new_level);
        }
    }

    /// C++ parity (GameData.ini veterancy bonuses):
    ///   Veteran: +10% dmg, +20% RoF, +20% HP
    ///   Elite:   +20% dmg, +40% RoF, +30% HP
    ///   Heroic:  +30% dmg, +60% RoF, +50% HP
    /// Returns (health_multiplier, damage_multiplier, rof_multiplier).
    fn veterancy_bonuses(level: VeterancyLevel) -> (f32, f32, f32) {
        match level {
            VeterancyLevel::Rookie => (1.0, 1.0, 1.0),
            VeterancyLevel::Veteran => (1.2, 1.1, 1.0 / 1.2), // +20% RoF
            VeterancyLevel::Elite => (1.3, 1.2, 1.0 / 1.4),   // +40% RoF
            VeterancyLevel::Heroic => (1.5, 1.3, 1.0 / 1.6),  // +60% RoF
        }
    }

    fn apply_veterancy_bonuses(
        &mut self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
    ) {
        let (_old_health_bonus, old_damage_bonus, old_rof_bonus) =
            Self::veterancy_bonuses(previous_level);
        let (health_bonus, damage_bonus, rof_bonus) = Self::veterancy_bonuses(new_level);

        // Apply health bonus
        let base_health = self.thing.template.max_health;
        let old_max_health = self.health.maximum.max(1.0);
        let health_ratio = (self.health.current / old_max_health).clamp(0.0, 1.0);
        self.health.maximum = base_health * health_bonus;
        self.health.current = (self.health.maximum * health_ratio).clamp(0.0, self.health.maximum);

        // Apply weapon damage and rate-of-fire bonuses
        if let Some(weapon) = &mut self.weapon {
            let dmg_scale = if old_damage_bonus > 0.0 {
                damage_bonus / old_damage_bonus
            } else {
                1.0
            };
            weapon.damage *= dmg_scale;
            // C++ parity: RoF bonus reduces reload time (faster firing).
            // Scale relative to previous level so multi-level transitions work.
            let rof_scale = rof_bonus / old_rof_bonus;
            weapon.reload_time *= rof_scale;
        }
    }

    pub fn select(&mut self) {
        if self.is_selectable() {
            self.selected = true;
            self.status.selected = true;
        }
    }

    pub fn deselect(&mut self) {
        self.selected = false;
        self.status.selected = false;
    }

    /// Set the AI state for autonomous behavior
    pub fn set_ai_state(&mut self, state: AIState) {
        self.ai_state = state;
    }

    /// Get visual information for rendering
    pub fn get_visual_info(&self) -> ObjectVisualInfo {
        ObjectVisualInfo {
            position: self.get_position(),
            orientation: self.get_orientation(),
            team_color: self.team_color,
            selection_radius: self.selection_radius,
            is_selected: self.selected,
            show_health_bar: self.show_health_bar && self.is_alive(),
            health_percentage: self.get_health_percentage(),
            model_name: self.thing.template.model_name.clone(),
            object_type: self.object_type,
            team: self.team,
            under_construction: self.status.under_construction,
            construction_percent: self.construction_percent,
        }
    }

    /// Update team color (useful for changing allegiance)
    pub fn set_team(&mut self, team: Team) {
        self.team = team;
        self.team_color = team.get_color();
    }

    /// Check if this object is visible to a team (for fog of war / targeting UI).
    /// C++ residual: stealthed-and-undetected units are hidden from non-allied teams.
    /// Detected stealthed units become visible (and targetable).
    pub fn is_visible_to_team(&self, team: Team) -> bool {
        // Team-local baseline visibility check. Global shroud/fog filtering is applied by
        // higher-level visibility queries in GameLogic that have object IDs and player context.
        if team == self.team {
            return true;
        }
        !self.is_effectively_stealthed()
    }

    /// Get a description string for UI display.
    /// C++ parity: prefers per-object name override, then template display
    /// name (from INI DisplayName), then template internal name.
    pub fn get_display_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let tmpl_display = &self.thing.template.display_name;
        if !tmpl_display.is_empty() && tmpl_display != &self.template_name {
            return tmpl_display.clone();
        }
        self.template_name.clone()
    }
}

/// Visual information structure for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectVisualInfo {
    pub position: Vec3,
    pub orientation: f32,
    pub team_color: [f32; 4],
    pub selection_radius: f32,
    pub is_selected: bool,
    pub show_health_bar: bool,
    pub health_percentage: f32,
    pub model_name: Option<String>,
    pub object_type: ObjectType,
    pub team: Team,
    pub under_construction: bool,
    pub construction_percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_object() -> Object {
        let template = ThingTemplate::new("TestUnit");
        let mut object = Object::new(template, ObjectId(1), Team::USA);
        object.weapon = Some(Weapon {
            damage: 100.0,
            ..Weapon::default()
        });
        object
    }

    #[test]
    fn veterancy_increases_weapon_damage() {
        let mut object = make_test_object();
        object.gain_experience(60.0); // Veteran → +10% dmg
        let veteran_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
        assert!((veteran_damage - 110.0).abs() < 0.01);

        object.gain_experience(90.0); // Elite → +20% dmg (total)
        let elite_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
        assert!((elite_damage - 120.0).abs() < 0.01);
    }

    #[test]
    fn veterancy_preserves_health_ratio_when_max_health_changes() {
        let mut object = make_test_object();
        object.health.current = 50.0;
        object.health.maximum = 100.0;

        object.gain_experience(60.0); // Veteran → +20% HP
        assert!((object.health.maximum - 120.0).abs() < 0.01);
        assert!((object.health.current - 60.0).abs() < 0.01);
    }

    #[test]
    fn stop_attack_clears_force_attack_and_targets() {
        let mut object = make_test_object();
        object.set_target(Some(ObjectId(99)));
        object.set_force_attack(true);
        object.set_target_location(Some(Vec3::new(1.0, 0.0, 2.0)));
        object.stop_attack();

        assert!(object.target.is_none());
        assert!(object.target_location.is_none());
        assert!(!object.force_attack);
        assert!(!object.status.attacking);
    }

    #[test]
    fn setting_target_location_clears_object_target() {
        let mut object = make_test_object();
        object.set_target(Some(ObjectId(77)));
        object.set_target_location(Some(Vec3::new(10.0, 0.0, 10.0)));

        assert!(object.target.is_none());
        assert!(object.target_location.is_some());
        assert!(object.status.attacking);
    }

    #[test]
    fn effectively_stealthed_blocks_enemy_visibility_and_targeting() {
        let mut stealthed = make_test_object();
        stealthed.team = Team::USA;
        stealthed.status.stealthed = true;
        stealthed.status.detected = false;
        stealthed.thing.template.add_kind_of(KindOf::Attackable);

        assert!(stealthed.is_effectively_stealthed());
        assert!(stealthed.is_visible_to_team(Team::USA));
        assert!(!stealthed.is_visible_to_team(Team::China));
        assert!(!stealthed.is_targetable_by_enemy_of(Team::China));

        stealthed.status.detected = true;
        assert!(!stealthed.is_effectively_stealthed());
        assert!(stealthed.is_visible_to_team(Team::China));
        assert!(stealthed.is_targetable_by_enemy_of(Team::China));
    }

    #[test]
    fn fire_at_breaks_stealth_when_forbidden_while_attacking() {
        let mut object = make_test_object();
        object.status.stealthed = true;
        object.stealth_breaks_on_attack = true;
        object.weapon = Some(Weapon {
            damage: 100.0,
            range: 100.0,
            reload_time: 0.5,
            last_fire_time: -1.0,
            ..Weapon::default()
        });
        assert!(object.fire_at(ObjectId(2), 0.0));
        assert!(!object.status.stealthed);
        assert!(!object.status.detected);
    }

    #[test]
    fn can_target_rejects_undetected_stealthed_enemy() {
        let mut attacker = make_test_object();
        attacker.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });

        let mut target = make_test_object();
        target.id = ObjectId(2);
        target.team = Team::China;
        target.status.stealthed = true;
        target.status.detected = false;
        target.set_position(Vec3::new(5.0, 0.0, 0.0));

        assert!(!attacker.can_target(&target));

        target.status.detected = true;
        assert!(attacker.can_target(&target));
    }
}

