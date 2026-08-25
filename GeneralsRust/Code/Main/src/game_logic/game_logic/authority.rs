//! Mechanical split from `game_logic/game_logic.rs`. No behavior change.
#![allow(non_snake_case, unused_imports, dead_code)]
use super::construct::*;
use super::crate_tick::*;
use super::host::*;
use super::player::*;
use super::prelude::*;
use super::script_camera::*;
use super::*;

impl GameLogic {
    pub(super) fn seed_sample_objectives() -> Vec<ObjectiveDisplay> {
        vec![
            ObjectiveDisplay {
                id: Some("sample_primary".to_string()),
                title: localization::localize("objectives.primary.sample.title", "Secure the Area"),
                description: localization::localize(
                    "objectives.primary.sample.desc",
                    "Capture all nearby resource points.",
                ),
                status: ObjectiveStatus::Active,
                progress: Some((0, 3)),
                category: ObjectiveCategory::Primary,
            },
            ObjectiveDisplay {
                id: Some("sample_secondary".to_string()),
                title: localization::localize(
                    "objectives.secondary.sample.title",
                    "Bonus: Destroy Radar",
                ),
                description: localization::localize(
                    "objectives.secondary.sample.desc",
                    "Take out the enemy radar installation.",
                ),
                status: ObjectiveStatus::Completed,
                progress: None,
                category: ObjectiveCategory::Secondary,
            },
        ]
    }
}

#[derive(Debug)]
pub(super) struct DestructionEvent {
    pub(super) id: ObjectId,
    pub(super) killer: Option<Team>,
}

/// An authoritative Gather command that the command executor accepted.
///
/// This is intentionally a narrow simulation event rather than an input
/// claim: Main attaches physical mouse provenance only while matching the
/// exact command fingerprint after it has succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedGatherCommand {
    pub(crate) command_id: u32,
    pub(crate) issued_at: SystemTime,
    pub(crate) player_id: u32,
    pub(crate) target_id: ObjectId,
    /// Only carriers whose Gather path assignment succeeded.
    pub(crate) carrier_ids: Vec<ObjectId>,
}

/// A real carried-supply deposit performed by `AIState::ReturningResources`.
///
/// It does not represent passive income, resource-total deltas, or an order
/// request.  Consumers can therefore match the carrier and credited player
/// directly without inferring an economy source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SupplyDropoffEvent {
    pub(crate) carrier_id: ObjectId,
    pub(crate) player_id: u32,
    pub(crate) carried_amount: u32,
}

impl Default for GameLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLogic {
    pub(super) const MAX_PENDING_GATHER_EVENTS: usize = 64;
    pub(super) const MAX_PENDING_SUPPLY_DROPOFF_EVENTS: usize = 128;

    /// Record the exact carrier subset accepted by `CommandExecutor::execute_gather`.
    /// The bounded queue is an observation edge only; it never affects command
    /// execution or resource simulation.
    pub(super) fn record_accepted_gather_command(&mut self, event: AcceptedGatherCommand) {
        if event.carrier_ids.is_empty() {
            return;
        }
        if self.accepted_gather_commands.len() >= Self::MAX_PENDING_GATHER_EVENTS {
            self.accepted_gather_commands.pop_front();
        }
        self.accepted_gather_commands.push_back(event);
    }

    /// Consume Gather acceptances observed since the previous Main input edge.
    pub(crate) fn take_accepted_gather_commands(&mut self) -> Vec<AcceptedGatherCommand> {
        self.accepted_gather_commands.drain(..).collect()
    }

    /// Record a positive carried-resource deposit after the owning player's
    /// `credit_supplies` call has succeeded.
    pub(super) fn record_supply_dropoff_event(&mut self, event: SupplyDropoffEvent) {
        if event.carried_amount == 0 {
            return;
        }
        if self.supply_dropoff_events.len() >= Self::MAX_PENDING_SUPPLY_DROPOFF_EVENTS {
            self.supply_dropoff_events.pop_front();
        }
        self.supply_dropoff_events.push_back(event);
    }

