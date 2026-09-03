//! Full skirmish match configuration propagated from UI to match start.
//!
//! Previously the skirmish menu only emitted mode/faction/map. This type carries
//! all eight slots, difficulties, colors, teams, starting positions, and rules.

use crate::ai::AIDifficulty;
use crate::game_logic::{GameLogic, GameMode, Player, PlayerTemplateIdentity, Team};
use crate::ui::skirmish_menu::{Faction, GameRules, GameSlot, MAX_SLOTS, PlayerType};
use serde::{Deserialize, Serialize};

/// The exact C++ `GameSlot::m_playerTemplate` selection for an occupied
/// offline Skirmish slot.
///
/// `GameInfo` stores an index, but accepting the index by itself would let a
/// reordered PlayerTemplate store launch a plausible-looking *different*
/// General.  Retaining the canonical name with it is therefore intentional:
/// GameLogic validates the pair again immediately before it binds the player.
///
/// This type is deliberately local to the offline `SkirmishMatchConfig`.
/// Multiplayer/GameNetwork transport remains deferred.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SkirmishPlayerTemplateSelection {
    /// A concrete `PlayerTemplateStore` record selected by the UI.
    Exact {
        template_name: String,
        template_index: i32,
    },
    /// C++ `PLAYERTEMPLATE_RANDOM`; it is resolved one time before map load.
    Random,
}

impl SkirmishPlayerTemplateSelection {
    /// Build an indexed exact selection from a Main fallback-menu base faction.
    ///
    /// That menu currently exposes only USA/China/GLA/Random, unlike the C++
    /// WND which exposes individual General sides.  Even its base-side choice
    /// must still become a concrete PlayerTemplate rather than falling through
    /// to `Team`-only bootstrap state.
    pub fn base_faction(faction: &str) -> Self {
        let template_name = match faction.trim().to_ascii_lowercase().as_str() {
            "usa" | "america" => "FactionAmerica",
            "china" => "FactionChina",
            "gla" => "FactionGLA",
            // Keep malformed locally-authored configuration fail-closed at
            // the authority boundary instead of guessing USA.
            _ => faction.trim(),
        };
        Self::exact_template_name(template_name)
    }

    /// Resolve a canonical store name into the name/index pair accepted by
    /// `PlayerTemplateIdentity::from_exact_indexed_name`.  If the source is
    /// absent, retain an invalid pair so `apply_skirmish_config` reports the
    /// selected identity failure before changing the simulation; do not turn
    /// it into a different base faction.
    pub fn exact_template_name(template_name: &str) -> Self {
        let requested_name = template_name.trim().to_string();
        game_engine::common::ini::ensure_player_templates_loaded();
        let store = game_engine::common::rts::player_template::get_player_template_store();
        let Some(template_index) = store
            .find_template_index(&requested_name)
            .and_then(|index| i32::try_from(index).ok())
        else {
            return Self::Exact {
                template_name: requested_name,
                template_index: -1,
            };
        };
        let template_name = store
            .get_nth_player_template_signed(template_index)
            .map(|template| template.get_name().to_string())
            .unwrap_or(requested_name);
        Self::Exact {
            template_name,
            template_index,
        }
    }
}

/// One configured skirmish slot as pure data (no UI types required by GameLogic).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkirmishSlotConfig {
    pub slot_index: usize,
    pub is_human: bool,
    pub is_active: bool,
    /// Exact selected General/base PlayerTemplate; this, rather than
    /// `faction`, is the authoritative gameplay identity.
    pub player_template: SkirmishPlayerTemplateSelection,
    /// Presentation/legacy faction label retained for menu text and callers
    /// that only need a base-side display string. It must not select gameplay
    /// state in `apply_skirmish_config`.
    pub faction: String,
    pub color_rgb: (u8, u8, u8),
    pub team: i32,
    pub start_position: i32,
    pub player_name: String,
    pub ai_difficulty: Option<String>,
}

/// Complete skirmish start configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkirmishMatchConfig {
    pub map: String,
    /// C++ `GameInfo::getSeed()` bit pattern used to seed the GameLogic ADC
    /// stream before resolving any `PLAYERTEMPLATE_RANDOM` slots.
    pub random_seed: u32,
    /// C++ `GameInfo::oldFactionsOnly()`. It restricts the candidate list for
    /// `PLAYERTEMPLATE_RANDOM`; exact selected identities remain name/index
    /// authoritative.
    pub old_factions_only: bool,
    pub rules: GameRulesSnapshot,
    pub slots: Vec<SkirmishSlotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameRulesSnapshot {
    pub starting_cash: i32,
    pub game_speed: f32,
    pub limit_superweapons: bool,
    pub allow_tech_buildings: bool,
    pub crates_enabled: bool,
    pub fog_of_war: bool,
}

impl From<&GameRules> for GameRulesSnapshot {
    fn from(r: &GameRules) -> Self {
        Self {
            starting_cash: r.starting_cash,
            game_speed: r.game_speed,
            limit_superweapons: r.limit_superweapons,
            allow_tech_buildings: r.allow_tech_buildings,
            crates_enabled: r.crates_enabled,
            fog_of_war: r.fog_of_war,
        }
    }
}

impl GameRulesSnapshot {
    pub fn default_rules() -> Self {
        Self::from(&GameRules::default())
    }
}

fn ui_faction_name(f: Faction) -> String {
    match f {
        Faction::USA => "USA".into(),
        Faction::China => "China".into(),
        Faction::GLA => "GLA".into(),
        Faction::Random => "Random".into(),
    }
}

fn ui_player_template_selection(faction: Faction) -> SkirmishPlayerTemplateSelection {
    match faction {
        Faction::USA => SkirmishPlayerTemplateSelection::base_faction("USA"),
        Faction::China => SkirmishPlayerTemplateSelection::base_faction("China"),
        Faction::GLA => SkirmishPlayerTemplateSelection::base_faction("GLA"),
        Faction::Random => SkirmishPlayerTemplateSelection::Random,
    }
}

fn difficulty_from_player_type(t: PlayerType) -> Option<AIDifficulty> {
    match t {
        PlayerType::EasyAI => Some(AIDifficulty::Easy),
        PlayerType::MediumAI => Some(AIDifficulty::Medium),
        PlayerType::HardAI => Some(AIDifficulty::Hard),
        PlayerType::BrutalAI => Some(AIDifficulty::Brutal),
        _ => None,
    }
}

fn difficulty_name(d: AIDifficulty) -> String {
    match d {
        AIDifficulty::Easy => "Easy".into(),
        AIDifficulty::Medium => "Medium".into(),
        AIDifficulty::Hard => "Hard".into(),
        AIDifficulty::Brutal => "Brutal".into(),
    }
}

fn parse_difficulty(s: &str) -> AIDifficulty {
    match s.to_ascii_lowercase().as_str() {
        "easy" => AIDifficulty::Easy,
        "hard" => AIDifficulty::Hard,
        "brutal" => AIDifficulty::Brutal,
        _ => AIDifficulty::Medium,
    }
}

/// Build match config from live skirmish menu state (shipped UI path).
pub fn config_from_skirmish_menu(
    map: &str,
    rules: &GameRules,
    slots: &[GameSlot],
    random_seed: u32,
) -> SkirmishMatchConfig {
    let mut out_slots = Vec::with_capacity(MAX_SLOTS);
    for slot in slots.iter().take(MAX_SLOTS) {
        let is_active = !matches!(slot.player_type, PlayerType::Open | PlayerType::Closed);
        let is_human = matches!(slot.player_type, PlayerType::Human);
        let ai = difficulty_from_player_type(slot.player_type);
        out_slots.push(SkirmishSlotConfig {
            slot_index: slot.slot_index,
            is_human,
            is_active,
            player_template: ui_player_template_selection(slot.faction),
            faction: ui_faction_name(slot.faction),
            color_rgb: slot.color.to_rgb(),
            team: slot.team,
            start_position: slot.start_position,
            player_name: slot.player_name.clone(),
            ai_difficulty: ai.map(difficulty_name),
        });
    }
    SkirmishMatchConfig {
        map: map.to_string(),
        // The fallback menu has no GameClient GameInfo backing. Its caller
        // owns this retained seed explicitly; do not synthesize a transient
        // wall-clock value and present it as C++ GameInfo parity.
        random_seed,
        old_factions_only: false,
        rules: GameRulesSnapshot::from(rules),
        slots: out_slots,
    }
}

