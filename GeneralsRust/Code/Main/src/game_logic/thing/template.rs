use super::*;

/// Thing Template - shared configuration data for Things
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThingTemplate {
    pub name: String,
    pub display_name: String,
    pub kind_of: HashSet<KindOf>,
    pub max_health: f32,
    pub armor: f32,
    /// C++ `ThingTemplate::m_visionRange` from Object INI `VisionRange`.
    /// Default 0 = reveal nothing (ThingTemplate.cpp:976).
    #[serde(default)]
    pub sight_range: f32,
    /// C++ `ThingTemplate::m_shroudClearingRange`. `-1` means use `sight_range`.
    #[serde(default = "default_template_shroud_clearing_range")]
    pub shroud_clearing_range: f32,
    /// C++ `ThingTemplate::m_shroudRevealToAllRange`. `-1` / `<= 0` means none.
    #[serde(default = "default_template_shroud_reveal_to_all_range")]
    pub shroud_reveal_to_all_range: f32,
    /// C++ `KINDOF_REVEAL_TO_ALL` — full ally-range looker for every player.
    #[serde(default)]
    pub reveal_to_all: bool,
    /// C++ `KINDOF_ALWAYS_VISIBLE` — never shrouded (UI feedback objects).
    #[serde(default)]
    pub always_visible: bool,
    pub build_cost: Resources,
    pub build_time: f32,
    /// C++ `ThingTemplate::m_buildable` (`BSTATUS_YES` = 0).
    #[serde(default)]
    pub buildable_status: u32,
    /// C++ `ThingTemplate::m_refundValue` from Object INI `RefundValue`.
    /// A zero value means "use BuildCost × GlobalData::SellPercentage";
    /// a non-zero value is an exact sale refund.
    #[serde(default)]
    pub refund_value: u16,
    /// C++ `ThingTemplate::m_threatValue` from Object INI `ThreatValue`.
    /// Object::addThreat stamps this, never BuildCost (Object.cpp:4873).
    #[serde(default)]
    pub threat_value: u16,
    pub model_name: Option<String>,
    pub texture_name: Option<String>,
    /// C++ `ThingTemplate::m_assetScale` from Object INI `Scale`.
    #[serde(default = "default_asset_scale")]
    pub asset_scale: f32,
    /// Authored DockUpdate family.  Never infer this from a template name.
    #[serde(default)]
    pub dock_kind: DockKind,
    /// `SupplyWarehouseDockUpdate::StartingBoxes`, when authored.  `Some(0)`
    /// is meaningful and must remain distinct from no warehouse module.
    #[serde(default)]
    pub dock_starting_boxes: Option<u32>,
    /// `SupplyWarehouseDockUpdate::DeleteWhenEmpty`.  It only applies to a
    /// warehouse dock; ordinary resource objects retain their own lifecycle.
    #[serde(default)]
    pub dock_delete_when_empty: bool,
    /// Exact `SupplyTruckAIUpdate` module data, when authored.
    #[serde(default)]
    pub supply_truck_metadata: Option<SupplyTruckMetadata>,
    /// C++ `SupplyTruckAIUpdateModuleData::m_suppliesDepletedVoice`.
    #[serde(default)]
    pub supplies_depleted_voice: String,

    /// `RailedTransportContain::Slots`, when that exact contain module is
    /// present.  A railed dock with no contain module never gains synthetic
    /// transport capacity.
    #[serde(default)]
    pub railed_transport_slots: Option<usize>,
    /// C++ `RailedTransportAIUpdateModuleData::m_pathPrefixName`.
    /// Empty unless that exact AI module authored `PathPrefixName`.
    #[serde(default)]
    pub railed_path_prefix_name: String,

    /// Exact source containment behavior and capacity, parsed from the Object
    /// INI module rather than inferred from VEHICLE, dimensions, or a name.
    #[serde(default)]
    pub contain_module: ContainModuleMetadata,
    /// Exact StealthUpdate FriendlyOpacityMin/Max values.  They are retained
    /// on the immutable template so presentation can select the C++ friendly
    /// look without re-reading INI or live GameLogic during WGPU collection.
    #[serde(default = "default_stealth_friendly_opacity_min")]
    pub stealth_friendly_opacity_min: f32,
    #[serde(default = "default_stealth_friendly_opacity_max")]
    pub stealth_friendly_opacity_max: f32,
    /// Exact `ParkingPlaceBehavior` data.  This remains absent when the
    /// source object has no such behavior, even if its KindOf is
    /// `FSAirfield`; physical aircraft landing then fails closed.
    #[serde(default)]
    pub parking_place: Option<ParkingPlaceMetadata>,
    /// Exact `FlightDeckBehavior` data.  Absent unless the source object
    /// declares that Behavior; a carrier KindOf never fabricates a deck.
    #[serde(default)]
    pub flight_deck: Option<FlightDeckMetadata>,
    /// Exact `DeployStyleAIUpdate` behavior.  It remains absent unless the
    /// source object actually declares that Behavior; a vehicle name or
    /// `CAN_ATTACK` KindOf never creates deploy authority.
    #[serde(default)]
    pub deploy_style_metadata: Option<DeployStyleMetadata>,
    /// Exact `QueueProductionExitUpdate` or `DefaultProductionExitUpdate`
    /// module.  Missing data never grants a named producer a synthetic Queue
    /// delay, authored exit point, or batch-release policy.
    #[serde(default)]
    pub production_exit_metadata: Option<ProductionExitMetadata>,
    /// Exact `VeterancyCrateCollide IsPilot` data.  This remains absent for a
    /// pilot-named template unless its Object INI authors and parses the
    /// corresponding behavior module.
    #[serde(default)]
    pub veterancy_crate_collide: Option<VeterancyCrateCollideMetadata>,
    /// Exact `EjectPilotDie` module data.  Presence records the C++ die
    /// interface for Hijacker behavior; active death spawning separately
    /// rejects unrepresentable filters or OCLs.
    #[serde(default)]
    pub eject_pilot_die: Option<EjectPilotDieMetadata>,
    /// Exact `RebuildHoleExposeDie` module data. Presence, not a GLA/name
    /// heuristic, is the C++ hole-expose authority.
    #[serde(default)]
    pub rebuild_hole_expose: Option<RebuildHoleExposeDieMetadata>,
    /// Exact `HackInternetAIUpdate` module data.  This remains absent when a
    /// source unit is merely named like a hacker; active command and income
    /// authority require this typed behavior.
    #[serde(default)]
    pub hack_internet_ai_update: Option<HackInternetAIUpdateMetadata>,
    /// Exact paired `SpecialAbility` + `SpecialAbilityUpdate` data for the
    /// Hacker Disable Building channel.  No generic Hacker/template-name
    /// fallback may populate this capability.
    #[serde(default)]
    pub hacker_disable_building: Option<HackerDisableBuildingMetadata>,
    /// Authored `SpecialAbilityUpdate` rows for timed/remote C4 / TNT plants.
    #[serde(default)]
    pub charge_plant_abilities: Vec<ChargePlantAbilityMetadata>,

    /// Source-ordered SpecialPowerModule interfaces.  This is generic module
    /// identity, not a replacement for the HDB paired-channel metadata above.
    #[serde(default)]
    pub special_power_modules: Vec<SpecialPowerModuleMetadata>,
    /// C++ Object INI `EnergyProduction`.  It is an Object field, not a
    /// SpecialPowerModule property: Scud/Particle/Nuke must not borrow a
    /// template-name fallback to affect team power.
    #[serde(default)]
    pub energy_production: Option<i32>,
    /// C++ Object INI `MaxSimultaneousLinkKey`; kept independently because a
    /// link-key object need not expose a player SpecialPowerModule.
    #[serde(default)]
    pub max_simultaneous_link_key: Option<String>,
    /// C++ Object INI `MaxSimultaneousOfType` numeric cap. `0` is unlimited
    /// unless `DeterminedBySuperweaponRestriction` overrides via GameLogic.
    #[serde(default)]
    pub max_simultaneous_of_type: u16,
    /// Exact `MaxSimultaneousOfType=DeterminedBySuperweaponRestriction`
    /// policy associated with this object template.
    #[serde(default)]
    pub max_simultaneous_determined_by_superweapon_restriction: bool,
    /// C++ Object INI `EnergyBonus`.  `None` is its constructor default of
    /// zero; a valid OverchargeBehavior therefore remains player-toggleable
    /// with no power delta.  The parser rejects a malformed *present* field
    /// before exposing the behavior as authority.
    #[serde(default)]
    pub energy_bonus: Option<i32>,
    /// Exact `OverchargeBehavior` data.  A missing or malformed behavior
    /// never receives a player-facing toggle merely because it is a China
    /// plant or has a PowerPlant KindOf.
    #[serde(default)]
    pub overcharge_behavior: Option<OverchargeBehaviorMetadata>,
    /// Exact `PowerPlantUpdate` data for visual rod state.  Its absence does
    /// not prevent an otherwise valid OverchargeBehavior from adding power.
    #[serde(default)]
    pub power_plant_update: Option<PowerPlantUpdateMetadata>,
    /// C++ Object INI `TransportSlotCount`: how much capacity this source
    /// consumes when boarding a normal transport.  `None` is intentionally
    /// unproven and fails closed in a player Enter command.
    #[serde(default)]
    pub transport_slot_count: Option<usize>,
    /// C++ `KINDOF_CAPTURABLE`, retained outside the packed KindOf bank so
    /// capture authorization is data-driven rather than inferred from a
    /// faction/building name.
    #[serde(default)]
    pub capturable: bool,
    /// C++ `KINDOF_IMMUNE_TO_CAPTURE`, likewise independent of targetability
    /// and ordinary structure classification.
    #[serde(default)]
    pub immune_to_capture: bool,
    /// Exact `GarrisonContain` module capacity.  `None` means this target is
    /// not garrisonable for the C++ capture legality check; `Some(0)` remains
    /// distinct and intentionally fail-closed.
    #[serde(default)]
    pub garrison_contain_max: Option<usize>,
    /// Exact `SpecialAbility` capture module on this source, if any.
    #[serde(default)]
    pub capture_power: CapturePowerKind,
    /// `SpecialAbility::StartsPaused` for the authored capture power.
    #[serde(default)]
    pub capture_starts_paused: bool,
    /// `UnpauseSpecialPowerUpgrade::TriggeredBy` for that same capture power.
    #[serde(default)]
    pub capture_upgrade_trigger: Option<String>,
    /// `SpecialAbilityUpdate::StartAbilityRange`.  The authority state uses
    /// this rather than a hero/template-name range fallback.
    #[serde(default)]
    pub capture_start_ability_range: Option<f32>,
    /// `SpecialAbilityUpdate::UnpackTime` (milliseconds).  Capture cannot
    /// begin preparation until this authored animation phase has elapsed.
    #[serde(default)]
    pub capture_unpack_time_ms: Option<u32>,
    /// `SpecialAbilityUpdate::PreparationTime` (milliseconds).  This is the
    /// live channel duration after unpacking, not a click-time delay.
    #[serde(default)]
    pub capture_preparation_time_ms: Option<u32>,
    /// `SpecialAbilityUpdate::PackTime` (milliseconds).  C++ keeps the
    /// ability active through this post-trigger phase before returning idle.
    #[serde(default)]
    pub capture_pack_time_ms: Option<u32>,
    /// `SpecialAbilityUpdate::PackUnpackVariationFactor` for the capture pair.
    #[serde(default)]
    pub capture_pack_unpack_variation_factor: f32,
    /// `SpecialAbilityUpdate::UnpackSound` for the capture module.
    #[serde(default)]
    pub capture_unpack_sound: Option<String>,
    /// `SpecialAbilityUpdate::PackSound` for the capture module.
    #[serde(default)]
    pub capture_pack_sound: Option<String>,
    /// `SpecialAbilityUpdate::TriggerSound` for the capture module.
    #[serde(default)]
    pub capture_trigger_sound: Option<String>,
    /// `SpecialAbilityUpdate::TriggerSound` for leftover steal/disable modules.
    #[serde(default)]
    pub leftover_sa_trigger_sound: Option<String>,

    pub special_power_cooldown: f32,
    /// C++ parity: XP awarded to the killer when this object is destroyed.
    /// Rookie/Regular token; prefer `experience_values` when authored.
    pub experience_value: f32,
    /// C++ `ExperienceValue` 4-int list [Regular, Veteran, Elite, Heroic].
    #[serde(default)]
    pub experience_values: [f32; 4],
    /// C++ `SkillPointValue` 4-int list. `-999` (`USE_EXP_VALUE_FOR_SKILL_VALUE`)
    /// falls back to `ExperienceValue` for that level.
    #[serde(default = "default_template_skill_point_values")]
    pub skill_point_values: [i32; 4],
    /// C++ `ExperienceRequired` mapped to [Veteran, Elite, Heroic] thresholds.
    /// Defaults to [60, 150, 300] for unparsed templates.
    pub veterancy_xp_thresholds: [f32; 3],
    /// C++ ThingTemplate `IsTrainable` (default FALSE).
    #[serde(default)]
    pub is_trainable: bool,
    /// C++ ThingTemplate `EnterGuard` (default FALSE). Guard boards instead of shooting.
    #[serde(default)]
    pub enter_guard: bool,
    /// C++ ThingTemplate `HijackGuard` (default FALSE). Guard hijacks enemy vehicles.
    #[serde(default)]
    pub hijack_guard: bool,
    /// Authored `VeterancyGainCreate` modules (StartingLevel / ScienceRequired).
    #[serde(default)]
    pub veterancy_gain_creates: Vec<VeterancyGainCreateMetadata>,
    /// Authored `GrantUpgradeCreate` modules (UpgradeToGrant / ExemptStatus).
    #[serde(default)]
    pub grant_upgrade_creates: Vec<GrantUpgradeCreateMetadata>,
    /// Authored `LockWeaponCreate` slot (PRIMARY=0, SECONDARY=1, TERTIARY=2).
    #[serde(default)]
    pub lock_weapon_slot: Option<u8>,
    /// Authored `PreorderCreate` module presence (not a template-name heuristic).
    #[serde(default)]
    pub has_preorder_create: bool,
    /// Authored `SpecialPowerCreate` module presence.
    #[serde(default)]
    pub has_special_power_create: bool,
    /// Authored `SupplyCenterCreate` module presence.
    #[serde(default)]
    pub has_supply_center_create: bool,
    /// Authored `SupplyWarehouseCreate` module presence.
    #[serde(default)]
    pub has_supply_warehouse_create: bool,
    /// Host primary weapon stats when the template defines combat capability.
    /// Prefer this over ad-hoc `Weapon::default()` injection at create time.
    pub primary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI primary weapon template name (resolved via WeaponStore).
    pub primary_weapon_name: Option<String>,
    /// An authored no-flag `WeaponSet` explicitly contained `PRIMARY None`.
    /// This is distinct from a template with no retained WeaponSet at all:
    /// the latter may use legacy host fallback while the former must remain
    /// unarmed until a supported conditional set is selected.
    #[serde(default)]
    pub primary_weapon_explicitly_none: bool,
    /// Exact `WeaponSet Conditions = MINE_CLEARING_DETAIL` PRIMARY instance.
    /// It is separate from the ordinary primary so toggling the C++ detail bit
    /// cannot overwrite cooldown/ammo state of a normal combat weapon.
    #[serde(default)]
    pub mine_clearing_primary_weapon: Option<Weapon>,
    /// Source Weapon.ini name for the bounded mine-clearing conditional slot.
    #[serde(default)]
    pub mine_clearing_primary_weapon_name: Option<String>,
    /// Host secondary weapon stats (Weapon = SECONDARY Name). Optional; no kind fallback.
    pub secondary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI secondary weapon template name (resolved via WeaponStore).
    pub secondary_weapon_name: Option<String>,
    /// Host tertiary weapon stats (`Weapon = TERTIARY Name`).
    ///
    /// Kept separate from SECONDARY because C++ WeaponSet has three concrete
    /// slots. In particular, Comanche rocket pods must not replace its
    /// anti-tank SECONDARY weapon.
    #[serde(default)]
    pub tertiary_weapon: Option<Weapon>,
    /// Weapon.ini / Object INI tertiary weapon template name (resolved via WeaponStore).
    #[serde(default)]
    pub tertiary_weapon_name: Option<String>,
    /// C++ `WeaponTemplateSet::m_preferredAgainst` per slot (0=PRIMARY).
    /// Empty means the live chooser falls back to residual damage heuristics.
    #[serde(default)]
    pub preferred_against: [Vec<KindOf>; 3],
    /// C++ `WeaponTemplateSet::m_isReloadTimeShared`.
    #[serde(default)]
    pub share_weapon_reload_time: bool,
    /// C++ `WeaponTemplateSet::m_autoChooseMask` per slot. Default all-sources.
    #[serde(default = "default_auto_choose_masks")]
    pub auto_choose_masks: [u32; 3],
    /// C++ `WeaponTemplateSet::m_isWeaponLockSharedAcrossSets`.
    #[serde(default)]
    pub weapon_lock_shared_across_sets: bool,
    /// C++ `WeaponSet` `AutoChooseSources = PRIMARY NONE`.
    ///
    /// The authored PRIMARY still resolves from Weapon.ini when present, but
    /// Object construction must not invent a kind-based `Weapon::default`
    /// after a store miss (Strategy Center artillery starts turret-disabled).
    #[serde(default)]
    pub primary_auto_choose_none: bool,
    /// C++ `FireOCLAfterWeaponCooldownUpdate` is present on the Object INI.
    /// Create installs the residual module from this flag, not a unit name.
    #[serde(default)]
    pub has_fire_ocl_after_weapon_cooldown: bool,
    /// Source-ordered `FireWeaponWhenDamagedBehavior` module data.  This is
    /// retained separately from ordinary WeaponSet slots because C++ creates
    /// up to eight independent PRIMARY `Weapon` instances per module.  Main
    /// does not activate the records until Object snapshot persistence can
    /// carry each mutable Weapon state in C++ Xfer order.
    #[serde(default)]
    pub fire_weapon_when_damaged_behaviors:
        Vec<crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDamagedMetadata>,
    /// Source-ordered `FireWeaponWhenDeadBehavior` module data.  C++ creates
    /// a fresh ephemeral PRIMARY Weapon for each qualifying death, so these
    /// records retain source gates/references only and add no object snapshot
    /// state by themselves.
    #[serde(default)]
    pub fire_weapon_when_dead_behaviors:
        Vec<crate::game_logic::host_temporary_weapon_behavior::FireWeaponWhenDeadMetadata>,
    /// Locomotor.ini SET_NORMAL template name (resolved via Common LocomotorStore).
    /// Primary member only; the full SET_* row lives in `locomotor_set_names`.
    pub locomotor_name: Option<String>,
    /// Authored SET_NORMAL (or current SET_*) members in declaration order.
    /// C++ `chooseGoodLocomotorFromCurrentSet` picks one by cell surface.
    #[serde(default)]
    pub locomotor_set_names: Vec<String>,
    /// None identifies legacy/catalog-less templates; Some retains authored rows,
    /// including an explicitly empty set, without consulting an ambient catalog.
    #[serde(default)]
    pub authored_locomotor_sets:
        Option<Vec<crate::game_logic::host_upgrade_module_residuals::AuthoredLocomotorSet>>,
    /// C++ CreateCrateDieModuleData::m_crateNameList residual (CrateData names).
    #[serde(default)]
    pub create_crate_data: Vec<String>,
    /// C++ `ThingTemplate::m_armorTemplateSets` from Object INI `ArmorSet`.
    #[serde(default)]
    pub armor_sets: Vec<HostArmorSet>,
    /// C++ `ActiveBodyModuleData::m_subdualDamageCap`. Default 0 = immune.
    #[serde(default)]
    pub subdual_damage_cap: f32,
    /// C++ `SubdualDamageHealRate` converted to logic frames.
    #[serde(default)]
    pub subdual_heal_rate_frames: u32,
    /// C++ `SubdualDamageHealAmount`.
    #[serde(default)]
    pub subdual_heal_amount: f32,
    /// C++ PhysicsBehaviorModuleData::m_mass from Object INI `Mass`.
    #[serde(default = "default_template_physics_mass")]
    pub physics_mass: f32,
    /// C++ PhysicsBehaviorModuleData::m_shockResistance from `ShockResistance`.
    #[serde(default)]
    pub shock_resistance: f32,
    /// C++ PhysicsBehaviorModuleData::m_pitchRollYawFactor (default 2.0).
    #[serde(default = "default_template_pitch_roll_yaw_factor")]
    pub pitch_roll_yaw_factor: f32,
    /// C++ PhysicsBehaviorModuleData friction (per-frame after parseFrictionPerSec).
    #[serde(default = "default_template_forward_friction")]
    pub forward_friction: f32,
    #[serde(default = "default_template_lateral_friction")]
    pub lateral_friction: f32,
    #[serde(default = "default_template_z_friction")]
    pub z_friction: f32,
    #[serde(default)]
    pub aerodynamic_friction: f32,
    /// C++ PhysicsBehaviorModuleData::m_centerOfMassOffset.
    #[serde(default)]
    pub center_of_mass_offset: f32,
    /// C++ PhysicsBehaviorModuleData::m_allowBouncing.
    #[serde(default)]
    pub allow_bouncing: bool,
    /// C++ PhysicsBehaviorModuleData::m_allowCollideForce (default true).
    #[serde(default = "default_template_allow_collide_force")]
    pub allow_collide_force: bool,
    /// C++ PhysicsBehaviorModuleData::m_killWhenRestingOnGround.
    #[serde(default)]
    pub kill_when_resting_on_ground: bool,
    /// C++ m_minFallSpeedForDamage after parseHeightToSpeed.
    #[serde(default = "default_template_min_fall_speed")]
    pub min_fall_speed_for_damage: f32,
    /// C++ PhysicsBehaviorModuleData::m_fallHeightDamageFactor (default 1).
    #[serde(default = "default_template_fall_height_damage_factor")]
    pub fall_height_damage_factor: f32,
    /// C++ `ThingTemplate::m_crusherLevel` from Object INI `CrusherLevel`.
    /// Default 0 = cannot crush anything (ThingTemplate.cpp:1023).
    #[serde(default)]
    pub crusher_level: u8,
    /// C++ `ThingTemplate::m_crushableLevel` from Object INI `CrushableLevel`.
    /// Default 255 = cannot be crushed (ThingTemplate.cpp:1024).
    #[serde(default = "default_template_crushable_level")]
    pub crushable_level: u8,
    /// C++ `ThingTemplate::m_fenceWidth` from Object INI `FenceWidth`.
    #[serde(default)]
    pub fence_width: f32,
    /// C++ `ThingTemplate::m_fenceXOffset` from Object INI `FenceXOffset`.
    #[serde(default)]
    pub fence_x_offset: f32,
    /// C++ `ThingTemplate::m_shadowSizeX` (Object INI `ShadowSizeX`).
    #[serde(default)]
    pub shadow_size_x: f32,
    /// C++ `ThingTemplate::m_shadowSizeY` (Object INI `ShadowSizeY`).
    #[serde(default)]
    pub shadow_size_y: f32,
    /// C++ `ThingTemplate::m_shadowType` (Object INI `Shadow` bitstring).
    #[serde(default)]
    pub shadow_type: u32,
    /// C++ `ThingTemplate::m_shadowOffsetX` (Object INI `ShadowOffsetX`).
    #[serde(default)]
    pub shadow_offset_x: f32,
    /// C++ `ThingTemplate::m_shadowOffsetY` (Object INI `ShadowOffsetY`).
    #[serde(default)]
    pub shadow_offset_y: f32,
    /// C++ `ThingTemplate::m_shadowTextureName` (Object INI `ShadowTexture`).
    #[serde(default)]
    pub shadow_texture: Option<String>,

    /// C++ `ThingTemplate::m_radarPriority` (Object INI `RadarPriority`).
    /// 0=INVALID, 1=NOT_ON_RADAR, 2=STRUCTURE, 3=UNIT, 4=LOCAL_UNIT_ONLY.
    #[serde(default)]
    pub radar_priority: u8,
    /// C++ `TTAUDIO_soundMoveStart`.
    #[serde(default)]
    pub sound_move_start: Option<String>,
    /// C++ `TTAUDIO_soundMoveStartDamaged`.
    #[serde(default)]
    pub sound_move_start_damaged: Option<String>,
    /// C++ `TTAUDIO_soundMoveLoop`.
    #[serde(default)]
    pub sound_move_loop: Option<String>,
    /// C++ `TTAUDIO_soundMoveLoopDamaged`.
    #[serde(default)]
    pub sound_move_loop_damaged: Option<String>,
    /// C++ `TTAUDIO_soundAmbient`.
    #[serde(default)]
    pub sound_ambient: Option<String>,
    /// C++ `TTAUDIO_soundAmbientDamaged`.
    #[serde(default)]
    pub sound_ambient_damaged: Option<String>,
    /// C++ `TTAUDIO_soundAmbientReallyDamaged`.
    #[serde(default)]
    pub sound_ambient_really_damaged: Option<String>,
    /// C++ `TTAUDIO_soundAmbientRubble`.
    #[serde(default)]
    pub sound_ambient_rubble: Option<String>,
    /// C++ `ThingTemplate::m_upgradeCameoUpgradeNames` (`UpgradeCameo1..5`).
    #[serde(default)]
    pub upgrade_cameo_names: [String; 5],

    /// C++ `ThingTemplate::m_geometryInfo` from Object INI Geometry*.
    #[serde(default)]
    pub geometry_info: HostGeometryInfo,
    /// C++ `ThingTemplate::m_structureRubbleHeight` (unsigned byte; 0 = GameData default).
    #[serde(default)]
    pub structure_rubble_height: u8,
    /// C++ `AIUpdateModuleData::m_autoAcquireEnemiesWhenIdle` from Object INI.
    #[serde(default)]
    pub auto_acquire_enemies_when_idle: u32,
    /// Exact authored module-interface metadata. `None` means unproven
    /// (hand-built rules or an unknown class), not absence. This belongs to
    /// the template; Object snapshots retain only the template identity.
    #[serde(default)]
    authored_ai_update_interface: Option<bool>,
    /// C++ `AIUpdateModuleData::m_forbidPlayerCommands` (Spectre gunship).
    #[serde(default)]
    pub forbid_player_commands: bool,
    /// Leftover `ThingTemplate::m_prereqInfo` from Object INI `Prerequisites`.
    /// Template data, not instance state — re-parsed from leftover factory / INI.
    #[serde(skip)]
    pub production_prerequisites: Vec<game_engine::common::rts::ProductionPrerequisite>,
    /// Names admitted with the prerequisite definitions, never resolved from
    /// another match's factory or science store during a gameplay query.
    #[serde(skip)]
    pub(crate) prerequisite_definitions: super::build_prerequisites::PrerequisiteDefinitions,
    /// CPP isEquivalentTo: reskins and authored build variations.
    #[serde(default)]
    pub(crate) reskinned_from: Option<String>,
    #[serde(default)]
    pub(crate) build_variations: Vec<String>,
}

