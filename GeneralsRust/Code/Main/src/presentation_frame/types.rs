use super::*;

/// Logic-frame index (30 Hz authority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogicFrame(pub u32);

/// ControlBar production cameo CanMake residual frozen for presentation/UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationCanMakeCameo {
    pub template_name: String,
    /// C++ CanMakeType ordinal residual (CANMAKE_*).
    pub can_make: u32,
    /// True when CANMAKE_OK residual.
    pub available: bool,
    /// Optional HelpBox status message residual (None when OK / silent statuses).
    pub help_status: Option<String>,
    /// C++ ThingTemplate::getBuildable() NO / ONLY_BY_AI hide residual.
    #[serde(default)]
    pub buildable_hidden: bool,
}

/// Snapshot-owned factory production queue entry (host BuildingData residual).
/// Fail-closed: not full ControlBar queue UI / cancel-button WND parity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationProductionItem {
    pub template_name: String,
    /// Absolute research/build progress seconds residual.
    pub progress: f32,
    pub total_time: f32,
    pub cost_supplies: u32,
    /// C++ PRODUCTION_UPGRADE residual on producer queue.
    pub is_upgrade: bool,
    /// Normalized 0..1 residual for ControlBar / build-queue strip.
    pub progress_ratio: f32,
}

impl PresentationProductionItem {
    #[inline]
    pub fn from_host_item(item: &crate::game_logic::buildings::ProductionItem) -> Self {
        let ratio = if item.total_time <= 0.0 {
            1.0
        } else {
            (item.progress / item.total_time).clamp(0.0, 1.0)
        };
        Self {
            template_name: item.template_name.clone(),
            progress: item.progress,
            total_time: item.total_time,
            cost_supplies: item.cost.supplies,
            is_upgrade: item.is_upgrade(),
            progress_ratio: ratio,
        }
    }

    /// Wave 489: GameWorld entity production queue → presentation strip.
    #[inline]
    pub fn from_entity_item(item: &gamelogic::world::entities::EntityProductionItem) -> Self {
        let ratio = if item.total_time <= 0.0 {
            1.0
        } else {
            (item.progress / item.total_time).clamp(0.0, 1.0)
        };
        Self {
            template_name: item.template_name.clone(),
            progress: item.progress,
            total_time: item.total_time,
            cost_supplies: item.cost_supplies,
            is_upgrade: item.is_upgrade,
            progress_ratio: ratio,
        }
    }
}

/// Snapshot-owned veterancy rank (host Experience residual).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationVeterancy {
    Rookie,
    Veteran,
    Elite,
    Heroic,
}

impl PresentationVeterancy {
    pub fn from_host(level: crate::game_logic::VeterancyLevel) -> Self {
        use crate::game_logic::VeterancyLevel as V;
        match level {
            V::Rookie => Self::Rookie,
            V::Veteran => Self::Veteran,
            V::Elite => Self::Elite,
            V::Heroic => Self::Heroic,
        }
    }

    /// Wave 490: GameWorld entity veterancy_ordinal residual.
    #[inline]
    pub fn from_ordinal(ord: u8) -> Self {
        match ord {
            1 => Self::Veteran,
            2 => Self::Elite,
            3 => Self::Heroic,
            _ => Self::Rookie,
        }
    }

    /// C++ ControlBar portrait chevron image residual (SSChevron*).
    pub fn chevron_overlay(self) -> Option<&'static str> {
        match self {
            Self::Rookie => None,
            Self::Veteran => Some("SSChevron1L"),
            Self::Elite => Some("SSChevron2L"),
            Self::Heroic => Some("SSChevron3L"),
        }
    }
}

/// Snapshot-owned object kind residual (host ObjectType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationObjectType {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Supply,
    Projectile,
    Neutral,
}

impl PresentationObjectType {
    pub fn from_host(t: crate::game_logic::ObjectType) -> Self {
        use crate::game_logic::ObjectType as T;
        match t {
            T::Infantry => Self::Infantry,
            T::Vehicle => Self::Vehicle,
            T::Aircraft => Self::Aircraft,
            T::Building => Self::Building,
            T::Supply => Self::Supply,
            T::Projectile => Self::Projectile,
            T::Neutral => Self::Neutral,
        }
    }
}

/// Snapshot-owned structure kind residual (host BuildingType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationBuildingType {
    CommandCenter,
    Barracks,
    WarFactory,
    Airfield,
    RepairPad,
    HealPad,
    SupplyCenter,
    PowerPlant,
    DefenseTurret,
    SupplyDropZone,
    Palace,
    Propaganda,
    Bunker,
}

impl PresentationBuildingType {
    pub fn from_host(t: crate::game_logic::BuildingType) -> Self {
        use crate::game_logic::BuildingType as B;
        match t {
            B::CommandCenter => Self::CommandCenter,
            B::Barracks => Self::Barracks,
            B::WarFactory => Self::WarFactory,
            B::Airfield => Self::Airfield,
            B::RepairPad => Self::RepairPad,
            B::HealPad => Self::HealPad,
            B::SupplyCenter => Self::SupplyCenter,
            B::PowerPlant => Self::PowerPlant,
            B::DefenseTurret => Self::DefenseTurret,
            B::SupplyDropZone => Self::SupplyDropZone,
            B::Palace => Self::Palace,
            B::Propaganda => Self::Propaganda,
            B::Bunker => Self::Bunker,
        }
    }

    /// Wave 490: GameWorld entity building_type_ordinal residual (255 = none).
    #[inline]
    pub fn from_ordinal(ord: u8) -> Option<Self> {
        match ord {
            0 => Some(Self::CommandCenter),
            1 => Some(Self::Barracks),
            2 => Some(Self::WarFactory),
            3 => Some(Self::Airfield),
            4 => Some(Self::RepairPad),
            5 => Some(Self::HealPad),
            6 => Some(Self::SupplyCenter),
            7 => Some(Self::PowerPlant),
            8 => Some(Self::DefenseTurret),
            9 => Some(Self::SupplyDropZone),
            10 => Some(Self::Palace),
            11 => Some(Self::Propaganda),
            12 => Some(Self::Bunker),
            _ => None,
        }
    }

    /// Factory / barracks / airfield residual for unit production UI.
    pub fn is_unit_producer(self) -> bool {
        matches!(self, Self::Barracks | Self::WarFactory | Self::Airfield)
    }
}

/// One C++ `Drawable::updateDrawableClipStatus` payload for a concrete
/// PRIMARY/SECONDARY/TERTIARY WeaponSet slot.
///
/// This is presentation input, not a renderer query: gameplay freezes the
/// exact remaining/max pair before the W3D path turns it into authored child
/// visibility. `None` means the current authority cannot provide a valid
/// finite clip pair and must not guess a projectile model state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationProjectileClipStatus {
    /// C++ `Weapon::getRemainingAmmo()` / `shotsRemaining`.
    pub shots_remaining: u32,
    /// C++ `Weapon::getClipSize()` / `maxShots`.
    pub max_shots: u32,
}

