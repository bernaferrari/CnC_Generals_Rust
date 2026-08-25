//! Mechanical split from `game_logic/game_logic.rs`. No behavior change.
#![allow(non_snake_case, unused_imports, dead_code)]
use super::HostHeliTakeoffOrLanding;
use super::authority::*;
use super::construct::*;
use super::crate_tick::*;
use super::player::*;
use super::prelude::*;
use super::script_camera::*;
use super::*;

pub struct GameLogic {
    /// Named AttackPriorityInfo residual map (script sets).
    pub attack_priority_sets: std::collections::HashMap<String, AttackPriorityInfo>,
    /// C++ `Team::m_commonAttackTarget` residual, keyed by team instance name.
    pub team_common_attack_targets: std::collections::HashMap<String, ObjectId>,
    /// C++ AIGuardIdleState::m_nextEnemyScanTime residual.
    pub(crate) guard_next_enemy_scan: HashMap<ObjectId, u32>,
    /// C++ AIHuntState::m_nextEnemyScanTime residual.
    pub(crate) hunt_next_enemy_scan: HashMap<ObjectId, u32>,
    /// C++ AIGuardIdleState::m_guardeePos residual.
    pub(crate) guard_guardee_pos: HashMap<ObjectId, glam::Vec3>,

    /// C++ TAiData::m_enableRepulsors residual (AI.ini EnableRepulsors).
    pub enable_repulsors: bool,
    /// C++ TAiData::m_retaliateFriendsRadius residual (default 120).
    pub retaliate_friends_radius: f32,
    /// C++ TAiData::m_maxRetaliateDistance residual (default 210).
    pub max_retaliate_distance: f32,
    /// Objects in the world.
    ///
    /// Own field (not a method on `&mut GameLogic`) so ticks can
    /// `self.objects.get_mut` while still reading `self.frame`.
    /// When a GameWorld shadow session is coupled this map is an ID roster /
    /// read-view. HP / pose / attack-target live in GameWorld;
    /// [`Self::host_object_mut`] overlays those fields from GameWorld.
    /// Coupled ticks do not dirty-push HashMap mutations back. Fail-open host
    /// fields only when shadow is off. Main still allocates ObjectId.
    pub objects: HostObjectStore,
    /// Host ids mutated this tick that must write through to GameWorld.
    pub(super) host_view_dirty: HashSet<ObjectId>,
    /// C++ Object partition last-look: unlook previous then look on move/death.
    /// (x, y, z, radius, player_mask) in shroud Coord3D space.
    pub(super) vision_last_looks: HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    /// C++ `m_partitionRevealAllLastLook` (ShroudRevealToAllRange).
    pub(super) vision_last_reveal_all: HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    /// C++ `m_partitionLastShroud` (Object::shroud / doShroudCover).
    pub(super) vision_last_shroud: HashMap<ObjectId, (f32, f32, f32, f32, u32)>,

    /// Players in the game. Coupled shadow: supplies/power/sciences last-write
    /// from GameWorld `PlayerData` (economy writeback). This map is a read-view
    /// plus residual UI/selection fields.
    pub(super) players: HashMap<u32, Player>,
    /// Exact Campaign/Challenge PlayerTemplate binding for each host player.
    ///
    /// This is deliberately GameLogic session state rather than a `Player`
    /// field until the shared save schema owns its Xfer representation.  It is
    /// reset with a match, survives the existing SP/skirmish map-load preserve
    /// path, and never mutates GameNetwork.
    pub(super) player_template_bindings: HashMap<u32, PlayerTemplateIdentity>,

    /// Object ID counter
    pub(super) next_object_id: ObjectId,
    /// C++ TheAI next formation id residual (starts at 1; 0 = none).
    pub(super) next_formation_id: u32,

    /// Simulation frame counter
    pub(crate) frame: u32,

    /// Next unused sequence for an actual accepted WeaponSet discharge.
    ///
    /// This is logical-world state, not renderer cadence. Sequence zero stays
    /// reserved for an Object with no accepted discharge marker.
    pub(super) next_weapon_discharge_sequence: u64,
    /// Host-only, presentation-facing accepted-discharge queue. It is not
    /// serialized: the durable Object marker + counter establish restore
    /// baselines, while pre-save transient cues must not replay after load.
    pub(super) weapon_discharge_log:
        crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeLog,
    /// Runtime-only world identity so recycled ObjectIds cannot inherit a stale plan.
    pub(super) visual_world_epoch: u64,
    pub(super) next_visual_object_generation: u64,

    /// Game mode
    pub(super) game_mode: GameMode,

    /// Active skirmish/match rules (from skirmish UI config).
    pub(super) skirmish_rules: SkirmishRulesState,

    /// Game world dimensions
    pub(super) world_width: f32,
    pub(super) world_height: f32,
    pub(super) world_min: Vec3,
    pub(super) world_max: Vec3,

    /// Victory conditions subsystem (mirrors SAGE VictoryConditions)
    pub(super) victory_conditions: VictoryConditions,

    /// Objects to destroy at end of frame
    pub(super) objects_to_destroy: VecDeque<DestructionEvent>,

    /// Host combat particle registry (kill/fire → observably registered systems).
    /// Residual hq-gq7n: not full W3D GPU parity; PresentationFrame can observe entries.
    pub(super) combat_particles: CombatParticleRegistry,

    /// Host superweapon strike residual (DaisyCutter / A10 / ScudStorm / ParticleCannon /
    /// NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb / ArtilleryBarrage /
    /// CruiseMissile). Queues on DoSpecialPower and completes with area damage —
    /// NuclearMissile also spawns residual radiation; AnthraxBomb also spawns residual
    /// toxin; SpectreGunship spawns residual orbit damage ticks; CarpetBomb applies
    /// delayed multi-point line damage; ArtilleryBarrage applies delayed multi-shell
    /// scatter damage; CruiseMissile applies delayed loft + MOAB area damage.
    /// Fail-closed vs full retail.
    pub(crate) special_power_strikes:
        crate::game_logic::special_power_strikes::HostSpecialPowerStrikeRegistry,

    /// Host America Paradrop / Airborne residual.
    /// Queues on DoSpecialPower and spawns infantry after approach delay — fail-closed vs full OCL plane.
    pub(crate) host_paradrops: crate::game_logic::host_paradrop::HostParadropRegistry,

    /// Host GLA Rebel Ambush residual.
    /// C++ OCLSpecialPower::doSpecialPowerAtLocation creates on the fire frame.
    /// Science UpgradeOCL selects Ambush1/2/3 and Chem/Demo/Slth rebel templates.
    pub(super) host_ambushes: crate::game_logic::host_ambush::HostAmbushRegistry,
    /// Residual: last SuperweaponCashHack requested science-tier amount.
    pub(super) last_cash_hack_request_amount: u32,
    /// Residual: last SuperweaponCashHack stolen amount.
    pub(super) last_cash_hack_stolen_amount: u32,
    /// Residual: last SuperweaponCrateDrop spawned crate count.
    pub(super) last_crate_drop_spawned: u32,

    /// Host USA Leaflet Drop residual.
    /// Queues on DoSpecialPower; after Delay disables enemy infantry/vehicles
    /// (DISABLED_EMP residual) — fail-closed vs full OCL B52 / LeafletContainer path.
    pub(crate) host_leaflet_drops: crate::game_logic::host_leaflet_drop::HostLeafletDropRegistry,

    /// Host GLA Sneak Attack residual.
    /// Queues on DoSpecialPower; after Lifetime delay spawns tunnel structure +
    /// residual shockwave damage — fail-closed vs full OCL Start animation / TunnelContain.
    pub(crate) host_sneak_attacks: crate::game_logic::host_sneak_attack::HostSneakAttackRegistry,

    /// Host upgrade queue/complete residual (Capture / FlashBang / TOW / SupplyLines).
    /// Completes research into unlocked_sciences and applies observable unit unlocks.
    pub(super) host_upgrades: crate::game_logic::host_upgrades::HostUpgradeRegistry,

    /// Supply Lines economy residual: total bonus cash credited on drop-off deposits.
    /// Matches C++ SupplyCenterDockUpdate + Chinook `getUpgradedSupplyBoost` path.
    /// Fail-closed: not per-template INI boost matrix / WorkerShoes / multiplayer.
    pub(super) supply_lines_bonus_cash_total: u32,

    /// Host cash bounty residual (GLA SCIENCE_CashBounty → kill awards cash).
    /// Fail-closed: not full CashBountyPower palace module / floating text.
    pub(super) cash_bounty: crate::game_logic::host_cash_bounty::HostCashBountyRegistry,

    /// Host garrison residual honesty counters (enter / exit / fire-from-garrison).
    /// Fail-closed: not C++ GarrisonContain fire-point bones or full weapon matrix.
    pub(super) garrison_residual_enters: u32,
    pub(super) garrison_residual_exits: u32,
    pub(super) garrison_residual_fires: u32,

    /// Host transport residual honesty counters (load / unload-all / evacuate).
    /// Fail-closed: not multi-door or Chinook air-transport path parity.
    pub(super) transport_residual_loads: u32,
    pub(super) transport_residual_unloads: u32,

    /// Host China Overlord BattleBunker residual honesty counters (enter / exit).
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn.
    pub(super) overlord_bunker_residual_enters: u32,
    pub(super) overlord_bunker_residual_exits: u32,