impl ThingTemplate {
    pub(crate) fn authored_ai_update_interface(&self) -> Option<bool> {
        self.authored_ai_update_interface
    }

    pub(crate) fn set_authored_ai_update_interface(&mut self, presence: Option<bool>) {
        self.authored_ai_update_interface = presence;
    }

    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: name.to_string(),
            kind_of: HashSet::new(),
            max_health: 100.0,
            armor: 0.0,
            sight_range: 0.0,
            shroud_clearing_range: default_template_shroud_clearing_range(),
            shroud_reveal_to_all_range: default_template_shroud_reveal_to_all_range(),
            reveal_to_all: false,
            always_visible: false,
            build_cost: Resources::default(),
            build_time: 1.0,
            buildable_status: 0,
            refund_value: 0,
            threat_value: 0,
            model_name: None,
            texture_name: None,
            asset_scale: default_asset_scale(),
            dock_kind: DockKind::None,
            dock_starting_boxes: None,
            dock_delete_when_empty: false,
            supply_truck_metadata: None,
            supplies_depleted_voice: String::new(),
            railed_transport_slots: None,
            railed_path_prefix_name: String::new(),

            contain_module: ContainModuleMetadata::default(),

            stealth_friendly_opacity_min: default_stealth_friendly_opacity_min(),
            stealth_friendly_opacity_max: default_stealth_friendly_opacity_max(),
            parking_place: None,
            flight_deck: None,
            deploy_style_metadata: None,
            production_exit_metadata: None,
            veterancy_crate_collide: None,
            eject_pilot_die: None,
            rebuild_hole_expose: None,
            hack_internet_ai_update: None,
            hacker_disable_building: None,
            charge_plant_abilities: Vec::new(),
            special_power_modules: Vec::new(),
            energy_production: None,
            max_simultaneous_link_key: None,
            max_simultaneous_determined_by_superweapon_restriction: false,
            max_simultaneous_of_type: 0,
            energy_bonus: None,
            overcharge_behavior: None,
            power_plant_update: None,
            transport_slot_count: None,
            capturable: false,
            immune_to_capture: false,
            garrison_contain_max: None,
            capture_power: CapturePowerKind::None,
            capture_starts_paused: false,
            capture_upgrade_trigger: None,
            capture_start_ability_range: None,
            capture_unpack_time_ms: None,
            capture_preparation_time_ms: None,
            capture_pack_time_ms: None,
            capture_pack_unpack_variation_factor: 0.0,
            capture_unpack_sound: None,
            capture_pack_sound: None,
            capture_trigger_sound: None,
            leftover_sa_trigger_sound: None,

