use super::*;
use crate::helpers::TheThingFactory;
use crate::modules::{PhysicsBehaviorExt, UpdateModulePtr};
use crate::object::PhysicsInterfaceHandle;
use crate::object::collide::{CollideModule, world_collision_manager};
use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
use crate::test_process::{TestProcess, run_bounded};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::sync::Mutex;

type ContactEvents = Arc<Mutex<Vec<(ObjectId, ObjectId, ObjectId)>>>;

struct ContactObserver {
    owner: ObjectId,
    physics: Option<PhysicsInterfaceHandle>,
    events: ContactEvents,
}

impl CollideModule for ContactObserver {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _: &Coord3D,
        _: &Coord3D,
    ) -> Result<(), CollisionError> {
        let last_collidee = self
            .physics
            .as_ref()
            .map_or(crate::common::INVALID_ID, |physics| {
                physics.get_last_collidee()
            });
        self.events
            .lock()
            .unwrap()
            .push((self.owner, other.unwrap().get_id(), last_collidee));
        Ok(())
    }

    fn would_like_to_collide_with(&self, _: &dyn GameObject) -> bool {
        true
    }
}

struct ContactWorld {
    world: Arc<EngineStores>,
    mover: Arc<RwLock<Object>>,
    mine: Arc<RwLock<Object>>,
    physics: PhysicsInterfaceHandle,
    update: UpdateModulePtr,
    events: ContactEvents,
}

fn contact_world(force: bool, dozer: bool, mine_x: f32) -> ContactWorld {
    let world = new_for_world();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mover, mine, physics, update) = with_active_stores(&world, || {
        assert!(ensure_thing_factory_exists());
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        let allow = if force { "Yes" } else { "No" };
        let kind = if dozer { "VEHICLE DOZER" } else { "SELECTABLE" };
        let ai = if dozer {
            "Behavior = DozerAIUpdate Dock\nEnd"
        } else {
            ""
        };
        let crushing = if dozer { "CrusherLevel = 2\nCrushableLevel = 2" } else { "" };
        let source = format!(
            "Object ContactMover
                KindOf = {kind}
                {crushing}
                Geometry = CYLINDER
                GeometryMajorRadius = 5
                GeometryHeight = 3
                Body = ActiveBody Health
                    MaxHealth = 100
                    InitialHealth = 100
                End
                Behavior = PhysicsBehavior Drive
                    Mass = 10
                    AllowCollideForce = {allow}
                End
                {ai}
            End
            Object ContactMine
                KindOf = STICK_TO_TERRAIN_SLOPE MINE IMMOBILE NO_HEAL_ICON
                Geometry = CYLINDER
                GeometryMajorRadius = 4
                GeometryHeight = 3
                Body = ActiveBody Health
                    MaxHealth = 100
                    InitialHealth = 100
                End
                Behavior = MinefieldBehavior Mines
                    NumVirtualMines = 8
                    DetonatedBy = ENEMIES
                End
            End"
        );
        assert_eq!(
            get_thing_factory()
                .unwrap()
                .as_mut()
                .unwrap()
                .load_ini_text(&source),
            2
        );
        let factory = TheThingFactory::get().unwrap();
        let team = Arc::new(RwLock::new(crate::team::Team::new("Contact".into(), 42)));
        let spawn = |name, x| {
            let object = factory
                .new_object_with_team_handle(
                    TheThingFactory::find_template(name).unwrap(),
                    Arc::clone(&team),
                )
                .unwrap();
            object
                .write()
                .unwrap()
                .set_position(&GameCoord3D::new(x, 105., 0.))
                .unwrap();
            object
        };
        let mover = spawn("ContactMover", 105.);
        let mine = spawn("ContactMine", mine_x);
        let (id, physics, update) = {
            let owner = mover.read().unwrap();
            assert_eq!(owner.get_ai().is_some(), dozer);
            let physics = owner.get_physics().unwrap();
            let update = physics.installed_update_proxy(&owner).unwrap();
            (owner.get_id(), physics, update)
        };
        let mut logic = world.game_logic_mutex().lock().unwrap();
        logic.set_current_frame(99);
        physics.set_velocity(&GameCoord3D::new(0.5, 0., 0.));
        logic.register_sleepy_update_module(id, Arc::clone(&update), 150);
        let mine_id = mine.read().unwrap().get_id();
        for (owner, physics) in [(id, Some(physics.clone())), (mine_id, None)] {
            world_collision_manager()
                .register_collide_module(
                    owner,
                    Box::new(ContactObserver {
                        owner,
                        physics,
                        events: Arc::clone(&events),
                    }),
                )
                .unwrap();
        }
        (mover, mine, physics, update)
    });
    ContactWorld {
        world,
        mover,
        mine,
        physics,
        update,
        events,
    }
}

