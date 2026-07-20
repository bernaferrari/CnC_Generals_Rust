//! Lightweight world representation used by the modern game-logic core.

pub mod entities;

use self::entities::{EntityId, EntityProductionItem, EntityStore, TemplateRef, Transform};
use std::collections::VecDeque;
use std::fmt;
use std::mem;

/// Identifier assigned to players.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerId(u8);

impl PlayerId {
    /// First playable slot.
    pub const FIRST: PlayerId = PlayerId(0);
    /// Neutral/observer slot.
    pub const NEUTRAL: PlayerId = PlayerId(0);

    /// Construct from a dense slot index (shadow/mirror helpers).
    pub fn from_index(idx: u8) -> Self {
        PlayerId(idx)
    }

    /// Raw numeric value.
    pub fn get(self) -> u8 {
        self.0
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get())
    }
}

/// Publicly visible player information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    /// Player identifier.
    pub id: PlayerId,
    /// Display name.
    pub name: String,
    /// Team slot (if any).
    pub team: Option<u8>,
    /// Whether the player is controlled by a human.
    pub is_human: bool,
    /// Cash/supplies mirror.
    pub supplies: u32,
    /// Power available residual.
    pub power_available: i32,
}

/// Immutable snapshot of the world state for deterministic consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldSnapshot {
    /// Frame index captured when the snapshot was produced.
    pub frame: u64,
    /// Present players in the simulation.
    pub players: Vec<PlayerInfo>,
    /// Active entities at the time of the snapshot.
    pub entities: Vec<EntitySummary>,
}

/// Summary information about an entity included in world snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySummary {
    /// Entity identifier.
    pub id: EntityId,
    /// Template name backing the entity.
    pub template: String,
    /// Owning player (if any).
    pub owner: Option<PlayerId>,
    /// World-space position.
    pub position: [f32; 3],
    /// Facing angle in radians.
    pub orientation: f32,
    /// Remaining hitpoints.
    pub health: f32,
}

/// Internal record stored for every allocated player.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerData {
    /// Display name.
    pub name: String,
    /// Team slot (if any).
    pub team: Option<u8>,
    /// Indicates whether the player is human-controlled.
    pub is_human: bool,
    /// Cash/supplies (host Player::resources.supplies mirror).
    pub supplies: u32,
    /// Power available residual (host power_available).
    pub power_available: i32,
    /// Host Player::power_produced residual (energy bar supply side).
    pub power_produced: i32,
    /// Host Player::power_consumed residual (energy bar demand side).
    pub power_consumed: i32,
    /// Completed upgrade names residual (host HostUpgradeRegistry complete channel).
    /// Fail-closed: not full PlayerUpgradeManager / science tree parity.
    pub completed_upgrades: Vec<String>,
    /// Unlocked science names residual (host Player::unlocked_sciences).
    /// Fail-closed: not full science store / rank / purchase matrix.
    pub unlocked_sciences: Vec<String>,
    /// Host Player::radar_count residual (CommandCenter / RadarVan providers).
    pub radar_count: i32,
    /// Host Player::radar_disabled residual (script/power disable).
    pub radar_disabled: bool,
    /// Host Player::is_alive residual (defeat / victory conditions).
    pub is_alive: bool,
    /// Host Player::cash_bounty_percent residual (GLA science bounty 0..1).
    pub cash_bounty_percent: f32,
    /// Host Player::color_rgb residual (team tint / UI).
    pub color_rgb: (u8, u8, u8),
    /// Host Player::rank_level residual (1-based GeneralsExperience).
    pub rank_level: u32,
    /// Host Player::skill_points residual.
    pub skill_points: i32,
    /// Host Player::science_purchase_points residual.
    pub science_purchase_points: i32,
    /// Host Player::shared_special_power_cooldowns residual.
    /// Keys are `SpecialPowerType` Debug names; values are seconds remaining.
    pub shared_special_power_cooldowns: Vec<(String, f32)>,
}

impl PlayerData {
    /// Convert the stored record into a publicly shareable snapshot.
    pub fn to_info(&self, id: PlayerId) -> PlayerInfo {
        PlayerInfo {
            id,
            name: self.name.clone(),
            team: self.team,
            is_human: self.is_human,
            supplies: self.supplies,
            power_available: self.power_available,
        }
    }
}

#[derive(Debug, Clone)]
enum PlayerState {
    Vacant,
    Active(PlayerData),
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState::Vacant
    }
}

#[derive(Debug, Clone)]
struct PlayerSlot {
    id: PlayerId,
    state: PlayerState,
}

impl PlayerSlot {
    fn is_active(&self) -> bool {
        matches!(self.state, PlayerState::Active(_))
    }

    fn summary(&self) -> Option<PlayerInfo> {
        match &self.state {
            PlayerState::Active(data) => Some(data.to_info(self.id)),
            PlayerState::Vacant => None,
        }
    }

    fn activate(&mut self, data: PlayerData) {
        self.state = PlayerState::Active(data);
    }

    fn deactivate(&mut self) -> Option<PlayerData> {
        match mem::replace(&mut self.state, PlayerState::Vacant) {
            PlayerState::Active(data) => Some(data),
            PlayerState::Vacant => None,
        }
    }

    fn data(&self) -> Option<&PlayerData> {
        match &self.state {
            PlayerState::Active(data) => Some(data),
            PlayerState::Vacant => None,
        }
    }

    fn data_mut(&mut self) -> Option<&mut PlayerData> {
        match &mut self.state {
            PlayerState::Active(data) => Some(data),
            PlayerState::Vacant => None,
        }
    }
}

/// Minimal world storage.
#[derive(Debug)]
pub struct World {
    frame: u64,
    slots: Vec<PlayerSlot>,
    available_ids: VecDeque<PlayerId>,
    entities: EntityStore,
}

impl World {
    /// Create a new world supporting the requested number of players.
    pub fn new(max_players: usize) -> Self {
        let mut slots = Vec::with_capacity(max_players);
        let mut available_ids = VecDeque::with_capacity(max_players);
        for idx in 0..max_players.min(255) {
            let id = PlayerId(idx as u8);
            slots.push(PlayerSlot {
                id,
                state: PlayerState::Vacant,
            });
            available_ids.push_back(id);
        }

        Self {
            frame: 0,
            slots,
            available_ids,
            entities: EntityStore::new(),
        }
    }