/// Presentation-owned copy of C++ `ObjectShroudStatus`.
///
/// This is deliberately an ordinal-compatible raw status rather than a
/// visibility/alpha approximation.  `RTS3DScene::renderOneObject` uses the
/// C++ ordering directly: only values greater than `Clear` select the shroud
/// material pass.  Keep every value, including `InvalidButPreviousValid`, so
/// a later client boundary can convert it without collapsing behavior.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresentationObjectShroudStatus {
    /// C++ `OBJECTSHROUD_INVALID`.
    #[default]
    Invalid = 0,
    /// C++ `OBJECTSHROUD_CLEAR`.
    Clear = 1,
    /// C++ `OBJECTSHROUD_PARTIAL_CLEAR`.
    PartialClear = 2,
    /// C++ `OBJECTSHROUD_FOGGED`.
    Fogged = 3,
    /// C++ `OBJECTSHROUD_SHROUDED`.
    Shrouded = 4,
    /// C++ `OBJECTSHROUD_INVALID_BUT_PREVIOUS_VALID`.
    InvalidButPreviousValid = 5,
}

impl PresentationObjectShroudStatus {
    /// Exact C++ scene-pass threshold: `ss > OBJECTSHROUD_CLEAR`.
    #[inline]
    pub const fn requires_scene_shroud_material(self) -> bool {
        (self as u8) > (Self::Clear as u8)
    }

    /// Convert without changing C++ ordinal meaning.  The presentation layer
    /// owns serialization; GameClient receives this only after a direct
    /// drawable association has been established.
    #[inline]
    pub const fn as_game_logic_status(self) -> gamelogic::common::types::ObjectShroudStatus {
        use gamelogic::common::types::ObjectShroudStatus;

        match self {
            Self::Invalid => ObjectShroudStatus::Invalid,
            Self::Clear => ObjectShroudStatus::Clear,
            Self::PartialClear => ObjectShroudStatus::PartialClear,
            Self::Fogged => ObjectShroudStatus::Fogged,
            Self::Shrouded => ObjectShroudStatus::Shrouded,
            Self::InvalidButPreviousValid => ObjectShroudStatus::InvalidButPreviousValid,
        }
    }
}

impl From<gamelogic::common::types::ObjectShroudStatus> for PresentationObjectShroudStatus {
    #[inline]
    fn from(value: gamelogic::common::types::ObjectShroudStatus) -> Self {
        use gamelogic::common::types::ObjectShroudStatus;

        match value {
            ObjectShroudStatus::Invalid => Self::Invalid,
            ObjectShroudStatus::Clear => Self::Clear,
            ObjectShroudStatus::PartialClear => Self::PartialClear,
            ObjectShroudStatus::Fogged => Self::Fogged,
            ObjectShroudStatus::Shrouded => Self::Shrouded,
            ObjectShroudStatus::InvalidButPreviousValid => Self::InvalidButPreviousValid,
        }
    }
}

/// Whether this frame has authoritative direct-object facts for a drawable.
///
/// A GameWorld entity does not carry a C++ `DrawableInfo`/drawable lifetime,
/// so it starts `Unknown`.  It must not be treated as a direct scene object
/// until the matching host-object overlay has frozen the raw status and
/// effective-death fact.  This deliberately does not invent a drawable ID or
/// generation; the client owns those associations and rejects stale bindings.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PresentationDrawableLifetime {
    /// No authoritative direct-object drawable facts are available.
    #[default]
    Unknown = 0,
    /// `RenderableObject::id` was backed by a host `Object` during frame build.
    DirectHostObject = 1,
}

impl PresentationDrawableLifetime {
    #[inline]
    pub const fn is_direct_host_object(self) -> bool {
        matches!(self, Self::DirectHostObject)
    }
}

/// Frozen input for the C++ direct-object shroud branch.
///
/// The raw status and effective-death bit are captured before rendering.  The
/// drawable-owned clear-frame timer is intentionally absent: C++ updates it at
/// view dispatch after resolving the live drawable, and Main must not emulate
/// that state from `ObjectVisibility` alpha.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct PresentationDrawableShroudFacts {
    pub lifetime: PresentationDrawableLifetime,
    pub raw_status: PresentationObjectShroudStatus,
    pub effectively_dead: bool,
}

impl PresentationDrawableShroudFacts {
    #[inline]
    pub const fn direct_host_object(
        raw_status: PresentationObjectShroudStatus,
        effectively_dead: bool,
    ) -> Self {
        Self {
            lifetime: PresentationDrawableLifetime::DirectHostObject,
            raw_status,
            effectively_dead,
        }
    }

    /// Fail closed for GameWorld-only/default records.  C++ material selection
    /// itself remains the exact raw ordinal threshold once direct ownership is
    /// known.
    #[inline]
    pub const fn requires_scene_shroud_material(self) -> bool {
        self.lifetime.is_direct_host_object() && self.raw_status.requires_scene_shroud_material()
    }

    /// Source-side payload for GameClient's direct-object update.  `Unknown`
    /// has no drawable/object association and therefore cannot enter that API.
    #[inline]
    pub fn direct_game_client_status(
        self,
    ) -> Option<(gamelogic::common::types::ObjectShroudStatus, bool)> {
        self.lifetime.is_direct_host_object().then_some((
            self.raw_status.as_game_logic_status(),
            self.effectively_dead,
        ))
    }
}

fn default_friendly_stealth_opacity() -> f32 {
    0.5
}

fn default_friendly_stealth_opacity_max() -> f32 {
    1.0
}

fn default_shadows_enabled() -> bool {
    true
}

fn default_terrain_decal_none() -> u8 {
    8
}