    /// Host GLA Battle Bus residual honesty counters
    /// (load / unload / passenger fire / armed-riders weapon-set).
    /// Fail-closed: not SlowDeath undeath SECOND_LIFE / multi-door exit matrix.
    pub(super) battle_bus: crate::game_logic::host_battle_bus::HostBattleBusRegistry,
    /// C++ HighlanderBody residual clamps.
    pub(super) highlander_body_reg:
        crate::game_logic::host_highlander_body::HostHighlanderBodyRegistry,
    /// C++ DeployStyleAIUpdate residual counters.
    pub(super) deploy_style_reg: crate::game_logic::host_deploy_style::HostDeployStyleRegistry,
    /// C++ TensileFormationUpdate residual counters.
    pub(super) tensile_formation_reg:
        crate::game_logic::host_tensile_formation::HostTensileFormationRegistry,
    /// C++ StatusBitsUpgrade residual counters.
    pub(super) status_bits_upgrade_reg:
        crate::game_logic::host_status_bits_upgrade::HostStatusBitsUpgradeRegistry,
    /// C++ FireSpreadUpdate residual counters.
    pub(super) fire_spread_reg: crate::game_logic::host_fire_spread::HostFireSpreadRegistry,
    /// C++ BaseRegenerateUpdate residual counters.
    pub(super) base_regenerate_reg:
        crate::game_logic::host_base_regenerate::HostBaseRegenerateRegistry,
    /// C++ EnemyNearUpdate residual counters.
    pub(super) enemy_near_reg: crate::game_logic::host_enemy_near::HostEnemyNearRegistry,
    /// C++ PassengersFireUpgrade residual counters.
    pub(super) passengers_fire_upgrade_reg:
        crate::game_logic::host_passengers_fire_upgrade::HostPassengersFireUpgradeRegistry,
    /// C++ AnimationSteeringUpdate residual counters.
    pub(super) animation_steering_reg:
        crate::game_logic::host_animation_steering::HostAnimationSteeringRegistry,
    /// C++ ActiveShroudUpgrade residual counters.
    pub(super) active_shroud_upgrade_reg:
        crate::game_logic::host_active_shroud_upgrade::HostActiveShroudUpgradeRegistry,
    /// C++ FloatUpdate residual counters.
    pub(super) float_update_reg: crate::game_logic::host_float_update::HostFloatUpdateRegistry,
    /// C++ ProneUpdate residual counters.
    pub(super) prone_update_reg: crate::game_logic::host_prone_update::HostProneUpdateRegistry,
    /// C++ RadiusDecalUpdate residual counters.
    pub(super) radius_decal_update_reg:
        crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateRegistry,
    /// C++ CheckpointUpdate residual counters.
    pub(super) checkpoint_update_reg:
        crate::game_logic::host_checkpoint_update::HostCheckpointUpdateRegistry,
    /// C++ SpectreGunshipDeploymentUpdate residual counters.
    pub(super) spectre_gunship_deployment_reg:
        crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentRegistry,
    /// C++ SmartBombTargetHomingUpdate residual counters.
    pub(super) smart_bomb_target_homing_reg:
        crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingRegistry,
    /// C++ OCLSpecialPower residual counters.
    pub(super) ocl_special_power_reg:
        crate::game_logic::host_ocl_special_power::HostOclSpecialPowerRegistry,
    /// C++ ObjectCreationList CreateDebris disposition residual.
    pub(super) ocl_create_debris_reg:
        crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisRegistry,
    /// C++ OCL FireWeaponNugget + AttackNugget residual.
    pub(super) ocl_fire_weapon_attack_reg:
        crate::game_logic::host_ocl_fire_weapon_attack::HostOclFireWeaponAttackRegistry,
    /// C++ FuelAir gas SlowDeathBehavior residual.
    pub(super) fuel_air_gas_reg:
        crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasRegistry,
    /// C++ OCL ApplyRandomForceNugget residual.
    pub(super) ocl_apply_random_force_reg:
        crate::game_logic::host_ocl_apply_random_force::HostOclApplyRandomForceRegistry,
    /// C++ NeutronMissileUpdate residual counters.
    pub(super) neutron_missile_update_reg:
        crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateRegistry,
    /// C++ ScudStormMissile ballistic flight residual counters.
    pub(super) scud_storm_missile_flight_reg:
        crate::game_logic::host_scud_storm_missile_flight::HostScudStormMissileFlightRegistry,
    /// C++ CarpetBomb DeliverPayload residual counters.
    pub(crate) carpet_bomb_flight_reg:
        crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightRegistry,
    /// C++ ArtilleryBarrage DeliverPayload residual counters.
    pub(crate) artillery_barrage_flight_reg:
        crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightRegistry,
    /// C++ A10Thunderbolt DeliverPayload residual counters.
    pub(crate) a10_strike_flight_reg:
        crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightRegistry,
    /// C++ DaisyCutter DeliverPayload residual counters.
    pub(crate) daisy_cutter_flight_reg:
        crate::game_logic::host_daisy_cutter_flight::HostDaisyCutterFlightRegistry,
    /// C++ AnthraxBomb DeliverPayload residual counters.
    pub(crate) anthrax_bomb_flight_reg:
        crate::game_logic::host_anthrax_bomb_flight::HostAnthraxBombFlightRegistry,
    /// C++ ClusterMines DeliverPayload residual counters.
    pub(crate) cluster_mines_flight_reg:
        crate::game_logic::host_cluster_mines_flight::HostClusterMinesFlightRegistry,
    /// C++ EMPPulse DeliverPayload residual counters.
    pub(crate) emp_pulse_flight_reg:
        crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightRegistry,
    /// C++ CommandButtonHuntUpdate residual counters.
    pub(super) command_button_hunt_reg:
        crate::game_logic::host_command_button_hunt::HostCommandButtonHuntRegistry,
    /// C++ PreorderCreate residual counters.
    pub(super) preorder_create_reg:
        crate::game_logic::host_preorder_create::HostPreorderCreateRegistry,
    /// C++ UpgradeDie residual removals.
    pub(super) upgrade_die_reg: crate::game_logic::host_upgrade_die::HostUpgradeDieRegistry,

    /// Host GLA Tunnel Network residual (TunnelContain shared MaxTunnelCapacity=10).
    /// Enter any allied tunnel; exit/evacuate at any allied tunnel (cross-tunnel).
    /// Last-tunnel cave-in, heal, and GuardTunnelNetwork nemesis are live.
    pub(crate) tunnel_network: crate::game_logic::host_tunnel_network::HostTunnelNetworkRegistry,

    /// C++ CaveSystem + CaveContain shared CaveIndex tracker.
    pub(super) cave_system: crate::game_logic::host_cave_system::HostCaveSystem,

    /// C++ BridgeBehavior scaffolding + rubble restamp/splat.
    pub(crate) bridge_behavior: crate::game_logic::host_bridge_behavior::HostBridgeBehaviorRegistry,

    /// Host AirF Combat Chinook residual honesty counters
    /// (load / unload / passenger fire / armed-riders weapon-set).
    /// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
    pub(super) combat_chinook: crate::game_logic::host_combat_chinook::HostCombatChinookRegistry,

    /// Host China Listening Outpost residual honesty counters
    /// (detect / load / unload / passenger fire / armed-riders / InitialPayload).
    /// Fail-closed: not IR FX / multi-door exit / RIDERS_ATTACKING uncloak matrix.
    pub(super) listening_outpost:
        crate::game_logic::host_listening_outpost::HostListeningOutpostRegistry,

    /// Host China Troop Crawler residual honesty counters
    /// (load / unload / initial payload / assault deploy / detect).
    /// Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve matrix.
    pub(super) troop_crawler: crate::game_logic::host_troop_crawler::HostTroopCrawlerRegistry,

    /// Host mine / demo-trap / timed demo-charge residual honesty counters.
    /// Fail-closed: not full MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate.
    pub(super) mine_residual_places: u32,
    pub(super) mine_residual_proximity_detonations: u32,
    pub(super) mine_residual_timed_detonations: u32,
    pub(super) mine_residual_manual_detonations: u32,
    /// Dozer/Worker safe mine-clear residual (DAMAGE_DISARM destroy without detonation).
    pub(super) mine_residual_clears: u32,

    /// Host structure/vehicle repair residual honesty counters.
    /// Fail-closed: not full DozerAIUpdate percent heal / RepairDockUpdate TimeForFullHeal.
    /// structure: dozer Repair command accepted / structure HP heal ticks applied.
    /// vehicle: SeekingRepair heal ticks at RepairPad / WarFactory / Airfield.
    pub(super) repair_residual_structure_commands: u32,
    pub(super) repair_residual_structure_heals: u32,
    pub(super) repair_residual_vehicle_heals: u32,

    /// Host infantry heal residual honesty counters.
    /// Fail-closed: not full AutoHealBehavior sole-benefactor / vehicle radius matrix.
    /// ambulance: radius AutoHeal infantry HP ticks (AmericaVehicleMedic residual).
    /// heal_pad: SeekingHealing HP ticks at HealPad.
    pub(super) heal_residual_ambulance_heals: u32,
    pub(super) ambulance_auto_heal_pulse_accum: f32,
    pub(super) heal_residual_heal_pad_heals: u32,

    /// Host China Propaganda / Speaker Tower residual honesty counters.
    /// Fail-closed: not full PropagandaTowerBehavior sole-benefactor / upgrade FX matrix.
    /// heals: radius %max-health heal ticks applied to same-team non-structures.
    /// buffs: ENTHUSIASTIC / SUBLIMINAL weapon-bonus flag grants.
    pub(super) propaganda_residual_heals: u32,
    pub(super) propaganda_residual_buffs: u32,
    pub(super) propaganda_scan: crate::game_logic::host_propaganda::HostPropagandaScanState,

    /// Host China ECM Tank / jammer residual honesty counters.
    /// jams: DISABLED_SUBDUED full halt + fire-only weapons_jammed grants
    /// applied to enemy/neutral vehicles after isSubdued().
    pub(super) ecm_residual_jams: u32,