    /// Adjust the number of supported player slots.
    pub fn resize(&mut self, max_players: usize) {
        if max_players == self.slots.len() {
            return;
        }

        let mut new_self = World::new(max_players);
        for slot in self.slots.iter().filter(|slot| slot.is_active()) {
            if let Some(id) = new_self.allocate_player_internal() {
                if id == slot.id {
                    if let Some(entry) = new_self.slots.iter_mut().find(|s| s.id == id) {
                        if let Some(data) = slot.data() {
                            entry.activate(data.clone());
                        }
                    }
                }
            }
        }
        new_self.frame = self.frame;
        new_self.entities = self.entities.clone();
        *self = new_self;
    }

    /// Drop all entities while keeping players and frame.
    pub fn clear_entities(&mut self) {
        self.entities.clear();
    }

    /// Mark the simulation as having advanced to the next frame.
    pub fn advance(&mut self, frame: u32) {
        self.frame = frame as u64;
    }

    /// Allocate a player slot if available.
    pub fn allocate_player(&mut self) -> Option<PlayerId> {
        self.allocate_player_with_name(None, None, true)
    }

    /// Allocate a player slot with custom metadata.
    pub fn allocate_player_with_name(
        &mut self,
        name: Option<String>,
        team: Option<u8>,
        is_human: bool,
    ) -> Option<PlayerId> {
        let id = self.allocate_player_internal()?;
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            let player_name = name.unwrap_or_else(|| format!("Player {}", id.get() + 1));
            slot.activate(PlayerData {
                name: player_name,
                team,
                is_human,
                supplies: 0,
                power_available: 0,
                power_produced: 0,
                power_consumed: 0,
                completed_upgrades: Vec::new(),
                unlocked_sciences: Vec::new(),
                radar_count: 0,
                radar_disabled: false,
                is_alive: true,
                cash_bounty_percent: 0.0,
                color_rgb: (200, 200, 200),
                rank_level: 1,
                skill_points: 0,
                science_purchase_points: 0,
                shared_special_power_cooldowns: Vec::new(),
            });
        }
        Some(id)
    }

    fn allocate_player_internal(&mut self) -> Option<PlayerId> {
        self.available_ids.pop_front()
    }

    /// Remove a player. Returns a snapshot of the removed player if the slot
    /// was occupied.
    pub fn remove_player(&mut self, id: PlayerId) -> Option<PlayerInfo> {
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.id == id) {
            if let Some(data) = slot.deactivate() {
                let info = data.to_info(id);
                self.available_ids.push_back(id);
                return Some(info);
            }
        }
        None
    }

    /// Fetch immutable player data.
    pub fn player(&self, id: PlayerId) -> Option<&PlayerData> {
        self.slots
            .iter()
            .find(|slot| slot.id == id)
            .and_then(PlayerSlot::data)
    }

    /// Fetch mutable player data.
    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerData> {
        self.slots
            .iter_mut()
            .find(|slot| slot.id == id)
            .and_then(PlayerSlot::data_mut)
    }

    /// Iterate over active players alongside their data.
    pub fn active_players(&self) -> impl Iterator<Item = (PlayerId, &PlayerData)> {
        self.slots
            .iter()
            .filter_map(|slot| slot.data().map(|data| (slot.id, data)))
    }

    /// Number of active players currently occupying slots.
    pub fn active_player_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_active()).count()
    }

    /// Number of entities present in the world.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Produce an immutable view of the world.
    pub fn snapshot(&self) -> WorldSnapshot {
        let players = self.slots.iter().filter_map(PlayerSlot::summary).collect();
        let entities = self.entities.iter().map(World::entity_summary).collect();
        WorldSnapshot {
            frame: self.frame,
            players,
            entities,
        }
    }

    /// Spawn an entity in the world and return its identifier.
    pub fn spawn_entity(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> EntityId {
        self.entities.spawn(template, owner, transform, health)
    }

    /// Remove an entity; returns `true` if one was removed.
    pub fn remove_entity(&mut self, id: EntityId) -> bool {
        self.entities.remove(id).is_some()
    }

    /// Immutable access to an entity.
    pub fn entity(&self, id: EntityId) -> Option<&entities::Entity> {
        self.entities.get(id)
    }

    /// Mutable access to an entity (borrow-first phase code).
    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut entities::Entity> {
        self.entities.get_mut(id)
    }

    /// Iterate over all active entities.
    pub fn entities(&self) -> impl Iterator<Item = &entities::Entity> {
        self.entities.iter()
    }

    /// Snapshot information for a specific entity.
    pub fn entity_summary_by_id(&self, id: EntityId) -> Option<EntitySummary> {
        self.entities.get(id).map(World::entity_summary)
    }

    fn entity_summary(entity: &entities::Entity) -> EntitySummary {
        EntitySummary {
            id: entity.id,
            template: entity.template.name.clone(),
            owner: entity.owner,
            position: [
                entity.transform.position.x,
                entity.transform.position.y,
                entity.transform.position.z,
            ],
            orientation: entity.transform.orientation,
            health: entity.health,
        }
    }
}

