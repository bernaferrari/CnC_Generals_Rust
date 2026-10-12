//! Object.cpp transfer phases on the actual receiving factory.
//! The Object loan ends at setOrRestoreTeam; no active world is published.
use super::*;
use crate::object::object_factory::ObjectFactory;
use crate::player::{PlayerList, PowerBrownoutServices};
use crate::team::TeamFactory;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

/// One original Object block after GameLogic's TOC has selected its template.
/// This is not a second wire format: bytes are the existing Object serializer.
pub(crate) struct ObjectSnapshotRecord {
    pub template_name: String,
    pub bytes: Vec<u8>,
}

/// Stable immutable WeaponTemplateSet identities built once by the receiving
/// session. Operation frames never rebuild these definition adapters.
pub(crate) struct ObjectWeaponSetCatalog {
    things: std::collections::HashMap<String, Arc<dyn ThingTemplate>>,
    weapons: std::collections::HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
    sets: std::collections::HashMap<String, Vec<Arc<crate::weapon::WeaponTemplateSet>>>,
}
impl ObjectWeaponSetCatalog {
    pub(crate) fn from_definitions(
        things: &std::collections::HashMap<String, Arc<dyn ThingTemplate>>,
        weapons: &std::collections::HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
    ) -> Result<Self, String> {
        let mut weapon_sets = std::collections::HashMap::new();
        for (name, thing) in things {
            let mut sets = Vec::new();
            for authored in thing.weapon_template_sets() {
                for slot in 0..WEAPONSLOT_COUNT {
                    if let Some(name) = authored
                        .weapon_template_name(slot)
                        .filter(|name| !name.is_empty())
                    {
                        if !weapons.contains_key(name) {
                            return Err(format!("receiving weapon definition {name} absent"));
                        }
                    }
                }
                let set = crate::weapon::WeaponTemplateSet::from_engine_set(authored, |name| {
                    weapons.get(name.as_str()).cloned()
                })
                .map_err(|error| error.to_string())?;
                sets.push(Arc::new(set));
            }
            weapon_sets.insert(name.clone(), sets);
        }
        Ok(Self {
            things: things.clone(),
            weapons: weapons.clone(),
            sets: weapon_sets,
        })
    }
    fn validate_owners(
        &self,
        things: &std::collections::HashMap<String, Arc<dyn ThingTemplate>>,
        weapons: &std::collections::HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
    ) -> Result<(), String> {
        if self.things.len() != things.len()
            || self.weapons.len() != weapons.len()
            || self.things.iter().any(|(name, template)| {
                !things
                    .get(name)
                    .is_some_and(|actual| Arc::ptr_eq(actual, template))
            })
            || self.weapons.iter().any(|(name, template)| {
                !weapons
                    .get(name)
                    .is_some_and(|actual| Arc::ptr_eq(actual, template))
            })
        {
            return Err("Object weapon definitions belong to another catalog owner".into());
        }
        Ok(())
    }
}

/// Immutable definitions selected by the receiving session. Named WeaponSet
/// lookup uses the named ThingTemplate, not the receiver's previous set list.
pub(crate) struct ObjectXferDefinitions<'a> {
    things: &'a std::collections::HashMap<String, Arc<dyn ThingTemplate>>,
    weapons: &'a std::collections::HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
    locomotors: &'a std::collections::HashMap<String, Arc<crate::locomotor::LocomotorTemplate>>,
    weapon_sets: Arc<ObjectWeaponSetCatalog>,
    pub upgrades: &'a game_engine::common::system::upgrade::UpgradeCenter,
    pub weapon_bonuses: &'a crate::weapon::WeaponBonusSet,
    pub body_definitions: Arc<crate::object::body::active_body::BodyDefinitionCatalog>,
    module_factory: &'a game_engine::common::thing::module_factory::ModuleFactory,
    pub frame: u32,
}

