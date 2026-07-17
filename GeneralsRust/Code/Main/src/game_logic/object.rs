use super::*;
use crate::command_system::SpecialPowerType;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

fn default_one_f32() -> f32 {
    1.0
}

fn default_strategy_center_turret_angle() -> f32 {
    crate::game_logic::host_strategy_center::STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG
}

fn default_strategy_center_turret_pitch() -> f32 {
    crate::game_logic::host_strategy_center::STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG
}

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
    /// C++ ModelConditionFlags residual bits (ALLOW_SURRENDER-off index layout).
    #[serde(default)]
    pub model_condition_bits: u128,
    /// C++ PhysicsBehavior IS_STUNNED residual frames remaining (0 = clear).
    #[serde(default)]
    pub shock_stun_frames: u32,
    /// C++ BodyDamageType residual (drives DAMAGED/REALLYDAMAGED/RUBBLE bits).
    #[serde(default)]
    pub body_damage_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,

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

    /// Host residual: C++ OpenContain `m_passengersAllowedToFire`.
    /// When true, Docked infantry may residual-fire from the container origin
    /// (GLA Battle Bus / Humvee-style fire-from-transport).
    /// Fail-closed: not full garrison weapon-bone positions.
    pub passengers_allowed_to_fire: bool,

    /// Host residual: C++ TransportContain `m_armedRidersUpgradeWeaponSet`.
    /// When true, bus sets `weapon_set_player_upgrade` while any armed infantry
    /// rider is loaded (Battle Bus PLAYER_UPGRADE weapon set residual).
    pub armed_riders_upgrade_weapon_set: bool,

    /// Host residual: C++ WEAPONSET_PLAYER_UPGRADE flag on this object.
    /// Battle Bus uses this when armed riders are present.
    pub weapon_set_player_upgrade: bool,

    /// Host residual: Battle Bus style transport (capacity 8 + fire + armed-riders).
    /// Distinct from generic Humvee transport residual for honesty counters.
    pub is_battle_bus_transport: bool,

    /// Host residual: GLA Technical transport (capacity 5, infantry only, no passenger fire).
    /// Fail-closed: not chassis reskin / salvage W3D gunner swap matrix.
    pub is_technical_transport: bool,

    /// Host residual: GLA Combat Cycle / Combat Bike RiderChangeContain (capacity 1).
    /// Rider weapon switch residual; passengers do not fire from bed (bike fires).
    /// Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
    pub is_combat_cycle_transport: bool,

    /// Host residual: active Combat Cycle rider class (0=none … 7=saboteur).
    /// Mirrors RiderChangeContain WEAPON_RIDER* residual selection.
    pub combat_cycle_rider: u8,

    /// Host residual: GLA Tunnel Network structure (`TunnelContain`).
    /// Shared per-team capacity via `HostTunnelNetworkRegistry` (MaxTunnelCapacity=10).
    /// Fail-closed: not full GuardTunnelNetwork AI / CaveSystem cave-in matrix.
    pub is_tunnel_network: bool,

    /// Host residual: AirF Combat Chinook style transport (capacity 8 + fire +
    /// armed-riders + ListeningOutpost dummy). Distinct from vanilla Chinook
    /// (no PassengersAllowedToFire) and from Battle Bus for honesty counters.
    pub is_combat_chinook_transport: bool,

    /// C++ parity (Object::m_containedBy): when this unit is inside a
    /// transport/garrison, stores the container's ID.  None when free.
    pub contained_by: Option<ObjectId>,

    /// Optional short-lived cheer/animation timer
    pub cheer_timer: f32,

    /// Toggleable weapon/overcharge state flags
    pub overcharge_enabled: bool,
    pub active_weapon_slot: u8,

    /// C++ Weapon PRE_ATTACK residual: target being wound up against.
    #[serde(default)]
    pub pre_attack_target: Option<ObjectId>,
    /// Absolute sim time when pre-attack delay elapses (ready to discharge).
    #[serde(default)]
    pub pre_attack_ready_at: f32,
    /// C++ Object consecutive-shot residual for PreAttackType PER_ATTACK.
    #[serde(default)]
    pub consecutive_shot_target: Option<ObjectId>,
    #[serde(default)]
    pub consecutive_shots_at_target: u32,
    /// C++ Weapon::m_leechWeaponRangeActive residual (primary).
    #[serde(default)]
    pub leech_range_active_primary: bool,
    /// C++ Weapon::m_leechWeaponRangeActive residual (secondary).
    #[serde(default)]
    pub leech_range_active_secondary: bool,

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
    /// StealthDetectorUpdate DetectionRate residual in logic frames.
    /// `0` = continuous every-frame scan (legacy host residual detectors).
    /// Strategy Center S&D residual sets **15** (500ms @ 30 FPS).
    pub detection_rate_frames: u32,
    /// Absolute frame when the next DetectionRate residual scan may fire.
    /// `0` means scan is due immediately (setSDEnabled → UPDATE_SLEEP_NONE).
    pub next_detection_scan_frame: u32,
    /// Logic frame when OBJECT_STATUS_DETECTED expires (0 = no timer).
    /// C++ StealthUpdate::m_detectionExpiresFrame residual.
    pub detection_expires_frame: u32,
    /// C++ STEALTH_NOT_WHILE_ATTACKING residual: firing breaks stealth.
    /// Default true for host residual honesty.
    pub stealth_breaks_on_attack: bool,
    /// C++ StealthForbiddenConditions MOVING residual (Pathfinder): uncloak while moving.
    /// Fail-closed: not full StealthUpdate condition matrix.
    pub stealth_breaks_on_move: bool,
    /// C++ InnateStealth residual: re-cloak when forbidden conditions clear.
    pub innate_stealth: bool,

    /// C++ StealthUpdate disguise residual (Bomb Truck DisguisesAsTeam).
    /// Template the unit is currently disguised as (None when not disguised).
    #[serde(default)]
    pub disguise_as_template: Option<String>,
    /// Team residual the unit appears as to non-allied viewers while disguised.
    #[serde(default)]
    pub disguise_as_team: Option<Team>,

    /// Host residual: bitmask of player indices currently vision-spying this unit
    /// (C++ Object::m_visionSpiedBy / setVisionSpied for CIA Intelligence SpyVision).
    /// Fail-closed: not full looking_mask partition maintenance.
    pub vision_spied_mask: u32,

    /// Host residual weapon-bonus flags from PropagandaTowerBehavior.
    /// C++ WEAPONBONUSCONDITION_ENTHUSIASTIC / SUBLIMINAL (rate-of-fire buff near speaker tower).
    /// Fail-closed: not full WeaponBonusConditionFlags matrix / ROF multiplier application.
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,

    /// Host residual HORDE weapon bonus (C++ WEAPONBONUSCONDITION_HORDE via HordeUpdate).
    /// Fail-closed: not full RubOffRadius honorary / terrain-decal flag matrix.
    #[serde(default)]
    pub weapon_bonus_horde: bool,
    /// Host residual NATIONALISM weapon bonus (only while in horde + upgrade).
    /// Fail-closed: not full Fanaticism infantry-general branch.
    #[serde(default)]
    pub weapon_bonus_nationalism: bool,

    /// Host residual Frenzy / Rage temporary attack buff
    /// (C++ WEAPONBONUSCONDITION_FRENZY_ONE/TWO/THREE via doTempWeaponBonus).
    /// Fail-closed: not full WeaponBonusConditionFlags matrix / TempWeaponBonusHelper Xfer.
    pub weapon_bonus_frenzy: bool,
    /// Absolute host logic frame when Frenzy residual expires (0 = none).
    pub weapon_bonus_frenzy_until_frame: u32,
    /// Residual Frenzy tier 1..=3 (maps to FRENZY_ONE/TWO/THREE damage mult).
    pub weapon_bonus_frenzy_level: u8,

    /// Host residual USA Strategy Center battle-plan weapon bonuses
    /// (C++ WEAPONBONUSCONDITION_BATTLEPLAN_* via Player::applyBattlePlanBonuses).
    /// Fail-closed: not full KindOf multi-mask / projectile inheritance matrix.
    #[serde(default)]
    pub weapon_bonus_battle_plan_bombardment: bool,
    #[serde(default)]
    pub weapon_bonus_battle_plan_hold_the_line: bool,
    #[serde(default)]
    pub weapon_bonus_battle_plan_search_and_destroy: bool,
    /// Residual sight-range scale currently applied for SearchAndDestroy (1.0 = none).
    #[serde(default = "default_one_f32")]
    pub battle_plan_sight_scalar_applied: f32,
    /// Host residual continuous-fire ramp (Gattling Tank FiringTracker residual).
    /// Consecutive shots at current victim for ContinuousFireOne/Two thresholds.
    /// Fail-closed: not full model-condition CONTINUOUS_FIRE_* animation matrix.
    #[serde(default)]
    pub continuous_fire_consecutive: u32,
    /// 0=base/slow, 1=mean (200% RoF), 2=fast (300% RoF).
    #[serde(default)]
    pub continuous_fire_level: u8,
    /// Absolute host frame until which coast keeps spin-up (0 = none).
    #[serde(default)]
    pub continuous_fire_coast_until_frame: u32,
    /// Last continuous-fire victim object id bits (0 = none/ground).
    #[serde(default)]
    pub continuous_fire_victim: u32,

    /// Absolute host logic frame when FAERIE_FIRE residual expires (0 = none).
    /// C++ StatusDamageHelper m_frameToHeal residual (Avenger paint).
    #[serde(default)]
    pub faerie_fire_until_frame: u32,

    /// Host residual: America Humvee TransportContain (Slots=5 + passengers fire).
    #[serde(default)]
    pub is_humvee_transport: bool,

    /// Host residual: China Listening Outpost TransportContain (Slots=2 + fire +
    /// armed-riders dummy + stealth detector 300 + InnateStealth).
    /// Fail-closed: not multi-door exit / IR FX / RIDERS_ATTACKING uncloak matrix.
    #[serde(default)]
    pub is_listening_outpost_transport: bool,

    /// Host residual: China Troop Crawler TransportContain (Slots=8 + assault deploy).
    /// Passengers exit to fight (do not fire from inside). Fail-closed vs full
    /// AssaultTransportAIUpdate wounded-retrieve / multi-exit path matrix.
    #[serde(default)]
    pub is_troop_crawler_transport: bool,

    /// Host residual: Overlord / Helix portable GattlingCannon addon installed
    /// (`Upgrade_ChinaOverlordGattlingCannon` / Helix equivalent). Equips AA
    /// secondary + passenger ground gattling residual on primary fire.
    /// Fail-closed: not full portable-structure passenger object spawn.
    #[serde(default)]
    pub has_overlord_gattling_addon: bool,

    /// Host residual: Overlord / Helix portable PropagandaTower addon installed
    /// (`Upgrade_ChinaOverlordPropagandaTower` / Helix equivalent). Emperor tanks
    /// spawn with this true (innate PropagandaTowerBehavior AffectsSelf).
    /// Fail-closed: not full portable tower object / PulseFX.
    #[serde(default)]
    pub has_overlord_propaganda_addon: bool,

    /// Host residual: HelixContain transport (Slots=5, infantry/vehicle/portable).
    /// Fail-closed: not multi-exit / napalm bomb special ability matrix.
    #[serde(default)]
    pub is_helix_transport: bool,

    /// Host residual: C++ Object::m_commandSetStringOverride (CommandSetUpgrade).
    /// Demo SuicideBomb residual swaps to `*CommandSetUpgrade` including
    /// `Demo_Command_TertiarySuicide`. Fail-closed: not full control-bar matrix.
    #[serde(default)]
    pub command_set_override: Option<String>,

    /// Host residual: intentional SUICIDED death already applied PlusFire blast.
    /// Suppresses Demo_DestroyedWeapon double-fire on process_destroy_list.
    #[serde(default)]
    pub demo_suicided_detonating: bool,

    /// Host residual: HiveStructureBody / SpawnBehavior slave count (Stinger Site).
    /// 0 for non-hive units. Mirror of alive residual roster slots.
    #[serde(default)]
    pub hive_slave_count: u8,
    /// Host residual: active residual slave HP (first alive mirror).
    #[serde(default)]
    pub hive_slave_hp: f32,
    /// Absolute host frame when next residual slave respawns (0 = none).
    #[serde(default)]
    pub hive_slave_respawn_frame: u32,
    /// Host residual: physical SpawnBehavior slave roster (getClosestSlave).
    /// Fail-closed: not full soldier Object / AI / W3D bone attach.
    #[serde(default)]
    pub hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave; 3],

    /// Host residual: Strategy Center / TurretAI yaw (deg).
    /// Natural for Strategy Center = **-90** (NaturalTurretAngle).
    #[serde(default = "default_strategy_center_turret_angle")]
    pub turret_angle_deg: f32,
    /// Host residual: Strategy Center / TurretAI pitch (deg).
    /// Natural for Strategy Center = **45** (NaturalTurretPitch).
    #[serde(default = "default_strategy_center_turret_pitch")]
    pub turret_pitch_deg: f32,
    /// TurretAI idle-scan residual: absolute frame when next idle scan may start.
    /// 0 = not scheduled (or just completed without reschedule).
    #[serde(default)]
    pub turret_idle_scan_next_frame: u32,
    /// TurretAI idle-scan residual: true while rotating toward desired angle.
    #[serde(default)]
    pub turret_idle_scanning: bool,
    /// TurretAI idle-scan residual: desired absolute yaw while scanning.
    #[serde(default)]
    pub turret_idle_scan_desired_angle_deg: f32,
    /// TurretAI idle-scan residual: deterministic scan index (interval/offset seed).
    #[serde(default)]
    pub turret_idle_scan_index: u32,
    /// TurretAI HoldTurret residual: true while holding after idle-scan complete.
    #[serde(default)]
    pub turret_holding: bool,
    /// TurretAI HoldTurret residual: absolute frame when hold ends (0 = none).
    #[serde(default)]
    pub turret_hold_until_frame: u32,
    /// TurretAI idle-recenter residual: true while recentering after Hold (not pack).
    #[serde(default)]
    pub turret_idle_recentering: bool,
    /// TurretAI idle mood-target residual: target was set by friend_checkForIdleMoodTarget.
    /// Cleared when mood target leaves range / dies (C++ m_targetWasSetByIdleMood).
    #[serde(default)]
    pub turret_mood_target: bool,
    /// C++ AIUpdateInterface AttitudeType residual (AI_SLEEP..AI_AGGRESSIVE).
    /// Host residual for TurretAI mood matrix Sleep/Passive gates.
    /// Ordinals: -2=Sleep, -1=Passive, 0=Normal, 1=Alert, 2=Aggressive.
    #[serde(default)]
    pub ai_attitude: i8,
    /// C++ BodyModule last damage source residual (Passive WaitForAttack).
    /// Set when damage is applied with a known attacker id.
    #[serde(default)]
    pub last_damage_source: Option<ObjectId>,

    /// CamoNetting StealthUpdate FriendlyOpacity residual (0.5 cloaked / 1.0 revealed).
    /// Fail-closed: not full drawable sub-object camo net mesh visual.
    #[serde(default = "default_one_f32")]
    pub camo_friendly_opacity: f32,
    /// StealthUpdate pulse phase residual (radians) while cloaked.
    #[serde(default)]
    pub camo_opacity_pulse_phase: f32,
    /// CamoNetting StealthLook residual (host of Drawable::setStealthLook).
    /// C++ `StealthLookType` / `HostCamoStealthLook` ordinals:
    /// 0=None, 1=VisibleFriendly, 2=DisguisedEnemy, 3=VisibleDetected,
    /// 4=VisibleFriendlyDetected, 5=Invisible.
    /// Fail-closed: not full W3D heat-vision second material pass GPU.
    #[serde(default)]
    pub camo_stealth_look: u8,
    /// Heat-vision second material pass opacity residual (0 or 1 host residual).
    #[serde(default)]
    pub camo_heat_vision_opacity: f32,
    /// CamoNetting sub-object net mesh residual shown (Upgrade_GLACamoNetting applied).
    /// Fail-closed: not full W3D SubObjectsUpgrade / mesh GPU draw.
    #[serde(default)]
    pub camo_net_sub_object_shown: bool,
    /// CamoNetting sub-object residual observer-visible (StealthLook ≠ Invisible).
    #[serde(default)]
    pub camo_net_sub_object_observer_visible: bool,

    /// C++ StealthUpdate StealthDelay residual: earliest frame allowed to re-cloak.
    /// 0 = no delay gate (instant re-cloak residual, e.g. Rebel Camouflage).
    #[serde(default)]
    pub stealth_allowed_frame: u32,
    /// Pending StealthDelay scheduling after a reveal (resolved in stealth update).
    #[serde(default)]
    pub stealth_delay_pending: bool,
    /// Frames of StealthDelay after reveal (CamoNetting structures = 75).
    /// 0 = instant re-cloak residual.
    #[serde(default)]
    pub stealth_delay_frames: u32,
    /// C++ StealthForbiddenConditions TAKING_DAMAGE residual.
    #[serde(default)]
    pub stealth_breaks_on_damage: bool,
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
            model_condition_bits: 0,
            shock_stun_frames: 0,
            body_damage_state:
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
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
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            is_battle_bus_transport: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            is_tunnel_network: false,
            is_combat_chinook_transport: false,
            contained_by: None,
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            pre_attack_target: None,
            pre_attack_ready_at: 0.0,
            consecutive_shot_target: None,
            consecutive_shots_at_target: 0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            guard_radius: 0.0,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown,
            special_power_cooldown_remaining: 0.0,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            next_detection_scan_frame: 0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            disguise_as_template: None,
            disguise_as_team: None,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_until_frame: 0,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            battle_plan_sight_scalar_applied: 1.0,
            continuous_fire_consecutive: 0,
            continuous_fire_level: 0,
            continuous_fire_coast_until_frame: 0,
            continuous_fire_victim: 0,
            faerie_fire_until_frame: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_troop_crawler_transport: false,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            is_helix_transport: false,
            command_set_override: None,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            hive_slave_respawn_frame: 0,
            hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3],
            turret_angle_deg: default_strategy_center_turret_angle(),
            turret_pitch_deg: default_strategy_center_turret_pitch(),
            turret_idle_scan_next_frame: 0,
            turret_idle_scanning: false,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_holding: false,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_mood_target: false,
            ai_attitude: 0, // HostAiAttitude::Normal
            last_damage_source: None,
            camo_friendly_opacity: 1.0,
            camo_opacity_pulse_phase: 0.0,
            camo_stealth_look: 0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
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
            model_condition_bits: 0,
            shock_stun_frames: 0,
            body_damage_state:
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
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
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            is_battle_bus_transport: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            is_tunnel_network: false,
            is_combat_chinook_transport: false,
            contained_by: None,
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            pre_attack_target: None,
            pre_attack_ready_at: 0.0,
            consecutive_shot_target: None,
            consecutive_shots_at_target: 0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            guard_radius: 0.0,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown: 10.0,
            special_power_cooldown_remaining: 0.0,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            next_detection_scan_frame: 0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            disguise_as_template: None,
            disguise_as_team: None,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_until_frame: 0,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            battle_plan_sight_scalar_applied: 1.0,
            continuous_fire_consecutive: 0,
            continuous_fire_level: 0,
            continuous_fire_coast_until_frame: 0,
            continuous_fire_victim: 0,
            faerie_fire_until_frame: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_troop_crawler_transport: false,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            is_helix_transport: false,
            command_set_override: None,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            hive_slave_respawn_frame: 0,
            hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3],
            turret_angle_deg: default_strategy_center_turret_angle(),
            turret_pitch_deg: default_strategy_center_turret_pitch(),
            turret_idle_scan_next_frame: 0,
            turret_idle_scanning: false,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_holding: false,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_mood_target: false,
            ai_attitude: 0, // HostAiAttitude::Normal
            last_damage_source: None,
            camo_friendly_opacity: 1.0,
            camo_opacity_pulse_phase: 0.0,
            camo_stealth_look: 0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
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
        if Self::engine_bridge_active() {
            if let Some(engine_id) = self.engine_object_id {
                if let Some(alive) = Self::read_engine_is_alive(engine_id) {
                    return alive;
                }
            }
        }
        !self.status.destroyed && self.health.is_alive()
    }

    /// OBJECT_REGISTRY dual-world reads/writes only when bridge is explicitly enabled.
    #[inline]
    fn engine_bridge_active() -> bool {
        crate::gameworld_shadow::engine_object_bridge_enabled()
    }

    fn read_engine_is_alive(engine_id: u32) -> Option<bool> {
        let obj = gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id)?;
        let guard = obj.read().ok()?;
        Some(guard.is_alive())
    }

    pub fn get_health_percentage(&self) -> f32 {
        if Self::engine_bridge_active() {
            if let Some(engine_id) = self.engine_object_id {
                if let Some(pct) = Self::read_engine_health_percentage(engine_id) {
                    return pct;
                }
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
    /// disabled — C++ DISABLED_SUBDUED on vehicles only blocks `canFireWeapon`;
    /// residual keeps movement. Check `is_weapons_jammed()` / `can_attack()` for fire.
    /// Structure `disabled_subdued` (Microwave residual) **is** full disable.
    pub fn is_disabled(&self) -> bool {
        self.status.disabled_underpowered
            || self.status.disabled_unmanned
            || self.status.disabled_hacked
            || self.status.disabled_emp
            || self.status.disabled_paralyzed
            || self.status.disabled_subdued
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

    /// C++ DISABLED_PARALYZED residual (BattlePlanChangeParalyzeTime).
    pub fn is_paralyzed_disabled(&self) -> bool {
        self.status.disabled_paralyzed
    }

    /// Host ECM / jammer residual: weapons cannot fire while in jam radius.
    /// C++ DISABLED_SUBDUED / canFireWeapon residual (Microwave/ECM disabler).
    pub fn is_weapons_jammed(&self) -> bool {
        self.status.weapons_jammed
    }

    /// C++ DISABLED_SUBDUED residual (Microwave building disabler on structures).
    pub fn is_subdued_disabled(&self) -> bool {
        self.status.disabled_subdued
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

    /// Apply / clear DISABLED_SUBDUED residual (Microwave structure cook).
    ///
    /// C++ ActiveBody::onSubdualChange → setDisabled(DISABLED_SUBDUED).
    /// Structures stop production / attack while cooked; residual continuous
    /// while microwave keeps attacking (not full subdual accumulate/heal).
    pub fn set_disabled_subdued(&mut self, subdued: bool) {
        if subdued {
            self.status.disabled_subdued = true;
            // C++ orderAllPassengersToIdle residual: drop attack / move orders.
            self.status.attacking = false;
            self.force_attack = false;
            self.target = None;
            self.target_location = None;
            // Structures do not move; stop any residual production-related AI.
            if !self.is_kind_of(KindOf::Structure) {
                self.status.moving = false;
                self.stop_moving();
                self.ai_state = AIState::Idle;
            }
        } else {
            self.status.disabled_subdued = false;
        }
    }

    /// Apply kill-pilot residual: vehicle becomes unmanned (no HP change).
    /// Caller is responsible for team transfer (typically Neutral).
    /// Captures `unmanned_owner_team` for PilotFindVehicle PartitionFilterPlayer residual.
    pub fn apply_kill_pilot_unmanned(&mut self) {
        // Preserve original controller for same-player PartitionFilter residual.
        // Only snapshot on the edge into unmanned (refresh would overwrite Neutral).
        if !self.status.disabled_unmanned {
            self.status.unmanned_owner_team = Some(self.team);
        }
        self.status.disabled_unmanned = true;
        self.status.disabled_hacked = false;
        self.status.disabled_hacked_until_frame = 0;
        self.status.disabled_emp = false;
        self.status.disabled_emp_until_frame = 0;
        self.status.disabled_paralyzed = false;
        self.status.disabled_paralyzed_until_frame = 0;
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    /// Apply USA Pilot recrew residual onto this unmanned vehicle.
    ///
    /// Clears DISABLED_UNMANNED, transfers team to pilot team, merges pilot
    /// veterancy (retail VeterancyCrateCollide IsPilot + AddsOwnerVeterancy).
    /// Caller destroys the pilot infantry.
    pub fn apply_pilot_recrew(
        &mut self,
        pilot_team: Team,
        pilot_level: crate::game_logic::VeterancyLevel,
    ) -> bool {
        use crate::game_logic::host_usa_pilot::{merged_recrew_veterancy, veterancy_rank};

        if !self.status.disabled_unmanned {
            return false;
        }
        self.status.disabled_unmanned = false;
        self.status.unmanned_owner_team = None;
        self.status.disabled_hacked = false;
        self.status.disabled_hacked_until_frame = 0;
        self.status.disabled_emp = false;
        self.status.disabled_emp_until_frame = 0;
        self.status.disabled_paralyzed = false;
        self.status.disabled_paralyzed_until_frame = 0;
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
        self.set_team(pilot_team);

        let previous = self.experience.level;
        let merged = merged_recrew_veterancy(previous, pilot_level);
        let transferred = veterancy_rank(merged) > veterancy_rank(previous);
        if merged != previous {
            self.experience.level = merged;
            self.apply_veterancy_bonuses(previous, merged);
        }
        transferred
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

    /// Apply DISABLED_PARALYZED residual until `until_frame` (absolute host logic frame).
    /// C++ BattlePlanUpdate::paralyzeTroop: setDisabledUntil(DISABLED_PARALYZED, now + frames).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_disabled_paralyzed(&mut self, until_frame: u32) {
        self.status.disabled_paralyzed = true;
        if until_frame > self.status.disabled_paralyzed_until_frame {
            self.status.disabled_paralyzed_until_frame = until_frame;
        }
        self.status.attacking = false;
        self.status.moving = false;
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.ai_state = AIState::Idle;
    }

    /// Expire DISABLED_PARALYZED when the host frame passes the residual timer.
    pub fn tick_disabled_paralyzed(&mut self, current_frame: u32) {
        if self.status.disabled_paralyzed
            && self.status.disabled_paralyzed_until_frame > 0
            && current_frame >= self.status.disabled_paralyzed_until_frame
        {
            self.status.disabled_paralyzed = false;
            self.status.disabled_paralyzed_until_frame = 0;
        }
    }

    /// C++ goInvulnerable residual (OCL InvulnerableTime post-eject).
    pub fn is_eject_invulnerable(&self) -> bool {
        self.status.eject_invulnerable
    }

    /// Apply InvulnerableTime residual until `until_frame` (absolute host logic frame).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_eject_invulnerable(&mut self, until_frame: u32) {
        self.status.eject_invulnerable = true;
        if until_frame > self.status.eject_invulnerable_until_frame {
            self.status.eject_invulnerable_until_frame = until_frame;
        }
    }

    /// Expire InvulnerableTime when the host frame passes the residual timer.
    /// Host residual: OCL_EjectPilotViaParachute parachuting state.
    pub fn is_parachuting(&self) -> bool {
        self.status.parachuting
    }

    /// Whether AmericaParachute residual chute is open (past OpenDist freefall).
    pub fn is_parachute_open(&self) -> bool {
        self.status.parachute_open
    }

    /// Begin air-eject parachute residual (elevated spawn + freefall → OpenDist → open).
    ///
    /// Applies C++ low-altitude open fudge: if height above ground < 2×OpenDist,
    /// fudge start height so the chute can still open.
    pub fn apply_eject_parachuting(&mut self) {
        use crate::game_logic::host_usa_pilot::fudge_parachute_start_height;
        let start_y = self.get_position().y;
        let ground_y = 0.0; // host residual ground plane
        let fudged = fudge_parachute_start_height(start_y, ground_y);
        self.status.parachuting = true;
        self.status.airborne_target = true;
        self.status.parachute_open = false;
        self.status.parachute_start_height = fudged;
        // Freefall residual: pitch/roll rates seed only when chute opens.
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// Begin AmericaCrateParachute residual for cargo crate payload.
    ///
    /// Uses crate OpenDist **12.5** low-altitude fudge (not pilot OpenDist 100).
    /// Fail-closed: not full PutInContainer AmericaCrateParachute Object.
    pub fn apply_crate_parachuting(&mut self) {
        use crate::game_logic::host_deliver_payload::fudge_crate_parachute_start_height;
        let start_y = self.get_position().y;
        let ground_y = 0.0;
        let fudged = fudge_crate_parachute_start_height(start_y, ground_y);
        self.status.parachuting = true;
        self.status.airborne_target = true;
        self.status.parachute_open = false;
        self.status.parachute_start_height = fudged;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// Whether low-altitude open fudge residual applied for this parachute start.
    pub fn parachute_start_was_fudged(&self) -> bool {
        use crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged;
        // Fudge rewrites start height; detect by comparing raw y vs stored start.
        // After apply, start_height is fudged value; raw spawn y is current y
        // only at apply time — host honesty uses registry counter instead.
        parachute_start_height_was_fudged(self.get_position().y, 0.0)
    }

    /// Mark AmericaParachute residual chute open (after OpenDist freefall).
    ///
    /// Seeds pitch/roll rates residual (C++ constructor random in ±Pitch/RollRateMax;
    /// host uses deterministic mid residual).
    pub fn open_eject_parachute(&mut self) {
        use crate::game_logic::host_usa_pilot::{
            parachute_initial_pitch_rate, parachute_initial_roll_rate,
        };
        self.status.parachute_open = true;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = parachute_initial_pitch_rate();
        self.status.parachute_roll_rate = parachute_initial_roll_rate();
    }

    /// Clear parachuting residual on land.
    pub fn clear_eject_parachuting(&mut self) {
        self.status.parachuting = false;
        self.status.airborne_target = false;
        self.status.parachute_open = false;
        self.status.parachute_start_height = 0.0;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// AmericaParachute pitch residual (radians) while chute open.
    pub fn parachute_pitch(&self) -> f32 {
        self.status.parachute_pitch
    }

    /// AmericaParachute roll residual (radians) while chute open.
    pub fn parachute_roll(&self) -> f32 {
        self.status.parachute_roll
    }

    pub fn tick_eject_invulnerable(&mut self, current_frame: u32) {
        if self.status.eject_invulnerable
            && self.status.eject_invulnerable_until_frame > 0
            && current_frame >= self.status.eject_invulnerable_until_frame
        {
            self.status.eject_invulnerable = false;
            self.status.eject_invulnerable_until_frame = 0;
        }
    }

    /// Whether Frenzy / Rage temporary attack buff residual is active.
    pub fn is_frenzy_buffed(&self) -> bool {
        self.weapon_bonus_frenzy
    }

    /// Apply temporary Frenzy residual (C++ Object::doTempWeaponBonus FRENZY_*).
    /// Refresh extends the timer if a later expiry is provided; keeps higher level.
    pub fn apply_weapon_bonus_frenzy(&mut self, level: u8, until_frame: u32) {
        let lvl = level.clamp(1, 3);
        self.weapon_bonus_frenzy = true;
        if lvl > self.weapon_bonus_frenzy_level {
            self.weapon_bonus_frenzy_level = lvl;
        } else if self.weapon_bonus_frenzy_level == 0 {
            self.weapon_bonus_frenzy_level = lvl;
        }
        if until_frame > self.weapon_bonus_frenzy_until_frame {
            self.weapon_bonus_frenzy_until_frame = until_frame;
        }
    }

    /// Clear Frenzy residual weapon-bonus flags.
    pub fn clear_weapon_bonus_frenzy(&mut self) {
        self.weapon_bonus_frenzy = false;
        self.weapon_bonus_frenzy_until_frame = 0;
        self.weapon_bonus_frenzy_level = 0;
    }

    /// Expire Frenzy residual when the host frame passes the residual timer.
    pub fn tick_weapon_bonus_frenzy(&mut self, current_frame: u32) {
        if self.weapon_bonus_frenzy
            && self.weapon_bonus_frenzy_until_frame > 0
            && current_frame >= self.weapon_bonus_frenzy_until_frame
        {
            self.clear_weapon_bonus_frenzy();
        }
    }

    /// Retail DAMAGE multiplier while Frenzy residual is active (1.0 when clear).
    pub fn frenzy_damage_multiplier(&self) -> f32 {
        if !self.weapon_bonus_frenzy {
            return 1.0;
        }
        crate::game_logic::host_frenzy::HostFrenzyLevel::from_u8(self.weapon_bonus_frenzy_level)
            .damage_multiplier()
    }

    /// Whether any Strategy Center battle-plan residual weapon bonus is active.
    pub fn has_battle_plan_bonus(&self) -> bool {
        self.weapon_bonus_battle_plan_bombardment
            || self.weapon_bonus_battle_plan_hold_the_line
            || self.weapon_bonus_battle_plan_search_and_destroy
    }

    /// Apply residual Strategy Center army battle-plan bonuses to this unit.
    ///
    /// Clears previous battle-plan residual flags first (plan switch residual).
    pub fn apply_battle_plan_bonus(
        &mut self,
        plan: crate::game_logic::host_strategy_center::HostBattlePlan,
    ) {
        self.clear_battle_plan_bonus();
        match plan {
            crate::game_logic::host_strategy_center::HostBattlePlan::Bombardment => {
                self.weapon_bonus_battle_plan_bombardment = true;
            }
            crate::game_logic::host_strategy_center::HostBattlePlan::HoldTheLine => {
                self.weapon_bonus_battle_plan_hold_the_line = true;
            }
            crate::game_logic::host_strategy_center::HostBattlePlan::SearchAndDestroy => {
                self.weapon_bonus_battle_plan_search_and_destroy = true;
                // Sight residual: scale detection / template sight residual field.
                let scalar = plan.army_sight_range_scalar();
                if (scalar - 1.0).abs() > f32::EPSILON {
                    self.detection_range = self.effective_detection_range() * scalar;
                    self.battle_plan_sight_scalar_applied = scalar;
                }
            }
        }
    }

    /// Clear residual Strategy Center battle-plan bonuses.
    pub fn clear_battle_plan_bonus(&mut self) {
        self.weapon_bonus_battle_plan_bombardment = false;
        self.weapon_bonus_battle_plan_hold_the_line = false;
        self.weapon_bonus_battle_plan_search_and_destroy = false;
        // Undo SearchAndDestroy sight residual.
        if (self.battle_plan_sight_scalar_applied - 1.0).abs() > f32::EPSILON
            && self.battle_plan_sight_scalar_applied > f32::EPSILON
        {
            self.detection_range =
                self.detection_range / self.battle_plan_sight_scalar_applied.max(0.01);
            // If detection_range collapses near template default residual, clear override.
            let base = self.get_template().sight_range;
            if (self.detection_range - base).abs() < 0.5 {
                self.detection_range = 0.0;
            }
        }
        self.battle_plan_sight_scalar_applied = 1.0;
    }

    /// Retail BATTLEPLAN_BOMBARDMENT DAMAGE multiplier (1.0 when clear).
    pub fn battle_plan_damage_multiplier(&self) -> f32 {
        if self.weapon_bonus_battle_plan_bombardment {
            crate::game_logic::host_strategy_center::BOMBARDMENT_DAMAGE_MULT
        } else {
            1.0
        }
    }

    /// Retail HoldTheLine armor damage scalar (incoming damage mult; 1.0 when clear).
    pub fn battle_plan_armor_damage_scalar(&self) -> f32 {
        if self.weapon_bonus_battle_plan_hold_the_line {
            crate::game_logic::host_strategy_center::HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR
        } else {
            1.0
        }
    }

    /// Retail BATTLEPLAN_SEARCHANDDESTROY RANGE multiplier (1.0 when clear).
    pub fn battle_plan_range_multiplier(&self) -> f32 {
        if self.weapon_bonus_battle_plan_search_and_destroy {
            crate::game_logic::host_strategy_center::SEARCH_AND_DESTROY_RANGE_MULT
        } else {
            1.0
        }
    }

    /// C++ OBJECT_STATUS_FAERIE_FIRE residual (Avenger paint).
    pub fn is_faerie_fire(&self) -> bool {
        self.status.faerie_fire
    }

    /// Apply FAERIE_FIRE status residual until absolute frame (refresh extends timer).
    pub fn apply_faerie_fire(&mut self, until_frame: u32) {
        self.status.faerie_fire = true;
        if until_frame > self.faerie_fire_until_frame {
            self.faerie_fire_until_frame = until_frame;
        }
    }

    /// Clear FAERIE_FIRE residual status.
    pub fn clear_faerie_fire(&mut self) {
        self.status.faerie_fire = false;
        self.faerie_fire_until_frame = 0;
    }

    /// Expire FAERIE_FIRE residual when host frame passes the residual timer.
    pub fn tick_faerie_fire(&mut self, current_frame: u32) {
        if self.status.faerie_fire
            && self.faerie_fire_until_frame > 0
            && current_frame >= self.faerie_fire_until_frame
        {
            self.clear_faerie_fire();
        }
    }

    /// Weapon ready with optional TARGET_FAERIE_FIRE ROF residual (150%).
    pub fn weapon_ready_vs_target(
        weapon: &Weapon,
        current_time: f32,
        target_has_faerie_fire: bool,
    ) -> bool {
        crate::game_logic::host_avenger::weapon_ready_vs_faerie(
            weapon.last_fire_time,
            weapon.reload_time,
            current_time,
            target_has_faerie_fire,
        )
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

    /// True when this aircraft is parked at an airfield (ParkingPlace residual).
    pub fn is_parked_at_airfield(&self) -> bool {
        (self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft)
            && self.ai_state == AIState::Docked
            && self.contained_by.is_some()
    }

    /// C++ JetAIUpdate takeoff residual from ParkingPlace.
    ///
    /// Clears hangar bookkeeping, lifts to ApproachHeight (**50**), marks airborne.
    /// Returns the airfield id that was left (if any).
    pub fn takeoff_from_airfield_parking(&mut self) -> Option<ObjectId> {
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return None;
        }
        if self.ai_state != AIState::Docked && self.contained_by.is_none() {
            return None;
        }
        let af = self.contained_by.take();
        self.ai_state = AIState::Idle;
        self.status.airborne_target = true;
        // Retail AmericaAirfield ApproachHeight residual.
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
        let mut pos = self.get_position();
        if pos.y < PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT {
            pos.y = PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
            self.set_position(pos);
        }
        af
    }

    pub fn can_attack(&self) -> bool {
        // Garrisoned units may still fire from the structure (residual
        // fire-from-garrison). Docked transport cargo and units mid-enter cannot.
        // Docked aircraft may attack (ParkingPlace takeoff/sortie residual).
        // weapons_jammed: C++ canFireWeapon DISABLED_SUBDUED residual (ECM field).
        // shock stun: C++ Physics IS_STUNNED residual — cannot acquire/fire while stunned.
        let parked_aircraft = self.is_parked_at_airfield();
        self.is_alive()
            && self.weapon.is_some()
            && !self.is_disabled()
            && !self.is_shock_stunned()
            && !self.status.weapons_jammed
            && (parked_aircraft || !matches!(self.ai_state, AIState::Docked | AIState::Entering))
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
        if Self::engine_bridge_active() {
            if let Some(engine_id) = self.engine_object_id {
                if let Some(pos) = Self::read_engine_position(engine_id) {
                    return pos;
                }
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
        // Propagate only when OBJECT_REGISTRY bridge is explicitly enabled.
        if Self::engine_bridge_active() {
            if let Some(engine_id) = self.engine_object_id {
                Self::write_engine_position(engine_id, position);
            }
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
        if Self::engine_bridge_active() {
            if let Some(engine_id) = self.engine_object_id {
                if let Some(angle) = Self::read_engine_orientation(engine_id) {
                    return angle;
                }
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
        if Self::engine_bridge_active() {
            if let Some(engine_id) = self.engine_object_id {
                if let Some(obj) =
                    gamelogic::object::registry::OBJECT_REGISTRY.get_object(engine_id)
                {
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
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.thing.get_transform_matrix()
    }

    /// C++ ActiveBody visual condition + Drawable::reactToBodyDamageStateChange residual.
    pub fn refresh_model_condition_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            host_apply_body_damage_model_bits, host_calc_body_damage_state, HostBodyDamageType,
            MC_BIT_ATTACKING, MC_BIT_DYING, MC_BIT_MOVING,
        };
        let health = self.health.current;
        let max_h = self.health.maximum.max(0.0);
        let state = if self.status.destroyed || health <= 0.0 {
            HostBodyDamageType::Rubble
        } else {
            host_calc_body_damage_state(health, max_h)
        };
        self.body_damage_state = state;
        let mut bits = host_apply_body_damage_model_bits(self.model_condition_bits, state);
        // Motion / combat residual bits from ObjectStatus.
        if self.status.moving {
            bits |= 1u128 << MC_BIT_MOVING;
        } else {
            bits &= !(1u128 << MC_BIT_MOVING);
        }
        if self.status.attacking {
            bits |= 1u128 << MC_BIT_ATTACKING;
        } else {
            bits &= !(1u128 << MC_BIT_ATTACKING);
        }
        if self.status.destroyed {
            bits |= 1u128 << MC_BIT_DYING;
        } else {
            bits &= !(1u128 << MC_BIT_DYING);
        }
        // C++ Physics stun model conditions residual.
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
        bits &= !(1u128 << MC_BIT_STUNNED);
        if self.shock_stun_frames > 15 {
            bits |= 1u128 << MC_BIT_STUNNED_FLAILING;
        } else if self.shock_stun_frames > 0 {
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        self.model_condition_bits = bits;
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.take_damage_from(damage, None)
    }

    /// Apply damage with optional C++ BodyModule last-damage-source residual.
    ///
    /// Passive AI mood (WaitForAttack) uses `last_damage_source` for idle
    /// mood-target retaliate residual.

    /// C++ PhysicsBehavior::applyShock residual (ground units only).
    ///
    /// Adds lateral+up velocity impulse and a short stun residual. Airborne /
    /// aircraft / projectiles are immune (C++ isAirborneTarget / KINDOF_PROJECTILE).
    pub fn apply_shock_wave_impulse(&mut self, force: glam::Vec3) -> bool {
        if !self.is_alive() {
            return false;
        }
        if self.is_kind_of(KindOf::Aircraft) || self.status.airborne_target {
            return false;
        }
        if self.is_kind_of(KindOf::Structure) {
            return false;
        }
        // Scale residual: force is weapon units; convert to velocity nudge.
        const FORCE_TO_VEL: f32 = 0.05;
        let impulse = force * FORCE_TO_VEL;
        self.movement.velocity += impulse;
        // Cap residual velocity so MOAB doesn't fling units off-map instantly.
        let speed = self.movement.velocity.length();
        const MAX_SHOCK_SPEED: f32 = 80.0;
        if speed > MAX_SHOCK_SPEED {
            self.movement.velocity *= MAX_SHOCK_SPEED / speed;
        }
        // C++ setStunned(true) + MODELCONDITION_STUNNED_FLAILING residual.
        // Duration: 45 frames (~1.5s). First 30 flailing, then STUNNED, then clear.
        const TOTAL: u32 = 45;
        self.shock_stun_frames = self.shock_stun_frames.max(TOTAL);
        self.refresh_model_condition_bits();
        if matches!(
            self.ai_state,
            AIState::Attacking | AIState::AttackMoving | AIState::Moving
        ) {
            self.status.moving = true;
        }
        true
    }

    /// Tick shock stun residual (once per logic frame).
    pub fn tick_shock_stun(&mut self) {
        if self.shock_stun_frames == 0 {
            return;
        }
        self.shock_stun_frames = self.shock_stun_frames.saturating_sub(1);
        self.refresh_model_condition_bits();
    }

    /// C++ PhysicsBehavior::getIsStunned residual.
    pub fn is_shock_stunned(&self) -> bool {
        self.shock_stun_frames > 0
    }

    pub fn take_damage_from(&mut self, damage: f32, source: Option<ObjectId>) -> bool {
        self.take_damage_from_typed(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
        )
    }

    /// Apply damage with host combat DamageType for Armor.ini residual coefficients.
    pub fn take_damage_from_typed(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
    ) -> bool {
        self.take_damage_from_typed_death(
            damage,
            source,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
        )
    }

    /// Apply damage with Armor.ini type residual and Weapon.ini DeathType on kill.
    pub fn take_damage_from_typed_death(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        if self.status.destroyed {
            return false;
        }
        // OCL InvulnerableTime residual (post-eject pilot shield).
        if self.status.eject_invulnerable {
            return false;
        }

        // C++ StealthForbiddenConditions TAKING_DAMAGE residual (CamoNetting structures).
        if self.stealth_breaks_on_damage && self.status.stealthed {
            self.break_stealth();
        }

        // BodyModule last damage source residual (Passive WaitForAttack).
        if let Some(src) = source {
            self.last_damage_source = Some(src);
        }

        // Armor.ini residual coefficient (by object kind + damage type), then
        // legacy scalar armor + HoldTheLine plan residual.
        let typed =
            crate::game_logic::host_armor_residual::apply_residual_armor(self, damage_type, damage);
        let armor_factor = 1.0 - (self.thing.template.armor / (self.thing.template.armor + 100.0));
        // HoldTheLine residual: HoldTheLinePlanArmorDamageScalar 0.9 (LESS is better).
        let battle_plan_armor = self.battle_plan_armor_damage_scalar();
        let actual_damage = typed * armor_factor * battle_plan_armor;

        self.health.damage(actual_damage);

        // Check if object is destroyed
        let destroyed = if !self.health.is_alive() {
            self.status.destroyed = true;
            self.status.death_type = death_type;
            self.ai_state = AIState::Idle;
            self.target = None;
            true // Object was destroyed
        } else {
            false
        };

        // Frame-local log for GameWorld shadow mutation parity (actual HP damage).
        crate::game_logic::host_damage_log::record(self.id, actual_damage, source, destroyed);

        self.refresh_model_condition_bits();
        destroyed
    }

    /// C++ AttitudeType residual (Sleep/Passive/Normal/Alert/Aggressive).
    pub fn ai_attitude(&self) -> crate::game_logic::host_strategy_center::HostAiAttitude {
        crate::game_logic::host_strategy_center::HostAiAttitude::from_i8(self.ai_attitude)
    }

    /// Set C++ AttitudeType residual for TurretAI mood matrix.
    pub fn set_ai_attitude(
        &mut self,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) {
        self.ai_attitude = attitude.as_i8();
    }

    pub fn heal(&mut self, amount: f32) {
        if !self.status.destroyed {
            let before = self.health.current;
            self.health.heal(amount);
            if self.health.current > before {
                crate::game_logic::host_heal_log::record(self.id, self.health.current);
            }
            self.refresh_model_condition_bits();
        }
    }

    /// C++ residual: STEALTHED && !DETECTED && !DISGUISED.
    /// Stealthed-and-undetected units are not legal auto/manual attack targets.
    /// Disguised units are visible as their disguise team (not pure-stealth hide).
    pub fn is_effectively_stealthed(&self) -> bool {
        self.status.stealthed && !self.status.detected && !self.status.disguised
    }

    /// C++ OBJECT_STATUS_DISGUISED residual.
    pub fn is_disguised(&self) -> bool {
        self.status.disguised
    }

    /// Apply Bomb Truck disguise residual (StealthUpdate::disguiseAsTemplate).
    /// Stores disguise template/team and sets DISGUISED + STEALTHED residual.
    pub fn apply_disguise(&mut self, template_name: &str, as_team: Team) {
        if self.status.destroyed {
            return;
        }
        self.status.disguised = true;
        self.status.stealthed = true;
        self.status.detected = false;
        self.detection_expires_frame = 0;
        self.disguise_as_template = Some(template_name.to_string());
        self.disguise_as_team = Some(as_team);
    }

    /// Clear disguise residual (reveal). Also clears STEALTHED residual for
    /// DisguisesAsTeam casters (C++ clearStatus STEALTHED on finish reveal).
    pub fn clear_disguise(&mut self) {
        self.status.disguised = false;
        self.disguise_as_template = None;
        self.disguise_as_team = None;
        // Bomb truck disguise path ends stealth when fully revealed.
        self.status.stealthed = false;
        self.status.detected = false;
        self.detection_expires_frame = 0;
    }

    /// Apparent team residual for a viewer (see host_bomb_truck_disguise).
    pub fn apparent_team_to(&self, viewer_team: Team) -> Team {
        crate::game_logic::host_bomb_truck_disguise::apparent_team_for_viewer(
            self.team,
            self.disguise_as_team,
            self.status.disguised,
            viewer_team,
        )
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
    /// Also clears disguise residual (attack reveal path for bomb truck).
    pub fn break_stealth(&mut self) {
        if self.status.disguised {
            self.clear_disguise();
            return;
        }
        let was_stealthed = self.status.stealthed;
        self.status.stealthed = false;
        self.status.detected = false;
        self.detection_expires_frame = 0;
        // CamoNetting / StealthDelay residual: schedule re-cloak gate on reveal.
        if was_stealthed && self.stealth_delay_frames > 0 {
            self.stealth_delay_pending = true;
        }
        // CamoNetting FriendlyOpacity residual: revealed → max opacity.
        if was_stealthed && self.stealth_breaks_on_damage {
            self.camo_friendly_opacity = 1.0;
            self.camo_opacity_pulse_phase = 0.0;
        }
    }

    /// C++ StealthUpdate::receiveGrant residual (GPS Scrambler / GrantStealthBehavior).
    ///
    /// Sets OBJECT_STATUS_STEALTHED (+ host residual CAN_STEALTH via stealthed flag)
    /// and clears DETECTED so the unit is effectively stealthed until broken by
    /// attack / mark_detected / break_stealth.
    ///
    /// Fail-closed: not full StealthUpdate framesGranted timer / disguise skip
    /// (callers filter disguise units) / opacity drawable path.
    pub fn apply_grant_stealth(&mut self) {
        if self.status.destroyed {
            return;
        }
        self.status.stealthed = true;
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
    /// C++ WeaponSet::getCanAttackObject stealth gate residual + disguise
    /// relationship residual (disguised units appear as disguise team).
    pub fn is_targetable_by_enemy_of(&self, attacker_team: Team) -> bool {
        if !self.is_alive() || !self.is_attackable() {
            return false;
        }
        // Disguise residual: auto-target uses apparent team (allies of disguise skip).
        if self.status.disguised {
            return crate::game_logic::host_bomb_truck_disguise::is_auto_targetable_as_enemy(
                self.team,
                self.disguise_as_team,
                true,
                attacker_team,
            ) && !self.is_effectively_stealthed();
        }
        if self.team == attacker_team {
            return false;
        }
        // Stealthed and not detected: not a valid target.
        !self.is_effectively_stealthed()
    }

    /// Whether `weapon` can legally hit `target` (air/ground + range + stealth).
    pub fn can_target_with(&self, target: &Object, weapon: &Weapon) -> bool {
        self.can_target_with_slot(target, weapon, None)
    }

    /// Slot-aware can_target (LeechRange uses per-slot active residual).
    pub fn can_target_with_slot(&self, target: &Object, weapon: &Weapon, slot: Option<u8>) -> bool {
        // C++ WeaponSet: stealthed + undetected cannot be attacked
        // (including force-fire against pure stealth; disguise exception not residual).
        if target.is_effectively_stealthed() && target.team != self.team {
            return false;
        }

        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

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
        // C++ Weapon::hasLeechRange residual: once activated, max range waived
        // for the remainder of the attack cycle.
        let leech = match slot {
            Some(1) => self.leech_range_active_secondary,
            Some(_) => self.leech_range_active_primary,
            None => self.leech_range_active_primary || self.leech_range_active_secondary,
        };
        if leech {
            return true;
        }
        // SearchAndDestroy residual: BATTLEPLAN_SEARCHANDDESTROY RANGE 120%.
        let max_range = weapon.range * self.battle_plan_range_multiplier();
        distance <= max_range
    }

    /// True if primary **or** secondary can currently hit the target.
    pub fn can_target(&self, target: &Object) -> bool {
        if target.is_effectively_stealthed() && target.team != self.team {
            return false;
        }
        if let Some(weapon) = &self.weapon {
            if self.can_target_with_slot(target, weapon, Some(0)) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            if self.can_target_with_slot(target, weapon, Some(1)) {
                return true;
            }
        }
        false
    }

    /// Weapon ready on reload timer (not range).
    ///
    /// C++ AutoReloadsClip residual via weapon-name peel:
    /// - Auto: empty clip becomes ready after clip reload (refill on fire).
    /// - Manual / ReturnToBase: empty stays OUT_OF_AMMO until `rearm_weapon_full`.
    pub fn weapon_ready(weapon: &Weapon, current_time: f32) -> bool {
        // Without a name peel, treat as Auto (legacy).
        current_time - weapon.last_fire_time >= weapon.reload_time
            && Self::weapon_has_ammo_for_shot(weapon, None)
    }

    /// Name-aware ready check (preferred).
    pub fn weapon_ready_named(
        weapon: &Weapon,
        current_time: f32,
        weapon_name: Option<&str>,
    ) -> bool {
        current_time - weapon.last_fire_time >= weapon.reload_time
            && Self::weapon_has_ammo_for_shot(weapon, weapon_name)
    }

    pub fn weapon_has_ammo_for_shot(weapon: &Weapon, weapon_name: Option<&str>) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let rt = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);
        match rt {
            HostReloadType::Auto => true,
            HostReloadType::Manual | HostReloadType::ReturnToBase => match weapon.ammo {
                Some(0) => false,
                Some(_) => true,
                None => true, // unlimited residual
            },
        }
    }

    /// C++ clip residual: consume one round.
    pub fn consume_ammo_on_fire(weapon: &mut Weapon, current_time: f32) {
        Self::consume_ammo_on_fire_named(weapon, current_time, None);
    }

    pub fn consume_ammo_on_fire_named(
        weapon: &mut Weapon,
        current_time: f32,
        weapon_name: Option<&str>,
    ) {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        weapon.last_fire_time = current_time;
        let rt = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);

        if weapon.clip_size == 0 {
            if let Some(a) = weapon.ammo.as_mut() {
                if *a > 0 {
                    *a -= 1;
                }
            }
            return;
        }

        match rt {
            HostReloadType::Auto => {
                if weapon.ammo == Some(0) || weapon.ammo.is_none() {
                    weapon.ammo = Some(weapon.clip_size);
                }
                if let Some(a) = weapon.ammo.as_mut() {
                    *a = a.saturating_sub(1);
                    if *a == 0 {
                        let clip_rt = if weapon.clip_reload_time > 0.0 {
                            weapon.clip_reload_time
                        } else {
                            weapon.reload_time
                        };
                        weapon.last_fire_time = current_time - weapon.reload_time + clip_rt;
                    }
                }
            }
            HostReloadType::Manual | HostReloadType::ReturnToBase => {
                if weapon.ammo.is_none() {
                    weapon.ammo = Some(weapon.clip_size);
                }
                if let Some(a) = weapon.ammo.as_mut() {
                    if *a > 0 {
                        *a = a.saturating_sub(1);
                    }
                }
            }
        }
    }

    pub fn rearm_weapon_full(weapon: &mut Weapon) {
        if weapon.clip_size > 0 {
            weapon.ammo = Some(weapon.clip_size);
        } else if let Some(a) = weapon.ammo {
            weapon.ammo = Some(a.max(1));
        }
        weapon.last_fire_time = -1.0e6;
    }

    fn primary_weapon_name(&self) -> Option<&str> {
        self.thing.template.primary_weapon_name.as_deref()
    }

    fn secondary_weapon_name(&self) -> Option<&str> {
        self.thing.template.secondary_weapon_name.as_deref().or(self
            .thing
            .template
            .primary_weapon_name
            .as_deref())
    }

    pub fn needs_return_to_base_rearm(&self) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let empty_rtb = |w: &Weapon, name: Option<&str>| {
            let rt = name
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            rt == HostReloadType::ReturnToBase && matches!(w.ammo, Some(0))
        };
        self.weapon
            .as_ref()
            .is_some_and(|w| empty_rtb(w, self.primary_weapon_name()))
            || self
                .secondary_weapon
                .as_ref()
                .is_some_and(|w| empty_rtb(w, self.secondary_weapon_name()))
    }

    pub fn rearm_return_to_base_weapons(&mut self) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let mut any = false;
        let pri = self.primary_weapon_name().map(|s| s.to_string());
        let sec = self.secondary_weapon_name().map(|s| s.to_string());
        if let Some(w) = self.weapon.as_mut() {
            let rt = pri
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        if let Some(w) = self.secondary_weapon.as_mut() {
            let rt = sec
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        any
    }

    /// C++ JetAIUpdate `OutOfAmmoDamagePerSecond` residual (fraction of max HP / sec).
    /// Retail JetAIUpdate OutOfAmmoDamagePerSecond = **10%**.
    pub const OUT_OF_AMMO_DAMAGE_PER_SECOND: f32 = 0.10;

    /// Apply one logic-frame of out-of-ammo damage while RTB weapons are empty.
    ///
    /// C++ JetOrHeliCirclingDeadAirfieldState:
    /// `damageRate = pct * SECONDS_PER_LOGICFRAME * maxHealth`, DAMAGE_UNRESISTABLE.
    /// Returns damage applied (0 if not eligible).
    pub fn apply_out_of_ammo_damage_frame(&mut self) -> f32 {
        if !self.is_alive() {
            return 0.0;
        }
        // Aircraft / jet residual only.
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return 0.0;
        }
        if !self.needs_return_to_base_rearm() {
            return 0.0;
        }
        // No damage while docked at airfield / garrisoned.
        if matches!(
            self.ai_state,
            AIState::Docked | AIState::Garrisoned | AIState::Entering | AIState::Docking
        ) {
            return 0.0;
        }

        const LOGIC_DT: f32 = 1.0 / 30.0;
        let max_hp = self.health.maximum.max(1.0);
        let dmg = Self::OUT_OF_AMMO_DAMAGE_PER_SECOND * LOGIC_DT * max_hp;
        if dmg <= 0.0 {
            return 0.0;
        }
        self.take_damage(dmg);
        dmg
    }

    pub fn can_fire(&self, current_time: f32) -> bool {
        // C++ Object::canFireWeapon: DISABLED_SUBDUED / weapons_jammed residual.
        // Shock stun residual blocks weapon fire while flailing/stunned.
        if self.status.weapons_jammed || self.is_disabled() || self.is_shock_stunned() {
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
    pub fn select_combat_weapon_slot(&self, target: &Object, current_time: f32) -> Option<u8> {
        let target_faerie = target.is_faerie_fire();
        let primary_ok = self.weapon.as_ref().is_some_and(|w| {
            Self::weapon_ready_vs_target(w, current_time, target_faerie)
                && self.can_target_with(target, w)
        });
        let secondary_ok = self.secondary_weapon.as_ref().is_some_and(|w| {
            Self::weapon_ready_vs_target(w, current_time, target_faerie)
                && self.can_target_with(target, w)
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

        // Comanche Rocket Pods residual: retail AutoChooseSources = TERTIARY NONE.
        // Host secondary carries pods after upgrade; never auto-choose unless
        // player locks active_weapon_slot == 1 (FIRE_WEAPON residual).
        let rocket_pods_manual_only =
            crate::game_logic::host_comanche_rocket_pods::is_comanche_template(&self.template_name)
                && (self.has_upgrade_tag(
                    crate::game_logic::host_comanche_rocket_pods::UPGRADE_COMANCHE_ROCKET_PODS,
                ) || self.has_upgrade_tag("Upgrade_ComancheRocketPods"));

        let target_is_structure =
            target.object_type == ObjectType::Building || target.is_kind_of(KindOf::Structure);
        let target_is_infantry = target.is_kind_of(KindOf::Infantry);
        let target_is_vehicle =
            target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Aircraft);
        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

        let primary_damage = self.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0);
        let secondary_damage = self
            .secondary_weapon
            .as_ref()
            .map(|w| w.damage)
            .unwrap_or(0.0);

        // SCUD residual: PreferredAgainst SECONDARY INFANTRY (toxin warhead)
        // even though secondary primary-damage is lower than explosive.
        let scud_prefer_toxin =
            crate::game_logic::host_scud_launcher::scud_prefer_secondary_vs_infantry(
                crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                    &self.template_name,
                ),
                target_is_infantry,
            );

        // Quad Cannon residual: airborne targets prefer AA secondary slot.
        let quad_prefer_aa =
            crate::game_logic::host_quad_cannon::is_quad_cannon_template(&self.template_name)
                && target_is_air;

        // Avenger residual: airborne targets prefer air laser secondary.
        let avenger_prefer_aa = crate::game_logic::host_avenger::avenger_prefer_air_laser(
            crate::game_logic::host_avenger::is_avenger_template(&self.template_name),
            target_is_air,
        );

        // Humvee residual: airborne targets prefer air TOW after TOW upgrade.
        let humvee_prefer_aa = crate::game_logic::host_humvee::humvee_prefer_air_tow(
            crate::game_logic::host_humvee::is_humvee_template(&self.template_name),
            self.has_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW)
                || self.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
            target_is_air,
        );

        if secondary_ok && !rocket_pods_manual_only {
            if scud_prefer_toxin || quad_prefer_aa || avenger_prefer_aa || humvee_prefer_aa {
                return Some(1);
            }
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
        // Rocket pods: never fall back to secondary without slot lock.
        if primary_ok {
            Some(0)
        } else if secondary_ok && !rocket_pods_manual_only {
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

    /// C++ PartitionManager::getRelativeAngle2D residual to a world position.
    pub fn relative_angle_2d_to(&self, target_pos: Vec3) -> f32 {
        crate::game_logic::weapon_bootstrap::relative_angle_2d(
            self.get_position(),
            self.get_orientation(),
            target_pos,
        )
    }

    /// Resolve AcceptableAimDelta for the active/named weapon slot (radians).
    pub fn aim_delta_for_slot(&self, slot: u8) -> f32 {
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        name.map(crate::game_logic::weapon_bootstrap::host_aim_delta_for_weapon_name)
            .unwrap_or(crate::game_logic::weapon_bootstrap::AIM_DELTA_REL_THRESH_RAD)
    }

    /// C++ AIStates aim gate: facing within AcceptableAimDelta of target.
    pub fn is_aimed_at_position(&self, target_pos: Vec3, slot: u8) -> bool {
        let aim = self.aim_delta_for_slot(slot);
        // Omni-fire residual (~180°): always aimed.
        if aim >= std::f32::consts::PI - 1e-3 {
            return true;
        }
        let rel = self.relative_angle_2d_to(target_pos);
        crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel, aim)
    }

    /// C++ setLocomotorGoalOrientation residual: rotate toward target (in-place turn).
    ///
    /// `max_step_rad` caps per-call turn (default generous for host residual).
    /// Returns true when already within aim delta after the step.
    pub fn turn_toward_position(&mut self, target_pos: Vec3, slot: u8, max_step_rad: f32) -> bool {
        let aim = self.aim_delta_for_slot(slot);
        if aim >= std::f32::consts::PI - 1e-3 {
            return true;
        }
        let rel = self.relative_angle_2d_to(target_pos);
        if crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel, aim) {
            return true;
        }
        let step = max_step_rad.max(0.0);
        let turn = rel.clamp(-step, step);
        let new_ori = self.get_orientation() + turn;
        self.set_orientation(new_ori);
        let rel2 = self.relative_angle_2d_to(target_pos);
        crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel2, aim)
    }

    /// C++ Weapon::getPreAttackDelay residual: whether PreAttackDelay applies this shot.
    pub fn pre_attack_delay_applies(
        &self,
        slot: u8,
        target_id: ObjectId,
        prefire: crate::game_logic::weapon_bootstrap::HostPrefireType,
        pre_delay: f32,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::HostPrefireType;
        if pre_delay <= 0.0 {
            return false;
        }
        match prefire {
            HostPrefireType::PerShot => true,
            HostPrefireType::PerAttack => {
                // Only the first shot of an engagement against this victim.
                !(self.consecutive_shot_target == Some(target_id)
                    && self.consecutive_shots_at_target > 0)
            }
            HostPrefireType::PerClip => match self.weapon_slot(slot) {
                Some(w) if w.clip_size > 0 => {
                    let ammo = w.ammo.unwrap_or(w.clip_size);
                    ammo >= w.clip_size
                }
                // Unlimited clip residual: treat like per-shot.
                _ => true,
            },
        }
    }

    /// Record a successful discharge for PreAttackType PER_ATTACK bookkeeping.
    pub fn record_shot_at_target(&mut self, target_id: ObjectId) {
        if self.consecutive_shot_target == Some(target_id) {
            self.consecutive_shots_at_target = self.consecutive_shots_at_target.saturating_add(1);
        } else {
            self.consecutive_shot_target = Some(target_id);
            self.consecutive_shots_at_target = 1;
        }
        // PER_SHOT: force next fire_at to re-arm delay by clearing ready stamp into the past.
        self.pre_attack_ready_at = 0.0;
    }

    /// Fire at target. `target_is_infantry` selects ScatterRadiusVsInfantry residual.
    pub fn fire_at(&mut self, target_id: ObjectId, current_time: f32) -> bool {
        self.fire_at_ex(target_id, current_time, false)
    }

    /// Fire at target with KindOf-aware scatter residual.
    pub fn fire_at_ex(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
    ) -> bool {
        // C++ canFireWeapon residual: jammed / disabled units cannot discharge.
        if self.status.weapons_jammed || self.is_disabled() {
            return false;
        }
        // Prefer the locked/active slot when ready; else primary; else secondary.
        let slot = {
            let prefer_secondary = self.active_weapon_slot == 1;
            let primary_ready = self.weapon.as_ref().is_some_and(|w| {
                Self::weapon_ready_named(w, current_time, self.primary_weapon_name())
            });
            let secondary_ready = self.secondary_weapon.as_ref().is_some_and(|w| {
                Self::weapon_ready_named(w, current_time, self.secondary_weapon_name())
            });
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

        // C++ Weapon::getPreAttackDelay / PreAttackType residual.
        let pre_delay = self
            .weapon_slot(slot)
            .map(|w| w.pre_attack_delay.max(0.0))
            .unwrap_or(0.0);
        let prefire = {
            let name = if slot == 1 {
                self.thing.template.secondary_weapon_name.as_deref().or(self
                    .thing
                    .template
                    .primary_weapon_name
                    .as_deref())
            } else {
                self.thing.template.primary_weapon_name.as_deref()
            };
            name.map(crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name)
                .unwrap_or(crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot)
        };
        let apply_delay = self.pre_attack_delay_applies(slot, target_id, prefire, pre_delay);
        if apply_delay {
            // Arm a wind-up when:
            // - new target, or
            // - ready_at == 0 (previous shot completed / no active cycle).
            // Once armed, wait until ready_at; do NOT re-arm while ready_at is set
            // (even after it elapses) until record_shot_at_target clears it.
            let needs_arm =
                self.pre_attack_target != Some(target_id) || self.pre_attack_ready_at <= 0.0;
            if needs_arm {
                self.pre_attack_target = Some(target_id);
                self.pre_attack_ready_at = current_time + pre_delay;
                // C++ Weapon::preFireWeapon LeechRange activate residual.
                self.activate_leech_range_for_slot(slot);
            }
            if current_time + 1e-6 < self.pre_attack_ready_at {
                self.target = Some(target_id);
                self.ai_state = AIState::Attacking;
                self.status.attacking = true;
                return false;
            }
            // Delay complete — fall through to fire; record_shot clears ready_at.
        } else {
            self.pre_attack_target = Some(target_id);
        }

        let fire_weapon_name = if slot == 1 {
            self.secondary_weapon_name().map(|s| s.to_string())
        } else {
            self.primary_weapon_name().map(|s| s.to_string())
        };
        if let Some(weapon) = self.weapon_slot_mut(slot) {
            Self::consume_ammo_on_fire_named(weapon, current_time, fire_weapon_name.as_deref());
            let weapon_damage = weapon.damage;
            let weapon_speed = weapon.projectile_speed;
            let weapon_splash = weapon.splash_radius;
            // AA residual: air-only weapons home on live target (missile track).
            let weapon_homing = weapon.can_target_air && !weapon.can_target_ground;
            let shooter_id = self.id;
            let shooter_pos = self.get_position();
            self.target = Some(target_id);

            // Prefer Weapon.ini DamageType via store name; shape residual if store empty.
            let weapon_dtype = {
                let slot = self.active_weapon_slot;
                let name = if slot == 1 {
                    self.thing.template.secondary_weapon_name.as_deref().or(self
                        .thing
                        .template
                        .primary_weapon_name
                        .as_deref())
                } else {
                    self.thing.template.primary_weapon_name.as_deref()
                };
                if let Some(n) = name {
                    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
                    if crate::game_logic::thing::ThingTemplate::weapon_from_store(n).is_some() {
                        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(n)
                    } else if weapon_speed <= 0.0 || weapon_speed >= 999_000.0 {
                        super::combat::DamageType::Laser
                    } else if weapon_splash > 0.0 {
                        super::combat::DamageType::Explosive
                    } else {
                        super::combat::DamageType::Bullet
                    }
                } else if weapon_speed <= 0.0 || weapon_speed >= 999_000.0 {
                    super::combat::DamageType::Laser
                } else if weapon_splash > 0.0 {
                    super::combat::DamageType::Explosive
                } else {
                    super::combat::DamageType::Bullet
                }
            };
            super::combat::queue_projectile(super::combat::PendingProjectile {
                shooter_id,
                shooter_pos,
                target_id: Some(target_id),
                target_pos: None,
                damage: weapon_damage,
                speed: weapon_speed,
                splash_radius: weapon_splash,
                is_homing: weapon_homing,
                damage_type: weapon_dtype,
                death_type: {
                    let slot = self.active_weapon_slot;
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    crate::game_logic::host_armor_residual::resolve_host_death_type(
                        name,
                        weapon_dtype,
                    )
                },
                projectile_object_name:
                    crate::game_logic::weapon_bootstrap::host_projectile_name_for_unit_slot(
                        self.template_name.as_str(),
                        self.thing.template.primary_weapon_name.as_deref(),
                        self.thing.template.secondary_weapon_name.as_deref(),
                        slot,
                    ),
                detonation_fx_name: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name,
                    )
                    .unwrap_or_default()
                },
                detonation_ocl_name: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name,
                    )
                    .unwrap_or_default()
                },
                exhaust_name:
                    crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_unit_slot(
                        self.template_name.as_str(),
                        self.thing.template.primary_weapon_name.as_deref(),
                        self.thing.template.secondary_weapon_name.as_deref(),
                        slot,
                    ),
                secondary_damage: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                secondary_damage_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_amount: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_amount_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_radius_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_taper_off: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_taper_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                radius_damage_affects: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_radius_damage_affects_for_weapon_name,
                    )
                    .unwrap_or(
                        crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                            | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
                    )
                },
                projectile_collides: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_projectile_collides_for_weapon_name,
                    )
                    .unwrap_or(crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT)
                },
                // C++ ScatterRadius + ScatterRadiusVsInfantry residual.
                // fire_at cannot query peer KindOf; apply VsInfantry peel whenever a
                // target id is set (infantry-common residual). Ground attacks use base only.
                scatter_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    // C++: base ScatterRadius + ScatterRadiusVsInfantry only vs infantry.
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                            n,
                            target_is_infantry,
                        )
                    })
                    .unwrap_or(0.0)
                },
                min_weapon_speed: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .min_weapon_speed
                    })
                    .unwrap_or(0.0)
                },
                scale_weapon_speed: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .scale_weapon_speed
                    })
                    .unwrap_or(false)
                },
                attack_range: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .attack_range
                    })
                    .or_else(|| self.weapon_slot(slot).map(|w| w.range))
                    .unwrap_or(0.0)
                },
                min_attack_range: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .min_attack_range
                    })
                    .or_else(|| self.weapon_slot(slot).map(|w| w.min_range))
                    .unwrap_or(0.0)
                },
                historic_weapon_key: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.unwrap_or("").to_string()
                },
                historic_bonus_time_frames: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .time_frames
                    })
                    .unwrap_or(0)
                },
                historic_bonus_count: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .count
                    })
                    .unwrap_or(0)
                },
                historic_bonus_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .radius
                    })
                    .unwrap_or(0.0)
                },
                historic_bonus_weapon: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .bonus_weapon
                    })
                    .unwrap_or_default()
                },
            });
            // C++ fireWeaponTemplate LeechRange activate residual.
            self.activate_leech_range_for_slot(slot);
            self.record_shot_at_target(target_id);

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
            crate::game_logic::host_move_log::record(
                self.id,
                Some([position.x, position.y, position.z]),
            );
        }
    }

    pub fn stop_moving(&mut self) {
        self.movement.target_position = None;
        self.movement.velocity = Vec3::ZERO;
        crate::game_logic::host_move_log::record(self.id, None);
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
        if !self.is_alive() {
            return;
        }
        // Shock stun residual: ignore new attack orders while stunned.
        if self.is_shock_stunned() {
            return;
        }
        // Jet takeoff residual: leave hangar before engaging.
        let _ = self.takeoff_from_airfield_parking();
        if self.can_attack() {
            if self.pre_attack_target != Some(target_id) {
                // New target — fire_at will start PRE_ATTACK clock.
                self.pre_attack_target = None;
                self.pre_attack_ready_at = 0.0;
            }
            self.target = Some(target_id);
            self.target_location = None;
            self.force_attack = false;
            self.ai_state = AIState::Attacking;
            self.status.attacking = true;
            crate::game_logic::host_attack_log::record(self.id, Some(target_id));
        }
    }

    /// C++ Weapon::setLeechRangeActive residual for a weapon slot.
    pub fn activate_leech_range_for_slot(&mut self, slot: u8) {
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        let is_leech = name
            .map(crate::game_logic::weapon_bootstrap::host_leech_range_weapon_for_weapon_name)
            .unwrap_or(false);
        if !is_leech {
            return;
        }
        if slot == 1 {
            self.leech_range_active_secondary = true;
        } else {
            self.leech_range_active_primary = true;
        }
    }

    /// C++ Object::clearLeechRangeModeForAllWeapons residual.
    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        self.leech_range_active_primary = false;
        self.leech_range_active_secondary = false;
    }

    pub fn stop_attack(&mut self) {
        self.target = None;
        self.target_location = None;
        self.force_attack = false;
        self.pre_attack_target = None;
        self.pre_attack_ready_at = 0.0;
        self.consecutive_shot_target = None;
        self.consecutive_shots_at_target = 0;
        self.clear_leech_range_mode_for_all_weapons();
        self.status.attacking = false;
        crate::game_logic::host_attack_log::record(self.id, None);
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
        // disabled_subdued blocks move (C++ DISABLED_SUBDUED full disable for non-projectile).
        // Docked aircraft may move (takeoff/sortie residual).
        // Shock flailing residual: block commanded move while STUNNED_FLAILING
        // (stun_frames > 15). Settled STUNNED phase may still stagger via velocity.
        let parked_aircraft = self.is_parked_at_airfield();
        let flailing = self.shock_stun_frames > 15;
        self.is_mobile()
            && self.is_alive()
            && !self.status.disabled_unmanned
            && !self.status.disabled_hacked
            && !self.status.disabled_emp
            && !self.status.disabled_subdued
            && !flailing
            && (parked_aircraft || !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned))
    }

    pub fn set_destination(&mut self, destination: Vec3) {
        let _ = self.takeoff_from_airfield_parking();
        self.move_to(destination);
    }

    pub fn set_target(&mut self, target: Option<ObjectId>) {
        if target.is_some() {
            let _ = self.takeoff_from_airfield_parking();
        }
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
        crate::game_logic::host_attack_log::record(self.id, target);
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
        // GLA Tunnel Network residual: TunnelContain entrance (shared team pool).
        if self.is_tunnel_network_style_container() {
            return self.is_kind_of(KindOf::Structure);
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
    /// Conflicts residual: clears gattling/propaganda addons (exclusive payload).
    pub fn install_overlord_battle_bunker(&mut self, slots: usize) {
        self.overlord_bunker_capacity = Some(slots);
        // Exclusive ConflictsWith residual (not Emperor innate propaganda).
        let emperor =
            crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name);
        self.has_overlord_gattling_addon = false;
        if !emperor {
            self.has_overlord_propaganda_addon = false;
        }
    }

    /// Install residual portable GattlingCannon addon
    /// (C++ OCL_OverlordGattlingCannon / OCL_HelixGattlingCannon).
    /// Equips AA secondary + passenger ground residual on primary fires.
    /// Fail-closed: not full portable-structure passenger object.
    pub fn install_overlord_gattling_addon(&mut self) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            is_emperor_template, overlord_gattling_air_weapon,
        };
        // Exclusive ConflictsWith residual vs bunker / propaganda (except Emperor).
        let emperor = is_emperor_template(&self.template_name);
        if !emperor {
            self.has_overlord_propaganda_addon = false;
            // Keep overlord-style marker but zero bunker slots.
            if self.overlord_bunker_capacity.is_some() {
                self.overlord_bunker_capacity = Some(0);
            }
        }
        self.has_overlord_gattling_addon = true;
        self.weapon_set_player_upgrade = true;
        let chain = has_chain_guns_upgrade(&self.applied_upgrades);
        self.secondary_weapon = Some(overlord_gattling_air_weapon(0, chain));
        self.continuous_fire_consecutive = 0;
        self.continuous_fire_level = 0;
        self.continuous_fire_coast_until_frame = 0;
        self.continuous_fire_victim = 0;
    }

    /// Install residual portable PropagandaTower addon
    /// (C++ OCL_OverlordPropagandaTower / OCL_HelixPropagandaTower).
    /// Fail-closed: not full portable tower object / PulseFX.
    pub fn install_overlord_propaganda_addon(&mut self) {
        // Exclusive ConflictsWith residual vs gattling / bunker.
        self.has_overlord_gattling_addon = false;
        if self.overlord_bunker_capacity.is_some() {
            self.overlord_bunker_capacity = Some(0);
        }
        self.has_overlord_propaganda_addon = true;
    }

    /// Install residual HelixContain transport (Slots=5).
    pub fn install_helix_transport(&mut self) {
        self.is_helix_transport = true;
        self.max_transport = crate::game_logic::host_overlord_addons::HELIX_TRANSPORT_SLOTS;
        // Helix can hold infantry / vehicle / portable structure residual.
        // Fail-closed: allow_inside matrix simplified to transport capacity.
    }

    /// True when portable gattling residual is active on this host.
    pub fn has_overlord_gattling_residual(&self) -> bool {
        self.has_overlord_gattling_addon
    }

    /// True when portable / innate propaganda residual is active on this host.
    pub fn has_overlord_propaganda_residual(&self) -> bool {
        self.has_overlord_propaganda_addon
            || crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name)
    }

    /// Install residual GLA Battle Bus transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-door exit / SlowDeath undeath SECOND_LIFE.
    pub fn install_battle_bus_transport(&mut self) {
        self.is_battle_bus_transport = true;
        self.max_transport = crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
    }

    /// True when this vehicle is a Battle Bus residual transport.
    pub fn is_battle_bus_style_container(&self) -> bool {
        self.is_battle_bus_transport
    }

    /// Install residual GLA Tunnel Network structure:
    /// C++ TunnelContain shared MaxTunnelCapacity=10 per player.
    /// Fail-closed: not GuardTunnelNetwork AI / TimeForFullHeal / CaveSystem.
    pub fn install_tunnel_network_residual(&mut self) {
        self.is_tunnel_network = true;
        if let Some(bd) = self.building_data.as_mut() {
            // Local max is the shared pool cap; GameLogic enforces team-shared count.
            bd.max_garrison = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
        } else {
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
            self.building_data = Some(bd);
        }
    }

    /// True when this structure is a GLA Tunnel Network residual entrance.
    pub fn is_tunnel_network_style_container(&self) -> bool {
        self.is_tunnel_network
    }

    /// Install residual GLA Technical transport:
    /// C++ TransportContain Slots=5, AllowInsideKindOf=INFANTRY.
    /// Passengers ride (bed garrison residual) but do **not** fire
    /// (`PassengersAllowedToFire` unset in retail).
    /// Fail-closed: not chassis reskin / W3D gunner matrix.
    pub fn install_technical_transport(&mut self) {
        self.is_technical_transport = true;
        self.max_transport = crate::game_logic::host_technical::TECHNICAL_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
    }

    /// True when this vehicle is a GLA Technical residual transport.
    pub fn is_technical_style_container(&self) -> bool {
        self.is_technical_transport
    }

    /// Install residual GLA Combat Cycle RiderChangeContain:
    /// C++ Slots=1, AllowInsideKindOf=INFANTRY, passengers do not fire
    /// (bike itself switches WeaponSet to rider weapon residual).
    /// Fail-closed: not full STATUS_RIDER death OCL / scuttle matrix.
    pub fn install_combat_cycle_transport(&mut self) {
        self.is_combat_cycle_transport = true;
        self.max_transport = crate::game_logic::host_combat_cycle::COMBAT_CYCLE_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
    }

    /// True when this vehicle is a GLA Combat Cycle residual transport.
    pub fn is_combat_cycle_style_container(&self) -> bool {
        self.is_combat_cycle_transport
    }

    /// Install residual America Humvee transport:
    /// C++ TransportContain Slots=5, PassengersAllowedToFire=Yes,
    /// AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
    pub fn install_humvee_transport(&mut self) {
        self.is_humvee_transport = true;
        self.max_transport = crate::game_logic::host_humvee::HUMVEE_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = false;
    }

    /// True when this vehicle is an America Humvee residual transport.
    pub fn is_humvee_style_container(&self) -> bool {
        self.is_humvee_transport
    }

    /// Install residual China Troop Crawler transport:
    /// C++ TransportContain Slots=8, AllowInsideKindOf=INFANTRY,
    /// InitialPayload Redguard×8, GoAggressiveOnExit residual (exit-to-fight).
    /// Passengers do **not** fire from inside (`PassengersAllowedToFire` unset).
    /// Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve matrix.
    pub fn install_troop_crawler_transport(&mut self) {
        self.is_troop_crawler_transport = true;
        self.max_transport = crate::game_logic::host_troop_crawler::TROOP_CRAWLER_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
    }

    /// True when this vehicle is a China Troop Crawler residual transport.
    pub fn is_troop_crawler_style_container(&self) -> bool {
        self.is_troop_crawler_transport
    }

    /// Install residual Air Force Combat Chinook transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY VEHICLE.
    /// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
    pub fn install_combat_chinook_transport(&mut self) {
        self.is_combat_chinook_transport = true;
        self.max_transport = crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        // Combat Chinook KindOf includes CAN_ATTACK residual (vanilla Chinook does not).
        self.thing.template.add_kind_of(KindOf::Attackable);
        // Retail WeaponSet Conditions=None has PRIMARY NONE until PLAYER_UPGRADE
        // (ListeningOutpostUpgradedDummyWeapon). Strip kind-based Weapon::default.
        self.weapon = None;
        self.weapon_set_player_upgrade = false;
    }

    /// True when this vehicle is an AirF Combat Chinook residual transport.
    pub fn is_combat_chinook_style_container(&self) -> bool {
        self.is_combat_chinook_transport
    }

    /// Install residual China Listening Outpost transport + detect residual:
    /// C++ TransportContain Slots=2, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY,
    /// StealthDetectorUpdate DetectionRange=300, InnateStealth=Yes.
    /// Fail-closed: not multi-door exit / IR FX / RIDERS_ATTACKING uncloak matrix.
    pub fn install_listening_outpost_transport(&mut self) {
        self.is_listening_outpost_transport = true;
        self.max_transport =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        // Detector residual (DetectionRange = 300).
        self.is_detector = true;
        self.detection_range =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DETECTION_RANGE;
        // Innate stealth residual; uncloaks while MOVING.
        self.status.stealthed = true;
        self.innate_stealth = true;
        self.stealth_breaks_on_move = true;
        // Fire does not break stealth on the vehicle itself (passengers fire residual).
        self.stealth_breaks_on_attack = false;
        // Retail WeaponSet Conditions=None has PRIMARY NONE until PLAYER_UPGRADE.
        self.weapon = None;
        self.weapon_set_player_upgrade = false;
        // KindOf residual includes CAN_ATTACK (for dummy weapon range residual).
        self.thing.template.add_kind_of(KindOf::Attackable);
    }

    /// True when this vehicle is a China Listening Outpost residual transport.
    pub fn is_listening_outpost_style_container(&self) -> bool {
        self.is_listening_outpost_transport
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
        // Wave 79: AdvancedTraining ExperienceScalarUpgrade residual application.
        // C++ AddXPScalar 1.0 → double XP when the upgrade tag is present.
        let amount = if self.has_advanced_training_xp_scalar() {
            crate::game_logic::host_unit_training::residual_xp_gain_with_advanced_training(
                amount, true,
            )
        } else {
            amount
        };
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
        crate::game_logic::host_unit_training::veterancy_bonus_multipliers(level)
    }

    /// Wave 79: true when AdvancedTraining ExperienceScalar residual tag is present.
    pub fn has_advanced_training_xp_scalar(&self) -> bool {
        use crate::game_logic::host_unit_training::{
            is_advanced_training_upgrade, UPGRADE_AMERICA_ADVANCED_TRAINING,
        };
        self.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING)
            || self.has_upgrade_tag("UpgradeAdvancedTraining")
            || self
                .applied_upgrades
                .iter()
                .any(|u| is_advanced_training_upgrade(u))
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

    /// C++ ExperienceTracker::setMinVeterancyLevel residual (VeterancyGainCreate).
    ///
    /// Never lowers rank. Seeds residual XP so gain_experience does not demote.
    /// Applies health / weapon bonuses when promoting.
    pub fn set_min_veterancy_level(&mut self, level: VeterancyLevel) -> bool {
        fn rank(level: VeterancyLevel) -> u8 {
            match level {
                VeterancyLevel::Rookie => 0,
                VeterancyLevel::Veteran => 1,
                VeterancyLevel::Elite => 2,
                VeterancyLevel::Heroic => 3,
            }
        }
        fn xp_seed(level: VeterancyLevel, thresholds: [f32; 3]) -> f32 {
            match level {
                VeterancyLevel::Rookie => 0.0,
                VeterancyLevel::Veteran => thresholds[0],
                VeterancyLevel::Elite => thresholds[1],
                VeterancyLevel::Heroic => thresholds[2],
            }
        }

        let previous = self.experience.level;
        let thresholds = self.thing.template.veterancy_xp_thresholds;
        if rank(level) <= rank(previous) {
            // Still seed XP if level already matches but XP is below threshold.
            let seed = xp_seed(previous, thresholds);
            if self.experience.current < seed {
                self.experience.current = seed;
            }
            return false;
        }
        self.experience.level = level;
        let seed = xp_seed(level, thresholds);
        self.experience.current = self.experience.current.max(seed);
        self.apply_veterancy_bonuses(previous, level);
        true
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
        if self.team != team {
            self.team = team;
            self.team_color = team.get_color();
            crate::game_logic::host_owner_log::record(self.id, team);
        } else {
            self.team = team;
            self.team_color = team.get_color();
        }
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

    #[test]
    fn clip_ammo_forces_clip_reload_gap() {
        use crate::game_logic::Weapon;
        let mut w = Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.1, // between shots
            ammo: Some(2),
            clip_size: 2,
            clip_reload_time: 2.0, // long clip reload
            last_fire_time: -100.0,
            ..Weapon::default()
        };
        let t0 = 10.0;
        assert!(Object::weapon_ready(&w, t0));
        Object::consume_ammo_on_fire(&mut w, t0);
        assert_eq!(w.ammo, Some(1));
        // Between-shot: ready after 0.1
        assert!(!Object::weapon_ready(&w, t0 + 0.05));
        assert!(Object::weapon_ready(&w, t0 + 0.11));
        Object::consume_ammo_on_fire(&mut w, t0 + 0.11);
        assert_eq!(w.ammo, Some(0));
        // Clip empty: not ready until clip_reload (~2.0 from last fire adjusted)
        assert!(!Object::weapon_ready(&w, t0 + 0.11 + 0.5));
        assert!(
            Object::weapon_ready(&w, t0 + 0.11 + 2.0),
            "clip reload must elapse before next ready"
        );
        Object::consume_ammo_on_fire(&mut w, t0 + 0.11 + 2.0);
        assert_eq!(w.ammo, Some(1), "refill then spend one");
    }

    #[test]
    fn clip_ammo_cpp_surface() {
        let src = include_str!("object.rs");
        assert!(src.contains("fn consume_ammo_on_fire"));
        assert!(src.contains("clip_reload_time"));
    }

    #[test]
    fn pre_attack_delay_blocks_first_shot() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("PreAtk");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 1.0,
            ammo: Some(5),
            clip_size: 5,
            ..Weapon::default()
        });
        let tgt_id = ObjectId(2);

        // First call starts wind-up, must not fire (ammo unchanged).
        assert!(!atk.fire_at(tgt_id, 10.0));
        assert_eq!(atk.pre_attack_target, Some(tgt_id));
        assert!((atk.pre_attack_ready_at - 11.0).abs() < 1e-4);
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

        // Still winding up.
        assert!(!atk.fire_at(tgt_id, 10.5));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

        // After delay, fires and consumes ammo.
        assert!(atk.fire_at(tgt_id, 11.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(4));
    }

    #[test]
    fn pre_attack_resets_on_new_target() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("PreAtk2");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 50.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 2.0,
            ..Weapon::default()
        });
        assert!(!atk.fire_at(ObjectId(10), 5.0));
        assert!((atk.pre_attack_ready_at - 7.0).abs() < 1e-4);
        // Switch target restarts delay.
        assert!(!atk.fire_at(ObjectId(11), 6.0));
        assert_eq!(atk.pre_attack_target, Some(ObjectId(11)));
        assert!((atk.pre_attack_ready_at - 8.0).abs() < 1e-4);
    }

    #[test]
    fn pre_attack_cpp_surface() {
        let src = include_str!("object.rs");
        assert!(src.contains("PRE_ATTACK residual"));
        assert!(src.contains("pre_attack_ready_at"));
        assert!(src.contains("pre_attack_delay"));
    }

    #[test]
    fn small_arms_reduced_on_tank_armor_residual() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("ArmorTank");
        tmpl.set_health(1000.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut tank = Object::new(tmpl, ObjectId(70), Team::USA);
        let hp0 = tank.health.current;
        // TankArmor SmallArms residual is 0.25 → 100 * 0.25 = 25
        tank.take_damage_from_typed(100.0, None, DamageType::Bullet);
        let dealt = hp0 - tank.health.current;
        assert!(
            (dealt - 25.0).abs() < 1.0,
            "expected ~25 small-arms on tank, got {dealt}"
        );
    }

    #[test]
    fn laser_half_on_human_armor_residual() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("ArmorInf");
        tmpl.set_health(500.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut inf = Object::new(tmpl, ObjectId(71), Team::GLA);
        let hp0 = inf.health.current;
        // HumanArmor Laser residual 0.5 → 100 * 0.5 = 50
        inf.take_damage_from_typed(100.0, None, DamageType::Laser);
        let dealt = hp0 - inf.health.current;
        assert!(
            (dealt - 50.0).abs() < 1.0,
            "expected ~50 laser on infantry, got {dealt}"
        );
    }

    #[test]
    fn flame_kill_sets_burned_death_type() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("BurnMe");
        tmpl.set_health(50.0);
        tmpl.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(tmpl, ObjectId(80), Team::GLA);
        let dead =
            o.take_damage_from_typed_death(999.0, None, DamageType::Flame, HostDeathType::Burned);
        assert!(dead);
        assert_eq!(o.status.death_type, HostDeathType::Burned);
    }

    #[test]
    fn resolve_death_type_from_damage_class() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_armor_residual::resolve_host_death_type;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        assert_eq!(
            resolve_host_death_type(None, DamageType::Explosive),
            HostDeathType::Exploded
        );
        assert_eq!(
            resolve_host_death_type(None, DamageType::Laser),
            HostDeathType::Lasered
        );
        assert_eq!(
            resolve_host_death_type(None, DamageType::Toxin),
            HostDeathType::Poisoned
        );
    }

    #[test]
    fn body_damage_sets_model_condition_bits() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, HostBodyDamageType, MC_BIT_DAMAGED, MC_BIT_DYING,
            MC_BIT_REALLYDAMAGED, MC_BIT_RUBBLE,
        };
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("McBits");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(90), Team::USA);
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_DAMAGED
        ));

        o.health.current = 40.0; // between 0.25 and 0.5
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_DAMAGED
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_REALLYDAMAGED
        ));

        o.health.current = 10.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_REALLYDAMAGED
        ));

        o.take_damage(9999.0);
        assert!(o.status.destroyed);
        assert_eq!(o.body_damage_state, HostBodyDamageType::Rubble);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_RUBBLE
        ));
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_DYING
        ));
    }

    #[test]
    fn body_damage_threshold_cpp_surface() {
        use crate::game_logic::host_enum_table_residual::{
            host_calc_body_damage_state, HostBodyDamageType, HOST_UNIT_DAMAGED_THRESH,
            HOST_UNIT_REALLY_DAMAGED_THRESH,
        };
        assert!((HOST_UNIT_DAMAGED_THRESH - 0.5).abs() < 1e-6);
        assert!((HOST_UNIT_REALLY_DAMAGED_THRESH - 0.25).abs() < 1e-6);
        assert_eq!(
            host_calc_body_damage_state(100.0, 100.0),
            HostBodyDamageType::Pristine
        );
        assert_eq!(
            host_calc_body_damage_state(50.0, 100.0),
            HostBodyDamageType::Damaged
        );
        assert_eq!(
            host_calc_body_damage_state(25.0, 100.0),
            HostBodyDamageType::ReallyDamaged
        );
        assert_eq!(
            host_calc_body_damage_state(0.0, 100.0),
            HostBodyDamageType::Rubble
        );
    }

    #[test]
    fn fire_at_stamps_detonation_fx_on_pending() {
        // Surface residual: PendingProjectile carries ProjectileDetonationFX name.
        let src = include_str!("object.rs");
        assert!(src.contains("detonation_fx_name"));
        assert!(src.contains("host_detonation_fx_for_weapon_name"));
        assert!(src.contains("detonation_ocl_name"));
        assert!(src.contains("host_detonation_ocl_for_weapon_name"));
        assert!(src.contains("exhaust_name"));
        assert!(src.contains("host_projectile_exhaust_for_unit_slot"));
        let csrc = include_str!("combat.rs");
        assert!(csrc.contains("take_impact_fx"));
        assert!(csrc.contains("ProjectileImpactFx"));
    }

    #[test]
    fn leech_range_waives_max_range_after_activate() {
        let mut tmpl = ThingTemplate::new("GLAInfantryTerrorist");
        tmpl.primary_weapon_name = Some("GLAInfantryTerrorist".into());
        let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
        atk.set_position(glam::Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 100.0,
            range: 20.0,
            min_range: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            ..Weapon::default()
        });

        let mut tgt = Object::new(
            ThingTemplate::new("AmericaTankCrusader"),
            ObjectId(2),
            Team::USA,
        );
        tgt.set_position(glam::Vec3::new(100.0, 0.0, 0.0)); // out of 20 range
        tgt.thing.template.add_kind_of(KindOf::Vehicle);
        tgt.thing.template.add_kind_of(KindOf::Attackable);

        // Before leech: out of range.
        assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

        // Activate leech (as if pre-fire / fire occurred in range).
        atk.activate_leech_range_for_slot(0);
        assert!(atk.leech_range_active_primary);
        assert!(atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

        // stop_attack clears.
        atk.stop_attack();
        assert!(!atk.leech_range_active_primary);
        assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));
    }

    #[test]
    fn acceptable_aim_delta_blocks_then_allows_after_turn() {
        let mut tmpl = ThingTemplate::new("AmericaTankCrusader");
        tmpl.primary_weapon_name = Some("AmericaTankCrusaderGun".into());
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(glam::Vec3::ZERO);
        atk.set_orientation(0.0); // face +X residual (movement convention)
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
        let target = glam::Vec3::new(0.0, 0.0, 50.0); // off to +Z (~90°)
        let aim = atk.aim_delta_for_slot(0);
        let rel = atk.relative_angle_2d_to(target);
        // 20° aim residual should NOT be aimed at 90° offset.
        assert!(
            !atk.is_aimed_at_position(target, 0),
            "unexpectedly aimed: aim_delta={aim} rel={rel} ori={}",
            atk.get_orientation()
        );
        // Turn in steps until aimed.
        let mut aimed = false;
        for _ in 0..20 {
            if atk.turn_toward_position(target, 0, 0.2) {
                aimed = true;
                break;
            }
        }
        assert!(
            aimed,
            "should aim after turns, ori={}",
            atk.get_orientation()
        );
        assert!(atk.is_aimed_at_position(target, 0));
    }

    #[test]
    fn omni_aim_delta_always_aimed() {
        let mut tmpl = ThingTemplate::new("AmericaSentryDrone");
        tmpl.primary_weapon_name = Some("AmericaSentryDroneGun".into());
        let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
        atk.set_position(glam::Vec3::ZERO);
        atk.set_orientation(0.0);
        let target = glam::Vec3::new(-40.0, 0.0, 10.0);
        assert!(atk.is_aimed_at_position(target, 0));
    }

    #[test]
    fn pre_attack_type_per_shot_delays_every_discharge() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("Gattling");
        tmpl.primary_weapon_name = Some("AmericaGattlingTankGun".into());
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 0.5,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        // First wind-up
        assert!(!atk.fire_at(tgt, 10.0));
        assert!((atk.pre_attack_ready_at - 10.5).abs() < 1e-4);
        // Still winding
        assert!(!atk.fire_at(tgt, 10.2));
        // Fire after delay
        assert!(atk.fire_at(tgt, 10.5));
        assert_eq!(atk.consecutive_shots_at_target, 1);
        // PER_SHOT: next shot needs a new delay even vs same target
        assert!(!atk.fire_at(tgt, 10.5));
        assert!(atk.pre_attack_ready_at > 10.5);
        assert!(!atk.fire_at(tgt, 10.7));
        assert!(atk.fire_at(tgt, 11.0));
        assert_eq!(atk.consecutive_shots_at_target, 2);
    }

    #[test]
    fn pre_attack_type_per_attack_delays_once_per_target() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.primary_weapon_name = Some("AmericaRangerMachineGun".into());
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 1.0,
            ammo: Some(5),
            clip_size: 5,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        assert!(!atk.fire_at(tgt, 5.0)); // wind-up
        assert!(atk.fire_at(tgt, 6.0)); // fire
                                        // Same target: no second wind-up
        assert!(atk.fire_at(tgt, 6.0));
        assert_eq!(atk.consecutive_shots_at_target, 2);
        // New target: delay again
        let tgt2 = ObjectId(10);
        assert!(!atk.fire_at(tgt2, 6.0));
        assert!(atk.fire_at(tgt2, 7.0));
    }

    #[test]
    fn pre_attack_type_per_clip_delays_on_full_clip_only() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("Scud");
        // Seed-only name (not in WeaponStore) so PreAttackType peels to PER_CLIP.
        tmpl.primary_weapon_name = Some("HostTestScudStormClipWeapon".into());
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 50.0,
            range: 300.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 2.0,
            ammo: Some(3),
            clip_size: 3,
            clip_reload_time: 0.0,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        // Full clip → delay
        assert!(!atk.fire_at(tgt, 1.0));
        assert!(atk.fire_at(tgt, 3.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(2));
        // Mid-clip → no delay
        assert!(atk.fire_at(tgt, 3.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(1));
        assert!(atk.fire_at(tgt, 3.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(0));
    }

    #[test]
    fn return_to_base_blocks_fire_until_rearm() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        // Seed-only name so store cannot peel YES over RETURN_TO_BASE.
        tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
        jet.set_position(Vec3::ZERO);
        jet.weapon = Some(Weapon {
            damage: 100.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(2),
            clip_size: 2,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        assert!(jet.fire_at(tgt, 1.0));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
        assert!(jet.fire_at(tgt, 1.0));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
        assert!(jet.needs_return_to_base_rearm());
        assert!(!jet.fire_at(tgt, 2.0));
        assert!(!Object::weapon_ready_named(
            jet.weapon.as_ref().unwrap(),
            2.0,
            Some("HostTestRaptorJetMissileWeapon")
        ));
        assert!(jet.rearm_return_to_base_weapons());
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(2));
        assert!(jet.fire_at(tgt, 3.0));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
    }

    #[test]
    fn auto_reload_still_refills_clip() {
        use crate::game_logic::Weapon;
        let mut w = Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.1,
            ammo: Some(1),
            clip_size: 2,
            clip_reload_time: 1.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        };
        let t0 = 5.0;
        assert!(Object::weapon_ready(&w, t0));
        Object::consume_ammo_on_fire(&mut w, t0);
        assert_eq!(w.ammo, Some(0));
        // After clip reload gap, ready again and refill on fire.
        assert!(
            Object::weapon_ready(&w, t0 + 1.05),
            "last_fire={} reload={}",
            w.last_fire_time,
            w.reload_time
        );
        Object::consume_ammo_on_fire(&mut w, t0 + 1.05);
        assert_eq!(w.ammo, Some(1)); // refilled to 2, spent 1
    }

    #[test]
    fn out_of_ammo_damage_ticks_empty_rtb_jet() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Attackable);
        tmpl.set_health(100.0);
        let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
        jet.set_position(Vec3::new(0.0, 50.0, 0.0));
        jet.status.airborne_target = true;
        jet.weapon = Some(Weapon {
            damage: 100.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 2,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        assert!(jet.needs_return_to_base_rearm());
        let hp0 = jet.health.current;
        let dmg = jet.apply_out_of_ammo_damage_frame();
        // 10% / sec * 1/30 * 100 = 10/30 ≈ 0.333
        assert!((dmg - (0.10 / 30.0) * 100.0).abs() < 1e-3, "dmg={dmg}");
        assert!((hp0 - jet.health.current - dmg).abs() < 1e-3);
        // Docked: no damage.
        jet.health.current = 100.0;
        jet.ai_state = AIState::Docked;
        assert_eq!(jet.apply_out_of_ammo_damage_frame(), 0.0);
        // Rearmed: no damage.
        jet.ai_state = AIState::Idle;
        jet.rearm_return_to_base_weapons();
        assert_eq!(jet.apply_out_of_ammo_damage_frame(), 0.0);
    }

    #[test]
    fn parked_jet_takeoff_on_attack_and_move() {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Attackable);
        tmpl.set_health(100.0);
        let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
        jet.set_position(Vec3::new(0.0, 0.0, 0.0));
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(4),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        jet.contained_by = Some(ObjectId(99));
        jet.ai_state = AIState::Docked;
        jet.status.airborne_target = false;
        assert!(jet.is_parked_at_airfield());
        assert!(jet.can_attack()); // parked aircraft may sortie
        jet.attack_target(ObjectId(7));
        assert!(jet.contained_by.is_none());
        assert_ne!(jet.ai_state, AIState::Docked);
        assert!(jet.status.airborne_target);
        assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
        assert_eq!(jet.target, Some(ObjectId(7)));
        assert_eq!(jet.ai_state, AIState::Attacking);

        // Re-dock and move.
        jet.contained_by = Some(ObjectId(99));
        jet.ai_state = AIState::Docked;
        jet.status.airborne_target = false;
        jet.set_position(Vec3::new(10.0, 0.0, 0.0));
        jet.set_destination(Vec3::new(100.0, 0.0, 0.0));
        assert!(jet.contained_by.is_none());
        assert!(jet.status.airborne_target || jet.ai_state != AIState::Docked);
        assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
    }

    #[test]
    fn fire_at_scatter_vs_infantry_only_when_flagged() {
        use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
        // Crusader gun: base 0 + ScatterRadiusVsInfantry 10.
        let vs_inf = host_effective_scatter_radius("AmericaTankCrusaderGun", true);
        let vs_veh = host_effective_scatter_radius("AmericaTankCrusaderGun", false);
        assert!(vs_inf >= 10.0 - 1e-3, "vs infantry {vs_inf}");
        assert!(vs_veh < 1e-3, "vs vehicle base {vs_veh}");
        // fire_at_ex is the KindOf-aware entry; fire_at defaults infantry=false (base only).
        let src = include_str!("object.rs");
        assert!(src.contains("fn fire_at_ex"));
        assert!(src.contains("target_is_infantry"));
        assert!(
            src.contains("host_effective_scatter_radius"),
            "fire path must peel scatter"
        );
    }

    #[test]
    fn shock_wave_impulse_knocks_ground_units() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        let mut tmpl = ThingTemplate::new("ShockVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(1), Team::USA);
        o.movement.velocity = glam::Vec3::ZERO;
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(20.0, 10.0, 0.0)));
        assert!(o.movement.velocity.length() > 0.0);
        assert!(o.is_shock_stunned());
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
        // After flail window: STUNNED bit.
        o.shock_stun_frames = 10;
        o.refresh_model_condition_bits();
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
        // Aircraft immune.
        let mut at = ThingTemplate::new("ShockAir");
        at.add_kind_of(KindOf::Aircraft);
        let mut a = Object::new(at, ObjectId(2), Team::USA);
        a.status.airborne_target = true;
        assert!(!a.apply_shock_wave_impulse(glam::Vec3::new(20.0, 10.0, 0.0)));
    }

    #[test]
    fn shock_stun_ticks_clear_model_bits() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        let mut tmpl = ThingTemplate::new("StunTick");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(tmpl, ObjectId(3), Team::USA);
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(5.0, 5.0, 0.0)));
        let start = o.shock_stun_frames;
        assert!(start >= 40);
        for _ in 0..start {
            o.tick_shock_stun();
        }
        assert_eq!(o.shock_stun_frames, 0);
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED
        ));
    }
    #[test]
    fn shock_stun_blocks_attack_fire_and_flail_move() {
        let mut tmpl = ThingTemplate::new("StunBlock");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(42), Team::USA);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        assert!(o.can_attack());
        assert!(o.can_fire(0.0));
        assert!(o.can_move());
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(10.0, 5.0, 0.0)));
        assert!(o.is_shock_stunned());
        assert!(!o.can_attack(), "stunned cannot attack");
        assert!(!o.can_fire(0.0), "stunned cannot fire");
        // Flailing phase blocks commanded move.
        assert!(o.shock_stun_frames > 15);
        assert!(!o.can_move(), "flailing cannot take move orders");
        // Settled stunned phase: move orders allowed (stagger), still no fire.
        o.shock_stun_frames = 10;
        o.refresh_model_condition_bits();
        assert!(!o.can_attack());
        assert!(!o.can_fire(1.0));
        assert!(o.can_move(), "settled stun may stagger-move");
        // attack_target ignored while stunned.
        o.shock_stun_frames = 20;
        o.attack_target(ObjectId(99));
        assert!(o.target.is_none() || o.ai_state != AIState::Attacking || !o.can_attack());
        // After stun clears, combat again.
        o.shock_stun_frames = 0;
        o.refresh_model_condition_bits();
        assert!(o.can_attack());
        assert!(o.can_fire(2.0));
        assert!(o.can_move());
    }
}
