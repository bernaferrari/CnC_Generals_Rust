use super::*;

/// Complete Player class (matching C++ Player)
#[derive(Debug)]
pub struct Player {
    // Core identity
    pub(super) player_index: PlayerIndex,
    pub(super) player_name_key: NameKeyType,
    pub(super) player_display_name: String,
    pub(super) player_template: Option<Arc<PlayerTemplate>>,

    // Gameplay properties
    pub(super) player_type: PlayerType,
    pub(super) side: String,
    pub(super) base_side: String,
    pub(super) color: Color,
    pub(super) night_color: Color,
    pub(super) difficulty: GameDifficulty,

    // Resources and economy
    pub(super) money: PlayerMoney,
    pub(super) energy: PlayerEnergy,
    pub(super) handicap: PlayerHandicap,

    // Research and upgrades
    pub(super) sciences: ScienceVec,
    pub(super) sciences_disabled: ScienceVec,
    pub(super) sciences_hidden: ScienceVec,
    pub(super) upgrade_list: Vec<Upgrade>,
    pub(super) upgrades_in_progress: UpgradeMaskType,
    pub(super) upgrades_completed: UpgradeMaskType,

    // Experience and ranking
    pub(super) rank_level: Int,
    pub(super) skill_points: Int,
    pub(super) science_purchase_points: Int,
    pub(super) skill_points_modifier: Real,
    pub(super) general_name: String,

    // Team and relationships
    pub(super) default_team: Option<Arc<RwLock<Team>>>,
    pub(super) player_team_prototypes: Vec<Arc<TeamPrototype>>,
    pub(super) player_relations: PlayerRelationMap,
    pub(super) team_relations: Option<TeamRelationMap>,

    // Production cost modifiers
    pub(super) kind_of_percent_production_change_list: Vec<KindOfPercentProductionChange>,

    // Radar and intelligence
    pub(super) radar_count: Int,
    pub(super) disable_proof_radar_count: Int,
    pub(super) radar_disabled: Bool,

    // Battle plans and bonuses
    pub(super) bombard_battle_plans: Int,
    pub(super) hold_the_line_battle_plans: Int,
    pub(super) search_and_destroy_battle_plans: Int,
    pub(super) battle_plan_bonuses: Option<BattlePlanBonuses>,

    // Special powers
    pub(super) special_power_ready_timers: RwLock<Vec<SpecialPowerReadyTimer>>,

    // Statistics and tracking
    pub(super) academy_stats: AcademyStats,
    pub(super) score_keeper: ScoreKeeper,

    // Control and AI
    pub(super) can_build_units: Bool,
    pub(super) can_build_base: Bool,
    pub(super) is_observer: Bool,
    pub(super) is_preorder: Bool,
    pub(super) is_player_dead: Bool,
    pub(super) list_in_score_screen: Bool,
    pub(super) units_should_hunt: Bool,
    pub(super) attacked_by: [Bool; MAX_PLAYER_COUNT],
    pub(super) attacked_frame: UnsignedInt,

    // Multiplayer
    pub(super) mp_start_index: Int,

    // Special properties
    pub(super) cash_bounty_percent: Real,

    // Hotkey squads
    pub(super) squads: [Option<Squad>; NUM_HOTKEY_SQUADS],
    pub(super) current_selection: Option<Squad>,

    // Cheats and debug
    #[cfg(any(debug_assertions, feature = "internal"))]
    pub(super) demo_ignore_prereqs: Bool,
    #[cfg(any(debug_assertions, feature = "internal"))]
    pub(super) demo_free_build: Bool,
    #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
    pub(super) demo_instant_build: Bool,

    // Retaliation mode
    pub(super) logical_retaliation_mode_enabled: Bool,

    // Tunnel network system (for GLA faction)
    pub(super) tunnel_tracker: Option<TunnelTracker>,
    pub(super) resource_manager: Option<ResourceGatheringManager>,

    // Player upgrade manager
    pub(super) upgrade_manager: PlayerUpgradeManager,

    // Objects owned by this player
    pub(super) owned_objects: Vec<ObjectID>,

    // AI build list (skirmish plans)
    pub(super) build_list: Option<Box<BuildListInfo>>,

    // Skirmish AI tracking
    pub(super) is_skirmish_ai: Bool,
    pub(super) current_enemy_player_index: Option<PlayerIndex>,
}

