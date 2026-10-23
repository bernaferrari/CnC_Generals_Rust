//! GameLogic.cpp1850–1860 / PlayerList.cpp262–275: validate originalOwner
//! before ThingFactory construction, retaining the exact receiving Team pointer.
use super::*;

/// Original map loop retains its selected Team across instance properties;
/// activation is not a second membership/Energy admission.
pub(in crate::game_logic) struct MapObjectBirth {
    pub(in crate::game_logic) id: ObjectId,
    team: std::sync::Arc<std::sync::RwLock<gamelogic::team::Team>>,
}
impl MapObjectBirth {
    pub(in crate::game_logic) fn activate(
        self,
        world: &GameLogic,
    ) -> gamelogic::GameLogicResult<()> {
        let id = self
            .team
            .read()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .get_id();
        let current = world
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .find_team_by_id(id);
        if current.is_none_or(|current| !std::sync::Arc::ptr_eq(&current, &self.team)) {
            return Err(gamelogic::GameLogicError::Configuration(
                "map Team was retired before activation".into(),
            ));
        }
        self.team
            .write()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .set_active();
        Ok(())
    }
}

impl GameLogic {
    pub(in crate::game_logic) fn create_placed_map_object_with_effects_checked(
        &mut self,
        placed: &crate::game_logic::script_loader::PlacedObject,
        position: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<MapObjectBirth>> {
        let template = placed.template.as_str();
        // Original prop/decorative/template misses do not run validateTeam.
        // Keep the existing map skip policy before any owned Team effects.
        if Self::should_skip_map_object_template(template)
            && !self
                .templates
                .keys()
                .any(|name| name.eq_ignore_ascii_case(template))
        {
            return Ok(None);
        }
        if !self.ensure_host_spawn_template(template) {
            return Ok(None);
        }
        if let Some(error) = self.team_creation_error() {
            return Err(error);
        }
        let invalid = |reason: &str| gamelogic::GameLogicError::Configuration(reason.into());
        let key =
            game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("originalOwner");
        if placed
            .properties
            .get_type(key)
            .is_some_and(|kind| kind != game_engine::common::dict::DictType::AsciiString)
        {
            return Err(invalid(
                "map originalOwner must be an authored ASCII Team name",
            ));
        }
        // The projected team_name/player_id are auxiliary parser conveniences.
        // Only the actual originalOwner Dict selects the original Team pointer.
        let raw = placed.properties.get_ascii_string(key);
        self.require_owned_team_creation_interpreter(&raw, false, None)?;
        let selected = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .find_team_with_owned_effects(&raw);
        let (team, actions) = match selected {
            Some(selected) => selected,
            None => {
                let mut neutral = self
                    .players
                    .values()
                    .filter(|player| player.is_reserved_neutral());
                let owner = neutral.next().map(|player| player.id).ok_or_else(|| {
                    invalid(
                        "map validateTeam fallback requires the receiving reserved neutral Player",
                    )
                })?;
                if neutral.next().is_some() {
                    return Err(invalid(
                        "map validateTeam has ambiguous receiving neutral Players",
                    ));
                }
                let identity = self.host_default_team_id(owner).ok_or_else(|| {
                    invalid("receiving neutral Player has no admitted default Team")
                })?;
                let team = self
                    .team_factory
                    .lock()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
                    .find_team_by_id(identity)
                    .ok_or_else(|| {
                        invalid("receiving neutral default Team identity disappeared")
                    })?;
                (team, Vec::new())
            }
        };
        let identity = team
            .read()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .get_id();
        // Implicit non-singleton creation runs its owned actions BEFORE the
        // incoming Object exists, just as validateTeam -> findTeam in C++.
        self.run_owned_team_creation_actions(&team, actions, None, effects)?;
        let current = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?
            .find_team_by_id(identity);
        if current.is_none_or(|current| !std::sync::Arc::ptr_eq(&current, &team)) {
            return Err(invalid(
                "map Team was retired during its creation callbacks",
            ));
        }
        let (name, owner) = {
            let team = team
                .read()
                .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
            (
                team.get_name().as_str().to_owned(),
                team.get_controlling_player_id(),
            )
        };
        let owner = owner
            .filter(|id| self.players.contains_key(id))
            .ok_or_else(|| invalid("map Team has no admitted receiving controller"))?;
        let id = self.create_object_on_exact_script_team_with_effects_checked(
            template, owner, &name, identity, position, false, None, effects,
        )?;
        Ok(id.map(|id| MapObjectBirth { id, team }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, PlayerSideRole};
    use game_engine::common::{dict::Dict, well_known_keys::*};

    fn fixture_world() -> GameLogic {
        let content = crate::assets::ReceivingObjectContent::from_sources([(
            "map-owner.ini".into(),
            "Object ChinaMapPlant\nKindOf = STRUCTURE\nEnergyProduction = 20\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject MapCallbackWitness\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
        )]).unwrap();
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(content).unwrap();
        let mut neutral = Player::new(0, Team::Neutral, "Neutral Label", false);
        neutral.map_side.role = PlayerSideRole::Neutral;
        neutral.map_side.map_player_name.clear();
        world.add_player(neutral);
        world.add_player(
            Player::new(1, Team::USA, "Same Display", true).with_script_identity("MapOne"),
        );
        world.add_player(
            Player::new(2, Team::USA, "Same Display", false).with_script_identity("MapTwo"),
        );
        world
    }
    fn placed(raw: &str) -> crate::game_logic::script_loader::PlacedObject {
        let mut properties = Dict::new();
        properties.set_ascii_string(
            game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("originalOwner"),
            raw,
        );
        crate::game_logic::script_loader::PlacedObject {
            template: "ChinaMapPlant".into(),
            name: None,
            position: gamelogic::common::Coord3D::new(0., 0., 0.),
            rotation: None,
            team_name: Some("teamMapOne".into()),
            player_id: Some(1),
            upgrade: None,
            unsellable: None,
            enabled: None,
            powered: None,
            indestructible: None,
            object_weather: None,
            properties,
        }
    }
    fn named(world: &mut GameLogic, scripted: bool) {
        let mut dict = Dict::new();
        dict.set_bool(key_team_executes_actions_on_create(), scripted);
        dict.set_ascii_string(key_team_production_condition(), "MapTeamCreate");
        let (_, effects) = world
            .team_factory
            .lock()
            .unwrap()
            .init_team_owned_with_effects(
                "ExactMapTeam".into(),
                "MapTwo".into(),
                false,
                Some(&dict),
                Some(2),
            )
            .unwrap();
        assert!(effects.is_empty());
    }
    #[test]
    fn authored_map_team_overrides_auxiliary_owner_and_template_faction_without_double_energy() {
        let mut world = fixture_world();
        let peer = fixture_world();
        named(&mut world, false);
        let birth = world
            .create_placed_map_object_with_effects_checked(
                &placed("ExactMapTeam"),
                Vec3::ZERO,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let id = birth.id;
        let actor = world.host_object(id).unwrap();
        assert_eq!(actor.owner_player_id, Some(2));
        assert_eq!(actor.team, Team::USA);
        assert_eq!(actor.team_instance_name, "ExactMapTeam");
        let team = world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(actor.team_instance_id.unwrap())
            .unwrap();
        assert_eq!(team.read().unwrap().get_controlling_player_id(), Some(2));
        assert_eq!(team.read().unwrap().get_members(), [id.0]);
        assert!(
            !team.read().unwrap().is_active(),
            "map Team waits for instance properties before activation"
        );
        birth.activate(&world).unwrap();
        assert!(team.read().unwrap().is_active());
        assert_eq!(world.get_player(1).unwrap().power_produced, 0);
        assert_eq!(world.get_player(2).unwrap().power_produced, 20);
        assert_eq!(world.get_player(0).unwrap().power_produced, 0);
        assert!(peer.host_object(id).is_none());
        assert_eq!(peer.get_player(2).unwrap().power_produced, 0);
    }
    #[test]
    fn unknown_map_team_falls_back_to_actual_reserved_neutral_not_auxiliary_player() {
        let mut world = fixture_world();
        let peer = fixture_world();
        let neutral_team = world.host_default_team_id(0).unwrap();
        let id = world
            .create_placed_map_object_with_effects_checked(
                &placed("UnknownAuthoredTeam"),
                Vec3::ZERO,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let id = id.id;
        let actor = world.host_object(id).unwrap();
        assert_eq!(actor.owner_player_id, Some(0));
        assert_eq!(actor.team, Team::Neutral);
        assert_eq!(actor.team_instance_name, "team");
        assert_eq!(actor.team_instance_id, Some(neutral_team));
        assert_eq!(world.get_player(0).unwrap().power_produced, 20);
        for id in [1, 2] {
            assert_eq!(world.get_player(id).unwrap().power_produced, 0);
        }
        assert!(
            world
                .team_factory
                .lock()
                .unwrap()
                .find_team_prototype("UnknownAuthoredTeam")
                .is_none()
        );
        assert_eq!(peer.get_player(0).unwrap().power_produced, 0);
    }
    #[test]
    fn implicit_map_team_creation_actions_finish_before_incoming_factory_birth() {
        use gamelogic::scripting::core::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
            ScriptActionType, ScriptList,
        };
        let mut world = fixture_world();
        let peer = fixture_world();
        named(&mut world, true);
        let mut increment = ScriptAction::new(ScriptActionType::IncrementCounter);
        increment
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        increment
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "MapTeamCreateCount".into(),
            ))
            .unwrap();
        let mut create = ScriptAction::new(ScriptActionType::CreateObject);
        create
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                "MapCallbackWitness".into(),
            ))
            .unwrap();
        create
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "ExactMapTeam".into(),
            ))
            .unwrap();
        create
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::common::Coord3D::new(77., 0., 0.),
            ))
            .unwrap();
        create
            .add_parameter(Parameter::with_real(ParameterType::Real, 0.))
            .unwrap();
        increment.next_action = Some(Box::new(create));
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        let mut script = Script::new();
        script.script_name = "MapTeamCreate".into();
        script.is_subroutine = true;
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(increment));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
        engine.set_counter("MapTeamCreateCount", 0).unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        world.install_owned_script_engine(engine);
        let parent = world
            .create_placed_map_object_with_effects_checked(
                &placed("ExactMapTeam"),
                Vec3::ZERO,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let parent = parent.id;
        let child = world
            .host_objects()
            .values()
            .find(|actor| actor.thing().template.name == "MapCallbackWitness")
            .unwrap();
        assert!(
            child.id.0 < parent.0,
            "actual nested constructor completes before incoming Object allocates its ID"
        );
        assert_eq!(child.owner_player_id, Some(2));
        assert_eq!(
            child.team_instance_id,
            world.host_object(parent).unwrap().team_instance_id
        );
        assert_eq!(world.get_player(2).unwrap().power_produced, 20);
        assert_eq!(world.get_player(1).unwrap().power_produced, 0);
        assert_eq!(
            world.with_owned_script_engine(|engine, _| engine
                .get_counter("MapTeamCreateCount")
                .unwrap()
                .value),
            Some(1)
        );
        assert!(peer.host_objects().is_empty());
        assert_eq!(peer.get_player(2).unwrap().power_produced, 0);
    }
}