    /// Consume real ReturningResources deposits for host-side presentation and
    /// physical-input evidence.  No economy totals are synthesized here.
    pub(crate) fn take_supply_dropoff_events(&mut self) -> Vec<SupplyDropoffEvent> {
        self.supply_dropoff_events.drain(..).collect()
    }
}

impl GameLogic {
    pub(super) fn load_campaign_objectives(&self, map_name: &str) -> Vec<ObjectiveDisplay> {
        let Some(manager) = &self.campaign_manager else {
            return Self::seed_sample_objectives();
        };

        let Ok(guard) = manager.lock() else {
            log::warn!(
                "Campaign manager unavailable while loading objectives for '{}'",
                map_name
            );
            return Self::seed_sample_objectives();
        };

        // Path-stem + short-name match (MD_USA01 ↔ .../MD_USA01.map); prefer
        // missions that actually define objectives (Campaign.ini residual table).
        let Some(mission) = guard.find_mission_for_map(map_name) else {
            log::info!(
                "No campaign mission metadata found for map '{}'; using sample objectives",
                map_name
            );
            return Self::seed_sample_objectives();
        };

        let mut displays = Vec::new();
        for (category, list) in [
            (ObjectiveCategory::Primary, &mission.primary_objectives),
            (ObjectiveCategory::Secondary, &mission.secondary_objectives),
            (ObjectiveCategory::Bonus, &mission.bonus_objectives),
        ] {
            for obj in list.iter() {
                displays.push(mission_objective_to_display(obj, category));
            }
        }

        if displays.is_empty() {
            log::warn!(
                "Mission '{}' ({}) does not define objectives; falling back to samples",
                mission.name,
                mission.id
            );
            Self::seed_sample_objectives()
        } else {
            log::info!(
                "Loaded {} mission objectives for '{}' ({})",
                displays.len(),
                mission.name,
                mission.id
            );
            displays
        }
    }

    pub(super) fn rebuild_objective_lookup(&mut self) {
        self.objective_lookup.clear();
        for (idx, objective) in self.mission_objectives.iter().enumerate() {
            if let Some(id) = &objective.id {
                self.objective_lookup.insert(id.to_ascii_lowercase(), idx);
            }
        }
    }

    pub(super) fn with_objective_mut<F>(&mut self, objective_id: &str, mut f: F) -> bool
    where
        F: FnMut(&mut ObjectiveDisplay),
    {
        let key = objective_id.to_ascii_lowercase();
        if let Some(&index) = self.objective_lookup.get(&key) {
            if let Some(objective) = self.mission_objectives.get_mut(index) {
                f(objective);
                return true;
            }
        } else {
            log::debug!("Objective '{}' not found in current mission", objective_id);
        }
        false
    }

    pub fn set_objective_status(&mut self, objective_id: &str, status: ObjectiveStatus) -> bool {
        self.with_objective_mut(objective_id, |objective| objective.status = status)
    }

    pub fn set_objective_progress(
        &mut self,
        objective_id: &str,
        current: u32,
        total: Option<u32>,
    ) -> bool {
        self.with_objective_mut(objective_id, |objective| {
            objective.progress = total.map(|goal| (current.min(goal), goal));
        })
    }

    pub fn mark_objective_completed(&mut self, objective_id: &str) -> bool {
        self.set_objective_status(objective_id, ObjectiveStatus::Completed)
    }

    pub fn mark_objective_failed(&mut self, objective_id: &str) -> bool {
        self.set_objective_status(objective_id, ObjectiveStatus::Failed)
    }

    /// Current mission objective displays (campaign residual / UI snapshot source).