            special_power_cooldown: 10.0,

            experience_value: 0.0,
            experience_values: [0.0; 4],
            skill_point_values:
                [crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL;
                    4],
            veterancy_xp_thresholds: [60.0, 150.0, 300.0],
            is_trainable: false,
            enter_guard: false,
            hijack_guard: false,
            veterancy_gain_creates: Vec::new(),
            grant_upgrade_creates: Vec::new(),
            lock_weapon_slot: None,
            has_preorder_create: false,
            has_special_power_create: false,
            has_supply_center_create: false,
            has_supply_warehouse_create: false,
            primary_weapon: None,
            primary_weapon_name: None,
            primary_weapon_explicitly_none: false,
            mine_clearing_primary_weapon: None,
            mine_clearing_primary_weapon_name: None,
            secondary_weapon: None,
            secondary_weapon_name: None,
            tertiary_weapon: None,
            tertiary_weapon_name: None,
            preferred_against: [Vec::new(), Vec::new(), Vec::new()],
            share_weapon_reload_time: false,
            auto_choose_masks: default_auto_choose_masks(),
            weapon_lock_shared_across_sets: false,
            primary_auto_choose_none: false,
            has_fire_ocl_after_weapon_cooldown: false,
            fire_weapon_when_damaged_behaviors: Vec::new(),
            fire_weapon_when_dead_behaviors: Vec::new(),
            locomotor_name: None,
            locomotor_set_names: Vec::new(),
            authored_locomotor_sets: None,
            create_crate_data: Vec::new(),
            armor_sets: Vec::new(),
            subdual_damage_cap: 0.0,
            subdual_heal_rate_frames: 0,
            subdual_heal_amount: 0.0,
            physics_mass: 1.0,
            shock_resistance: 0.0,
            pitch_roll_yaw_factor: 2.0,
            forward_friction: 0.15,
            lateral_friction: 0.15,
            z_friction: 0.8,
            aerodynamic_friction: 0.0,
            center_of_mass_offset: 0.0,
            allow_bouncing: false,
            allow_collide_force: true,
            kill_when_resting_on_ground: false,
            min_fall_speed_for_damage: default_template_min_fall_speed(),
            fall_height_damage_factor: 1.0,
            crusher_level: 0,
            crushable_level: 255,
            fence_width: 0.0,
            fence_x_offset: 0.0,
            shadow_size_x: 0.0,
            shadow_size_y: 0.0,
            shadow_type: 0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_texture: None,