#[test]
fn ordinary_mounted_contact_wakes_only_the_actor_that_receives_original_force() {
    const NAME: &str = concat!(
        module_path!(),
        "::ordinary_mounted_contact_wakes_only_the_actor_that_receives_original_force"
    );
    if let TestProcess::ParentVerified = run_bounded(
        NAME.strip_prefix("gamelogic::").unwrap_or(NAME),
        "GENERALS_MOUNTED_CONTACT_CHILD",
    ) {
        return;
    }

    for (allow_force, dozer) in [(true, false), (false, false), (true, true)] {
        let local = contact_world(allow_force, dozer, 112.);
        let peer = contact_world(allow_force, dozer, 212.);
        let mover = local.mover.read().unwrap().get_id();
        let mine = local.mine.read().unwrap().get_id();
        let invalid = crate::common::INVALID_ID;
        assert_eq!(mover, peer.mover.read().unwrap().get_id());
        assert_eq!(mine, peer.mine.read().unwrap().get_id());

        with_active_stores(&peer.world, || {
            local
                .world
                .game_logic_mutex()
                .lock()
                .unwrap()
                .update(100, || None)
                .unwrap();
        });
        let starting_position = GameCoord3D::new(105., 105., 0.);
        assert_eq!(
            *local.mover.read().unwrap().get_position(),
            starting_position
        );
        assert_eq!(
            *local.mine.read().unwrap().get_position(),
            GameCoord3D::new(112., 105., 0.)
        );
        let last = if dozer { invalid } else { mine };
        assert_eq!(local.physics.get_last_collidee(), last);
        assert_eq!(
            *local.events.lock().unwrap(),
            [(mover, mine, last), (mine, mover, invalid)]
        );

        let expect_force = allow_force && !dozer;
        let velocity = if expect_force {
            GameCoord3D::ZERO
        } else {
            GameCoord3D::new(0.5, 0., 0.)
        };
        assert_eq!(local.physics.get_velocity(), velocity);
        with_active_stores(&local.world, || {
            let mut logic = local.world.game_logic_mutex().lock().unwrap();
            assert!(logic.next_owned_scheduled_update(100).unwrap().is_none());
            let due = logic.next_owned_scheduled_update(101).unwrap();
            if expect_force {
                let due = due.unwrap();
                assert_eq!(due.object_id, mover);
                assert!(Arc::ptr_eq(&due.module, &local.update));
                assert_eq!(due.phase, crate::modules::SleepyUpdatePhase::Physics);
                assert!(due.should_process);
            } else {
                assert!(due.is_none());
            }
            logic.update(101, || None).unwrap();
        });
        let position = *local.mover.read().unwrap().get_position();
        let applied = local.physics.access().unwrap().get_acceleration();
        if expect_force {
            assert_eq!(applied, GameCoord3D::new(-0.25, 0., 0.));
            assert_eq!(position, GameCoord3D::new(104.75, 105., 0.));
        } else {
            assert_eq!(applied, GameCoord3D::ZERO);
            assert_eq!(position, starting_position);
        }
        assert_eq!(
            *peer.mover.read().unwrap().get_position(),
            starting_position
        );
        assert_eq!(
            *peer.mine.read().unwrap().get_position(),
            GameCoord3D::new(212., 105., 0.)
        );
        assert_eq!(peer.physics.get_last_collidee(), invalid);
        assert!(peer.events.lock().unwrap().is_empty());
        assert_eq!(
            peer.world.game_logic_mutex().lock().unwrap().get_frame(),
            99
        );
    }
}
