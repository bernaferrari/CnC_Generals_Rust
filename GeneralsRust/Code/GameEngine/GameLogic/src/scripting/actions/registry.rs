//! Leftover Rhai/name ActionRegistry (hq-8ta4n).
//!
//! C++ `executeAction` is `scripting/executor/dispatch.rs`. This HashMap
//! registry is leftover-only and must not run as a second action brain.

use super::ScriptAction;
use super::building::*;
use super::camera_ui::*;
use super::leftover::*;
use super::music_audio::*;
use super::named_unit::*;
use super::object_actions::*;
use super::player_command::*;
use super::player_economy::*;
use super::science_special::*;
use super::team_command::*;
use super::unit_actions::*;
use super::weather_radar::*;

use std::collections::HashMap;

/// Action registry for managing script actions
pub struct ActionRegistry {
    actions: HashMap<String, Box<dyn ScriptAction>>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            actions: HashMap::new(),
        };

        // Register built-in actions
        registry.register_builtin_actions();
        registry
    }

    /// Register built-in actions
    fn register_builtin_actions(&mut self) {
        // Unit and object actions
        self.register_action(Box::new(CreateUnitAction));
        self.register_action(Box::new(DestroyObjectAction));
        self.register_action(Box::new(MoveUnitAction));
        self.register_action(Box::new(AttackUnitAction));
        self.register_action(Box::new(SetObjectHealthAction));
        self.register_action(Box::new(SetObjectExperienceAction));

        // Team actions (15 critical actions)
        self.register_action(Box::new(TeamAttackTeamAction));
        self.register_action(Box::new(TeamFollowWaypointsAction));
        self.register_action(Box::new(TeamGuardAction));
        self.register_action(Box::new(TeamHuntAction));
        self.register_action(Box::new(TeamMoveToWaypointAction));
        self.register_action(Box::new(TeamGarrisonBuildingAction));
        self.register_action(Box::new(TeamExitBuildingAction));
        self.register_action(Box::new(TeamCaptureBuildingAction));
        self.register_action(Box::new(TeamRepairAction));
        self.register_action(Box::new(TeamWanderAction));
        self.register_action(Box::new(TeamIdleAction));
        self.register_action(Box::new(TeamSetStateAction));
        self.register_action(Box::new(TeamDeleteAction));
        self.register_action(Box::new(TeamFollowTeamAction));
        self.register_action(Box::new(TeamGuardInTunnelAction));

        // Named unit actions (10 critical actions)
        self.register_action(Box::new(NamedAttackAction));
        self.register_action(Box::new(NamedAttackAreaAction));
        self.register_action(Box::new(NamedAttackTeamAction));
        self.register_action(Box::new(NamedMoveToAction));
        self.register_action(Box::new(NamedGarrisonAction));
        self.register_action(Box::new(NamedFollowWaypointsAction));
        self.register_action(Box::new(NamedGuardAction));
        self.register_action(Box::new(NamedHuntAction));
        self.register_action(Box::new(NamedDeleteAction));
        self.register_action(Box::new(NamedEnterNamedAction));
        self.register_action(Box::new(NamedExitAction));
        self.register_action(Box::new(NamedSetAttitudeAction));

        // Player actions (10 critical actions)
        self.register_action(Box::new(PlayerGrantScienceAction));
        self.register_action(Box::new(PlayerDisableFactoriesAction));
        self.register_action(Box::new(PlayerEnableFactoriesAction));
        self.register_action(Box::new(PlayerBuildBaseDefenseAction));
        self.register_action(Box::new(PlayerHuntAction));
        self.register_action(Box::new(PlayerGarrisonAllBuildingsAction));
        self.register_action(Box::new(PlayerSellBuildingAction));
        self.register_action(Box::new(PlayerEvacuateBuildingAction));
        self.register_action(Box::new(PlayerSetActiveAction));
        self.register_action(Box::new(PlayerAddMoneyAction));

        // Original player and team actions
        self.register_action(Box::new(SetPlayerResourceAction));
        self.register_action(Box::new(AddPlayerResourceAction));
        self.register_action(Box::new(SetPlayerRelationAction));
        self.register_action(Box::new(DefeatPlayerAction));

        // Map/Camera actions (8 critical actions)
        self.register_action(Box::new(MapRevealAreaAction));
        self.register_action(Box::new(MapShroudAreaAction));
        self.register_action(Box::new(CameraMoveToWaypointAction));
        self.register_action(Box::new(CameraTrackNamedAction));
        self.register_action(Box::new(CameraLetterboxBeginAction));
        self.register_action(Box::new(CameraLetterboxEndAction));
        self.register_action(Box::new(CameraSetFinalZoomAction));
        self.register_action(Box::new(WeatherSetAction));

        // Audio/Visual actions (7 critical actions)
        self.register_action(Box::new(SoundPlayAction));
        self.register_action(Box::new(MusicPlayAction));
        self.register_action(Box::new(MoviePlayAction));
        self.register_action(Box::new(TextDisplayAction));
        self.register_action(Box::new(SpeechPlayAction));
        self.register_action(Box::new(RadarCreateEventAction));
        self.register_action(Box::new(ObjectCreateRadarEventAction));
        self.register_action(Box::new(TeamCreateRadarEventAction));
        self.register_action(Box::new(RadarEnableAction));
        self.register_action(Box::new(RadarDisableAction));
        self.register_action(Box::new(RadarForceEnableAction));
        self.register_action(Box::new(RadarRevertToNormalAction));

        // Original camera and UI actions
        self.register_action(Box::new(MoveCameraAction));
        self.register_action(Box::new(ShowTextMessageAction));
        self.register_action(Box::new(PlaySoundAction));
        self.register_action(Box::new(PlayMusicAction));

        // Map and environment actions
        self.register_action(Box::new(RevealMapAreaAction));
        self.register_action(Box::new(ShroudMapAreaAction));
        self.register_action(Box::new(SetWeatherAction));
        self.register_action(Box::new(SetTimeOfDayAction));

        // Special abilities and powers
        self.register_action(Box::new(TriggerSpecialPowerAction));
        self.register_action(Box::new(EnableSpecialPowerAction));
        self.register_action(Box::new(DisableSpecialPowerAction));

        // Technology and upgrades
        self.register_action(Box::new(GrantUpgradeAction));
        self.register_action(Box::new(EnableScienceAction));
        self.register_action(Box::new(DisableScienceAction));

        // Scripting control actions (C++ ScriptEngine enable/disable/execute).
        // Invented leftover WaitAction is leftover-only — C++ sequential waits
        // live in scripting/executor (hq-8ta4n).
        self.register_action(Box::new(EnableScriptAction));
        self.register_action(Box::new(DisableScriptAction));
        self.register_action(Box::new(ExecuteScriptAction));
        self.register_action(Box::new(SetVariableAction));

        // 20 Core Actions - Priority 1 Implementation
        self.register_action(Box::new(VictoryAction));
        self.register_action(Box::new(DefeatAction));
        self.register_action(Box::new(StartTimerAction));
        self.register_action(Box::new(StopTimerAction));
        self.register_action(Box::new(CreateBuildingAction));
        self.register_action(Box::new(DestroyBuildingAction));
        self.register_action(Box::new(SetTeamAllianceAction));
        self.register_action(Box::new(GiveSpecialPowerAction));
        self.register_action(Box::new(RevealAreaAction));
        self.register_action(Box::new(CreateExplosionAction));
        self.register_action(Box::new(SpawnReinforcementsAction));
        self.register_action(Box::new(CameraZoomAction));

        // High-Priority Missing Actions - Ported from C++
        self.register_action(Box::new(GiveMoneyAction));
        self.register_action(Box::new(SetMoneyAction));
        self.register_action(Box::new(SetHandicapAction));
        self.register_action(Box::new(DamageObjectAction));
        self.register_action(Box::new(KillObjectAction));
        self.register_action(Box::new(HealObjectAction));
        self.register_action(Box::new(RevealMapEntireAction));
        self.register_action(Box::new(ShroudMapEntireAction));
        self.register_action(Box::new(SnapCameraAction));
        self.register_action(Box::new(LetterBoxBeginAction));
        self.register_action(Box::new(LetterBoxEndAction));
        self.register_action(Box::new(TeamAttackAction));
        self.register_action(Box::new(TeamAttackAreaAction));
        self.register_action(Box::new(TeamGuardAreaAction));
        self.register_action(Box::new(TeamFollowAction));
        self.register_action(Box::new(SetTimerAction));
        self.register_action(Box::new(CountdownTimerAction));
        self.register_action(Box::new(PlaySoundAtAction));
        self.register_action(Box::new(StopMusicAction));
    }

    /// Register an action
    pub fn register_action(&mut self, action: Box<dyn ScriptAction>) {
        self.actions.insert(action.name().to_string(), action);
    }

    /// Get action by name
    pub fn get_action(&self, name: &str) -> Option<&dyn ScriptAction> {
        self.actions.get(name).map(|action| action.as_ref())
    }

    /// List all available actions
    pub fn list_actions(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }
}
