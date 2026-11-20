//! RebuildHoleExposeDie - Creates rebuild hole on structure death
//!
//! Original C++ location: GameLogic/Module/RebuildHoleExposeDie.h/.cpp
//! Original C++ Author: Colin Day, June 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::ai::the_ai;
use crate::common::{AsciiString, Bool, ObjectID, ObjectStatusTypes, Real};
use crate::damage::DamageInfo;
use crate::helpers::TheThingFactory;
use crate::object::Object;
use crate::object::behavior::behavior_module::RebuildHoleBehaviorInterface;
use crate::object::behavior::rebuild_hole_behavior::RebuildHoleBehaviorModule;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::scripting::engine::transfer_object_name_to_object;
use crate::system::game_logic::get_game_logic;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for RebuildHoleExposeDie
/// (Matches C++ RebuildHoleExposeDieModuleData)
#[derive(Debug, Clone)]
pub struct RebuildHoleExposeDieModuleData {
    pub base: DieModuleData,
    /// Name of the hole object template to create
    pub hole_name: AsciiString,
    /// Maximum health of the rebuild hole
    pub hole_max_health: Real,
    /// Whether to transfer attackers to the hole
    pub transfer_attackers: Bool,
}

impl Default for RebuildHoleExposeDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            hole_name: AsciiString::default(),
            hole_max_health: 0.0,
            transfer_attackers: true,
        }
    }
}

impl Snapshotable for RebuildHoleExposeDieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData xfer version: {e:?}"))?;

        self.base.xfer(xfer)?;

        let mut name = self.hole_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData hole_name: {e:?}"))?;
        self.hole_name = AsciiString::from(name.as_str());

        let mut health = self.hole_max_health;
        xfer.xfer_real(&mut health)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData hole_max_health: {e:?}"))?;
        self.hole_max_health = health;

        let mut transfer = self.transfer_attackers;
        xfer.xfer_bool(&mut transfer)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData transfer_attackers: {e:?}"))?;
        self.transfer_attackers = transfer;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_via_base!(RebuildHoleExposeDieModuleData, base);

impl RebuildHoleExposeDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, REBUILD_HOLE_EXPOSE_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_hole_name(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.hole_name = AsciiString::from(*token);
    Ok(())
}

fn parse_hole_max_health(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.hole_max_health = INI::parse_real(token)?;
    Ok(())
}

fn parse_transfer_attackers(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.transfer_attackers = INI::parse_bool(token)?;
    Ok(())
}

const REBUILD_HOLE_EXPOSE_DIE_FIELDS: &[FieldParse<RebuildHoleExposeDieModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_die_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_die_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_die_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_die_required_status,
    },
    FieldParse {
        token: "HoleName",
        parse: parse_hole_name,
    },
    FieldParse {
        token: "HoleMaxHealth",
        parse: parse_hole_max_health,
    },
    FieldParse {
        token: "TransferAttackers",
        parse: parse_transfer_attackers,
    },
];

/// RebuildHoleExposeDie - Creates a rebuild hole when structure dies
///
/// When a structure with this module is destroyed, it creates a "rebuild hole"
/// in its place. The rebuild hole is a special object that:
/// - Appears as rubble/destroyed foundation
/// - Can automatically rebuild the structure over time
/// - Can be captured by other players
/// - Can be repaired to speed up rebuilding
///
/// This is used for tech buildings and other important structures that
/// should be capturable rather than permanently destroyed.
///
/// Optionally transfers attackers from the dying structure to the hole,
/// so units will continue attacking the hole instead of wandering off.
/// (Matches C++ RebuildHoleExposeDie)
#[derive(Debug)]
pub struct RebuildHoleExposeDie {
    base: DieModule<RebuildHoleExposeDieModuleData>,
}

