//! Operations and queries of the match executing an authored script.

use super::command_button_actions::CommandButtonAction;
use super::interpreter::ScriptInterpreter;
use super::lifecycle_actions::LifecycleAction;
use super::map_reveals::{NamedReveal, NamedRevealOperation, WaypointVisibility};
use super::object_appearance_actions::ObjectAppearanceAction;
use super::object_control_actions::{ForceSelection, NamedSpecialPower};
use super::object_status_actions::ObjectStatusAction;
use super::presentation_actions::ScriptPresentationWorld;
use super::program::{ComparisonType, Script, ScriptActionType};
use super::program_execution::ScriptDifficulty;
use super::radar::RadarEventTarget;
use super::world::*;
use crate::{GameLogicResult, science::ScienceAvailabilityType};
use game_engine::common::{
    message_stream::ObjectID,
    random_value::SessionRandom,
    rts::{ScienceType, team::TeamID},
};
use glam::Vec3;

/// Borrowed access to the receiving match for one synchronous script walk.
///
/// The team-state phase borrows its running interpreter for synchronous
/// subroutine calls.
pub trait ScriptExecutionDriver {
    /// The native match supplies all authored presentation effects by loan.
    fn presentation_world(&mut self) -> Option<&mut dyn ScriptPresentationWorld> {
        None
    }

    /// The receiving world's logic stream for authored random timers.
    fn logic_random(&self) -> Option<&SessionRandom> {
        None
    }

    /// A supplied duration, including zero, is authoritative for this world.
    fn script_audio_duration_ms(&self, _name: &str) -> Option<f32> {
        None
    }

    fn music_track_completed(
        &mut self,
        _track: &str,
        _number_of_times: i32,
    ) -> Option<GameLogicResult<bool>> {
        None
    }

    fn audio(&mut self, _request: ScriptAudioRequest<'_>) -> Option<GameLogicResult<()>> {
        None
    }

    fn speech_play(&mut self, _name: &str, _allow_overlap: bool) -> Option<GameLogicResult<()>> {
        None
    }

