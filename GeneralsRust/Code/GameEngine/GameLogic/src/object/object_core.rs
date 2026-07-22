//! Object core - struct definition, construction, destruction, and basic identification.

use std::fmt;
use std::sync::{Arc, Mutex, RwLock, Weak};

use game_engine::common::thing::module::ModuleInterfaceType;

use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
use crate::common::{
    AsciiString, Bool, Color, Coord2D, Coord3D, DisabledMaskType, FormationID, GeometryInfo,
    ICoord3D, KindOf, ObjectID, ObjectStatusMaskType, PathfindLayerEnum, PlayerMaskType, Real,
    UnsignedInt, UpgradeMaskType, VeterancyLevel, WeaponBonusConditionFlags,
};
use crate::experience::ExperienceTracker;
use crate::helpers::{FiringTracker, ObjectHeldHelper};
use crate::modules::{
    AIUpdateInterface, BehaviorModuleInterface, BodyModuleInterface, ContainModuleInterface,
    PhysicsBehavior, UpdateModulePtr,
};
use crate::object::drawable::Drawable;
use crate::object::helper::{
    ObjectDefectionHelper, ObjectRepulsorHelper, ObjectSMCHelper, ObjectWeaponStatusHelper,
    StatusDamageHelper, SubdualDamageHelper, TempWeaponBonusHelper,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_types::SpecialPowerMask;
use crate::object::weapon_set::WeaponSet;
use crate::object::weapon_set::WeaponSetFlags;
use crate::team::Team;
use crate::template::ObjectTemplate;

use super::{
    ArmorSetFlagBits, ModuleEntry, PartitionData, RadarObject, SightingInfo, TriggerInfo,
    CONSTRUCTION_COMPLETE, DISABLED_COUNT, INVALID_ID, MAX_PLAYER_COUNT, MAX_TRIGGER_AREA_INFOS,
    NEVER, WEAPONSLOT_COUNT,
};

/// Main Object struct - the core game entity

pub struct Object {
    // Core identification
    id: ObjectID,
    producer_id: ObjectID,
    builder_id: ObjectID,
    name: AsciiString,
    thing_template: Arc<dyn ThingTemplate>,

    // Linked list pointers for efficient iteration
    next: Option<Arc<RwLock<Object>>>,
    prev: Option<Weak<RwLock<Object>>>,

    // Status and state
    status: ObjectStatusMaskType,
    private_status: u8,
    script_status: u8,

    // Geometry and position
    geometry_info: GeometryInfo,
    health_box_offset: Coord3D,
    i_pos: ICoord3D,

    // Team and ownership
    team: Option<Arc<RwLock<Team>>>,
    original_team_name: AsciiString,
    indicator_color: Color,

    // Modules - using Arc<Mutex<>> for thread safety
    behaviors: Vec<Arc<Mutex<dyn BehaviorModuleInterface>>>,
    modules: Vec<Arc<ModuleEntry>>,
    body_module_handles: Vec<Arc<ModuleEntry>>,
    die_module_handles: Vec<Arc<ModuleEntry>>,
    update_module_handles: Vec<Arc<ModuleEntry>>,
    update_module_registrations: Vec<UpdateModulePtr>,
    collide_module_handles: Vec<Arc<ModuleEntry>>,
    contain_module_handles: Vec<Arc<ModuleEntry>>,
    upgrade_module_handles: Vec<Arc<ModuleEntry>>,
    body: Option<Arc<Mutex<dyn BodyModuleInterface>>>,
    contain: Option<Arc<Mutex<dyn ContainModuleInterface>>>,
    stealth: Option<StealthUpdateHandle>,
    ai: Option<Arc<Mutex<dyn AIUpdateInterface>>>,
    physics: Option<Arc<Mutex<dyn PhysicsBehavior>>>,

    // Helper modules
    repulsor_helper: Option<Arc<Mutex<ObjectRepulsorHelper>>>,
    smc_helper: Option<Arc<Mutex<ObjectSMCHelper>>>,
    ws_helper: Option<Arc<Mutex<ObjectWeaponStatusHelper>>>,
    defection_helper: Option<Arc<Mutex<ObjectDefectionHelper>>>,
    status_damage_helper: Option<Arc<Mutex<StatusDamageHelper>>>,
    subdual_damage_helper: Option<Arc<Mutex<SubdualDamageHelper>>>,
    temp_weapon_bonus_helper: Option<Arc<Mutex<TempWeaponBonusHelper>>>,
    firing_tracker: Option<Arc<Mutex<FiringTracker>>>,
    held_helper: Option<Arc<Mutex<ObjectHeldHelper>>>,

    // Spatial and partition data
    partition_data: Option<Arc<Mutex<PartitionData>>>,
    radar_data: Option<Arc<Mutex<RadarObject>>>,

    // Vision and detection
    partition_last_look: SightingInfo,
    partition_reveal_all_last_look: SightingInfo,
    partition_last_shroud: SightingInfo,
    partition_last_threat: SightingInfo,
    partition_last_value: SightingInfo,
    vision_spied_by: [i32; MAX_PLAYER_COUNT],
    vision_spied_mask: PlayerMaskType,
    vision_range: Real,
    shroud_clearing_range: Real,
    shroud_range: Real,

    // Containment
    contained_by: Option<Weak<RwLock<Object>>>,
    xfer_contained_by_id: ObjectID,
    contained_by_frame: UnsignedInt,
    is_transporting: Bool,

    // Construction and upgrades
    construction_percent: Real,
    object_upgrades_completed: UpgradeMaskType,

    // Group membership
    group_id: Option<u32>,

    // Experience and combat
    experience_tracker: Option<Arc<Mutex<ExperienceTracker>>>,
    captured: bool,
    veterancy_level: VeterancyLevel,
    experience_points: Real,

    // Weapons and combat
    pub weapon_set: WeaponSet,
    /// Multiplicative weapon bonus (e.g., upgrades/veterancy). 1.0 = none.
    weapon_bonus_multiplier: f32,
    cur_weapon_set_flags: WeaponSetFlags,
    armor_set_flags: ArmorSetFlagBits,
    weapon_bonus_condition: WeaponBonusConditionFlags,
    last_weapon_condition: [u8; WEAPONSLOT_COUNT],
    special_power_bits: SpecialPowerMask,

    // Healing tracking (for non-stacking healers)
    sole_healing_benefactor_id: ObjectID,
    sole_healing_benefactor_expiration_frame: UnsignedInt,

    // Disabled states
    disabled_mask: DisabledMaskType,
    disabled_till_frame: [UnsignedInt; DISABLED_COUNT],
    smc_until: UnsignedInt,
    special_model_condition_flag: ModelConditionFlags,
    invulnerable_until_frame: UnsignedInt,

    // Trigger areas
    trigger_info: [TriggerInfo; MAX_TRIGGER_AREA_INFOS],
    entered_or_exited_frame: UnsignedInt,
    num_trigger_areas_active: u8,

    // Pathfinding
    layer: PathfindLayerEnum,
    destination_layer: PathfindLayerEnum,

    // Formation
    formation_id: FormationID,
    formation_offset: Coord2D,

    // Command overrides
    command_set_string_override: AsciiString,

    // Rendering
    safe_occlusion_frame: UnsignedInt,
    carrier_deck_height: Real,

    // Drawable association
    drawable: Option<Arc<RwLock<Drawable>>>,

    // Visibility flags for rendering (per-player fog-of-war)
    // Track which players can see this object for rendering optimization
    visibility_flags: [bool; MAX_PLAYER_COUNT],
    visibility_alpha: [f32; MAX_PLAYER_COUNT], // Alpha blending for partial visibility
    last_visibility_update_frame: UnsignedInt,

    // Flags
    is_selectable: bool,
    modules_ready: bool,
    single_use_command_used: bool,
    is_receiving_difficulty_bonus: bool,

    /// Guard flag to prevent double destruction when `on_destroy()` is called
    /// both explicitly and via `Drop`.
    destroyed: bool,

    #[cfg(any(debug_assertions, feature = "internal"))]
    has_died_already: bool,
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Object")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("template", &self.thing_template.get_name())
            .finish()
    }
}