/// One renderable object as seen after a completed logic step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderableObject {
    pub id: ObjectId,
    pub template_name: String,
    pub team: Team,
    /// Controlling host player; distinct from faction for same-faction slots.
    #[serde(default)]
    pub owner_player_id: Option<u32>,
    /// Team tint for presentation-only draw (RGBA 0..1), mirrors Object::team_color.
    pub team_color: [f32; 4],
    pub position: Vec3,
    pub orientation: f32,
    /// C++ FloatUpdate yaw residual (radians).
    #[serde(default)]
    pub float_yaw: f32,
    /// C++ FloatUpdate pitch residual (radians).
    #[serde(default)]
    pub float_pitch: f32,
    /// C++ ToppleUpdate lean residual (radians fallen about fall axis).
    #[serde(default)]
    pub topple_lean_radians: f32,
    /// C++ `m_toppleDirection.x` (host X).
    #[serde(default)]
    pub topple_dir_x: f32,
    /// C++ `m_toppleDirection.y` (host Z).
    #[serde(default)]
    pub topple_dir_y: f32,
    /// C++ `Drawable::setShadowsEnabled` residual.
    #[serde(default = "default_shadows_enabled")]
    pub shadows_enabled: bool,
    /// C++ `Drawable` terrain-decal type residual.
    #[serde(default = "default_terrain_decal_none")]
    pub terrain_decal_type: u8,
    #[serde(default)]
    pub terrain_decal_size: f32,
    #[serde(default)]
    pub terrain_decal_opacity: f32,
    /// Current movement order destination (host Movement::target_position).
    pub move_destination: Option<Vec3>,
    /// Host Object::target_location residual (script/order point).
    pub target_location: Option<Vec3>,
    /// Host guard_target residual.
    pub guard_target: Option<ObjectId>,
    /// Host ObjectStatus::using_ability residual.
    pub using_ability: bool,
    /// Host ObjectStatus::airborne_target residual.
    pub airborne_target: bool,
    /// Wave 982: producer/slaver residual for IgnoredInGui mouseover remap.
    #[serde(default)]
    pub producer_id: Option<ObjectId>,
    /// Wave 983: healing icon residual (sole-benefactor / heal timer).
    #[serde(default)]
    pub show_healing: bool,
    #[serde(default)]
    pub healing_icon_type: u8,
    /// Wave 505: C++ OBJECT_STATUS_PARACHUTING residual.
    pub parachuting: bool,
    /// Wave 509: C++ parachute open residual (false + parachuting => FREEFALL).
    pub parachute_open: bool,
    /// C++ `TheKey_objectWeather` (0 follow map, 1 force normal, 2 force snow).
    #[serde(default)]
    pub object_weather: i32,

    /// Wave 510: C++ CAPTURED model-condition residual.
    pub captured: bool,
    /// Wave 512: C++ prone residual (Infantry goProne timer).
    pub prone: bool,
    /// Wave 514: C++ Drawable emoticon residual name.
    pub emoticon_name: String,
    /// Wave 514: remaining logic frames for emoticon.
    pub emoticon_frames_left: i32,
    /// Wave 515: C++ AIUpdateInterface::setSurrendered residual.
    pub is_surrendered: bool,
    /// Wave 515: C++ Object::m_formationID residual (0 = none).
    pub formation_id: u32,
    /// Wave 515: C++ Object::m_formationOffset residual.
    pub formation_offset: glam::Vec2,
    /// Wave 507: C++ OVER_WATER model condition residual (hover craft / water).
    pub over_water: bool,
    /// Wave 526: MOVING/ATTACKING via name-table helpers.
    /// Wave 525: FRONTCRUSHED/BACKCRUSHED/PREORDER/USER_1/USER_2 residual.
    /// Wave 524: multi-door DOOR_2..4 banks + SMOLDERING residual.
    /// Wave 523: stamp STUNNED_FLAILING / SECOND_LIFE / POST_COLLAPSE / SPECIAL_DAMAGED.
    /// Wave 522: C++ terrain cell cliff residual.
    pub cell_is_cliff: bool,
    /// Wave 522: C++ terrain cell underwater residual.
    pub cell_is_underwater: bool,
    /// Host movement max speed residual.
    pub move_max_speed: f32,
    /// Host velocity residual.
    pub velocity: Vec3,
    /// Host AI state ordinal residual.
    pub ai_state_ordinal: u8,
    /// Attack target object id when set.
    pub attack_target: Option<ObjectId>,
    /// Path waypoints residual (capped) for line pack / debug draw.
    pub path_waypoints: Vec<Vec3>,
    /// Host movement path length residual.
    pub path_len: u16,
    /// Host movement current path index residual.
    pub path_index: u16,
    /// Host occupant_count residual (transport/contain).
    pub occupant_count: u16,
    /// Structure production queue residual (empty for non-buildings).
    pub production_queue: Vec<PresentationProductionItem>,
    /// Wave 986: host BuildingData production pause residual.
    #[serde(default)]
    pub production_paused: bool,
    /// Structure rally point residual.
    pub rally_point: Option<Vec3>,
    /// Guard position residual (units).
    pub guard_position: Option<Vec3>,
    /// Contained unit ids (garrison / transport residual, capped).
    pub garrisoned_units: Vec<ObjectId>,
    /// C++ `getStealthUnitsContained` frozen independently of hide-from-nonallies
    /// so RMB `canEnterObject` can allow a stealth-only civilian garrison.
    #[serde(default)]
    pub stealth_garrison_occupant_count: u16,

    /// Max garrison slots (0 = not a container).
    pub max_garrison: usize,
    /// Structure/unit power provided residual.
    pub power_provided: i32,
    /// Structure/unit power consumed residual.
    pub power_consumed: i32,
    /// Host Object::stored_resources.supplies residual (supply center / drop zone).
    pub stored_supplies: u32,
    /// C++ `updateDrawableSupplyStatus` current boxes (warehouse crate pile).
    #[serde(default)]
    pub drawable_supply_boxes: u32,
    /// C++ startingBoxes argument to `updateDrawableSupplyStatus`.
    #[serde(default)]
    pub drawable_supply_max_boxes: u32,
    /// Exact Object INI DockUpdate family, carried separately from KindOf so
    /// physical RMB classification never derives dockability from a name.
    #[serde(default)]
    pub dock_kind: DockKind,
    /// Exact C++ `KINDOF_CAPTURABLE` semantic, carried outside the compact
    /// KindOf bank for physical CaptureBuilding classification.
    #[serde(default)]
    pub capturable: bool,
    /// Exact C++ `KINDOF_IMMUNE_TO_CAPTURE` semantic.
    #[serde(default)]
    pub immune_to_capture: bool,
    /// Exact `GarrisonContain` presence.  This must remain distinct from an
    /// ordinary Enter/transport capacity for capture legality.
    #[serde(default)]
    pub capture_garrisonable: bool,
    /// Exact source capture SpecialPower module, if one is authored.
    #[serde(default)]
    pub capture_power: crate::game_logic::CapturePowerKind,
    /// Snapshot-time SpecialPower readiness for that same exact module.
    #[serde(default)]
    pub capture_power_ready: bool,
    /// Exact paired `SpecialAbility` + `SpecialAbilityUpdate` HDB capability.
    /// This is frozen from host authority; UI must not recognize Hacker by
    /// display/template spelling.
    #[serde(default)]
    pub hacker_disable_building_capable: bool,
    /// Frozen HDB module readiness for the same presentation frame.
    #[serde(default)]
    pub hacker_disable_building_ready: bool,
    /// Exact parsed SpecialPowerTemplate identity for the ready structure
    /// module.  UI maps this canonical source name through the explicit
    /// command adapter; it must not reclassify `template_name` by substring.
    #[serde(default)]
    pub special_power_ready_template_name: Option<String>,
    /// Stable loaded SpecialPowerTemplate id paired with the canonical name.
    #[serde(default)]
    pub special_power_ready_template_id: Option<u32>,
    /// C++ overridable special-power dest while PUC/Spectre is active.
    #[serde(default)]
    pub special_power_override_destination: Option<Vec3>,
    pub health_current: f32,
    pub health_max: f32,
    pub selected: bool,
    /// C++ OBJECT_STATUS_DEPLOYED residual.
    pub is_deployed: bool,
    /// C++ Drawable selection flash envelope residual frames.
    pub selection_flash_remaining: u32,
    /// C++ `flashAsSelected(&myHouseColor)` envelope RGB. `None` is white default.
    #[serde(default)]
    pub selection_flash_color: Option<[f32; 3]>,

    pub destroyed: bool,
    /// C++ ModelConditionFlags residual (ALLOW_SURRENDER-off bit layout, low 128).
    pub model_condition_bits: u128,
    /// C++ RadarUpdate m_radarActive residual.
    pub radar_active: bool,
    /// C++ RadarUpdate m_extendComplete residual.
    pub radar_extend_complete: bool,
    /// C++ ProductionUpdate door residual phase (0 idle .. 3 closing).
    pub production_door_phase: u8,
    /// C++ BodyDamageType residual ordinal (0 pristine .. 3 rubble).
    pub body_damage_state: u8,
    /// C++ TransitionDamageFX / FXListDie residual name frozen at snapshot.
    #[serde(default)]
    pub damage_fx_name: Option<String>,
    /// C++ BoneFXDamage / BoneFXUpdate residual FXList name.
    #[serde(default)]
    pub bone_fx_name: Option<String>,
    /// C++ TINT_STATUS_POISONED residual.
    #[serde(default)]
    pub poison_tinted: bool,
    /// C++ UNDETECTED_DEFECTOR residual.
    #[serde(default)]
    pub undetected_defector: bool,
    /// C++ DefectionHelper selection flash residual.
    #[serde(default)]
    pub defector_flash: bool,
    /// C++ FXListDie death FX residual name.
    #[serde(default)]
    pub death_fx_name: Option<String>,
    /// C++ DeathType residual name for death FX (empty when alive).
    pub death_type_name: String,
    pub under_construction: bool,
    /// C++ `dozerAI->isTaskPending(DOZER_TASK_BUILD)` (`dozer_task_build_target`).
    #[serde(default)]
    pub is_dozer_task_pending: bool,
    /// Construction progress 0..1 residual (structures / dozer builds).
    pub construction_percent: f32,
    /// Wave 1031: OCL timer residual seconds (ControlBar OclTimer dual path).
    pub ocl_timer_seconds: u32,
    /// C++ OBJECT_STATUS_SOLD residual frozen for presentation/UI.
    pub sold: bool,
    /// C++ OBJECT_STATUS_SCRIPT_UNSELLABLE residual frozen for ControlBar hide.
    #[serde(default)]
    pub script_unsellable: bool,
    /// C++ `Object::m_singleUseCommandUsed` — leftover strip Restricted after first click.
    #[serde(default)]
    pub single_use_command_used: bool,
    /// C++ OBJECT_STATUS_UNSELECTABLE residual frozen for presentation/UI.
    pub unselectable: bool,
    /// C++ RebuildHole residual frozen for presentation/UI.
    pub is_rebuild_hole: bool,
    /// Wave 993: C++ RebuildHoleBehavior m_rebuildTemplate residual.
    #[serde(default)]
    pub rebuild_template_name: String,
    /// Wave 993: host rebuild_ready_frame residual.
    #[serde(default)]
    pub rebuild_ready_frame: u32,
    /// Wave 993: RebuildHoleBehavior m_spawnerID residual.
    #[serde(default)]
    pub rebuild_spawner_id: Option<ObjectId>,
    /// Wave 993: RebuildHoleBehavior m_workerID residual.
    #[serde(default)]
    pub rebuild_worker_id: Option<ObjectId>,
    /// Wave 993: RebuildHoleBehavior m_reconstructingID residual.
    #[serde(default)]
    pub rebuild_reconstructing_id: Option<ObjectId>,
    /// C++ OBJECT_STATUS_RECONSTRUCTING residual frozen for presentation.
    pub reconstructing: bool,
    /// Veterancy rank residual for chevrons / UI.
    pub veterancy: PresentationVeterancy,
    /// Experience points residual (display / debug).
    pub experience_points: f32,
    /// Host ObjectStatus::moving residual.
    pub moving: bool,
    /// Host ObjectStatus::attacking residual.
    pub attacking: bool,
    /// Host ObjectStatus::is_firing_weapon residual.
    pub is_firing_weapon: bool,
    /// Host ObjectStatus::is_aiming_weapon residual.
    pub is_aiming_weapon: bool,
    /// Host ObjectStatus::disabled_emp residual.
    pub disabled_emp: bool,
    /// Host ObjectStatus::disabled_paralyzed residual.
    pub disabled_paralyzed: bool,
    /// Host ObjectStatus::disabled_underpowered residual.
    #[serde(default)]
    pub disabled_underpowered: bool,
    /// Host ObjectStatus::disabled_hacked residual.
    #[serde(default)]
    pub disabled_hacked: bool,
    pub disabled_unmanned: bool,
    /// C++ DISABLED_FREEFALL residual for Drawable TINT_STATUS_DISABLED.
    #[serde(default)]
    pub disabled_freefall: bool,
    /// C++ DISABLED_DEFAULT residual for Drawable TINT_STATUS_DISABLED.
    #[serde(default)]
    pub disabled_default: bool,
    /// C++ DISABLED_SCRIPT_UNDERPOWERED residual for Drawable TINT_STATUS_DISABLED.
    #[serde(default)]
    pub disabled_script_underpowered: bool,
    /// C++ OBJECT_STATUS_SCRIPT_DISABLED residual for ControlBar hide.
    #[serde(default)]
    pub disabled_script_disabled: bool,
    /// C++ HackInternetAIInterface::isHackingPackingOrUnpacking residual.
    #[serde(default)]
    pub hacking_packing_or_unpacking: bool,
    /// Host ObjectStatus::weapons_jammed residual.
    pub weapons_jammed: bool,
    /// Host ObjectStatus::masked residual.
    pub masked: bool,
    /// C++ KINDOF_UNATTACKABLE victim override.  This deliberately lives
    /// outside the compact 32-bit KindOf mirror so the existing bit layout
    /// remains snapshot-compatible while weapon-target picking stays exact.
    #[serde(default)]
    pub unattackable: bool,
    /// C++ `KINDOF_FORCEATTACKABLE` frozen outside the compact 32-bit KindOf
    /// bank. Force-attack and hover pick civ fences / cargo planes that are
    /// not Selectable.
    #[serde(default)]
    pub is_force_attackable: bool,
    /// C++ `KINDOF_ALWAYS_SELECTABLE` frozen outside the compact 32-bit KindOf
    /// bank. Dead UI-feedback / rubble stays clickable (SelectionXlat.cpp:113).
    #[serde(default)]
    pub always_selectable: bool,

    /// C++ `KINDOF_CRATE` frozen outside the compact 32-bit KindOf bank.
    /// Physical crate-click routing uses this instead of a template name.
    #[serde(default)]
    pub is_crate: bool,
    /// C++ `Object::isSalvageCrate` / host money-crate salvage residual.
    #[serde(default)]
    pub is_salvage_crate: bool,
    /// Host ObjectStatus::ignoring_stealth residual.
    pub ignoring_stealth: bool,
    /// Host ObjectStatus::repulsor residual.
    pub repulsor: bool,
    /// C++ OBJECT_STATUS_STEALTHED residual.
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual.
    pub detected: bool,
    /// Stealthed && !detected && !disguised (not a legal auto-target).
    pub effectively_stealthed: bool,
    /// Exact active-host equivalent of C++
    /// `StealthUpdate::canDisguise()` (`DisguisesAsTeam`). This is a frozen
    /// source capability, not a transition-state inference: a Bomb Truck is
    /// scene-visible before its disguise reaches the visual halfpoint even
    /// though the host temporarily marks it STEALTHED.
    #[serde(default)]
    pub can_disguise_as_team: bool,
    /// Frozen C++ StealthUpdate FriendlyOpacityMin used by the two
    /// viewer-relative friendly looks.  Retail defaults to 50%; this stays
    /// separate from FOW alpha and from CamoNetting's object-local pulse.
    #[serde(default = "default_friendly_stealth_opacity")]
    pub friendly_stealth_opacity: f32,
    /// Frozen FriendlyOpacityMax retained alongside the minimum for pulse
    /// and detected-friendly look transitions.
    #[serde(default = "default_friendly_stealth_opacity_max")]
    pub friendly_stealth_opacity_max: f32,
    /// Any host disable residual that blocks acting.
    pub disabled: bool,
    /// Container residual when this unit is inside another object.
    pub contained_by: Option<ObjectId>,
    /// Force-attack order residual.
    pub force_attack: bool,
    /// Primary weapon present residual.
    pub has_weapon: bool,
    /// Primary weapon range residual (0 when unarmed).
    pub weapon_range: f32,
    /// Primary weapon damage residual (0 when unarmed).
    pub weapon_damage: f32,
    /// Primary weapon min range residual.
    pub weapon_min_range: f32,
    /// Primary weapon reload time residual (seconds-ish).
    pub weapon_reload_time: f32,
    /// Primary weapon ammo residual (`u32::MAX` = unlimited).
    pub weapon_ammo: u32,
    /// Exact per-slot clip pairs for the C++ W3D projectile-bone feedback
    /// path. Older saved presentation frames contain no such vector and
    /// therefore fail closed with no dynamic projectile directives.
    #[serde(default)]
    pub projectile_clip_statuses: [Option<PresentationProjectileClipStatus>; 3],
    /// C++ getAmmoPipShowingInfo residual (0 = no ShowsAmmoPips weapon).
    pub ammo_pip_total: u32,
    /// Remaining rounds for the ShowsAmmoPips weapon.
    pub ammo_pip_full: u32,
    /// C++ getMostPercentReadyToFireAnyWeapon residual (0..100).
    pub weapon_ready_percent: u32,
    /// Primary weapon air/ground targeting residual.
    pub weapon_can_target_air: bool,
    pub weapon_can_target_ground: bool,
    /// Primary weapon projectile speed residual.
    pub weapon_projectile_speed: f32,
    /// Host armed_riders_upgrade_weapon_set residual.
    pub armed_riders_upgrade_weapon_set: bool,
    /// Host weapon_set_player_upgrade residual.
    pub weapon_set_player_upgrade: bool,
    /// Wave 523: C++ ARMORSET_SECOND_LIFE / battle bus second life residual.
    pub second_life: bool,
    /// Wave 525: C++ front crushed residual.
    pub front_crushed: bool,
    /// Wave 525: C++ back crushed residual.
    pub back_crushed: bool,
    /// Wave 525: host model-condition USER_1 residual.
    pub user_1: bool,
    /// Wave 525: host model-condition USER_2 residual.
    pub user_2: bool,
    /// Wave 518: C++ weapon_crate_upgrade residual (0/1/2).
    pub weapon_crate_upgrade: u8,
    /// Wave 518: C++ armor_crate_upgrade residual (0/1/2).
    pub armor_crate_upgrade: u8,
    /// Wave 518: C++ EnemyNearUpdate model_enemy_near residual.
    pub enemy_near: bool,
    /// Wave 518: C++ armed riders / ARMED model residual.
    pub armed: bool,
    /// CamoNetting StealthLook ordinal residual (0..5).
    pub camo_stealth_look: u8,
    /// Bomb-truck disguise template residual.
    pub disguise_as_template: Option<String>,
    /// Apparent team while disguised.
    pub disguise_as_team: Option<Team>,
    /// C++ OBJECT_STATUS_DISGUISED residual.
    pub disguised: bool,
    /// Host ObjectStatus::disabled_subdued residual.
    pub disabled_subdued: bool,
    pub is_carbomb: bool,
    /// C++ WEAPONSET_CARBOMB residual for the local-player CarBomb icon.
    #[serde(default)]
    pub weapon_set_carbomb: bool,
    /// C++ drawBombed sticky type: 0 none, 1 timed, 2 remote.
    #[serde(default)]
    pub bomb_type: u8,
    /// C++ StickyBombUpdate countdown residual in whole seconds.
    #[serde(default)]
    pub bomb_timer_seconds: u32,
    /// Host ObjectStatus::hijacked residual.
    pub hijacked: bool,
    /// C++ StealthUpdate disguise transition opacity residual (0..1).
    pub disguise_transition_opacity: f32,
    /// Stealth detector range residual (0 = none).
    pub detection_range: f32,
    /// Host detection_rate_frames residual (0 = continuous).
    pub detection_rate_frames: u32,
    /// Host stealth_breaks_on_attack residual.
    pub stealth_breaks_on_attack: bool,
    /// Host stealth_breaks_on_move residual.
    pub stealth_breaks_on_move: bool,
    /// Host innate_stealth residual.
    pub innate_stealth: bool,
    /// Host weapon_bonus_frenzy_until_frame residual.
    pub weapon_bonus_frenzy_until_frame: u32,
    /// Host continuous_fire_consecutive residual.
    pub continuous_fire_consecutive: u16,
    /// Host continuous_fire_coast_until_frame residual.
    pub continuous_fire_coast_until_frame: u32,
    /// Host battle_plan_sight_scalar_applied residual (1.0 = none).
    pub battle_plan_sight_scalar_applied: f32,
    /// Special power ready residual (superweapon / hero ability).
    pub special_power_ready: bool,
    /// Special power full cooldown seconds residual.
    pub special_power_cooldown: f32,
    /// Special power remaining cooldown seconds residual.
    pub special_power_cooldown_remaining: f32,
    /// Host ObjectType residual (UI / command set feed).
    pub object_type: PresentationObjectType,
    /// Applied upgrade tags residual (capped, sorted).
    pub applied_upgrades: Vec<String>,
    /// C++ `ThingTemplate::m_upgradeCameoUpgradeNames` (`UpgradeCameo1..5`).
    #[serde(default)]
    pub upgrade_cameo_names: [String; 5],
    /// C++ SubObjectsUpgrade show/hide residual (Bombload / BombWing).
    /// Reveal halfpoint `forceRefreshSubObjectUpgradeStatus` rebuilds this
    /// after the disguise drawable discarded the previous W3D children.
    #[serde(default)]
    pub sub_object_visibility: crate::game_logic::host_sub_objects_upgrade::HostSubObjectVisibility,

    /// Secondary weapon present residual.
    pub has_secondary_weapon: bool,
    /// Secondary weapon range residual (0 when none).
    pub secondary_weapon_range: f32,
    /// Secondary weapon damage residual (0 when none).
    pub secondary_weapon_damage: f32,
    /// Host turret yaw residual (degrees).
    pub turret_angle_deg: f32,
    /// Host turret pitch residual (degrees).
    pub turret_pitch_deg: f32,
    /// Host turret idle-scan residual.
    pub turret_idle_scanning: bool,
    /// Host weapon-bonus residual flags (presentation UI/FX).
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,
    pub weapon_bonus_horde: bool,
    pub weapon_bonus_nationalism: bool,
    pub weapon_bonus_frenzy: bool,
    pub weapon_bonus_frenzy_level: u8,
    /// Host battle-plan weapon-bonus residual (Strategy Center).
    pub weapon_bonus_battle_plan_bombardment: bool,
    pub weapon_bonus_battle_plan_hold_the_line: bool,
    pub weapon_bonus_battle_plan_search_and_destroy: bool,
    /// Host continuous-fire residual (gattling spin-up).
    pub continuous_fire_level: u8,
    /// Host faerie_fire_until_frame residual.
    pub faerie_fire_until_frame: u32,
    /// Host hive slave residual (Stinger Site etc.).
    pub hive_slave_count: u8,
    pub hive_slave_hp: f32,
    /// Host AI attitude residual.
    pub ai_attitude: i8,
    /// Host camo friendly opacity residual.
    pub camo_friendly_opacity: f32,
    /// Host vision_spied_mask residual.
    pub vision_spied_mask: u32,
    /// Wave 994: host Object::vision_range residual.
    #[serde(default)]
    pub vision_range: f32,
    /// Wave 994: host Object::shroud_clearing_range residual.
    #[serde(default)]
    pub shroud_clearing_range: f32,
    /// Wave 994: host Object::crusher_level residual.
    #[serde(default)]
    pub crusher_level: u8,
    /// Wave 994: host Object::crushable_level residual.
    #[serde(default)]
    pub crushable_level: u8,
    /// Host cheer_timer residual.
    pub cheer_timer: f32,
    /// Host transport-kind residual markers.
    pub is_humvee_transport: bool,
    pub is_listening_outpost_transport: bool,
    pub is_troop_crawler_transport: bool,
    pub is_helix_transport: bool,
    pub has_overlord_gattling_addon: bool,
    pub has_overlord_propaganda_addon: bool,
    pub is_battle_bus_transport: bool,
    pub is_technical_transport: bool,
    pub is_combat_cycle_transport: bool,
    pub combat_cycle_rider: u8,
    pub is_tunnel_network: bool,
    pub is_combat_chinook_transport: bool,
    pub max_transport: usize,
    pub overlord_bunker_capacity: usize,
    /// Exact parsed normal ContainModule presence.  This prevents a generic
    /// vehicle with a stale capacity field from becoming a player transport.
    #[serde(default)]
    pub contain_module_present: bool,
    /// Concrete parsed containment implementation.  RiderChange is retained
    /// separately so frozen RMB input can fail closed until its authored
    /// Rider1..Rider8 replacement transaction is represented.
    #[serde(default)]
    pub contain_module_kind: crate::game_logic::ContainModuleKind,
    /// Frozen `AllowInsideKindOf`/`ForbidInsideKindOf` result for physical
    /// RMB Enter. Unsupported metadata is deliberately fail-closed.
    #[serde(default)]
    pub contain_admission: crate::game_logic::ContainAdmission,
    /// The only RiderChange data frozen into the physical-input frame: exact
    /// supported rider template identities.  Visual/status/weapon metadata
    /// stays on the authoritative ThingTemplate; RMB needs only a bounded
    /// membership decision and repeats it in GameLogic at arrival.
    #[serde(default)]
    pub rider_change_allowed_templates: Vec<String>,
    /// C++ OpenContain relationship gates retained from the same module.
    #[serde(default = "default_presentation_allow_inside")]
    pub contain_allow_allies_inside: bool,
    #[serde(default = "default_presentation_allow_inside")]
    pub contain_allow_enemies_inside: bool,
    #[serde(default = "default_presentation_allow_inside")]
    pub contain_allow_neutral_inside: bool,
    /// C++ `TransportSlotCount` frozen for source validation in the physical
    /// input path. Zero/missing metadata rejects normal Enter.
    #[serde(default)]
    pub transport_slot_count: usize,
    /// C++ faction-structure distinction used by normal Enter's non-owner
    /// gate.  This is not inferred from a presentation template spelling.
    #[serde(default)]
    pub is_faction_structure: bool,
    pub passengers_allowed_to_fire: bool,
    pub display_name: String,
    pub demo_suicided_detonating: bool,
    /// Host turret_holding residual.
    pub turret_holding: bool,
    /// Host last_damage_source residual (0 = none).
    pub last_damage_source_host: u32,
    /// Host Object::command_set_override residual (empty = template default).
    pub command_set_override: String,
    /// Effective command-set name freeze (override or ThingFactory template).
    pub command_set_name: String,
    /// Host Object::is_detector residual.
    pub is_detector: bool,
    /// Host Object::active_weapon_slot residual.
    pub active_weapon_slot: u8,
    /// Wave 517: C++ WeaponFireStatus ordinal residual (Ready/OutOfAmmo/Between/Reload/PreAttack).
    pub weapon_fire_status: u8,
    /// Wave 517: C++ loco/AI panicking residual.
    pub is_panicking: bool,
    /// Wave 517: C++ moving_backwards residual.
    pub moving_backwards: bool,
    /// Host Object::overcharge_enabled residual.
    pub overcharge_enabled: bool,
    /// Frozen typed OverchargeBehavior authority.  The command strip uses
    /// this snapshot fact only for presentation; CommandExecutor repeats the
    /// metadata validation on the live ThingTemplate.
    #[serde(default)]
    pub can_toggle_overcharge: bool,
    /// Wave 519: C++ shockwave airborne residual.
    pub shock_was_airborne: bool,
    /// Wave 519: C++ shock allow bounce residual.
    pub shock_allow_bounce: bool,
    /// Wave 519: C++ shock grounded-once residual.
    pub shock_grounded_once: bool,
    /// Wave 519: remaining shock stun frames.
    pub shock_stun_frames: u32,
    /// Wave 519: C++ PowerPlantUpdate m_extended residual.
    pub power_plant_rods_extended: bool,
    /// Wave 519: frame when rods finish upgrading (0 idle).
    pub power_plant_rods_done_frame: u32,
    /// Wave 519: jet slow-death residual active.
    pub jet_slow_death_active: bool,
    /// Wave 520: C++ AnimationSteeringUpdate turn anim ordinal residual
    /// (0 invalid, 1 CTR, 2 CTL, 3 LTC, 4 RTC).
    pub anim_steer_turn: u8,
    /// Host Object::show_health_bar residual.
    pub show_health_bar: bool,
    /// Host Object::guard_radius residual.
    pub guard_radius: f32,
    /// Mine / demo-trap residual present.
    pub has_mine: bool,
    /// Host ThingTemplate KindOf set residual (sorted, capped).
    /// Lets ControlBar / unit_control classify without live template re-read.
    pub kind_of: Vec<crate::game_logic::KindOf>,
    pub is_structure: bool,
    pub is_unit: bool,
    /// Mobile residual (infantry/vehicle/aircraft) for runtime-host select.
    pub is_mobile: bool,
    /// C++ `Object::m_safeOcclusionFrame`. Behind-building silhouettes wait.
    #[serde(default)]
    pub safe_occlusion_frame: u32,

    /// Structure can enqueue production (host building_data present + constructed).
    pub can_produce: bool,
    /// Host BuildingType residual when structure has building_data.
    pub building_type: Option<PresentationBuildingType>,
    /// W3D / mesh resolve key (template model name). Snapshot-owned so the unit
    /// mesh pass does not re-read live ThingTemplate during GPU collect.
    pub model_key: Option<String>,
    /// Exact source-authored W3D models selected from every active Object INI
    /// Draw module, in declaration order.  The primary `model_key` above is
    /// retained for snapshot compatibility and UI/prewarm callers; rendering
    /// submits this whole list without deduplicating same-name modules.
    #[serde(default)]
    pub draw_models: Vec<crate::assets::AuthoredDrawModel>,
    /// Mesh scale residual (Object INI Scale; common combat units retail **1.0**).
    /// Snapshot-owned so the unit mesh pass does not re-read live template Scale.
    /// Fail-closed: not full draw-scale bone / animation scale matrix.
    pub mesh_scale: f32,
    /// Cull / selection radius for presentation-only draw (no live GameLogic re-read).
    pub selection_radius: f32,
    /// C++ getHealthBoxDimensions width (geometry major+minor clamp).
    #[serde(default)]
    pub health_box_width: f32,
    /// C++ getHealthBoxPosition Z lift (maxHeight + 10 + offset [+20 nexus]).
    #[serde(default)]
    pub health_box_z_offset: f32,
    /// C++ `GeometryInfo::getMaxHeightAbovePosition` for construction z-sink.
    #[serde(default)]
    pub max_height_above_position: f32,

    /// True when bridged to GameEngine ObjectFactory (retired host dual-id).
    /// Presentation-owned so the unit mesh pass can skip double-draw without
    /// locking live GameLogic for identity.
    pub engine_bridged: bool,
    /// FOW visibility for `PresentationFrame.local_player_id` at snapshot time.
    /// Unit mesh pass applies alpha / never-explored skip from this only — no
    /// live shroud re-query mid-render.
    pub fow_visibility: ObjectVisibility,
    /// Exact C++ direct-object shroud input frozen with this object.  This is
    /// separate from scalar `fow_visibility`: it retains raw ordinal status,
    /// effective-death, and whether host drawable lifetime facts exist.
    #[serde(default)]
    pub drawable_shroud: PresentationDrawableShroudFacts,
    /// Terrain ground-height residual sampled at object XY (Wave 77 deepen).
    /// Defaults to `PRESENTATION_DEFAULT_GROUND_HEIGHT` when map height unavailable.
    /// Fail-closed: not full HeightMap bilinear / bridge-aware sample; does **not**
    /// rewrite `position.y` (locomotor ground clamp residual separate).
    pub ground_height: f32,
    pub ground_height_from_terrain: bool,
    /// C++ Drawable fadeIn/fadeOut residual (0 none, 1 in, 2 out).
    #[serde(default)]
    pub drawable_fade_mode: u8,
    /// Logic frame when the current Drawable fade started.
    #[serde(default)]
    pub drawable_fade_start_frame: u32,
    /// C++ `m_timeToFade` residual (logic frames).
    #[serde(default)]
    pub drawable_fade_frames: u32,
    /// C++ TINT_STATUS_GAINING_SUBDUAL_DAMAGE residual (`subdual_damage > 0`).
    #[serde(default)]
    pub gaining_subdual: bool,
    /// C++ `Drawable::m_explicitOpacity` residual.
    #[serde(default = "default_one_f32_presentation")]
    pub drawable_explicit_opacity: f32,
    /// C++ `Drawable::m_secondMaterialPassOpacity` residual.
    #[serde(default)]
    pub camo_heat_vision_opacity: f32,
}