impl RebuildHoleExposeDie {
    /// Create a new RebuildHoleExposeDie module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: super::ModuleDataHandle,
    ) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "RebuildHoleExposeDie"
    }

    /// Create the rebuild hole object
    fn create_hole(&self, dying_object: &mut Object) -> Option<Arc<RwLock<Object>>> {
        let module_data = self.base.module_data.read();
        let hole_name = &module_data.hole_name;

        let position = *dying_object.get_position();
        let geometry = dying_object.get_geometry_info().clone();
        let orientation = dying_object.get_orientation();

        let Some(template) = TheThingFactory::find_template(hole_name.as_str()) else {
            return None;
        };

        let factory = match TheThingFactory::get() {
            Ok(factory) => factory,
            Err(_) => return None,
        };

        let hole = factory.new_rebuild_hole(template, dying_object).ok()?;

        if let Ok(mut hole_guard) = hole.write() {
            let _ = hole_guard.set_position(&position);
            let _ = hole_guard.set_orientation(orientation);
            hole_guard.set_geometry_info(geometry);
            let _ = transfer_object_name_to_object(dying_object.get_name(), &mut hole_guard);
        }

        Some(hole)
    }

    /// RebuildHoleExposeDie.cpp:153–162 visits the receiving list newest first.
    fn transfer_attackers(&self, dying_object: &Object, hole_id: ObjectID) {
        if !self.base.module_data.read().transfer_attackers {
            return;
        }
        let Some(world) = dying_object.owning_world_services() else {
            return;
        };

        // Hole creation already checked that this is the parent's GameLogic.
        // Release the roster before entering AI, which can resolve its new target.
        let Ok(object_ids) = get_game_logic()
            .lock()
            .map(|logic| logic.get_all_object_ids().to_vec())
        else {
            return;
        };
        let old_id = dying_object.get_id();
        for object_id in object_ids {
            let ai = if object_id == old_id {
                dying_object.get_ai_update_interface()
            } else {
                world
                    .object_registry()
                    .with_object(object_id, Object::get_ai_update_interface)
                    .flatten()
            };
            if let Some(ai) = ai {
                if let Ok(mut ai) = ai.lock() {
                    ai.transfer_attack(old_id, hole_id);
                }
            }
        }
    }
}