impl<'a> ObjectXferDefinitions<'a> {
    pub(crate) fn with_catalog(
        things: &'a std::collections::HashMap<String, Arc<dyn ThingTemplate>>,
        weapons: &'a std::collections::HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
        locomotors: &'a std::collections::HashMap<String, Arc<crate::locomotor::LocomotorTemplate>>,
        upgrades: &'a game_engine::common::system::upgrade::UpgradeCenter,
        weapon_bonuses: &'a crate::weapon::WeaponBonusSet,
        body_definitions: Arc<crate::object::body::active_body::BodyDefinitionCatalog>,
        weapon_sets: Arc<ObjectWeaponSetCatalog>,
        module_factory: &'a game_engine::common::thing::module_factory::ModuleFactory,
        frame: u32,
    ) -> Result<Self, String> {
        weapon_sets.validate_owners(things, weapons)?;
        Ok(Self {
            things,
            weapons,
            locomotors,
            weapon_sets,
            upgrades,
            weapon_bonuses,
            body_definitions,
            module_factory,
            frame,
        })
    }

    pub(super) fn thing(&self, name: &str) -> Option<Arc<dyn ThingTemplate>> {
        self.things.get(name).cloned()
    }
    pub(super) fn weapon(&self, name: &str) -> Option<Arc<crate::weapon::WeaponTemplate>> {
        self.weapons.get(name).cloned()
    }
    pub(super) fn locomotor(&self, name: &str) -> Option<Arc<crate::locomotor::LocomotorTemplate>> {
        self.locomotors.get(name).cloned()
    }
    pub(super) fn weapon_sets(
        &self,
        name: &str,
    ) -> Option<Vec<Arc<crate::weapon::WeaponTemplateSet>>> {
        // These stable Arcs are also installed at birth. Rebuilding them on
        // every lookup would spuriously reset ammo on the next updateWeaponSet.
        self.weapon_sets.sets.get(name).cloned()
    }

    fn admitted_template(&self, name: &str) -> Result<Arc<dyn ThingTemplate>, String> {
        let template = self
            .thing(name)
            .ok_or_else(|| format!("missing receiving ThingTemplate {name}"))?;
        // This helper's construction branch would otherwise consult TheAI.
        if template.is_kind_of(KindOf::CanBeRepulsed) {
            return Err(
                "native Object restore requires supplied repulsor construction rules".into(),
            );
        }
        if template.get_energy_production() != 0 || template.get_energy_bonus() != 0 {
            return Err(
                "native Object restore does not admit powered template construction".into(),
            );
        }
        for entry in template.get_behavior_module_info() {
            if entry.name.as_str() == "ActiveBody" {
                let data = entry
                    .data
                    .as_ref()
                    .downcast_ref::<crate::object::body::active_body::ActiveBodyModuleData>()
                    .ok_or("native ActiveBody module data has the wrong family")?;
                if !data.initial_health.is_finite()
                    || !data.max_health.is_finite()
                    || (template.is_kind_of(KindOf::Structure) && data.initial_health <= 0.0)
                {
                    return Err("native Body birth needs finite health and supplied initial rubble services".into());
                }
            }
            match entry.name.as_str() {
                "InstantDeathBehavior" => entry.data.as_ref()
                    .downcast_ref::<crate::object::behavior::instant_death_behavior::InstantDeathBehaviorModuleData>()
                    .ok_or("native InstantDeath definition has the wrong family")?.validate_owned_effects()?,
                "SlowDeathBehavior" => {
                    if template.is_kind_of(KindOf::Hulk) { return Err("native SlowDeath hulk construction not admitted".into()); }
                    entry.data.as_ref().downcast_ref::<crate::object::behavior::slow_death_behavior::SlowDeathBehaviorModuleData>()
                        .ok_or("native SlowDeath definition has the wrong family")?.validate_owned_effects()?;
                }
                _ => {},
            }
            if !matches!(
                entry.name.as_str(),
                "ActiveBody"
                    | "InactiveBody"
                    | "PhysicsBehavior"
                    | "AIUpdateInterface"
                    | "OpenContain"
                    | "TransportContain"
                    | "InstantDeathBehavior"
                    | "SlowDeathBehavior"
            ) {
                return Err(format!(
                    "native Object restore module {} requires its own construction services",
                    entry.name
                ));
            }
        }
        Ok(template)
    }