impl Player {
    /// Create a new player with the given index
    pub fn new(player_index: PlayerIndex) -> Self {
        Self {
            player_index,
            player_name_key: 0,
            player_display_name: String::new(),
            player_template: None,

            player_type: PlayerType::Human,
            side: String::new(),
            base_side: String::new(),
            color: Color::default(),
            night_color: Color::default(),
            difficulty: GameDifficulty::Normal,

            money: PlayerMoney::new(player_index),
            energy: PlayerEnergy::new(),
            handicap: PlayerHandicap::new(),

            sciences: Vec::new(),
            sciences_disabled: Vec::new(),
            sciences_hidden: Vec::new(),
            upgrade_list: Vec::new(),
            upgrades_in_progress: UpgradeMaskType::none(),
            upgrades_completed: UpgradeMaskType::none(),

            rank_level: 1,
            skill_points: 0,
            science_purchase_points: 0,
            skill_points_modifier: 1.0,
            general_name: String::new(),

            default_team: None,
            player_team_prototypes: Vec::new(),
            player_relations: PlayerRelationMap::new(),
            team_relations: None,

            kind_of_percent_production_change_list: Vec::new(),

            radar_count: 0,
            disable_proof_radar_count: 0,
            radar_disabled: false,

            bombard_battle_plans: 0,
            hold_the_line_battle_plans: 0,
            search_and_destroy_battle_plans: 0,
            battle_plan_bonuses: None,

            special_power_ready_timers: RwLock::new(Vec::new()),

            academy_stats: AcademyStats::new(),
            score_keeper: ScoreKeeper::new_for_player(player_index),

            can_build_units: true,
            can_build_base: true,
            is_observer: false,
            is_preorder: false,
            is_player_dead: false,
            list_in_score_screen: true,
            units_should_hunt: false,
            attacked_by: [false; MAX_PLAYER_COUNT],
            attacked_frame: 0,

            mp_start_index: 0,

            cash_bounty_percent: 0.0,

            squads: Default::default(),
            current_selection: None,

            #[cfg(any(debug_assertions, feature = "internal"))]
            demo_ignore_prereqs: false,
            #[cfg(any(debug_assertions, feature = "internal"))]
            demo_free_build: false,
            #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
            demo_instant_build: false,

            logical_retaliation_mode_enabled: false,

            tunnel_tracker: None,
            resource_manager: None,

            upgrade_manager: PlayerUpgradeManager::new(player_index as u32),

            owned_objects: Vec::new(),
            build_list: None,

            is_skirmish_ai: false,
            current_enemy_player_index: None,
        }
    }

    /// Get the player ID (player index)
    pub fn get_id(&self) -> PlayerIndex {
        self.player_index
    }

    pub fn get_build_list(&self) -> Option<&BuildListInfo> {
        self.build_list.as_deref()
    }