    /// Host America Microwave Tank residual (DISABLED_SUBDUED on structures).
    /// Fail-closed: not full subdual accumulate/heal / laser stream / emitter field.
    pub(super) microwaves: crate::game_logic::host_microwave::HostMicrowaveRegistry,
    /// C++ ParkingPlaceBehavior `m_spaces`: exact per-airfield reservation
    /// records, sized from authored NumRows × NumCols rather than a generic
    /// container roster or retail-name capacity.
    pub(super) airfield_parking_spaces:
        std::collections::HashMap<ObjectId, Vec<AirfieldParkingSpace>>,
    /// C++ ParkingPlaceBehavior runway in-use residual (airfield → runway slots → jet).
    pub(super) runway_reservations: std::collections::HashMap<ObjectId, Vec<Option<ObjectId>>>,
    /// C++ `RunwayInfo::m_nextInLineForTakeoff` (same length as `runway_reservations`).
    pub(super) airfield_runway_next_in_line:
        std::collections::HashMap<ObjectId, Vec<Option<ObjectId>>>,
    /// C++ `RunwayInfo::m_wasInLine` (same length as `runway_reservations`).
    pub(super) airfield_runway_was_in_line: std::collections::HashMap<ObjectId, Vec<bool>>,
    /// Produced-at-helipad exits waiting for the next airfield tick (`HeliPark01` + rally).
    pub(super) airfield_pending_helipad_exits: std::collections::HashMap<ObjectId, ObjectId>,
    /// C++ `HeliTakeoffOrLandingState` two-point climb/descent (NeedsRunway=No).
    pub(super) heli_takeoff_or_landing:
        std::collections::HashMap<ObjectId, HostHeliTakeoffOrLanding>,
    /// C++ `ParkingPlaceBehavior::m_healing` — list can exceed stall count.
    pub(super) airfield_healing: std::collections::HashMap<ObjectId, Vec<AirfieldHealingInfo>>,
    /// C++ `ParkingPlaceBehavior::m_nextHealFrame`.
    pub(super) airfield_next_heal_frame: std::collections::HashMap<ObjectId, u32>,

    /// C++ FlightDeckBehavior runtime stalls / launch-wave / designated orders.
    pub(crate) flight_decks: std::collections::HashMap<
        ObjectId,
        crate::game_logic::host_flight_deck::HostFlightDeckState,
    >,

    /// Host China EMP Pulse residual (DISABLED_EMP on vehicles/structures).
    /// Fail-closed: not full OCL EMPPulseBomb / EMPPulseEffectSpheroid drawable path.
    pub(super) emp_pulses: crate::game_logic::host_emp_pulse::HostEmpPulseRegistry,
    /// Host BaikonurLaunchPower residual (door open + detonation multi-blast).
    pub(super) baikonur_launches:
        crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry,
    /// Host DefectorSpecialPower residual.
    pub(super) defector_special:
        crate::game_logic::host_defector_special_power::HostDefectorSpecialPowerRegistry,
    /// Host CostModifier/Unpause/WeaponBonus upgrade module residuals.
    pub(super) upgrade_module_residuals:
        crate::game_logic::host_upgrade_module_residuals::HostUpgradeModuleResidualLog,
    /// Host ReplaceObject / GrantScience / CommandSet upgrade residuals.
    pub(super) replace_grant_command_upgrades:
        crate::game_logic::host_replace_object_upgrade::HostReplaceGrantCommandUpgradeLog,
    /// Host SubObjectsUpgrade residual log.
    pub(super) sub_objects_upgrades:
        crate::game_logic::host_sub_objects_upgrade::HostSubObjectsUpgradeLog,

    /// Host China Frenzy ("Rage") residual — temporary ally attack buff in radius.
    /// Frenzy_InvisibleMarker + DeletionUpdate residual closed; fail-closed vs FrenzyCloud GPU.
    pub(super) frenzies: crate::game_logic::host_frenzy::HostFrenzyRegistry,

    /// Host USA Strategy Center battle-plan residual (Bombardment / HoldTheLine / S&D).
    /// Fail-closed: not full BattlePlanUpdate pack/unpack / paralyze / turret matrix.
    pub(super) battle_plans: crate::game_logic::host_strategy_center::HostBattlePlanRegistry,
    /// Honesty: StrategyCenterGun ScatterRadius peels applied.
    pub(super) strategy_center_gun_scatter_applied: u32,
    /// Honesty: StrategyCenterGun scatter residual misses.
    pub(super) strategy_center_gun_scatter_misses: u32,

    /// Host Emergency Repair residual — SingleBurst ally vehicle heal in radius.
    /// Fail-closed: not full OCL RepairVehicles invisible marker / RepairCloud path.
    pub(super) emergency_repairs:
        crate::game_logic::host_emergency_repair::HostEmergencyRepairRegistry,

    /// Host Cleanup Area residual — clear toxin/radiation fields + mines at location.
    /// Fail-closed: not full CleanupHazardUpdate projectile stream / scan loop.
    pub(crate) cleanup_areas: crate::game_logic::host_cleanup_area::HostCleanupAreaRegistry,

    /// Host GLA GPS Scrambler residual — GrantStealth ally vehicles/infantry in radius.
    /// Fail-closed: not full OCL GPSScrambler_InvisibleMarker grow-radius pulse path.
    pub(crate) gps_scramblers: crate::game_logic::host_gps_scrambler::HostGpsScramblerRegistry,

    /// Host base-defense residual honesty (Patriot / Gattling auto-fire).
    /// Fail-closed: not full AutoAcquire / WeaponSet / continuous-fire matrix.
    pub(super) base_defense_residual_fires: u32,

    /// Host PointDefenseLaser residual honesty (Paladin / Avenger intercept).
    /// Fail-closed: not full PointDefenseLaserUpdate velocity prediction matrix.
    pub(super) point_defense_residual_intercepts: u32,
    /// Honesty: ECMTankMissileJammer missiles jammed/scattered residual.
    pub(super) ecm_missiles_jammed: u32,
    /// Honesty: ECMDisableStream laser beams spawned residual.
    pub(super) ecm_laser_beams_spawned: u32,
    /// Honesty: PointDefenseLaserBeam objects spawned on intercept residual.
    pub(super) point_defense_laser_beams_spawned: u32,
    /// Per-carrier next ready frame for leftover PDL module 0 (DelayBetweenShots).
    pub(crate) point_defense_next_ready_frame: HashMap<ObjectId, u32>,
    /// Avenger leftover PointDefenseLaserUpdate module 1 fire clock (independent
    /// 500ms stream). Paladin / Chinook / King Raptor residual do not use this.
    pub(crate) point_defense_next_ready_frame_1: HashMap<ObjectId, u32>,

    /// Host America Avenger residual honesty (FAERIE_FIRE paint / air laser / ROF).
    /// Fail-closed: not full portable laser turret / dual AirLaser stream matrix.
    pub(super) avenger: crate::game_logic::host_avenger::HostAvengerRegistry,

    /// Host Neutron Shell residual honesty (Nuke Cannon secondary blast).
    /// Fail-closed: not full DumbProjectileBehavior / NeutronBlastBehavior modules.
    pub(super) neutron_shell_residual_blasts: u32,
    pub(super) neutron_shell_residual_infantry_kills: u32,
    pub(super) neutron_shell_residual_vehicles_unmanned: u32,

    /// Host Bunker Buster residual (Stealth Fighter + Upgrade_AmericaBunkerBusters).
    /// DetonationFX on the bunker + CrashThroughBunkerFX while MISSILE_KILLING_SELF.
    /// Fail-closed: not full seismic sim path.
    pub(super) bunker_buster: crate::game_logic::host_bunker_buster::HostBunkerBusterRegistry,

    /// Host Comanche Rocket Pods residual honesty (area attack when secondary fires).
    /// ScatterTarget projectile residual closed; tertiary WeaponSet fail-closed.
    pub(super) comanche_rocket_pod_residual_area_attacks: u32,
    pub(super) comanche_rocket_pod_residual_units_hit: u32,
    pub(super) comanche_rocket_pod_shot_index: std::collections::HashMap<ObjectId, u32>,
    pub(super) comanche_rocket_pod_projectiles_spawned: u32,

    /// Host Sentry Drone residual honesty (auto-detect spawn + gun auto-fire).
    /// Fail-closed: not full DeployStyleAIUpdate pack/unpack matrix.
    pub(super) sentry_drone_residual_auto_fires: u32,
    pub(super) sentry_drone_residual_detects: u32,

    /// Host Pathfinder residual honesty (innate stealth detector + sniper).
    /// Fail-closed: not full StealthUpdate pulse / SCIENCE_Pathfinder gate.
    pub(super) pathfinder_residual_detects: u32,
    pub(super) pathfinder_residual_sniper_fires: u32,

    /// Host Scout / Hellfire slave-drone residual honesty.
    /// SlavedUpdate follow/guard/2x GuardMaxRange recall is ticked in update_ai.
    pub(super) scout_drone_residual_detects: u32,
    pub(super) scout_drone_residual_attaches: u32,
    pub(super) hellfire_drone_residual_auto_fires: u32,
    pub(super) hellfire_drone_residual_attaches: u32,
    /// Honesty: Hellfire ScatterRadiusVsInfantry peels applied.
    pub(super) hellfire_scatter_applied: u32,
    /// Honesty: Hellfire ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) hellfire_scatter_misses: u32,

    /// Host RadarScan / RadarVanScan FOW temporary-reveal residual.
    /// RadarVanPing object residual closed; fail-closed vs grid decal GPU path.
    pub(crate) radar_scans: crate::game_logic::host_radar_scan::HostRadarScanRegistry,

    /// Host SpySatellite FOW temporary-reveal residual.
    /// SpySatellitePing object residual closed; fail-closed vs GridDecal GPU path.
    pub(crate) spy_drones: crate::game_logic::host_spy_drone::HostSpyDroneRegistry,
    pub(crate) spy_satellites: crate::game_logic::host_spy_satellite::HostSpySatelliteRegistry,
    /// Host America Countermeasures residual (aircraft flare diversion).
    /// CountermeasureFlare SpecialObject spawn residual closed.
    pub(crate) countermeasures:
        crate::game_logic::host_countermeasures::HostCountermeasuresRegistry,

    /// Host CIA Intelligence / SpyVision residual (setUnitsVisionSpied).
    /// Fail-closed: not full SpyVisionUpdate module / kindof filter / sabotage path.
    pub(super) cia_intelligence:
        crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,

    /// Host hero special-ability residual (snipe / timed C4 / cash hack).
    /// Fail-closed: not full SpecialAbilityUpdate preparation / flee / upgrade matrix.
    pub(super) hero_abilities: crate::game_logic::host_hero_abilities::HostHeroAbilityRegistry,