    /// Inert preparation of the same actual installed family used for load.
    /// Birth/team admission remains an explicit driving-session phase.
    pub(crate) fn prepare_object(
        &self,
        factory: &mut ObjectFactory,
        template_name: &str,
        id: ObjectID,
    ) -> Result<Arc<RwLock<Object>>, String> {
        let template = self.admitted_template(template_name)?;
        factory
            .prepare_saved_object(
                template,
                id,
                &mut |name| Ok(self.locomotor(name)),
                self.body_definitions.clone(),
                self.module_factory,
            )
            .map_err(|error| error.to_string())?;
        let owner = factory
            .get_object(id)
            .and_then(|slot| slot.get_base_object())
            .ok_or("prepared Object absent from receiving factory")?;
        self.install_prepared_weapons(&owner, template_name)?;
        Ok(owner)
    }

    /// Native ordinary creation borrows the same identity owner as scripts,
    /// direct construction, scheduling and subsequent restore admission.
    pub(crate) fn prepare_new_object(
        &self,
        factory: &mut ObjectFactory,
        game_logic: &mut crate::system::game_logic::GameLogic,
        template_name: &str,
        position: Coord3D,
        team: Option<Arc<RwLock<Team>>>,
        flags: crate::object::object_factory::ObjectCreationFlags,
    ) -> Result<Arc<RwLock<Object>>, String> {
        let template = self
            .thing(template_name)
            .ok_or("native birth template absent")?;
        let id = factory
            .prepare_object_owned(
                game_logic,
                template,
                position,
                team,
                flags,
                &mut |name| Ok(self.locomotor(name)),
                self.body_definitions.clone(),
                self.module_factory,
            )
            .map_err(|error| error.to_string())?;
        let owner = factory
            .get_object(id)
            .and_then(|slot| slot.get_base_object())
            .ok_or("native birth absent from receiving factory")?;
        self.install_prepared_weapons(&owner, template_name)?;
        Ok(owner)
    }

