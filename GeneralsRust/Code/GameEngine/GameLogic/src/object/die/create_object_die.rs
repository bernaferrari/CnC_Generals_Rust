//! CreateObjectDie - Spawns objects when the object dies
//!
//! Original C++ location: GameLogic/Module/CreateObjectDie.h/.cpp
//! Original C++ Author: Michael S. Booth, January 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::{Bool, INVALID_ID};
use crate::damage::{DamageInfo, DamageType};
use crate::helpers::TheObjectCreationListStore;
use crate::modules::{BodyModuleInterface, BodyModuleInterfaceExt};
use crate::object::Object;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for CreateObjectDie
/// (Matches C++ CreateObjectDieModuleData)
#[derive(Debug, Clone)]
pub struct CreateObjectDieModuleData {
    pub base: DieModuleData,
    /// Object creation list - names of objects to spawn
    pub ocl: Vec<String>,
    /// Whether to transfer health from the dying object to the created object
    pub transfer_previous_health: Bool,
}

impl Default for CreateObjectDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            ocl: Vec::new(),
            transfer_previous_health: false,
        }
    }
}

impl Snapshotable for CreateObjectDieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(CreateObjectDieModuleData, base);

impl CreateObjectDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CREATE_OBJECT_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_creation_list(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.ocl = vec![(*token).to_string()];
    Ok(())
}

fn parse_transfer_previous_health(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.transfer_previous_health = INI::parse_bool(token)?;
    Ok(())
}

const CREATE_OBJECT_DIE_FIELDS: &[FieldParse<CreateObjectDieModuleData>] = &[
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
        token: "CreationList",
        parse: parse_creation_list,
    },
    FieldParse {
        token: "TransferPreviousHealth",
        parse: parse_transfer_previous_health,
    },
];

/// CreateObjectDie - Creates new objects when this object dies
///
/// This module spawns new objects at the death location. It can be used for:
/// - Debris and wreckage
/// - Spawning pilots when vehicles are destroyed
/// - Creating resource crates
/// - Spawning replacement units
///
/// Optionally transfers the previous health state to the new object(s).
/// (Matches C++ CreateObjectDie)
#[derive(Debug)]
pub struct CreateObjectDie {
    base: DieModule<CreateObjectDieModuleData>,
}

impl CreateObjectDie {
    /// Create a new CreateObjectDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: super::ModuleDataHandle) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "CreateObjectDie"
    }

    /// Transfer health and damage state from old object to new object
    fn transfer_health(&self, old_object: &Object, new_object: &mut Object) {
        if !self.base.module_data.read().transfer_previous_health {
            return;
        }

        use crate::damage::DeathType;

        let Some(old_body) = old_object.get_body_module() else {
            return;
        };
        let Some(new_body) = new_object.get_body_module() else {
            return;
        };

        let Ok(old_body_guard) = old_body.lock() else {
            return;
        };
        let Ok(mut new_body_guard) = new_body.lock() else {
            return;
        };

        let subdual_damage = old_body_guard.get_current_subdual_damage_amount();
        if subdual_damage > 0.0 {
            let mut info = DamageInfo::with_simple(
                subdual_damage,
                INVALID_ID,
                DamageType::SubdualUnresistable,
                DeathType::Normal,
            );
            let _ = new_body_guard.attempt_damage(&mut info);
        }

        let last_damage_source = old_body_guard
            .get_last_damage_info()
            .map(|info| info.input.source_id)
            .unwrap_or(INVALID_ID);

        let damage_amount =
            (old_body_guard.get_max_health() - old_body_guard.get_previous_health()).max(0.0);
        if damage_amount > 0.0 {
            let mut info = DamageInfo::with_simple(
                damage_amount,
                last_damage_source,
                DamageType::Unresistable,
                DeathType::Normal,
            );
            let _ = new_body_guard.attempt_damage(&mut info);
        }
    }
}