impl Object {
    /// Creates a new Object instance with no predetermined ID.
    pub fn new(
        thing_template: Arc<dyn ThingTemplate>,
        object_status_mask: ObjectStatusMaskType,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<Arc<RwLock<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_id(thing_template, INVALID_ID, object_status_mask, team)
    }

    /// Creates a new Object instance with a specified object ID.
    pub fn new_with_id(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<Arc<RwLock<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let obj = Self::new_raw(
            thing_template.clone(),
            object_id,
            object_status_mask,
            team.clone(),
        );

        let object_arc = Arc::new(RwLock::new(obj));

        {
            let mut guard = object_arc
                .write()
                .map_err(|_| "object lock poisoned during initialization")?;
            guard.set_team(team)?;
        }

        if object_id != INVALID_ID {
            OBJECT_REGISTRY.register_object(object_id, &object_arc);
            register_legacy_object(&object_arc);
        }

        if let Err(err) = Self::init_modules_for(&object_arc, &thing_template) {
            if object_id != INVALID_ID {
                OBJECT_REGISTRY.unregister_object(object_id);
                unregister_legacy_object(object_id);
            }
            return Err(err);
        }

        Ok(object_arc)
    }

    /// Creates a raw Object instance (internal use)
    pub fn new_raw(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Self {
        Self {
            id: object_id,
            producer_id: INVALID_ID,
            builder_id: INVALID_ID,
            name: AsciiString::new(),
            thing_template: Arc::clone(&thing_template),

            next: None,
            prev: None,

            status: object_status_mask,
            private_status: 0,
            script_status: 0,

            geometry_info: thing_template.get_template_geometry_info(),
            health_box_offset: Coord3D::new(0.0, 0.0, 0.0),
            i_pos: ICoord3D::ZERO,

            team, // Note: mirror C++ weak ref handoff once team lifetime rules are restored
            original_team_name: AsciiString::new(),
            indicator_color: Color::default(),

            behaviors: Vec::new(),
            modules: Vec::new(),
            body_module_handles: Vec::new(),
            die_module_handles: Vec::new(),
            update_module_handles: Vec::new(),
            update_module_registrations: Vec::new(),
            collide_module_handles: Vec::new(),
            contain_module_handles: Vec::new(),
            upgrade_module_handles: Vec::new(),
            body: None,
            contain: None,
            stealth: None,
            ai: None,
            physics: None,

            repulsor_helper: None,
            smc_helper: None,
            ws_helper: None,
            defection_helper: None,
            status_damage_helper: None,
            subdual_damage_helper: None,
            temp_weapon_bonus_helper: None,
            firing_tracker: None,
            held_helper: None,

            partition_data: None,
            radar_data: None,

            partition_last_look: SightingInfo::new(),
            partition_reveal_all_last_look: SightingInfo::new(),
            partition_last_shroud: SightingInfo::new(),
            partition_last_threat: SightingInfo::new(),
            partition_last_value: SightingInfo::new(),
            vision_spied_by: [0; MAX_PLAYER_COUNT],
            vision_spied_mask: PlayerMaskType::none(),
            vision_range: thing_template.calc_vision_range(),
            shroud_clearing_range: {
                let range = thing_template.calc_shroud_clearing_range();
                if range < 0.0 {
                    thing_template.calc_vision_range()
                } else {
                    range
                }
            },
            shroud_range: 0.0,

            contained_by: None,
            xfer_contained_by_id: INVALID_ID,
            contained_by_frame: 0,
            is_transporting: false,

            construction_percent: CONSTRUCTION_COMPLETE,
            object_upgrades_completed: UpgradeMaskType::none(),

            group_id: None,
            experience_tracker: None,
            captured: false,
            veterancy_level: VeterancyLevel::Regular,
            experience_points: 0.0,

            weapon_set: WeaponSet::new(),
            weapon_bonus_multiplier: 1.0,
            cur_weapon_set_flags: WeaponSetFlags::new(),
            armor_set_flags: ArmorSetFlagBits::default(),
            weapon_bonus_condition: WeaponBonusConditionFlags::empty(),
            last_weapon_condition: [0; WEAPONSLOT_COUNT],
            special_power_bits: SpecialPowerMask::default(),

            sole_healing_benefactor_id: INVALID_ID,
            sole_healing_benefactor_expiration_frame: NEVER,

            disabled_mask: DisabledMaskType::none(),
            disabled_till_frame: [NEVER; DISABLED_COUNT],
            smc_until: NEVER,
            special_model_condition_flag: ModelConditionFlags::empty(),
            invulnerable_until_frame: 0,

            trigger_info: Default::default(),
            entered_or_exited_frame: 0,
            num_trigger_areas_active: 0,

            layer: PathfindLayerEnum::Ground,
            destination_layer: PathfindLayerEnum::Ground,

            formation_id: FormationID::NONE,
            formation_offset: Coord2D::ZERO,

            command_set_string_override: AsciiString::new(),
            safe_occlusion_frame: 0,
            carrier_deck_height: 0.0,
            drawable: None,

            // Initialize visibility flags - by default all players see the object (will be updated by rendering)
            visibility_flags: [true; MAX_PLAYER_COUNT],
            visibility_alpha: [1.0; MAX_PLAYER_COUNT],
            last_visibility_update_frame: 0,

            is_selectable: thing_template.is_kind_of(KindOf::Selectable),
            modules_ready: false,
            single_use_command_used: false,
            is_receiving_difficulty_bonus: false,
            destroyed: false,

            #[cfg(any(debug_assertions, feature = "internal"))]
            has_died_already: false,
        }
    }

    /// Initialize object after creation
    pub fn init_object(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = self.id;
        for module in self.modules_with_interface(ModuleInterfaceType::CREATE) {
            module.with_module(|module| {
                if let Some(create) = module.get_create_interface() {
                    create.on_create();
                } else {
                    log::debug!(
                        "Object {} module '{}' advertises CREATE but has no create interface",
                        object_id,
                        module.get_module_name_key()
                    );
                }
            });
        }

        if self.firing_tracker.is_none()
            && !self.has_firing_tracker_module()
            && self.weapon_set.has_any_weapons()
        {
            self.firing_tracker = Some(Arc::new(Mutex::new(FiringTracker::new(self.id))));
        }

        Ok(())
    }

    /// Notify create modules that construction has completed.
    pub fn on_build_complete(&mut self) {
        let object_id = self.id;
        for module in self.modules_with_interface(ModuleInterfaceType::CREATE) {
            module.with_module(|module| {
                if let Some(create) = module.get_create_interface() {
                    if create.should_do_on_build_complete() {
                        create.on_build_complete();
                    }
                } else {
                    log::debug!(
                        "Object {} module '{}' advertises CREATE but has no create interface",
                        object_id,
                        module.get_module_name_key()
                    );
                }
            });
        }
    }

    /// Called during object destruction
    pub fn on_destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;

        let _ = crate::scripting::engine::get_named_object_tracker().unregister_object(self.id);

        for module in self.update_module_registrations.drain(..) {
            let _ = crate::helpers::TheGameLogic::unregister_update_module(self.id, module);
        }

        self.on_destroy_internal();
    }