    fn install_prepared_weapons(
        &self,
        owner: &Arc<RwLock<Object>>,
        template_name: &str,
    ) -> Result<(), String> {
        let id = owner
            .read()
            .map_err(|_| "prepared Object poisoned")?
            .get_id();
        {
            let mut object = owner.write().map_err(|_| "prepared Object poisoned")?;
            object.weapon_set.install_definition_sets(
                template_name,
                self.weapon_sets(template_name)
                    .ok_or("receiving weapon sets absent")?,
            )?;
            let flags = object.cur_weapon_set_flags;
            let bonuses = object.get_weapon_bonus_condition();
            object
                .weapon_set
                .update_weapon_set_with_services(
                    id,
                    &flags,
                    bonuses,
                    self.frame,
                    self.weapon_bonuses,
                )
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Prepare all actual controller identities before references are resolved.
    /// The supplied factory must be the empty detached receiving candidate.
    pub(crate) fn prepare_objects(
        &self,
        factory: &mut ObjectFactory,
        records: &[ObjectSnapshotRecord],
    ) -> Result<Vec<Arc<RwLock<Object>>>, String> {
        if factory.get_statistics().total_objects != 0 {
            return Err("Object restore requires an empty detached receiving factory".into());
        }
        let mut prepared = Vec::with_capacity(records.len());
        let mut identities = std::collections::HashSet::new();
        // Validate every identity/template before constructing any candidate.
        let mut plans = Vec::with_capacity(records.len());
        for record in records {
            let id = record.saved_id()?;
            if !identities.insert(id) {
                return Err(format!("duplicate saved Object id {id}"));
            }
            plans.push((id, self.admitted_template(&record.template_name)?));
        }
        for ((id, _), record) in plans.into_iter().zip(records) {
            prepared.push(self.prepare_object(factory, &record.template_name, id)?);
        }
        Ok(prepared)
    }
}

impl ObjectSnapshotRecord {
    pub(crate) fn saved_id(&self) -> Result<ObjectID, String> {
        // Peek the same prefix with Xfer, never replace the real field reader.
        let mut xfer = XferLoad::new(Cursor::new(&self.bytes), 1);
        let mut version = 9;
        xfer.xfer_version(&mut version, 9)
            .map_err(|error| error.to_string())?;
        let mut id = INVALID_ID;
        xfer.xfer_object_id(&mut id)
            .map_err(|error| error.to_string())?;
        if id == INVALID_ID || id == ObjectID::MAX {
            return Err(format!("invalid saved Object identity {id}"));
        }
        Ok(id)
    }
}

/// Synchronous services for one detached candidate's standard headless family.
/// Callers supply actual receiving owners; there are no stored callbacks or
/// registry fallbacks. Richer render/script/create families require admission.
pub(crate) struct ObjectXferServices<'a> {
    pub factory: &'a ObjectFactory,
    pub teams: &'a TeamFactory,
    pub players: &'a PlayerList,
    pub terrain: &'a crate::terrain::TerrainLogic,
    pub collision: &'a mut crate::object::collide::collision_system::CollisionSystem,
    pub power: &'a mut dyn PowerBrownoutServices,
    pub changed: &'a mut dyn FnMut(ObjectID),
    pub terrain_moved: &'a mut dyn FnMut(&Object),
    pub definitions: ObjectXferDefinitions<'a>,
}

impl ObjectXferServices<'_> {
    pub(crate) fn admit_prepared_object(
        &mut self,
        source: &Arc<RwLock<Object>>,
        team: Arc<RwLock<crate::team::Team>>,
    ) -> Result<(), String> {
        self.validate_owner(source)?;
        if source
            .read()
            .map_err(|_| "Object birth poisoned")?
            .get_team()
            .is_some()
        {
            return Err("prepared Object already has a team".into());
        }
        self.power.validate_team(&team)?;
        self.validate_team_profile(&team)?;
        let teams = self.teams;
        Object::set_or_restore_team_from_handle_with_power_and_tail(
            source,
            Some(team),
            false,
            self.players,
            self.power,
            Some(&mut |source, _, _, _, _, _| restoring_team_tail(source, teams)),
        )
    }
    fn validate_owner(&self, source: &Arc<RwLock<Object>>) -> Result<(), String> {
        let object = source.read().map_err(|_| "Object Xfer owner poisoned")?;
        let owned = self
            .factory
            .get_object(object.id)
            .and_then(|slot| slot.get_base_object())
            .ok_or("Object Xfer owner absent from receiving factory")?;
        if !Arc::ptr_eq(&owned, source) {
            return Err("Object Xfer owner belongs to another factory".into());
        }
        let template = self
            .definitions
            .thing(object.get_template_name())
            .ok_or("Object Xfer ThingTemplate absent from receiving definitions")?;
        if !Arc::ptr_eq(&template, &object.thing_template) {
            return Err("Object Xfer ThingTemplate belongs to another definition owner".into());
        }
        self.definitions
            .admitted_template(object.get_template_name())?;
        if object.drawable.is_some() || object.radar_data.is_some() {
            return Err("native Object Xfer render/radar services are not admitted".into());
        }
        for entry in &object.modules {
            if !matches!(
                entry.name().as_str(),
                "ActiveBody"
                    | "InactiveBody"
                    | "PhysicsBehavior"
                    | "AIUpdateInterface"
                    | "OpenContain"
                    | "TransportContain"
                    | "InstantDeathBehavior"
                    | "SlowDeathBehavior"
            ) {
                return Err(format!(
                    "native Object Xfer module {} requires its own services",
                    entry.name()
                ));
            }
        }
        object.validate_owned_death_callbacks(1.0)?;
        for player in self.players.iter() {
            if player
                .read()
                .map_err(|_| "receiving Player poisoned")?
                .get_num_battle_plans_active()
                != 0
            {
                return Err("native Object restore requires supplied battle-plan callbacks".into());
            }
        }
        drop(object);
        self.power.validate_source(source)?;
        Ok(())
    }

