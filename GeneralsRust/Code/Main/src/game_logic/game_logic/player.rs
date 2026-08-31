//! Mechanical split from `game_logic/game_logic.rs`. No behavior change.
#![allow(non_snake_case, unused_imports, dead_code)]
use super::authority::*;
use super::construct::*;
use super::crate_tick::*;
use super::host::*;
use super::prelude::*;
use super::script_camera::*;
use super::*;

/// Map-authored SidesList leftovers applied onto a live host player.
/// C++ `Player::initFromDict` + `PlayerList` relationship pass.
#[derive(Debug, Clone)]
pub struct PlayerMapSideState {
    /// Dict `playerName` used to resolve playerAllies / playerEnemies tokens.
    pub map_player_name: String,
    /// Explicit overrides. Missing entries default Neutral (C++ PlayerRelationMap).
    pub relations: HashMap<u32, gamelogic::common::Relationship>,
    /// C++ `Handicap::m_handicaps[BUILDCOST][GENERIC]`.
    pub handicap_build_cost_generic: f32,
    /// C++ `Handicap::m_handicaps[BUILDCOST][BUILDINGS]`.
    pub handicap_build_cost_buildings: f32,
    /// C++ `Handicap::m_handicaps[BUILDTIME][GENERIC]`.
    pub handicap_build_time_generic: f32,
    /// C++ `Handicap::m_handicaps[BUILDTIME][BUILDINGS]`.
    pub handicap_build_time_buildings: f32,
    /// SidesList BuildListInfo rows (template, world xz, rebuilds, initiallyBuilt).
    pub build_list: Vec<HostAuthoredBuild>,
}

/// One SidesList build-list row consumed by live host AI.
#[derive(Debug, Clone)]
pub struct HostAuthoredBuild {
    pub template: String,
    pub position: (f32, f32, f32),
    pub num_rebuilds: u32,
    pub initially_built: bool,
}

impl Default for PlayerMapSideState {
    fn default() -> Self {
        Self {
            map_player_name: String::new(),
            relations: HashMap::new(),
            handicap_build_cost_generic: 1.0,
            handicap_build_cost_buildings: 1.0,
            handicap_build_time_generic: 1.0,
            handicap_build_time_buildings: 1.0,
            build_list: Vec::new(),
        }
    }
}

impl PlayerMapSideState {
    fn read_handicap_from_dict(&mut self, dict: &Dict) {
        const KEYS: [(&str, fn(&mut PlayerMapSideState, f32)); 4] = [
            ("HANDICAP_BUILDCOST_GENERIC", |s, v| {
                s.handicap_build_cost_generic = v;
            }),
            ("HANDICAP_BUILDCOST_BUILDINGS", |s, v| {
                s.handicap_build_cost_buildings = v;
            }),
            ("HANDICAP_BUILDTIME_GENERIC", |s, v| {
                s.handicap_build_time_generic = v;
            }),
            ("HANDICAP_BUILDTIME_BUILDINGS", |s, v| {
                s.handicap_build_time_buildings = v;
            }),
        ];
        for (name, apply) in KEYS {
            let key = NameKeyGenerator::name_to_key(name);
            if dict.get_type(key).is_some() {
                apply(self, dict.get_real(key));
            }
        }
    }
}

/// Player structure
#[derive(Debug, Clone)]
pub struct Player {
    pub id: u32,
    pub team: Team,
    pub name: String,
    pub resources: Resources,
    /// In-flight supply delta under GameWorld economy authority (cleared on writeback).
    pub pending_supply_delta: i64,
    pub power_available: i32,
    /// Total power produced by this player's power plants (for energy ratio).
    pub power_produced: i32,
    /// Total power consumed by this player's buildings (for energy ratio).
    pub power_consumed: i32,
    /// C++ `OverchargeBehavior::onCapture` is a fire-and-forget mutation of
    /// `Energy`, not an ownership-derived object field.  When an active
    /// Overcharge plant is captured while disabled, its module deliberately
    /// does not move the template `EnergyBonus` to the new controller.  The
    /// host normally recomputes power from current object ownership, so this
    /// signed correction preserves that one historical bonus location.
    ///
    /// This is intentionally not part of `PlayerSnapshot`: C++ `Energy::xfer`
    /// reconstructs production on load and `OverchargeBehavior::loadPostProcess`
    /// re-adds an active bonus for the object's current controller.
    pub captured_overcharge_power_delta: i32,
    pub income_accumulator: f32,
    pub selected_objects: Vec<ObjectId>,
    pub unlocked_sciences: HashSet<String>,
    pub queued_upgrades: HashSet<String>,
    pub is_local: bool,
    pub is_alive: bool,
    /// C++ `Player::m_observer` / `isPlayerObserver`.
    pub is_observer: bool,
    /// C++ Player::didPlayerPreorder residual (shell/skirmish preorder bonus).
    pub did_preorder: bool,
    pub statistics: PlayerStatistics,
    /// Frame at which power sabotage expires (0 = not sabotaged).
    /// Matches C++ Player::m_powerSabotagedUntilFrame.
    pub power_sabotaged_till_frame: u32,
    /// Skirmish UI color (RGB) applied from match config.
    pub color_rgb: (u8, u8, u8),
    /// C++ `Player::m_nightColor` / `getPlayerNightColor`.
    pub color_night_rgb: (u8, u8, u8),

    /// Skirmish start position index from match config.
    pub start_position: i32,
    /// Skirmish alliance team index from match config (not faction Team).
    pub alliance_team: i32,
    /// Cash bounty percent residual (GLA SCIENCE_CashBounty).
    /// C++ Player::m_cashBountyPercent — fraction of victim build cost awarded on kill.
    /// 0.0 = disabled; retail tiers 0.05 / 0.10 / 0.20.
    pub cash_bounty_percent: f32,
    /// C++ Player::m_kindOfPercentProductionChangeList residual (CostModifierUpgrade).
    pub kind_of_production_cost_changes:
        Vec<crate::game_logic::host_upgrade_module_residuals::KindOfProductionCostChange>,
    /// Radar residual count (C++ Player::m_radarCount).
    /// Providers: CommandCenter / RadarVan residual ownership path.
    pub radar_count: i32,
    /// True when radar is disabled by script/power residual (C++ m_radarDisabled).
    pub radar_disabled: bool,
    /// C++ Player::m_disableProofRadarCount — RadarVan RadarUpgrade DisableProof.
    pub disable_proof_radar_count: i32,
    /// C++ Player::m_logicalRetaliationModeEnabled residual (options Auto-Retaliate).
    pub logical_retaliation_mode_enabled: bool,
    /// C++ Player::m_rankLevel residual (1-based retail ranks).
    pub rank_level: u32,
    /// C++ Player::m_skillPoints residual.
    pub skill_points: i32,
    /// C++ Player::m_sciencePurchasePoints residual.
    pub science_purchase_points: i32,
    /// C++ Player::m_skillPointsModifier residual (default 1.0).
    pub skill_points_modifier: f32,
    /// C++ AcademyStats::m_specialPowersUsed — ACT_SUPERPOWER fires this match.
    pub special_powers_used: u32,

    /// C++ Player::m_canBuildUnits (Player.cpp:2301). Scripts flip this via
    /// PLAYER_DISABLE/ENABLE_UNIT_CONSTRUCTION.
    pub can_build_units: bool,
    /// C++ Player::m_canBuildBase (Player.cpp:2297).
    pub can_build_base: bool,
    /// C++ Player::m_unitsShouldHunt. PLAYER_HUNT keeps map-wide hunt after clear.
    pub units_should_hunt: bool,
    /// C++ Player::m_listInScoreScreen (`excludePlayerFromScoreScreen`).
    pub list_in_score_screen: bool,

    /// C++ Player::m_specialPowerReadyTimerList residual (seconds remaining).
    /// SharedSyncedTimer superweapons sync across a player's command centers.
    pub shared_special_power_cooldowns: HashMap<crate::command_system::SpecialPowerType, f32>,
    /// C++ GrantUpgradeCreate / Player::addUpgrade completed names.
    pub completed_upgrades: HashSet<String>,
    /// C++ ResourceGatheringManager supply-center object IDs.
    pub resource_supply_centers: Vec<ObjectId>,
    /// C++ ResourceGatheringManager supply-warehouse object IDs.
    pub resource_supply_warehouses: Vec<ObjectId>,
    /// C++ initFromDict / PlayerList relationship leftovers from the map SidesList.
    pub map_side: PlayerMapSideState,
    /// C++ `Player::m_teamRelations` — script team-id overrides keyed by
    /// team instance name (`PLAYER_SET_OVERRIDE_RELATION_TO_TEAM`).
    pub team_relations: HashMap<String, gamelogic::common::Relationship>,
    /// C++ `Team::m_teamRelations` drained from TEAM_SET_OVERRIDE_RELATION_TO_TEAM.
    /// Outer key: source team instance. Inner: target team instance.
    pub team_instance_team_relations:
        HashMap<String, HashMap<String, gamelogic::common::Relationship>>,
    /// C++ `Team::m_playerRelations` drained from TEAM_SET_OVERRIDE_RELATION_TO_PLAYER.
    /// Outer key: source team instance. Inner: target player id.
    pub team_instance_player_relations:
        HashMap<String, HashMap<u32, gamelogic::common::Relationship>>,
    /// C++ `Player::m_sciencesDisabled` (script `PLAYER_SCIENCE_AVAILABILITY`).
    pub sciences_disabled: HashSet<String>,
    /// C++ `Player::m_sciencesHidden`.
    pub sciences_hidden: HashSet<String>,
    /// C++ `Player::m_attackedBy[MAX_PLAYER_COUNT]` (Player.cpp:3864).
    pub attacked_by: [bool; Self::MAX_ATTACKED_BY_PLAYERS],
    /// C++ `Player::m_attackedFrame` — last frame `setAttackedBy` fired.
    pub attacked_frame: u32,
}

