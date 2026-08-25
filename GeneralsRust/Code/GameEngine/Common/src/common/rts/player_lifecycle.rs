use super::*;

impl Player {
    pub(super) fn current_player_template(&self) -> Option<PlayerTemplate> {
        let store = crate::common::rts::player_template::get_player_template_store();

        if !self.player_name.is_empty() {
            if let Some(template) = store.find_template(&self.player_name) {
                return Some(template.clone());
            }
        }

        if !self.side.is_empty() {
            if let Some(template) = store.iter().find(|template| template.side == self.side) {
                return Some(template.clone());
            }
        }

        if !self.base_side.is_empty() {
            if let Some(template) = store
                .iter()
                .find(|template| template.base_side == self.base_side)
            {
                return Some(template.clone());
            }
        }

        None
    }

    pub(super) fn refresh_academy_base_side_context(&mut self) {
        if let Some(template) = self.current_player_template() {
            self.academy_stats
                .set_base_side_context(&template.base_side);
            self.observer = template.is_observer;
            if self.observer {
                self.is_player_dead = true;
            }
        } else {
            self.academy_stats.set_base_side_context(&self.base_side);
        }
    }

    /// Create a new Player with the given index
    ///
    /// C++ Reference: Player::Player() (Player.cpp lines 193-250)
    pub fn new(index: i32) -> Self {
        // C++ lines 195-199: Initialize state flags
        let is_preorder = false;
        let is_player_dead = false;

        // C++ lines 202-204: Allocate relation maps
        let player_relations = PlayerRelationMap::new();

        // C++ lines 225-228: Initialize attacked tracking
        let attacked_by = vec![false; super::super::player_list::MAX_PLAYER_COUNT as usize];
        let attacked_frame = 0;

        // C++ lines 230-234: Units should hunt
        let units_should_hunt = false;

        let player = Self {
            index,
            player_display_name: String::new(),
            player_name: String::new(),
            side: String::new(),
            base_side: String::new(),
            player_type: PlayerType::Computer,
            money: Money::new(),
            energy: Energy::new(),
            mission_stats: MissionStats::new(),
            handicap: Handicap::new(),
            score_keeper: ScoreKeeper::new(),
            academy_stats: AcademyStats::new(),
            sciences: HashSet::new(),
            sciences_disabled: HashSet::new(),
            sciences_hidden: HashSet::new(),
            science_purchase_points: 0,
            skill_points: 0,
            rank_level: 0,
            level_up: 0,
            level_down: 0,
            skill_points_modifier: 1.0,
            general_name: String::new(),
            player_relations,
            team_relations: super::super::team::TeamRelationMap::new(),
            default_team: None,
            mp_start_index: 0,
            radar_count: 0,
            disable_proof_radar_count: 0,
            radar_disabled: false,
            bombard_battle_plans: 0,
            hold_the_line_battle_plans: 0,
            search_and_destroy_battle_plans: 0,
            battle_plan_bonuses: None,
            can_build_units: true,
            can_build_base: true,
            is_player_dead,
            observer: false,
            is_preorder,
            list_in_score_screen: true,
            units_should_hunt,
            logical_retaliation_mode_enabled: false,
            cash_bounty_percent: 0.0,
            attacked_by,
            attacked_frame,
            // AI System
            ai: None,
            difficulty: GameDifficulty::Normal,
            // Build List
            build_list: None,
            // Resource Gathering
            supply_centers: Vec::new(),
            supply_warehouses: Vec::new(),
            // Squad System - initialize with empty squads
            hotkey_squads: Default::default(),
            current_selection: Squad::new(),
            // Upgrade System
            upgrade_list: Vec::new(),
            upgrades_in_progress: 0,
            upgrades_completed: 0,
            // Team prototypes
            team_prototypes: Vec::new(),
            // Tunnel system
            tunnel_entrances: Vec::new(),
            // Production changes
            production_cost_changes: HashMap::new(),
            production_time_changes: HashMap::new(),
            kind_of_production_cost_changes: Vec::new(),

            // Special Power Timers
            special_power_timers: HashMap::new(),
            owned_objects: Vec::new(),
            is_local: false,
        };

        player
    }

    // =========================================================
    // Accessor Methods
    // =========================================================

    /// Get the player index
    /// C++ Reference: Player::getPlayerIndex() (Player.h line 162)
    pub fn get_player_index(&self) -> i32 {
        self.index
    }

    /// Get a bitmask that is unique to this player
    /// C++ Reference: Player::getPlayerMask() (Player.h line 164)
    pub fn get_player_mask(&self) -> u32 {
        1 << self.index
    }