/// Deferred world mutation collected during a phase, applied at phase end.
/// Borrow-checker friendly and deterministic when sorted by apply order.
#[derive(Debug, Clone, PartialEq)]
pub enum WorldMutation {
    /// Apply damage to an entity by id.
    Damage { target: EntityId, amount: f32 },
    /// Remove an entity (destroy).
    Destroy(EntityId),
    /// Set absolute health.
    SetHealth { target: EntityId, health: f32 },
    /// Host Object::max_health / health.maximum residual (armor / veterancy).
    SetMaxHealth {
        target: EntityId,
        max_health: f32,
    },
    /// Transfer ownership.
    TransferOwner {
        object: EntityId,
        player: Option<PlayerId>,
    },
    /// Set absolute player supplies (economy last-writer).
    SetSupplies { player: PlayerId, supplies: u32 },
    /// Set absolute player power_available residual.
    SetPower {
        player: PlayerId,
        power_available: i32,
    },
    /// Record a completed host upgrade on a player (shadow last-writer residual).
    CompleteUpgrade { player: PlayerId, name: String },
    /// Spawn a new entity (shadow/host spawn channel).
    Spawn {
        template: String,
        owner: Option<PlayerId>,
        position: [f32; 3],
        health: f32,
    },
    /// Set entity world position / orientation (move command channel).
    SetTransform {
        target: EntityId,
        position: [f32; 3],
        orientation: f32,
    },
    /// Set attack target entity id (command channel).
    SetAttackTarget {
        attacker: EntityId,
        target: Option<EntityId>,
    },
    /// Set move destination (command channel).
    SetMoveTarget {
        unit: EntityId,
        destination: Option<[f32; 3]>,
    },
    /// Set combat/status residual flags (borrow-first status channel).
    SetCombatStatus {
        target: EntityId,
        stealthed: Option<bool>,
        detected: Option<bool>,
        attacking: Option<bool>,
        moving: Option<bool>,
        is_firing_weapon: Option<bool>,
        is_aiming_weapon: Option<bool>,
        selected: Option<bool>,
        disabled_emp: Option<bool>,
        weapons_jammed: Option<bool>,
        disabled_hacked: Option<bool>,
        disabled_unmanned: Option<bool>,
        disabled_paralyzed: Option<bool>,
        disabled_subdued: Option<bool>,
        masked: Option<bool>,
        disguised: Option<bool>,
        no_collisions: Option<bool>,
        private_captured: Option<bool>,
        disguise_transitioning_to: Option<bool>,
        disguise_halfpoint_reached: Option<bool>,
        faerie_fire: Option<bool>,
        booby_trapped: Option<bool>,
        eject_invulnerable: Option<bool>,
        pilot_did_move_to_base: Option<bool>,
        parachuting: Option<bool>,
        parachute_open: Option<bool>,
        parachute_landing_override_set: Option<bool>,
        using_ability: Option<bool>,
        deployed: Option<bool>,
        under_construction: Option<bool>,
        sold: Option<bool>,
        reconstructing: Option<bool>,
        unselectable: Option<bool>,
        ignoring_stealth: Option<bool>,
        repulsor: Option<bool>,
        disabled_underpowered: Option<bool>,
        disabled_freefall: Option<bool>,
        is_carbomb: Option<bool>,
        hijacked: Option<bool>,
        force_attack: Option<bool>,
    },
    /// Set veterancy ordinal residual (0 Rookie .. 3 Heroic).
    SetVeterancy { target: EntityId, ordinal: u8 },
    /// Host Object::experience.current residual.
    SetExperience {
        target: EntityId,
        points: f32,
    },
    /// Host Object weapon-bonus residual pack (propaganda/horde/nationalism/frenzy/battle plan).
    SetWeaponBonus {
        target: EntityId,
        enthusiastic: bool,
        subliminal: bool,
        horde: bool,
        nationalism: bool,
        frenzy: bool,
        frenzy_level: u8,
        battle_plan_bombardment: bool,
        battle_plan_hold_the_line: bool,
        battle_plan_search_and_destroy: bool,
    },
    /// Host Object::active_weapon_slot residual (0 primary, 1 secondary, …).
    SetActiveWeaponSlot {
        target: EntityId,
        slot: u8,
    },
    /// Host Object::power_provided / power_consumed residual (plant overcharge / rods).
    SetEntityPower {
        target: EntityId,
        power_provided: i32,
        power_consumed: i32,
    },
    /// Host Object turret residual (angle/pitch/holding/idle-scan).
    SetTurret {
        target: EntityId,
        angle_deg: f32,
        pitch_deg: f32,
        holding: bool,
        idle_scanning: bool,
    },
    /// Host Object::target_location residual (ground attack aim point).
    SetTargetLocation {
        unit: EntityId,
        location: Option<[f32; 3]>,
    },
    /// Host Object detector residual (is_detector / range / rate).
    SetDetector {
        target: EntityId,
        is_detector: bool,
        detection_range: f32,
        detection_rate_frames: u32,
    },
    /// Host Object continuous-fire residual (gattling/minigun spin-up).
    SetContinuousFire {
        target: EntityId,
        level: u8,
        consecutive: u16,
        coast_until_frame: u32,
    },
    /// Host Object guard residual (area position + guarded object id).
    SetGuard {
        unit: EntityId,
        position: Option<[f32; 3]>,
        target_host: u32,
    },
    /// Host Object::ai_attitude residual (-2 Sleep .. +2 Aggressive).
    SetAiAttitude {
        target: EntityId,
        attitude: i8,
    },
    /// Host Object weapon-set residual flags (player upgrade / armed riders).
    SetWeaponSetFlags {
        target: EntityId,
        player_upgrade: bool,
        armed_riders: bool,
    },
    /// Host Object::overcharge_enabled residual (China power plant).
    SetOvercharge {
        target: EntityId,
        enabled: bool,
    },
    /// Host Object contain capacity residual (transport slots / garrison max).
    SetContainCapacity {
        target: EntityId,
        max_transport: usize,
        max_garrison: u16,
    },
    /// Host Object hive-slave residual (Stinger site soldier pool).
    SetHiveSlaves {
        target: EntityId,
        slave_count: u8,
        slave_hp: f32,
    },
    /// Host Object stealth/tunnel/passenger-fire residual flags.
    SetStealthFlags {
        target: EntityId,
        innate_stealth: bool,
        stealth_breaks_on_attack: bool,
        stealth_breaks_on_move: bool,
        is_tunnel_network: bool,
        passengers_allowed_to_fire: bool,
    },
    /// Host Object Overlord/Helix addon residual.
    SetOverlordAddon {
        target: EntityId,
        has_gattling: bool,
        has_propaganda: bool,
        /// `u16::MAX` = host Option::None bunker residual.
        bunker_capacity: u16,
        is_helix_transport: bool,
    },
    /// Host Object::command_set_override residual (empty = none).
    SetCommandSet {
        target: EntityId,
        command_set: String,
    },
    /// Host Object disguise residual (empty template = none; team 255 = none).
    SetDisguise {
        target: EntityId,
        template: String,
        team_ordinal: u8,
    },
    /// Host Object vision_spied_mask + camo residual.
    SetVisionCamo {
        target: EntityId,
        vision_spied_mask: u32,
        camo_friendly_opacity: f32,
        camo_stealth_look: u8,
    },
    /// Host Object primary/secondary weapon stats residual.
    SetWeaponStats {
        target: EntityId,
        has_weapon: bool,
        weapon_damage: f32,
        weapon_range: f32,
        weapon_min_range: f32,
        weapon_reload_time: f32,
        weapon_ammo: u32,
        weapon_can_target_air: bool,
        weapon_can_target_ground: bool,
        weapon_projectile_speed: f32,
        has_secondary_weapon: bool,
        secondary_weapon_damage: f32,
        secondary_weapon_range: f32,
    },
    /// Host Movement velocity/path residual.
    SetMovement {
        target: EntityId,
        velocity: [f32; 3],
        max_speed: f32,
        path_index: u16,
        path_len: u16,
        path_waypoints: Vec<[f32; 3]>,
    },
    /// Host Object::selection_radius residual.
    SetSelectionRadius {
        target: EntityId,
        selection_radius: f32,
    },
    /// Host Object::model_condition_bits residual.
    SetModelCondition {
        target: EntityId,
        model_condition_bits: u128,
    },
    /// Host demo-suicide / mine-present / cheer-timer residual.
    SetDemoMineCheer {
        target: EntityId,
        demo_suicided_detonating: bool,
        has_mine_data: bool,
        cheer_timer: f32,
    },
    /// Host crush levels + vision/shroud ranges residual.
    SetCrushVision {
        target: EntityId,
        crusher_level: u8,
        crushable_level: u8,
        vision_range: f32,
        shroud_clearing_range: f32,
    },
    /// Host Object building_data present + BuildingType ordinal residual.
    SetBuildingType {
        target: EntityId,
        is_building: bool,
        building_type_ordinal: u8,
    },
    /// Host Object name + team_color residual (presentation identity).
    SetIdentity {
        target: EntityId,
        name: String,
        team_color: [f32; 4],
    },
    /// Host/terrain ground height residual at object XY.
    SetGroundHeight {
        target: EntityId,
        ground_height: f32,
        from_terrain: bool,
    },
    /// Replace entity production queue residual (borrow-first production channel).
    SetProductionQueue {
        target: EntityId,
        items: Vec<EntityProductionItem>,
    },
    /// Set structure construction progress residual (0..1).
    SetConstruction {
        target: EntityId,
        percent: f32,
        under_construction: bool,
    },
    /// Set special-power ready residual on an entity.
    SetSpecialPower {
        target: EntityId,
        ready: bool,
        /// Aggregate remaining cooldown seconds (host special_power_cooldown_remaining).
        cooldown_remaining: f32,
        /// Full cooldown duration seconds (host special_power_cooldown).
        cooldown: f32,
    },
    /// Set unit/structure stored supplies residual (supply truck / dock cargo).
    SetStoredSupplies { target: EntityId, supplies: u32 },
    /// Set AI state ordinal residual (Idle=0 .. Capturing=19, GuardRetaliating=20).
    SetAiState { target: EntityId, ordinal: u8 },
    /// Set contain/garrison residual (passenger container + building roster).
    SetContain {
        target: EntityId,
        contained_by_host: u32,
        garrison_count: Option<u16>,
        garrisoned_host_ids: Option<Vec<u32>>,
    },
    /// Set player radar provider count / disabled residual.
    SetPlayerRadar {
        player: PlayerId,
        radar_count: i32,
        radar_disabled: bool,
    },
    /// Set player rank / skill / science / bounty residual.
    SetPlayerProgress {
        player: PlayerId,
        rank_level: u32,
        skill_points: i32,
        science_purchase_points: i32,
        cash_bounty_percent: f32,
    },
    /// Replace unlocked sciences residual for a player.
    SetPlayerSciences {
        player: PlayerId,
        unlocked_sciences: Vec<String>,
    },
    /// Set player alive residual (defeat / victory).
    SetPlayerAlive { player: PlayerId, is_alive: bool },
    /// Replace shared special-power cooldown residual for a player.
    SetPlayerCooldowns {
        player: PlayerId,
        cooldowns: Vec<(String, f32)>,
    },
}