/// Main-owned identity of the C++ `PlayerTemplate` that constructed a host
/// player.  A base [`Team`] remains a compatibility/faction-routing value; it
/// is not sufficient to represent a Zero Hour General.
///
/// Challenge selection is position-sensitive in C++ (`GameSlot` stores the
/// selected PlayerTemplate index), so an indexed identity validates both the
/// retained index and canonical template name on every resolution.  Ordinary
/// campaign entries carry the exact name only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTemplateIdentity {
    pub template_name: String,
    pub template_index: Option<i32>,
}

impl PlayerTemplateIdentity {
    /// Resolve an ordinary Campaign `PlayerFaction` only when it names an
    /// exact PlayerTemplate.  A bare base-side label that is not a template is
    /// deliberately not guessed into a selected General.
    pub fn from_exact_name(template_name: &str) -> Option<Self> {
        let template_name = template_name.trim();
        if template_name.is_empty() {
            return None;
        }

        game_engine::common::ini::ensure_player_templates_loaded();
        let store = game_engine::common::rts::player_template::get_player_template_store();
        let template = store.find_template(template_name)?;
        Some(Self {
            template_name: template.get_name().to_string(),
            template_index: None,
        })
    }

    /// Resolve a Challenge selection only when its retained slot still names
    /// the exact template selected by the shell.  Name-only lookup would let a
    /// reordered PlayerTemplate store launch a plausible but wrong General.
    pub fn from_exact_indexed_name(template_name: &str, template_index: i32) -> Option<Self> {
        let template_name = template_name.trim();
        if template_name.is_empty() {
            return None;
        }

        game_engine::common::ini::ensure_player_templates_loaded();
        let store = game_engine::common::rts::player_template::get_player_template_store();
        let template = store.get_nth_player_template_signed(template_index)?;
        (template.get_name() == template_name).then(|| Self {
            template_name: template.get_name().to_string(),
            template_index: Some(template_index),
        })
    }

    /// Re-resolve the immutable Common store record at the GameLogic authority
    /// boundary.  Returning a clone keeps the store lock short and gives the
    /// host only source-authored PlayerTemplate fields.
    pub fn resolve(&self) -> Option<game_engine::common::rts::player_template::PlayerTemplate> {
        game_engine::common::ini::ensure_player_templates_loaded();
        let store = game_engine::common::rts::player_template::get_player_template_store();
        match self.template_index {
            Some(index) => store
                .get_nth_player_template_signed(index)
                .filter(|template| template.get_name() == self.template_name)
                .cloned(),
            None => store.find_template(&self.template_name).cloned(),
        }
    }

    /// Main's current Team enum represents only the three C++ base sides.
    /// Keep that conversion exact and reject observer/civilian/Boss identities
    /// rather than silently choosing a different General.
    pub fn base_team(&self) -> Option<Team> {
        let template = self.resolve()?;
        Self::team_for_template(&template)
    }

    pub(crate) fn team_for_template(
        template: &game_engine::common::rts::player_template::PlayerTemplate,
    ) -> Option<Team> {
        Self::team_from_side(template.get_base_side())
            .or_else(|| Self::team_from_side(template.get_side()))
    }

    /// C++ `Player::getProductionCostChangePercent`: a PlayerTemplate
    /// modifier is keyed by the *exact* ThingTemplate name, not by its base
    /// faction or KindOf.  Keep this computation beside the identity so every
    /// Main consumer uses the same NameKey lookup as `Player.cpp`.
    pub(crate) fn production_cost_factor_for_template(
        template: &game_engine::common::rts::player_template::PlayerTemplate,
        build_template_name: &str,
    ) -> f32 {
        let key = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            build_template_name,
        );
        1.0 + template
            .get_production_cost_changes()
            .get(&key)
            .copied()
            .unwrap_or(0.0)
    }

    /// C++ `Player::getProductionTimeChangePercent`.  This deliberately does
    /// not fold in low-power timing: `ThingTemplate::calcTimeToBuild` applies
    /// the authored General factor first and the energy penalty afterwards.
    pub(crate) fn production_time_factor_for_template(
        template: &game_engine::common::rts::player_template::PlayerTemplate,
        build_template_name: &str,
    ) -> f32 {
        let key = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            build_template_name,
        );
        1.0 + template
            .get_production_time_changes()
            .get(&key)
            .copied()
            .unwrap_or(0.0)
    }

    /// C++ `Player::getProductionVeterancyLevel`, translated from Common's
    /// `Regular` spelling to Main's long-lived `Rookie` spelling.  The C++
    /// default is LEVEL_FIRST/Regular, so callers never invent a veterancy
    /// level when an exact template has no entry for this object.
    pub(crate) fn production_veterancy_for_template(
        template: &game_engine::common::rts::player_template::PlayerTemplate,
        build_template_name: &str,
    ) -> VeterancyLevel {
        use game_engine::common::game_common::VeterancyLevel as CommonVeterancyLevel;

        let key = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
            build_template_name,
        );
        match template
            .get_production_veterancy_levels()
            .get(&key)
            .copied()
            .unwrap_or(CommonVeterancyLevel::Regular)
        {
            CommonVeterancyLevel::Regular => VeterancyLevel::Rookie,
            CommonVeterancyLevel::Veteran => VeterancyLevel::Veteran,
            CommonVeterancyLevel::Elite => VeterancyLevel::Elite,
            CommonVeterancyLevel::Heroic => VeterancyLevel::Heroic,
        }
    }

    pub(super) fn team_from_side(value: &str) -> Option<Team> {
        match value.trim().to_ascii_lowercase().as_str() {
            "usa" | "us" | "america" | "factionamerica" => Some(Team::USA),
            "china" | "factionchina" => Some(Team::China),
            "gla" | "factiongla" => Some(Team::GLA),
            "observer" | "factionobserver" => Some(Team::Neutral),
            _ => None,
        }
    }
}

impl Player {
    /// C&C Generals default starting money is $10,000 (Normal difficulty).
    /// Matches the `StartingMoney::Normal` variant from the LAN API game-info crate.
    pub const DEFAULT_STARTING_MONEY: u32 = 10_000;
    /// C++ `GameCommon.h MAX_PLAYER_COUNT`.
    pub const MAX_ATTACKED_BY_PLAYERS: usize = 16;

    pub fn new(id: u32, team: Team, name: &str, is_local: bool) -> Self {
        Self {
            id,
            team,
            name: name.to_string(),
            resources: Resources {
                supplies: Self::DEFAULT_STARTING_MONEY,
                power: 0,
            },
            pending_supply_delta: 0,
            power_available: 0,
            power_produced: 0,
            power_consumed: 0,
            captured_overcharge_power_delta: 0,
            income_accumulator: 0.0,
            selected_objects: Vec::new(),
            unlocked_sciences: HashSet::new(),
            queued_upgrades: HashSet::new(),
            is_local,
            did_preorder: false,
            is_alive: true,
            is_observer: false,
            statistics: PlayerStatistics::default(),
            power_sabotaged_till_frame: 0,
            color_rgb: (200, 200, 200),
            color_night_rgb: (200, 200, 200),

            start_position: -1,
            alliance_team: -1,
            cash_bounty_percent: 0.0,
            kind_of_production_cost_changes: Vec::new(),
            radar_count: 0,
            radar_disabled: false,
            disable_proof_radar_count: 0,
            logical_retaliation_mode_enabled: false,
            rank_level: 1,
            skill_points: 0,
            science_purchase_points: 0,
            skill_points_modifier: 1.0,
            special_powers_used: 0,

            can_build_units: true,
            can_build_base: true,
            units_should_hunt: false,
            list_in_score_screen: true,

            shared_special_power_cooldowns: HashMap::new(),
            team_relations: HashMap::new(),
            team_instance_team_relations: HashMap::new(),
            team_instance_player_relations: HashMap::new(),
            sciences_disabled: HashSet::new(),
            sciences_hidden: HashSet::new(),

            map_side: PlayerMapSideState::default(),
            completed_upgrades: HashSet::new(),
            resource_supply_centers: Vec::new(),
            resource_supply_warehouses: Vec::new(),
            attacked_by: [false; Self::MAX_ATTACKED_BY_PLAYERS],
            attacked_frame: 0,
        }
    }

    /// C++ `Object::getNightIndicatorColor` / `getIndicatorColor` by TOD.
    pub fn house_color_rgb(&self) -> (u8, u8, u8) {
        if crate::game_logic::host_radar::host_time_of_day_is_night() {
            self.color_night_rgb
        } else {
            self.color_rgb
        }
    }

    /// C++ `Player::addUpgrade(..., UPGRADE_STATUS_COMPLETE)`.
    pub fn add_completed_upgrade(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        self.completed_upgrades.insert(name.to_string());
        self.unlocked_sciences.insert(name.to_string());
        self.sync_leftover_player_upgrade_from_host(name);
    }

    /// C++ SpecialPowerModule::initiateIntentToDoSpecialPower →
    /// AcademyStats::recordSpecialPowerUsed (ACT_SUPERPOWER only).
    pub fn record_special_power_used(&mut self) {
        self.special_powers_used = self.special_powers_used.saturating_add(1);
        self.sync_leftover_academy_special_power_used();
    }

    fn sync_leftover_academy_special_power_used(&self) {
        let named = format!("player{}", self.id);
        let leftover = {
            let Ok(list) = gamelogic::player::player_list().read() else {
                return;
            };
            let by_name = [
                self.name.as_str(),
                self.map_side.map_player_name.as_str(),
                named.as_str(),
            ]
            .into_iter()
            .find_map(|n| {
                if n.is_empty() {
                    None
                } else {
                    list.find_player_by_name(n)
                }
            });
            by_name
                .or_else(|| list.get_player(self.id as i32).cloned())
                .or_else(|| {
                    list.iter().find_map(|arc| {
                        arc.read()
                            .ok()
                            .is_some_and(|guard| guard.get_player_index() as u32 == self.id)
                            .then(|| std::sync::Arc::clone(arc))
                    })
                })
        };
        let Some(arc) = leftover else {
            return;
        };
        let Ok(mut guard) = arc.write() else {
            return;
        };
        guard.get_academy_stats_mut().record_special_power_used(
            game_engine::common::rts::academy_stats::AcademyClassificationType::Superpower,
        );
    }