    /// Upsert a mission objective residual for presentation/UI freeze.
    pub fn upsert_mission_objective(&mut self, objective: crate::ui::objectives::ObjectiveDisplay) {
        if let Some(id) = objective.id.as_ref() {
            let key = id.to_ascii_lowercase();
            if let Some(&idx) = self.objective_lookup.get(&key) {
                if let Some(slot) = self.mission_objectives.get_mut(idx) {
                    *slot = objective;
                    return;
                }
            }
            self.mission_objectives.push(objective);
            let idx = self.mission_objectives.len().saturating_sub(1);
            self.objective_lookup.insert(key, idx);
        } else {
            self.mission_objectives.push(objective);
        }
    }

    pub fn mission_objectives(&self) -> &[ObjectiveDisplay] {
        &self.mission_objectives
    }

    /// Number of mission scripts currently installed from the last map load.
    pub fn installed_mission_script_count(&self) -> usize {
        self.mission_script_count()
    }
}

/// Wave 930: host direct player-order authority payload.
#[derive(Debug, Clone, Copy)]
pub enum DirectPlayerOrder {
    Attack { player_id: u32, target: ObjectId },
    Stop { player_id: u32 },
    Move { player_id: u32, dest: glam::Vec3 },
    AttackMove { player_id: u32, dest: glam::Vec3 },
}

/// Wave 931: object lifecycle authority payload (create/destroy/prod/path/guard).
#[derive(Debug, Clone)]
pub enum ObjectLifecycleOp {
    Create {
        name: String,
        team: Team,
        spawn_at: glam::Vec3,
    },
    Destroy {
        id: ObjectId,
    },
    ForceCompleteConstruction {
        id: ObjectId,
    },
    ClearMovementPath {
        id: ObjectId,
    },
    AdjustGuardRadius {
        id: ObjectId,
        delta: f32,
    },
    EnqueueProduction {
        producer: ObjectId,
        template_name: String,
    },
    CancelProduction {
        id: ObjectId,
        template_name: String,
    },
    /// Cancel the exact queue slot selected by the Control Bar.
    ///
    /// The UI identifies queue entries by index, so collapsing this to a
    /// template-name lookup can cancel an earlier duplicate instead of the
    /// clicked item.
    CancelProductionAtIndex {
        id: ObjectId,
        queue_index: usize,
    },
}

/// Wave 931: heterogeneous result for [`ObjectLifecycleOp`].
#[derive(Debug, Clone, Copy)]
pub enum ObjectLifecycleResult {
    Created(Option<ObjectId>),
    Bool(bool),
    Radius(Option<f32>),
    Destroyed,
}

/// Wave 932: command-pipeline authority payload (queue/process).
#[derive(Debug, Clone)]
pub enum CommandPipelineOp {
    Queue {
        command: crate::command_system::GameCommand,
    },
    QueueAndProcess {
        command: crate::command_system::GameCommand,
    },
    ProcessIfNeeded,
}

/// Wave 933: session-control authority payload (select/pause/start/reset/camera/world).
#[derive(Debug, Clone)]
pub enum SessionControlOp {
    SelectObjects {
        player_id: u32,
        ids: Vec<ObjectId>,
    },
    SetPaused {
        paused: bool,
    },
    SetCameraFollow {
        id: Option<ObjectId>,
    },
    StartNewGameWithFaction {
        mode: GameMode,
        player_id: u32,
        faction_team: Team,
        setup_skirmish_ai: bool,
    },
    /// Selected Campaign/Challenge PlayerTemplate start.  This is separate
    /// from the Team-only compatibility path so ordinary/skirmish callers
    /// cannot accidentally inherit a stale General identity.
    StartNewGameWithPlayerTemplate {
        mode: GameMode,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    },
    Reset,
    OverrideWorldSize {
        width: f32,
        height: f32,
    },
}

