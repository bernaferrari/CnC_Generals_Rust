//! Split-out inherent `lifecycle (construct, destroy, identity, team, death)` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    pub(super) fn disabled_tint_exceptions() -> DisabledMaskType {
        let mut exceptions = DisabledMaskType::none();
        exceptions.set_disabled(DisabledType::Held);
        exceptions.set_disabled(DisabledType::DisabledScriptDisabled);
        exceptions.set_disabled(DisabledType::DisabledUnmanned);
        exceptions
    }

    pub(super) fn flags_requiring_disabled_tint(flags: DisabledMaskType) -> DisabledMaskType {
        flags.difference(Self::disabled_tint_exceptions())
    }

    /// Creates a new Object instance with no predetermined ID.
    pub fn new(
        thing_template: Arc<dyn ThingTemplate>,
        object_status_mask: ObjectStatusMaskType,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<Arc<RwLock<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_id(thing_template, INVALID_ID, object_status_mask, team)
    }

    /// Creates a new Object instance with a specified object ID.
    pub fn new_with_id(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<Arc<RwLock<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let obj = Self::new_raw(
            thing_template.clone(),
            object_id,
            object_status_mask,
            team.clone(),
        );

        let object_arc = Arc::new(RwLock::new(obj));

        {
            let mut guard = object_arc
                .write()
                .map_err(|_| "object lock poisoned during initialization")?;
            guard.set_team(team)?;
        }

        if object_id != INVALID_ID {
            OBJECT_REGISTRY.register_object(object_id, &object_arc);
            register_legacy_object(&object_arc);
        }

        if let Err(err) = Self::init_modules_for(&object_arc, &thing_template) {
            if object_id != INVALID_ID {
                OBJECT_REGISTRY.unregister_object(object_id);
                unregister_legacy_object(object_id);
            }
            return Err(err);
        }

        Ok(object_arc)
    }

    /// Creates a raw Object instance (internal use)
    pub fn new_raw(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Self {
        Self {
            id: object_id,
            producer_id: INVALID_ID,
            builder_id: INVALID_ID,
            name: AsciiString::new(),
            thing_template: Arc::clone(&thing_template),

            next_object_id: None,
            prev_object_id: None,

            status: object_status_mask,
            private_status: 0,
            script_status: 0,

            geometry_info: thing_template.get_template_geometry_info(),
            health_box_offset: Coord3D::new(0.0, 0.0, 0.0),
            i_pos: ICoord3D::ZERO,

            team_id: None,
            team_pin: None,
            original_team_name: AsciiString::new(),
            indicator_color: Color::default(),

            behaviors: Vec::new(),
            modules: Vec::new(),
            body_module_handles: Vec::new(),
            die_module_handles: Vec::new(),
            update_module_handles: Vec::new(),
            update_module_registrations: Vec::new(),
            collide_module_handles: Vec::new(),
            contain_module_handles: Vec::new(),
            upgrade_module_handles: Vec::new(),
            body: None,
            contain: None,
            stealth: None,
            ai: None,
            physics: None,

            repulsor_helper: None,
            smc_helper: None,
            ws_helper: None,
            defection_helper: None,
            status_damage_helper: None,
            subdual_damage_helper: None,
            temp_weapon_bonus_helper: None,
            firing_tracker: None,
            held_helper: None,

            partition_data: Some(Arc::new(Mutex::new(PartitionData::new()))),
            radar_data: None,

            partition_last_look: SightingInfo::new(),
            partition_reveal_all_last_look: SightingInfo::new(),
            partition_last_shroud: SightingInfo::new(),
            partition_last_threat: SightingInfo::new(),
            partition_last_value: SightingInfo::new(),
            vision_spied_by: [0; MAX_PLAYER_COUNT],
            vision_spied_mask: PlayerMaskType::none(),
            vision_range: thing_template.calc_vision_range(),
            shroud_clearing_range: {
                let range = thing_template.calc_shroud_clearing_range();
                if range < 0.0 {
                    thing_template.calc_vision_range()
                } else {
                    range
                }
            },
            shroud_range: 0.0,

            contained_by_id: INVALID_ID,
            contained_by_frame: 0,
            is_transporting: false,

            construction_percent: CONSTRUCTION_COMPLETE,
            object_upgrades_completed: UpgradeMaskType::none(),

            group_id: None,
            experience_tracker: None,
            captured: false,
            veterancy_level: VeterancyLevel::Regular,
            experience_points: 0.0,

            weapon_set: WeaponSet::new(),
            weapon_bonus_multiplier: 1.0,
            cur_weapon_set_flags: WeaponSetFlags::new(),
            armor_set_flags: ArmorSetFlagBits::default(),
            weapon_bonus_condition: WeaponBonusConditionFlags::empty(),
            last_weapon_condition: [0; WEAPONSLOT_COUNT],
            special_power_bits: SpecialPowerMask::default(),

            sole_healing_benefactor_id: INVALID_ID,
            sole_healing_benefactor_expiration_frame: NEVER,

            disabled_mask: DisabledMaskType::none(),
            disabled_till_frame: [NEVER; DISABLED_COUNT],
            smc_until: NEVER,
            special_model_condition_flag: ModelConditionFlags::empty(),
            invulnerable_until_frame: 0,

            trigger_info: Default::default(),
            entered_or_exited_frame: 0,
            num_trigger_areas_active: 0,

            layer: PathfindLayerEnum::Ground,
            destination_layer: PathfindLayerEnum::Ground,

            formation_id: FormationID::NONE,
            formation_offset: Coord2D::ZERO,

            command_set_string_override: AsciiString::new(),
            safe_occlusion_frame: 0,
            carrier_deck_height: 0.0,
            drawable: None,

            // Initialize visibility flags - by default all players see the object (will be updated by rendering)
            visibility_flags: [true; MAX_PLAYER_COUNT],
            visibility_alpha: [1.0; MAX_PLAYER_COUNT],
            last_visibility_update_frame: 0,

            is_selectable: thing_template.is_kind_of(KindOf::Selectable),
            modules_ready: false,
            single_use_command_used: false,
            is_receiving_difficulty_bonus: false,
            destroyed: false,

            #[cfg(any(debug_assertions, feature = "internal"))]
            has_died_already: false,
        }
    }

    /// Initialize object after creation
    pub fn init_object(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = self.id;
        crate::object::create::with_create_owner_object(self as *mut Object, || {
            for module in self.modules_with_interface(ModuleInterfaceType::CREATE) {
                module.with_module(|module| {
                    if let Some(create) = module.get_create_interface() {
                        create.on_create();
                    } else {
                        log::debug!(
                            "Object {} module '{}' advertises CREATE but has no create interface",
                            object_id,
                            module.get_module_name_key()
                        );
                    }
                });
            }
        });

        if self.firing_tracker.is_none()
            && !self.has_firing_tracker_module()
            && self.weapon_set.has_any_weapons()
        {
            self.firing_tracker = Some(Arc::new(Mutex::new(FiringTracker::new(self.id))));
        }

        self.init_object_cpp_sequence();

        Ok(())
    }

    /// Notify create modules that construction has completed.
    pub fn on_build_complete(&mut self) {
        let object_id = self.id;
        crate::object::create::with_create_owner_object(self as *mut Object, || {
            for module in self.modules_with_interface(ModuleInterfaceType::CREATE) {
                module.with_module(|module| {
                    if let Some(create) = module.get_create_interface() {
                        if create.should_do_on_build_complete() {
                            create.on_build_complete();
                        }
                    } else {
                        log::debug!(
                            "Object {} module '{}' advertises CREATE but has no create interface",
                            object_id,
                            module.get_module_name_key()
                        );
                    }
                });
            }
        });
    }

    /// Called during object destruction
    pub fn on_destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.status.set_status(ObjectStatusTypes::Destroyed);

        let _ = crate::scripting::engine::get_named_object_tracker().unregister_object(self.id);

        for module in self.update_module_registrations.drain(..) {
            let _ = crate::helpers::TheGameLogic::unregister_update_module(self.id, module);
        }

        self.on_destroy_internal();
        self.run_destructor_tail();
    }

    /// C++ `Object::~Object` after `onDestroy`: pathfinder, scripts, radar,
    /// `sendObjectDestroyed`, clear team/group, ControlBar dirty.
    pub(crate) fn run_destructor_tail(&mut self) {
        let pos = *self.get_position();
        let footprint = crate::ai::object_footprint_positions(self).unwrap_or_else(|| vec![pos]);
        if let Ok(ai) = crate::ai::THE_AI.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(mut pf) = pf.write() {
                    pf.remove_object_from_map(self.id, &footprint);
                    pf.remove_wall_from_object(self);
                }
            }
        }

        if !self.is_kind_of(KindOf::Projectile) && !self.is_kind_of(KindOf::Inert) {
            crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(self.id);
            crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();
        }

        if self.radar_data.is_some() {
            let radar = game_engine::common::system::radar::get_radar_system();
            if let Ok(mut radar_guard) = radar.write() {
                radar_guard.remove_object(self.id);
            }
            self.radar_data = None;
        }

        if let Ok(logic) = crate::system::game_logic::get_game_logic().try_lock() {
            logic.send_object_destroyed(self.id);
        }

        let _ = self.set_team(None);

        if let Some(group) = self.get_group() {
            if let Ok(mut group_guard) = group.write() {
                let _ = group_guard.remove(self.id);
            }
        }
        self.group_id = None;

        if let Ok(mut engine) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = engine.as_mut() {
                engine.notify_of_object_destruction(self.id);
            }
        }

        crate::control_bar::mark_ui_dirty();
    }

    /// Internal destroy routine that performs per-object module cleanup
    /// without touching the global `GameLogic` instance directly.
    pub(crate) fn on_destroy_internal(&mut self) {
        // C++ counterpart releases containment before running module onDelete.
        if let Some(container_id) = self.get_container_id() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(
                container_id,
                |container_read| {
                    if let Some(contain_module) = container_read.get_contain() {
                        if let Ok(mut contain_guard) = contain_module.lock() {
                            let _ = contain_guard.release_object(self.id);
                        }
                    }
                },
            );
            let _ = self.on_removed_from(container_id);
        }

        self.upgrade_module_handles.clear();

        let mut modules = std::mem::take(&mut self.modules);
        for entry in modules.drain(..) {
            entry.with_module(|module| {
                if let Some(upgrade) = super::module_upgrade_kind(module) {
                    upgrade.into_interface().on_delete(self);
                }
                module.on_delete();
            });
        }
        self.modules = modules;

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.clear_modules();
            }
        }
        self.drawable = None;

        // Match C++ Object::onDestroy -> handlePartitionCellMaintenance.
        // This clears partition/shroud/value/threat bookkeeping before the object is fully removed.
        self.handle_partition_cell_maintenance();

        self.modules_ready = false;
    }

    // Core identification methods
    pub fn get_id(&self) -> ObjectID {
        self.id
    }

    pub fn get_object_id(&self) -> ObjectID {
        self.id
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    pub fn is_receiving_difficulty_bonus(&self) -> bool {
        self.is_receiving_difficulty_bonus
    }

    // Linked list navigation
    pub fn get_next_object_id(&self) -> Option<ObjectID> {
        self.next_object_id
    }

    pub fn get_prev_object_id(&self) -> Option<ObjectID> {
        self.prev_object_id
    }

    pub(crate) fn set_next_object_id(&mut self, next_object_id: Option<ObjectID>) {
        self.next_object_id = next_object_id.filter(|id| *id != INVALID_ID);
    }

    pub(crate) fn set_prev_object_id(&mut self, prev_object_id: Option<ObjectID>) {
        self.prev_object_id = prev_object_id.filter(|id| *id != INVALID_ID);
    }

    pub fn get_next_object(&self) -> Option<Arc<RwLock<Object>>> {
        // C++ Object::getNextObject (Object.h:155) returns m_next with no
        // dual-world gate. Registry lookup already falls back to GameLogic.
        self.next_object_id
            .and_then(|object_id| OBJECT_REGISTRY.get_object(object_id))
    }

    pub fn get_prev_object(&self) -> Option<Arc<RwLock<Object>>> {
        // C++ Object::getNextObject sibling: m_prev, no empty-world skip.
        self.prev_object_id
            .and_then(|object_id| OBJECT_REGISTRY.get_object(object_id))
    }

    // Producer/Builder relationships
    pub fn get_producer_id(&self) -> ObjectID {
        self.producer_id
    }

    pub fn set_producer(&mut self, obj: Option<&Object>) {
        self.producer_id = obj.map(|o| o.get_id()).unwrap_or(INVALID_ID);
    }

    pub fn set_producer_id(&mut self, producer_id: ObjectID) {
        self.producer_id = producer_id;
    }

    pub fn get_builder_id(&self) -> ObjectID {
        self.builder_id
    }

    pub fn set_builder(&mut self, obj: Option<&Object>) {
        self.builder_id = obj.map(|o| o.get_id()).unwrap_or(INVALID_ID);
    }

    // Team management
    pub fn get_team(&self) -> Option<Arc<RwLock<Team>>> {
        if let Some(id) = self.team_id {
            if let Ok(factory) = crate::team::get_team_factory().lock() {
                if let Some(team) = factory.find_team_by_id(id) {
                    return Some(team);
                }
            }
        }
        self.team_pin.clone()
    }

    pub fn get_team_id(&self) -> Option<TeamID> {
        if self.team_id.is_some() {
            return self.team_id;
        }
        self.team_pin
            .as_ref()
            .and_then(|t| t.read().ok())
            .map(|g| g.get_id())
    }

    pub fn set_team(
        &mut self,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // C++ parity (Object::setTeam): if team owner is inactive, force neutral default team.
        let resolved_team = if let Some(team_ref) = team {
            let owner_inactive = team_ref
                .read()
                .ok()
                .and_then(|team_guard| team_guard.get_controlling_player_id())
                .and_then(|player_id| {
                    player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(player_id as PlayerIndex).cloned())
                })
                .and_then(|player_arc| {
                    player_arc
                        .read()
                        .ok()
                        .map(|player| !player.is_player_active())
                })
                .unwrap_or(false);

            if owner_inactive {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_neutral_player())
                    .and_then(|neutral| neutral.read().ok().and_then(|p| p.get_default_team()))
            } else {
                Some(team_ref)
            }
        } else {
            None
        };

        self.set_or_restore_team(resolved_team, false)?;
        self.original_team_name = {
            let team = self.get_team();
            team.and_then(|team_ref| team_ref.read().ok().map(|g| g.get_name().clone()))
                .unwrap_or_else(AsciiString::new)
        };
        Ok(())
    }

    pub fn set_temporary_team(
        &mut self,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.set_or_restore_team(team, false)
    }

    pub fn restore_original_team(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::team::get_team_factory;

        if (self.get_team_id().is_none() && self.team_pin.is_none())
            || self.original_team_name.is_empty()
        {
            return Ok(());
        }

        let original_name = self.original_team_name.to_string();
        let restored_team = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&original_name));

        let Some(restored_team) = restored_team else {
            log::warn!(
                "Object::restore_original_team failed to resolve original team '{}'",
                original_name
            );
            return Ok(());
        };

        let current_team_id = self.get_team_id();
        let restored_team_id = restored_team
            .read()
            .ok()
            .map(|team_guard| team_guard.get_id());
        if current_team_id.is_some() && current_team_id == restored_team_id {
            return Ok(());
        }

        self.set_team(Some(restored_team))?;

        Ok(())
    }

    /// Get relationship to another object (mirrors C++ Object::getRelationshipTo).
    pub fn get_relationship_to(
        &self,
        other: &Object,
    ) -> crate::object::contain::open_contain::ObjectRelationship {
        use crate::common::Relationship;
        use crate::object::contain::open_contain::ObjectRelationship;
        if self.get_id() == other.get_id() {
            return ObjectRelationship::Self_;
        }

        let relationship = self.relationship_to(other);

        match relationship {
            Relationship::Enemies => ObjectRelationship::Enemy,
            Relationship::Allies => ObjectRelationship::Ally,
            _ => ObjectRelationship::Neutral,
        }
    }

    // Status management
    pub fn is_destroyed(&self) -> bool {
        self.test_status(ObjectStatusTypes::Destroyed)
    }

    pub fn is_alive(&self) -> bool {
        !self.is_effectively_dead()
    }

    /// Convenience method for getting ID (alias for get_id())
    /// Some C++ code uses .id() instead of .get_id()
    pub fn id(&self) -> ObjectID {
        self.get_id()
    }

    pub(super) fn set_or_restore_team(
        &mut self,
        team: Option<Arc<RwLock<Team>>>,
        restoring: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_team = self.get_team();
        let old_team_id = self.get_team_id();

        let old_player_id = old_team
            .as_ref()
            .and_then(|team_ref| team_ref.read().ok())
            .and_then(|team_guard| team_guard.get_controlling_player_id());
        let incoming_player_id = team
            .as_ref()
            .and_then(|team_ref| team_ref.read().ok())
            .and_then(|team_guard| team_guard.get_controlling_player_id());
        if old_player_id != incoming_player_id {
            self.adjust_power_for_player(false);
        }

        // Store ID when factory-resolvable; keep pin only as unregistered fallback.
        match &team {
            Some(team_ref) => {
                let id = team_ref.read().ok().map(|g| g.get_id());
                self.team_id = id;
                let factory_has = id
                    .and_then(|tid| {
                        crate::team::get_team_factory()
                            .lock()
                            .ok()
                            .and_then(|f| f.find_team_by_id(tid))
                    })
                    .is_some();
                self.team_pin = if factory_has {
                    None
                } else {
                    Some(Arc::clone(team_ref))
                };
            }
            None => {
                self.team_id = None;
                self.team_pin = None;
            }
        }

        let new_team = self.get_team();
        let new_team_id = self.get_team_id();

        // C++ parity: Object::setOrRestoreTeam() is a no-op if team hasn't changed.
        if old_team_id == new_team_id {
            return Ok(());
        }

        let new_player_id = new_team
            .as_ref()
            .and_then(|team_ref| team_ref.read().ok())
            .and_then(|team_guard| team_guard.get_controlling_player_id());

        if old_player_id != new_player_id {
            let Ok(list_guard) = player_list().read() else {
                return Ok(());
            };
            if let Some(old_id) = old_player_id {
                if let Some(player_arc) = list_guard.get_player(old_id as PlayerIndex).cloned() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        if self.modules_ready && player_guard.get_num_battle_plans_active() > 0 {
                            player_guard.remove_battle_plan_bonuses_for_object(self);
                        }
                        player_guard.remove_owned_object(self.id);
                    }
                }
            }
            if let Some(new_id) = new_player_id {
                if let Some(player_arc) = list_guard.get_player(new_id as PlayerIndex).cloned() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        player_guard.add_owned_object(self.id);
                        if self.modules_ready && player_guard.get_num_battle_plans_active() > 0 {
                            player_guard.apply_battle_plan_bonuses_for_object(self);
                        }
                    }
                }
            }
            drop(list_guard);
            self.adjust_power_for_player(true);
            self.notify_team_switch_side_effects(
                old_player_id.map(|id| id as i32),
                new_player_id.map(|id| id as i32),
            );
        }

        // Keep per-team member lists in sync with object team ownership.
        // Use non-blocking team locks to avoid lock inversion with callers that already
        // hold team write locks while changing object ownership.
        if old_team_id != new_team_id {
            if let Some(old_team_ref) = old_team {
                if let Ok(mut old_team_guard) = old_team_ref.try_write() {
                    old_team_guard.remove_member(self.id);
                }
            }
            if let Some(new_team_ref) = new_team {
                if let Ok(mut new_team_guard) = new_team_ref.try_write() {
                    new_team_guard.add_member(self.id);
                }
            }
        }

        if old_team_id.is_some() && new_team_id.is_some() && !restoring {
            let (old_owner, new_owner) = if let Ok(list_guard) = player_list().read() {
                let old_owner =
                    old_player_id.and_then(|id| list_guard.get_player(id as PlayerIndex).cloned());
                let new_owner =
                    new_player_id.and_then(|id| list_guard.get_player(id as PlayerIndex).cloned());
                (old_owner, new_owner)
            } else {
                (None, None)
            };
            self.on_capture(old_owner, new_owner);
        }

        if !restoring {
            if let Some(new_id) = new_player_id {
                if let Ok(list_guard) = player_list().read() {
                    let player = list_guard.get_player(new_id as PlayerIndex).cloned();
                    drop(list_guard);
                    self.award_initial_capture_bonus_if_needed(player);
                }
            }
        }

        self.refresh_radar_object_from_state();

        // C++ parity: team switches update AI attitude from the new team prototype.
        self.apply_team_ai_profile();

        self.update_drawable_team_visuals();
        Ok(())
    }

    pub(super) fn apply_team_ai_profile(&self) {
        let team_name = {
            let team = self.get_team();
            team.and_then(|team_ref| team_ref.read().ok().map(|g| g.get_name().to_string()))
        };

        let attitude = team_name
            .as_deref()
            .and_then(|name| {
                crate::team::get_team_factory()
                    .lock()
                    .ok()
                    .and_then(|factory| factory.find_team_prototype(name))
            })
            .map(|prototype| match prototype.get_initial_team_attitude() {
                crate::team::AttitudeType::Sleep => AIAttitudeType::Sleep,
                crate::team::AttitudeType::Passive => AIAttitudeType::Passive,
                crate::team::AttitudeType::Alert => AIAttitudeType::Defensive,
                crate::team::AttitudeType::Aggressive => AIAttitudeType::Aggressive,
                crate::team::AttitudeType::Normal | crate::team::AttitudeType::Invalid => {
                    AIAttitudeType::Normal
                }
            });

        let Some(attitude) = attitude else {
            return;
        };

        if let Some(ai) = self.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_attitude(attitude);
            }
        }
    }

    pub(super) fn set_id(&mut self, id: ObjectID) {
        self.id = id;
    }

    /// Handle object death - called when health reaches zero
    /// This is the entry point for death - it sets up the death state and then calls on_die()
    pub fn handle_death(&mut self, damage_info: Option<&DamageInfo>) {
        // Prevent multiple death calls
        if self.is_effectively_dead() {
            return;
        }

        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            if self.has_died_already {
                log::warn!("Object {} died multiple times!", self.id);
                return;
            }
            self.has_died_already = true;
        }

        // Mark as effectively dead immediately to prevent recursive death
        self.set_effectively_dead(true);

        // OBJECT_STATUS_DESTROYED is set later in GameLogic::destroyObject.

        log::debug!("Object {} is dying (health reached 0)", self.id);

        // Fire destruction event
        let killer_id = damage_info
            .map(|d| d.input.source_id)
            .filter(|&id| id != INVALID_ID);
        self.fire_destroyed_event(killer_id);

        // Call the main on_die method which handles all object-level death logic
        // If we have damage_info, call on_die; otherwise create a default one
        if let Some(damage) = damage_info {
            self.on_die(damage);
        } else {
            // Create a default damage info for death without damage
            let default_damage = DamageInfo {
                input: DamageInfoInput {
                    damage_type: DamageType::Unresistable,
                    death_type: DeathType::Normal,
                    amount: 0.0,
                    kill: true,
                    source_id: INVALID_ID,
                    ..Default::default()
                },
                ..Default::default()
            };
            self.on_die(&default_damage);
        }

        log::debug!("Object {} death processing complete", self.id);
    }

    /// Call OnDie hooks on all modules that support the die interface
    pub(super) fn call_on_die_hooks(&mut self, damage_info: Option<&DamageInfo>) {
        // Collect die module handles
        let die_modules: Vec<Arc<ModuleEntry>> = self.die_module_handles.clone();

        for module_entry in die_modules {
            module_entry.with_module(|module| {
                if let Some(die_module) = module_die_kind(module) {
                    if let Some(damage) = damage_info {
                        let _ = die_module.into_interface().on_die(damage);
                    }
                }
            });
        }
    }

    /// Check health and trigger death if needed
    /// Returns true if the object died
    ///
    /// C++ Reference: Object.cpp lines 1862-1892 (death check after attemptDamage)
    ///
    /// # Arguments
    /// * `damage_info` - Optional mutable reference to damage info (to set killed_target flag)
    ///
    /// # Returns
    /// * `true` - Object died and death was handled
    /// * `false` - Object is still alive
    ///
    /// # Behavior
    /// - Checks if health <= 0
    /// - Awards experience to attacker if this is a kill
    /// - Calls handle_death() to process death
    /// - Sets killed_target flag in damage_info if object died
    pub fn check_health_and_die(&mut self, damage_info: Option<&mut DamageInfo>) -> bool {
        if self.is_effectively_dead() {
            return true;
        }

        let current_health = self.get_health();

        if current_health <= 0.0 {
            // Process death
            self.handle_death(damage_info.as_deref());

            // Mark that we killed the target
            if let Some(info) = damage_info {
                info.output.killed_target = true;
            }

            return true;
        }

        false
    }

    //=========================================================================
    // OBJECT DEATH AND CAPTURE HANDLING
    // C++ Reference: Object.cpp lines 4548-4647 (onDie), 4509-4544 (onCapture)
    //=========================================================================

    /// Central point for onDie logic - called when object dies
    /// C++ Reference: Object.cpp lines 4548-4647
    ///
    /// This method handles all object-level death processing:
    /// - Notifies all behavior modules via die interface
    /// - Handles spawner notification
    /// - Removes from radar
    /// - Clears terrain decals
    /// - Notifies team of death
    /// - Plays EVA notifications for locally controlled units
    /// - Handles rebuild hole logic for GLA structures
    ///
    /// # Arguments
    /// * `damage_info` - Information about the damage that caused death
    ///
    /// # Notes
    /// - This is called AFTER the object is marked as effectively dead
    /// - Multiple calls are prevented by has_died_already flag
    /// - This should only be called internally by handle_death()
    pub fn on_die(&mut self, damage_info: &DamageInfo) {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            if self.has_died_already {
                log::error!(
                    "Object::on_die has been called multiple times for object {}",
                    self.id
                );
                return;
            }
        }

        let self_inflicted = damage_info.input.source_id == self.id;
        self.on_die_detonate_booby_trap();

        // FIRST, call our die modules
        log::debug!("Object {} calling die modules", self.id);
        self.call_on_die_hooks(Some(damage_info));

        if let Some(contain) = &self.contain {
            if let Ok(mut contain_guard) = contain.lock() {
                if let Err(err) = contain_guard.on_die(Some(damage_info)) {
                    log::warn!("Object {} contain on_die failed: {}", self.id, err);
                }
            }
        }

        self.on_die_remove_from_radar();

        // Just in case I have been sporting one of those fancy Terrain Decals,
        // I naturally lose it now, because I'm dead.
        self.on_die_fade_terrain_decal();

        // Objects that were spawned from something need to tell their spawner that they have died
        if self.producer_id != INVALID_ID {
            if let Some(spawner) = crate::helpers::TheGameLogic::find_object_by_id(self.producer_id)
            {
                if let Ok(spawner_guard) = spawner.write() {
                    let mut spawn_damage = damage_info.clone();
                    let _ = spawner_guard.with_spawn_behavior_full_interface(|spawn_behavior| {
                        let _ = spawn_behavior.on_spawn_death(self.id, &mut spawn_damage);
                    });
                }
            }
        }

        // Handle partition cell maintenance
        self.handle_partition_cell_maintenance();

        // Notify team of object death
        if let Some(team) = self.get_team() {
            if let Ok(mut team_guard) = team.write() {
                log::debug!("Object {} notifying team of death", self.id);
                team_guard.remove_member(self.id);
                team_guard.notify_team_of_object_death();
            }
        }

        // Play EVA notifications for locally controlled units
        if self.is_locally_controlled() && !self_inflicted {
            if self.is_kind_of(KindOf::Structure) && self.is_kind_of(KindOf::CountsForVictory) {
                log::debug!(
                    "Object {} (structure) lost - EVA notification would play",
                    self.id
                );
                if let Err(err) =
                    crate::helpers::TheEva::set_should_play(crate::helpers::EvaEvent::BuildingLost)
                {
                    log::warn!(
                        "Object {} failed to queue building lost EVA event: {:?}",
                        self.id,
                        err
                    );
                }
            } else if self.is_kind_of(KindOf::Infantry) || self.is_kind_of(KindOf::Vehicle) {
                log::debug!(
                    "Object {} (unit) lost - EVA notification would play",
                    self.id
                );
                if let Err(err) =
                    crate::helpers::TheEva::set_should_play(crate::helpers::EvaEvent::UnitLost)
                {
                    log::warn!(
                        "Object {} failed to queue unit lost EVA event: {:?}",
                        self.id,
                        err
                    );
                }
                self.on_die_unit_lost_fake_radar();
            }
        }

        // Remove from idle worker list if applicable
        if let Some(player_id) = self.get_controlling_player_id() {
            log::trace!("Object {} removing from idle worker list", self.id);
            crate::helpers::TheInGameUI::remove_idle_worker(self, player_id as Int);
        }

        self.on_die_rebuild_hole_transfer();

        log::debug!("Object {} on_die processing complete", self.id);
    }

    /// Kill the object instantly without going through normal damage sequence
    /// This is an overload of the kill() method that calls on_die() explicitly
    /// C++ Reference: Object.cpp lines 1930-1944
    ///
    /// # Arguments
    /// * `damage_type` - Optional damage type (defaults to Unresistable)
    /// * `death_type` - Optional death type (defaults to Normal)
    ///
    /// # Notes
    /// - This creates a DamageInfo with kill flag set
    /// - Calls attemptDamage() which will trigger on_die() internally
    /// - The existing kill_with_type() already handles this correctly
    pub fn kill_instant(
        &mut self,
        damage_type: Option<DamageType>,
        death_type: Option<DeathType>,
    ) -> Result<(), ObjectError> {
        // Delegate to existing implementation
        self.kill_with_type(damage_type, death_type)
    }

    /// Create an object for save/load when ThingFactory cannot rebuild the template yet.
    pub fn new_for_xfer_load(id: ObjectID, max_health: f32) -> Self {
        let template = Arc::new(DefaultThingTemplate::new("XferLoadObject".to_string()));
        let mut obj = Self::new_raw(template, id, ObjectStatusMaskType::none(), None);
        let mut module_data = crate::object::body::active_body::ActiveBodyModuleData::default();
        module_data.max_health = max_health;
        module_data.initial_health = max_health;
        let body: Arc<Mutex<dyn crate::object::body::body_module::BodyModuleInterface>> =
            Arc::new(Mutex::new(
                crate::object::body::active_body::ActiveBody::new_with_owner(
                    module_data,
                    obj.get_id(),
                ),
            ));
        obj.body = Some(body);
        obj.install_ctor_helpers();
        obj
    }

    /// Create a test object for unit tests
    #[cfg(any(test, feature = "internal"))]
    pub fn new_test(id: ObjectID, max_health: f32) -> Self {
        let template = Arc::new(DefaultThingTemplate::new("TestObject".to_string()));
        Self::new_test_from_template(id, max_health, template)
    }

    #[cfg(any(test, feature = "internal"))]
    pub fn new_test_from_template(
        id: ObjectID,
        max_health: f32,
        template: Arc<dyn ThingTemplate>,
    ) -> Self {
        let mut obj = Self::new_raw(template, id, ObjectStatusMaskType::none(), None);
        let mut module_data = crate::object::body::active_body::ActiveBodyModuleData::default();
        module_data.max_health = max_health;
        module_data.initial_health = max_health;
        let body: Arc<Mutex<dyn crate::object::body::body_module::BodyModuleInterface>> =
            Arc::new(Mutex::new(
                crate::object::body::active_body::ActiveBody::new_with_owner(
                    module_data,
                    obj.get_id(),
                ),
            ));
        obj.body = Some(body);
        obj.install_ctor_helpers();
        obj
    }

    /// Swap the thing template on a test object (KINDOF flags, trainable, etc.).
    #[cfg(any(test, feature = "internal"))]
    pub fn set_template_for_test(&mut self, template: Arc<dyn ThingTemplate>) {
        self.thing_template = template;
    }

    /// Attach a behavior module for unit tests / internal harnesses.
    #[cfg(any(test, feature = "internal"))]
    pub fn push_behavior_module_for_test(
        &mut self,
        behavior: Arc<Mutex<dyn crate::modules::BehaviorModuleInterface>>,
    ) {
        self.behaviors.push(behavior);
    }

    /// Attach radar-object data so Object::attemptDamage can fire
    /// TheRadar->tryUnderAttackEvent (C++ Object.cpp:1852 m_radarData != NULL).
    #[cfg(any(test, feature = "internal"))]
    pub fn set_radar_data_for_test(&mut self, data: Option<Arc<Mutex<RadarObject>>>) {
        self.radar_data = data;
    }

    /// C++ Object ctor always owns an ExperienceTracker. Test objects skip
    /// module install, so inherit-veterancy OCL tests attach one explicitly.
    #[cfg(any(test, feature = "internal"))]
    pub fn attach_experience_tracker_for_test(&mut self, trainable: bool) {
        let mut tracker = crate::experience::ExperienceTracker::new(self.id);
        tracker.set_trainable_override(trainable);
        self.experience_tracker = Some(Arc::new(Mutex::new(tracker)));
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        self.on_destroy();
    }
}