    /// Get player display name
    /// C++ Reference: Player::getPlayerDisplayName() (Player.h line 118)
    pub fn get_player_display_name(&self) -> &str {
        &self.player_display_name
    }

    /// Get player internal name
    pub fn get_player_name(&self) -> &str {
        &self.player_name
    }

    /// Get player side
    /// C++ Reference: Player::getSide() (Player.h line 121)
    pub fn get_side(&self) -> &str {
        &self.side
    }

    /// Get player base side
    /// C++ Reference: Player::getBaseSide() (Player.h line 122)
    pub fn get_base_side(&self) -> &str {
        &self.base_side
    }

    /// Get player type
    /// C++ Reference: Player::getPlayerType() (Player.h line 138)
    pub fn get_player_type(&self) -> PlayerType {
        self.player_type
    }

    /// Set player type
    /// C++ Reference: Player::setPlayerType() (Player.cpp lines 695-712)
    pub fn set_player_type(&mut self, player_type: PlayerType, _skirmish: bool) {
        self.player_type = player_type;
        // Note: AI player creation would happen here in C++ (lines 706-712)
    }

    /// Get the money object
    /// C++ Reference: Player::getMoney() (Player.h lines 127-128)
    pub fn get_money(&self) -> &Money {
        &self.money
    }

    /// Get mutable reference to money
    pub fn get_money_mut(&mut self) -> &mut Money {
        &mut self.money
    }

    /// Get the energy object
    /// C++ Reference: Player::getEnergy() (Player.h lines 135-136)
    pub fn get_energy(&self) -> &Energy {
        &self.energy
    }

    /// Get mutable reference to energy
    pub fn get_energy_mut(&mut self) -> &mut Energy {
        &mut self.energy
    }

    /// Get academy stats
    /// C++ Reference: Player::getAcademyStats() (Player.h lines 417-418)
    pub fn get_academy_stats(&self) -> &AcademyStats {
        &self.academy_stats
    }

    /// Get mutable reference to academy stats
    pub fn get_academy_stats_mut(&mut self) -> &mut AcademyStats {
        &mut self.academy_stats
    }

    /// Get mission stats
    pub fn get_mission_stats(&self) -> &MissionStats {
        &self.mission_stats
    }

    /// Get mutable reference to mission stats
    pub fn get_mission_stats_mut(&mut self) -> &mut MissionStats {
        &mut self.mission_stats
    }

    /// Get handicap
    /// C++ Reference: Player::getHandicap() (Player.h lines 125-126)
    pub fn get_handicap(&self) -> &Handicap {
        &self.handicap
    }

    /// Get score keeper
    /// C++ Reference: Player::getScoreKeeper() (Player.h line 415)
    pub fn get_score_keeper(&self) -> &ScoreKeeper {
        &self.score_keeper
    }

    /// Get mutable reference to score keeper
    pub fn get_score_keeper_mut(&mut self) -> &mut ScoreKeeper {
        &mut self.score_keeper
    }

    /// Get multiplayer start index
    /// C++ Reference: Player::getMpStartIndex() (Player.h line 311)
    pub fn get_mp_start_index(&self) -> i32 {
        self.mp_start_index
    }

    /// Set multiplayer start index
    pub fn set_mp_start_index(&mut self, index: i32) {
        self.mp_start_index = index;
    }

    // =========================================================
    // Initialization Methods (C++ Player.cpp lines 252-437)
    // =========================================================