/// Wave 934: host-support residual authority payload (barracks/supplies/shell/destroy/template).
#[derive(Debug, Clone)]
pub enum HostSupportOp {
    EnsureBarracksBuildingData {
        id: ObjectId,
    },
    ForceEnsureBarracksBuildingData {
        id: ObjectId,
    },
    EnsurePlayerMinSupplies {
        player_id: u32,
        floor: u32,
    },
    UpdateShellWithBudget {
        dt: f32,
        budget: usize,
    },
    ProcessDestroyListIfNeeded,
    InsertThingTemplate {
        name: String,
        template: ThingTemplate,
    },
}

/// Wave 934: heterogeneous result for [`HostSupportOp`].
#[derive(Debug, Clone, Copy)]
pub enum HostSupportResult {
    Bool(bool),
    Snapshot(SimTimingSnapshot),
    Unit,
}

/// Wave 937: production complete/spawn authority payload.
#[derive(Debug, Clone)]
pub enum ProductionAuthorityOp {
    /// Sole-tick path: collect ready producers after GW writeback, then apply.
    ApplyCompletionsAfterReadyWriteback { dt: f32 },
    /// Spawn one completed unit (ObjectId host bind residual).
    SpawnUnit {
        template: String,
        team: Team,
        /// Exact producer owner when known. `team` is retained for template
        /// and legacy payloads, not same-faction ownership selection.
        owner_player_id: Option<u32>,
        spawn_pos: glam::Vec3,
    },
    /// Drain production spawn-ready log into host door/notify residuals.
    ApplySpawnReadyCompletions,
    /// Drain production door-ready log into host door model residuals.
    ApplyDoorReadyCompletions,
}

/// Wave 937: result for [`ProductionAuthorityOp`].
#[derive(Debug, Clone, Copy)]
pub enum ProductionAuthorityResult {
    Unit,
    Spawned(Option<ObjectId>),
}

/// Wave 938: post-writeback sole-tick complete authority (construction/sell/SP).
#[derive(Debug, Clone, Copy)]
pub enum PostWritebackCompleteOp {
    /// Wave 715: construction complete after GW construction writeback.
    ConstructionCompletionsAfterReadyWriteback,
    /// Wave 716: sell finish after GW construction/sell writeback.
    SellCompletionsAfterReadyWriteback,
    /// Wave 717: special-power ready EVA after GW SP writeback.
    SpecialPowerReadyAfterWriteback,
}

/// Wave 939: post-writeback ready-log drain authority payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyLogDrainOp {
    Contain,
    Projectiles,
    AttackTarget,
    AiState,
    Movement,
    FireIntent,
    MoveTarget,
    Transform,
    Locomotor,
    AiRequest,
    Hijacker,
    PhysicsMotive,
    BounceLand,
    CombatStatus,
    BodyDamage,
    DeathType,
    RadarExtend,
    ShockStun,
    ConstructionCompleteClear,
    SoleHealing,
    AiMood,
    Owner,
    Veterancy,
    WeaponBonus,
    FaerieFire,
    Repulsor,
    DisableTimers,
    WeaponSlot,
    EntityPower,
    Turret,
    StealthDelay,
    CombatAttack,
    TargetLocation,
    Detector,
    ContinuousFire,
    Guard,
    AiAttitude,
    WeaponSet,
    Overcharge,
    Hive,
    StealthFlags,
    Overlord,
    CommandSet,
    Disguise,
    VisionCamo,
    WeaponStats,
    SelectionRadius,
    ModelCondition,
    DemoMineCheer,
    CrushVision,
    BuildingType,
    Identity,
    GroundHeight,
    Economy,
    Upgrade,
    StoredSupplies,
}

/// Wave 940: post-writeback sole-tick batch is a single authority call (no enum).
/// Host ObjectId mutations used by shadow residual drains.
#[derive(Debug, Clone)]
pub enum HostObjectIdOp {
    MarkForDestruction {
        id: ObjectId,
        team: Option<Team>,
    },
    Create {
        template: String,
        team: Team,
        spawn_at: glam::Vec3,
    },
}