            radar_priority: 0,
            sound_move_start: None,
            sound_move_start_damaged: None,
            sound_move_loop: None,
            sound_move_loop_damaged: None,
            sound_ambient: None,
            sound_ambient_damaged: None,
            sound_ambient_really_damaged: None,
            sound_ambient_rubble: None,
            upgrade_cameo_names: Default::default(),

            geometry_info: HostGeometryInfo::default(),
            structure_rubble_height: 0,
            auto_acquire_enemies_when_idle: 0,
            authored_ai_update_interface: None,
            forbid_player_commands: false,
            production_prerequisites: Vec::new(),
            prerequisite_definitions: Default::default(),
            reskinned_from: None,
            build_variations: Vec::new(),
        }
    }
    /// C++ `ThingTemplate::getExperienceValue(level)`. Uses the authored
    /// 4-int table when any token is non-zero; otherwise the single
    /// `experience_value` field (tests / unparsed templates).
    pub fn experience_value_for_level(&self, level: VeterancyLevel) -> f32 {
        let idx = match level {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        if self.experience_values.iter().any(|v| *v != 0.0) {
            self.experience_values[idx]
        } else {
            self.experience_value
        }
    }

    /// C++ `ThingTemplate::getSkillPointValue(level)`.
    pub fn skill_point_value_for_level(&self, level: VeterancyLevel) -> i32 {
        let idx = match level {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        let value = self.skill_point_values[idx];
        if value == crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL
        {
            self.experience_value_for_level(level) as i32
        } else {
            value
        }
    }

    pub fn charge_plant_ability_for_timed(&self) -> Option<&ChargePlantAbilityMetadata> {
        self.charge_plant_abilities
            .iter()
            .find(|ability| ability.is_timed_charge_power())
    }

    pub fn charge_plant_ability_for_remote(&self) -> Option<&ChargePlantAbilityMetadata> {
        self.charge_plant_abilities
            .iter()
            .find(|ability| ability.is_remote_charge_power())
    }

    /// Preserve a drawable authored C++ asset scale. Retail Object INIs use
    /// positive finite values; malformed values retain the default instead of
    /// entering a WGPU transform as NaN or infinity.
    pub fn set_asset_scale(&mut self, scale: f32) -> &mut Self {
        if scale.is_finite() && scale > 0.0 {
            self.asset_scale = scale;
        }
        self
    }

    /// Whether this template crossed the typed authority boundary for an
    /// Overcharge command.  The behavior alone is authoritative: C++ permits
    /// an authored module when ThingTemplate::EnergyBonus retains its default
    /// zero.  The parser rejects malformed present bonus fields before this
    /// can return true.
    #[inline]
    pub fn supports_overcharge(&self) -> bool {
        self.overcharge_behavior.is_some()
    }

    /// C++ `Object::getSpecialPowerModule`-style lookup by the host command
    /// adapter.  The stored module record remains keyed by exact loaded
    /// template name/id; the enum comparison merely routes an already parsed
    /// command to the first matching source module in declaration order.
    #[inline]
    pub fn special_power_module_for_command(
        &self,
        power: &crate::command_system::SpecialPowerType,
    ) -> Option<&SpecialPowerModuleMetadata> {
        self.special_power_modules
            .iter()
            .find(|module| module.command_power.as_ref() == Some(power))
    }

    /// Whether this exact Object INI template participates in the C++
    /// `DeterminedBySuperweaponRestriction` link-key quota.
    #[inline]
    pub fn has_superweapon_restriction_link_key(&self) -> bool {
        self.max_simultaneous_determined_by_superweapon_restriction
            && self
                .max_simultaneous_link_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty())
    }

    /// C++ `ThingTemplate::getMaxSimultaneousOfType`.
    #[inline]
    pub fn get_max_simultaneous_of_type(&self, superweapon_restriction: u32) -> u32 {
        if self.max_simultaneous_determined_by_superweapon_restriction {
            superweapon_restriction
        } else {
            u32::from(self.max_simultaneous_of_type)
        }
    }

    /// C++ `countExisting` template match: `isEquivalentTo` (name) or shared
    /// `MaxSimultaneousLinkKey`.
    #[inline]
    pub fn counts_toward_max_simultaneous_of(&self, wanted: &ThingTemplate) -> bool {
        if self.is_equivalent_to(wanted) {
            return true;
        }
        match (
            wanted
                .max_simultaneous_link_key
                .as_deref()
                .map(str::trim)
                .filter(|key| !key.is_empty()),
            self.max_simultaneous_link_key
                .as_deref()
                .map(str::trim)
                .filter(|key| !key.is_empty()),
        ) {
            (Some(wanted_key), Some(candidate_key))
                if wanted_key.eq_ignore_ascii_case(candidate_key) =>
            {
                true
            }
            _ => false,
        }
    }

    /// Attach host primary weapon stats (damage/range/reload) to this template.
    /// C++ CreateCrateDie CrateData residual append.
    pub fn add_create_crate_data(&mut self, crate_data_name: &str) -> &mut Self {
        let n = crate_data_name.trim();
        if !n.is_empty() {
            self.create_crate_data.push(n.to_string());
        }
        self
    }

    pub fn set_primary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.primary_weapon = Some(weapon);
        self.primary_weapon_explicitly_none = false;
        self
    }

    /// Record the Weapon.ini template name for store lookup at create time.
    pub fn set_primary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.primary_weapon_name = Some(n.to_string());
            self.primary_weapon_explicitly_none = false;
        }
        self
    }

    /// Preserve `WeaponSet Conditions = None` / `Weapon = PRIMARY None`.
    /// This must suppress generic kind/name fallback so a dozer or worker
    /// cannot gain an invented ordinary primary before its authored mine-clear
    /// detail set is selected.
    pub fn set_primary_weapon_none(&mut self) -> &mut Self {
        self.primary_weapon = None;
        self.primary_weapon_name = None;
        self.primary_weapon_explicitly_none = true;
        self
    }

    /// Attach exact host stats for a supported mine-clearing conditional
    /// primary. This has no relation to a generic `Worker`/`Dozer` identity.
    pub fn set_mine_clearing_primary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.mine_clearing_primary_weapon = Some(weapon);
        self
    }

    /// Record the exact Weapon.ini name used by `MINE_CLEARING_DETAIL`.
    /// Empty/`None` rows remain absent and therefore fail closed at arm time.
    pub fn set_mine_clearing_primary_weapon_name(&mut self, name: &str) -> &mut Self {
        let name = name.trim();
        if !name.is_empty() && !name.eq_ignore_ascii_case("none") {
            self.mine_clearing_primary_weapon_name = Some(name.to_string());
        }
        self
    }

    /// Attach host secondary weapon stats (damage/range/reload) to this template.
    pub fn set_secondary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.secondary_weapon = Some(weapon);
        self
    }

    /// Record the Weapon.ini secondary template name for store lookup at create time.
    /// Fail-closed: "None"/empty does not register a secondary slot.
    pub fn set_secondary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.secondary_weapon_name = Some(n.to_string());
        }
        self
    }

    /// Attach host tertiary weapon stats (damage/range/reload) to this template.
    pub fn set_tertiary_weapon(&mut self, weapon: Weapon) -> &mut Self {
        self.tertiary_weapon = Some(weapon);
        self
    }

    /// Record the Weapon.ini tertiary template name for store lookup at create time.
    /// Fail-closed: "None"/empty does not register a tertiary slot.
    pub fn set_tertiary_weapon_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.tertiary_weapon_name = Some(n.to_string());
        }
        self
    }

    /// Record the Locomotor.ini SET_NORMAL template name for store lookup at create time.
    /// Fail-closed: empty/"None" does not register a locomotor bind.
    pub fn set_locomotor_name(&mut self, name: &str) -> &mut Self {
        let n = name.trim();
        if !n.is_empty() && !n.eq_ignore_ascii_case("none") {
            self.locomotor_name = Some(n.to_string());
        }
        self
    }

    /// Store every authored SET_* member so live march can surface-switch.
    pub fn set_locomotor_set_names(&mut self, names: &[String]) -> &mut Self {
        self.locomotor_set_names = names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("none"))
            .map(|n| n.to_string())
            .collect();
        if self.locomotor_name.is_none() {
            if let Some(first) = self.locomotor_set_names.first() {
                self.locomotor_name = Some(first.clone());
            }
        }
        self
    }

    /// Resolve host Movement stats from the Locomotor catalog:
    /// 1) explicit locomotor_name → LocomotorStore (seed/INI)
    /// Fail-closed: no kind-based default — units without a name keep Movement::default().
    pub fn resolve_movement(&self) -> Option<super::locomotor_bootstrap::HostMovementStats> {
        if let Some(name) = self.locomotor_name.as_deref() {
            // Host residual: unit tests / early create often have an empty store
            // (no AssetManager archive load). Bootstrap seeds known locomotors or
            // loads extracted Locomotor.ini when present — see locomotor_bootstrap.rs.
            return super::locomotor_bootstrap::resolve_host_movement(name);
        }
        None
    }

    /// Resolve weapon for a newly created combat unit:
    /// 1) explicit host stats, 2) WeaponStore by primary_weapon_name,
    /// 3) host residual map by template name (`primary_weapon_name_for_unit`),
    /// 4) kind-based default fallback (fail-open last resort for Attackable kinds).
    pub fn resolve_primary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.primary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.primary_weapon_name.as_deref() {
            // Host residual: unit tests / early create often have an empty store
            // (no AssetManager archive load). Bootstrap seeds known weapons or
            // loads extracted Weapon.ini when present — see weapon_bootstrap.rs.
            if let Some(w) = Self::weapon_from_host_store(name) {
                return Some(w);
            }
        }
        if self.primary_weapon_explicitly_none || self.primary_auto_choose_none {
            // C++ Object.cpp:160-497 arms only ThingTemplate WeaponSet data.
            // AutoChooseSources=PRIMARY NONE must not fall through to a
            // kind-based Weapon::default after a store miss.
            return None;
        }
        // Host residual map: templates often omit primary_weapon_name (units.rs /
        // setup_templates gaps) but have a known retail weapon for the unit name.
        // Prefer store residual over kind-based Weapon::default().
        if let Some(wname) = super::weapon_bootstrap::primary_weapon_name_for_unit(&self.name) {
            if let Some(w) = Self::weapon_from_host_store(wname) {
                return Some(w);
            }
        }
        // C++ dozers carry only MINE_CLEARING_DETAIL (Object.cpp:160-497 arms
        // ThingTemplate WeaponSet data; a Dozer template has no primary gun).
        // The kind-based Weapon::default() last resort must not hand
        // Dozer-kind templates an attack weapon or can_attack flips true.
        if self.is_kind_of(KindOf::Dozer) {
            return None;
        }
        if self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
            || self.is_kind_of(KindOf::Attackable)
        {
            // Last-resort host combat stats when no template/store weapon is usable.
            return Some(Weapon::default());
        }
        None
    }

    /// Resolve secondary weapon for a newly created combat unit.
    /// Fail-closed (not full WeaponSet):
    /// 1) explicit host stats, 2) WeaponStore by secondary_weapon_name,
    /// 3) host residual map by template name (`secondary_weapon_name_for_unit`).
    /// No kind-based `Weapon::default()` fallback — units without SECONDARY stay unarmed there.
    pub fn resolve_secondary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.secondary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.secondary_weapon_name.as_deref() {
            if let Some(w) = Self::weapon_from_host_store(name) {
                return Some(w);
            }
        }
        // Host residual map: secondary slot by unit template name when not set.
        if let Some(wname) = super::weapon_bootstrap::secondary_weapon_name_for_unit(&self.name) {
            if let Some(w) = Self::weapon_from_host_store(wname) {
                return Some(w);
            }
        }
        None
    }

    /// Resolve tertiary weapon for a newly created combat unit.
    ///
    /// TERTIARY has no template-name or KindOf fallback: it is generally a
    /// manual/conditional WeaponSet slot, so inventing one would turn an
    /// unavailable ability into a primary shot.
    pub fn resolve_tertiary_weapon(&self) -> Option<Weapon> {
        if let Some(w) = &self.tertiary_weapon {
            return Some(w.clone());
        }
        if let Some(name) = self.tertiary_weapon_name.as_deref() {
            return Self::weapon_from_host_store(name);
        }
        None
    }

    /// Resolve the exact supported `MINE_CLEARING_DETAIL` primary. Unlike the
    /// ordinary primary there is intentionally no template-name or KindOf
    /// fallback: an untyped unit may not acquire mine-clearing authority.
    pub fn resolve_mine_clearing_primary_weapon(&self) -> Option<Weapon> {
        if let Some(weapon) = &self.mine_clearing_primary_weapon {
            return Some(weapon.clone());
        }
        let name = self.mine_clearing_primary_weapon_name.as_deref()?;
        Self::weapon_from_host_store(name)
    }

    /// Convert a gamelogic WeaponStore template into Main host Weapon stats.
    /// Returns None if store is missing or stats are unusable (0 dmg/range).
    pub fn weapon_from_store(name: &str) -> Option<Weapon> {
        let template =
            gamelogic::weapon::with_weapon_store(|store| store.find_weapon_template(name).cloned())
                .ok()??;
        Self::weapon_from_template(&template)
    }

    /// Explicit host admission followed by the same native-rule conversion.
    /// Ordinary resolution uses this; raw `weapon_from_store` stays a lookup.
    pub(in crate::game_logic) fn weapon_from_host_store(name: &str) -> Option<Weapon> {
        let template = super::weapon_bootstrap::with_host_weapon_store(|store| {
            store.find_weapon_template(name).cloned()
        })
        .ok()??;
        Self::weapon_from_template(&template)
    }

    fn weapon_from_template(wt: &gamelogic::weapon::WeaponTemplate) -> Option<Weapon> {
        use gamelogic::weapon::{WeaponAntiMask, WeaponBonus};
        const FPS: f32 = 30.0;
        if wt.primary_damage <= 0.0 || wt.attack_range <= 0.0 {
            return None;
        }
        // Leftover WeaponTemplate::get_delay_between_shots (Weapon.cpp:475-490).
        // DelayBetweenShots is a Min/Max range, not clip vs between. Identity
        // RATE_OF_FIRE here — leftover applies the ROF floor at fire. Ready-checks
        // must not consume GameLogicRandomValue (C++ draws once in privateFireWeapon);
        // force leftover's min==max branch for the stored yardstick.
        let delay_frames = if wt.min_delay_between_shots == wt.max_delay_between_shots {
            wt.get_delay_between_shots(&WeaponBonus::new())
        } else {
            let mut yardstick = gamelogic::weapon::WeaponTemplate::new(wt.name.clone());
            yardstick.min_delay_between_shots = wt.min_delay_between_shots;
            yardstick.max_delay_between_shots = wt.min_delay_between_shots;
            yardstick.get_delay_between_shots(&WeaponBonus::new())
        };
        let reload_time = if delay_frames > 0 {
            delay_frames as f32 / FPS
        } else {
            1.0
        };
        let pre_attack_delay = (wt.pre_attack_delay.max(0) as f32) / FPS;
        let projectile_speed = if wt.weapon_speed >= 999_999.0 {
            0.0
        } else {
            wt.weapon_speed
        };
        let suspend_fx_frame = crate::game_logic::host_historic_bonus::logic_frame()
            .saturating_add(wt.suspend_fx_delay);
        // Keep authored range values, as WeaponTemplate does in C++.
        // Runtime applies RANGE first, then one quarter-cell deduction
        // (Weapon.cpp:437-462); pre-reducing here would deduct twice and
        // multiply the first deduction by RANGE bonuses.
        Some(Weapon {
            damage: wt.primary_damage,
            range: wt.get_unmodified_attack_range(),
            min_range: wt.minimum_attack_range,
            reload_time,
            last_fire_time: 0.0,
            ammo: if wt.clip_size > 0 {
                Some(wt.clip_size as u32)
            } else {
                None
            },
            clip_size: wt.clip_size.max(0) as u32,
            // C++ ClipReloadTime is independent of DelayBetweenShots. Store
            // already converted msec → frames; host Weapon uses seconds.
            // Absent/0 stays 0 — reloadWithBonus is ready the same frame.
            clip_reload_time: if wt.clip_size > 0 {
                (wt.clip_reload_time.max(0) as f32) / FPS
            } else {
                0.0
            },
            can_target_air: wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_VEHICLE)
                || wt.anti_mask.contains(WeaponAntiMask::AIRBORNE_INFANTRY),
            // C++ WeaponTemplate defaults to AntiGround and accepts a ground
            // victim only when that actual anti-mask bit is set.  Treating an
            // arbitrary non-air mask (for example AntiProjectile) as ground
            // let point-defense-only weapons attack ordinary units.
            can_target_ground: wt.anti_mask.contains(WeaponAntiMask::GROUND),
            projectile_speed,
            pre_attack_delay,
            splash_radius: wt.primary_damage_radius.max(0.0),
            reloading_clip: false,
            last_bonus_rof: 0.0,
            suspend_fx_frame,
        })
    }

    /// C++ FiringTracker thresholds copied from the live WeaponStore.
    /// `weapon_from_store` only builds host `Weapon` fire stats; these fields
    /// live on the Object (ContinuousFireOne/Two/Coast, AutoReloadWhenIdle).
    pub fn weapon_tracker_from_store(name: &str) -> WeaponTrackerBind {
        use super::weapon_bootstrap::with_host_weapon_store as with_weapon_store;
        with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|wt| WeaponTrackerBind {
                    continuous_fire_one_shots: shots_needed_to_host(
                        wt.continuous_fire_one_shots_needed,
                    ),
                    continuous_fire_two_shots: shots_needed_to_host(
                        wt.continuous_fire_two_shots_needed,
                    ),
                    continuous_fire_coast_frames: wt.continuous_fire_coast_frames,
                    auto_reload_when_idle_frames: wt.auto_reload_when_idle_frames,
                })
        })
        .ok()
        .flatten()
        .unwrap_or_default()
    }

    /// Resolve ContinuousFire / AutoReloadWhenIdle for this template's primary.
    pub fn weapon_tracker_bind(&self) -> WeaponTrackerBind {
        let name = self
            .primary_weapon_name
            .as_deref()
            .or_else(|| super::weapon_bootstrap::primary_weapon_name_for_unit(&self.name));
        match name {
            Some(name) => Self::weapon_tracker_from_store(name),
            None => WeaponTrackerBind::default(),
        }
    }

    /// Apply one unconditional Object INI `WeaponSet` row (PreferredAgainst +
    /// ShareWeaponReloadTime + AutoChooseSources + WeaponLockSharedAcrossSets).
    /// C++ WeaponSet.cpp parsePreferredAgainst / parseAutoChoose / parseBool.
    pub fn apply_weapon_set_definition(&mut self, set: &crate::assets::WeaponSetDefinition) {
        for (key, value) in &set.attributes {
            if key.eq_ignore_ascii_case("ShareWeaponReloadTime")
                || key.eq_ignore_ascii_case("ShareReloadTime")
            {
                self.share_weapon_reload_time = parse_ini_bool(value);
                continue;
            }
            if key.eq_ignore_ascii_case("WeaponLockSharedAcrossSets")
                || key.eq_ignore_ascii_case("ShareWeaponLock")
            {
                self.weapon_lock_shared_across_sets = parse_ini_bool(value);
                continue;
            }
            let lower = key.to_ascii_lowercase();
            if lower == "preferredagainst" || lower.starts_with("preferredagainst") {
                if let Some((slot, kinds)) = parse_preferred_against_value(value) {
                    if let Some(slot_kinds) = self.preferred_against.get_mut(slot as usize) {
                        *slot_kinds = kinds;
                    }
                }
                continue;
            }
            if lower == "autochoosesources" || lower.starts_with("autochoosesources") {
                if let Some((slot, mask)) = parse_auto_choose_value(value) {
                    if let Some(slot_mask) = self.auto_choose_masks.get_mut(slot as usize) {
                        *slot_mask = mask;
                    }
                    if slot == 0 && mask == 0 {
                        self.primary_auto_choose_none = true;
                    }
                }
            }
        }
    }

    /// Fill PreferredAgainst / ShareWeaponReloadTime from the live Object INI
    /// catalog when the template has not already authored them (tests).
    pub fn bind_weapon_set_from_live_assets(&mut self) {
        if !(self.preferred_against.iter().any(|kinds| !kinds.is_empty())
            || self.share_weapon_reload_time)
        {
            if let Some(manager) = crate::assets::get_asset_manager() {
                if let Ok(guard) = manager.lock() {
                    if let Some(definition) = guard.get_object_definition(&self.name) {
                        if let Some(set) = definition
                            .weapon_sets
                            .iter()
                            .find(|set| set.is_unconditional())
                        {
                            self.apply_weapon_set_definition(set);
                        }
                    }
                }
            }
        }
        self.apply_retail_button_only_auto_choose();
    }

    /// C++ `AutoChooseSources = SECONDARY NONE` (Jarmen snipe / Missile Defender
    /// laser / Toxin Tractor sprayer). Stamp only while the slot is still the
    /// WeaponTemplateSet::clear default so authored INI bits win.
    pub fn apply_retail_button_only_auto_choose(&mut self) {
        if self.auto_choose_masks.get(1).copied() != Some(u32::MAX) {
            return;
        }
        if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(&self.name)
            || crate::game_logic::host_missile_defender::is_missile_defender_template(&self.name)
            || crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(&self.name)
        {
            self.auto_choose_masks[1] = 0;
        }
    }

    /// C++ WeaponSet.cpp:869-877 — victim matches this slot's PreferredAgainst.

    /// C++ WeaponSet.cpp:816-822 — AutoChooseSources includes FROM_PLAYER /
    /// FROM_AI / DEFAULT_SWITCH_WEAPON. NONE (mask 0) is button-only.
    pub fn slot_allows_auto_choose(&self, slot: u8) -> bool {
        if slot == 0 && self.primary_auto_choose_none {
            return false;
        }
        let mask = self
            .auto_choose_masks
            .get(slot as usize)
            .copied()
            .unwrap_or(u32::MAX);
        // Missing INI still must not auto-pick retail button-only secondaries.
        if slot == 1
            && mask == u32::MAX
            && (crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(&self.name)
                || crate::game_logic::host_missile_defender::is_missile_defender_template(
                    &self.name,
                )
                || crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(&self.name))
        {
            return false;
        }
        const COMBAT: u32 = (1 << 0) | (1 << 2) | (1 << 4);
        (mask & COMBAT) != 0
    }
    pub fn slot_preferred_against(&self, slot: u8, target_kinds: impl Fn(KindOf) -> bool) -> bool {
        let Some(kinds) = self.preferred_against.get(slot as usize) else {
            return false;
        };
        !kinds.is_empty() && kinds.iter().copied().any(target_kinds)
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kind_of.contains(&kind)
    }

    /// C++ Object ctor: `m_shroudClearingRange == -1` → `m_visionRange`.
    pub fn resolved_shroud_clearing_range(&self) -> f32 {
        if self.shroud_clearing_range < 0.0 {
            self.sight_range
        } else {
            self.shroud_clearing_range
        }
    }

    pub fn add_kind_of(&mut self, kind: KindOf) -> &mut Self {
        self.kind_of.insert(kind);
        self
    }

    pub fn set_health(&mut self, health: f32) -> &mut Self {
        self.max_health = health;
        self
    }

    /// Author a `RebuildHoleExposeDie` HoleName / HoleMaxHealth pair.
    pub fn set_rebuild_hole_expose(&mut self, hole_name: &str, hole_max_health: f32) -> &mut Self {
        self.rebuild_hole_expose = Some(RebuildHoleExposeDieMetadata::authored(
            hole_name,
            hole_max_health,
        ));
        self
    }

    pub fn set_cost(&mut self, supplies: u32, power: i32) -> &mut Self {
        self.build_cost = Resources { supplies, power };
        self
    }

    /// C++ `ThingTemplate::getThreatValue`. Leftover factory wins when loaded.
    pub fn get_threat_value(&self) -> u16 {
        leftover_template_threat_value(&self.name).unwrap_or(self.threat_value)
    }

    /// C++ ControlBarCommand.cpp:1119-1121 / 1175-1177 — hide for humans.
    /// `ThingTemplate::getBuildable` consults GameLogic override first.
    pub fn human_control_bar_buildable_hidden(&self) -> bool {
        let status = gamelogic::helpers::TheGameLogic::find_buildable_status_override(&self.name)
            .map(|s| s.max(0) as u32)
            .unwrap_or(self.buildable_status);
        !crate::game_logic::host_production_buildable_command_residual::buildable_status_allows_human_residual(
            status,
        )
    }

    /// C++ `ThingTemplate::parsePrerequisites` via leftover parse_prerequisites_block.
    pub fn parse_prerequisites_from_ini_lines(&mut self, lines: &[String]) {
        let mut scratch = game_engine::common::thing::thing_template::ThingTemplate::new();
        scratch.parse_prerequisites_block(lines);
        self.set_production_prerequisites(scratch.get_prereqs().to_vec());
    }

    /// Copy leftover-factory `m_prereqInfo` (already parsed + resolveNames).
    pub fn set_production_prerequisites(
        &mut self,
        prereqs: Vec<game_engine::common::rts::ProductionPrerequisite>,
    ) {
        self.prerequisite_definitions =
            super::build_prerequisites::PrerequisiteDefinitions::admit(&prereqs);
        self.production_prerequisites = prereqs;
    }

    pub fn set_model(&mut self, model: &str) -> &mut Self {
        self.model_name = Some(model.to_string());
        self
    }

    /// Get the model name for this template, or fall back to template name
    pub fn get_model_name(&self) -> &str {
        self.model_name.as_deref().unwrap_or(&self.name)
    }

    /// Get the W3D model filename (with .w3d extension if needed)
    pub fn get_w3d_filename(&self) -> String {
        let model_name = self.get_model_name();
        if model_name.to_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{}.w3d", model_name)
        }
    }
}