    /// C++ ProductionUpdate.cpp:874-879 / 931 — purchased research complete.
    /// AcademyStats::recordUpgrade(upgrade, FALSE) + ScoreKeeper::addMoneySpent.
    pub fn record_upgrade_production_complete(&self, upgrade_name: &str) {
        self.sync_leftover_upgrade_production_complete(upgrade_name);
    }

    fn leftover_player_arc_for_sync(
        &self,
    ) -> Option<std::sync::Arc<std::sync::RwLock<gamelogic::player::Player>>> {
        let named = format!("player{}", self.id);
        let Ok(list) = gamelogic::player::player_list().read() else {
            return None;
        };
        let by_name = [
            self.name.as_str(),
            self.map_side.map_player_name.as_str(),
            named.as_str(),
        ]
        .into_iter()
        .find_map(|n| {
            if n.is_empty() {
                None
            } else {
                list.find_player_by_name(n)
            }
        });
        by_name
            .or_else(|| list.get_player(self.id as i32).cloned())
            .or_else(|| {
                list.iter().find_map(|arc| {
                    arc.read()
                        .ok()
                        .is_some_and(|guard| guard.get_player_index() as u32 == self.id)
                        .then(|| std::sync::Arc::clone(arc))
                })
            })
    }

    fn sync_leftover_upgrade_production_complete(&self, upgrade_name: &str) {
        let Some(template) =
            gamelogic::upgrade::center::with_upgrade_center(|c| c.find_upgrade(upgrade_name))
        else {
            return;
        };
        let Some(arc) = self.leftover_player_arc_for_sync() else {
            return;
        };
        let Ok(mut guard) = arc.write() else {
            return;
        };
        let cost = template.calc_cost_to_build(&guard).max(0) as u32;
        guard
            .get_academy_stats_mut()
            .record_upgrade(template.as_ref(), false);
        guard.get_score_keeper_mut().add_money_spent(cost);
    }

    /// Leftover `Player::addUpgrade` already xfers `m_upgradesCompleted`.
    fn sync_leftover_player_upgrade_from_host(&self, upgrade_name: &str) {
        use gamelogic::player::PlayerArcExt;

        let names = [self.name.as_str(), self.map_side.map_player_name.as_str()];
        let leftover = {
            let Ok(list) = gamelogic::player::player_list().read() else {
                return;
            };
            names.iter().find_map(|n| {
                if n.is_empty() {
                    None
                } else {
                    list.find_player_by_name(n)
                }
            })
        };
        let Some(arc) = leftover else {
            return;
        };
        let Some(template) =
            gamelogic::upgrade::center::with_upgrade_center(|c| c.find_upgrade(upgrade_name))
        else {
            return;
        };
        arc.add_upgrade(
            template.as_ref(),
            gamelogic::upgrade::UpgradeStatus::Complete,
        );
    }

    /// C++ `ResourceGatheringManager::addSupplyCenter`.
    pub fn add_supply_center(&mut self, center_id: ObjectId) {
        if !self.resource_supply_centers.contains(&center_id) {
            self.resource_supply_centers.push(center_id);
        }
    }