impl DieModuleInterface for RebuildHoleExposeDie {
    /// Called when the structure dies - creates rebuild hole
    /// (Matches C++ RebuildHoleExposeDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.read().base.die_mux_data,
        ) {
            return;
        }

        let player_active = object.with_controlling_player(|player_guard| {
            player_guard.get_player_type() != crate::player::PlayerType::Neutral
                && player_guard.is_player_active()
        });
        if !player_active.unwrap_or(false) {
            return;
        }

        if object
            .get_status_bits()
            .test(ObjectStatusTypes::UnderConstruction)
        {
            return;
        }

        let Some(hole) = self.create_hole(object) else {
            return;
        };
        let Ok(hole) = hole.read() else {
            return;
        };
        let hole_id = hole.get_id();
        let pos = *hole.get_position();
        let ai_store = the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    pf.add_object_to_map(hole_id, &[pos], false);
                }
            }
        }

        if let Some(body) = hole.get_body_module() {
            if let Ok(mut body) = body.lock() {
                // RebuildHoleExposeDie.cpp sets the max after pathfinding.
                let hole_max_health = self.base.module_data.read().hole_max_health;
                let _ = body.set_max_health(
                    hole_max_health,
                    MaxHealthChangeType::SameCurrentHealth,
                );
            }
        }

        let rebuild = hole.find_module_by_name("RebuildHoleBehavior");
        // Starting the authored rebuild can change the hole's visible mask.
        drop(hole);
        if let Some(rebuild) = rebuild {
            rebuild.with_module(|module| {
                if let Some(rebuild) = module.as_any_mut().downcast_mut::<RebuildHoleBehaviorModule>() {
                    rebuild.behavior_mut().start_rebuild_process(
                        Arc::clone(object.get_template()),
                        object.get_id(),
                    );
                }
            });
        }

        self.transfer_attackers(object, hole_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebuild_hole_expose_die_module_data_default() {
        let data = RebuildHoleExposeDieModuleData::default();
        assert!(data.hole_name.is_empty());
        assert_eq!(data.hole_max_health, 0.0);
        assert_eq!(data.transfer_attackers, true);
    }

    #[test]
    fn test_rebuild_hole_expose_die_module_name() {
        assert_eq!(
            RebuildHoleExposeDie::get_module_name(),
            "RebuildHoleExposeDie"
        );
    }

    #[test]
    fn test_rebuild_hole_expose_die_with_config() {
        let mut data = RebuildHoleExposeDieModuleData::default();
        data.hole_name = AsciiString::from("RebuildHole_TechBuilding");
        data.hole_max_health = 100.0;
        data.transfer_attackers = true;

        assert_eq!(data.hole_name.as_str(), "RebuildHole_TechBuilding");
        assert_eq!(data.hole_max_health, 100.0);
        assert_eq!(data.transfer_attackers, true);
    }

    #[test]
    fn hole_max_health_uses_same_current_health() {
        // C++ RebuildHoleExposeDie.cpp:141 body->setMaxHealth(m_holeMaxHealth)
        // default MaxHealthChangeType is SAME_CURRENTHEALTH (BodyModule.h:152).
        assert_eq!(
            MaxHealthChangeType::default(),
            MaxHealthChangeType::SameCurrentHealth
        );
    }

    #[test]
    fn lethal_weapon_creates_the_mounted_rebuild_hole_once() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            concat!(module_path!(), "::lethal_weapon_creates_the_mounted_rebuild_hole_once")
                .strip_prefix("gamelogic::")
                .unwrap(),
            "GENERALS_REBUILD_HOLE_WEAPON_CHILD",
        ) {
            return;
        }

        lethal_rebuild_hole_case(false);
        lethal_rebuild_hole_case(true);
    }

    fn lethal_rebuild_hole_case(transfer_attackers: bool) {
        use crate::common::Coord3D;
        use crate::player::{Player, PlayerTemplate, PlayerType};
        use crate::scripting::engine::get_named_object_tracker;
        use crate::system::engine_stores::{new_for_world, with_active_stores};
        use crate::weapon::{WeaponStore, WeaponTemplate};
        use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
        use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};

        let world = new_for_world();
        let peer = new_for_world();
        let fixture = || {
            assert!(ensure_thing_factory_exists());
            if get_module_factory().unwrap().is_none() {
                init_module_factory().unwrap();
            }
            crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
            crate::initialize_weapon_store().unwrap();
            let fixture_ini = r#"
Object RebuildableVictim
    KindOf = IMMOBILE STRUCTURE
    Geometry = BOX
    GeometryMajorRadius = 35
    GeometryMinorRadius = 24
    GeometryHeight = 10
    Body = ActiveBody Health
        MaxHealth = 100
        InitialHealth = 100
    End
    Behavior = RebuildHoleExposeDie Expose
        HoleName = RebuildHoleFixture
        HoleMaxHealth = 30
        TransferAttackers = No
    End
End
Object RebuildHoleFixture
    KindOf = IMMOBILE STRUCTURE
    Geometry = BOX
    GeometryMajorRadius = 2
    GeometryMinorRadius = 2
    GeometryHeight = 1
    Body = ActiveBody Health
        MaxHealth = 5
        InitialHealth = 5
    End
    Behavior = RebuildHoleBehavior Rebuild
        WorkerObjectName = RebuildFirer
        WorkerRespawnDelay = 20000
    End
End
Object RebuildFirer
    KindOf = VEHICLE CAN_ATTACK
    Behavior = AIUpdateInterface Drive
    End
    Geometry = SPHERE
    GeometryMajorRadius = 3
    Body = ActiveBody Health
        MaxHealth = 100
        InitialHealth = 100
    End
End
"#;
            // An omitted TransferAttackers field selects the stock default.
            let overrides = fixture_ini.replace("        TransferAttackers = No\n", "");
            let fixture_ini = if transfer_attackers { &overrides } else { fixture_ini };
            assert_eq!(
                get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(fixture_ini),
                3
            );
            let mut playable = PlayerTemplate::new("RebuildPlayer".into());
            playable.playable = true;
            let player = Arc::new(RwLock::new(Player::new(0)));
            {
                let mut player = player.write().unwrap();
                player.init(Arc::new(playable));
                player.set_player_type(PlayerType::Human, false);
                assert!(player.is_player_active());
            }
            crate::player::player_list().write().unwrap().add_player(player);
            let team = {
                let factory = crate::team::get_team_factory();
                let mut factory = factory.lock().unwrap();
                let prototype = factory
                    .init_team_owned("RebuildTeam".into(), "".into(), false, None, Some(0))
                    .unwrap();
                factory.create_team_on_prototype_with_id(&prototype, 12).unwrap()
            };
            let spawn = |name, position| {
                let template = TheThingFactory::find_template(name).unwrap();
                let handle = TheThingFactory::get()
                    .unwrap()
                    .new_object_with_team_handle(template, team.clone())
                    .unwrap();
                handle.write().unwrap().set_position(&position).unwrap();
                handle
            };

            let source = spawn("RebuildFirer", Coord3D::new(105., 105., 0.));
            let target = spawn("RebuildableVictim", Coord3D::new(225., 105., 0.));
            {
                let mut object = target.write().unwrap();
                object.set_orientation(0.75).unwrap();
                transfer_object_name_to_object(&"RebuildSite".into(), &mut object).unwrap();
            }
            let ai = source.read().unwrap().get_ai_update_interface().unwrap();
            let target_id = target.read().unwrap().get_id();
            ai.lock().unwrap().set_current_victim(Some(target_id));
            (source, target, ai)
        };
        let (source, target, ai) = with_active_stores(&world, fixture);
        let (peer_source, peer_target, peer_ai) = with_active_stores(&peer, fixture);
        let source_id = source.read().unwrap().get_id();
        assert_eq!(peer_source.read().unwrap().get_id(), source_id);
        let target_id = target.read().unwrap().get_id();
        assert_eq!(peer_target.read().unwrap().get_id(), target_id);
        with_active_stores(&world, || {
            assert!(
                target.read().unwrap().find_module_by_name("RebuildHoleExposeDie").is_some()
            );
            let mut gun = WeaponTemplate::new("RebuildLethal".into());
            gun.attack_range = 1000.0;
            gun.primary_damage = 150.0;
            gun.weapon_speed = 99999.0;
            gun.min_weapon_speed = 99999.0;
            gun.damage_type = crate::damage::DamageType::Unresistable;
            gun.death_type = crate::damage::DeathType::Normal;
            let gun = Arc::new(gun);

            WeaponStore::create_and_fire_temp_weapon_at_target(&gun, source_id, target_id).unwrap();
            WeaponStore::create_and_fire_temp_weapon_at_target(&gun, source_id, target_id).unwrap();
            assert_eq!(target.read().unwrap().get_health(), 0.0);
            let holes: Vec<_> = world.services().object_registry().get_all_objects()
                .into_iter().filter(|actor| {
                    actor.read().unwrap().get_template().get_name().as_str() == "RebuildHoleFixture"
                }).collect();
            assert_eq!(holes.len(), 1);
            let hole = holes[0].read().unwrap();
            assert_eq!(hole.get_controlling_player_id(), Some(0));
            assert_eq!(*hole.get_position(), Coord3D::new(225.0, 105.0, 0.0));
            assert_eq!(hole.get_geometry_info().get_major_radius(), 35.0);
            assert_eq!(hole.get_geometry_info().get_minor_radius(), 24.0);
            assert_eq!(hole.get_health(), 5.0);
            assert_eq!(hole.get_max_health(), 30.0);
            hole.find_module_by_name("RebuildHoleBehavior")
                .expect("authored hole behavior")
                .with_module(|module| {
                    let rebuild = module.as_any().downcast_ref::<RebuildHoleBehaviorModule>()
                        .unwrap().behavior();
                    assert_eq!(rebuild.get_spawner_id(), target_id);
                    let template = rebuild.get_rebuild_template().expect("hole starts rebuilding");
                    assert_eq!(template.get_name().as_str(), "RebuildableVictim");
                });
            let hole_id = hole.get_id();
            let expected_victim = if transfer_attackers { hole_id } else { target_id };
            assert_eq!(ai.lock().unwrap().get_current_victim(), Some(expected_victim));
            assert_eq!(hole.get_orientation(), 0.75);
            assert_eq!(hole.get_name().as_str(), "RebuildSite");
            assert_eq!(get_named_object_tracker().get_object_id("RebuildSite").unwrap(), Some(hole_id));
            assert_eq!(hole.get_next_object_id(), Some(target_id));
            assert_eq!(hole.get_prev_object_id(), None);
            assert_eq!(target.read().unwrap().get_prev_object_id(), Some(hole_id));
            let logic = get_game_logic().lock().unwrap();
            assert_eq!(logic.get_first_object_id(), Some(hole_id));
            assert_eq!(logic.get_all_object_ids(), &[hole_id, target_id, source_id]);
        });
        assert_eq!(peer_target.read().unwrap().get_health(), 100.0);
        assert_eq!(peer_target.read().unwrap().get_prev_object_id(), None);
        assert_eq!(peer_ai.lock().unwrap().get_current_victim(), Some(target_id));
        with_active_stores(&peer, || {
            assert_eq!(get_game_logic().lock().unwrap().get_first_object_id(), Some(target_id));
            assert_eq!(
                get_named_object_tracker().get_object_id("RebuildSite").unwrap(),
                Some(target_id)
            );
        });
        assert_eq!(peer.services().object_registry().get_all_objects().len(), 2);
    }

}