    /// Host GLA Black Market residual cash (AutoDepositUpdate residual).
    /// Fail-closed: not full floating text / InitialCaptureBonus / upgrade boost matrix.
    pub(crate) black_markets: crate::game_logic::host_black_market::HostBlackMarketRegistry,

    /// Host Tech Oil Derrick residual cash (AutoDepositUpdate residual).
    /// AutoDeposit residual (SupplyLines boost + floating text host residual closed).
    pub(crate) oil_derricks: crate::game_logic::host_oil_derrick::HostOilDerrickRegistry,

    /// Host China Hacker / Internet Center residual cash (HackInternetAIUpdate).
    /// Fail-closed: not full unpack/pack state machine / variation / floating text.
    pub(crate) hacker_income: crate::game_logic::host_hacker_income::HostHackerIncomeRegistry,

    /// Host America Supply Drop Zone residual cash (OCLUpdate residual).
    /// Fail-closed: not full CreateAtEdge cargo plane / parachute crate fall path
    /// (delayed DeliverPayload spawn residual via host_deliver_payloads).
    pub(crate) supply_drop_zones:
        crate::game_logic::host_supply_drop_zone::HostSupplyDropZoneRegistry,

    /// Host DeliverPayload cargo residual (delayed payload spawn at location).
    /// Fail-closed: not full AmericaJetCargoPlane Object / DeliverPayloadAIUpdate
    /// flight state machine / parachute container physics (DropDelay stagger +
    /// DropOffset / MaxAttempts / PreOpenDistance constants residual closed).
    pub(super) host_deliver_payloads:
        crate::game_logic::host_deliver_payload::HostDeliverPayloadRegistry,

    /// Host MoneyCrateCollide residual (unit + BuildingPickup).
    /// Fail-closed: not full CollideModule partition pair / Anim2D MoneyPickUp.
    pub(crate) host_money_crates: crate::game_logic::host_money_crate::HostMoneyCrateRegistry,

    /// Host CommandCenter / RadarVan radar-online residual (Player::hasRadar).
    /// Fail-closed: not full RadarUpgrade/RadarUpdate grant matrix / power-disable proof.
    pub(crate) host_radar: crate::game_logic::host_radar::HostRadarRegistry,

    /// Host GLA Hijack / ConvertToCarBomb residual.
    /// Fail-closed: not full HijackerUpdate hide-in-vehicle / WeaponSet chooser matrix.
    pub(super) car_bomb: crate::game_logic::host_car_bomb::HostCarBombRegistry,
    /// Host GLA Saboteur structure sabotage residual (power/cash/factory/superweapon).
    /// Fail-closed: not full BuildingPickup CrateCollide / EVA floating-text matrix.
    pub(super) saboteur: crate::game_logic::host_saboteur::HostSaboteurRegistry,
    /// USA Pilot recrew residual honesty.
    pub(super) usa_pilot: crate::game_logic::host_usa_pilot::HostUsaPilotRegistry,
    /// GLA Worker / WorkerShoes residual honesty.
    pub(super) gla_worker: crate::game_logic::host_gla_worker::HostGlaWorkerRegistry,

    /// Host GLA Bomb Truck disguise residual (SpecialAbilityDisguiseAsVehicle).
    /// Fail-closed: not full StealthUpdate transition opacity / model swap matrix.
    pub(super) bomb_truck_disguise:
        crate::game_logic::host_bomb_truck_disguise::HostBombTruckDisguiseRegistry,

    /// Host GLA Bomb Truck HE/Bio FireWeaponWhenDead detonation residual.
    /// Fail-closed: not full exclusive FireWeaponWhenDead module / SubObjects matrix.
    pub(super) bomb_truck_detonate:
        crate::game_logic::host_bomb_truck_detonate::HostBombTruckDetonateRegistry,
    /// Host China Nuclear Tanks residual (death blast + speed + radiation).
    /// Fail-closed: not full FireWeaponWhenDead exclusive / locomotor visual matrix.
    pub(super) nuclear_tanks: crate::game_logic::host_nuclear_tanks::HostNuclearTanksRegistry,
    /// Host GLA Rebel BoobyTrap residual (plant + capture/death detonate).
    /// Fail-closed: not full StickyBombUpdate SpecialObject / MaxSpecialObjects matrix.
    pub(super) booby_trap: crate::game_logic::host_booby_trap::HostBoobyTrapRegistry,
    /// Honesty: BoobyTrap SpecialObject Things spawned.
    pub(super) booby_trap_objects_spawned: u32,

    /// Host China Helix NapalmBomb special ability residual (blast + FirestormSmall).
    /// Fail-closed: not full SpecialObject NapalmBomb fall / expand animation.
    pub(crate) helix_napalm: crate::game_logic::host_helix_napalm::HostHelixNapalmRegistry,

    /// Host China FireWall / Firestorm residual (Dragon Tank line of fire zones).
    /// FireWallSegment OCL spawn + InchForwardLocomotor crawl residual closed.
    pub(crate) fire_walls: crate::game_logic::host_firewall::HostFireWallRegistry,

    /// Host China Inferno Cannon residual fire zones (FireFieldSmall DoT).
    /// Fail-closed: not full InfernoTankShell projectile / OCL_FireFieldSmall object spawn.
    pub(crate) inferno_fire_zones:
        crate::game_logic::host_inferno_cannon::HostInfernoFireZoneRegistry,

    /// Host America Aurora dive bomb residual (delayed FuelAir / AuroraBomb area damage).
    /// FuelAir CreateObjectDie gas SpecialObject residual closed.
    pub(crate) aurora_bombs: crate::game_logic::host_aurora_bomb::HostAuroraBombRegistry,
    /// Honesty: AirF/SupW Aurora FuelAir gas objects spawned on dive impact.
    pub(super) aurora_fuel_air_gas_spawned: u32,

    /// Host GLA Angry Mob residual (nexus damages nearby enemies / expands members).
    /// SpawnBehavior member SpecialObject residual closed.
    pub(crate) angry_mobs: crate::game_logic::host_angry_mob::HostAngryMobRegistry,

    /// Host SCIENCE_StealthFighter production gate residual honesty.
    /// Fail-closed: not full PrerequisiteSciences rank tree / control-bar science UI.
    pub(super) stealth_fighter_science:
        crate::game_logic::host_stealth_fighter::HostStealthFighterRegistry,

    /// Host SCIENCE unit-training residual (VeterancyGainCreate StartingLevel).
    /// Fail-closed: not full PrerequisiteSciences rank tree / IsTrainable matrix.
    pub(super) unit_training: crate::game_logic::host_unit_training::HostUnitTrainingRegistry,

    /// Host Demo SuicideBomb residual (Demo_Upgrade_SuicideBomb death blast).
    /// Fail-closed: not full FireWeaponWhenDead exclusive / SlowDeath fling matrix.
    pub(super) demo_suicide_bomb:
        crate::game_logic::host_demo_suicide_bomb::HostDemoSuicideBombRegistry,

    /// Host GLA Rocket Buggy residual honesty (long-range rocket + scatter splash).
    /// Fail-closed: not full projectile flight / AP rocket mult matrix.
    pub(super) rocket_buggy_residual_fires: u32,
    pub(super) rocket_buggy_residual_units_hit: u32,
    pub(super) rocket_buggy_residual_scatter_misses: u32,

    /// Host GLA Quad Cannon residual honesty (ground gun + AA secondary + multi-barrel).
    /// Fail-closed: not full salvage W3D turret subobject matrix.
    pub(super) quad_cannon_residual_ground_fires: u32,
    pub(super) quad_cannon_residual_aa_fires: u32,
    pub(super) quad_cannon_residual_barrel_upgrades: u32,

    /// Host GLA SCUD launcher residual (area blast + MediumPoisonField toxin DoT).
    /// Fail-closed: not full SCUDMissile projectile lob / salvage PlusOne matrix.
    pub(super) scud_poison_zones: crate::game_logic::host_scud_launcher::HostScudPoisonRegistry,
    /// Host Overlord/Helix/Emperor portable addon residual honesty registry.
    pub(super) overlord_addons: crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry,
    /// Host Nuke Cannon primary residual (area + medium radiation).
    pub(super) nuke_cannon_residual: crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry,
    /// Host residual: GLA Technical transport + salvage weapon honesty.
    /// Fail-closed: not full SalvageCrate W3D gunner subobject matrix.
    pub(super) technical_residual_fires: u32,
    pub(super) technical_residual_units_hit: u32,
    pub(super) technical_residual_weapon_upgrades: u32,
    pub(super) technical_residual_loads: u32,
    pub(super) technical_residual_unloads: u32,
    /// Host residual: GLA Toxin Tractor stream/spray/death poison fields.
    pub(super) toxin_tractor: crate::game_logic::host_toxin_tractor::HostToxinTractorRegistry,

    /// Host residual: GLA Marauder salvage fire-rate tiers honesty.
    /// Fail-closed: not full SalvageCrate W3D turret subobject matrix.
    pub(super) marauder_residual_fires: u32,
    pub(super) marauder_residual_units_hit: u32,
    pub(super) marauder_residual_weapon_upgrades: u32,

    /// Host residual: GLA Scorpion gun + salvage + rocket secondary honesty.
    /// Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
    pub(super) scorpion_residual_fires: u32,
    pub(super) scorpion_residual_units_hit: u32,
    pub(super) scorpion_residual_rocket_upgrades: u32,
    pub(super) scorpion_residual_salvage_upgrades: u32,
    pub(super) scorpion_residual_missile_fires: u32,

    /// Host residual: USA Tomahawk dual-radius missile honesty.
    /// TomahawkMissile projectile lob residual closed (MissileAI peels + impact).
    pub(super) tomahawk_residual_fires: u32,
    pub(super) tomahawk_residual_units_hit: u32,