    /// C++ `ResourceGatheringManager::addSupplyWarehouse`.
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectId) {
        if !self.resource_supply_warehouses.contains(&warehouse_id) {
            self.resource_supply_warehouses.push(warehouse_id);
        }
    }

    /// C++ `Player::setAttackedBy` (Player.cpp:3864-3868).
    pub fn set_attacked_by(&mut self, player_index: i32) {
        if player_index >= 0 && (player_index as usize) < Self::MAX_ATTACKED_BY_PLAYERS {
            self.attacked_by[player_index as usize] = true;
            self.attacked_frame = crate::game_logic::host_historic_bonus::logic_frame();
        }
    }

    /// C++ `Player::getAttackedBy` (Player.cpp:3875-3878).
    pub fn get_attacked_by(&self, player_index: i32) -> bool {
        if player_index >= 0 && (player_index as usize) < Self::MAX_ATTACKED_BY_PLAYERS {
            self.attacked_by[player_index as usize]
        } else {
            false
        }
    }

    /// C++ `Player::getAttackedFrame`.
    pub fn get_attacked_frame(&self) -> u32 {
        self.attacked_frame
    }

    /// C++ Player::getOrStartSpecialPowerReadyFrame residual (seconds remaining).
    /// Missing entry means ready (C++ starts timer at "now" on first query).
    pub fn shared_special_power_remaining(
        &self,
        power: &crate::command_system::SpecialPowerType,
    ) -> f32 {
        self.shared_special_power_cooldowns
            .get(power)
            .copied()
            .unwrap_or(0.0)
            .max(0.0)
    }

    pub fn is_shared_special_power_ready(
        &self,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        self.shared_special_power_remaining(power) <= 0.0
    }

    /// C++ Player::resetOrStartSpecialPowerReadyFrame residual.
    pub fn reset_shared_special_power_timer(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        reload_seconds: f32,
    ) {
        let cd = reload_seconds.max(0.0);
        if cd <= 0.0 {
            self.shared_special_power_cooldowns.remove(power);
        } else {
            self.shared_special_power_cooldowns
                .insert(power.clone(), cd);
        }
        self.record_host_cooldowns();
    }

    /// C++ Player::expressSpecialPowerReadyFrame(now) residual — ready immediately.
    pub fn express_shared_special_power_ready_now(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
    ) {
        self.shared_special_power_cooldowns.remove(power);
    }

    /// Tick SharedSyncedTimer residual cooldowns.
    ///
    /// Returns powers that just became ready this tick (C++ PublicTimer ready edge).
    pub fn tick_shared_special_power_timers(
        &mut self,
        dt: f32,
    ) -> Vec<crate::command_system::SpecialPowerType> {
        let mut became_ready = Vec::new();
        if dt <= 0.0 || self.shared_special_power_cooldowns.is_empty() {
            return became_ready;
        }
        let keys: Vec<_> = self
            .shared_special_power_cooldowns
            .keys()
            .cloned()
            .collect();
        for power in keys {
            let Some(rem) = self.shared_special_power_cooldowns.get_mut(&power) else {
                continue;
            };
            let was = *rem;
            *rem = (*rem - dt).max(0.0);
            if was > 0.0 && *rem <= 0.0 {
                became_ready.push(power.clone());
            }
        }
        self.shared_special_power_cooldowns
            .retain(|_, rem| *rem > 0.0);
        self.record_host_cooldowns();
        became_ready
    }

    /// C++ Player::hasRadar (Player.cpp:3207-3213).
    /// Brownout sets `radar_disabled`, but a disable-proof provider (Radar Van)
    /// keeps radar online while `disable_proof_radar_count > 0`.
    pub fn has_radar(&self) -> bool {
        if self.radar_disabled && self.disable_proof_radar_count == 0 {
            return false;
        }
        self.radar_count > 0
    }

    /// C++ Player::addRadar (Player.cpp:3132-3140).
    pub fn add_radar(&mut self, disable_proof: bool) {
        self.radar_count = self.radar_count.saturating_add(1);
        if disable_proof {
            self.disable_proof_radar_count = self.disable_proof_radar_count.saturating_add(1);
        }
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    /// C++ Player::removeRadar (Player.cpp:3154-3172).
    pub fn remove_radar(&mut self, disable_proof: bool) {
        self.radar_count = (self.radar_count - 1).max(0);
        if disable_proof {
            self.disable_proof_radar_count = (self.disable_proof_radar_count - 1).max(0);
        }
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    /// C++ Player::disableRadar (Player.cpp:3175-3188).
    pub fn disable_radar(&mut self) {
        self.radar_disabled = true;
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    /// C++ Player::enableRadar (Player.cpp:3191-3203).
    pub fn enable_radar(&mut self) {
        self.radar_disabled = false;
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    pub fn set_radar_state(&mut self, radar_count: i32, radar_disabled: bool) {
        self.radar_count = radar_count;
        self.radar_disabled = radar_disabled;
        crate::game_logic::host_radar_log::record(self.id, self.radar_count, self.radar_disabled);
    }

    /// C++ Player::getCashBounty().
    pub fn get_cash_bounty(&self) -> f32 {
        self.cash_bounty_percent
    }

    /// C++ Player::setCashBounty — only raises if new percent is higher (CashBountyPower).
    pub fn set_cash_bounty(&mut self, percentage: f32) {
        if percentage > self.cash_bounty_percent {
            self.cash_bounty_percent = percentage;
            self.record_host_progress();
        }
    }

    /// Force-set cash bounty percent (tests / load restore).
    pub fn force_set_cash_bounty(&mut self, percentage: f32) {
        self.cash_bounty_percent = percentage.max(0.0);
        self.record_host_progress();
    }

    /// C++ Player::getProductionCostChangeBasedOnKindOf residual.
    pub fn production_cost_factor(&self, kind_tokens: &[&str]) -> f32 {
        crate::game_logic::host_upgrade_module_residuals::production_cost_factor_for_kindof(
            &self.kind_of_production_cost_changes,
            kind_tokens,
        )
    }

    /// C++ Player::addKindOfProductionCostChange residual.
    pub fn add_kind_of_production_cost_change(&mut self, kind_of: &str, percent: f32) {
        crate::game_logic::host_upgrade_module_residuals::add_kind_of_production_cost_change(
            &mut self.kind_of_production_cost_changes,
            kind_of,
            percent,
        );
    }

    pub fn record_host_progress(&self) {
        crate::game_logic::host_player_progress_log::record(
            self.id,
            self.rank_level,
            self.skill_points,
            self.science_purchase_points,
            self.cash_bounty_percent,
        );
    }

    pub fn record_host_sciences(&self) {
        crate::game_logic::host_player_meta_log::record_sciences(
            self.id,
            self.unlocked_sciences.iter().cloned(),
        );
    }

    pub fn record_host_alive(&self) {
        crate::game_logic::host_player_meta_log::record_alive(self.id, self.is_alive);
    }

    pub fn record_host_cooldowns(&self) {
        let mut cds: Vec<(String, f32)> = self
            .shared_special_power_cooldowns
            .iter()
            .map(|(k, v)| (format!("{k:?}"), *v))
            .collect();
        cds.sort_by(|a, b| a.0.cmp(&b.0));
        crate::game_logic::host_player_cooldown_log::record(self.id, cds);
    }

    /// Award cash for a kill: `ceil(victim_calcCostToBuild * cash_bounty_percent)`.
    /// C++ Player::doBountyForKill: deposit then ScoreKeeper::addMoneyEarned.
    /// Returns cash awarded (0 when disabled or zero cost).
    pub fn do_bounty_for_kill(&mut self, victim_build_cost: u32) -> u32 {
        let bounty = crate::game_logic::host_cash_bounty::compute_bounty_award(
            victim_build_cost,
            self.cash_bounty_percent,
        );
        if bounty > 0 {
            self.statistics.resources_collected =
                self.statistics.resources_collected.saturating_add(bounty);
            if crate::gameworld_shadow::gameworld_economy_authority_live() {
                self.pending_supply_delta += bounty as i64;
                crate::game_logic::host_economy_log::record(
                    self.id,
                    self.effective_supplies(),
                    self.power_available,
                );
            } else {
                self.resources.supplies = self.resources.supplies.saturating_add(bounty);
                crate::game_logic::host_economy_log::record(
                    self.id,
                    self.resources.supplies,
                    self.power_available,
                );
            }
            // C++ Player.cpp:2416 m_scoreKeeper.addMoneyEarned(bounty).
            self.add_money_earned(bounty);
        }
        bounty
    }

    /// C++ Player::addSkillPoints — modifier ceil, point cap, rank-up loop.
    /// Negative deltas lower skill points (C++ `min(pointCap, skill+delta)`).
    /// Rank-down is `set_rank_level` / `reset_rank`, not this loop.
    pub fn add_skill_points(&mut self, points: i32) -> bool {
        self.add_skill_points_limited(
            points,
            gamelogic::helpers::TheGameLogic::get_rank_level_limit(),
        )
    }

    /// C++ Player::addSkillPoints with GameLogic rank-level limit.
    pub fn add_skill_points_limited(&mut self, points: i32, rank_level_limit: i32) -> bool {
        use crate::game_logic::host_rank_ui_residual::{
            RankSkillStateResidual, add_skill_points_residual, rank_level_down_threshold_residual,
            rank_level_up_threshold_residual,
        };
        use crate::game_logic::host_science_rank::retail_rank_for_level;

        let old_level = self.rank_level.max(1);
        let old_skill = self.skill_points;
        let state = RankSkillStateResidual {
            rank_level: old_level,
            skill_points: self.skill_points,
            science_purchase_points: self.science_purchase_points,
            level_up: rank_level_up_threshold_residual(old_level),
            level_down: rank_level_down_threshold_residual(old_level),
        };
        let (new_state, level_gained) =
            add_skill_points_residual(state, points, self.skill_points_modifier, rank_level_limit);
        self.rank_level = new_state.rank_level;
        self.skill_points = new_state.skill_points;
        self.science_purchase_points = new_state.science_purchase_points;
        if new_state.rank_level > old_level {
            for lvl in (old_level + 1)..=new_state.rank_level {
                if let Some(row) = retail_rank_for_level(lvl) {
                    self.unlocked_sciences
                        .insert(row.science_granted.to_string());
                }
            }
        }
        if self.skill_points != old_skill || self.rank_level != old_level {
            self.record_host_progress();
            self.record_host_sciences();
            self.sync_leftover_player_sciences_from_host();
        }
        level_gained
    }

    /// C++ Player::addSkillPointsForKill — victim template SkillPointValue.
    pub fn add_skill_points_for_kill(&mut self, victim_skill_value: i32) -> bool {
        self.add_skill_points(victim_skill_value)
    }

    /// C++ Player::resetRank — rank 1, skill 0, intrinsic+Rank1 SPP, sciences reset.
    pub fn reset_rank(&mut self) {
        self.reset_rank_from_template(None);
    }

    /// C++ `Player::resetRank` + `resetSciences` with a resolved PlayerTemplate.
    /// Intrinsic sciences / IntrinsicSciencePurchasePoints come from the
    /// template when present; otherwise the base-faction residual is used.
    pub(crate) fn reset_rank_from_template(
        &mut self,
        template: Option<&game_engine::common::rts::player_template::PlayerTemplate>,
    ) {
        use crate::game_logic::host_rank_ui_residual::reset_rank_residual;
        use crate::game_logic::host_science_rank::SCIENCE_RANK1;

        let intrinsic_spp = template
            .map(|t| t.get_intrinsic_science_purchase_points())
            .unwrap_or(0);
        let reset = reset_rank_residual(intrinsic_spp);
        self.rank_level = reset.rank_level;
        self.skill_points = reset.skill_points;
        self.unlocked_sciences.clear();
        if let Some(template) = template {
            self.unlocked_sciences
                .extend(template.get_intrinsic_sciences().iter().cloned());
            self.unlocked_sciences.insert(SCIENCE_RANK1.to_string());
        } else {
            self.apply_faction_intrinsic_sciences();
        }
        self.science_purchase_points = reset.science_purchase_points;
        self.record_host_progress();
        self.record_host_sciences();
        self.sync_leftover_player_sciences_from_host();
    }

    /// C++ Player::setRankLevel — downgrade calls resetRank then climbs.
    pub fn set_rank_level(&mut self, new_level: u32) -> bool {
        self.set_rank_level_from_template(new_level, None)
    }

    pub(crate) fn set_rank_level_from_template(
        &mut self,
        new_level: u32,
        template: Option<&game_engine::common::rts::player_template::PlayerTemplate>,
    ) -> bool {
        use crate::game_logic::host_rank_ui_residual::{
            RankSkillStateResidual, rank_level_down_threshold_residual,
            rank_level_up_threshold_residual, set_rank_level_residual,
        };
        use crate::game_logic::host_science_rank::{RETAIL_RANK_COUNT, retail_rank_for_level};

        let limit = gamelogic::helpers::TheGameLogic::get_rank_level_limit().max(1) as u32;
        let old = self.rank_level.max(1);
        let target = new_level.max(1).min(RETAIL_RANK_COUNT).min(limit);
        if target == old {
            return false;
        }
        if target < old {
            self.reset_rank_from_template(template);
            if target == 1 {
                return true;
            }
        }
        let climb_from = self.rank_level.max(1);
        let state = RankSkillStateResidual {
            rank_level: climb_from,
            skill_points: self.skill_points,
            science_purchase_points: self.science_purchase_points,
            level_up: rank_level_up_threshold_residual(climb_from),
            level_down: rank_level_down_threshold_residual(climb_from),
        };
        let new_state = set_rank_level_residual(state, target, limit as i32);
        self.rank_level = new_state.rank_level;
        self.skill_points = new_state.skill_points;
        self.science_purchase_points = new_state.science_purchase_points;
        if new_state.rank_level > climb_from {
            for lvl in (climb_from + 1)..=new_state.rank_level {
                if let Some(row) = retail_rank_for_level(lvl) {
                    self.unlocked_sciences
                        .insert(row.science_granted.to_string());
                }
            }
        }
        self.record_host_progress();
        self.record_host_sciences();
        self.sync_leftover_player_sciences_from_host();
        true
    }

    /// C++ Player::addScience / addSciencePurchasePoints after rank-up.
    /// Live host leftover PlayerList stays stale unless we write it here.
    fn sync_leftover_player_sciences_from_host(&self) {
        use game_engine::common::rts::science::{SCIENCE_INVALID, get_science_store};

        let names = [self.name.as_str(), self.map_side.map_player_name.as_str()];
        let leftover = {
            let Ok(list) = gamelogic::player::player_list().read() else {
                return;
            };
            names.iter().find_map(|n| {
                if n.is_empty() {
                    None
                } else {
                    list.find_player_by_name(n)
                }
            })
        };
        let Some(arc) = leftover else {
            return;
        };
        let Ok(mut leftover) = arc.write() else {
            return;
        };
        let current = leftover.get_science_purchase_points();
        let delta = self.science_purchase_points - current;
        if delta != 0 {
            leftover.add_science_purchase_points(delta);
        }
        if let Some(store) = get_science_store() {
            for name in &self.unlocked_sciences {
                let science = store.get_science_from_internal_name(name);
                if science != SCIENCE_INVALID {
                    leftover.add_science(science);
                }
            }
        }
    }

    /// Supplies visible to purchase gates (includes in-flight economy-authority delta).
    pub fn effective_supplies(&self) -> u32 {
        let v = self.resources.supplies as i64 + self.pending_supply_delta;
        if v <= 0 {
            0
        } else if v >= u32::MAX as i64 {
            u32::MAX
        } else {
            v as u32
        }
    }

    /// Clear in-flight economy delta after GameWorld writeback.
    pub fn clear_pending_supply_delta(&mut self) {
        self.pending_supply_delta = 0;
    }

    pub fn can_afford(&self, cost: &Resources) -> bool {
        // Money is the hard construction / purchase gate. Power is separate (slows
        // production / disables powered buildings). Do not block structure starts when
        // the grid is already negative — GLA has no power plants, and USA/China must
        // still place a PowerPlant after the first Command Center finishes.
        //
        // Template `build_cost.power` is the post-build power draw residual (often
        // negative). It is applied in spend_resources, not as an affordability gate.
        self.effective_supplies() >= cost.supplies
    }

    pub fn spend_resources(&mut self, cost: &Resources) -> bool {
        if !self.can_afford(cost) {
            return false;
        }
        let power_after = self.power_available + cost.power; // Negative for consumption
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta -= cost.supplies as i64;
            self.power_available = power_after;
            if cost.supplies > 0 {
                self.record_resources_spent(cost.supplies);
                crate::game_logic::host_economy_log::record_money_audio(
                    self.id,
                    crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
                );
            }
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies -= cost.supplies;
            self.power_available = power_after;
            if cost.supplies > 0 {
                crate::game_logic::host_economy_log::record_money_audio(
                    self.id,
                    crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
                );

                self.record_resources_spent(cost.supplies);
            }
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
        true
    }

    pub fn add_resources(&mut self, amount: &Resources) {
        if amount.supplies == 0 {
            return;
        }
        if amount.supplies > 0 {
            self.statistics.resources_collected = self
                .statistics
                .resources_collected
                .saturating_add(amount.supplies);
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta += amount.supplies as i64;
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies = self.resources.supplies.saturating_add(amount.supplies);
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
    }

    /// Queue an upgrade for this player when not already queued/completed and affordable.
    /// Credit absolute supplies (income residual) and log economy channel.

    /// C++ ScoreKeeper::addMoneyEarned residual.

    /// C++ AcademyStats::recordBuildingCapture residual.
    pub fn record_building_capture(&mut self) {
        self.statistics.structures_captured = self.statistics.structures_captured.saturating_add(1);
        self.statistics.academy_building_captures =
            self.statistics.academy_building_captures.saturating_add(1);
    }
    /// C++ ScoreKeeper::addObjectCaptured residual.
    pub fn record_object_captured(&mut self) {
        self.statistics.objects_captured = self.statistics.objects_captured.saturating_add(1);
    }
    /// C++ AcademyStats::recordBuildingGarrisoned residual.
    pub fn record_building_garrisoned(&mut self) {
        self.statistics.structures_garrisoned =
            self.statistics.structures_garrisoned.saturating_add(1);
        self.statistics.academy_buildings_garrisoned = self
            .statistics
            .academy_buildings_garrisoned
            .saturating_add(1);
    }

    pub fn add_money_earned(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.statistics.money_earned = self.statistics.money_earned.saturating_add(amount);
    }

    /// Gain supplies under economy authority (pending delta) or direct mutate.
    pub fn apply_supply_gain(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta += amount as i64;
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies = self.resources.supplies.saturating_add(amount);
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
    }

    /// Spend supplies already validated via can_afford / effective_supplies.
    pub fn apply_supply_spend_unchecked(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        if crate::gameworld_shadow::gameworld_economy_authority_live() {
            self.pending_supply_delta -= amount as i64;
            crate::game_logic::host_economy_log::record(
                self.id,
                self.effective_supplies(),
                self.power_available,
            );
        } else {
            self.resources.supplies = self.resources.supplies.saturating_sub(amount);
            crate::game_logic::host_economy_log::record(
                self.id,
                self.resources.supplies,
                self.power_available,
            );
        }
    }

    pub fn credit_supplies(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        self.statistics.resources_collected =
            self.statistics.resources_collected.saturating_add(amount);
        crate::game_logic::host_economy_log::record_money_audio(
            self.id,
            crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
        );

        self.apply_supply_gain(amount);
    }

    pub fn queue_upgrade(&mut self, upgrade_name: &str, cost: &Resources) -> bool {
        // C++ ProductionUpdate.cpp:250-272 — PLAYER refuses if complete or
        // already in production. OBJECT is per-producer (`giveUpgrade`), not
        // a player-wide unlock (one add-on per unit, not per player).
        let object_scoped =
            crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name);
        if !object_scoped
            && (self.has_unlocked_upgrade(upgrade_name) || self.has_queued_upgrade(upgrade_name))
        {
            return false;
        }
        if !self.spend_resources(cost) {
            return false;
        }
        self.queued_upgrades.insert(upgrade_name.to_string());
        true
    }

    /// Cancel a queued upgrade and refund the requested resources.
    pub fn cancel_queued_upgrade(&mut self, upgrade_name: &str, refund: &Resources) -> bool {
        let Some(queued_name) = self.find_queued_upgrade_name(upgrade_name) else {
            return false;
        };
        self.queued_upgrades.remove(&queued_name);
        self.apply_supply_gain(refund.supplies);
        self.power_available -= refund.power;
        crate::game_logic::host_economy_log::record(
            self.id,
            self.effective_supplies(),
            self.power_available,
        );
        true
    }

    /// Mark research finished. OBJECT upgrades stay off the player completed set.
    pub fn complete_researched_upgrade(&mut self, upgrade_name: &str) {
        if let Some(queued) = self.find_queued_upgrade_name(upgrade_name) {
            self.queued_upgrades.remove(&queued);
        }
        // C++ ProductionUpdate.cpp:874-879 / 931 — purchased, not granted.
        self.record_upgrade_production_complete(upgrade_name);
        if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name) {
            return;
        }
        self.add_completed_upgrade(upgrade_name);
    }

    /// Complete all queued player upgrades into the unlocked upgrade/science set.
    pub fn complete_queued_upgrades(&mut self) -> Vec<String> {
        let mut completed: Vec<String> = self.queued_upgrades.drain().collect();
        completed.sort();
        for upgrade in &completed {
            if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade) {
                continue;
            }
            self.add_completed_upgrade(upgrade);
        }
        completed
    }

    pub fn has_unlocked_upgrade(&self, upgrade_name: &str) -> bool {
        let expected = normalize_upgrade_name(upgrade_name);
        self.unlocked_sciences
            .iter()
            .chain(self.completed_upgrades.iter())
            .any(|unlocked| normalize_upgrade_name(unlocked) == expected)
    }

    pub fn has_unlocked_science(&self, science_name: &str) -> bool {
        self.has_unlocked_upgrade(science_name)
    }

    pub fn unlock_science(&mut self, science_name: &str) -> bool {
        if self.has_unlocked_science(science_name) {
            return false;
        }
        let inserted = self.unlocked_sciences.insert(science_name.to_string());
        // C++ Player::addScience never sets m_cashBountyPercent. CashBountyPower
        // on an existing palace module is the only setter (onSpecialPowerCreation).
        if inserted {
            self.record_host_sciences();
        }
        inserted
    }

    /// C++ `Player::grantScience` — refuse when `ScienceStore::isScienceGrantable` is false.
    pub fn grant_science(&mut self, science_name: &str) -> bool {
        if !host_science_is_grantable(science_name) {
            return false;
        }
        self.unlock_science(science_name)
    }

    /// C++ Player::resetSciences / IntrinsicSciences + Rank1 residual at match start.
    ///
    /// Grants faction SCIENCE_AMERICA/CHINA/GLA, SCIENCE_Rank1, and Rank1
    /// SciencePurchasePointsGranted (**1**). Fail-closed: not full PlayerTemplate
    /// multi-science vector / multiplayer override matrix.
    pub fn apply_faction_intrinsic_sciences(&mut self) {
        use crate::game_logic::host_faction_skirmish_residual::intrinsic_science_for_team;
        use crate::game_logic::host_science_rank::{
            RANK_SCIENCE_POINTS_DEFAULT, SCIENCE_RANK1, retail_rank_for_level,
        };
        if let Some(sci) = intrinsic_science_for_team(self.team) {
            self.unlocked_sciences.insert(sci.to_string());
        }
        self.unlocked_sciences.insert(SCIENCE_RANK1.to_string());
        // Rank level starts at 1 residual.
        if self.rank_level < 1 {
            self.rank_level = 1;
        }
        // Ensure at least Rank1 science purchase points residual if still zero.
        if self.science_purchase_points <= 0 {
            let grant = retail_rank_for_level(1)
                .map(|r| r.science_purchase_points_granted)
                .unwrap_or(RANK_SCIENCE_POINTS_DEFAULT);
            self.science_purchase_points = grant;
        }
    }

    /// C++ `Player::init(PlayerTemplate)` followed by `resetRank` /
    /// `resetSciences` for a concrete Campaign or Challenge General.
    ///
    /// The caller has already resolved the exact template identity.  Unlike
    /// the base-team fallback above, this preserves every authored intrinsic
    /// science and its template purchase-point grant before adding the Rank1
    /// grant.  Template money of zero retains Main's current GameInfo/default
    /// starting cash, matching the C++ fallback.
    pub(crate) fn apply_player_template_start_state(
        &mut self,
        template: &game_engine::common::rts::player_template::PlayerTemplate,
    ) {
        use crate::game_logic::host_science_rank::{
            RANK_SCIENCE_POINTS_DEFAULT, SCIENCE_RANK1, retail_rank_for_level,
        };

        let starting_money = template.get_money().count_money();
        if starting_money != 0 {
            self.resources.supplies = starting_money;
        }

        let color = template.get_preferred_color();
        self.color_rgb = (
            ((color >> 16) & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            (color & 0xff) as u8,
        );
        self.color_night_rgb = self.color_rgb;

        self.is_observer = template.is_observer();
        self.is_alive = !self.is_observer;
        self.rank_level = 1;
        self.skill_points = 0;
        self.unlocked_sciences.clear();
        self.unlocked_sciences
            .extend(template.get_intrinsic_sciences().iter().cloned());
        self.unlocked_sciences.insert(SCIENCE_RANK1.to_string());

        let rank1_grant = retail_rank_for_level(1)
            .map(|rank| rank.science_purchase_points_granted)
            .unwrap_or(RANK_SCIENCE_POINTS_DEFAULT);
        self.science_purchase_points = template
            .get_intrinsic_science_purchase_points()
            .saturating_add(rank1_grant);
        self.record_host_sciences();
        self.record_host_progress();
    }

    /// C++ Player::isCapableOfPurchasingScience residual.
    pub fn is_capable_of_purchasing_science(&self, science_name: &str) -> bool {
        if self.is_science_disabled(science_name) || self.is_science_hidden(science_name) {
            return false;
        }
        if crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::leftover_science_hidden_or_disabled(
            self.id,
            science_name,
        ) {
            return false;
        }
        crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::is_capable_of_purchasing_science_residual(
            &self.unlocked_sciences,
            self.science_purchase_points,
            science_name,
        )
    }

    /// C++ Player::attemptToPurchaseScience residual.
    ///
    /// Spends **science purchase points** (not supplies). Cost 0 = not purchasable.
    pub fn attempt_to_purchase_science(&mut self, science_name: &str) -> bool {
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::{
            normalize_science_name_residual, science_purchase_point_cost_residual,
        };
        let canonical = normalize_science_name_residual(science_name);
        if !self.is_capable_of_purchasing_science(&canonical) {
            return false;
        }
        let cost = science_purchase_point_cost_residual(&canonical).unwrap_or(0);
        if cost <= 0 || cost > self.science_purchase_points {
            return false;
        }
        self.science_purchase_points -= cost;
        // Wave 202: SPP spend must last-write SetPlayerProgress (sciences meta already
        // records via unlock_science → record_host_sciences).
        let unlocked = self.unlock_science(&canonical);
        if unlocked {
            self.record_host_progress();
            crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::sync_host_science_to_crate_player(
                self.id,
                &canonical,
            );
            crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::sync_host_spp_to_crate_player(
                self.id,
                self.science_purchase_points,
            );
        }
        unlocked
    }

    pub fn has_queued_upgrade(&self, upgrade_name: &str) -> bool {
        self.find_queued_upgrade_name(upgrade_name).is_some()
    }

    pub fn find_queued_upgrade_name(&self, upgrade_name: &str) -> Option<String> {
        let expected = normalize_upgrade_name(upgrade_name);
        self.queued_upgrades
            .iter()
            .find(|queued| normalize_upgrade_name(queued) == expected)
            .cloned()
    }

    pub fn record_unit_destroyed(&mut self) {
        self.statistics.units_destroyed = self.statistics.units_destroyed.saturating_add(1);
    }

    pub fn record_unit_lost(&mut self) {
        self.statistics.units_lost = self.statistics.units_lost.saturating_add(1);
    }

    pub fn record_unit_produced(&mut self) {
        self.statistics.units_built = self.statistics.units_built.saturating_add(1);
    }

    pub fn record_structure_built(&mut self) {
        self.statistics.structures_built = self.statistics.structures_built.saturating_add(1);
    }

    pub fn record_structure_destroyed(&mut self) {
        self.statistics.structures_destroyed =
            self.statistics.structures_destroyed.saturating_add(1);
    }

    /// C++ `ScoreKeeper::addObjectDestroyed` still increments the per-victim
    /// array for self-kills; `calculateScore` skips `i == m_myPlayerIdx`.
    pub fn record_self_unit_destroyed(&mut self) {
        self.statistics.units_destroyed_self =
            self.statistics.units_destroyed_self.saturating_add(1);
    }

    pub fn record_self_structure_destroyed(&mut self) {
        self.statistics.structures_destroyed_self =
            self.statistics.structures_destroyed_self.saturating_add(1);
    }

    pub fn record_structure_lost(&mut self) {
        self.statistics.structures_lost = self.statistics.structures_lost.saturating_add(1);
    }

    /// C++ `ScoreKeeper::calculateScore` — self-kills stay in display totals
    /// but never enter the score.
    pub fn calculate_score(&self) -> i32 {
        let s = &self.statistics;
        let enemy_units = s.units_destroyed.saturating_sub(s.units_destroyed_self);
        let enemy_structures = s
            .structures_destroyed
            .saturating_sub(s.structures_destroyed_self);
        (s.units_built as i32)
            .saturating_mul(100)
            .saturating_add(s.money_earned as i32)
            .saturating_add((s.structures_built as i32).saturating_mul(100))
            .saturating_add((enemy_units as i32).saturating_mul(100))
            .saturating_add((enemy_structures as i32).saturating_mul(100))
    }

    pub fn record_resources_spent(&mut self, amount: u32) {
        self.statistics.resources_spent = self.statistics.resources_spent.saturating_add(amount);
    }

    /// C++ `Player::initFromDict` money/color/handicap.
    ///
    /// `replace_default_money` is the campaign/map-create path: dict
    /// `playerStartMoney` replaces `Player::new`'s $10k fallback.
    /// Skirmish lobby cash is applied separately (`replace_default_money=false`).
    pub fn apply_map_side_dict(&mut self, dict: &Dict, replace_default_money: bool) {
        let map_name = dict.get_ascii_string(key_player_name());
        if !map_name.is_empty() {
            self.map_side.map_player_name = map_name;
        }

        if replace_default_money && dict.get_type(key_player_start_money()).is_some() {
            // C++ deposits onto template money (usually 0). Host `new` planted
            // DEFAULT as fallback when no map key existed.
            self.resources.supplies = dict.get_int(key_player_start_money()).max(0) as u32;
        }

        if dict.get_type(key_player_color()).is_some() {
            let color = dict.get_int(key_player_color()) as u32;
            self.color_rgb = (
                ((color >> 16) & 0xff) as u8,
                ((color >> 8) & 0xff) as u8,
                (color & 0xff) as u8,
            );
            self.color_night_rgb = self.color_rgb;
        }
        if dict.get_type(key_player_night_color()).is_some() {
            let color = dict.get_int(key_player_night_color()) as u32;
            self.color_night_rgb = (
                ((color >> 16) & 0xff) as u8,
                ((color >> 8) & 0xff) as u8,
                (color & 0xff) as u8,
            );
        }

        self.map_side.read_handicap_from_dict(dict);
    }

    /// C++ `Player::setPlayerRelationship`.
    pub fn set_map_relationship(
        &mut self,
        other_player_id: u32,
        relationship: gamelogic::common::Relationship,
    ) {
        self.map_side
            .relations
            .insert(other_player_id, relationship);
    }

    /// C++ `Player::getRelationship` map lookup (missing → Neutral at the caller).
    pub fn map_relationship(
        &self,
        other_player_id: u32,
    ) -> Option<gamelogic::common::Relationship> {
        self.map_side.relations.get(&other_player_id).copied()
    }

    /// C++ `Handicap::getHandicap(BUILDTIME, …)`.
    pub fn handicap_build_time_multiplier(&self, is_structure: bool) -> f32 {
        if is_structure {
            self.map_side.handicap_build_time_buildings
        } else {
            self.map_side.handicap_build_time_generic
        }
    }

    fn normalize_team_instance_name(name: &str) -> String {
        name.trim().to_ascii_lowercase()
    }

    /// C++ `Player::setTeamRelationship`.
    pub fn set_team_relationship_override(
        &mut self,
        team_name: &str,
        relationship: gamelogic::common::Relationship,
    ) {
        let key = Self::normalize_team_instance_name(team_name);
        if key.is_empty() {
            return;
        }
        self.team_relations.insert(key, relationship);
    }

    /// C++ `Player::removeTeamRelationship` for one named team.
    pub fn remove_team_relationship_override(&mut self, team_name: &str) -> bool {
        let key = Self::normalize_team_instance_name(team_name);
        if key.is_empty() {
            return false;
        }
        self.team_relations.remove(&key).is_some()
    }

    /// C++ `Player::getRelationship(const Team*)` team-id map lookup.
    pub fn team_relationship_override(
        &self,
        team_name: &str,
    ) -> Option<gamelogic::common::Relationship> {
        let key = Self::normalize_team_instance_name(team_name);
        if key.is_empty() {
            return None;
        }
        self.team_relations.get(&key).copied()
    }

    /// C++ `Team::setOverrideTeamRelationship` drained onto the host player.
    pub fn set_team_instance_team_override(
        &mut self,
        source_team: &str,
        target_team: &str,
        relationship: gamelogic::common::Relationship,
    ) {
        let source = Self::normalize_team_instance_name(source_team);
        let target = Self::normalize_team_instance_name(target_team);
        if source.is_empty() || target.is_empty() {
            return;
        }
        self.team_instance_team_relations
            .entry(source)
            .or_default()
            .insert(target, relationship);
    }

    /// C++ `Team::removeOverrideTeamRelationship` for one named target team.
    pub fn remove_team_instance_team_override(
        &mut self,
        source_team: &str,
        target_team: &str,
    ) -> bool {
        let source = Self::normalize_team_instance_name(source_team);
        let target = Self::normalize_team_instance_name(target_team);
        if source.is_empty() || target.is_empty() {
            return false;
        }
        let Some(inner) = self.team_instance_team_relations.get_mut(&source) else {
            return false;
        };
        let removed = inner.remove(&target).is_some();
        if inner.is_empty() {
            self.team_instance_team_relations.remove(&source);
        }
        removed
    }

    /// C++ `Team::m_teamRelations` lookup by source/target team instance name.
    pub fn team_instance_team_override(
        &self,
        source_team: &str,
        target_team: &str,
    ) -> Option<gamelogic::common::Relationship> {
        let source = Self::normalize_team_instance_name(source_team);
        let target = Self::normalize_team_instance_name(target_team);
        if source.is_empty() || target.is_empty() {
            return None;
        }
        self.team_instance_team_relations
            .get(&source)
            .and_then(|inner| inner.get(&target).copied())
    }

    /// C++ `Team::setOverridePlayerRelationship` drained onto the host player.
    pub fn set_team_instance_player_override(
        &mut self,
        source_team: &str,
        target_player_id: u32,
        relationship: gamelogic::common::Relationship,
    ) {
        let source = Self::normalize_team_instance_name(source_team);
        if source.is_empty() {
            return;
        }
        self.team_instance_player_relations
            .entry(source)
            .or_default()
            .insert(target_player_id, relationship);
    }

    /// C++ `Team::removeOverridePlayerRelationship` for one player index.
    pub fn remove_team_instance_player_override(
        &mut self,
        source_team: &str,
        target_player_id: u32,
    ) -> bool {
        let source = Self::normalize_team_instance_name(source_team);
        if source.is_empty() {
            return false;
        }
        let Some(inner) = self.team_instance_player_relations.get_mut(&source) else {
            return false;
        };
        let removed = inner.remove(&target_player_id).is_some();
        if inner.is_empty() {
            self.team_instance_player_relations.remove(&source);
        }
        removed
    }

    /// C++ `Team::m_playerRelations` lookup by source team and target player.
    pub fn team_instance_player_override(
        &self,
        source_team: &str,
        target_player_id: u32,
    ) -> Option<gamelogic::common::Relationship> {
        let source = Self::normalize_team_instance_name(source_team);
        if source.is_empty() {
            return None;
        }
        self.team_instance_player_relations
            .get(&source)
            .and_then(|inner| inner.get(&target_player_id).copied())
    }

    /// C++ `Team::removeOverrideTeamRelationship(NULL)` + player-map clear.
    pub fn clear_team_instance_overrides(&mut self, source_team: &str) {
        let source = Self::normalize_team_instance_name(source_team);
        if source.is_empty() {
            return;
        }
        self.team_instance_team_relations.remove(&source);
        self.team_instance_player_relations.remove(&source);
    }

    /// C++ `Player::isScienceDisabled`.
    pub fn is_science_disabled(&self, science_name: &str) -> bool {
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual;
        let key = normalize_science_name_residual(science_name);
        if key.is_empty() {
            return false;
        }
        self.sciences_disabled
            .iter()
            .any(|s| normalize_science_name_residual(s) == key)
    }

    /// C++ `Player::isScienceHidden`.
    pub fn is_science_hidden(&self, science_name: &str) -> bool {
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual;
        let key = normalize_science_name_residual(science_name);
        if key.is_empty() {
            return false;
        }
        self.sciences_hidden
            .iter()
            .any(|s| normalize_science_name_residual(s) == key)
    }

    /// C++ `Player::setScienceAvailability`.
    pub fn set_science_availability(&mut self, science_name: &str, availability: &str) -> bool {
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual;
        let key = normalize_science_name_residual(science_name);
        if key.is_empty() {
            return false;
        }
        let avail = availability.trim();
        if avail.eq_ignore_ascii_case("Available") {
            self.sciences_disabled
                .retain(|s| normalize_science_name_residual(s) != key);
            self.sciences_hidden
                .retain(|s| normalize_science_name_residual(s) != key);
            true
        } else if avail.eq_ignore_ascii_case("Disabled") {
            self.sciences_hidden
                .retain(|s| normalize_science_name_residual(s) != key);
            self.sciences_disabled.insert(key);
            true
        } else if avail.eq_ignore_ascii_case("Hidden") {
            self.sciences_disabled
                .retain(|s| normalize_science_name_residual(s) != key);
            self.sciences_hidden.insert(key);
            true
        } else {
            false
        }
    }

    /// C++ `Handicap::getHandicap(BUILDCOST, …)`.
    pub fn handicap_build_cost_multiplier(&self, is_structure: bool) -> f32 {
        if is_structure {
            self.map_side.handicap_build_cost_buildings
        } else {
            self.map_side.handicap_build_cost_generic
        }
    }
}

impl Player {
    /// C++ Player::allowedToBuild (Player.cpp:2295-2305).
    pub fn allowed_to_build(&self, is_structure: bool) -> bool {
        if !self.can_build_base && is_structure {
            return false;
        }
        if !self.can_build_units && !is_structure {
            return false;
        }
        true
    }

    /// C++ Player::setCanBuildUnits.
    pub fn set_can_build_units(&mut self, can_build: bool) {
        self.can_build_units = can_build;
    }

    /// C++ Player::setCanBuildBase.
    pub fn set_can_build_base(&mut self, can_build: bool) {
        self.can_build_base = can_build;
    }
}

/// C++ `ScienceStore::isScienceGrantable` — unknown science is not grantable.
fn host_science_is_grantable(science_name: &str) -> bool {
    let name = science_name.trim();
    if name.is_empty() {
        return false;
    }
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return store.is_science_grantable(science);
        }
    }
    let leftover = game_engine::common::ini::ini_science::get_science_store();
    if let Some(st) = leftover.get_science_from_internal_name(name) {
        return leftover.is_science_grantable(st);
    }
    crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::science_store_row_wave109(
        name,
    )
    .map(|row| row.grantable)
    .unwrap_or(false)
}