/// Frozen direct-object visual source retained independently of the primary
/// GameWorld presentation roster.
///
/// C++ keeps an Object-backed Drawable resident through deferred death and
/// rubble lifetime.  The coupled GameWorld roster may intentionally omit that
/// entity before the host Object is removed, so this owned record keeps the
/// direct drawable input available without redefining gameplay destruction.
/// It deliberately contains no DrawableId, clear-frame timer, or binding
/// generation: those runtime-only associations belong to GameClient.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationDirectHostDrawable {
    /// Full frozen visual input from the host Object.  In particular,
    /// `drawable_shroud` contains the raw C++ shroud ordinal and
    /// effectively-dead fact for the direct drawable path.
    pub object: RenderableObject,
    /// Stable C++ visual identity.  A completed disguise selects
    /// `disguise_as_template`; otherwise this is the immutable ThingTemplate
    /// name, never the mutable Object `template_name` bookkeeping field.
    pub visual_template_name: String,
    /// Authored instance scale of `visual_template_name`.  C++ destroys and
    /// recreates the Drawable when a disguise visual is committed, so the
    /// replacement template's `ThingTemplate::asset_scale` must replace both
    /// the source mesh scale and its pre-model cull radius.
    pub visual_mesh_scale: f32,
    /// Host-object presence at frame construction.  This is intentionally
    /// independent of HP and `RenderableObject::destroyed`: removal from the
    /// host roster, rather than gameplay death, ends direct visual residency.
    pub resident: bool,
}

