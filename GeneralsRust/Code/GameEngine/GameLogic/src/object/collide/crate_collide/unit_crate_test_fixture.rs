//! Exact player/team inputs and retirement for real unit-crate factory calls.

use crate::common::{AsciiString, Coord3D, ObjectID};
use crate::helpers::TheThingFactory;
use crate::object::Object;
use crate::object_manager::get_object_manager;
use crate::player::{Player, PlayerList, player_list};
use crate::system::game_logic::get_game_logic;
use crate::team::{Team, get_team_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, RwLock};

type Admission = (ObjectID, Arc<RwLock<Object>>);

pub(super) struct UnitCrateFixture {
    baseline: Vec<Admission>,
    previous_players: Option<PlayerList>,
    team: Arc<RwLock<Team>>,
    seed: [u32; 6],
    retired: bool,
}

impl UnitCrateFixture {
    pub(super) const TEMPLATE: &str = "OwnedUnitCrateFixtureInfantry";

    pub(super) fn admissions() -> Result<Vec<Admission>, String> {
        let owner = get_game_logic();
        let owner = owner.lock().map_err(|_| "unit-crate owner poisoned")?;
        owner
            .get_all_object_ids()
            .iter()
            .map(|id| {
                owner
                    .find_object_by_id(*id)
                    .map(|object| (*id, object))
                    .ok_or_else(|| format!("unit-crate admission {id} missing object"))
            })
            .collect()
    }

    fn require_live(objects: &[Admission]) -> Result<(), String> {
        for (id, object) in objects {
            if object
                .read()
                .map_err(|_| "unit-crate object poisoned")?
                .is_destroyed()
            {
                return Err(format!("foreign object {id} has pending destruction"));
            }
        }
        Ok(())
    }

    pub(super) fn new() -> Self {
        let baseline = Self::admissions().expect("unit-crate baseline admissions");
        Self::require_live(&baseline).expect("unit-crate must preserve foreign pending work");
        let seed = game_engine::common::random_value::get_game_logic_random_seed_state();
        let team_id = {
            let factory = get_team_factory();
            let factory = factory.lock().expect("fixture team factory read");
            (0x71C0_0000..0x71C0_0100).find(|id| factory.find_team_by_id(*id).is_none())
        }
        .expect("unused unit-crate fixture team ID");
        let team = Arc::new(RwLock::new(Team::new(
            AsciiString::from("OwnedUnitCrateFixtureTeam"),
            team_id,
        )));
        team.write().unwrap().set_controlling_player_id(Some(0));
        let player = Arc::new(RwLock::new(Player::new(0)));
        player
            .write()
            .unwrap()
            .set_default_team(Some(Arc::clone(&team)));
        let mut temporary_players = PlayerList::new();
        temporary_players.add_player(player);
        let previous_players =
            std::mem::replace(&mut *player_list().write().unwrap(), temporary_players);
        let fixture = Self {
            baseline,
            previous_players: Some(previous_players),
            team,
            seed,
            retired: false,
        };
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        let loaded = {
            let mut factory = get_thing_factory().unwrap();
            let factory = factory.as_mut().unwrap();
            if factory.find_template(Self::TEMPLATE, false).is_none() {
                Some(factory.load_ini_text(&format!(
                    "Object {}\n  KindOf = INFANTRY\n  Geometry = CYLINDER\n  GeometryMajorRadius = 1.0\n  GeometryMinorRadius = 1.0\n  GeometryHeight = 2.0\nEnd\n",
                    Self::TEMPLATE
                )))
            } else {
                None
            }
        };
        if let Some(loaded) = loaded {
            assert_eq!(loaded, 1);
        }
        fixture
    }

    pub(super) fn create_picker(&self) -> Arc<RwLock<Object>> {
        let factory = TheThingFactory::get().expect("unit-crate real factory");
        let template = TheThingFactory::find_template(Self::TEMPLATE)
            .expect("unit-crate authored infantry template");
        // CPP UnitCrateCollide.cpp:47–50 passes the actual default-team pointer.
        // Our team is deliberately unregistered; ID rediscovery loses it.
        let picker = factory
            .new_object_with_team_handle(template, Arc::clone(&self.team))
            .expect("unit-crate picker creation");
        let positioned = {
            let mut picker = picker.write().unwrap();
            picker
                .set_orientation(0.625)
                .and_then(|_| picker.set_position(&Coord3D::new(128.0, 128.0, 0.0)))
        };
        positioned.expect("unit-crate picker transform");
        self.require_owned(&picker)
            .expect("picker exact team identity");
        picker
    }