/// Wave 940: result for [`HostObjectIdOp`].
#[derive(Debug, Clone, Copy)]
pub enum HostObjectIdResult {
    Unit,
    Created(Option<ObjectId>),
}

/// Wave 941/942: host residual mutation payload (poison/kill/pending fire/expire/field).
#[derive(Debug, Clone)]
pub enum HostResidualMutationOp {
    /// PoisonedBehavior DoT — UNRESISTABLE typed death.
    PoisonDot {
        object: ObjectId,
        amount: f32,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    },
    /// Force HP to 0 / destroyed (+ optional death_type / model refresh).
    ForceKill {
        id: ObjectId,
        death_type: Option<crate::game_logic::host_usa_pilot::HostDeathType>,
        refresh_model_condition: bool,
        mark_destroy: bool,
    },
    /// FireWeaponWhenDamaged pending weapon residual.
    SetPendingFireWhenDamaged {
        id: ObjectId,
        weapon: String,
        /// When false, only set if pending is currently None (continuous).
        overwrite: bool,
    },
    /// Projectile/field expire: optional damage-log lethal, flags, position, mark-destroy.
    LethalExpire {
        id: ObjectId,
        position: Option<glam::Vec3>,
        effectively_dead: bool,
        clear: ObjectIdentityClear,
        /// None => do not mark; Some(team_opt) => MarkForDestruction with team.
        mark_destroy_team: Option<Option<Team>>,
    },
    /// Bomb/mine payload residual destroy (no damage-log path).
    DestroyBomb { id: ObjectId, mark_destroy: bool },
    /// Model condition bits residual (actively constructing).
    SetModelConditionBits {
        id: ObjectId,
        bits: u128,
        count_update: bool,
    },
    /// Power plant control rods completion residual.
    PowerPlantRodsComplete {
        id: ObjectId,
        model_condition_bits: u128,
    },
    /// Horde weapon-bonus residual (Battlemaster / China infantry).
    SetWeaponBonusHorde {
        id: ObjectId,
        now_horde: bool,
        was_horde: bool,
        grant: HordeGrantCounter,
    },
    /// Stinger hive slave residual snapshot.
    ApplyStingerHiveState {
        id: ObjectId,
        hive_slave_count: u8,
        hive_slave_hp: f32,
        hive_slave_respawn_frame: u32,
        slaves_alive: [bool; 3],
        slaves_hp: [f32; 3],
    },
    /// Sticky/booby follow position residual.
    SetPosition {
        id: ObjectId,
        position: glam::Vec3,
        sticky_follow_tick: bool,
    },
    /// Post-create flight payload config (producer, identity flag, velocity, target).
    ConfigureSpawnedPayload {
        id: ObjectId,
        producer: ObjectId,
        target: glam::Vec3,
        kind: SpawnedPayloadKind,
    },
    /// Host-only raw HP damage for combat events with no shadow entity mapping.
    /// `amount` is already post-armor — do not re-run armor/log.
    ApplyRawHpDamage { id: ObjectId, amount: f32 },
}

/// Wave 942: which projectile/identity flag to clear on lethal expire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectIdentityClear {
    None,
    FlashbangGrenadeProjectile,
    ScorpionMissileProjectile,
    SpySatellitePing,
    AngryMobMember,
    AuroraBombProjectile,
    InfernoShellProjectile,
    ToxinStreamProjectile,
    AngryMobProjectile,
    /// Clears scud/neutron/nuke cannon shell projectile flags.
    CannonShellProjectile,
    LeafletContainer,
    ParadropCargo,
    ComancheRocketPodProjectile,
    EmpPulseSpheroid,
    FieldObject(crate::game_logic::host_field_object_expire_log::FieldObjectKind),
}

/// Wave 942: which residual counter to bump on horde grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HordeGrantCounter {
    #[default]
    None,
    Battlemaster,
    RedGuard,
    TankHunter,
    Minigunner,
}