    /// Host residual: USA Raptor jet missiles + Laser Missiles honesty.
    /// RETURN_TO_BASE ClipReload airfield rearm residual (dock then ClipReload frames).
    pub(super) raptor_residual_fires: u32,
    pub(super) raptor_residual_units_hit: u32,
    pub(super) raptor_residual_laser_missiles_upgrades: u32,
    /// Host China MiG residual napalm / BlackNapalm / Nuke missile honesty.
    pub(super) mig_residual_fires: u32,
    pub(super) mig_residual_units_hit: u32,
    pub(super) mig_residual_black_napalm_upgrades: u32,
    pub(super) mig_residual_tactical_nuke_upgrades: u32,
    pub(super) mig_residual_fire_fields: u32,
    pub(super) mig_residual_radiation_fields: u32,
    /// Honesty: MiG NapalmMissile ScatterRadiusVsInfantry peels applied.
    pub(super) mig_scatter_applied: u32,
    /// Honesty: MiG ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) mig_scatter_misses: u32,
    /// Host America Fire Base howitzer residual honesty.
    pub(super) fire_base_residual_fires: u32,
    pub(super) fire_base_residual_units_hit: u32,

    /// Host Stealth Fighter residual honesty (missile fire + splash).
    /// Fail-closed: not full RETURN_TO_BASE ClipReload / science production matrix.
    pub(super) stealth_fighter_residual_fires: u32,
    pub(super) stealth_fighter_residual_units_hit: u32,
    /// Honesty: StealthJetMissile projectiles spawned residual.
    pub(super) stealth_jet_missiles_spawned: u32,
    /// Honesty: Stealth Jet ScatterRadiusVsInfantry aim offsets applied.
    pub(super) stealth_jet_scatter_applied: u32,
    /// Honesty: Stealth Jet ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) stealth_jet_scatter_misses: u32,
    /// Honesty: SCUDMissile projectiles spawned residual.
    pub(super) scud_missiles_spawned: u32,
    /// Honesty: TomahawkMissile projectiles spawned residual.
    pub(super) tomahawk_missiles_spawned: u32,
    /// Honesty: Tomahawk ScatterRadiusVsInfantry aim offsets applied.
    pub(super) tomahawk_scatter_applied: u32,
    /// Honesty: Tomahawk ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) tomahawk_scatter_misses: u32,
    /// Honesty: RocketBuggyMissile projectiles spawned residual.
    pub(super) rocket_buggy_missiles_spawned: u32,
    /// Honesty: Rocket Buggy ScatterRadiusVsInfantry aim offsets applied.
    pub(super) rocket_buggy_scatter_applied: u32,
    /// Honesty: SCUD Launcher ScatterRadiusVsInfantry aim offsets applied.
    pub(super) scud_launcher_scatter_applied: u32,
    /// Honesty: SCUD launcher ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) scud_launcher_scatter_misses: u32,
    /// Honesty: NeutronCannonShell projectiles spawned residual.
    pub(super) neutron_shells_spawned: u32,
    /// Honesty: Neutron shell ScatterRadiusVsInfantry aim offsets applied.
    pub(super) neutron_shell_scatter_applied: u32,
    /// Honesty: Neutron shell ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) neutron_shell_scatter_misses: u32,
    /// Honesty: TunnelDefenderMissile / RPG projectiles spawned residual.
    pub(super) rpg_trooper_missiles_spawned: u32,
    /// Honesty: RPG Trooper ScatterRadiusVsInfantry aim offsets applied.
    pub(super) rpg_trooper_scatter_applied: u32,
    /// Honesty: RPG Trooper ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) rpg_trooper_scatter_misses: u32,
    /// Honesty: TankHunterMissile projectiles spawned residual.
    pub(super) tank_hunter_missiles_spawned: u32,
    /// Honesty: Tank Hunter ScatterRadiusVsInfantry aim offsets applied.
    pub(super) tank_hunter_scatter_applied: u32,
    /// Honesty: Tank Hunter ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) tank_hunter_scatter_misses: u32,
    /// Honesty: MissileDefenderMissile projectiles spawned residual.
    pub(super) missile_defender_missiles_spawned: u32,
    /// Honesty: Missile Defender ScatterRadiusVsInfantry aim offsets applied.
    pub(super) missile_defender_scatter_applied: u32,
    /// Honesty: Missile Defender ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) missile_defender_scatter_misses: u32,
    /// Honesty: ScorpionTankShell projectiles spawned residual.
    pub(super) scorpion_shells_spawned: u32,
    /// Honesty: Scorpion ScatterRadiusVsInfantry aim offsets applied.
    pub(super) scorpion_scatter_applied: u32,
    /// Honesty: Scorpion ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) scorpion_scatter_misses: u32,
    /// Honesty: ScorpionMissile projectiles spawned residual.
    pub(super) scorpion_missiles_spawned: u32,
    /// Honesty: NukeCannonShell projectiles spawned residual.
    pub(super) nuke_cannon_shells_spawned: u32,
    /// Honesty: NukeCannon ScatterRadiusVsInfantry aim offsets applied.
    pub(super) nuke_cannon_scatter_applied: u32,
    /// Honesty: Nuke Cannon ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) nuke_cannon_scatter_misses: u32,
    /// Honesty: GenericTankShell (USA Crusader/Paladin) projectiles spawned residual.
    pub(super) usa_tank_shells_spawned: u32,
    /// Honesty: USA tank ScatterRadiusVsInfantry aim offsets applied.
    pub(super) usa_tank_scatter_applied: u32,
    /// Honesty: USA tank ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) usa_tank_scatter_misses: u32,
    /// Honesty: BattleMasterTankShell projectiles spawned residual.
    pub(super) battlemaster_shells_spawned: u32,
    /// Honesty: Battlemaster ScatterRadiusVsInfantry aim offsets applied.
    pub(super) battlemaster_scatter_applied: u32,
    /// Honesty: Battlemaster ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) battlemaster_scatter_misses: u32,
    /// Honesty: OverlordTankShell projectiles spawned residual.
    pub(super) overlord_shells_spawned: u32,
    /// Honesty: Overlord ScatterRadiusVsInfantry aim offsets applied.
    pub(super) overlord_scatter_applied: u32,
    /// Honesty: Overlord ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) overlord_scatter_misses: u32,
    /// Honesty: InfernoTankShell projectiles spawned residual.
    pub(super) inferno_shells_spawned: u32,
    /// Honesty: Inferno Cannon ScatterRadiusVsInfantry aim offsets applied.
    pub(super) inferno_scatter_applied: u32,
    /// Honesty: Inferno Cannon ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) inferno_scatter_misses: u32,
    /// Honesty: MarauderTankShell projectiles spawned residual.
    pub(super) marauder_shells_spawned: u32,
    /// Honesty: Marauder ScatterRadiusVsInfantry aim offsets applied.
    pub(super) marauder_scatter_applied: u32,
    /// Honesty: Marauder ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) marauder_scatter_misses: u32,
    /// Honesty: Fire Base GenericTankShell lob projectiles spawned residual.
    pub(super) fire_base_shells_spawned: u32,
    /// Honesty: FireBaseHowitzer ScatterRadiusVsInfantry aim offsets applied.
    pub(super) fire_base_scatter_applied: u32,
    /// Honesty: Fire Base ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) fire_base_scatter_misses: u32,
    /// Honesty: RaptorJetMissile projectiles spawned residual.
    pub(super) raptor_missiles_spawned: u32,
    /// Honesty: Raptor ScatterRadiusVsInfantry aim offsets applied.
    pub(super) raptor_scatter_applied: u32,
    /// Honesty: Raptor ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) raptor_scatter_misses: u32,
    /// Honesty: NapalmMissile / MiG projectiles spawned residual.
    pub(super) mig_missiles_spawned: u32,
    /// Honesty: RangerFlashBangGrenade projectiles spawned residual.
    pub(super) flashbang_grenades_spawned: u32,
    /// Honesty: Flashbang ScatterRadius aim offsets applied.
    pub(super) flashbang_scatter_applied: u32,
    /// Honesty: Flashbang ScatterRadius residual misses intended target.
    pub(super) flashbang_scatter_misses: u32,
    /// Honesty: HumveeMissile / PatriotMissile TOW projectiles spawned residual.
    pub(super) humvee_tow_missiles_spawned: u32,
    /// Honesty: Humvee ground TOW ScatterRadiusVsInfantry aim offsets applied.
    pub(super) humvee_tow_scatter_applied: u32,
    /// Honesty: Humvee ground TOW ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) humvee_tow_scatter_misses: u32,
    /// Honesty: Humvee TOW residual fires (spawn or instant fallback).
    pub(super) humvee_tow_residual_fires: u32,
    /// Honesty: DragonTankFlameProjectile spawned residual.
    pub(super) dragon_flame_missiles_spawned: u32,
    /// Honesty: ToxinTruckStreamProjectile spawned residual.
    pub(super) toxin_stream_missiles_spawned: u32,
    /// Honesty: TechnicalRPGMissile spawned residual.
    pub(super) technical_rpg_missiles_spawned: u32,
    /// Honesty: Technical cannon GenericTankShell spawned residual.
    pub(super) technical_cannon_shells_spawned: u32,
    /// Honesty: Technical cannon ScatterRadiusVsInfantry aim offsets applied.
    pub(super) technical_cannon_scatter_applied: u32,
    /// Honesty: TechnicalCannon ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) technical_cannon_scatter_misses: u32,
    /// Honesty: CleanupStreamProjectile spawned residual.
    pub(super) cleanup_stream_missiles_spawned: u32,
    /// Honesty: Angry Mob rock/molotov projectiles spawned residual.
    pub(super) angry_mob_projectiles_spawned: u32,
    /// Honesty: USA tank gun residual units hit.
    pub(super) usa_tank_residual_units_hit: u32,

    /// Host Comanche combat residual honesty (20mm + anti-tank dual-radius).
    /// Rocket pods residual counters remain separate below.
    pub(super) comanche_cannon_residual_fires: u32,
    pub(super) comanche_cannon_residual_units_hit: u32,
    pub(super) comanche_antitank_residual_fires: u32,
    pub(super) comanche_antitank_residual_units_hit: u32,
    /// Honesty: Comanche AT ScatterRadiusVsInfantry peels applied.
    pub(super) comanche_at_scatter_applied: u32,
    /// Honesty: Comanche AT ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) comanche_at_scatter_misses: u32,