    fn validate_team_profile(&self, team: &Arc<RwLock<crate::team::Team>>) -> Result<(), String> {
        let name = team
            .read()
            .map_err(|_| "receiving Team poisoned")?
            .get_name()
            .to_string();
        let prototype = self
            .teams
            .find_team_prototype(&name)
            .ok_or("receiving Team prototype absent")?;
        if !prototype.get_attack_priority_name().is_empty() {
            return Err("native Object restore requires receiving attack-priority scripts".into());
        }
        Ok(())
    }

    /// Complete wire decoding mutates only this already-detached candidate.
    /// Caller commits the whole receiving session after supplied postload.
    pub(crate) fn load_objects(
        &mut self,
        records: &[ObjectSnapshotRecord],
    ) -> Result<(), ObjectXferError> {
        let neutral = self.players.get_neutral_player().ok_or_else(|| {
            ObjectXferError::new(INVALID_ID, None, "birth", "receiving neutral Player absent")
        })?;
        let default_team = neutral
            .read()
            .map_err(|error| ObjectXferError::new(INVALID_ID, None, "birth", error))?
            .get_default_team()
            .ok_or_else(|| {
                ObjectXferError::new(
                    INVALID_ID,
                    None,
                    "birth",
                    "receiving neutral default Team absent",
                )
            })?;
        self.power
            .validate_team(&default_team)
            .map_err(|error| ObjectXferError::new(INVALID_ID, None, "birth", error))?;
        self.validate_team_profile(&default_team)
            .map_err(|error| ObjectXferError::new(INVALID_ID, None, "birth", error))?;
        for record in records {
            let id = record
                .saved_id()
                .map_err(|error| ObjectXferError::new(INVALID_ID, None, "identity", error))?;
            let source = self
                .factory
                .get_object(id)
                .and_then(|slot| slot.get_base_object())
                .ok_or_else(|| {
                    ObjectXferError::new(id, None, "identity", "saved Object was not prepared")
                })?;
            self.validate_owner(&source)
                .map_err(|error| ObjectXferError::new(id, None, "admission", error))?;
            let teams = self.teams;
            Object::set_or_restore_team_from_handle_with_power_and_tail(
                &source,
                Some(default_team.clone()),
                false,
                self.players,
                self.power,
                Some(&mut |source, _, _, _, _, _| restoring_team_tail(source, teams)),
            )
            .map_err(|error| ObjectXferError::new(id, None, "birth", error))?;
            let mut cursor = Cursor::new(record.bytes.as_slice());
            {
                let mut xfer = XferLoad::new(&mut cursor, 1);
                Object::xfer_from_handle_with_services(&source, &mut xfer, self)?;
            }
            if cursor.position() != record.bytes.len() as u64 {
                return Err(ObjectXferError::new(
                    id,
                    None,
                    "record",
                    "trailing Object bytes",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn save_object(
        &mut self,
        source: &Arc<RwLock<Object>>,
    ) -> Result<ObjectSnapshotRecord, ObjectXferError> {
        let name = source
            .read()
            .map_err(|error| ObjectXferError::new(INVALID_ID, None, "owner", error))?
            .get_template_name()
            .to_string();
        let mut output = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut output, 1);
            Object::xfer_from_handle_with_services(source, &mut xfer, self)?;
        }
        Ok(ObjectSnapshotRecord {
            template_name: name,
            bytes: output.into_inner(),
        })
    }

    fn transform(
        &mut self,
        object: &mut Object,
        change: SavedTransformChange,
    ) -> Result<(), String> {
        match change {
            SavedTransformChange::Matrix(matrix) => object.set_snapshot_transform_with_services(
                &matrix,
                self.terrain,
                self.collision,
                self.definitions.frame,
                self.changed,
                self.terrain_moved,
            ),
            SavedTransformChange::Position(position) => object.set_snapshot_position_with_services(
                &position,
                self.terrain,
                self.collision,
                self.definitions.frame,
                self.changed,
                self.terrain_moved,
            ),
            SavedTransformChange::Orientation(angle) => object
                .set_snapshot_orientation_with_services(
                    angle,
                    self.terrain,
                    self.collision,
                    self.definitions.frame,
                    self.changed,
                    self.terrain_moved,
                ),
        }
    }
}

fn restoring_team_tail(source: &Arc<RwLock<Object>>, teams: &TeamFactory) -> Result<(), String> {
    // Object.cpp:855-901: prototype AI, Drawable, capture (skipped when
    // restoring), radar, UI. The admitted family has no Drawable/radar/SP.
    let (team_name, ai) = {
        let object = source
            .read()
            .map_err(|_| "Object team tail owner poisoned")?;
        let team = object.get_team().ok_or("Object team tail missing team")?;
        let name = team
            .read()
            .map_err(|_| "Object team tail team poisoned")?
            .get_name()
            .to_string();
        (name, object.get_ai_update_interface())
    };
    if let Some(prototype) = teams.find_team_prototype(&team_name) {
        if !prototype.get_attack_priority_name().is_empty() {
            return Err("native Object restore requires receiving attack-priority scripts".into());
        }
        let attitude = match prototype.get_initial_team_attitude() {
            crate::team::AttitudeType::Sleep => AIAttitudeType::Sleep,
            crate::team::AttitudeType::Passive => AIAttitudeType::Passive,
            crate::team::AttitudeType::Alert => AIAttitudeType::Defensive,
            crate::team::AttitudeType::Aggressive => AIAttitudeType::Aggressive,
            crate::team::AttitudeType::Normal | crate::team::AttitudeType::Invalid => {
                AIAttitudeType::Normal
            }
        };
        if let Some(ai) = ai {
            ai.lock()
                .map_err(|_| "Object team tail AI poisoned")?
                .set_attitude(attitude)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

impl Object {
    /// Resolve saved references using the same installed controllers, after
    /// every Object record exists. No state-entry/onContaining is replayed.
    pub(crate) fn load_post_process_from_handle_with_world(
        source: &Arc<RwLock<Object>>,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        let (id, contained_by, modules, ai, contain) = {
            let object = source
                .read()
                .map_err(|_| "Object postload owner poisoned")?;
            (
                object.id,
                object.get_contained_by(),
                object.modules.clone(),
                object.ai.clone(),
                object.contain.clone(),
            )
        };
        let owned = world
            .object(id)
            .ok_or("Object postload owner absent from receiving world")?;
        if !Arc::ptr_eq(source, &owned) {
            return Err("Object postload owner belongs to another world".into());
        }
        if let Some(parent) = contained_by {
            let parent = world
                .object(parent)
                .ok_or("Object postload containing owner absent")?;
            if Arc::ptr_eq(source, &parent)
                || parent
                    .read()
                    .map_err(|_| "containing owner poisoned")?
                    .get_contain()
                    .is_none()
            {
                return Err("invalid containing owner in Object postload".into());
            }
        }
        // Original registered Snapshot traversal follows actual module order.
        // The supplied versions replace only the owner-dependent wrappers.
        for entry in modules {
            match entry.name().as_str() {
                "AIUpdateInterface" => {
                    let ai = ai.as_ref().ok_or("installed AI postload cache absent")?;
                    ai.lock()
                        .map_err(|_| "installed AI postload poisoned")?
                        .load_post_process_with_world(world)?;
                }
                "OpenContain" | "TransportContain" => {
                    let contain = contain
                        .as_ref()
                        .ok_or("installed contain postload cache absent")?;
                    contain
                        .lock()
                        .map_err(|_| "installed contain postload poisoned")?
                        .snapshot_load_post_process_with_world(world)?;
                }
                "ActiveBody"
                | "InactiveBody"
                | "PhysicsBehavior"
                | "InstantDeathBehavior"
                | "SlowDeathBehavior" => {
                    entry.with_module(|module| module.load_post_process())?;
                }
                _ => {
                    return Err(format!(
                        "Object postload module {} is not admitted",
                        entry.name()
                    ));
                }
            }
        }
        Ok(())
    }

    /// Admit this Object's actual installed update proxies after preparation
    /// or candidate postload. This phase never publishes a compatibility owner.
    pub(crate) fn admit_update_schedule_from_handle(
        source: &Arc<RwLock<Object>>,
        scheduler: &mut crate::system::game_logic::GameLogic,
    ) -> Result<(), String> {
        let (id, updates) = {
            let object = source
                .read()
                .map_err(|_| "restored scheduler owner poisoned")?;
            let updates = object
                .update_module_registrations
                .iter()
                .map(|registration| {
                    let wake = registration
                        .module_index
                        .and_then(|index| object.modules.get(index))
                        .map(|entry| entry.with_module(initial_update_wake_frame))
                        .unwrap_or(0);
                    (registration.module.clone(), wake)
                })
                .collect::<Vec<_>>();
            (object.id, updates)
        };
        scheduler.validate_scheduling_owner(id, source)?;
        scheduler.admit_scheduling_owner(id, source.clone());
        for (module, wake) in updates {
            scheduler.register_sleepy_update_module(id, module, wake);
        }
        Ok(())
    }

    /// Transfer the real installed object while releasing its guard exactly
    /// around the original team lifecycle. Errors mutate only the supplied
    /// candidate; this operation itself is intentionally not a transaction.
    pub(crate) fn xfer_from_handle_with_services(
        source: &Arc<RwLock<Object>>,
        xfer: &mut dyn Xfer,
        services: &mut ObjectXferServices<'_>,
    ) -> Result<(), ObjectXferError> {
        let object_id = source
            .read()
            .map_err(|error| ObjectXferError::new(INVALID_ID, None, "owner", error))?
            .id;
        services
            .validate_owner(source)
            .map_err(|error| ObjectXferError::new(object_id, None, "native admission", error))?;
        let prefix = source
            .write()
            .map_err(|error| ObjectXferError::new(object_id, None, "owner", error))?
            .xfer_prefix(xfer, &mut |object, change| {
                services.transform(object, change)
            })?;
        services
            .validate_owner(source)
            .map_err(|error| ObjectXferError::new(object_id, None, "native identity", error))?;
        if prefix.mode == game_engine::system::XferMode::Load {
            let team = resolve_saved_team(services.teams, prefix.team_id)
                .map_err(|error| ObjectXferError::new(object_id, None, "team", error))?;
            services
                .validate_team_profile(&team)
                .map_err(|error| ObjectXferError::new(object_id, None, "team", error))?;
            let teams = services.teams;
            Self::set_or_restore_team_from_handle_with_power_and_tail(
                source,
                Some(team),
                true,
                services.players,
                services.power,
                Some(&mut |source, _, _, _, _, _| restoring_team_tail(source, teams)),
            )
            .map_err(|error| ObjectXferError::new(object_id, None, "team", error))?;
        }
        let mut object = source
            .write()
            .map_err(|error| ObjectXferError::new(object_id, None, "owner", error))?;
        object.xfer_tail(xfer, prefix, Some(services))?;
        if prefix.mode == game_engine::system::XferMode::Load {
            let geometry = &object.geometry_info;
            geometry
                .shape
                .validate_saved_shape()
                .map_err(|error| ObjectXferError::new(object_id, None, "geometry", error))?;
        }
        Ok(())
    }
}