    /// Internal destroy routine that performs per-object module cleanup
    /// without touching the global `GameLogic` instance directly.
    pub(crate) fn on_destroy_internal(&mut self) {
        // C++ counterpart releases containment before running module onDelete.
        if let Some(container_weak) = self.contained_by.take() {
            if let Some(container_arc) = container_weak.upgrade() {
                if let Ok(container_read) = container_arc.read() {
                    if let Some(contain_module) = container_read.get_contain() {
                        if let Ok(mut contain_guard) = contain_module.lock() {
                            let _ = contain_guard.release_object(self.id);
                        }
                    }
                }

                let _ = self.on_removed_from(container_arc);
            }
        }

        self.upgrade_module_handles.clear();

        let mut modules = std::mem::take(&mut self.modules);
        for entry in modules.drain(..) {
            entry.with_module(|module| module.on_delete());
        }
        self.modules = modules;

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.clear_modules();
            }
        }
        self.drawable = None;

        // Match C++ Object::onDestroy -> handlePartitionCellMaintenance.
        // This clears partition/shroud/value/threat bookkeeping before the object is fully removed.
        self.handle_partition_cell_maintenance();

        self.modules_ready = false;
    }

    // Core identification methods
    pub fn get_id(&self) -> ObjectID {
        self.id
    }

    pub fn get_object_id(&self) -> ObjectID {
        self.id
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    pub fn set_receiving_difficulty_bonus(&mut self, value: bool) {
        self.is_receiving_difficulty_bonus = value;
    }

    pub fn is_receiving_difficulty_bonus(&self) -> bool {
        self.is_receiving_difficulty_bonus
    }

    // Linked list navigation
    pub fn get_next_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.next.clone()
    }

    // Producer/Builder relationships
    pub fn get_producer_id(&self) -> ObjectID {
        self.producer_id
    }

    pub fn set_producer(&mut self, obj: Option<&Object>) {
        self.producer_id = obj.map(|o| o.get_id()).unwrap_or(INVALID_ID);
    }

    pub fn get_builder_id(&self) -> ObjectID {
        self.builder_id
    }

    pub fn set_builder(&mut self, obj: Option<&Object>) {
        self.builder_id = obj.map(|o| o.get_id()).unwrap_or(INVALID_ID);
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        self.on_destroy();
    }
}