    /// Host Helix PRIMARY minigun residual honesty.
    /// Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
    pub(super) helix_minigun_residual_fires: u32,
    pub(super) helix_minigun_residual_units_hit: u32,

    /// Host Inferno BlackNapalm FireFieldUpgraded residual honesty.
    /// Fail-closed: not HistoricBonus Firestorm multi-shell matrix.
    pub(super) inferno_black_napalm_residual_upgrades: u32,
    pub(super) inferno_black_napalm_residual_zones: u32,

    /// Host residual: USA Battle Drone attach / gun / repair honesty.
    /// ConflictsWith mux is live on queue/giveUpgrade (one drone per vehicle).
    pub(super) battle_drone_residual_attaches: u32,
    pub(super) battle_drone_residual_fires: u32,
    pub(super) battle_drone_residual_units_hit: u32,
    pub(super) battle_drone_residual_repairs: u32,
    pub(super) battle_drone_residual_repair_amount: f32,
    /// C++ SlavedUpdate weld duty cycle per Battle Drone (`doRepairLogic`).
    pub(super) battle_drone_weld_states: std::collections::HashMap<
        ObjectId,
        crate::game_logic::host_slave_drones::BattleDroneWeldState,
    >,

    /// Host residual: China Overlord / Emperor main gun dual-radius + Uranium honesty.
    /// Fail-closed: not full ClipSize=2 dual-volley / Nuclear Tanks death residual.
    pub(super) overlord_gun_residual_fires: u32,
    pub(super) overlord_gun_residual_units_hit: u32,
    pub(super) overlord_gun_residual_uranium_upgrades: u32,

    /// Host residual: GLA Jarmen Kell sniper + AP Bullets honesty.
    /// Fail-closed: not full secondary pilot-sniper AutoChoose matrix.
    pub(super) jarmen_kell_residual_fires: u32,
    pub(super) jarmen_kell_residual_units_hit: u32,
    pub(super) jarmen_kell_residual_ap_upgrades: u32,

    /// Host residual: China Battlemaster tank gun + Uranium / horde / nationalism honesty.
    /// HordeUpdate RubOff + terrain-decal fade residual closed.
    pub(super) battlemaster_residual_fires: u32,
    pub(super) battlemaster_residual_units_hit: u32,
    pub(super) battlemaster_residual_uranium_upgrades: u32,
    pub(super) battlemaster_residual_nationalism_upgrades: u32,
    pub(crate) battlemaster_residual_horde_grants: u32,

    /// Host residual: China Red Guard gun + bayonet + horde / nationalism honesty.
    /// Fail-closed: not full WeaponSet tertiary auto-choose matrix.
    pub(super) red_guard_residual_fires: u32,
    pub(super) red_guard_residual_bayonet_kills: u32,
    pub(super) red_guard_residual_nationalism_upgrades: u32,
    pub(crate) red_guard_residual_horde_grants: u32,

    /// Host residual: China Tank Hunter RPG + TNT special + horde / nationalism honesty.
    /// Fail-closed: not full SpecialAbilityUpdate flee / MaxSpecialObjects matrix.
    pub(super) tank_hunter_residual_fires: u32,
    pub(super) tank_hunter_residual_units_hit: u32,
    pub(super) tank_hunter_residual_tnt_plants: u32,
    pub(super) tank_hunter_residual_nationalism_upgrades: u32,
    pub(crate) tank_hunter_residual_horde_grants: u32,
    /// Per-unit TNT special residual last plant frame (ReloadTime 7500ms).
    pub(super) tank_hunter_tnt_last_frame: HashMap<ObjectId, u32>,

    /// Host residual: GLA Rebel machine gun + AP Bullets honesty.
    /// Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
    pub(super) rebel_residual_fires: u32,
    pub(super) rebel_residual_ap_upgrades: u32,

    /// Host residual: USA Ranger rifle + FlashBang splash honesty.
    /// Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
    pub(super) ranger_residual_rifle_fires: u32,
    pub(super) ranger_residual_flashbang_fires: u32,
    pub(super) ranger_residual_units_hit: u32,

    /// Host residual: China Hacker DisableBuilding honesty.
    /// Fail-closed: not full unpack/prep/persistent stream matrix.
    pub(super) hacker_disable_building_count: u32,

    /// Host residual: China MiniGunner ground/AA + continuous fire + chain guns + horde.
    /// Fail-closed: not full FiringTracker CONTINUOUS_FIRE_* anim / bayonet tertiary matrix.
    pub(super) minigunner_residual_ground_fires: u32,
    pub(super) minigunner_residual_aa_fires: u32,
    pub(super) minigunner_residual_ramp_mean: u32,
    pub(super) minigunner_residual_ramp_fast: u32,
    pub(super) minigunner_residual_chain_gun_upgrades: u32,
    pub(super) minigunner_residual_nationalism_upgrades: u32,
    pub(crate) minigunner_residual_horde_grants: u32,

    /// Host residual: Colonel Burton sniper + knife melee honesty.
    /// Fail-closed: not full clip volley / pre-attack knife anim lock matrix.
    pub(super) burton_residual_sniper_fires: u32,
    pub(super) burton_residual_knife_kills: u32,

    /// Host residual: GLA RPG Trooper / Tunnel Defender rocket + AP Rockets honesty.
    /// Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
    pub(super) rpg_trooper_residual_fires: u32,
    pub(super) rpg_trooper_residual_units_hit: u32,
    pub(super) rpg_trooper_residual_ap_upgrades: u32,

    /// Host residual: GLA Terrorist SuicideDynamitePack detonation honesty.
    /// Fail-closed: not ConvertToCarBomb full matrix / Chem anthrax death weapons.
    pub(super) terrorist_residual_detonations: u32,
    pub(super) terrorist_residual_units_hit: u32,
    pub(super) terrorist_residual_damage_dealt: f32,

    /// Host residual: USA Missile Defender missile + laser guided special honesty.
    /// Fail-closed: not full SpecialAbilityUpdate prep / LaserBeam object matrix.
    pub(super) missile_defender_residual_fires: u32,
    pub(super) missile_defender_residual_units_hit: u32,
    pub(super) missile_defender_residual_laser_specials: u32,
    pub(super) missile_defender_residual_laser_fires: u32,
    /// Honesty: LaserBeam SpecialObject spawned on MD laser-guided activate.
    pub(super) missile_defender_laser_beams_spawned: u32,

    /// Host residual: GLA Combat Cycle rider weapon switch honesty.
    /// Fail-closed: not full RiderChangeContain STATUS_RIDER death OCL matrix.
    pub(super) combat_cycle_residual_fires: u32,
    pub(super) combat_cycle_residual_units_hit: u32,
    pub(super) combat_cycle_residual_rider_switches: u32,
    pub(super) combat_cycle_residual_loads: u32,
    pub(super) combat_cycle_residual_suicides: u32,

    /// Host residual: China Dragon Tank primary flame honesty.
    /// Fail-closed: not full projectile stream / garrison-clear matrix.
    pub(super) dragon_tank_residual_fires: u32,
    pub(super) dragon_tank_residual_units_hit: u32,
    pub(super) dragon_tank_residual_black_napalm_upgrades: u32,

    /// Host residual: China Gattling Tank continuous-fire ramp honesty.
    /// Fail-closed: not full FiringTracker model-condition animation matrix.
    pub(super) gattling_tank_residual_ground_fires: u32,
    pub(super) gattling_tank_residual_aa_fires: u32,
    pub(super) gattling_tank_residual_ramp_mean: u32,
    pub(super) gattling_tank_residual_ramp_fast: u32,
    pub(super) gattling_tank_residual_chain_gun_upgrades: u32,

    /// Host residual: China Gattling Cannon structure continuous-fire ramp honesty.
    /// Fail-closed: not full CONTINUOUS_FIRE_* model-condition animation matrix.
    pub(super) gattling_building_residual_ground_fires: u32,
    pub(super) gattling_building_residual_aa_fires: u32,
    pub(super) gattling_building_residual_ramp_mean: u32,
    pub(super) gattling_building_residual_ramp_fast: u32,
    pub(super) gattling_building_residual_chain_gun_upgrades: u32,

    /// Host residual: GLA Stinger Site SPAWNS_ARE_THE_WEAPONS dual ground/AA honesty.
    pub(super) stinger_site_residual_ground_fires: u32,
    pub(super) stinger_site_residual_aa_fires: u32,
    pub(super) stinger_site_residual_ap_rockets_upgrades: u32,
    /// HiveStructureBody residual: damage hits applied to residual slaves.
    pub(super) stinger_hive_residual_slave_hits: u32,
    /// HiveStructureBody residual: residual slaves killed.
    pub(super) stinger_hive_residual_slave_kills: u32,
    /// HiveStructureBody residual: swallowed damage when no slaves.
    pub(super) stinger_hive_residual_swallows: u32,
    /// SpawnBehavior residual: slave respawns completed.
    pub(crate) stinger_hive_residual_respawns: u32,
    /// getClosestSlave residual: propagate hits that used shooter world position.
    pub(super) stinger_hive_residual_closest_slave_hits: u32,
    /// CamoNetting StealthLook / heat-vision residual applications.
    pub(super) camo_netting_heat_vision_count: u32,
    /// CamoNetting structure StealthUpdate residual: attack/damage reveals.
    pub(super) camo_netting_structure_residual_reveals: u32,
    /// OrderIdleEnemiesToAttackMeUponReveal residual wake count (CamoNetting).
    pub(super) camo_netting_order_idle_enemies_count: u32,
    /// CamoNetting structure StealthUpdate residual: StealthDelay re-cloaks.
    pub(super) camo_netting_structure_residual_recloaks: u32,
    /// CamoNetting FriendlyOpacity residual: cloaked (min opacity) applications.
    pub(super) camo_netting_opacity_cloak_count: u32,
    /// CamoNetting FriendlyOpacity residual: revealed (max opacity) applications.
    pub(super) camo_netting_opacity_reveal_count: u32,
    /// CamoNetting sub-object net mesh residual show applications.
    pub(super) camo_netting_sub_object_show_count: u32,
    /// Stinger physical soldier orderSlavesToAttackTarget residual orders.
    pub(super) stinger_slave_order_attack_count: u32,