/// Borrow-first façade over [`World`] — the target API shape for simulation code.
///
/// Policy: subsystems take `&mut GameWorld` (or `&GameWorld`) for one explicit phase.
/// Cross-object references use [`EntityId`] / [`PlayerId`], never long-lived borrows.
/// `Arc` is not part of this API surface.
#[derive(Debug)]
pub struct GameWorld {
    inner: World,
    pending: Vec<WorldMutation>,
    /// Most recent entity created via `WorldMutation::Spawn` (shadow ID map).
    last_spawned_entity: Option<EntityId>,
}

impl GameWorld {
    /// Create a world with the given player-slot capacity.
    pub fn new(max_players: usize) -> Self {
        Self {
            inner: World::new(max_players),
            pending: Vec::new(),
            last_spawned_entity: None,
        }
    }

    /// Take the entity id from the last applied Spawn mutation, if any.
    pub fn take_last_spawned_entity(&mut self) -> Option<EntityId> {
        self.last_spawned_entity.take()
    }

    /// Immutable access to the underlying world.
    pub fn world(&self) -> &World {
        &self.inner
    }

    /// Mutable access for phase code that still needs full World APIs.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.inner
    }

    pub fn frame(&self) -> u64 {
        self.inner.snapshot().frame
    }

    /// Set the world frame counter (no entity simulation).
    /// Used by shadow/mirror probes and deterministic test harnesses.
    pub fn set_frame(&mut self, frame: u64) {
        let clamped = frame.min(u32::MAX as u64) as u32;
        self.inner.advance(clamped);
    }

    /// Advance the world frame counter by `frames` (no entity simulation).
    pub fn advance_frames(&mut self, frames: u64) {
        if frames == 0 {
            return;
        }
        let next = self.frame().saturating_add(frames);
        self.set_frame(next);
    }

    pub fn entity(&self, id: EntityId) -> Option<&entities::Entity> {
        self.inner.entity(id)
    }

    pub fn player(&self, id: PlayerId) -> Option<&PlayerData> {
        self.inner.player(id)
    }

    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerData> {
        self.inner.player_mut(id)
    }

    /// Queue a mutation for end-of-phase apply (does not mutate entities yet).
    pub fn queue_mutation(&mut self, m: WorldMutation) {
        self.pending.push(m);
    }

    /// Apply all pending mutations in queue order. Returns how many succeeded.
    pub fn apply_pending_mutations(&mut self) -> usize {
        let pending = std::mem::take(&mut self.pending);
        let mut applied = 0;
        for m in pending {
            match m {
                WorldMutation::Damage { target, amount } => {
                    let kill = if let Some(e) = self.inner.entity_mut(target) {
                        e.health = (e.health - amount).max(0.0);
                        applied += 1;
                        e.health <= 0.0
                    } else {
                        false
                    };
                    if kill {
                        let _ = self.inner.remove_entity(target);
                    }
                }
                WorldMutation::Destroy(id) => {
                    if self.inner.remove_entity(id) {
                        applied += 1;
                    }
                }
                WorldMutation::SetHealth { target, health } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.health = health.max(0.0);
                        applied += 1;
                    }
                }
                WorldMutation::SetMaxHealth { target, max_health } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        let m = max_health.max(1.0);
                        e.max_health = m;
                        // Keep current health within new max (C++ armor/veterancy residual).
                        if e.health > m {
                            e.health = m;
                        }
                        applied += 1;
                    }
                }
                WorldMutation::TransferOwner { object, player } => {
                    if let Some(e) = self.inner.entity_mut(object) {
                        e.owner = player;
                        applied += 1;
                    }
                }
                WorldMutation::SetSupplies { player, supplies } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.supplies = supplies;
                        applied += 1;
                    }
                }
                WorldMutation::SetPower {
                    player,
                    power_available,
                } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.power_available = power_available;
                        applied += 1;
                    }
                }
                WorldMutation::CompleteUpgrade { player, name } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        if !p.completed_upgrades.iter().any(|u| u == &name) {
                            p.completed_upgrades.push(name);
                            // Keep deterministic order for probes/snapshots.
                            p.completed_upgrades.sort();
                        }
                        applied += 1;
                    }
                }

                WorldMutation::Spawn {
                    template,
                    owner,
                    position,
                    health,
                } => {
                    let id = self.inner.spawn_entity(
                        entities::TemplateRef::new(template),
                        owner,
                        entities::Transform::new(position, 0.0),
                        health.max(0.0),
                    );
                    self.last_spawned_entity = Some(id);
                    applied += 1;
                }
                WorldMutation::SetTransform {
                    target,
                    position,
                    orientation,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.transform = entities::Transform::new(position, orientation);
                        applied += 1;
                    }
                }
                WorldMutation::SetAttackTarget { attacker, target } => {
                    if let Some(e) = self.inner.entity_mut(attacker) {
                        e.attack_target = target;
                        applied += 1;
                    }
                }
                WorldMutation::SetMoveTarget { unit, destination } => {
                    if let Some(e) = self.inner.entity_mut(unit) {
                        e.move_target = destination;
                        applied += 1;
                    }
                }
                WorldMutation::SetCombatStatus {
                    target,
                    stealthed,
                    detected,
                    attacking,
                    moving,
                    is_firing_weapon,
                    is_aiming_weapon,
                    selected,
                    disabled_emp,
                    weapons_jammed,
                    disabled_hacked,
                    disabled_unmanned,
                    disabled_paralyzed,
                    disabled_subdued,
                    masked,
                    disguised,
                    no_collisions,
                    private_captured,
                    disguise_transitioning_to,
                    disguise_halfpoint_reached,
                    faerie_fire,
                    booby_trapped,
                    eject_invulnerable,
                    pilot_did_move_to_base,
                    parachuting,
                    parachute_open,
                    parachute_landing_override_set,
                    using_ability,
                    deployed,
                    under_construction,
                    sold,
                    reconstructing,
                    unselectable,
                    ignoring_stealth,
                    repulsor,
                    disabled_underpowered,
                    disabled_freefall,
                    is_carbomb,
                    hijacked,
                    force_attack,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        if let Some(v) = stealthed {
                            e.stealthed = v;
                        }
                        if let Some(v) = detected {
                            e.detected = v;
                        }
                        if let Some(v) = attacking {
                            e.attacking = v;
                        }
                        if let Some(v) = moving {
                            e.moving = v;
                        }
                        if let Some(v) = is_firing_weapon {
                            e.is_firing_weapon = v;
                        }
                        if let Some(v) = is_aiming_weapon {
                            e.is_aiming_weapon = v;
                        }
                        if let Some(v) = selected {
                            e.selected = v;
                        }
                        if let Some(v) = disabled_emp {
                            e.disabled_emp = v;
                        }
                        if let Some(v) = weapons_jammed {
                            e.weapons_jammed = v;
                        }
                        if let Some(v) = disabled_hacked {
                            e.disabled_hacked = v;
                        }
                        if let Some(v) = disabled_unmanned {
                            e.disabled_unmanned = v;
                        }
                        if let Some(v) = disabled_paralyzed {
                            e.disabled_paralyzed = v;
                        }
                        if let Some(v) = disabled_subdued {
                            e.disabled_subdued = v;
                        }
                        if let Some(v) = masked {
                            e.masked = v;
                        }
                        if let Some(v) = disguised {
                            e.disguised = v;
                        }
                        if let Some(v) = no_collisions {
                            e.no_collisions = v;
                        }
                        if let Some(v) = private_captured {
                            e.private_captured = v;
                        }
                        if let Some(v) = disguise_transitioning_to {
                            e.disguise_transitioning_to = v;
                        }
                        if let Some(v) = disguise_halfpoint_reached {
                            e.disguise_halfpoint_reached = v;
                        }
                        if let Some(v) = faerie_fire {
                            e.faerie_fire = v;
                        }
                        if let Some(v) = booby_trapped {
                            e.booby_trapped = v;
                        }
                        if let Some(v) = eject_invulnerable {
                            e.eject_invulnerable = v;
                        }
                        if let Some(v) = pilot_did_move_to_base {
                            e.pilot_did_move_to_base = v;
                        }
                        if let Some(v) = parachuting {
                            e.parachuting = v;
                        }
                        if let Some(v) = parachute_open {
                            e.parachute_open = v;
                        }
                        if let Some(v) = parachute_landing_override_set {
                            e.parachute_landing_override_set = v;
                        }
                        if let Some(v) = using_ability {
                            e.using_ability = v;
                        }
                        if let Some(v) = deployed {
                            e.deployed = v;
                        }
                        if let Some(v) = under_construction {
                            e.under_construction = v;
                        }
                        if let Some(v) = sold {
                            e.sold = v;
                        }
                        if let Some(v) = reconstructing {
                            e.reconstructing = v;
                        }
                        if let Some(v) = unselectable {
                            e.unselectable = v;
                        }
                        if let Some(v) = ignoring_stealth {
                            e.ignoring_stealth = v;
                        }
                        if let Some(v) = repulsor {
                            e.repulsor = v;
                        }
                        if let Some(v) = disabled_underpowered {
                            e.disabled_underpowered = v;
                        }
                        if let Some(v) = disabled_freefall {
                            e.disabled_freefall = v;
                        }
                        if let Some(v) = is_carbomb {
                            e.is_carbomb = v;
                        }
                        if let Some(v) = hijacked {
                            e.hijacked = v;
                        }
                        if let Some(v) = force_attack {
                            e.force_attack = v;
                        }
                        applied += 1;
                    }
                }
                WorldMutation::SetVeterancy { target, ordinal } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.veterancy_ordinal = ordinal.min(3);
                        applied += 1;
                    }
                }
                WorldMutation::SetExperience { target, points } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.experience_points = points.max(0.0);
                        applied += 1;
                    }
                }
                WorldMutation::SetWeaponBonus {
                    target,
                    enthusiastic,
                    subliminal,
                    horde,
                    nationalism,
                    frenzy,
                    frenzy_level,
                    battle_plan_bombardment,
                    battle_plan_hold_the_line,
                    battle_plan_search_and_destroy,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.weapon_bonus_enthusiastic = enthusiastic;
                        e.weapon_bonus_subliminal = subliminal;
                        e.weapon_bonus_horde = horde;
                        e.weapon_bonus_nationalism = nationalism;
                        e.weapon_bonus_frenzy = frenzy;
                        e.weapon_bonus_frenzy_level = frenzy_level;
                        e.weapon_bonus_battle_plan_bombardment = battle_plan_bombardment;
                        e.weapon_bonus_battle_plan_hold_the_line = battle_plan_hold_the_line;
                        e.weapon_bonus_battle_plan_search_and_destroy =
                            battle_plan_search_and_destroy;
                        applied += 1;
                    }
                }
                WorldMutation::SetActiveWeaponSlot { target, slot } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.active_weapon_slot = slot;
                        applied += 1;
                    }
                }
                WorldMutation::SetEntityPower {
                    target,
                    power_provided,
                    power_consumed,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.power_provided = power_provided;
                        e.power_consumed = power_consumed;
                        applied += 1;
                    }
                }
                WorldMutation::SetTurret {
                    target,
                    angle_deg,
                    pitch_deg,
                    holding,
                    idle_scanning,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.turret_angle_deg = angle_deg;
                        e.turret_pitch_deg = pitch_deg;
                        e.turret_holding = holding;
                        e.turret_idle_scanning = idle_scanning;
                        applied += 1;
                    }
                }
                WorldMutation::SetTargetLocation { unit, location } => {
                    if let Some(e) = self.inner.entity_mut(unit) {
                        e.target_location = location;
                        applied += 1;
                    }
                }
                WorldMutation::SetDetector {
                    target,
                    is_detector,
                    detection_range,
                    detection_rate_frames,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.is_detector = is_detector;
                        e.detection_range = detection_range.max(0.0);
                        e.detection_rate_frames = detection_rate_frames;
                        applied += 1;
                    }
                }
                WorldMutation::SetContinuousFire {
                    target,
                    level,
                    consecutive,
                    coast_until_frame,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.continuous_fire_level = level;
                        e.continuous_fire_consecutive = consecutive;

                        e.continuous_fire_coast_until_frame = coast_until_frame;
                        applied += 1;
                    }
                }
                WorldMutation::SetGuard {
                    unit,
                    position,
                    target_host,
                } => {
                    if let Some(e) = self.inner.entity_mut(unit) {
                        e.guard_position = position;
                        e.guard_target_host = target_host;
                        applied += 1;
                    }
                }
                WorldMutation::SetAiAttitude { target, attitude } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.ai_attitude = attitude.clamp(-2, 2);
                        applied += 1;
                    }
                }
                WorldMutation::SetWeaponSetFlags {
                    target,
                    player_upgrade,
                    armed_riders,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.weapon_set_player_upgrade = player_upgrade;
                        e.armed_riders_upgrade_weapon_set = armed_riders;
                        applied += 1;
                    }
                }
                WorldMutation::SetOvercharge { target, enabled } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.overcharge_enabled = enabled;
                        applied += 1;
                    }
                }
                WorldMutation::SetContainCapacity {
                    target,
                    max_transport,
                    max_garrison,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.max_transport = max_transport;
                        e.max_garrison = max_garrison;
                        applied += 1;
                    }
                }
                WorldMutation::SetHiveSlaves {
                    target,
                    slave_count,
                    slave_hp,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.hive_slave_count = slave_count;
                        e.hive_slave_hp = slave_hp.max(0.0);
                        applied += 1;
                    }
                }
                WorldMutation::SetStealthFlags {
                    target,
                    innate_stealth,
                    stealth_breaks_on_attack,
                    stealth_breaks_on_move,
                    is_tunnel_network,
                    passengers_allowed_to_fire,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.innate_stealth = innate_stealth;
                        e.stealth_breaks_on_attack = stealth_breaks_on_attack;
                        e.stealth_breaks_on_move = stealth_breaks_on_move;
                        e.is_tunnel_network = is_tunnel_network;
                        e.passengers_allowed_to_fire = passengers_allowed_to_fire;
                        applied += 1;
                    }
                }
                WorldMutation::SetOverlordAddon {
                    target,
                    has_gattling,
                    has_propaganda,
                    bunker_capacity,
                    is_helix_transport,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.has_overlord_gattling_addon = has_gattling;
                        e.has_overlord_propaganda_addon = has_propaganda;
                        e.overlord_bunker_capacity = bunker_capacity;
                        e.is_helix_transport = is_helix_transport;
                        applied += 1;
                    }
                }
                WorldMutation::SetCommandSet { target, command_set } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.command_set_override = command_set;
                        applied += 1;
                    }
                }
                WorldMutation::SetDisguise {
                    target,
                    template,
                    team_ordinal,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.disguise_as_template = template;
                        e.disguise_as_team_ordinal = team_ordinal;
                        applied += 1;
                    }
                }
                WorldMutation::SetVisionCamo {
                    target,
                    vision_spied_mask,
                    camo_friendly_opacity,
                    camo_stealth_look,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.vision_spied_mask = vision_spied_mask;
                        e.camo_friendly_opacity = camo_friendly_opacity;
                        e.camo_stealth_look = camo_stealth_look;
                        applied += 1;
                    }
                }
                WorldMutation::SetWeaponStats {
                    target,
                    has_weapon,
                    weapon_damage,
                    weapon_range,
                    weapon_min_range,
                    weapon_reload_time,
                    weapon_ammo,
                    weapon_can_target_air,
                    weapon_can_target_ground,
                    weapon_projectile_speed,
                    has_secondary_weapon,
                    secondary_weapon_damage,
                    secondary_weapon_range,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.has_weapon = has_weapon;
                        e.weapon_damage = weapon_damage;
                        e.weapon_range = weapon_range;
                        e.weapon_min_range = weapon_min_range;
                        e.weapon_reload_time = weapon_reload_time;
                        e.weapon_ammo = weapon_ammo;
                        e.weapon_can_target_air = weapon_can_target_air;
                        e.weapon_can_target_ground = weapon_can_target_ground;
                        e.weapon_projectile_speed = weapon_projectile_speed;
                        e.has_secondary_weapon = has_secondary_weapon;
                        e.secondary_weapon_damage = secondary_weapon_damage;
                        e.secondary_weapon_range = secondary_weapon_range;
                        applied += 1;
                    }
                }
                WorldMutation::SetMovement {
                    target,
                    velocity,
                    max_speed,
                    path_index,
                    path_len,
                    path_waypoints,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.velocity = velocity;
                        e.move_max_speed = max_speed;
                        e.path_index = path_index;
                        e.path_len = path_len;
                        e.path_waypoints = path_waypoints;
                        applied += 1;
                    }
                }
                WorldMutation::SetSelectionRadius {
                    target,
                    selection_radius,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.selection_radius = selection_radius;
                        applied += 1;
                    }
                }
                WorldMutation::SetModelCondition {
                    target,
                    model_condition_bits,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.model_condition_bits = model_condition_bits;
                        applied += 1;
                    }
                }
                WorldMutation::SetDemoMineCheer {
                    target,
                    demo_suicided_detonating,
                    has_mine_data,
                    cheer_timer,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.demo_suicided_detonating = demo_suicided_detonating;
                        e.has_mine_data = has_mine_data;
                        e.cheer_timer = cheer_timer;
                        applied += 1;
                    }
                }
                WorldMutation::SetCrushVision {
                    target,
                    crusher_level,
                    crushable_level,
                    vision_range,
                    shroud_clearing_range,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.crusher_level = crusher_level;
                        e.crushable_level = crushable_level;
                        e.vision_range = vision_range;
                        e.shroud_clearing_range = shroud_clearing_range;
                        applied += 1;
                    }
                }
                WorldMutation::SetBuildingType {
                    target,
                    is_building,
                    building_type_ordinal,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.is_building = is_building;
                        e.building_type_ordinal = building_type_ordinal;
                        applied += 1;
                    }
                }
                WorldMutation::SetIdentity {
                    target,
                    name,
                    team_color,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.display_name = name;
                        e.team_color = team_color;
                        applied += 1;
                    }
                }
                WorldMutation::SetGroundHeight {
                    target,
                    ground_height,
                    from_terrain,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.ground_height = ground_height;
                        e.ground_height_from_terrain = from_terrain;
                        applied += 1;
                    }
                }
                WorldMutation::SetProductionQueue { target, items } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.production_queue_items = items;
                        if let Some(head) = e.production_queue_items.first() {
                            e.production_template = head.template_name.clone();
                            e.production_progress = head.progress;
                        } else {
                            e.production_template.clear();
                            e.production_progress = 0.0;
                        }
                        applied += 1;
                    }
                }
                WorldMutation::SetConstruction {
                    target,
                    percent,
                    under_construction,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.construction_percent = percent.clamp(0.0, 1.0);
                        e.under_construction = under_construction;
                        applied += 1;
                    }
                }
                WorldMutation::SetSpecialPower {
                    target,
                    ready,
                    cooldown_remaining,
                    cooldown,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.special_power_ready = ready;
                        e.special_power_cooldown_remaining = cooldown_remaining.max(0.0);
                        e.special_power_cooldown = cooldown.max(0.0);
                        applied += 1;
                    }
                }
                WorldMutation::SetStoredSupplies { target, supplies } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.stored_supplies = supplies;
                        applied += 1;
                    }
                }
                WorldMutation::SetAiState { target, ordinal } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.ai_state_ordinal = ordinal;
                        applied += 1;
                    }
                }
                WorldMutation::SetContain {
                    target,
                    contained_by_host,
                    garrison_count,
                    garrisoned_host_ids,
                } => {
                    if let Some(e) = self.inner.entity_mut(target) {
                        e.contained_by_host = contained_by_host;
                        if let Some(c) = garrison_count {
                            e.garrison_count = c;
                        }
                        if let Some(ids) = garrisoned_host_ids {
                            e.garrisoned_host_ids = ids;
                        }
                        applied += 1;
                    }
                }
                WorldMutation::SetPlayerRadar {
                    player,
                    radar_count,
                    radar_disabled,
                } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.radar_count = radar_count;
                        p.radar_disabled = radar_disabled;
                        applied += 1;
                    }
                }
                WorldMutation::SetPlayerProgress {
                    player,
                    rank_level,
                    skill_points,
                    science_purchase_points,
                    cash_bounty_percent,
                } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.rank_level = rank_level;
                        p.skill_points = skill_points;
                        p.science_purchase_points = science_purchase_points;
                        p.cash_bounty_percent = cash_bounty_percent;
                        applied += 1;
                    }
                }
                WorldMutation::SetPlayerSciences {
                    player,
                    unlocked_sciences,
                } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.unlocked_sciences = unlocked_sciences;
                        applied += 1;
                    }
                }
                WorldMutation::SetPlayerAlive { player, is_alive } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.is_alive = is_alive;
                        applied += 1;
                    }
                }
                WorldMutation::SetPlayerCooldowns { player, cooldowns } => {
                    if let Some(p) = self.inner.player_mut(player) {
                        p.shared_special_power_cooldowns = cooldowns;
                        applied += 1;
                    }
                }
            }
        }
        applied
    }

    /// Produce an immutable presentation/world snapshot (no live borrows retained).
    pub fn snapshot(&self) -> WorldSnapshot {
        self.inner.snapshot()
    }

    pub fn spawn_entity(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> EntityId {
        self.inner.spawn_entity(template, owner, transform, health)
    }

    pub fn allocate_player_with_name(
        &mut self,
        name: Option<String>,
        team: Option<u8>,
        is_human: bool,
    ) -> Option<PlayerId> {
        self.inner.allocate_player_with_name(name, team, is_human)
    }

    /// Allocate a player and set economy fields in one call (shadow/host mirror).
    pub fn allocate_player_with_economy(
        &mut self,
        name: Option<String>,
        team: Option<u8>,
        is_human: bool,
        supplies: u32,
        power_available: i32,
    ) -> Option<PlayerId> {
        let id = self.inner.allocate_player_with_name(name, team, is_human)?;
        if let Some(p) = self.inner.player_mut(id) {
            p.supplies = supplies;
            p.power_available = power_available;
        }
        Some(id)
    }

    /// Set supplies for a player (borrow-first economy phase helper).
    pub fn set_player_supplies(&mut self, id: PlayerId, supplies: u32) -> bool {
        if let Some(p) = self.inner.player_mut(id) {
            p.supplies = supplies;
            true
        } else {
            false
        }
    }

    /// Clear all entities (incremental shadow rebuild helper).
    pub fn clear_entities(&mut self) {
        self.inner.clear_entities();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_players_until_capacity() {
        let mut world = World::new(2);
        let first = world.allocate_player().unwrap();
        assert_eq!(first, PlayerId::FIRST);
        let second = world.allocate_player().unwrap();
        assert_eq!(second.get(), 1);
        assert!(world.allocate_player().is_none());

        let removed = world.remove_player(first).expect("player removed");
        assert_eq!(removed.id, first);
        let recycled = world.allocate_player().unwrap();
        assert_eq!(recycled, first);
    }

    #[test]
    fn snapshot_reflects_active_players() {
        let mut world = World::new(3);
        world.allocate_player();
        world.advance(5);

        let snapshot = world.snapshot();
        assert_eq!(snapshot.frame, 5);
        assert_eq!(snapshot.players.len(), 1);
        assert!(snapshot.entities.is_empty());
    }

    #[test]
    fn resize_preserves_active_players() {
        let mut world = World::new(4);
        let p1 = world.allocate_player_with_name(Some("Alice".into()), Some(1), true);
        let p2 = world.allocate_player_with_name(Some("Bot".into()), Some(2), false);
        assert!(p1.is_some() && p2.is_some());

        world.resize(2);
        let snapshot = world.snapshot();
        assert_eq!(snapshot.players.len(), 2);
        assert!(snapshot.players.iter().any(|p| p.name == "Alice"));
        assert!(snapshot.players.iter().any(|p| p.name == "Bot"));
        assert!(snapshot.players.iter().all(|p| p.team.is_some()));
    }

    #[test]
    fn removing_unknown_player_is_noop() {
        let mut world = World::new(1);
        assert!(world.remove_player(PlayerId::FIRST).is_none());
    }

    #[test]
    fn entity_lifecycle_affects_snapshot() {
        let mut world = World::new(2);
        let entity_id = world.spawn_entity(
            TemplateRef::new("GLAInfantryRebel"),
            Some(PlayerId::FIRST),
            Transform::new([0.0, 0.0, 0.0], 0.0),
            100.0,
        );

        let snapshot = world.snapshot();
        assert_eq!(snapshot.entities.len(), 1);
        let entity = &snapshot.entities[0];
        assert_eq!(entity.id, entity_id);
        assert_eq!(entity.template, "GLAInfantryRebel");
        assert_eq!(entity.owner, Some(PlayerId::FIRST));

        assert!(world.remove_entity(entity_id));
        let snapshot = world.snapshot();
        assert!(snapshot.entities.is_empty());
    }
    #[test]
    fn game_world_deferred_mutations_apply_deterministically() {
        let mut gw = GameWorld::new(4);
        let owner = gw
            .allocate_player_with_name(Some("A".into()), Some(0), true)
            .expect("player");
        let id = gw.spawn_entity(
            TemplateRef::new("Unit"),
            Some(owner),
            Transform::new([0.0, 0.0, 0.0], 0.0),
            100.0,
        );
        gw.queue_mutation(WorldMutation::Damage {
            target: id,
            amount: 40.0,
        });
        // Not applied yet
        assert!((gw.entity(id).unwrap().health - 100.0).abs() < f32::EPSILON);
        assert_eq!(gw.apply_pending_mutations(), 1);
        assert!((gw.entity(id).unwrap().health - 60.0).abs() < f32::EPSILON);
        gw.queue_mutation(WorldMutation::Destroy(id));
        assert_eq!(gw.apply_pending_mutations(), 1);
        assert!(gw.entity(id).is_none());
        let snap = gw.snapshot();
        assert_eq!(snap.entities.len(), 0);
    }
}