    pub fn get_build_list_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.build_list.as_deref_mut()
    }

    pub fn set_build_list(&mut self, build_list: Option<BuildListInfo>) {
        self.build_list = build_list.map(Box::new);
    }

    /// C++ `Player::addToBuildList` — prepend a live factory/structure to the build list.
    pub fn add_to_build_list(
        &mut self,
        object_id: crate::common::ObjectID,
        template_name: AsciiString,
        location: Coord3D,
        angle: Real,
    ) {
        let mut info = BuildListInfo::new();
        info.set_template_name(template_name);
        info.set_location(location);
        info.set_angle(angle);
        info.set_object_id(object_id);
        info.set_num_rebuilds(0); // can't rebuild placed factories
        info.set_next_build_list(self.build_list.take().map(|b| *b));
        self.build_list = Some(Box::new(info));
    }

    /// C++ `Player::addToPriorityBuildList` (Player.cpp).
    pub fn add_to_priority_build_list(
        &mut self,
        template_name: AsciiString,
        location: Coord3D,
        angle: Real,
    ) {
        let mut info = BuildListInfo::new();
        info.set_template_name(template_name);
        info.set_location(location);
        info.set_angle(angle);
        info.mark_priority_build();
        info.set_num_rebuilds(1); // build once
        info.set_next_build_list(self.build_list.take().map(|b| *b));
        self.build_list = Some(Box::new(info));
    }

    pub fn is_skirmish_ai(&self) -> Bool {
        self.is_skirmish_ai
    }

    pub fn set_is_skirmish_ai(&mut self, value: Bool) {
        self.is_skirmish_ai = value;
    }

    pub fn get_current_enemy_player_index(&self) -> Option<PlayerIndex> {
        self.current_enemy_player_index
    }

    pub fn set_current_enemy_player_index(&mut self, index: Option<PlayerIndex>) {
        self.current_enemy_player_index = index;
    }

    /// Get all objects owned by this player
    /// Matches C++ Player::getObjectList
    pub fn get_all_objects(&self) -> Vec<crate::common::ObjectID> {
        self.owned_objects.clone()
    }

    /// Count objects by thing template, matching C++ Player::countObjectsByThingTemplate.
    pub fn count_objects_by_thing_template(
        &self,
        templates: &[Arc<dyn ThingTemplate>],
        ignore_dead: Bool,
        ignore_under_construction: Bool,
        counts: &mut [Int],
    ) {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            counts.fill(0);
            return;
        }
        counts.fill(0);
        let max_templates = templates.len().min(counts.len());
        if max_templates == 0 {
            return;
        }

        for &object_id in &self.owned_objects {
            let _ =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                    if ignore_dead && object_guard.is_effectively_dead() {
                        return;
                    }
                    if ignore_under_construction
                        && object_guard.test_status(ObjectStatusTypes::UnderConstruction)
                    {
                        return;
                    }

                    let obj_template = object_guard.get_template();
                    for i in 0..max_templates {
                        if !obj_template.is_equivalent_to(templates[i].as_ref()) {
                            continue;
                        }
                        counts[i] += 1;
                        break;
                    }
                });
        }
    }

    /// Count player-owned structures, matching C++ Player::countBuildings.
    pub fn count_buildings(&self) -> Int {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let mut count = 0;
        for &object_id in &self.owned_objects {
            let _ =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                    if object_guard.get_template().is_kind_of(KindOf::Structure) {
                        count += 1;
                    }
                });
        }
        count
    }

    /// Count player-owned objects by KindOf masks, matching C++ Player::countObjects.
    pub fn count_objects_by_kindof(
        &self,
        required: KindOfMaskType,
        forbidden: KindOfMaskType,
    ) -> Int {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let mut count = 0;
        for &object_id in &self.owned_objects {
            let _ =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                    if object_guard.is_kind_of_multi(required, forbidden) {
                        count += 1;
                    }
                });
        }
        count
    }

    /// Add an object to this player's ownership
    /// Matches C++ Player::addObject
    pub fn add_owned_object(&mut self, object_id: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if !self.owned_objects.contains(&object_id) {
            self.owned_objects.push(object_id);
        }

        let Some((under_construction, power, disabled, is_dozer, idle_dozer)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                let under_construction =
                    object_guard.test_status(ObjectStatusTypes::UnderConstruction);
                let power = object_guard.get_template().get_energy_production();
                let disabled = object_guard.is_disabled();
                let is_dozer = object_guard.is_kind_of(crate::common::KindOf::Dozer);
                let idle_dozer = if is_dozer {
                    object_guard
                        .get_ai_update_interface()
                        .and_then(|ai| ai.lock().ok().map(|g| g.is_idle()))
                        .unwrap_or(false)
                } else {
                    false
                };
                (under_construction, power, disabled, is_dozer, idle_dozer)
            })
        else {
            return;
        };

        if !under_construction {
            if power > 0 {
                if !disabled {
                    self.add_power_production(power);
                }
            } else if power < 0 {
                self.add_power_consumption(-power);
            }
        }

        // Idle-worker UI still needs a short Arc borrow (callback takes &Object).
        if is_dozer && idle_dozer {
            let _ =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                    crate::helpers::TheInGameUI::add_idle_worker(object_guard, self.player_index);
                });
        }
    }

    /// Find a drone owned by this player that was produced by the given object ID.
    pub fn find_drone_id_by_producer_id(&self, producer_id: ObjectID) -> Option<ObjectID> {
        // Wave 268: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        for object_id in &self.owned_objects {
            let matches = crate::object::registry::OBJECT_REGISTRY
                .with_object(*object_id, |obj_ref| {
                    obj_ref.get_producer_id() == producer_id
                        && obj_ref.is_kind_of(crate::common::KindOf::Drone)
                })
                .unwrap_or(false);
            if matches {
                return Some(*object_id);
            }
        }
        None
    }

    /// Prefer [`Self::find_drone_id_by_producer_id`].
    pub fn find_drone_by_producer_id(
        &self,
        producer_id: ObjectID,
    ) -> Result<Option<Arc<RwLock<Object>>>, String> {
        // Wave 268: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        Ok(self
            .find_drone_id_by_producer_id(producer_id)
            .and_then(|id| crate::object::registry::OBJECT_REGISTRY.get_object(id)))
    }

    /// Remove an object from this player's ownership
    /// Matches C++ Player::removeObject
    pub fn remove_owned_object(&mut self, object_id: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        self.owned_objects.retain(|&id| id != object_id);

        let Some((under_construction, power, disabled, is_dozer, idle_dozer)) =
            crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                let under_construction =
                    object_guard.test_status(ObjectStatusTypes::UnderConstruction);
                let power = object_guard.get_template().get_energy_production();
                let disabled = object_guard.is_disabled();
                let is_dozer = object_guard.is_kind_of(crate::common::KindOf::Dozer);
                let idle_dozer = if is_dozer {
                    object_guard
                        .get_ai_update_interface()
                        .and_then(|ai| ai.lock().ok().map(|g| g.is_idle()))
                        .unwrap_or(false)
                } else {
                    false
                };
                (under_construction, power, disabled, is_dozer, idle_dozer)
            })
        else {
            return;
        };

        if !under_construction {
            if power > 0 {
                if !disabled {
                    self.add_power_production(-power);
                }
            } else if power < 0 {
                self.add_power_consumption(power);
            }
        }

        if is_dozer && idle_dozer {
            let _ =
                crate::object::registry::OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                    crate::helpers::TheInGameUI::remove_idle_worker(
                        object_guard,
                        self.player_index,
                    );
                });
        }
    }

    /// Get the number of objects owned by this player
    pub fn get_owned_object_count(&self) -> usize {
        self.owned_objects.len()
    }

    /// Get the upgrade manager for this player
    /// Matches C++ Player::getUpgradeManager
    pub fn get_upgrade_manager(&self) -> Option<&PlayerUpgradeManager> {
        Some(&self.upgrade_manager)
    }

    /// Get mutable upgrade manager for this player
    pub fn get_upgrade_manager_mut(&mut self) -> Option<&mut PlayerUpgradeManager> {
        Some(&mut self.upgrade_manager)
    }

    /// Update player state each frame
    pub fn update(&mut self) {
        let sabotage_frame = self.energy.get_power_sabotaged_till_frame();
        if sabotage_frame != 0 && TheGameLogic::get_frame() > sabotage_frame {
            self.energy.set_power_sabotaged_till_frame(0);
            let _ = self.on_power_brown_out_change(!self.energy.has_sufficient_power());
        }
    }

    /// Called when a new map is loaded
    pub fn new_map(&mut self) {
        // Reset transient state for new map
        self.radar_count = 0;
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            timers.clear();
        }
        self.attacked_by = [false; MAX_PLAYER_COUNT];
        self.attacked_frame = 0;
    }

    pub(super) fn add_new_shared_special_power_timer(
        &mut self,
        template: &SpecialPowerTemplate,
        frame: UnsignedInt,
    ) {
        let mut timer = SpecialPowerReadyTimer::new();
        timer.template_id = template.get_id();
        timer.ready_frame = frame;
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            timers.push(timer);
        }
    }

    pub fn reset_or_start_special_power_ready_frame(&mut self, template: &SpecialPowerTemplate) {
        let now = TheGameLogic::get_frame();
        let lookup_id = template.get_id();
        let mut needs_insert = true;

        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == lookup_id {
                    timer.ready_frame = now + template.get_reload_time();
                    needs_insert = false;
                    break;
                }
            }
        }

        if needs_insert {
            self.add_new_shared_special_power_timer(template, now);
        }
    }

    pub fn express_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        frame: UnsignedInt,
    ) {
        let lookup_id = template.get_id();
        let mut needs_insert = true;
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == lookup_id {
                    timer.ready_frame = frame;
                    needs_insert = false;
                    break;
                }
            }
        }

        if needs_insert {
            self.add_new_shared_special_power_timer(template, frame);
        }
    }

    pub fn get_or_start_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
    ) -> UnsignedInt {
        let now = TheGameLogic::get_frame();
        let lookup_id = template.get_id();
        let mut ready_frame = None;

        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == lookup_id {
                    ready_frame = Some(timer.ready_frame);
                    break;
                }
            }
        }

        if let Some(frame) = ready_frame {
            frame
        } else {
            self.add_new_shared_special_power_timer(template, now);
            now
        }
    }

    pub fn set_display_name<S: Into<String>>(&mut self, name: S) {
        let name = name.into();
        self.player_display_name = name.clone();
        if self.player_name_key == 0 && !name.is_empty() {
            self.player_name_key = NameKeyGenerator::name_to_key(&name);
        }
    }

    pub fn set_player_name_key(&mut self, key: NameKeyType) {
        self.player_name_key = key;
    }

    pub fn set_side<S: Into<String>>(&mut self, side: S) {
        self.side = side.into();
    }

    pub fn set_base_side<S: Into<String>>(&mut self, base_side: S) {
        self.base_side = base_side.into();
    }

    pub fn set_colors(&mut self, primary: Color, night: Color) {
        self.color = primary;
        self.night_color = night;
    }

    pub fn set_observer(&mut self, observer: Bool) {
        self.is_observer = observer;
    }

    /// Initialize from player template
    pub fn init(&mut self, player_template: Arc<PlayerTemplate>) {
        self.energy.reset();
        let mut template = (*player_template).clone();
        if template.production_cost_changes.is_empty()
            && template.production_time_changes.is_empty()
            && template.production_veterancy_levels.is_empty()
        {
            template.hydrate_from_common_store();
        }
        self.player_template = Some(Arc::new(template.clone()));
        self.side = template.side.clone();
        self.base_side = template.base_side.clone();
        self.player_display_name = template.display_name.clone();
        if self.player_name_key == 0 {
            let key_source = if !template.name.is_empty() {
                template.name.as_str()
            } else {
                self.player_display_name.as_str()
            };
            if !key_source.is_empty() {
                self.player_name_key = NameKeyGenerator::name_to_key(key_source);
            }
        }
        self.is_observer = template.is_observer;

        // Apply starting money from the player template.
        // In C++ this is set during Player::init() via the PlayerTemplate's
        // StartingMoney field.  When the template has not been populated from
        // INI yet (Money::count_money() == 0) we fall back to the standard
        // skirmish default of $10,000 so that players always start with money.
        let starting = template.starting_money.count_money();
        let amount = if starting > 0 {
            starting as i32
        } else {
            10_000
        };
        self.money.set_money(amount);

        self.reset_rank_impl();
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();
    }

    pub fn init_from_dict_defaults(&mut self) {
        for slot in &mut self.squads {
            *slot = Some(Squad::new());
        }
        self.current_selection = Some(Squad::new());
        self.tunnel_tracker = Some(TunnelTracker::new());
        self.resource_manager = Some(ResourceGatheringManager::new());
        self.player_relations.map.clear();
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut team_relations) = self.team_relations {
            team_relations.map.clear();
        }
        self.attacked_by = [false; MAX_PLAYER_COUNT];
        self.attacked_frame = 0;
    }

    /// Set default team
    pub fn set_default_team(&mut self, team: Option<Arc<RwLock<Team>>>) {
        self.default_team = team;
    }

    pub fn get_default_team(&self) -> Option<Arc<RwLock<Team>>> {
        self.default_team.as_ref().map(Arc::clone)
    }

    /// C++ Player::getPlayerTeams() — team prototypes owned by this player.
    pub fn get_player_team_prototypes(&self) -> &[Arc<TeamPrototype>] {
        &self.player_team_prototypes
    }

    /// Get the default team ID for this player
    pub fn get_default_team_id(&self) -> Option<TeamID> {
        self.default_team
            .as_ref()
            .and_then(|team| team.read().ok().map(|t| t.get_id()))
    }

    /// Heal all objects owned by this player.
    /// Matches C++ Player::healAllObjects.
    pub fn heal_all_objects(&mut self) {
        if let Ok(factory) = get_team_factory().lock() {
            for prototype in &self.player_team_prototypes {
                for team in factory.find_team_instances(prototype.get_name().as_str()) {
                    if let Ok(mut team_guard) = team.write() {
                        team_guard.heal_all_objects();
                    }
                }
            }
        }

        if self.player_team_prototypes.is_empty() {
            if let Some(team) = &self.default_team {
                if let Ok(mut team_guard) = team.write() {
                    team_guard.heal_all_objects();
                }
            }
        }
    }

    // Getters for core properties
    pub fn get_player_display_name(&self) -> &String {
        &self.player_display_name
    }

    pub fn get_player_name_key(&self) -> NameKeyType {
        self.player_name_key
    }

    pub fn get_mp_start_index(&self) -> Int {
        self.mp_start_index
    }

    pub fn set_mp_start_index(&mut self, index: Int) {
        self.mp_start_index = index;
    }

    pub fn set_is_preorder(&mut self, value: Bool) {
        self.is_preorder = value;
    }

    pub fn get_side(&self) -> &String {
        &self.side
    }

    pub fn get_base_side(&self) -> &String {
        &self.base_side
    }

    pub fn get_player_template(&self) -> Option<&Arc<PlayerTemplate>> {
        self.player_template.as_ref()
    }

    pub fn get_object_ids(&self) -> Vec<ObjectID> {
        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Vec::new();
        };
        manager.get_objects_owned_by_player(self.player_index as UnsignedInt)
    }
}