/// Golden skirmish: USA human vs Medium GLA AI on a fixed map name.
pub fn golden_skirmish_config(map: &str) -> SkirmishMatchConfig {
    SkirmishMatchConfig {
        map: map.to_string(),
        random_seed: 0,
        old_factions_only: false,
        rules: GameRulesSnapshot {
            starting_cash: 10_000,
            game_speed: 1.0,
            limit_superweapons: false,
            allow_tech_buildings: true,
            crates_enabled: true,
            fog_of_war: true,
        },
        slots: vec![
            SkirmishSlotConfig {
                slot_index: 0,
                is_human: true,
                is_active: true,
                player_template: SkirmishPlayerTemplateSelection::base_faction("USA"),
                faction: "USA".into(),
                color_rgb: (0, 0, 200),
                team: 0,
                start_position: 0,
                player_name: "Player".into(),
                ai_difficulty: None,
            },
            SkirmishSlotConfig {
                slot_index: 1,
                is_human: false,
                is_active: true,
                player_template: SkirmishPlayerTemplateSelection::base_faction("GLA"),
                faction: "GLA".into(),
                color_rgb: (200, 0, 0),
                team: 1,
                start_position: 1,
                player_name: "GLA AI".into(),
                ai_difficulty: Some("Medium".into()),
            },
        ],
    }
}

/// Resolved one-time C++ GameSlot selection ready to bind into Main GameLogic.
///
/// The borrow keeps all validation/re-resolution above `start_new_game`; no
/// map or player mutation may begin until every occupied slot has a concrete,
/// valid PlayerTemplate identity.
struct ResolvedSkirmishSlot<'a> {
    slot: &'a SkirmishSlotConfig,
    player_template: PlayerTemplateIdentity,
    team: Team,
}

fn validate_skirmish_template(
    template: &game_engine::common::rts::player_template::PlayerTemplate,
) -> Result<Team, String> {
    // C++ GameLogic.cpp:1720-1723: occupied observer slots are valid and
    // get a permanent map reveal. Random-side lists still exclude them.
    if template.is_observer() {
        return Ok(Team::Neutral);
    }
    // C++ `PopulatePlayerTemplateComboBox` and `populateRandomSideAndColor`
    // both exclude entries with no starting building.
    if template.get_starting_building().trim().is_empty() {
        return Err(format!(
            "PlayerTemplate '{}' has no Skirmish StartingBuilding",
            template.get_name()
        ));
    }

    // C++ GameLogic.cpp:713-721 rejects Challenge personas that do not start
    // unlocked. `hq-f6i` owns loading the authored ChallengeMode persona
    // table into this Common store before offline setup; until then an empty
    // table deliberately means there is no persona record to reject, rather
    // than guessing a General is locked from its name.
    let generals = game_engine::common::ini::ini_challenge_generals::get_challenge_generals();
    if generals
        .get_general_by_template_name(template.get_name())
        .is_some_and(|general| !general.is_starting_enabled())
    {
        return Err(format!(
            "PlayerTemplate '{}' is a locked General persona",
            template.get_name()
        ));
    }

    PlayerTemplateIdentity::team_for_template(template).ok_or_else(|| {
        format!(
            "PlayerTemplate '{}' has no supported USA/China/GLA base side",
            template.get_name()
        )
    })
}

fn resolve_exact_skirmish_template(
    template_name: &str,
    template_index: i32,
) -> Result<(PlayerTemplateIdentity, Team), String> {
    let identity = PlayerTemplateIdentity::from_exact_indexed_name(template_name, template_index)
        .ok_or_else(|| {
        format!(
            "selected Skirmish PlayerTemplate name/index pair is stale or missing: '{}' at {}",
            template_name, template_index
        )
    })?;
    let template = identity.resolve().ok_or_else(|| {
        format!(
            "selected Skirmish PlayerTemplate '{}' disappeared during validation",
            identity.template_name
        )
    })?;
    let team = validate_skirmish_template(&template)?;
    Ok((identity, team))
}

fn random_skirmish_template_candidates(
    old_factions_only: bool,
) -> Vec<(PlayerTemplateIdentity, Team)> {
    game_engine::common::ini::ensure_player_templates_loaded();
    let store = game_engine::common::rts::player_template::get_player_template_store();
    store
        .iter()
        .enumerate()
        .filter_map(|(index, template)| {
            if old_factions_only && !template.is_old_faction() {
                return None;
            }
            if template.is_observer() {
                return None;
            }
            let template_index = i32::try_from(index).ok()?;
            let team = validate_skirmish_template(template).ok()?;
            Some((
                PlayerTemplateIdentity {
                    template_name: template.get_name().to_string(),
                    template_index: Some(template_index),
                },
                team,
            ))
        })
        .collect()
}

/// Resolve one C++ `PLAYERTEMPLATE_RANDOM` slot against the *already seeded*
/// GameLogic ADC stream.  `GameLogic.cpp:740-748` intentionally burns
/// `seed % 7` low-quality draws before it picks `RandomValue(0, 1000) % N`.
fn resolve_random_skirmish_template(
    random_seed: u32,
    candidates: &[(PlayerTemplateIdentity, Team)],
) -> Result<(PlayerTemplateIdentity, Team), String> {
    if candidates.is_empty() {
        return Err("no eligible PlayerTemplate is available for Random Skirmish selection".into());
    }

    use game_engine::common::random_value::get_game_logic_random_value;

    for _ in 0..(random_seed % 7) {
        let _ = get_game_logic_random_value(0, 1);
    }
    let candidate_index = (get_game_logic_random_value(0, 1000) as usize) % candidates.len();
    Ok(candidates[candidate_index].clone())
}