const fn default_presentation_allow_inside() -> bool {
    true
}

const fn default_one_f32_presentation() -> f32 {
    1.0
}

impl RenderableObject {
    /// Frozen capacity for a normal player `MSG_ENTER` order.
    ///
    /// This deliberately uses only containment state that reached the
    /// presentation frame.  In particular, it does not turn every vehicle
    /// into a transport from its footprint or template spelling: C++ asks the
    /// target's ContainModule whether it can accept the selected object.
    pub fn normal_enter_capacity(&self) -> Option<usize> {
        if !self.supports_normal_enter() {
            return None;
        }
        if self.contain_module_kind == crate::game_logic::ContainModuleKind::RiderChange {
            // C++ RiderChangeContain ignores normal capacity while replacing
            // the previous rider.  This sentinel is not a free transport
            // slot; input separately checks the frozen authored roster.
            return Some(usize::MAX);
        }
        // InternetHackContain is physically a structure but C++ consumes its
        // authored `Slots` through transport-slot accounting, not the
        // building garrison counter.
        if self.contain_module_kind == crate::game_logic::ContainModuleKind::InternetHack {
            return (self.max_transport > 0).then_some(self.max_transport);
        }
        if self.is_tunnel_network
            || self.is_structure
            || self.max_garrison > 0
            || self.contain_module_kind == crate::game_logic::ContainModuleKind::Garrison
        {
            return (self.max_garrison > 0).then_some(self.max_garrison);
        }

        // `usize::MAX` is the host-frame sentinel for a non-Overlord object;
        // a positive non-sentinel value is the BattleBunker redirect capacity.
        if self.overlord_bunker_capacity != usize::MAX && self.overlord_bunker_capacity > 0 {
            return Some(self.overlord_bunker_capacity);
        }

        (self.max_transport > 0).then_some(self.max_transport)
    }

