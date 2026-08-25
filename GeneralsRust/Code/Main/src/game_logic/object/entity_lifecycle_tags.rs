//! Ordered Main Object residual-group tags for the Entity lifecycle envelope.
//!
//! Order matches `Object` field-group declaration order (not C++ helper-first
//! xfer order). That declaration order is the inventory contract for this slice.

/// C++ UpgradeDie residual.
pub const TAG_UPGRADE_DIE: &str = "UpgradeDie";
/// C++ SpecialPowerCompletionDie residual.
pub const TAG_SPECIAL_POWER_COMPLETION: &str = "SpecialPowerCompletionDie";
/// C++ BattleBusSlowDeathBehavior residual.
pub const TAG_BATTLE_BUS_BODY: &str = "BattleBusSlowDeath";
/// C++ SpecialAbilityUpdate capture channel residual.
pub const TAG_CAPTURE_CHANNEL: &str = "CaptureChannel";
/// C++ SpecialAbilityUpdate Hacker Disable Building residual.
pub const TAG_HACKER_DISABLE_CHANNEL: &str = "HackerDisableChannel";
/// C++ ToppleUpdate residual.
pub const TAG_TOPPLE: &str = "ToppleUpdate";
/// C++ StructureToppleUpdate residual.
pub const TAG_STRUCTURE_TOPPLE: &str = "StructureToppleUpdate";
/// C++ StructureCollapseUpdate residual.
pub const TAG_STRUCTURE_COLLAPSE: &str = "StructureCollapseUpdate";
/// C++ KeepObjectDie residual.
pub const TAG_KEEP_OBJECT_DIE: &str = "KeepObjectDie";
/// C++ WaveGuideUpdate residual.
pub const TAG_WAVE_GUIDE: &str = "WaveGuideUpdate";
/// C++ BoneFXDamage residual.
pub const TAG_BONE_FX_DAMAGE: &str = "BoneFXDamage";
/// C++ PoisonedBehavior residual.
pub const TAG_POISONED: &str = "PoisonedBehavior";
/// C++ ObjectDefectionHelper residual.
pub const TAG_DEFECTION: &str = "ObjectDefectionHelper";
/// C++ FireWeaponPower residual.
pub const TAG_FIRE_WEAPON_POWER: &str = "FireWeaponPower";
/// FireWeaponWhenDamaged + temporary weapon runtime + pending name.
pub const TAG_FIRE_WEAPON_WHEN_DAMAGED: &str = "FireWeaponWhenDamaged";
/// C++ TransitionDamageFX residual.
pub const TAG_TRANSITION_DAMAGE_FX: &str = "TransitionDamageFX";
/// C++ FXListDie residual.
pub const TAG_FX_LIST_DIE: &str = "FXListDie";
/// C++ CreateObjectDie residual.
pub const TAG_CREATE_OBJECT_DIE: &str = "CreateObjectDie";
/// C++ LifetimeUpdate residual.
pub const TAG_LIFETIME: &str = "LifetimeUpdate";
/// C++ SlowDeathBehavior residual.
pub const TAG_SLOW_DEATH: &str = "SlowDeath";
/// C++ HeightDieUpdate residual.
pub const TAG_HEIGHT_DIE: &str = "HeightDieUpdate";
/// C++ FuelAir gas SlowDeath residual.
pub const TAG_FUEL_AIR_GAS_SLOW_DEATH: &str = "FuelAirGasSlowDeath";
/// C++ NeutronMissileUpdate residual.
pub const TAG_NEUTRON_MISSILE: &str = "NeutronMissileUpdate";
/// C++ MissileLauncherBuildingUpdate residual.
pub const TAG_MISSILE_LAUNCHER_BUILDING: &str = "MissileLauncherBuildingUpdate";
/// C++ ScudStormMissile flight residual.
pub const TAG_SCUD_STORM_FLIGHT: &str = "ScudStormMissileFlight";
/// C++ CarpetBomb transport residual.
pub const TAG_CARPET_BOMB_TRANSPORT: &str = "CarpetBombTransport";
/// C++ ArtilleryBarrage transport residual.
pub const TAG_ARTILLERY_BARRAGE_TRANSPORT: &str = "ArtilleryBarrageTransport";
/// C++ A10 strike transport residual.
pub const TAG_A10_STRIKE_TRANSPORT: &str = "A10StrikeTransport";
/// C++ DaisyCutter transport residual.
pub const TAG_DAISY_CUTTER_TRANSPORT: &str = "DaisyCutterTransport";
/// C++ AnthraxBomb transport residual.
pub const TAG_ANTHRAX_BOMB_TRANSPORT: &str = "AnthraxBombTransport";
/// C++ ClusterMines transport residual.
pub const TAG_CLUSTER_MINES_TRANSPORT: &str = "ClusterMinesTransport";
/// C++ EMPPulse transport residual.
pub const TAG_EMP_PULSE_TRANSPORT: &str = "EmpPulseTransport";
/// C++ TensileFormationUpdate residual.
pub const TAG_TENSILE_FORMATION: &str = "TensileFormationUpdate";
/// C++ FireSpreadUpdate residual.
pub const TAG_FIRE_SPREAD: &str = "FireSpreadUpdate";
/// C++ BaseRegenerateUpdate residual.
pub const TAG_BASE_REGENERATE: &str = "BaseRegenerateUpdate";
/// C++ ModuleTag_DefaultAutoHealBehavior residual.
pub const TAG_DEFAULT_AUTO_HEAL: &str = "DefaultAutoHealBehavior";
/// C++ EnemyNearUpdate residual.
pub const TAG_ENEMY_NEAR: &str = "EnemyNearUpdate";