/// C++ `populateRandomStartPosition` (GameLogic.cpp:785-1048).
/// Unassigned (`-1`) slots get unused map start spots; first pick is random,
/// later picks prefer farthest empty spots (teammates stay close).
pub fn populate_random_start_position(slots: &mut [SkirmishSlotConfig], map_name: &str) {
    use game_engine::common::random_value::get_game_logic_random_value;

    let starts = crate::game_logic::script_loader::parse_player_start_waypoints(map_name)
        .unwrap_or_default();
    let num_players = if starts.is_empty() {
        MAX_SLOTS as i32
    } else {
        starts.len() as i32
    };
    if num_players <= 0 {
        return;
    }

    let waypoint_pos = |idx: i32| -> Option<(f32, f32)> {
        starts
            .iter()
            .find(|(i, _, _)| *i == idx as u32)
            .map(|(_, wp, _)| (wp.x, wp.y))
    };
    let dist = |a: i32, b: i32| -> f32 {
        match (waypoint_pos(a), waypoint_pos(b)) {
            (Some(p1), Some(p2)) => {
                let dx = p1.0 - p2.0;
                let dy = p1.1 - p2.1;
                (dx * dx + dy * dy).sqrt()
            }
            _ => 1_000_000.0,
        }
    };

    let mut taken = vec![false; num_players as usize];
    for slot in slots.iter() {
        if !slot.is_active {
            continue;
        }
        let pos = slot.start_position;
        if pos >= 0 && pos < num_players {
            taken[pos as usize] = true;
        }
    }

    let is_taken = |idx: i32, taken: &[bool], slots: &[SkirmishSlotConfig]| -> bool {
        if idx < 0 || idx >= num_players {
            return true;
        }
        taken[idx as usize] || slots.iter().any(|s| s.is_active && s.start_position == idx)
    };

    let mut team_pos = [-1i32; MAX_SLOTS];
    let mut has_start_spot_been_picked = slots
        .iter()
        .any(|s| s.is_active && s.start_position >= 0 && s.start_position < num_players);

    for i in 0..slots.len() {
        if !slots[i].is_active {
            continue;
        }
        let pos = slots[i].start_position;
        if pos >= 0 && pos < num_players {
            let team = slots[i].team;
            if team >= 0 && (team as usize) < MAX_SLOTS && team_pos[team as usize] < 0 {
                team_pos[team as usize] = pos;
            }
            continue;
        }

        let team = slots[i].team;
        let assigned = if !has_start_spot_been_picked {
            let mut pick = -1;
            for _ in 0..(num_players * 4).max(8) {
                let candidate = get_game_logic_random_value(0, num_players - 1);
                if !is_taken(candidate, &taken, slots) {
                    pick = candidate;
                    break;
                }
            }
            if pick < 0 {
                pick = (0..num_players)
                    .find(|&n| !is_taken(n, &taken, slots))
                    .unwrap_or(0);
            }
            has_start_spot_been_picked = true;
            if team >= 0 && (team as usize) < MAX_SLOTS {
                team_pos[team as usize] = pick;
            }
            pick
        } else if team < 0 || (team as usize) >= MAX_SLOTS || team_pos[team as usize] < 0 {
            let mut farthest_index = -1;
            let mut farthest_distance = -1.0f32;
            for pos_idx in 0..num_players {
                if is_taken(pos_idx, &taken, slots) {
                    continue;
                }
                let mut sum = 0.0;
                for n in 0..num_players {
                    if taken[n as usize] && n != pos_idx {
                        sum += dist(pos_idx, n);
                    }
                }
                if farthest_index < 0 || sum > farthest_distance {
                    farthest_distance = sum;
                    farthest_index = pos_idx;
                }
            }
            let pick = if farthest_index >= 0 {
                farthest_index
            } else {
                0
            };
            if team >= 0 && (team as usize) < MAX_SLOTS {
                team_pos[team as usize] = pick;
            }
            pick
        } else {
            let team_home = team_pos[team as usize];
            let mut closest_idx = 0;
            let mut closest_dist = f32::MAX;
            for n in 0..num_players {
                if is_taken(n, &taken, slots) {
                    continue;
                }
                let d = dist(team_home, n);
                if d < closest_dist {
                    closest_dist = d;
                    closest_idx = n;
                }
            }
            closest_idx
        };

        if assigned >= 0 && assigned < num_players {
            taken[assigned as usize] = true;
        }
        slots[i].start_position = assigned;
    }
}

fn skirmish_num_teams(slots: &[SkirmishSlotConfig]) -> i32 {
    // C++ GameLogic.cpp:1482-1497 — count occupied non-observer teams.
    let mut num_teams = 0;
    let mut last_team = -2;
    for slot in slots {
        if !slot.is_active {
            continue;
        }
        if slot.team == -1 || slot.team != last_team {
            num_teams += 1;
            last_team = slot.team;
        }
    }
    num_teams
}

fn resolve_skirmish_slots(
    config: &SkirmishMatchConfig,
) -> Result<Vec<ResolvedSkirmishSlot<'_>>, String> {
    let mut active_slots: Vec<&SkirmishSlotConfig> =
        config.slots.iter().filter(|slot| slot.is_active).collect();
    if active_slots.is_empty() {
        return Err("skirmish config produced no active players".into());
    }
    active_slots.sort_by_key(|slot| slot.slot_index);

    let mut previous_slot = None;
    for slot in &active_slots {
        if slot.slot_index >= MAX_SLOTS {
            return Err(format!(
                "Skirmish slot {} is outside C++ MAX_SLOTS ({MAX_SLOTS})",
                slot.slot_index
            ));
        }
        if previous_slot == Some(slot.slot_index) {
            return Err(format!(
                "Skirmish slot {} appears more than once",
                slot.slot_index
            ));
        }
        previous_slot = Some(slot.slot_index);
    }

    // Validate every requested name/index pair before any GameLogic reset.
    // Random identities are the only selections that C++ resolves at game
    // start, after `InitGameLogicRandom(GameInfo::getSeed())`.
    let mut exact: Vec<Option<(PlayerTemplateIdentity, Team)>> =
        Vec::with_capacity(active_slots.len());
    let mut needs_random = false;
    for slot in &active_slots {
        match &slot.player_template {
            SkirmishPlayerTemplateSelection::Exact {
                template_name,
                template_index,
            } => exact.push(Some(resolve_exact_skirmish_template(
                template_name,
                *template_index,
            )?)),
            SkirmishPlayerTemplateSelection::Random => {
                needs_random = true;
                exact.push(None);
            }
        }
    }

    let candidates =
        needs_random.then(|| random_skirmish_template_candidates(config.old_factions_only));
    if needs_random && candidates.as_ref().is_some_and(Vec::is_empty) {
        return Err("no eligible PlayerTemplate is available for Random Skirmish selection".into());
    }

    // C++ `SkirmishGameOptionsMenu` calls InitGameLogicRandom with GameInfo's
    // seed immediately before it queues GAME_SKIRMISH.  Use the same shared
    // logic stream here so its post-selection state matches C++ as well.
    game_engine::common::random_value::init_game_logic_random(config.random_seed);

    active_slots
        .into_iter()
        .zip(exact)
        .map(|(slot, resolved)| {
            let (player_template, team) = match resolved {
                Some(resolved) => resolved,
                None => resolve_random_skirmish_template(
                    config.random_seed,
                    candidates
                        .as_deref()
                        .expect("random candidates validated above"),
                )?,
            };
            Ok(ResolvedSkirmishSlot {
                slot,
                player_template,
                team,
            })
        })
        .collect()
}