pub(super) fn normalize_upgrade_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub(super) fn capture_upgrade_names_for_team(team: Team) -> &'static [&'static str] {
    match team {
        Team::USA => &[
            "Upgrade_AmericaRangerCaptureBuilding",
            "Upgrade_InfantryCaptureBuilding",
        ],
        Team::China => &[
            "Upgrade_ChinaRedguardCaptureBuilding",
            "Upgrade_InfantryCaptureBuilding",
        ],
        Team::GLA => &[
            "Upgrade_GLARebelCaptureBuilding",
            "Upgrade_InfantryCaptureBuilding",
        ],
        Team::Neutral => &[],
    }
}

/// Skirmish/match rules applied from UI configuration (FOW, crates, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct SkirmishRulesState {
    pub fog_of_war: bool,
    pub crates_enabled: bool,
    pub limit_superweapons: bool,
    pub allow_tech_buildings: bool,
    pub game_speed: f32,
}

impl Default for SkirmishRulesState {
    fn default() -> Self {
        Self {
            fog_of_war: true,
            crates_enabled: true,
            limit_superweapons: false,
            allow_tech_buildings: true,
            game_speed: 1.0,
        }
    }
}

/// Main GameLogic system

/// C++ BuildAssistant FRAMES_TO_ALLOW_SCAFFOLD residual (LOGICFRAMES_PER_SECOND * 1.5 = 45).