    /// Whether this frozen target has enough actual ContainModule/typed-role
    /// data to offer a normal player Enter command.
    pub fn supports_normal_enter(&self) -> bool {
        if self.contain_module_kind == crate::game_logic::ContainModuleKind::RiderChange {
            return self.contain_admission != crate::game_logic::ContainAdmission::Unsupported
                && !self.rider_change_allowed_templates.is_empty();
        }
        if self.is_combat_cycle_transport {
            return false;
        }
        if self.contain_module_present {
            return self.contain_admission != crate::game_logic::ContainAdmission::Unsupported;
        }
        self.is_tunnel_network
            || (self.overlord_bunker_capacity != usize::MAX && self.overlord_bunker_capacity > 0)
            || self.is_humvee_transport
            || self.is_listening_outpost_transport
            || self.is_troop_crawler_transport
            || self.is_battle_bus_transport
            || self.is_technical_transport
            || self.is_combat_cycle_transport
            || self.is_combat_chinook_transport
            || self.is_helix_transport
    }

    /// Frozen passenger count for a normal player `MSG_ENTER` order.
    pub fn normal_enter_occupant_count(&self) -> usize {
        if self.is_tunnel_network || self.is_structure || self.max_garrison > 0 {
            self.garrisoned_units
                .len()
                .max(self.occupant_count as usize)
        } else {
            (self.occupant_count as usize).max(self.garrisoned_units.len())
        }
    }