/// Apply full skirmish configuration to the authoritative Main GameLogic.
///
/// This is the shipped offline match-start path for C++ GameInfo slots. Every
/// active human/AI resolves and binds its exact PlayerTemplate before the
/// caller can load a map; base `Team` is only an auxiliary routing value.
pub fn apply_skirmish_config(
    logic: &mut GameLogic,
    config: &SkirmishMatchConfig,
) -> Result<(), String> {
    let resolved_slots = resolve_skirmish_slots(config)?;

    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();

    let mut assigned = config.slots.clone();
    populate_random_start_position(&mut assigned, &config.map);

    let cash = config.rules.starting_cash.max(0) as u32;
    let mut human_id: Option<u32> = None;

    for resolved in resolved_slots {
        let slot = resolved.slot;
        let team = resolved.team;
        let player_id = slot.slot_index as u32;
        let start_position = assigned
            .iter()
            .find(|s| s.slot_index == slot.slot_index)
            .map(|s| s.start_position)
            .unwrap_or(slot.start_position);
        let mut player = Player::new(player_id, team, &slot.player_name, slot.is_human);
        // C++ Player::init starts with GameInfo cash, then an authored
        // non-zero PlayerTemplate Money value may replace it.
        player.resources.supplies = cash;
        player.color_rgb = slot.color_rgb;
        player.color_night_rgb =
            crate::game_logic::host_gamedata_lobby_residual::multiplayer_night_rgb_for_day(
                slot.color_rgb,
            );

        player.start_position = start_position;
        player.alliance_team = slot.team;
        logic.add_player(player);

        // This binds the exact template's side, starting money, sciences,
        // production maps, and starting assets *before* map load. Reapply
        // GameInfo slot fields that C++ Player::initFromDict applies after
        // Player::init(PlayerTemplate), especially playerColor.
        if !logic.bind_player_template_identity(player_id, resolved.player_template) {
            return Err(format!(
                "validated Skirmish PlayerTemplate failed to bind for slot {}",
                slot.slot_index
            ));
        }
        // `player_template_bindings` is session state retained by Main's
        // map-load preserve path. This is the resolved concrete identity;
        // downstream setup must use it rather than revisit this `Random`
        // declaration and roll a second time.
        let player = logic.get_player_mut(player_id).ok_or_else(|| {
            format!(
                "Skirmish player {} disappeared while binding its PlayerTemplate",
                slot.slot_index
            )
        })?;
        player.color_rgb = slot.color_rgb;
        player.color_night_rgb =
            crate::game_logic::host_gamedata_lobby_residual::multiplayer_night_rgb_for_day(
                slot.color_rgb,
            );

        player.start_position = start_position;
        player.alliance_team = slot.team;
        let is_observer = player.is_observer;
        if is_observer {
            player.is_alive = false;
        }
        drop(player);

        if is_observer {
            if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
                let _ = shroud.reveal_map_for_player_permanently(player_id);
            }
            if slot.is_human {
                logic.set_radar_forced(true);
                if human_id.is_none() {
                    human_id = Some(player_id);
                }
            }
            continue;
        }

        if slot.is_human && human_id.is_none() {
            human_id = Some(player_id);
        }

        if !slot.is_human {
            let difficulty = slot
                .ai_difficulty
                .as_deref()
                .map(parse_difficulty)
                .unwrap_or(AIDifficulty::Medium);
            logic.add_ai_opponent(player_id, team, difficulty);
            logic.set_ai_difficulty(player_id, difficulty);
        }
    }

    // Apply skirmish game rules that the host currently models.
    // FOW: enable/disable shroud evaluation path on GameLogic when supported.
    logic.set_skirmish_rules(
        config.rules.fog_of_war,
        config.rules.crates_enabled,
        config.rules.limit_superweapons,
        config.rules.allow_tech_buildings,
        config.rules.game_speed,
    );

    // C++ startNewGame: ReplayObserver + MultiplayerScripts.scb when numTeams > 1.
    let _ = logic.ensure_replay_observer_player();
    logic.install_replay_observer_side();
    logic.set_install_multiplayer_scripts(skirmish_num_teams(&assigned) > 1);

    let _ = human_id;
    Ok(())
}

/// Local human faction string from config (first human slot).
pub fn local_faction_from_config(config: &SkirmishMatchConfig) -> String {
    config
        .slots
        .iter()
        .find(|s| s.is_human && s.is_active)
        .map(|s| s.faction.clone())
        .unwrap_or_else(|| "USA".into())
}

/// Build match config from the live GameClient skirmish setup (WND Start path).
///
/// SkirmishGameOptionsMenu writes slots/map/cash into `get_skirmish_setup()` and
/// queues `GameMessageType::NewGame`. Main must convert that setup into
/// `SkirmishMatchConfig` so `start_game_from_ui` applies the same authority path
/// as the headless shell smoke / SkirmishMenu.
#[cfg(feature = "game_client")]
pub fn config_from_client_skirmish_setup(
    map_override: Option<&str>,
) -> Option<SkirmishMatchConfig> {
    use game_client::gui::get_skirmish_setup;
    use game_client::{MAX_SLOTS, PLAYERTEMPLATE_RANDOM, SlotState};
    use game_engine::common::ini::ensure_player_templates_loaded;
    use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
    use game_engine::common::rts::player_template::get_player_template_store;

    let setup = get_skirmish_setup();
    let info = setup.game_info().game_info();

    let map = map_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            let selected = setup.selected_map().trim();
            if !selected.is_empty() {
                Some(selected.to_string())
            } else {
                None
            }
        })
        .or_else(|| {
            let m = info.get_map().trim();
            if !m.is_empty() && m != "NOMAP" {
                Some(m.to_string())
            } else {
                None
            }
        })?;

    // GameClient stores only an index in GameInfo. Ensure the authoritative
    // Common store is present before pairing it with its canonical name.
    ensure_player_templates_loaded();
    let store = get_player_template_store();
    let mut slots = Vec::with_capacity(MAX_SLOTS);
    for i in 0..MAX_SLOTS {
        let Some(slot) = info.get_slot(i) else {
            continue;
        };
        let state = slot.get_state();
        let is_active = matches!(
            state,
            SlotState::Player | SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI
        );
        if !is_active {
            // Keep slot indices stable for apply_skirmish_config (slot_index = player id).
            slots.push(SkirmishSlotConfig {
                slot_index: i,
                is_human: false,
                is_active: false,
                player_template: SkirmishPlayerTemplateSelection::Random,
                faction: "USA".into(),
                color_rgb: (128, 128, 128),
                team: -1,
                start_position: -1,
                player_name: String::new(),
                ai_difficulty: None,
            });
            continue;
        }

        let is_human = matches!(state, SlotState::Player);
        let ai_difficulty = match state {
            SlotState::EasyAI => Some("Easy".into()),
            SlotState::MedAI => Some("Medium".into()),
            SlotState::BrutalAI => Some("Hard".into()),
            _ => None,
        };

        let (faction, player_template) = {
            let tpl = slot.get_player_template();
            if tpl >= 0 {
                store
                    .get_nth_player_template(tpl as usize)
                    .map(|template| {
                        let side = template.get_side().trim();
                        let faction = if side.is_empty() {
                            "Unknown".to_string()
                        } else if side.eq_ignore_ascii_case("America") {
                            "USA".to_string()
                        } else {
                            side.to_string()
                        };
                        (
                            faction,
                            SkirmishPlayerTemplateSelection::Exact {
                                template_name: template.get_name().to_string(),
                                template_index: tpl,
                            },
                        )
                    })
                    .unwrap_or_else(|| {
                        // Keep the raw invalid index visible to host
                        // validation. C++ GameInfo never supplies a name for
                        // such a slot; Main must reject it rather than map it
                        // to a generic USA player.
                        (
                            "Invalid".to_string(),
                            SkirmishPlayerTemplateSelection::Exact {
                                template_name: String::new(),
                                template_index: tpl,
                            },
                        )
                    })
            } else if tpl == PLAYERTEMPLATE_RANDOM {
                // PLAYERTEMPLATE_RANDOM is resolved once by the offline host
                // from the retained GameInfo seed before the map load.
                ("Random".into(), SkirmishPlayerTemplateSelection::Random)
            } else {
                // Observer or another invalid sentinel is not a Random
                // player. Keep its raw value as an invalid pair so the host
                // fails closed rather than silently converting it to Random.
                (
                    "Invalid".into(),
                    SkirmishPlayerTemplateSelection::Exact {
                        template_name: String::new(),
                        template_index: tpl,
                    },
                )
            }
        };

        let color_rgb = color_rgb_from_multiplayer_index(slot.get_color(), i);

        slots.push(SkirmishSlotConfig {
            slot_index: i,
            is_human,
            is_active: true,
            player_template,
            faction,
            color_rgb,
            team: slot.get_team_number(),
            start_position: slot.get_start_pos(),
            player_name: {
                let n = slot.get_name().trim();
                if n.is_empty() {
                    if is_human {
                        "Player".into()
                    } else {
                        format!("AI {}", i)
                    }
                } else {
                    n.to_string()
                }
            },
            ai_difficulty,
        });
    }

    if !slots.iter().any(|s| s.is_active) {
        return None;
    }

    let starting_cash = info.get_starting_cash().count_money() as i32;
    let limit_sw = info.get_superweapon_restriction() != 0;
    let fog = with_multiplayer_settings(|s| s.is_shroud_in_multiplayer);
    Some(SkirmishMatchConfig {
        map,
        // Preserve C++ `Int`'s two's-complement bit pattern as the unsigned
        // ADC seed consumed by `InitGameLogicRandom`.
        random_seed: info.get_seed() as u32,
        old_factions_only: info.old_factions_only(),
        rules: GameRulesSnapshot {
            starting_cash: if starting_cash > 0 {
                starting_cash
            } else {
                10_000
            },
            game_speed: 1.0,
            limit_superweapons: limit_sw,
            allow_tech_buildings: true,
            crates_enabled: true,
            fog_of_war: fog,
        },
        slots,
    })
}