/// Wave 942: post-create flight payload identity/config residual.
#[derive(Debug, Clone)]
pub enum SpawnedPayloadKind {
    DaisyCutter { moab_template: Option<String> },
    AnthraxBomb,
    ClusterMinesBomb,
    EmpPulseBomb,
    A10StrikeMissile,
    ArtilleryBarrageShell,
    CarpetBomb,
    LeafletContainer,
    ParadropParachute,
}

/// Wave 944: shadow→host writeback payload (core combat/pose channels).
#[derive(Debug, Clone)]
pub enum HostWritebackOp {
    /// Health/max + optional destroy bookkeeping fields.
    Health {
        id: ObjectId,
        current: f32,
        maximum: f32,
        /// When true, mark destroyed + clear AI target.
        destroy: bool,
    },
    /// Experience points and/or veterancy level.
    Experience {
        id: ObjectId,
        points: Option<f32>,
        level: Option<crate::game_logic::VeterancyLevel>,
    },
    /// World pose last-write.
    Transform {
        id: ObjectId,
        position: glam::Vec3,
        orientation: f32,
    },
    /// Attack target last-write (direct assign; ready-log handles residuals).
    AttackTarget {
        id: ObjectId,
        target: Option<ObjectId>,
        clear_target_location: bool,
    },
    /// Move destination last-write.
    MoveTarget {
        id: ObjectId,
        destination: Option<glam::Vec3>,
    },
    /// Wave 945: AI state ordinal last-write.
    AiState { id: ObjectId, ordinal: u8 },
    /// Wave 945: AI attitude last-write.
    AiAttitude { id: ObjectId, attitude: i8 },
    /// Wave 945: owner last-write. `team` remains faction/presentation state;
    /// `owner_player_id` distinguishes same-faction skirmish slots.
    Owner {
        id: ObjectId,
        team: Team,
        team_color: [f32; 4],
        owner_player_id: Option<u32>,
    },
    /// Wave 945: special-power ready/cooldown last-write.
    SpecialPower {
        id: ObjectId,
        ready: bool,
        cooldown_remaining: f32,
        cooldown: f32,
    },
    /// Wave 945: overcharge flag last-write.
    Overcharge { id: ObjectId, enabled: bool },
    /// Wave 945: active weapon slot last-write.
    WeaponSlot { id: ObjectId, slot: u8 },
    /// Wave 945: selection radius last-write.
    SelectionRadius { id: ObjectId, radius: f32 },
    /// Wave 945: entity power provided/consumed last-write.
    EntityPower {
        id: ObjectId,
        provided: i32,
        consumed: i32,
    },
    /// Wave 945: target location last-write.
    TargetLocation {
        id: ObjectId,
        location: Option<glam::Vec3>,
    },
    /// Wave 945: command-set override last-write.
    CommandSet {
        id: ObjectId,
        override_name: Option<String>,
    },
    /// Wave 945: ground height last-write.
    GroundHeight {
        id: ObjectId,
        height: f32,
        from_terrain: bool,
    },
    /// Wave 945: body damage state last-write.
    BodyDamage {
        id: ObjectId,
        state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    },
    /// Wave 945: death type last-write.
    DeathType {
        id: ObjectId,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    },
    /// Wave 945: stored supplies last-write.
    StoredSupplies { id: ObjectId, supplies: u32 },
    /// Wave 945: faerie-fire status last-write.
    FaerieFire {
        id: ObjectId,
        active: bool,
        until_frame: u32,
    },
    /// Wave 945: repulsor status last-write.
    Repulsor {
        id: ObjectId,
        active: bool,
        until_frame: u32,
    },
    /// Wave 945: detector residual last-write.
    Detector {
        id: ObjectId,
        is_detector: bool,
        range: f32,
        rate_frames: u32,
    },
    /// Wave 945: guard residual last-write.
    Guard {
        id: ObjectId,
        position: Option<glam::Vec3>,
        target: Option<ObjectId>,
        radius: f32,
    },
}