/// C++ AnimationSteeringUpdate residual.
pub const TAG_ANIMATION_STEERING: &str = "AnimationSteeringUpdate";
/// C++ FloatUpdate residual.
pub const TAG_FLOAT_UPDATE: &str = "FloatUpdate";
/// C++ ProneUpdate residual.
pub const TAG_PRONE_UPDATE: &str = "ProneUpdate";
/// C++ RadiusDecalUpdate residual.
pub const TAG_RADIUS_DECAL: &str = "RadiusDecalUpdate";
/// C++ CheckpointUpdate residual.
pub const TAG_CHECKPOINT: &str = "CheckpointUpdate";
/// C++ SpectreGunshipDeploymentUpdate residual.
pub const TAG_SPECTRE_GUNSHIP_DEPLOYMENT: &str = "SpectreGunshipDeployment";
/// C++ SpectreGunshipUpdate residual.
pub const TAG_SPECTRE_GUNSHIP_UPDATE: &str = "SpectreGunshipUpdate";
/// C++ SmartBombTargetHomingUpdate residual.
pub const TAG_SMART_BOMB_HOMING: &str = "SmartBombTargetHoming";
/// C++ HelicopterSlowDeathBehavior residual.
pub const TAG_HELICOPTER_SLOW_DEATH: &str = "HelicopterSlowDeath";
/// C++ JetSlowDeathBehavior residual.
pub const TAG_JET_SLOW_DEATH: &str = "JetSlowDeath";
/// C++ MinefieldBehavior residual.
pub const TAG_MINE: &str = "MinefieldBehavior";
/// C++ SpawnBehavior residual (Stinger hive slave roster).
pub const TAG_SPAWN_BEHAVIOR: &str = "SpawnBehavior";
/// C++ FiringTracker residual (consecutive shots / gattling spin).
pub const TAG_FIRING_TRACKER: &str = "FiringTracker";
/// C++ FireOCLAfterWeaponCooldownUpdate residual.
pub const TAG_FIRE_OCL_AFTER_COOLDOWN: &str = "FireOclAfterCooldown";
/// C++ AssaultTransportAIUpdate residual.
pub const TAG_ASSAULT_TRANSPORT: &str = "AssaultTransport";
/// C++ DeployStyleAIUpdate residual.
pub const TAG_DEPLOY_STYLE: &str = "DeployStyleAIUpdate";
/// C++ CommandButtonHuntUpdate residual.
pub const TAG_COMMAND_BUTTON_HUNT: &str = "CommandButtonHuntUpdate";
/// C++ FireWeaponWhenDeadBehavior once-fired residual (`Object.cpp` module xfer).
pub const TAG_FIRE_WEAPON_WHEN_DEAD: &str = "FireWeaponWhenDead";
/// C++ CreateObjectDie TransferPreviousHealth residual.
pub const TAG_CREATE_OBJECT_DIE_TRANSFER: &str = "CreateObjectDieTransfer";
/// C++ SpecialPowerModule per-power cooldown map + override dest.
pub const TAG_SPECIAL_POWER_COOLDOWNS: &str = "SpecialPowerCooldown";
/// C++ WeaponSet lock residual (`WeaponLockType` + slot).
pub const TAG_WEAPON_LOCK: &str = "WeaponLock";
/// C++ Drawable::setEmoticon + AIUpdateInterface::setSurrendered timers.
pub const TAG_EMOTICON_SURRENDER: &str = "EmoticonSurrender";
/// C++ MissileAIUpdate / DumbProjectileBehavior / LifetimeUpdate flight residuals.
pub const TAG_PROJECTILE_FLIGHT: &str = "ProjectileFlightResiduals";
/// C++ ActiveBody::xfer crush flags + UNIT INDESTRUCTIBLE + last-damage stamps.
pub const TAG_ACTIVE_BODY: &str = "ActiveBody";
/// C++ PhysicsBehavior::xfer v2 stun rates, overlap chain, ignore-collisions, motive.
pub const TAG_PHYSICS_BEHAVIOR: &str = "PhysicsBehavior";
/// C++ RailroadBehavior::xfer v3 conductor / track / hitch residual.
pub const TAG_RAILROAD: &str = "RailroadBehavior";