/// C++ FactionBuilding.ini RebuildHoleBehavior WorkerRespawnDelay = 20000ms → 600f.
pub(super) const REBUILD_HOLE_WORKER_RESPAWN_FRAMES: u32 = 600;
/// C++ RebuildHoleExposeDie HoleMaxHealth residual default for GLA holes.
/// Used only when the hole ThingTemplate is missing; never overwrite INI HP.
pub(super) const REBUILD_HOLE_MAX_HEALTH_RESIDUAL: f32 = 500.0;
/// C++ FactionBuilding.ini HoleHealthRegen%PerSecond = 0.5% → 0.005 / sec.
pub(super) const REBUILD_HOLE_HEALTH_REGEN_PERCENT_PER_SEC: f32 = 0.005;
/// C++ FactionBuilding.ini WorkerObjectName = GLAInfantryWorker.
pub(super) const REBUILD_HOLE_WORKER_TEMPLATE: &str = "GLAInfantryWorker";
pub(super) const FRAMES_TO_ALLOW_SCAFFOLD_RESIDUAL: u32 = 45;
/// C++ TOTAL_FRAMES_TO_SELL_OBJECT residual (LOGICFRAMES_PER_SECOND * 3.0 = 90).
pub(super) const TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL: u32 = 90;
/// C++ construction percent is 0..100; host uses 0..1. Decrement per frame after scaffold.
pub(super) const SELL_CONSTRUCTION_DECREMENT_RESIDUAL: f32 =
    1.0 / (TOTAL_FRAMES_TO_SELL_OBJECT_RESIDUAL as f32);