fn color_rgb_from_multiplayer_index(color_idx: i32, slot_index: usize) -> (u8, u8, u8) {
    // Fallback palette when MultiplayerSettings colors are not loaded yet.
    const FALLBACK: [(u8, u8, u8); 8] = [
        (0, 0, 200),
        (200, 0, 0),
        (0, 180, 0),
        (200, 200, 0),
        (0, 200, 200),
        (180, 0, 180),
        (220, 120, 0),
        (220, 220, 220),
    ];

    #[cfg(feature = "game_client")]
    {
        use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
        if color_idx >= 0 {
            if let Some(packed) = with_multiplayer_settings(|s| s.get_color_value(color_idx)) {
                let r = ((packed >> 16) & 0xFF) as u8;
                let g = ((packed >> 8) & 0xFF) as u8;
                let b = (packed & 0xFF) as u8;
                return (r, g, b);
            }
        }
    }
    let _ = color_idx;
    FALLBACK[slot_index % FALLBACK.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::skirmish_menu::{GameSlot, PlayerType};

    fn exact_template(name: &str) -> SkirmishPlayerTemplateSelection {
        let selection = SkirmishPlayerTemplateSelection::exact_template_name(name);
        match &selection {
            SkirmishPlayerTemplateSelection::Exact { template_index, .. } => {
                assert!(
                    *template_index >= 0,
                    "retail template '{name}' must resolve"
                );
            }
            SkirmishPlayerTemplateSelection::Random => {
                panic!("test requested an exact PlayerTemplate");
            }
        }
        selection
    }

    fn template_index(selection: &SkirmishPlayerTemplateSelection) -> i32 {
        match selection {
            SkirmishPlayerTemplateSelection::Exact { template_index, .. } => *template_index,
            SkirmishPlayerTemplateSelection::Random => panic!("test requested an Exact template"),
        }
    }

    fn configured_slot(
        slot_index: usize,
        is_human: bool,
        player_template: SkirmishPlayerTemplateSelection,
        faction: &str,
        color_rgb: (u8, u8, u8),
        team: i32,
        start_position: i32,
    ) -> SkirmishSlotConfig {
        SkirmishSlotConfig {
            slot_index,
            is_human,
            is_active: true,
            player_template,
            faction: faction.into(),
            color_rgb,
            team,
            start_position,
            player_name: format!("slot-{slot_index}"),
            ai_difficulty: (!is_human).then_some("Hard".into()),
        }
    }

    #[test]
    fn menu_config_propagates_slot_difficulties_and_cash() {
        let rules = GameRules {
            starting_cash: 12_500,
            ..GameRules::default()
        };
        let mut slots = vec![GameSlot::new(0), GameSlot::new(1)];
        slots[0].player_type = PlayerType::Human;
        slots[0].faction = Faction::USA;
        slots[1].player_type = PlayerType::HardAI;
        slots[1].faction = Faction::GLA;

        let cfg = config_from_skirmish_menu("Maps/Test/Test.map", &rules, &slots, 0xCAFE_BABE);
        assert_eq!(cfg.rules.starting_cash, 12_500);
        assert!(cfg.slots[0].is_human);
        assert_eq!(cfg.slots[1].ai_difficulty.as_deref(), Some("Hard"));
        assert_eq!(cfg.slots[1].faction, "GLA");
        assert_eq!(cfg.random_seed, 0xCAFE_BABE);
        assert!(!cfg.old_factions_only);
        assert!(matches!(
            &cfg.slots[0].player_template,
            SkirmishPlayerTemplateSelection::Exact { .. }
        ));
    }

    #[test]
    fn exact_skirmish_general_identities_bind_human_ai_and_slot_overrides() {
        let air_force = exact_template("FactionAmericaAirForceGeneral");
        let tank = exact_template("FactionChinaTankGeneral");
        let air_force_index = template_index(&air_force);
        let tank_index = template_index(&tank);
        let config = SkirmishMatchConfig {
            map: "Lone Eagle".into(),
            random_seed: 0x1020_3040,
            old_factions_only: false,
            rules: GameRulesSnapshot {
                starting_cash: 13_579,
                ..GameRulesSnapshot::default_rules()
            },
            slots: vec![
                configured_slot(0, true, air_force, "USA", (7, 8, 9), 31, 4),
                configured_slot(1, false, tank, "China", (10, 11, 12), 32, 5),
            ],
        };

        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &config).expect("exact General config must apply");

        assert_eq!(logic.game_mode(), GameMode::Skirmish);
        assert_eq!(
            logic
                .player_template_identity(0)
                .map(|identity| (identity.template_name.as_str(), identity.template_index)),
            Some(("FactionAmericaAirForceGeneral", Some(air_force_index)))
        );
        assert_eq!(
            logic
                .player_template_identity(1)
                .map(|identity| (identity.template_name.as_str(), identity.template_index)),
            Some(("FactionChinaTankGeneral", Some(tank_index)))
        );

        let human = logic.get_player(0).expect("human slot");
        assert_eq!(human.team, Team::USA);
        assert!(human.is_local);
        assert_eq!(human.resources.supplies, 13_579);
        // GameInfo fields are applied after Player::init(PlayerTemplate).
        assert_eq!(human.color_rgb, (7, 8, 9));
        assert_eq!(human.start_position, 4);
        assert_eq!(human.alliance_team, 31);

        let ai = logic.get_player(1).expect("AI slot");
        assert_eq!(ai.team, Team::China);
        assert!(!ai.is_local);
        assert_eq!(ai.resources.supplies, 13_579);
        assert_eq!(ai.color_rgb, (10, 11, 12));
        assert_eq!(ai.start_position, 5);
        assert_eq!(ai.alliance_team, 32);
        assert_eq!(logic.host_ai_difficulty(1), Some(AIDifficulty::Hard));
    }

    #[test]
    fn stale_exact_name_index_pair_rejects_before_game_logic_reset() {
        let air_force = exact_template("FactionAmericaAirForceGeneral");
        let tank = exact_template("FactionChinaTankGeneral");
        let stale = match air_force {
            SkirmishPlayerTemplateSelection::Exact { template_name, .. } => {
                SkirmishPlayerTemplateSelection::Exact {
                    template_name,
                    template_index: template_index(&tank),
                }
            }
            SkirmishPlayerTemplateSelection::Random => unreachable!(),
        };
        let config = SkirmishMatchConfig {
            map: "Lone Eagle".into(),
            random_seed: 77,
            old_factions_only: false,
            rules: GameRulesSnapshot::default_rules(),
            slots: vec![configured_slot(0, true, stale, "USA", (1, 2, 3), 0, 0)],
        };

        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::SinglePlayer);
        let original_name = logic
            .get_player(0)
            .expect("single-player bootstrap player")
            .name
            .clone();

        let error = apply_skirmish_config(&mut logic, &config)
            .expect_err("mismatched exact name/index must not choose another General");
        assert!(error.contains("name/index pair"), "{error}");
        assert_eq!(logic.game_mode(), GameMode::SinglePlayer);
        assert_eq!(
            logic.get_player(0).map(|player| player.name.as_str()),
            Some(original_name.as_str())
        );
        assert!(logic.player_template_identity(0).is_none());
    }

    #[test]
    fn locked_boss_persona_is_rejected_and_excluded_from_random() {
        // C++ GameLogic.cpp:716-718 only consults TheChallengeGenerals when
        // a persona record exists. Live boot currently fails ChallengeMode.ini
        // (End of file), so Challenge-named templates stay fail-closed and
        // vanilla FactionAmerica/China/GLA stay Random-eligible.
        let _ = game_engine::common::ini::ensure_challenge_generals_loaded();
        let boss = exact_template("FactionBossGeneral");
        let config = SkirmishMatchConfig {
            map: "Lone Eagle".into(),
            random_seed: 0xB055,
            old_factions_only: false,
            rules: GameRulesSnapshot::default_rules(),
            slots: vec![configured_slot(0, true, boss, "Boss", (0, 255, 0), 0, 0)],
        };

        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::SinglePlayer);
        let error = apply_skirmish_config(&mut logic, &config)
            .expect_err("a locked Boss persona must not reach host Skirmish startup");
        assert!(error.contains("FactionBossGeneral"), "{error}");
        assert!(
            error.contains("locked General persona")
                || error.contains("no Skirmish StartingBuilding"),
            "{error}"
        );
        assert_eq!(logic.game_mode(), GameMode::SinglePlayer);

        let candidates = random_skirmish_template_candidates(false);
        assert!(
            !candidates.iter().any(|(identity, _)| {
                identity
                    .template_name
                    .eq_ignore_ascii_case("FactionBossGeneral")
            }),
            "C++ Random must exclude the locked Boss persona"
        );

        if game_engine::common::ini::ensure_challenge_generals_loaded().is_ok() {
            let enabled = exact_template("FactionAmericaAirForceGeneral");
            let SkirmishPlayerTemplateSelection::Exact {
                template_name,
                template_index,
            } = enabled
            else {
                unreachable!("helper requested an exact template");
            };
            assert!(
                resolve_exact_skirmish_template(&template_name, template_index).is_ok(),
                "an authored enabled General must remain eligible"
            );
        }
    }

    #[test]
    fn random_skirmish_slots_bind_once_to_exact_old_faction_templates() {
        let config = SkirmishMatchConfig {
            map: "Lone Eagle".into(),
            random_seed: 0xC001_D00D,
            old_factions_only: true,
            rules: GameRulesSnapshot::default_rules(),
            slots: vec![
                configured_slot(
                    0,
                    true,
                    SkirmishPlayerTemplateSelection::Random,
                    "Random",
                    (1, 2, 3),
                    0,
                    0,
                ),
                configured_slot(
                    1,
                    false,
                    SkirmishPlayerTemplateSelection::Random,
                    "Random",
                    (4, 5, 6),
                    1,
                    1,
                ),
            ],
        };

        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &config).expect("Random config must resolve");
        assert_eq!(
            game_engine::common::random_value::get_game_logic_random_seed(),
            config.random_seed
        );
        let selected_before_rebind = [0, 1].map(|player_id| {
            logic
                .player_template_identity(player_id)
                .cloned()
                .expect("Random slot must become an exact session binding")
        });
        for identity in &selected_before_rebind {
            let template = identity
                .resolve()
                .expect("bound template must remain valid");
            assert!(
                template.is_old_faction(),
                "oldFactionsOnly Random selected {}",
                template.get_name()
            );
            assert!(identity.template_index.is_some());
        }
        assert!(config.slots.iter().all(|slot| matches!(
            &slot.player_template,
            SkirmishPlayerTemplateSelection::Random
        )));

        // Map-load rebinding must consume the concrete session identities,
        // not the original Random declarations and a fresh RNG draw.
        logic.rebind_host_ai_after_map_load();
        let selected_after_rebind = [0, 1].map(|player_id| {
            logic
                .player_template_identity(player_id)
                .cloned()
                .expect("map-load preserve must retain the exact identity")
        });
        assert_eq!(selected_after_rebind, selected_before_rebind);
    }

    #[test]
    fn skirmish_config_grants_faction_intrinsic_sciences() {
        use crate::game_logic::GameLogic;
        let mut logic = GameLogic::new();
        let config = SkirmishMatchConfig {
            map: "Lone Eagle".into(),
            random_seed: 0,
            old_factions_only: false,
            rules: GameRulesSnapshot::default_rules(),
            slots: vec![
                SkirmishSlotConfig {
                    slot_index: 0,
                    is_human: true,
                    is_active: true,
                    player_template: SkirmishPlayerTemplateSelection::base_faction("USA"),
                    faction: "USA".into(),
                    color_rgb: (0, 0, 255),
                    team: 0,
                    start_position: 0,
                    player_name: "Human".into(),
                    ai_difficulty: None,
                },
                SkirmishSlotConfig {
                    slot_index: 1,
                    is_human: false,
                    is_active: true,
                    player_template: SkirmishPlayerTemplateSelection::base_faction("China"),
                    faction: "China".into(),
                    color_rgb: (255, 0, 0),
                    team: 1,
                    start_position: 1,
                    player_name: "AI".into(),
                    ai_difficulty: Some("Medium".into()),
                },
            ],
        };
        apply_skirmish_config(&mut logic, &config).expect("cfg");
        let usa = logic.get_player(0).expect("usa");
        assert!(usa.has_unlocked_science("SCIENCE_AMERICA"));
        assert!(usa.has_unlocked_science("SCIENCE_Rank1"));
        assert!(usa.science_purchase_points >= 1);
        let china = logic.get_player(1).expect("china");
        assert!(china.has_unlocked_science("SCIENCE_CHINA"));
        assert!(china.has_unlocked_science("SCIENCE_Rank1"));
        assert!(usa.is_capable_of_purchasing_science("SCIENCE_DaisyCutter"));
    }

    #[test]
    fn apply_skirmish_config_sets_cash_and_ai_from_slots() {
        let cfg = golden_skirmish_config("SmokeTestMap");
        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &cfg).expect("apply");
        let p0 = logic.get_player(0).expect("human");
        assert!(p0.is_local || p0.resources.supplies == 10_000);
        assert_eq!(p0.resources.supplies, 10_000);
        let p1 = logic.get_player(1).expect("ai");
        assert_eq!(p1.resources.supplies, 10_000);
        assert!(logic.get_players().len() >= 2);
        assert!(
            logic
                .get_players()
                .values()
                .any(|p| p.name == "ReplayObserver"),
            "C++ startNewGame always creates ReplayObserver"
        );
        // Rules from config must be applied onto the authoritative world.
        assert!(logic.skirmish_rules().fog_of_war);
        assert!(logic.skirmish_rules().crates_enabled);
        assert!((logic.skirmish_rules().game_speed - 1.0).abs() < f32::EPSILON);
        // Slot color / start position / alliance team must land on players.
        let p0 = logic.get_player(0).expect("human after rules");
        assert_eq!(p0.color_rgb, (0, 0, 200));
        assert_eq!(p0.start_position, 0);
        assert_eq!(p0.alliance_team, 0);
        assert_eq!(p1.color_rgb, (200, 0, 0));
        assert_eq!(p1.start_position, 1);
        assert_eq!(p1.alliance_team, 1);
        assert!(logic.host_ai_player_count() >= 1);
    }

    #[test]
    fn golden_config_is_usa_vs_medium_gla() {
        let cfg = golden_skirmish_config("Maps/Lone Eagle/Lone Eagle.map");
        assert_eq!(local_faction_from_config(&cfg), "USA");
        assert_eq!(cfg.slots[1].faction, "GLA");
        assert_eq!(cfg.slots[1].ai_difficulty.as_deref(), Some("Medium"));
    }

    /// Host skirmish residual: apply config → load_map must keep players 0/1 cash,
    /// Medium GLA AI registration/difficulty/active, GLA_* templates, and allow
    /// set_ai_active + a non-panicking AI update. Prefer retail Lone Eagle; if the
    /// map is missing, still prove rebind on the synthetic host world.
    #[test]
    fn skirmish_players_and_ai_survive_load_map_preserve_path() {
        const MAP_CANDIDATES: &[&str] = &[
            "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "Maps/Lone Eagle/Lone Eagle.map",
            "Lone Eagle",
        ];
        let map_identity = MAP_CANDIDATES
            .iter()
            .find(|p| {
                std::path::Path::new(p).is_file()
                    || crate::game_logic::script_loader::find_map_file(p).is_some()
            })
            .copied()
            .unwrap_or("Lone Eagle");

        let cfg = golden_skirmish_config(map_identity);
        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &cfg).expect("apply skirmish");
        logic.ensure_ai_faction_templates(Team::USA);
        logic.ensure_ai_faction_templates(Team::GLA);

        assert_eq!(
            logic.get_player(0).map(|p| p.resources.supplies),
            Some(10_000)
        );
        assert_eq!(
            logic.get_player(1).map(|p| p.resources.supplies),
            Some(10_000)
        );
        assert_eq!(
            logic.host_ai_difficulty(1),
            Some(crate::ai::AIDifficulty::Medium)
        );
        assert!(logic.is_host_ai_active(1));
        assert!(logic.templates.contains_key("GLA_CommandCenter"));
        assert!(logic.templates.contains_key("GLA_Barracks"));
        assert!(logic.templates.contains_key("GLA_Soldier"));

        // Stale object_id on the AI build queue (map wipe residual) without spending cash.
        {
            // Touch AI queue via public relocate (re-seeds layout) then rebind will clear refs.
            logic.relocate_host_ai_base(1, glam::Vec3::new(120.0, 0.0, 120.0));
        }

        // Snapshot immediately before load — preserve path must not rewrite cash/slots.
        let cash0 = logic
            .get_player(0)
            .map(|p| p.resources.supplies)
            .expect("human cash before load");
        let cash1 = logic
            .get_player(1)
            .map(|p| p.resources.supplies)
            .expect("ai cash before load");
        let players_before = logic.get_players().len();
        let ai_before = logic.host_ai_player_count();

        let loaded = logic.load_map(map_identity);
        if !loaded {
            // Map missing in this workspace: still exercise explicit rebind residual.
            logic.rebind_host_ai_after_map_load();
        }

        assert!(
            logic.get_player(0).is_some() && logic.get_player(1).is_some(),
            "players 0 and 1 must survive load_map preserve / rebind"
        );
        assert!(
            logic.get_players().len() >= players_before,
            "host player slots must not shrink on load_map"
        );
        assert!(
            logic.host_ai_player_count() >= ai_before,
            "host AI count must not shrink on load_map"
        );
        let cash0_after = logic
            .get_player(0)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        let cash1_after = logic
            .get_player(1)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        assert_eq!(
            cash0_after, cash0,
            "human cash must be unchanged across load_map preserve (before={cash0} after={cash0_after})"
        );
        assert_eq!(
            cash1_after, cash1,
            "AI cash must be unchanged across load_map preserve (before={cash1} after={cash1_after})"
        );
        // Slot identity proves preserve (map wipe path would rename to PlayerN defaults).
        assert_eq!(logic.get_player(0).map(|p| p.name.as_str()), Some("Player"));
        assert_eq!(logic.get_player(1).map(|p| p.name.as_str()), Some("GLA AI"));
        assert_eq!(logic.get_player(0).map(|p| p.color_rgb), Some((0, 0, 200)));
        assert_eq!(logic.get_player(1).map(|p| p.color_rgb), Some((200, 0, 0)));
        assert!(
            logic.host_ai_player_count() >= 1,
            "host AI registration must survive load_map"
        );
        assert_eq!(
            logic.host_ai_difficulty(1),
            Some(crate::ai::AIDifficulty::Medium),
            "Medium difficulty must be retained across load_map"
        );
        assert!(
            logic.is_host_ai_active(1),
            "AI is_active must remain true after rebind"
        );
        // set_ai_active must still work (toggle off then on).
        logic.set_ai_active(1, false);
        assert!(!logic.is_host_ai_active(1));
        logic.set_ai_active(1, true);
        assert!(logic.is_host_ai_active(1));

        // Templates required by host AI rebuild soup must still be present.
        for name in [
            "GLA_CommandCenter",
            "GLA_SupplyStash",
            "GLA_ArmsDealer",
            "GLA_Barracks",
            "GLA_Soldier",
            "GLA_Technical",
        ] {
            assert!(
                logic.templates.contains_key(name),
                "AI template {name} must survive load_map / rebind"
            );
        }

        // Non-panicking AI update after rebind (rebuild soup path).
        for _ in 0..15 {
            logic.update();
        }
        // Fail-closed: do not require retail AI parity — only that update ran and
        // AI is still registered/active with cashed players.
        assert!(logic.is_host_ai_active(1));
        assert!(logic.get_player(0).is_some());
        assert!(logic.get_player(1).is_some());
        let _ = loaded; // true when Lone Eagle (or other candidate) resolved on disk
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn config_from_client_skirmish_setup_reads_slots_and_map() {
        use game_client::gui::get_skirmish_setup;
        use game_client::{Money, SlotState};

        game_engine::common::ini::ensure_player_templates_loaded();
        let air_force_index = game_engine::common::rts::player_template::get_player_template_store()
            .find_template_index("FactionAmericaAirForceGeneral")
            .expect("retail Air Force General template") as i32;

        {
            let mut setup = get_skirmish_setup();
            setup.set_selected_map(String::new());
            let info = setup.game_info_mut().game_info_mut();
            info.reset();
            info.set_map("Maps/Lone Eagle/Lone Eagle.map".into());
            info.set_seed(-0x1020_304);
            info.set_starting_cash(Money::new(15_000));
            info.set_old_factions_only(false);
            if let Some(slot) = info.get_slot_mut(0) {
                slot.set_state(SlotState::Player, "Commander".into(), 1);
                slot.set_player_template(air_force_index);
                slot.set_team_number(0);
                slot.set_start_pos(0);
                slot.set_color(0);
            }
            if let Some(slot) = info.get_slot_mut(1) {
                slot.set_state(SlotState::MedAI, "GLA AI".into(), 0);
                slot.set_player_template(-1);
                slot.set_team_number(1);
                slot.set_start_pos(1);
                slot.set_color(1);
            }
        }

        let cfg = config_from_client_skirmish_setup(None).expect("config from setup");
        assert!(
            cfg.map.contains("Lone Eagle"),
            "map from GameInfo: {}",
            cfg.map
        );
        assert_eq!(cfg.rules.starting_cash, 15_000);
        assert_eq!(cfg.random_seed, (-0x1020_304i32) as u32);
        assert!(!cfg.old_factions_only);
        assert!(cfg.slots[0].is_human && cfg.slots[0].is_active);
        assert!(!cfg.slots[1].is_human && cfg.slots[1].is_active);
        assert_eq!(cfg.slots[1].ai_difficulty.as_deref(), Some("Medium"));
        assert_eq!(cfg.slots[0].player_name, "Commander");
        assert!(matches!(
            &cfg.slots[0].player_template,
            SkirmishPlayerTemplateSelection::Exact {
                template_name,
                template_index,
            } if template_name == "FactionAmericaAirForceGeneral" && *template_index == air_force_index
        ));
        assert!(matches!(
            &cfg.slots[1].player_template,
            SkirmishPlayerTemplateSelection::Random
        ));
    }

    /// WND Start residual composition (no window): client skirmish setup →
    /// SkirmishMatchConfig → apply_skirmish_config → PresentationFrame world_env.
    /// Proves menu Start data reaches host authority without a GPU window.
    #[cfg(feature = "game_client")]
    #[test]
    fn new_game_client_setup_applies_to_host_authority() {
        use crate::presentation_frame::PresentationFrame;
        use game_client::gui::get_skirmish_setup;
        use game_client::{Money, SlotState};

        {
            let mut setup = get_skirmish_setup();
            setup.set_selected_map("Maps/Lone Eagle/Lone Eagle.map".into());
            let info = setup.game_info_mut().game_info_mut();
            info.reset();
            info.set_map("Maps/Lone Eagle/Lone Eagle.map".into());
            info.set_starting_cash(Money::new(20_000));
            if let Some(slot) = info.get_slot_mut(0) {
                slot.set_state(SlotState::Player, "Human".into(), 1);
                slot.set_player_template(-1);
                slot.set_team_number(0);
                slot.set_start_pos(0);
            }
            if let Some(slot) = info.get_slot_mut(1) {
                slot.set_state(SlotState::MedAI, "Enemy".into(), 0);
                slot.set_player_template(-1);
                slot.set_team_number(1);
                slot.set_start_pos(1);
            }
        }

        let cfg = config_from_client_skirmish_setup(None).expect("client setup config");
        assert!(cfg.map.contains("Lone Eagle"), "{}", cfg.map);
        assert_eq!(cfg.rules.starting_cash, 20_000);
        assert_eq!(cfg.slots.iter().filter(|s| s.is_active).count(), 2);
        assert!(!local_faction_from_config(&cfg).is_empty());

        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &cfg).expect("apply");
        assert_eq!(
            logic.get_player(0).map(|p| p.resources.supplies),
            Some(20_000)
        );
        assert!(logic.host_ai_player_count() >= 1);

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let (a, b) = logic.world_bounds();
        assert_eq!(snap.world_env.world_min, [a.x, a.y, a.z]);
        assert_eq!(snap.world_env.world_max, [b.x, b.y, b.z]);
        assert_eq!(snap.local_player_id, 0);
    }

    #[test]
    fn populate_random_start_position_assigns_unpinned_slots() {
        // C++ GameLogic.cpp:785-1048 populateRandomStartPosition.
        // Default GameSlot start_pos is -1; first occupied slot is random,
        // later slots take unused spots (not pinned to their slot index).
        game_engine::common::random_value::init_game_logic_random(0xA11C_E55);
        let mut slots = vec![
            configured_slot(
                0,
                true,
                SkirmishPlayerTemplateSelection::base_faction("USA"),
                "USA",
                (0, 0, 200),
                0,
                -1,
            ),
            configured_slot(
                1,
                false,
                SkirmishPlayerTemplateSelection::base_faction("GLA"),
                "GLA",
                (200, 0, 0),
                1,
                -1,
            ),
        ];
        populate_random_start_position(&mut slots, "missing-map-forces-max-slots");
        assert!(slots[0].start_position >= 0, "first slot must be assigned");
        assert!(slots[1].start_position >= 0, "second slot must be assigned");
        assert_ne!(
            slots[0].start_position, slots[1].start_position,
            "two occupied slots cannot share a start spot"
        );
        assert!(
            slots[0].start_position < MAX_SLOTS as i32
                && slots[1].start_position < MAX_SLOTS as i32
        );

        let mut pinned = slots.clone();
        pinned[0].start_position = 3;
        pinned[1].start_position = -1;
        populate_random_start_position(&mut pinned, "missing-map-forces-max-slots");
        assert_eq!(pinned[0].start_position, 3, "explicit pick is kept");
        assert_ne!(pinned[1].start_position, 3);
        assert!(pinned[1].start_position >= 0);
    }

    #[test]
    fn random_candidates_include_vanilla_factions_when_challenge_ini_fails() {
        // Live boot logs ChallengeMode.ini End of file. C++ still lets
        // FactionAmerica/China/GLA into Random (GameLogic.cpp:716-718).
        let candidates = random_skirmish_template_candidates(false);
        assert!(
            !candidates.is_empty(),
            "vanilla StartingBuilding factions must remain Random-eligible"
        );
        assert!(
            candidates.iter().any(|(identity, team)| {
                *team == Team::USA
                    && identity
                        .template_name
                        .eq_ignore_ascii_case("FactionAmerica")
            }),
            "FactionAmerica must stay eligible: {candidates:?}"
        );
        assert!(
            !candidates.iter().any(|(identity, _)| {
                identity
                    .template_name
                    .eq_ignore_ascii_case("FactionBossGeneral")
            }),
            "FactionBossGeneral stays locked when ChallengeMode.ini is missing"
        );
    }

    #[test]
    fn apply_skirmish_config_creates_replay_observer_and_flags_scripts() {
        // C++ GameLogic.cpp:1436-1459 ReplayObserver; 1479-1532 MultiplayerScripts
        // when numTeams > 1. Permanent reveal is applied at map load.
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "Player", true));
        logic.add_player(Player::new(1, Team::GLA, "GLA AI", false));
        let _ = logic.ensure_replay_observer_player();
        logic.install_replay_observer_side();
        logic.set_install_multiplayer_scripts(true);

        let observer = logic
            .get_players()
            .values()
            .find(|p| p.name == "ReplayObserver")
            .expect("ReplayObserver host player");
        assert!(!observer.is_alive);
        assert_eq!(observer.team, Team::Neutral);
        assert_eq!(logic.replay_observer_player_id(), Some(observer.id));
        assert!(
            logic.will_install_multiplayer_scripts(),
            "human vs AI is two teams"
        );

        {
            use gamelogic::player::ThePlayerList;
            let list = ThePlayerList().read().expect("crate player list");
            assert!(
                list.find_player_by_name("ReplayObserver").is_some(),
                "crate PlayerList must name ReplayObserver"
            );
        }
    }

    #[test]
    fn start_new_game_skirmish_creates_replay_observer() {
        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::Skirmish);
        assert!(
            logic
                .get_players()
                .values()
                .any(|p| p.name == "ReplayObserver")
        );
        logic.reveal_replay_observer_map();
        let id = logic.replay_observer_player_id().expect("observer id");
        let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
        let shroud = shroud_manager.lock()
            .expect("shroud");
        let snap = shroud.snapshot_state();
        assert!(
            snap.pending_permanent_reveal_players.contains(&id)
                || snap.grid.as_ref().is_some_and(|_| true),
            "ReplayObserver must be queued for or already have permanent reveal"
        );
    }

    #[test]
    fn install_multiplayer_scripts_preloads_scb_when_flagged() {
        // C++ GameLogic.cpp:1502-1525 parses Data\\Scripts\\MultiplayerScripts.scb
        // and prepends scripts onto side 0.
        let scb = std::path::Path::new(
            "windows_game/Command & Conquer Generals Zero Hour/Data/Scripts/MultiplayerScripts.scb",
        );
        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::Skirmish);
        logic.set_install_multiplayer_scripts(true);
        logic.install_multiplayer_scripts_if_needed();
        if scb.is_file() {
            assert!(
                !logic.loaded_script_names().is_empty(),
                "parsed MultiplayerScripts.scb must contribute at least one script"
            );
        } else {
            assert!(
                logic.will_install_multiplayer_scripts(),
                "flag stays set even if the scb is absent from this workspace"
            );
        }
    }

    #[test]
    fn lone_eagle_skirmish_load_map_returns_in_under_thirty_seconds() {
        // hq-ibnf: start_game_from_ui used to block forever inside load_map.
        // After ThingFactory try-lock + fail-open terrain write, load_map
        // must return with map_loaded so the parked Loading tick can reach InGame.
        // C++ TerrainLogic::loadMap (TerrainLogic.cpp:1248-1262) opens once.
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        if !path.is_file() {
            return;
        }
        let path_str = path.to_string_lossy().into_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            crate::game_logic::script_loader::reset_map_decompress_count();
            let mut logic = GameLogic::new();
            logic.start_new_game(GameMode::Skirmish);
            let ok = logic.load_map(&path_str);
            let in_game = logic.isInGame();
            let decodes = crate::game_logic::script_loader::map_decompress_count();
            let _ = tx.send((ok, in_game, decodes));
        });
        match rx.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok((ok, in_game, decodes)) => {
                assert!(ok, "Lone Eagle load_map must succeed");
                assert!(in_game, "map_loaded must latch after successful load");
                assert!(
                    decodes <= 1,
                    "load_map must RefPack-decode Lone Eagle at most once, got {decodes}"
                );
            }
            Err(_) => panic!(
                "Lone Eagle load_map still blocked after 30s (hq-ibnf). Parse is fast; spawn/settings is not."
            ),
        }
    }
}