/// Declaration-order inventory. Producer emits tags in this sequence.
pub const INVENTORY_TAGS: &[&str] = &[
    TAG_UPGRADE_DIE,
    TAG_SPECIAL_POWER_COMPLETION,
    TAG_BATTLE_BUS_BODY,
    TAG_CAPTURE_CHANNEL,
    TAG_HACKER_DISABLE_CHANNEL,
    TAG_TOPPLE,
    TAG_STRUCTURE_TOPPLE,
    TAG_STRUCTURE_COLLAPSE,
    TAG_KEEP_OBJECT_DIE,
    TAG_WAVE_GUIDE,
    TAG_BONE_FX_DAMAGE,
    TAG_POISONED,
    TAG_DEFECTION,
    TAG_FIRE_WEAPON_POWER,
    TAG_FIRE_WEAPON_WHEN_DAMAGED,
    TAG_TRANSITION_DAMAGE_FX,
    TAG_FX_LIST_DIE,
    TAG_CREATE_OBJECT_DIE,
    TAG_LIFETIME,
    TAG_SLOW_DEATH,
    TAG_HEIGHT_DIE,
    TAG_FUEL_AIR_GAS_SLOW_DEATH,
    TAG_NEUTRON_MISSILE,
    TAG_MISSILE_LAUNCHER_BUILDING,
    TAG_SCUD_STORM_FLIGHT,
    TAG_CARPET_BOMB_TRANSPORT,
    TAG_ARTILLERY_BARRAGE_TRANSPORT,
    TAG_A10_STRIKE_TRANSPORT,
    TAG_DAISY_CUTTER_TRANSPORT,
    TAG_ANTHRAX_BOMB_TRANSPORT,
    TAG_CLUSTER_MINES_TRANSPORT,
    TAG_EMP_PULSE_TRANSPORT,
    TAG_TENSILE_FORMATION,
    TAG_FIRE_SPREAD,
    TAG_BASE_REGENERATE,
    TAG_DEFAULT_AUTO_HEAL,
    TAG_ENEMY_NEAR,
    TAG_ANIMATION_STEERING,
    TAG_FLOAT_UPDATE,
    TAG_PRONE_UPDATE,
    TAG_RADIUS_DECAL,
    TAG_CHECKPOINT,
    TAG_SPECTRE_GUNSHIP_DEPLOYMENT,
    TAG_SPECTRE_GUNSHIP_UPDATE,
    TAG_SMART_BOMB_HOMING,
    TAG_HELICOPTER_SLOW_DEATH,
    TAG_JET_SLOW_DEATH,
    TAG_MINE,
    TAG_SPAWN_BEHAVIOR,
    TAG_FIRING_TRACKER,
    TAG_FIRE_OCL_AFTER_COOLDOWN,
    TAG_ASSAULT_TRANSPORT,
    TAG_DEPLOY_STYLE,
    TAG_COMMAND_BUTTON_HUNT,
    TAG_FIRE_WEAPON_WHEN_DEAD,
    TAG_CREATE_OBJECT_DIE_TRANSFER,
    TAG_SPECIAL_POWER_COOLDOWNS,
    TAG_WEAPON_LOCK,
    TAG_EMOTICON_SURRENDER,
    TAG_PROJECTILE_FLIGHT,
    TAG_ACTIVE_BODY,
    TAG_PHYSICS_BEHAVIOR,
    TAG_RAILROAD,
];