    /// Initialize player from a player template
    ///
    /// C++ Reference: Player::init() (Player.cpp lines 252-437)
    ///
    /// # Arguments
    /// * `name` - Optional player name to set
    pub fn init(&mut self, name: Option<String>) {
        let template = self.current_player_template();

        // C++ lines 257-259: Reset skill point modifier
        self.skill_points_modifier = 1.0;
        self.attacked_frame = 0;

        // C++ lines 261-263: Reset state flags
        self.is_preorder = false;
        self.is_player_dead = false;

        // C++ lines 265-269: Reset radar
        self.radar_count = 0;
        self.disable_proof_radar_count = 0;
        self.radar_disabled = false;

        // C++ lines 271-280: Reset battle plans
        self.bombard_battle_plans = 0;
        self.hold_the_line_battle_plans = 0;
        self.search_and_destroy_battle_plans = 0;
        self.battle_plan_bonuses = None;
        self.team_relations.clear();

        // C++ lines 285: Initialize energy
        let handle = PlayerHandle::new(self.index.max(0) as u32);
        self.energy.init(handle);

        // C++ line 286: Initialize stats
        self.mission_stats.init();

        // C++ lines 288-291: Initialize handicap
        self.handicap.init();

        // C++ lines 293-310: Initialize squads (simplified - we don't have Squad class yet)

        // C++ lines 318-319: Reset build permissions
        self.can_build_base = true;
        self.can_build_units = true;

        // C++ lines 320-321: Reset observer and bounty
        self.observer = false;
        self.cash_bounty_percent = 0.0;
        self.list_in_score_screen = true;
        self.units_should_hunt = false;

        // C++ lines 333-340: Initialize default values (no player template = neutral player)
        if let Some(name) = name {
            self.player_display_name = name;
        }
        self.player_name.clear();
        self.side.clear();
        self.base_side.clear();
        self.player_type = PlayerType::Computer;

        // C++ line 354: Reset score keeper
        self.score_keeper.reset(self.index);

        if let Some(template) = &template {
            self.observer = template.is_observer;
            self.is_player_dead = self.observer;
        }

        // C++ lines 357-358: Reset rank and sciences
        self.reset_rank();
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();

        // C++ lines 369-371: Initialize academy stats
        self.academy_stats.init_for_base_side(
            handle,
            template
                .as_ref()
                .map(|template| template.base_side.as_str()),
        );

        // C++ line 376: Reset retaliation mode
        self.logical_retaliation_mode_enabled = false;

        // Initialize money
        self.money.init();
        self.money.set_player_index(self.index);
    }

    /// Reset rank to 1
    /// C++ Reference: Player::resetRank() (Player.cpp lines 439-449)
    pub fn reset_rank(&mut self) {
        self.rank_level = 1;
        self.skill_points = 0;
        self.science_purchase_points = self
            .current_player_template()
            .map(|template| template.intrinsic_science_purchase_points.max(0))
            .unwrap_or(0);
        self.sciences.clear();

        let rank_store = get_rank_info_store();
        if !rank_store.is_empty() {
            self.level_up = rank_store
                .get_rank_info(self.rank_level + 1)
                .map(|rank| rank.skill_points_needed)
                .unwrap_or(i32::MAX);
            self.level_down = 0;

            if let Some(rank) = rank_store.get_rank_info(self.rank_level) {
                self.science_purchase_points += rank.science_purchase_points_granted as i32;
                for &science in &rank.sciences_granted {
                    self.grant_science(science);
                }
            }

            self.reset_sciences();
            return;
        }

        self.level_up = 100;
        self.level_down = 0;
        self.reset_sciences();
    }

    /// Reset sciences to just intrinsic ones from player template
    /// C++ Reference: Player::resetSciences() (Player.cpp lines 451-466)
    pub fn reset_sciences(&mut self) {
        self.sciences.clear();

        if let Some(template) = self.current_player_template() {
            if let Some(science_store) = get_science_store() {
                for science_name in &template.intrinsic_sciences {
                    let science = science_store.get_science_from_internal_name(science_name);
                    self.grant_science(science);
                }
            }
        }

        let rank_store = get_rank_info_store();
        for rank_level in 1..=self.rank_level {
            if let Some(rank) = rank_store.get_rank_info(rank_level) {
                for &science in &rank.sciences_granted {
                    self.grant_science(science);
                }
            }
        }
    }

    // =========================================================
    // Update Method (C++ Player.cpp lines 540-590)
    // =========================================================

    /// Update player (called each frame)
    ///
    /// C++ Reference: Player::update() (Player.cpp lines 540-590)
    ///
    /// This method handles:
    /// - AI updates (if computer player)
    /// - Team script updates
    /// - Power sabotage checks
    /// - Academy stats updates
    /// - Retaliation mode sync
    pub fn update(&mut self) {
        // C++ lines 545-546: AI update would happen here

        // C++ lines 548-562: Team script updates would happen here

        // C++ lines 564-569: Check power sabotage expiry
        let current_frame = crate::common::time::frame();
        if self.energy.get_power_sabotaged_till_frame() != 0
            && current_frame > self.energy.get_power_sabotaged_till_frame()
        {
            self.energy.set_power_sabotaged_till_frame(0);
            self.on_power_brown_out_change(!self.energy.has_sufficient_power());
        }

        // C++ line 571: Update academy stats
        if crate::common::rts::money::take_pending_income(self.index) {
            self.academy_stats.record_income();
        }
        self.academy_stats
            .update_from_player(self.money.count_money(), self.energy.has_sufficient_power());

        // C++ lines 573-590: Retaliation mode sync would happen here
        // (requires access to ThePlayerList and TheGlobalData)
    }
}