    /// C++ `TransportContain::isValidContainerFor` demands the exact same
    /// controlling player.  GarrisonContain and TunnelContain deliberately
    /// keep their different relationship/body-count behavior.
    #[inline]
    pub fn normal_enter_requires_exact_controller(&self) -> bool {
        use crate::game_logic::ContainModuleKind;

        if self.is_tunnel_network {
            return false;
        }
        if self.overlord_bunker_capacity != usize::MAX && self.overlord_bunker_capacity > 0 {
            return true;
        }
        match self.contain_module_kind {
            ContainModuleKind::Transport
            | ContainModuleKind::RiderChange
            | ContainModuleKind::RailedTransport
            | ContainModuleKind::InternetHack => true,
            ContainModuleKind::Garrison
            | ContainModuleKind::Heal
            | ContainModuleKind::Cave
            | ContainModuleKind::Tunnel => false,
            ContainModuleKind::None => {
                self.is_helix_transport
                    || self.is_battle_bus_transport
                    || self.is_technical_transport
                    || self.is_humvee_transport
                    || self.is_troop_crawler_transport
                    || self.is_combat_chinook_transport
                    || self.is_listening_outpost_transport
            }
        }
    }

    /// Whether frozen capacity is measured in authored passenger slots rather
    /// than bodies.  RiderChange intentionally returns false because it is
    /// never advertised as normal Enter before its roster transaction exists.
    #[inline]
    pub fn normal_enter_uses_transport_slots(&self) -> bool {
        use crate::game_logic::ContainModuleKind;

        if self.contain_module_kind == ContainModuleKind::InternetHack {
            return true;
        }
        if self.is_tunnel_network
            || self.is_structure
            || self.max_garrison > 0
            || self.contain_module_kind == ContainModuleKind::Garrison
        {
            return false;
        }
        matches!(
            self.contain_module_kind,
            ContainModuleKind::Transport | ContainModuleKind::RailedTransport
        ) || (self.overlord_bunker_capacity != usize::MAX && self.overlord_bunker_capacity > 0)
            || self.is_helix_transport
            || self.is_battle_bus_transport
            || self.is_technical_transport
            || self.is_humvee_transport
            || self.is_troop_crawler_transport
            || self.is_combat_chinook_transport
            || self.is_listening_outpost_transport
    }