    /// Host residual: USA Patriot ground/AA dual-slot honesty.
    pub(super) patriot_residual_ground_fires: u32,
    pub(super) patriot_residual_aa_fires: u32,
    /// Honesty: Patriot ScatterRadiusVsInfantry offsets / miss peels applied.
    pub(super) patriot_scatter_applied: u32,
    /// Honesty: Patriot ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) patriot_scatter_misses: u32,
    /// Honesty: Stinger ScatterRadiusVsInfantry peels applied.
    pub(super) stinger_scatter_applied: u32,
    /// Honesty: Stinger ScatterRadiusVsInfantry residual misses vs infantry.
    pub(super) stinger_scatter_misses: u32,
    /// Superweapon General EMP Patriot residual: DISABLED_EMP grants applied.
    pub(super) supw_patriot_emp_residual_grants: u32,
    /// Honesty: EMPPatriotEffectSpheroid objects spawned on Patriot EMP blast.
    pub(super) supw_patriot_emp_spheroids_spawned: u32,
    /// Honesty: EMPSparks particle systems spawned on EMP victims.
    pub(super) supw_patriot_emp_sparks_spawned: u32,
    /// Honesty: SupW EMPBlast ScatterRadiusVsInfantry peels applied.
    pub(super) supw_emp_scatter_applied: u32,
    /// Honesty: SupW EMPBlast scatter residual misses vs infantry.
    pub(super) supw_emp_scatter_misses: u32,
    /// AssistedTargetingUpdate residual: RequestAssistRange requests issued.
    pub(super) patriot_assist_residual_requests: u32,
    /// AssistedTargetingUpdate residual: assist weapon shots fired.
    pub(super) patriot_assist_residual_fires: u32,
    /// AssistedTargetingUpdate residual: assistants that accepted a request.
    pub(super) patriot_assist_residual_accepts: u32,
    /// BinaryDataStream residual: LaserFromAssisted beams spawned.
    pub(super) patriot_assist_laser_from_assisted: u32,
    /// BinaryDataStream residual: LaserToTarget beams spawned.
    pub(super) patriot_assist_laser_to_target: u32,
    /// Active residual BinaryDataStream assist lasers (DeletionUpdate lifetime).
    pub(crate) patriot_assist_lasers:
        Vec<crate::game_logic::host_base_defense::ResidualPatriotAssistLaser>,
    /// Weapon.ini LaserName residual beams (combat fire → presentation freeze).
    pub(super) weapon_lasers: Vec<crate::game_logic::host_weapon_laser::ResidualWeaponLaser>,
    /// Honesty: Weapon.ini LaserName SpecialObject Things spawned.
    pub(super) weapon_laser_beams_spawned: u32,
    /// C++ ProjectileStreamUpdate residual registry.
    pub(crate) projectile_streams:
        crate::game_logic::host_projectile_stream::ProjectileStreamRegistry,
    /// Pending AssistingClipSize residual clips (DelayBetweenShots cadence).
    pub(super) pending_patriot_assists:
        Vec<crate::game_logic::host_base_defense::PendingPatriotAssist>,
    /// StealthDetectorUpdate DetectionRate residual scans performed.
    pub(super) stealth_detector_rate_scans: u32,

    /// Game paused state
    pub(super) is_paused: bool,

    /// Time tracking
    pub(super) sim_time_seconds: f32,
    pub(super) accumulated_time: f32,
    pub(super) last_fixed_step_diagnostics: FixedStepDiagnostics,

    /// Thing templates registry
    pub templates: HashMap<String, ThingTemplate>,
    /// Names that already failed host-template + already-loaded Object INI lookup.
    /// Map spawn repeats the same decorative names; do not re-scan archives.
    pub(super) unresolved_spawn_templates: HashSet<String>,

    /// Map data
    pub(super) map_name: String,
    pub(super) map_loaded: bool,

    /// Combat system for parallel projectile processing
    pub(crate) combat_system: CombatSystem,

    /// Pathfinding system for parallel path computation
    pub(crate) pathfinding_system: PathfindingSystem,

    /// AI Management System
    pub(crate) ai_manager: AIManager,

    /// Script execution tracking
    pub scripts_loaded: bool,
    pub mission_script_counter: u32,

    /// Audio events queued this frame (mirrors C++ TheAudio pattern)
    /// In production, these would be sent to the audio engine
    pub queued_audio_events: Vec<AudioEventRequest>,

    /// Command queue for UI-generated commands
    pub command_queue: VecDeque<crate::command_system::GameCommand>,
    /// Narrow command-acceptance observation edge used to bind an actual
    /// physical right-click Gather input to the executor-confirmed carriers.
    pub(super) accepted_gather_commands: VecDeque<AcceptedGatherCommand>,
    /// Narrow economy observation edge emitted only by ReturningResources
    /// after crediting carried supplies to a concrete player.
    pub(super) supply_dropoff_events: VecDeque<SupplyDropoffEvent>,
    pub(super) pending_special_abilities: HashMap<ObjectId, PendingSpecialAbility>,

    /// Currently selected objects (used by UI)
    pub selected_objects: Vec<ObjectId>,

    pub(super) partition_manager: PartitionManager,
    pub(super) radar_notifications: &'static RadarNotifications,
    pub(super) last_radar_kind_time: [f32; 3],
    pub(super) last_radar_audio_time: f32,
    pub(super) last_radar_event: Option<RadarEntry>,
    /// C++ Radar tryUnderAttackEvent throttle residual (frame, xz pos).
    pub(super) under_attack_event_history: Vec<(u32, f32, f32)>,
    /// tryUnderAttackEvent residual honesty fires.
    pub(super) under_attack_events: u32,
    /// EVA BaseUnderAttack residual honesty fires.
    pub(super) eva_base_under_attack: u32,
    /// EVA AllyUnderAttack residual honesty fires.
    pub(super) eva_ally_under_attack: u32,
    /// EVA LowPower residual honesty fires.
    pub(super) eva_low_power: u32,
    /// Next frame LowPower may re-fire (C++ framesBetweenChecks residual).
    pub(super) eva_low_power_next_frame: u32,
    /// Tracks previous low-power state for edge residual.
    pub(super) eva_low_power_active: bool,
    /// EVA InsufficientFunds residual honesty fires.
    pub(super) eva_insufficient_funds: u32,