/// C++ finish threshold constructionPercent <= -50.0 (host -0.5).
pub(super) const SELL_FINISH_CONSTRUCTION_PERCENT_RESIDUAL: f32 = -0.5;

/// C++ ObjectSellInfo residual.
#[derive(Debug, Clone)]
pub(super) struct ObjectSellInfo {
    pub(super) id: ObjectId,
    pub(super) sell_frame: u32,
}

/// One live C++ `ParkingPlaceBehavior::ParkingPlaceInfo` slot.
///
/// This is deliberately not a generic building `garrisoned_units` entry:
/// ParkingPlace tracks a per-space reservation while an aircraft is returning
/// as well as while it is physically parked.  `reserved_for_exit` is retained
/// so a later authored production-exit bridge cannot treat an exit door as a
/// free landing slot.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct AirfieldParkingSpace {
    pub(super) object_id: Option<ObjectId>,
    pub(super) reserved_for_exit: bool,
}

/// C++ `ParkingPlaceBehavior::HealingInfo` — healee list, not containment.
#[derive(Debug, Clone, Copy)]
pub(super) struct AirfieldHealingInfo {
    pub(super) getting_healed_id: ObjectId,
    pub(super) heal_start_frame: u32,
}

/// C++ `HeliTakeoffOrLandingState` two-point path (JetAIUpdate.cpp:961-1125).
#[derive(Debug, Clone, Copy)]
pub(super) struct HostHeliTakeoffOrLanding {
    pub(super) path: [glam::Vec3; 2],
    pub(super) index: u8,
    pub(super) landing: bool,
    pub(super) airfield_id: ObjectId,
}

/// Fat-object ID store as its **own field** so a tick can mut-borrow objects
/// without `&mut self` on the whole [`GameLogic`] (`self.objects.get_mut` +
/// `self.frame` split-borrow).
///
/// Deref to the inner `HashMap` so existing `self.objects.get_mut` call sites
/// keep compiling. When a GameWorld session is coupled the map is a roster /
/// read-view — `host_authoritative_*` is truth.
#[derive(Debug, Default)]
pub struct HostObjectStore {
    pub(super) map: HashMap<ObjectId, Object>,
}