impl DieModuleInterface for CreateObjectDie {
    /// Called when the object dies - creates new objects
    /// (Matches C++ CreateObjectDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.read().base.die_mux_data,
        ) {
            return;
        }

        let (ocl, transfer_previous_health) = {
            let data = self.base.module_data.read();
            let ocl = data.ocl.first()
                .and_then(|name| TheObjectCreationListStore::find_object_creation_list(name));
            (ocl, data.transfer_previous_health)
        };
        let Some(ocl) = ocl else {
            return;
        };
        let ctx = crate::object_creation_list::live_creation_context();
        let created = ocl.create_with_borrowed_primary(&ctx, object, damage_info.input.source_id);
        if transfer_previous_health && let Some(created) = created {
            let new_id = {
                let Ok(mut created) = created.write() else {
                    return;
                };
                self.transfer_health(object, &mut created);
                created.get_id()
            };
            // CreateObjectDie.cpp:113–121 visits the original receiving list once.
            super::transfer_attackers_with_borrowed_parent(object, new_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_object_die_module_data_default() {
        let data = CreateObjectDieModuleData::default();
        assert_eq!(data.ocl.len(), 0);
        assert_eq!(data.transfer_previous_health, false);
    }

    #[test]
    fn test_create_object_die_module_name() {
        assert_eq!(CreateObjectDie::get_module_name(), "CreateObjectDie");
    }

    #[test]
    fn test_create_object_die_with_objects() {
        let mut data = CreateObjectDieModuleData::default();
        data.ocl.push("DebrisSmall".to_string());
        data.ocl.push("DebrisLarge".to_string());
        assert_eq!(data.ocl.len(), 2);
    }

    #[test]
    fn lethal_fire_creates_authored_debris_in_order_with_a_borrowed_victim() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::lethal_fire_creates_authored_debris_in_order_with_a_borrowed_victim"
            ).strip_prefix("gamelogic::").unwrap(),
            "GENERALS_CREATE_OBJECT_DIE_CHILD",
        ) {
            return;
        }

        lethal_creation_case(false);
        lethal_creation_case(true);
    }

    fn lethal_creation_case(earlier_hole: bool) {
        use crate::common::Coord3D;
        use crate::helpers::TheThingFactory;
        use crate::player::{Player, PlayerTemplate, PlayerType};
        use crate::system::engine_stores::{new_for_world, with_active_stores};
        use crate::system::game_logic::get_game_logic;
        use crate::weapon::{WeaponStore, WeaponTemplate};
        use game_engine::common::thing::thing_factory::{
            ensure_thing_factory_exists, get_thing_factory,
        };

        let driving = new_for_world();
        let peer = new_for_world();
        let load = || {
            assert!(ensure_thing_factory_exists());
            game_engine::common::thing::module_factory::init_module_factory().unwrap();
            crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
            crate::initialize_weapon_store().unwrap();
            let hole = if earlier_hole {
                "    Behavior = RebuildHoleExposeDie Expose
        HoleName = FixtureHole
        HoleMaxHealth = 30
        TransferAttackers = No
    End
"
            } else {
                ""
            };
            let text = format!(r#"
Object OclVictim
    KindOf = IMMOBILE STRUCTURE
    Geometry = BOX
    GeometryMajorRadius = 20
    GeometryMinorRadius = 14
    Body = ActiveBody Health
        InitialHealth = 100
        MaxHealth = 100
    End
{hole}    Behavior = DestroyDie Destroy
    End
    Behavior = CreateObjectDie Debris
        RequiredStatus = DESTROYED
        CreationList = OCL_AuthoredDeathFixture
        TransferPreviousHealth = Yes
    End
End
Object OclFirer
    KindOf = VEHICLE CAN_ATTACK
    Geometry = SPHERE
    GeometryMajorRadius = 4
    Behavior = AIUpdateInterface AI
    End
    Body = ActiveBody Health
        InitialHealth = 100
        MaxHealth = 100
    End
End
Object ReplacementOcl
    KindOf = IMMOBILE
    Geometry = SPHERE
    GeometryMajorRadius = 4
    Body = ActiveBody Health
        InitialHealth = 100
        MaxHealth = 100
    End
End
Object GenericDebris
    KindOf = UNATTACKABLE
    Geometry = SPHERE
    GeometryMajorRadius = 2
    Body = ActiveBody Health
        InitialHealth = 100
        MaxHealth = 100
    End
End
Object FixtureHole
    KindOf = IMMOBILE STRUCTURE
    Geometry = BOX
    GeometryMajorRadius = 4
    GeometryMinorRadius = 4
    Body = ActiveBody Health
        InitialHealth = 5
        MaxHealth = 5
    End
End
"#);
            assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(&text), 5);
            let text = r#"
ObjectCreationList OCL_AuthoredDeathFixture
    CreateObject
        ObjectNames = ReplacementOcl
        Count = 1
        Offset = X:3 Y:-2 Z:0
    End
    CreateDebris
        ModelNames = FixtureDebris01
        Count = 1
        Offset = X:-2 Y:4 Z:0
    End
End
"#;
            assert_eq!(
                crate::object_creation_list::store::load_object_creation_lists_from_str(text)
                    .unwrap(), 1
            );
            let mut template = PlayerTemplate::new("OclPlayer".into());
            template.playable = true;
            let player = Arc::new(RwLock::new(Player::new(0)));
            {
                let mut player = player.write().unwrap();
                player.init(Arc::new(template));
                player.set_player_type(PlayerType::Human, false);
            }
            crate::player::player_list().write().unwrap().add_player(Arc::clone(&player));
            let team = {
                let factory = crate::team::get_team_factory();
                let mut factory = factory.lock().unwrap();
                let prototype = factory
                    .init_team_owned("OclTeam".into(), "".into(), false, None, Some(0))
                    .unwrap();
                factory.create_team_on_prototype_with_id(&prototype, 12).unwrap()
            };
            player.write().unwrap().set_default_team(Some(Arc::clone(&team)));
            let spawn = |name, position| {
                let template = TheThingFactory::find_template(name).unwrap();
                let actor = TheThingFactory::get()
                    .unwrap()
                    .new_object_with_team_handle(template, Arc::clone(&team))
                    .unwrap();
                actor.write().unwrap().set_position(&position).unwrap();
                actor
            };
            let source = spawn("OclFirer", Coord3D::new(105., 105., 0.));
            let victim = spawn("OclVictim", Coord3D::new(225., 105., 0.));
            let source_id = source.read().unwrap().get_id();
            let victim_id = victim.read().unwrap().get_id();
            let ai = source.read().unwrap().get_ai_update_interface().unwrap();
            ai.lock().unwrap().set_current_victim(Some(victim_id));
            assert!(victim.read().unwrap().find_module_by_name("CreateObjectDie").is_some());
            (source_id, victim_id, victim, ai)
        };
        let (source, victim_id, victim, ai) = with_active_stores(&driving, load);
        let (peer_source, peer_id, peer_victim, peer_ai) = with_active_stores(&peer, load);
        assert_eq!((source, victim_id), (peer_source, peer_id));

        with_active_stores(&driving, || {
            let fire = |amount| {
                let mut gun = WeaponTemplate::new("OclFixtureFire".into());
                gun.attack_range = 1000.0;
                gun.primary_damage = amount;
                gun.weapon_speed = 99999.0;
                gun.min_weapon_speed = 99999.0;
                gun.damage_type = DamageType::Unresistable;
                gun.death_type = crate::damage::DeathType::Normal;
                WeaponStore::create_and_fire_temp_weapon_at_target(
                    &Arc::new(gun), source, victim_id,
                ).unwrap();
            };
            fire(40.0);
            assert_eq!(victim.read().unwrap().get_health(), 60.0);
            fire(150.0);
            assert!(victim.read().unwrap().is_destroyed());
            let logic = get_game_logic().lock().unwrap();
            let mut ids = logic.get_all_object_ids().iter().copied();
            let debris_id = ids.next().unwrap();
            let replacement_id = ids.next().unwrap();
            let prior = if earlier_hole { ids.next().unwrap() } else { victim_id };
            assert_eq!(ids.collect::<Vec<_>>(), [victim_id, source]);
            let debris = logic.find_object_by_id(debris_id).unwrap();
            let replacement = logic.find_object_by_id(replacement_id).unwrap();
            let debris = debris.read().unwrap();
            let replacement = replacement.read().unwrap();
            assert_eq!(debris.get_template().get_name().as_str(), "GenericDebris");
            assert_eq!(replacement.get_template().get_name().as_str(), "ReplacementOcl");
            assert_eq!(debris.get_producer_id(), victim_id);
            assert_eq!(replacement.get_producer_id(), victim_id);
            assert_eq!(debris.get_next_object_id(), Some(replacement_id));
            assert_eq!(replacement.get_next_object_id(), Some(prior));
            assert_eq!(replacement.get_prev_object_id(), Some(debris_id));
            assert_eq!(victim.read().unwrap().get_prev_object_id(), Some(
                if earlier_hole { prior } else { replacement_id }
            ));
            assert_eq!(*debris.get_position(), Coord3D::new(223., 109., 0.));
            assert_eq!(*replacement.get_position(), Coord3D::new(228., 103., 0.));
            assert_eq!(debris.get_health(), 100.0);
            assert_eq!(replacement.get_health(), 60.0);
            assert_eq!(ai.lock().unwrap().get_current_victim(), Some(replacement_id));
        });
        assert_eq!(peer_victim.read().unwrap().get_health(), 100.0);
        assert_eq!(peer_ai.lock().unwrap().get_current_victim(), Some(peer_id));
        assert_eq!(peer.services().object_registry().get_all_objects().len(), 2);
    }
}