    /// EVA UpgradeComplete residual honesty fires.
    pub(super) eva_upgrade_complete: u32,
    /// EVA GeneralLevelUp residual honesty fires.
    pub(super) eva_general_level_up: u32,
    /// EVA SuperweaponReady residual honesty fires.
    pub(super) eva_superweapon_ready: u32,
    /// EVA SuperweaponDetected residual honesty fires.
    pub(super) eva_superweapon_detected: u32,
    /// EVA SuperweaponLaunched residual honesty fires.
    pub(super) eva_superweapon_launched: u32,
    /// EVA BeaconDetected residual honesty fires.
    pub(super) eva_beacon_detected: u32,
    /// EVA hero Own/Enemy *Detected residual honesty fires.
    pub(super) eva_hero_detected: u32,
    /// EVA SuperweaponLaunched GPS/Sneak residual honesty fires.
    pub(super) eva_special_launched_misc: u32,
    /// RADAR_EVENT_UPGRADE residual honesty fires.
    pub(super) radar_upgrade_events: u32,
    /// Structure construction-complete residual honesty fires.
    pub(super) structure_complete_events: u32,
    /// Unit production-complete residual honesty fires.
    pub(super) unit_ready_events: u32,
    /// RadarUpdate extendRadar residual starts.
    pub(super) radar_extend_starts: u32,
    /// RadarUpdate extend completion residual fires.
    pub(super) radar_extend_completes: u32,
    /// RADAR_EVENT_CONSTRUCTION residual honesty fires.
    pub(super) radar_construction_events: u32,
    /// Production door cycle residual honesty starts.
    pub(super) production_door_cycles: u32,
    /// Under-construction model condition residual updates.
    pub(super) construction_model_condition_updates: u32,
    /// ACTIVELY_CONSTRUCTING residual bit updates.
    pub(crate) actively_constructing_updates: u32,
    /// C++ BuildAssistant m_sellList residual.
    pub(super) sell_list: Vec<ObjectSellInfo>,
    /// Sell residual process starts / finishes.
    pub(super) sell_process_starts: u32,
    pub(super) sell_process_finishes: u32,
    /// C++ sellObject destroy owned mines residual.
    pub(super) sell_owned_mines_destroyed: u32,
    /// C++ OpenContain::onSelling passenger eject residual.
    pub(super) sell_passengers_ejected: u32,
    /// C++ ParkingPlaceBehavior::killAllParkedUnits residual.
    pub(super) sell_parked_units_killed: u32,
    /// C++ TunnelContain::onSelling last-tunnel eject residual.
    pub(super) sell_tunnel_last_ejects: u32,
    /// C++ ContainModule::onCapture kick residual events.
    pub(super) capture_kick_outs: u32,
    /// C++ Object::onCapture skirmish AI auto-sell residual events.
    pub(super) capture_ai_auto_sells: u32,
    /// C++ deselectObject on capture residual events.
    pub(super) capture_deselections: u32,
    /// C++ TunnelContain::onCapture entrance transfer residual events.
    pub(super) capture_tunnel_transfers: u32,
    /// C++ TunnelContain last-entrance capture eject residual events.
    pub(super) capture_tunnel_last_ejects: u32,
    /// C++ TechBuildingBehavior CAPTURED model residual events.
    pub(super) capture_tech_model_updates: u32,
    /// C++ infantry→unmanned vehicle recrew residual events.
    pub(super) unmanned_reclaims: u32,
    /// C++ car-bomb dead-man on DISABLED_UNMANNED residual events.
    pub(super) carbomb_unmanned_detonations: u32,
    /// C++ OverchargeBehavior toggle residual events.
    pub(super) overcharge_toggles: u32,
    /// C++ OverchargeBehavior drain residual ticks.
    pub(super) overcharge_drain_ticks: u32,
    /// C++ OverchargeBehavior exhausted auto-disable residual events.
    pub(super) overcharge_exhaustions: u32,
    /// C++ PowerPlantUpgrade Advanced Control Rods residual completions.
    pub(super) control_rods_upgrades: u32,
    /// Plants that received EnergyBonus from control rods residual.
    pub(super) control_rods_plants_affected: u32,
    /// C++ SubliminalMessaging upgrade residual completions.
    pub(super) subliminal_messaging_upgrades: u32,
    /// Propaganda towers tagged by subliminal residual.
    pub(super) subliminal_towers_affected: u32,
    /// CONSTRUCTION_COMPLETE duration clears residual.
    pub(super) construction_complete_clears: u32,
    /// C++ DozerAIUpdate::cancelTask residual events.
    pub(super) dozer_cancel_task_events: u32,
    /// C++ MSG_RESUME_CONSTRUCTION residual assigns.
    pub(super) resume_construction_events: u32,
    /// C++ DOZER:RepairComplete residual events.
    pub(super) repair_complete_events: u32,
    /// C++ attemptHealingFromSoleBenefactor reject residual (dozer repair).
    pub(super) sole_benefactor_repair_rejects: u32,
    /// C++ DozerPrimaryIdleState bored auto-repair residual events.
    pub(super) dozer_bored_repair_events: u32,
    /// C++ DozerPrimaryIdleState bored mine-clear residual events.
    pub(super) dozer_bored_mine_clear_events: u32,
    /// C++ RebuildHoleExposeDie spawn residual events.
    pub(super) rebuild_hole_spawns: u32,
    /// C++ SupplyWarehouseCreate::onCreate residual registers.
    pub(super) supply_create_warehouse_registers: u32,
    /// C++ SupplyCenterCreate::onBuildComplete residual registers.
    pub(super) supply_create_center_registers: u32,
    /// C++ GenerateMinefieldBehavior structure mine placements.
    pub(super) structure_minefield_placements: u32,
    /// C++ SpecialPowerCompletionDie + PowerPlantUpdate residual log.
    pub(crate) special_power_completion_log:
        crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionLog,
    /// C++ StickyBombUpdate follow-position residual ticks.
    pub(crate) sticky_bomb_follow_ticks: u32,
    /// C++ StickyBombUpdate target-dead charge destroy residual.
    pub(crate) sticky_bomb_target_deaths: u32,
    /// C++ RebuildHoleBehavior reconstruct residual events.
    pub(super) rebuild_hole_reconstructs: u32,
    pub(super) rebuild_hole_workers: u32,
    pub(super) rebuild_hole_heals: u32,
    pub(super) rebuild_hole_completes: u32,
    /// C++ transferAttack residual events around rebuild holes.
    pub(super) rebuild_hole_attack_transfers: u32,
    /// C++ RebuildHoleBehavior::transferBombs residual events.
    pub(super) rebuild_hole_bomb_transfers: u32,
    /// C++ RECONSTRUCTING death → hole restart residual events.
    pub(super) rebuild_hole_recon_deaths: u32,
    /// C++ newWorkerRespawnProcess residual events.
    pub(super) rebuild_hole_worker_restarts: u32,
    pub(super) pending_camera_focus: Option<Vec3>,
    pub(super) script_camera_focus_estimate: Vec3,
    pub(super) script_camera_move_to: Option<ScriptCameraMoveTo>,
    pub(super) script_camera_path: Option<ScriptCameraPathMove>,
    pub(super) camera_follow_target: Option<ObjectId>,
    pub(super) camera_tether_play: Option<f32>,
    pub(super) script_look_toward_object_id: Option<u32>,
    pub(super) script_look_toward_hold_seconds: f32,
    pub(super) script_default_camera_pitch: f32,
    pub(super) script_default_camera_angle: f32,
    pub(super) script_default_camera_max_height: f32,
    pub(super) script_camera_freeze_time_armed: bool,
    /// C++ `W3DView::m_freezeTimeForCameraMovement`.
    pub(super) script_camera_freeze_time: bool,
    /// Remaining scripted rotate / look-toward seconds (survives presentation take).
    pub(super) script_camera_rotate_remaining: f32,
    /// Remaining scripted zoom seconds (survives presentation take).
    pub(super) script_camera_zoom_remaining: f32,
    /// Remaining scripted pitch seconds (survives presentation take).
    pub(super) script_camera_pitch_remaining: f32,
    pub(super) script_camera_freeze_angle_armed: bool,
    pub(super) script_camera_pending_final_speed_multiplier: Option<f32>,
    pub(super) visual_speed_multiplier: f32,
    pub(super) script_time_frozen_by_script: bool,
    pub(super) pending_script_fps_limit: Option<i32>,
    pub(super) pending_camera_zoom_reset: bool,
    pub(super) pending_camera_zoom_reset_duration: f32,
    pub(super) pending_camera_zoom_reset_ease_in: f32,
    pub(super) pending_camera_zoom_reset_ease_out: f32,
    pub(super) pending_camera_zoom: Option<CameraZoomRequest>,
    pub(super) pending_camera_pitch: Option<CameraPitchRequest>,
    pub(super) pending_camera_rotate: Option<CameraRotateRequest>,
    pub(super) pending_camera_look_toward: Option<CameraLookTowardWaypointRequest>,
    pub(super) pending_camera_slave_mode_enable: Option<CameraSlaveModeRequest>,
    pub(super) pending_camera_slave_mode_disable: bool,
    pub(super) pending_screen_shakes: Vec<ScreenShakeRequest>,
    pub(super) pending_camera_add_shakers: Vec<CameraAddShakerRequest>,
    pub(super) pending_popup_messages: Vec<ScriptPopupMessageRequest>,
    pub(super) pending_view_guardband: Option<ViewGuardbandRequest>,
    pub(super) pending_camera_bw_mode: Option<CameraBwModeRequest>,
    pub(super) pending_camera_motion_blur: Vec<CameraMotionBlurRequest>,
    pub(super) script_skybox_enabled: bool,
    pub(super) script_cameo_flash_count: HashMap<String, i32>,
    pub(super) script_named_timers: HashMap<String, (String, bool)>,
    pub(super) script_named_timer_display_shown: bool,
    pub(super) script_superweapon_display_enabled: bool,
    pub(super) script_superweapon_hidden_objects: HashSet<ObjectId>,
    /// C++ SuperweaponInfo::m_hiddenByScience latch (InGameUI.cpp:559).
    /// Presence means snapshotted at timer-create; `true` skips SuperweaponReady EVA
    /// and never clears, matching srj InGameUI.cpp:568-570.
    pub(super) eva_superweapon_science_hidden: HashMap<ObjectId, bool>,
    /// Host-owned active beacon world positions (presentation freeze; Wave 211).
    /// Mirrors beacon_manager place/remove without mid-frame Mutex dual-read.
    pub(super) host_beacons: Vec<Vec3>,
    /// Beacon locations created this frame for HUD highlighting/bloom.
    pub(super) recent_beacons: Vec<Vec3>,
    /// Leftover ScriptingEngine second brain (hq-8ta4n). Always None on live host.
    pub(super) script_engine: Option<Arc<ScriptingEngine>>,
    pub(super) script_event_pump_in_flight: Arc<AtomicBool>,
    pub(super) script_event_pump_busy_frames: u32,
    pub(super) loaded_script_lists: Vec<ScriptList>,
    pub(super) script_source_path: Option<PathBuf>,
    pub(super) mission_scripts: Arc<MissionScriptHooks>,
    pub(super) script_broadcasts: Vec<ScriptBroadcast>,
    pub(super) new_script_messages: Vec<String>,
    pub(super) cinematic_letterbox: bool,
    pub(super) cinematic_text: Option<(String, f32)>,
    /// C++ FONT_NAME leftover (`Name - Size:N [Bold]`).
    pub(super) cinematic_font: Option<String>,
    pub(super) military_caption: Option<(String, f32)>,
    pub(super) radar_enabled: bool,
    pub(super) radar_forced: bool,
    pub(super) pending_music_stop: bool,
    pub(super) pending_movie: Option<String>,
    pub(super) pending_radar_movie: Option<String>,
    pub(super) mission_objectives: Vec<ObjectiveDisplay>,
    pub(super) objective_lookup: HashMap<String, usize>,
    pub(super) campaign_manager: Option<Arc<Mutex<CampaignManager>>>,
    pub(super) last_map_settings: Option<super::script_loader::MapMetadata>,
    pub(super) spawned_map_object_ids: Vec<(ObjectId, usize)>,
    pub(super) terrain: Option<super::terrain::TerrainData>,
    pub(super) runtime_road_segments: Vec<super::script_loader::RuntimeRoadSegment>,
    pub(super) runtime_terrain_texture_classes: Vec<super::script_loader::BlendTileTextureClass>,
    pub(super) pathfinding_height_samples: Option<PathfindingHeightSamples>,
    pub(super) weather_state: RuntimeWeatherState,
    /// C++ `m_sleepyUpdates` host residual (`GameLogic.cpp:3699-3740`).
    pub(super) host_sleepy: super::world_tick::HostSleepyHeap,
    /// C++ always adds a ReplayObserver side at startNewGame; host player id.
    pub(super) replay_observer_player_id: Option<u32>,
    /// C++ startNewGame installs MultiplayerScripts.scb when numTeams > 1.
    pub(super) install_multiplayer_scripts: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PathfindingHeightSamples {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct RuntimeWeatherState {
    pub current_weather: String,
    pub intensity: f32,
    pub duration_remaining: f32,
    pub next_change_time: f32,
    pub visible: bool,
}