    fn require_owned(&self, object: &Arc<RwLock<Object>>) -> Result<(), String> {
        let (id, team, player, template) = {
            let object = object
                .read()
                .map_err(|_| "unit-crate owned object poisoned")?;
            (
                object.get_id(),
                object.get_team(),
                object.get_controlling_player_id(),
                object.get_template().get_name().as_str().to_owned(),
            )
        };
        if player != Some(0)
            || template != Self::TEMPLATE
            || !team
                .as_ref()
                .is_some_and(|team| Arc::ptr_eq(team, &self.team))
        {
            return Err(format!(
                "unit-crate object {id} lost exact team/template identity"
            ));
        }
        let default_team = player_list()
            .read()
            .map_err(|_| "unit-crate roster poisoned")?
            .get_player(0)
            .cloned()
            .and_then(|player| player.read().ok()?.get_default_team());
        if !default_team
            .as_ref()
            .is_some_and(|team| Arc::ptr_eq(team, &self.team))
        {
            return Err("unit-crate controlling player's default-team identity changed".to_owned());
        }
        Ok(())
    }

    pub(super) fn created(&self) -> Result<Vec<Admission>, String> {
        let current = Self::admissions()?;
        for (id, expected) in &self.baseline {
            if !current
                .iter()
                .any(|(live_id, live)| live_id == id && Arc::ptr_eq(live, expected))
            {
                return Err(format!("foreign baseline admission {id} disappeared"));
            }
        }
        let mut created = Vec::new();
        for (id, object) in current {
            if let Some((_, baseline)) = self.baseline.iter().find(|(old, _)| *old == id) {
                if !Arc::ptr_eq(baseline, &object) {
                    return Err(format!("foreign baseline identity {id} changed"));
                }
            } else {
                self.require_owned(&object)?;
                created.push((id, object));
            }
        }
        Ok(created)
    }

    pub(super) fn retire(&mut self) -> Result<(), String> {
        if self.retired {
            return Ok(());
        }
        let created = self.created()?;
        Self::require_live(&self.baseline)?;
        let owner = get_game_logic();
        let mut owner = owner.lock().map_err(|_| "unit-crate owner poisoned")?;
        let detached = {
            let manager = get_object_manager();
            let mut manager = manager.write().map_err(|_| "unit-crate manager poisoned")?;
            created
                .iter()
                .map(|(id, object)| {
                    manager
                        .detach_fixture_object_slot(*id, object, &owner)
                        .map(|slot| (*id, slot, Arc::clone(object)))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        // Neither object nor manager guards span canonical destruction hooks.
        for (id, _) in &created {
            owner.destroy_object(*id);
        }
        owner
            .process_destroy_list()
            .map_err(|error| error.to_string())?;
        {
            let manager = get_object_manager();
            let manager = manager.read().map_err(|_| "unit-crate manager poisoned")?;
            for (id, slot, object) in detached {
                manager.finish_fixture_object_slot_retirement(id, slot, &object, &owner)?;
            }
        }
        drop(owner);
        let current = Self::admissions()?;
        if current.len() != self.baseline.len() || !self.created()?.is_empty() {
            return Err("unit-crate retirement changed baseline admissions".to_owned());
        }
        Self::require_live(&current)?;
        self.retired = true;
        Ok(())
    }
}

impl Drop for UnitCrateFixture {
    fn drop(&mut self) {
        let unwinding = std::thread::panicking();
        let result = catch_unwind(AssertUnwindSafe(|| self.retire()));
        if let Some(previous) = self.previous_players.take() {
            *player_list()
                .write()
                .unwrap_or_else(|error| error.into_inner()) = previous;
        }
        crate::helpers::set_game_logic_random_seed(self.seed);
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) if unwinding => eprintln!("unit-crate unwind retirement: {error}"),
            Err(_) if unwinding => eprintln!("unit-crate retirement panicked during unwind"),
            Ok(Err(error)) => panic!("unit-crate retirement: {error}"),
            Err(error) => resume_unwind(error),
        }
    }
}