    fn popup_message(
        &mut self,
        _message: &str,
        _x: i32,
        _y: i32,
        _width: i32,
        _pause: bool,
        _pause_music: bool,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Receiving application handles CampaignManager mutations at the authored
    /// action phase. `None` preserves standalone handler behavior.
    fn set_campaign_victorious(&mut self, _victorious: bool) -> Option<GameLogicResult<()>> {
        None
    }

    /// Receiving application applies map-wide visibility actions against its
    /// own player and shroud owners. `None` keeps standalone Core behavior.
    fn map_visibility_all(
        &mut self,
        _operation: ScriptMapVisibilityOperation,
        _player_name: &str,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Place an authored radar event on the driving world's radar immediately.
    fn radar_event(
        &mut self,
        _target: RadarEventTarget<'_>,
        _event_type: game_engine::common::system::radar::RadarEventType,
        _this_object: Option<u32>,
        _calling_team: Option<u32>,
        _condition_team: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Apply and undo a temporary look or cover for the receiving world.
    fn waypoint_visibility(
        &mut self,
        _operation: WaypointVisibility,
        _waypoint: &str,
        _radius: f32,
        _player: &str,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Set authored radar visibility on the receiving match.
    fn radar_visibility(&mut self, _enabled: bool) -> Option<GameLogicResult<()>> {
        None
    }

    /// Apply the selected waypoint reveal to the driving world.
    fn named_map_visibility(
        &mut self,
        _operation: NamedRevealOperation,
        _reveal: &NamedReveal,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Native host applies this window operation immediately when it can lend
    /// the receiving UI; standalone handlers retain their existing path.
    fn create_win_lose_window(&mut self, _layout_filename: &str) -> Option<GameLogicResult<()>> {
        None
    }

    /// Synchronous C++ closeWindows and GameLogic::closeWindows presentation.
    /// Some is authoritative, including errors; no other owner is consulted.
    fn destroy_win_lose_window(&mut self) -> Option<GameLogicResult<()>> {
        None
    }
    fn close_game_windows(&mut self) -> Option<GameLogicResult<()>> {
        None
    }

    fn initialize_named_cache(&mut self) -> Option<()> {
        None
    }

    /// Main intercepts the owned flag; standalone Core retains its original owner.
    fn set_draw_icon_ui(&mut self, _enabled: bool) -> Option<GameLogicResult<()>> {
        None
    }

    /// C++ GameLogic's per-match cinematic marker flag.
    fn set_show_behind_building_markers(&mut self, _enabled: bool) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP4793/4814: missing objects or AI do not start the sequential timer.
    fn named_frame_command(
        &mut self,
        _request: ScriptNamedCommand<'_>,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<()> {
        ScriptOwnerQuery::Unavailable
    }

    fn named_query_fact(
        &self,
        _name: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<(u32, HostScriptQueryObject)> {
        ScriptOwnerQuery::Unavailable
    }
    fn receiving_player_query_name(
        &self,
        _name: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<String> {
        ScriptOwnerQuery::Unavailable
    }

    /// C++ ScriptEngine::update maps each side slot to its receiving world's
    /// corresponding Player. `Present(None)` means the world has no Player for
    /// that side and must not consult the standalone PlayerList.
    fn side_script_current_player(&self, _side_index: usize) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Unavailable
    }

    /// Select the authored script flags from the receiving AI. Missing uses
    /// the interpreter's global setting, as it does for a non-AI Player.
    fn current_player_ai_difficulty(&self, _player: &str) -> ScriptOwnerQuery<ScriptDifficulty> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve authored Player tokens before Core's PlayerList adapter runs.
    /// The interpreter's current Player is supplied by the exact driving walk.
    fn script_player_name_token(
        &mut self,
        _raw: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Unavailable
    }

    fn named_selection(
        &self,
        _literal_name: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<ScriptSelectionStatus> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP2330 / Team2223,2414: synchronous effects in the supplied match.
    fn object_lifecycle(
        &mut self,
        _request: &LifecycleAction,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn kill_player(
        &mut self,
        _player: &str,
        _current: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn prioritize_skirmish_building(
        &mut self,
        _template: &str,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn player_hunt(&mut self, _player: &str, _current: Option<&str>) -> Option<GameLogicResult<()>> {
        None
    }

    fn exclude_player_from_score(
        &mut self,
        _player: &str,
        _current: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn player_relates(
        &mut self,
        _source: &str,
        _destination: &str,
        _relationship: game_engine::common::game_common::Relationship,
        _current: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn object_status_action(
        &mut self,
        _action: &ObjectStatusAction,
        _this_object: Option<ObjectID>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn object_appearance(
        &mut self,
        _request: &ObjectAppearanceAction,
        _this_object: Option<ObjectID>,
        _calling_team: Option<u32>,
        _condition_team: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn force_select_object(
        &mut self,
        _request: &ForceSelection<'_>,
        _calling_team: Option<u32>,
        _condition_team: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn object_sound(
        &mut self,
        _request: &HostScriptObjectSoundRequest,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP3978–4020: synchronous cash mutation in this script's player owner.
    fn money(
        &mut self,
        _request: &HostScriptMoneyRequest,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP1008/1589 requires the actual contain interface, even at zero capacity.
    fn named_containment(
        &self,
        _unit: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<ScriptContainmentStatus> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP1217 compares controlling Player identity without an alive gate.
    fn named_owned_by_player(
        &self,
        _unit: &str,
        _player: &str,
        _current_player: Option<&str>,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    fn named_lifecycle(
        &self,
        _name: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<ScriptNamedLifecycle> {
        ScriptOwnerQuery::Unavailable
    }

    /// Supplied match applies existing-object bonuses and records future births.
    fn set_object_difficulty_bonus(&mut self, _enable: bool) -> Option<()> {
        None
    }

    fn after_action(&mut self) -> GameLogicResult<()>;

    /// CPP5573: supplied match owns its Player/prototype/instance state walk.
    fn update_team_states(
        &mut self,
        _interpreter: &ScriptInterpreter,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn world(&mut self, _request: ScriptWorldRequest<'_>) -> Option<GameLogicResult<()>> {
        None
    }

    fn create_named_object(
        &mut self,
        _name: &str,
        _request: ScriptObjectCreation<'_>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn create_named_object_at_waypoint(
        &mut self,
        _name: Option<&str>,
        _template: &str,
        _team: &str,
        _waypoint: &str,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn create_object(
        &mut self,
        _request: ScriptObjectCreation<'_>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP7785: the driving match owns object-count invalidation timing.
    fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
        ScriptOwnerQuery::Unavailable
    }

    /// Original win/lose layout selection observes this session's local player.
    fn local_player_observer(&self) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// Query OpenContain's last entering player in the receiving world.
    fn building_entered_by_player(
        &self,
        _player: &str,
        _building: &str,
        _current_player: Option<&str>,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve the literal observer before resolving or caching the SIDE operand.
    fn sighting_observer(
        &self,
        _unit: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<ObjectID> {
        ScriptOwnerQuery::Unavailable
    }

    /// Search the receiving partition with the interpreter's selected filter.
    fn any_sighted_object(
        &self,
        _observer: ObjectID,
        _player: u32,
        _filter: ScriptSightingFilter<'_>,
    ) -> bool {
        false
    }

    /// Resolve an authored SIDE or its cached mask to this world's event slot.
    fn script_event_player_slot(
        &mut self,
        _player: &str,
        _mask: u32,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<usize> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve a SIDE against this world's roster, including a cached player mask.
    fn condition_player_id(
        &mut self,
        _player: &str,
        _mask: u32,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<u32> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP ScriptConditions2354/ObjectTypes97: candidates come from this
    /// execution's live list, with raw player identity and borrowed context.
    /// Missing is authoritative false; Unavailable retains standalone lookup.
    fn skirmish_player_can_build_any(
        &mut self,
        _player: &str,
        _mask: u32,
        _object_types: &[String],
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        ScriptOwnerQuery::Unavailable
    }

    /// C++ ScriptConditions::evaluateSkirmishSuppliesWithinDistancePerimeter.
    fn supplies_value_in_perimeter(
        &mut self,
        _player: u32,
        _distance: f32,
        _area: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<f32> {
        ScriptOwnerQuery::Unavailable
    }

    /// Present(false) is a completed empty search and must latch. Missing is
    /// an absent trigger/query and must not latch. After player selection,
    /// neither Missing nor Unavailable permits a foreign-owner fallback.
    fn tech_building_within_distance(
        &mut self,
        _player: u32,
        _distance: f32,
        _area: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP doUnitReceiveUpgrade: Some completes giveUpgrade synchronously
    /// on this receiving owner, including authoritative missing-name no-ops.
    /// None selects the standalone Core adapter.
    fn named_upgrade(
        &mut self,
        _name: &str,
        _upgrade: &str,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Named countdown actions use this owner before any standalone named
    /// tracker or action-handler access. Some includes authoritative absence.
    fn named_special_power_countdown(
        &mut self,
        _name: &str,
        _power: &str,
        _action: ScriptActionType,
        _seconds: i32,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Some selects this owner exclusively, including missing objects or a
    /// failed command. None retains the standalone Core command adapter.
    fn nearest_object_move(
        &mut self,
        _request: ScriptNearestObjectRequest<'_>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_attack_named(
        &mut self,
        _team: &str,
        _target: &str,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP ScriptActions1336 resolves the driving object and qualified polygon.
    fn named_object_id(
        &self,
        _name: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<ObjectID> {
        ScriptOwnerQuery::Unavailable
    }
    fn named_superweapon_display(
        &mut self,
        _name: &str,
        _show: bool,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_guard_position(
        &mut self,
        _team: &str,
        _waypoint: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_guard_area(
        &mut self,
        _team: &str,
        _area: &str,
        _current_player: Option<&str>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_guard_object(
        &mut self,
        _team: &str,
        _target: &str,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_attack_team(
        &mut self,
        _source: &str,
        _target: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_attack_area(
        &mut self,
        _team: &str,
        _area: &str,
        _current_player: Option<&str>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// None follows a single named waypoint; Some(exact) follows its path.
    fn named_waypoint(
        &mut self,
        _unit: &str,
        _waypoint: &str,
        _follow: Option<bool>,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Execute C++ `NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH` against the
    /// receiving session's live Object, weapon catalog, and waypoint data.
    fn named_fire_weapon_following_waypoint_path(
        &mut self,
        _unit: &str,
        _waypoint_path: &str,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_hunt_button(
        &mut self,
        _team: &str,
        _button: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Resolve an existing raw Team instance in the driving world's script
    /// context and issue C++ `AIGroup::groupHunt(CMD_FROM_SCRIPT)`.
    fn team_hunt(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Resolve the contextual Team and issue its transport-loading orders in
    /// the session executing this action.
    fn team_load_transports(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Mutate the receiving Team at the action's original observation point.
    fn team_relationship_override(
        &mut self,
        _action: ScriptTeamRelationship<'_>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Apply the Player's override to the resolved Team in this session.
    fn player_team_relationship_override(
        &mut self,
        _action: ScriptPlayerTeamRelationship<'_>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Resolve and command the named team's live roster in the receiving
    /// session, matching C++ `doTeamCaptureNearestUnownedFactionUnit` order.
    fn team_capture_nearest_unowned_faction_unit(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_transfer(
        &mut self,
        _team: &str,
        _player: &str,
        _current_player: Option<&str>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    /// Transfer a player's objects and money through the receiving match.
    fn transfer_player_assets(
        &mut self,
        _source: &str,
        _destination: &str,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn sell_everything(
        &mut self,
        _player: &str,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Apply PLAYER_DISABLE/ENABLE unit, base or factory construction against
    /// the receiving world. Some is authoritative, including missing players;
    /// None retains the standalone Core PlayerList adapter.
    fn player_construction(
        &mut self,
        _request: &ScriptPlayerConstructionRequest,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Apply rank/skill actions synchronously on the receiving match.
    /// `None` retains the standalone Core PlayerList implementation.
    fn player_rank(
        &mut self,
        _request: &ScriptPlayerRankRequest,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_waypoint_path(
        &mut self,
        _team: &str,
        _path: &str,
        _as_team: bool,
        _exact: bool,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// C++ `ScriptActions::doMoveToWaypoint`: the receiving session resolves
    /// its team and waypoint, then applies the script move synchronously.
    fn team_move_to_waypoint(
        &mut self,
        _team: &str,
        _waypoint: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn named_attack_area(
        &mut self,
        _unit: &str,
        _area: &str,
        _current_player: Option<&str>,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn named_attack_team(
        &mut self,
        _unit: &str,
        _team: &str,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP ScriptActions6585 resolves this owner before changing its AI mood.
    fn named_attitude(
        &mut self,
        _unit: &str,
        _this_object: Option<ObjectID>,
        _mood: i32,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP ScriptActions6034: update the driving session's constructor-only
    /// HULK lifetime override. Some is authoritative; None is standalone Core.
    fn override_hulk_lifetime(&mut self, _frames: i32) -> Option<GameLogicResult<()>> {
        None
    }

    fn named_attack_priority(
        &mut self,
        _name: &str,
        _set: &str,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn named_evac_disposition(
        &mut self,
        _name: &str,
        _disposition: i32,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn named_panel_flag(
        &mut self,
        _name: &str,
        _flag: super::object_actions::ObjectPanelFlag,
        _enabled: bool,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_panel_flag(
        &mut self,
        _team: &str,
        _flag: super::object_actions::ObjectPanelFlag,
        _enabled: bool,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn named_warehouse_value(
        &mut self,
        _name: &str,
        _cash: i32,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn named_runtime(
        &mut self,
        _request: ScriptNamedRuntimeRequest<'_>,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn garrison_request(
        &mut self,
        _request: HostScriptGarrisonEnterExitRequest,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_exit_all(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn named_special_power(
        &mut self,
        _request: &NamedSpecialPower<'_>,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn command_button_request(
        &mut self,
        _request: &CommandButtonAction<'_>,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_face(
        &mut self,
        _team: &str,
        _target: super::navigation_actions::TeamFaceTarget<'_>,
        _this_object: Option<ObjectID>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn named_command(
        &mut self,
        _request: ScriptNamedCommand<'_>,
        _this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP ScriptActions1882: the driving owner resolves the raw team against
    /// its own roster. Both context identities are supplied before token resolution;
    /// THIS_TEAM prefers calling_team. Some is authoritative even for absence
    /// or an error; None retains the standalone Core adapter.
    fn team_guard(
        &mut self,
        _team: &str,
        _calling_team: Option<TeamID>,
        _condition_team: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// CPP timed Guard and Idle both issue groupIdle, with no disband.
    fn team_idle(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<bool>> {
        None
    }

    fn team_guard_supply_center(
        &mut self,
        _team: &str,
        _supplies: i32,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_guard_tunnel(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn player_units_idle(&mut self, _idle: bool) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_stop(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
        _disband: bool,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Receiving team's definition, waypoint and creation callbacks.
    fn create_reinforcements(
        &mut self,
        _team: &str,
        _waypoint: &str,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Main owns team membership and synchronous inactive-create callbacks.
    fn merge_team(
        &mut self,
        _source: &str,
        _destination: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_attitude(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
        _mood: i32,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Some selects this owner exclusively, including a failed effect.
    fn water(&mut self, _request: ScriptWaterRequest<'_>) -> Option<GameLogicResult<()>> {
        None
    }

    /// Some is authoritative even when the owner is missing or returns an
    /// error. None selects only the standalone Core player adapter.
    fn ai_player(&mut self, _request: ScriptAiPlayerRequest<'_>) -> Option<GameLogicResult<()>> {
        None
    }

    /// None selects the standalone retained-handler adapter. Some is
    /// authoritative, including a failed callback: never retry on another
    /// owner's handler. Effects are flushed by after_action, as before.
    fn display(&mut self, _request: ScriptDisplayRequest<'_>) -> Option<GameLogicResult<()>> {
        None
    }

    /// Same authoritative/fallback contract as display; no retained foreign owner.
    fn camera(&mut self, _request: ScriptCameraRequest<'_>) -> Option<GameLogicResult<()>> {
        None
    }

    /// Raw authored waypoint position for camera and positional sound actions.
    /// Some is authoritative, including a missing waypoint or failed query.
    /// None selects terrain bound to the standalone interpreter.
    fn script_waypoint_position(&mut self, _name: &str) -> Option<GameLogicResult<Option<Vec3>>> {
        None
    }

    /// Some(false) is authoritative; None selects the standalone camera adapter.
    fn camera_movement_finished(&mut self) -> Option<bool> {
        None
    }

    /// CPP ScriptConditions397: integer current position in this qualified polygon.
    /// Missing is authoritative; Unavailable retains standalone Core discovery.
    fn named_inside_area(
        &mut self,
        _unit: &str,
        _area: &str,
        _current_player: Option<&str>,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP ScriptConditions1614: non-inert unit's current/previous-frame entry edge.
    fn named_entered_area(
        &mut self,
        _unit: &str,
        _area: &str,
        _current_player: Option<&str>,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP ScriptConditions1637: exit includes inert objects, unlike entry.
    fn named_exited_area(
        &mut self,
        _unit: &str,
        _area: &str,
        _current_player: Option<&str>,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP ScriptConditions924: selected body's current / InitialHealth percent.
    /// Missing is authoritative absence; Unavailable retains standalone access.
    fn unit_health_percent(
        &self,
        _name: &str,
        _this_object: Option<ObjectID>,
    ) -> ScriptOwnerQuery<i32> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP ScriptConditions872: BUILT_BY_PLAYER requires a literal definition
    /// before resolving an ObjectTypes list with the same name.
    /// CPP VictoryConditions local allied victory, allied defeat, local defeat.
    /// The live owner derives these facts synchronously; no process flag mirror.
    fn local_victory_state(&self) -> ScriptOwnerQuery<(bool, bool, bool)> {
        ScriptOwnerQuery::Unavailable
    }

    fn set_buildable_status_override(
        &mut self,
        _template: &str,
        _status: i32,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn command_bar_override(
        &mut self,
        _button: &str,
        _template: &str,
        _designer_slot: Option<i32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn priority_templates_with_kind(
        &self,
        _kind: game_engine::common::system::kind_of::KindOfMask,
    ) -> ScriptOwnerQuery<Vec<String>> {
        ScriptOwnerQuery::Unavailable
    }

    fn literal_template_exists(&self, _name: &str) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    fn team_nearest_button(
        &mut self,
        _request: &CommandButtonAction<'_>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_partial_button(
        &mut self,
        _team: &str,
        _button: &str,
        _percentage: f32,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn skirmish_attack_nearest_group(
        &mut self,
        _team: &str,
        _comparison: Option<ComparisonType>,
        _value: i32,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn skirmish_button_on_most_valuable(
        &mut self,
        _team: &str,
        _button: &str,
        _range: f32,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    fn team_panic(
        &mut self,
        _team: &str,
        _path: &str,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn team_wander(
        &mut self,
        _team: &str,
        _path: Option<&str>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn skirmish_approach_path(
        &mut self,
        _team: &str,
        _path: &str,
        _as_team: bool,
        _follow: bool,
        _current_player: Option<&str>,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        None
    }
    fn skirmish_special_power_exists(&self, _power: &str) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    fn skirmish_special_power_readiness(
        &self,
        _player: u32,
        _power: &str,
    ) -> ScriptOwnerQuery<ScriptSpecialPowerReadiness> {
        ScriptOwnerQuery::Unavailable
    }
    /// Receiving named Object's actual AI completed-waypoint labels.
    /// Missing is authoritative; only Unavailable retains standalone fallback.
    fn named_waypoint_complete(
        &self,
        _unit: &str,
        _path: &str,
        _this_object: Option<u32>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }
    fn team_waypoint_complete(
        &self,
        _team: &str,
        _path: &str,
        _calling: Option<u32>,
        _condition: Option<u32>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// Receiving-team membership at one trigger polygon observation point.
    /// Missing means the owner lacks the named team or trigger area.
    fn team_area_membership(
        &mut self,
        _team: &str,
        _area: &str,
        _current_player: Option<&str>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
        _surfaces: u32,
    ) -> ScriptOwnerQuery<ScriptTeamAreaMembership> {
        ScriptOwnerQuery::Unavailable
    }

    fn skirmish_census(&self, _query: ScriptSkirmishCensus) -> ScriptOwnerQuery<i32> {
        ScriptOwnerQuery::Unavailable
    }
    fn player_discovered_by(
        &self,
        _player: u32,
        _viewer: u32,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// `Missing` means the area was absent before the SIDE lookup.
    /// ScriptConditions.cpp:2607 checks the player before the first polygon lookup.
    fn player_outside_area_preflight(
        &mut self,
        _player: &str,
        _mask: u32,
        _area: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        ScriptOwnerQuery::Unavailable
    }

    fn player_area_value(
        &mut self,
        _player: &str,
        _player_mask: u32,
        _area: &str,
        _current_player: Option<&str>,
        _query: ScriptPlayerAreaQuery<'_>,
    ) -> ScriptOwnerQuery<ScriptPlayerAreaCensus> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve the SIDE parameter at the condition and read the driving Player's
    /// live census. A cached mask takes precedence over the authored name.
    fn player_census(
        &mut self,
        _player: &str,
        _player_mask: u32,
        _current_player: Option<&str>,
        _scope: ScriptPlayerCensusScope,
    ) -> ScriptOwnerQuery<(i32, HostScriptPlayerCensus)> {
        ScriptOwnerQuery::Unavailable
    }

    fn player_template_count(
        &self,
        _player: i32,
        _templates: &[String],
        _ignore_dead: bool,
    ) -> ScriptOwnerQuery<Option<i32>> {
        ScriptOwnerQuery::Unavailable
    }

    fn player_count_active(&self, _player: i32) -> bool {
        true
    }

    fn player_has_any_objects(
        &self,
        _player: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve the player and return its mask and supply condition at the
    /// evaluation point (ScriptConditions.cpp:2547–2575).
    fn player_supply_source(
        &mut self,
        _player: &str,
        _player_mask: u32,
        _current_player: Option<&str>,
        _query: ScriptSupplyQuery,
    ) -> ScriptOwnerQuery<(u32, bool)> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve the receiving player slot and, if defined, the science type.
    /// A missing definition still leaves a cacheable player selection.
    fn player_acquired_science_source(
        &mut self,
        _player: &str,
        _player_mask: u32,
        _science: &str,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<(usize, Option<ScienceType>)> {
        ScriptOwnerQuery::Unavailable
    }

    fn player_can_purchase_science(
        &self,
        _player: u32,
        _science: &str,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    fn player_faction(&self, _player: u32) -> Option<&str> {
        None
    }

    fn player_start_index(&self, _player: u32) -> Option<i32> {
        None
    }

    fn script_area_exists(
        &mut self,
        _area: &str,
        _current_player: Option<&str>,
    ) -> Option<bool> {
        None
    }

    fn player_science_action(
        &mut self,
        _player: &str,
        _science: &str,
        _grant: bool,
        _current_player: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// Apply ScriptActions.cpp::doPlayerSetScienceAvailability to the driving world.
    fn player_science_availability(
        &mut self,
        _player: &str,
        _science: &str,
        _availability: ScienceAvailabilityType,
        _current_player: Option<&str>,
    ) -> ScriptOwnerQuery<()> {
        ScriptOwnerQuery::Unavailable
    }

    /// Present/Missing are authoritative; Unavailable preserves standalone Core behavior.
    /// CPP ENABLE/DISABLE_SCORING mutates the driving GameLogic instance.
    fn scoring_enabled(&mut self, _enabled: bool) -> ScriptOwnerQuery<()> {
        ScriptOwnerQuery::Unavailable
    }

    fn eva_enabled(&mut self, _enabled: bool) -> ScriptOwnerQuery<()> {
        ScriptOwnerQuery::Unavailable
    }

    fn object_status(&self, _id: ObjectID) -> ScriptOwnerQuery<ScriptObjectStatus> {
        ScriptOwnerQuery::Unavailable
    }

    fn condition_team_roster(
        &self,
        _name: &str,
        _calling: Option<TeamID>,
        _contextual: Option<TeamID>,
    ) -> GameLogicResult<ScriptOwnerQuery<ScriptConditionTeam>> {
        Ok(ScriptOwnerQuery::Unavailable)
    }

    fn script_command_button(&self, _name: &str) -> ScriptOwnerQuery<ScriptCommandButton> {
        ScriptOwnerQuery::Unavailable
    }

    /// None means the member has no power of the requested type. Some reports
    /// whether the first module with the exact template is fully charged.
    fn script_command_button_power(&self, _member: ObjectID, _power: &str) -> Option<bool> {
        None
    }

    fn script_command_button_upgrade(&self, _member: ObjectID, _upgrade: &str) -> bool {
        false
    }

    fn script_last_damage_source(&self, _member: ObjectID) -> Option<ScriptDamageSource<'_>> {
        None
    }

    fn script_object_status_bits(&self, _member: ObjectID) -> Option<u64> {
        None
    }

    /// A missing object, held object, or undetected stealth cannot be discovered.
    fn script_discovery_candidate(&self, _member: ObjectID) -> bool {
        false
    }

    fn script_object_shroud(
        &self,
        _member: ObjectID,
        _viewer: u32,
    ) -> Option<crate::object::ObjectShroudStatus> {
        None
    }

    fn player_was_attacked(&self, _player: u32, _attacker: u32) -> bool {
        false
    }

    /// Ordered instances belonging to the driving session. Missing means an
    /// absent prototype; an empty Present list is authoritative too.
    fn team_instances(&self, _name: &str) -> ScriptOwnerQuery<Vec<TeamID>> {
        ScriptOwnerQuery::Unavailable
    }

    /// Re-query the live next instance only after synchronous effects have
    /// finished. None selects the initial head. A missing cursor is final;
    /// never restart at the head after destruction or owner reset.
    fn condition_team_after(
        &self,
        name: &str,
        after: Option<TeamID>,
    ) -> ScriptOwnerQuery<Option<TeamID>> {
        match self.team_instances(name) {
            ScriptOwnerQuery::Present(ids) => match after {
                None => ScriptOwnerQuery::Present(ids.first().copied()),
                Some(id) => match ids.iter().position(|live| *live == id) {
                    Some(index) => ScriptOwnerQuery::Present(ids.get(index + 1).copied()),
                    None => ScriptOwnerQuery::Missing,
                },
            },
            ScriptOwnerQuery::Missing => ScriptOwnerQuery::Missing,
            ScriptOwnerQuery::Unavailable => ScriptOwnerQuery::Unavailable,
        }
    }

    fn team_name(&self, _id: TeamID) -> ScriptOwnerQuery<String> {
        ScriptOwnerQuery::Unavailable
    }

    /// Current state of the receiving team's first/contextual instance.
    /// Missing is authoritative for an owner that has no matching team.
    fn team_state(
        &self,
        _name: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> ScriptOwnerQuery<String> {
        ScriptOwnerQuery::Unavailable
    }

    /// Mutate only the receiving team's first/contextual instance. `None`
    /// retains standalone Core behavior.
    fn set_team_state(
        &mut self,
        _name: &str,
        _state: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        None
    }

    /// The callback controller, including human players (unlike sequential AI).
    fn team_current_player(&self, _id: TeamID) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Unavailable
    }

    /// Resolve the exact Team and idle its group if a sequence is starting.
    /// The interpreter owns queue admission and removal.
    fn team_for_sequence(
        &mut self,
        _team: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
        _starting: bool,
    ) -> Option<GameLogicResult<Option<TeamID>>> {
        None
    }

    /// CPP evaluateTeamIsContained: literal relationships on the selected roster.
    fn team_contained(
        &self,
        _name: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
        _all: bool,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    fn set_named_train_held(
        &mut self,
        _name: &str,
        _held: bool,
        _this_object: Option<ObjectID>,
    ) -> Option<()> {
        None
    }

    fn set_named_cave_index(&mut self, _name: &str, _index: i32) -> Option<()> {
        None
    }

    fn team_presence(
        &self,
        _name: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> ScriptOwnerQuery<ScriptTeamPresence> {
        ScriptOwnerQuery::Unavailable
    }

    /// CPP HasUnits selects the contextual instance, otherwise all prototype instances.
    fn team_has_units(
        &self,
        _name: &str,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    fn team_status(&self, _id: TeamID) -> ScriptOwnerQuery<ScriptTeamStatus> {
        ScriptOwnerQuery::Unavailable
    }

    /// C++ Team::didAllEnter/didPartialEnter/didAllExit/didPartialExit. The
    /// receiving world owns team membership and trigger edge slots.
    fn team_area_transition(
        &mut self,
        _transition: ScriptTeamAreaTransition,
        _team: &str,
        _area: &str,
        _current_player: Option<&str>,
        _calling: Option<TeamID>,
        _condition: Option<TeamID>,
        _surfaces_allowed: u32,
    ) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Unavailable
    }

    /// The aggregate C++ terrain edge is checked before looking up a bridge.
    fn bridge_damage_states_changed(&self) -> Option<bool> {
        None
    }

    fn bridge_status(
        &self,
        _name: &str,
        _this_object: Option<ObjectID>,
    ) -> Option<ScriptBridgeStatus> {
        None
    }

    /// CPP ScriptEngine.cpp:7888–7910: select the controlling player only
    /// for a skirmish AI owner. Present(None) is an authoritative human or
    /// non-skirmish owner, not permission to discover another world's player.
    fn sequential_current_player(
        &self,
        _object_id: ObjectID,
        _team_id: Option<TeamID>,
    ) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Unavailable
    }
}