    /// C++ `AllowInsideKindOf = INFANTRY` role as represented by the active
    /// host containment implementations.  TunnelContain is the deliberate
    /// exception: it accepts non-aircraft units from the shared tunnel pool.
    pub fn normal_enter_requires_infantry(&self) -> bool {
        if self.contain_module_present {
            return self.contain_admission == crate::game_logic::ContainAdmission::InfantryOnly;
        }
        if self.is_tunnel_network {
            return false;
        }

        self.is_structure
            || (self.overlord_bunker_capacity != usize::MAX && self.overlord_bunker_capacity > 0)
            || self.is_humvee_transport
            || self.is_listening_outpost_transport
            || self.is_troop_crawler_transport
            || self.is_battle_bus_transport
            || self.is_technical_transport
            || self.is_combat_cycle_transport
    }

    /// C++ `TunnelContain` and `CombatChinookContain` reject aircraft.
    pub fn normal_enter_forbids_aircraft(&self) -> bool {
        if self.contain_module_present {
            return matches!(
                self.contain_admission,
                crate::game_logic::ContainAdmission::InfantryOnly
                    | crate::game_logic::ContainAdmission::InfantryOrVehicle
            );
        }
        self.is_tunnel_network || self.is_combat_chinook_transport || self.is_helix_transport
    }

    /// C++ OpenContain allows relationships independently of the global
    /// non-owner-empty rule.  The frame resolves the relationship from its
    /// frozen player provenance before calling this method.
    pub fn normal_enter_allows_relationship(
        &self,
        relationship: gamelogic::common::Relationship,
    ) -> bool {
        if !self.contain_module_present {
            return true;
        }
        match relationship {
            gamelogic::common::Relationship::Allies => self.contain_allow_allies_inside,
            gamelogic::common::Relationship::Enemies => self.contain_allow_enemies_inside,
            gamelogic::common::Relationship::Neutral => self.contain_allow_neutral_inside,
        }
    }
}

#[cfg(test)]
mod drawable_shroud_tests {
    use super::*;
    use std::hash::Hash;

    #[test]
    fn drawable_shroud_status_keeps_cxx_ordinals_and_safe_defaults() {
        fn requires_hash<T: Hash>() {}

        let statuses = [
            PresentationObjectShroudStatus::Invalid,
            PresentationObjectShroudStatus::Clear,
            PresentationObjectShroudStatus::PartialClear,
            PresentationObjectShroudStatus::Fogged,
            PresentationObjectShroudStatus::Shrouded,
            PresentationObjectShroudStatus::InvalidButPreviousValid,
        ];
        for (ordinal, status) in statuses.into_iter().enumerate() {
            assert_eq!(status as u8, ordinal as u8);
            assert_eq!(
                status.as_game_logic_status() as u8,
                ordinal as u8,
                "conversion must retain C++ ObjectShroudStatus ordinal {ordinal}"
            );
            assert_eq!(
                PresentationObjectShroudStatus::from(status.as_game_logic_status()),
                status
            );
        }

        requires_hash::<PresentationObjectShroudStatus>();
        requires_hash::<PresentationDrawableLifetime>();
        requires_hash::<PresentationDrawableShroudFacts>();

        let default_from_old_frame: PresentationDrawableShroudFacts =
            serde_json::from_str("{}").expect("missing fields use safe defaults");
        assert_eq!(
            default_from_old_frame,
            PresentationDrawableShroudFacts::default()
        );
        assert_eq!(
            default_from_old_frame.lifetime,
            PresentationDrawableLifetime::Unknown
        );
        assert_eq!(
            default_from_old_frame.raw_status,
            PresentationObjectShroudStatus::Invalid
        );
        assert!(!default_from_old_frame.requires_scene_shroud_material());
        assert!(default_from_old_frame.direct_game_client_status().is_none());

        let partial = PresentationDrawableShroudFacts::direct_host_object(
            PresentationObjectShroudStatus::PartialClear,
            false,
        );
        assert!(partial.requires_scene_shroud_material());
        assert_eq!(
            partial.direct_game_client_status(),
            Some((
                gamelogic::common::types::ObjectShroudStatus::PartialClear,
                false
            ))
        );
        assert!(
            !PresentationDrawableShroudFacts::direct_host_object(
                PresentationObjectShroudStatus::Clear,
                false,
            )
            .requires_scene_shroud_material()
        );
        assert!(
            PresentationDrawableShroudFacts::direct_host_object(
                PresentationObjectShroudStatus::InvalidButPreviousValid,
                true,
            )
            .requires_scene_shroud_material()
        );
    }
}