impl HostObjectStore {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    #[inline]
    pub fn map(&self) -> &HashMap<ObjectId, Object> {
        &self.map
    }

    #[inline]
    pub fn map_mut(&mut self) -> &mut HashMap<ObjectId, Object> {
        &mut self.map
    }
}

impl std::ops::Deref for HostObjectStore {
    type Target = HashMap<ObjectId, Object>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl std::ops::DerefMut for HostObjectStore {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl<'a> IntoIterator for &'a HostObjectStore {
    type Item = (&'a ObjectId, &'a Object);
    type IntoIter = std::collections::hash_map::Iter<'a, ObjectId, Object>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a> IntoIterator for &'a mut HostObjectStore {
    type Item = (&'a ObjectId, &'a mut Object);
    type IntoIter = std::collections::hash_map::IterMut<'a, ObjectId, Object>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter_mut()
    }
}

#[cfg(test)]
mod map_side_dict_tests {
    use super::*;

    #[test]
    fn map_start_money_replaces_default_ten_k() {
        let mut player = Player::new(0, Team::USA, "PlyrAmerica", true);
        assert_eq!(player.resources.supplies, Player::DEFAULT_STARTING_MONEY);
        let mut dict = Dict::new();
        dict.set_int(key_player_start_money(), 2_500);
        dict.set_int(key_player_color(), 0x00aa_3311);
        dict.set_ascii_string(key_player_name(), "PlyrAmerica");
        player.apply_map_side_dict(&dict, true);
        assert_eq!(player.resources.supplies, 2_500);
        assert_eq!(player.color_rgb, (0xaa, 0x33, 0x11));
        assert_eq!(player.map_side.map_player_name, "PlyrAmerica");
    }

    #[test]
    fn lobby_cash_is_not_clobbered_by_map_start_money() {
        let mut player = Player::new(0, Team::USA, "Human", true);
        player.resources.supplies = 20_000;
        let mut dict = Dict::new();
        dict.set_int(key_player_start_money(), 2_500);
        player.apply_map_side_dict(&dict, false);
        assert_eq!(player.resources.supplies, 20_000);
    }

    #[test]
    fn handicap_keys_apply_from_dict() {
        let mut player = Player::new(0, Team::USA, "P", true);
        let mut dict = Dict::new();
        dict.set_real(
            NameKeyGenerator::name_to_key("HANDICAP_BUILDCOST_BUILDINGS"),
            0.75,
        );
        player.apply_map_side_dict(&dict, false);
        assert!((player.handicap_build_cost_multiplier(true) - 0.75).abs() < f32::EPSILON);
        assert!((player.handicap_build_cost_multiplier(false) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn object_upgrade_does_not_complete_into_player_set() {
        let mut player = Player::new(0, Team::China, "China", true);
        player.resources.supplies = 10_000;
        let cost = Resources {
            supplies: 100,
            power: 0,
        };
        assert!(player.queue_upgrade("Upgrade_ChinaOverlordBattleBunker", &cost));
        player.complete_researched_upgrade("Upgrade_ChinaOverlordBattleBunker");
        assert!(
            !player.has_unlocked_upgrade("Upgrade_ChinaOverlordBattleBunker"),
            "OBJECT upgrades must not enter the player completed set"
        );
        assert!(
            player.queue_upgrade("Upgrade_ChinaOverlordBattleBunker", &cost),
            "a second unit must still be able to queue the same OBJECT upgrade"
        );
    }

    #[test]
    fn player_upgrade_still_refuses_duplicate_queue() {
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 10_000;
        let cost = Resources {
            supplies: 100,
            power: 0,
        };
        assert!(player.queue_upgrade("Upgrade_AmericaSupplyLines", &cost));
        assert!(!player.queue_upgrade("Upgrade_AmericaSupplyLines", &cost));
        player.complete_researched_upgrade("Upgrade_AmericaSupplyLines");
        assert!(player.has_unlocked_upgrade("Upgrade_AmericaSupplyLines"));
        assert!(!player.queue_upgrade("Upgrade_AmericaSupplyLines", &cost));
    }

    #[test]
    fn handicap_time_keys_apply_from_dict() {
        let mut player = Player::new(0, Team::USA, "P", true);
        let mut dict = Dict::new();
        dict.set_real(
            NameKeyGenerator::name_to_key("HANDICAP_BUILDTIME_BUILDINGS"),
            0.5,
        );
        player.apply_map_side_dict(&dict, false);
        assert!((player.handicap_build_time_multiplier(true) - 0.5).abs() < f32::EPSILON);
        assert!((player.handicap_build_time_multiplier(false) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn science_hide_and_disable_gate_purchase() {
        // C++ Player.cpp:2616-2618 isCapableOfPurchasingScience.
        let mut player = Player::new(0, Team::USA, "P", true);
        player.unlocked_sciences.insert("SCIENCE_Rank1".into());
        player.science_purchase_points = 5;
        assert!(player.set_science_availability("SCIENCE_DaisyCutter", "Hidden"));
        assert!(player.is_science_hidden("SCIENCE_DaisyCutter"));
        assert!(!player.is_capable_of_purchasing_science("SCIENCE_DaisyCutter"));
        assert!(!player.attempt_to_purchase_science("SCIENCE_DaisyCutter"));
        assert!(player.set_science_availability("SCIENCE_DaisyCutter", "Available"));
        assert!(!player.is_science_hidden("SCIENCE_DaisyCutter"));
        assert!(player.set_science_availability("SCIENCE_DaisyCutter", "Disabled"));
        assert!(player.is_science_disabled("SCIENCE_DaisyCutter"));
        assert!(!player.is_capable_of_purchasing_science("SCIENCE_DaisyCutter"));
    }

    #[test]
    fn handicap_multiplies_live_build_cost_and_time() {
        // C++ ThingTemplate.cpp:1508-1527 calcCostToBuild / calcTimeToBuild.
        let mut logic = GameLogic::new();
        let mut p = Player::new(0, Team::USA, "P", true);
        p.map_side.handicap_build_cost_buildings = 0.75;
        p.map_side.handicap_build_time_buildings = 0.5;
        logic.add_player(p);
        let mut factory = ThingTemplate::new("WarFactory");
        factory.add_kind_of(KindOf::Structure);
        logic.templates.insert("WarFactory".into(), factory);
        assert_eq!(
            logic.modified_build_cost_supplies(0, "WarFactory", 1000),
            750
        );
        let secs = logic.modified_build_time_seconds(0, "WarFactory", 10.0);
        // 10s * 30 = 300 frames * 0.5 handicap = 150 frames + 0.25 encoding / 30.
        let expected = (150.0 + 0.25) / 30.0;
        assert!(
            (secs - expected).abs() < 1e-5,
            "handicap time seconds {secs} vs {expected}"
        );
    }

    #[test]
    fn has_radar_honors_disable_proof_through_brownout() {
        // C++ Player.cpp:3207-3213 — disable-proof van stays up when radar_disabled.
        let mut player = Player::new(0, Team::GLA, "GLA", true);
        player.add_radar(true);
        assert!(player.has_radar());
        assert_eq!(player.disable_proof_radar_count, 1);
        player.disable_radar();
        assert!(
            player.has_radar(),
            "Radar Van DisableProof must survive brownout"
        );
        player.remove_radar(true);
        assert!(!player.has_radar());
        assert_eq!(player.disable_proof_radar_count, 0);
        player.add_radar(false);
        player.disable_radar();
        assert!(
            !player.has_radar(),
            "ordinary CC radar goes dark on brownout"
        );
        player.enable_radar();
        assert!(player.has_radar());
    }

    #[test]
    fn upgrade_complete_records_leftover_academy_and_score_spent() {
        // C++ ProductionUpdate.cpp:874-879 / 931.
        struct LeftoverGuard;
        impl Drop for LeftoverGuard {
            fn drop(&mut self) {
                if let Ok(mut list) = gamelogic::player::ThePlayerList().write() {
                    list.clear();
                }
            }
        }
        let _guard = LeftoverGuard;
        let leftover = gamelogic::player::Player::new(0);
        let leftover = std::sync::Arc::new(std::sync::RwLock::new(leftover));
        {
            let mut list = gamelogic::player::ThePlayerList()
                .write()
                .unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(std::sync::Arc::clone(&leftover));
        }
        const NAME: &str = "Upgrade_W4106AcademyRadar";
        gamelogic::upgrade::center::with_upgrade_center_mut(|center| {
            let mut ini = game_engine::common::ini::INI::new();
            let source =
                format!("{NAME}\nBuildCost = 800\nAcademyClassify = ACT_UPGRADE_RADAR\nEnd\n");
            ini.with_inline_source(&source, |ini| {
                // parse_upgrade_definition expects the upgrade-name line
                // already staged in the tokenizer buffer (INI::get_next_token
                // reads from the current line, see the sibling registration
                // helper in object_ai_combat.rs::register_upgrade_completion_sounds).
                ini.read_line()?;
                center
                    .parse_upgrade_definition(ini)
                    .map_err(|_| game_engine::common::ini::INIError::InvalidData)
            })
            .expect("register leftover upgrade");
        });
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.complete_researched_upgrade(NAME);
        let leftover_guard = leftover.read().expect("leftover player");
        assert!(
            leftover_guard.get_academy_stats().has_researched_radar(),
            "purchased ACT_UPGRADE_RADAR must set leftover researched_radar"
        );
        assert_eq!(
            leftover_guard.get_academy_stats().get_upgrades_purchased(),
            1,
            "purchased (not granted) upgrade increments leftover upgrades_purchased"
        );
        assert_eq!(
            leftover_guard.get_score_keeper().get_total_money_spent(),
            800,
            "complete must add leftover ScoreKeeper money spent"
        );
    }
}
