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
        team: Option<TeamID>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::build(thing_template, INVALID_ID, object_status_mask, team)
    }

    /// Creates a new Object instance with a specified object ID.
    /// A valid id is moved into the object registry.
    pub fn new_with_id(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        team: Option<TeamID>,
    ) -> Result<ObjectID, Box<dyn std::error::Error + Send + Sync>> {
        let obj = Self::build(thing_template, object_id, object_status_mask, team)?;
        let id = obj.get_id();
        if id != INVALID_ID {
            OBJECT_REGISTRY.register_object(id, obj);
            register_legacy_object(id);
        }
        Ok(id)
    }

    fn build(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        team: Option<TeamID>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let obj = Self::new_raw(thing_template.clone(), object_id, object_status_mask, team);
        let handle = std::sync::Arc::new(std::sync::RwLock::new(obj));
        {
            let mut guard = handle
                .write()
                .map_err(|_| "object lock poisoned during initialization")?;
            guard.set_team(team)?;
        }
        if let Err(err) = Self::init_modules_for(&handle, &thing_template) {
            return Err(err);
        }
        std::sync::Arc::try_unwrap(handle)
            .map_err(|_| "object still shared after initialization")?
            .into_inner()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                e.to_string().into()
            })
    }

    /// Creates a raw Object instance (internal use)
    pub fn new_raw(
        thing_template: Arc<dyn ThingTemplate>,
        object_id: ObjectID,
        object_status_mask: ObjectStatusMaskType,
        _team: Option<TeamID>,
    ) -> Self {
        Self {
            id: object_id,
            producer_id: INVALID_ID,
            cached_main_turret_yaw: 0.0,
            cached_main_turret_pitch: 0.0,
            cached_main_turret_valid: false,
            ai_fire_attack_ok: true,
            ai_fire_turrets_linked: false,
            ai_fire_has_primary: false,
            ai_fire_has_secondary: false,
            ai_fire_primary_enabled: false,
            ai_fire_secondary_enabled: false,
            ai_fire_current_victim: None,
            ai_fire_original_victim_pos: None,
            ai_fire_last_command_source: crate::common::CommandSourceType::FromAi,
            ai_fire_mood_value: 0,
            ai_fire_pending_victim: None,
            ai_fire_which_turret: crate::common::TurretType::Invalid,
            ai_fire_primary_turn_rate: 0.0,
            ai_fire_secondary_turn_rate: 0.0,
            ai_pending_desired_speed: None,
            ai_fire_state_id: None,
            ai_fire_mood_target: None,
            ai_fire_ground_movement: false,
            ai_fire_can_turn_in_place: false,
            ai_fire_is_idle: false,
            ai_fire_ultra_accurate: false,
            ai_fire_next_mood_check: 0,
            ai_fire_idle_mood_adjust: 0,
            ai_fire_crate_id: crate::common::INVALID_ID,
            ai_fire_idle_attack_target: None,
            ai_pending_move_crate: None,
            ai_pending_attack_id: None,
            ai_pending_attack_move: None,
            ai_pending_attack_follow_waypoint: None,
            ai_pending_attack_follow_as_team: false,
            ai_pending_state_id: None,
            ai_pending_clear_guard_target: false,
            ai_pending_wake_path: false,
            ai_pending_clear_move_out: false,
            ai_fire_locomotor_speed: 0.0,
            ai_fire_blocked_and_stuck: false,
            ai_fire_has_path_destination: false,
            ai_fire_path_destination: None,
            ai_fire_loco_appearance: None,
            ai_pending_ending_move: false,
            ai_fire_is_moving: false,
            ai_fire_waypoint_queue_empty: true,
            ai_pending_completed_waypoint: None,
            ai_pending_precise_z: None,
            ai_pending_goal_path_index: None,
            ai_pending_busy: false,
            ai_fire_in_rappel: false,
            ai_pending_combat_drop: false,
            ai_pending_hack: false,
            ai_pending_hack_source: crate::common::CommandSourceType::FromAi,
            ai_pending_idle: false,
            ai_pending_idle_source: crate::common::CommandSourceType::FromAi,
            ai_pending_exit: None,
            ai_pending_exit_source: crate::common::CommandSourceType::FromAi,
            ai_pending_exit_obj: None,
            ai_pending_produced_exits: Vec::new(),
            ai_fire_hacking: false,
            ai_fire_hack_known: false,
            ai_fire_combat_drop: false,
            ai_fire_desired_speed: 0.0,
            ai_pending_rappel: false,
            ai_pending_follow_pos: None,
            ai_pending_heal: None,
            ai_pending_evacuate: false,
            ai_pending_rappel_obj: None,
            ai_pending_rappel_pos: None,
            ai_pending_combat_drop_obj: None,
            ai_pending_combat_drop_pos: None,
            ai_fire_has_path: false,
            ai_fire_waiting_for_path: false,
            ai_pending_path_goal: None,
            ai_pending_ignore_id: None,
            ai_pending_path_extra: None,
            ai_pending_attack_path: None,
            ai_pending_original_victim_pos: None,
            ai_pending_clear_victim: false,
            ai_pending_clear_goal: false,
            ai_pending_set_victim: None,
            ai_pending_path_through_units: None,
            ai_pending_allow_invalid_position: None,
            ai_pending_goal_id: None,
            ai_pending_reset_mood: false,
            ai_pending_victim_dead: false,
            ai_pending_destroy_path: false,
            ai_pending_clear_ignore: false,
            ai_pending_goal_orientation: None,
            ai_pending_goal_position: None,
            ai_pending_goal_none: false,
            ai_pending_turret_objects: Vec::new(),
            ai_pending_turret_positions: Vec::new(),
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
            sole_healing_benefactor_expiration_frame: 0,

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
        self.install_template_weapons();
        self.run_create_hooks(false);
        if self.firing_tracker.is_none()
            && !self.has_firing_tracker_module()
            && self.weapon_set.has_any_weapons()
        {
            self.firing_tracker = Some(Box::new(FiringTracker::new(self.id)));
        }

        self.init_object_cpp_sequence();

        Ok(())
    }

    fn install_template_weapons(&mut self) {
        if self.weapon_set.has_weapon_template_sets() {
            return;
        }
        for engine_set in self.thing_template.weapon_template_sets() {
            let Ok(set) = crate::weapon::WeaponTemplateSet::from_engine_set(engine_set, |name| {
                crate::weapon::with_weapon_store(|store| {
                    store.find_weapon_template(name.as_str()).cloned()
                })
                .ok()
                .flatten()
            }) else {
                continue;
            };
            self.weapon_set.add_weapon_template_set(set);
        }
        if self.weapon_set.has_weapon_template_sets() {
            let flags = self.cur_weapon_set_flags;
            let _ = self.weapon_set.update_weapon_set(self.id, &flags);
        }
    }

    /// Notify create modules that construction has completed.
    pub fn on_build_complete(&mut self) {
        self.run_create_hooks(true);
    }

    /// C++ `CreateModule::getObject()` is this `Object`, already mutably borrowed.
    /// Hooks receive it directly so they do not re-lock or stash a raw pointer.
    fn run_create_hooks(&mut self, build_complete: bool) {
        let modules = self.modules_with_interface(ModuleInterfaceType::CREATE);
        let object_id = self.id;
        for module in modules {
            // The contain binding's create hooks run against the Object-owned
            // contain field (CaveContain) with take/restore so no borrow is
            // held across the callback.
            let is_contain = module.with_module(|module| {
                crate::contain_module_overrides::is_contain_binding(module)
            });
            if is_contain {
                let mut contain = self.contain.take();
                if let Some(contain) = contain.as_mut() {
                    let run = if build_complete {
                        contain.should_do_on_build_complete()
                    } else {
                        true
                    };
                    if run {
                        let result = if build_complete {
                            contain.on_build_complete()
                        } else {
                            contain.on_create()
                        };
                        if let Err(err) = result {
                            warn!(
                                "CaveContain create hook failed for object {}: {}",
                                self.id, err
                            );
                        }
                    }
                }
                self.contain = contain;
                continue;
            }
            module.with_module(|module| {
                if let Some(create) = module.get_create_interface() {
                    if build_complete {
                        if create.should_do_on_build_complete() {
                            create.on_build_complete_with_owner(self);
                        }
                    } else {
                        create.on_create_with_owner(self);
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
    }

    /// Called during object destruction
    pub fn on_destroy(&mut self) {
        self.on_destroy_with_game_logic_services(|object_id, action| match action {
            ObjectDestroyServiceAction::UnregisterUpdateModule(update_module) => {
                let _ = crate::helpers::TheGameLogic::unregister_update_module(
                    object_id,
                    update_module,
                );
            }
            ObjectDestroyServiceAction::QueueTriggerAreaRefresh => {
                crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(object_id);
            }
        });
    }

    /// Run destruction using immediate services borrowed from the owning
    /// GameLogic. The named actions keep both effects on the same owner borrow
    /// and preserve their original call sites.
    pub(crate) fn on_destroy_with_game_logic_services(
        &mut self,
        mut service: impl FnMut(ObjectID, ObjectDestroyServiceAction),
    ) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        self.status.set_status(ObjectStatusTypes::Destroyed);

        let _ = crate::scripting::engine::get_named_object_tracker().unregister_object(self.id);

        for module in self.update_module_registrations.drain(..) {
            service(
                self.id,
                ObjectDestroyServiceAction::UnregisterUpdateModule(module),
            );
        }

        self.on_destroy_internal();
        self.run_destructor_tail_with_game_logic_service(&mut service);
    }

    /// C++ `Object::~Object` after `onDestroy`: pathfinder, scripts, radar,
    /// `sendObjectDestroyed`, clear team/group, ControlBar dirty.
    pub(crate) fn run_destructor_tail(&mut self) {
        self.run_destructor_tail_with_game_logic_service(&mut |object_id, action| {
            if let ObjectDestroyServiceAction::QueueTriggerAreaRefresh = action {
                crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(object_id);
            }
        });
    }

    fn run_destructor_tail_with_game_logic_service(
        &mut self,
        service: &mut impl FnMut(ObjectID, ObjectDestroyServiceAction),
    ) {
        let pos = *self.get_position();
        let footprint = crate::ai::object_footprint_positions(self).unwrap_or_else(|| vec![pos]);
        let ai_store = crate::ai::the_ai();
        if let Ok(ai) = ai_store.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(mut pf) = pf.write() {
                    pf.remove_object_from_map_at_positions(&footprint);
                    pf.remove_wall_from_object(self);
                }
            }
        }

        if !self.is_kind_of(KindOf::Projectile) && !self.is_kind_of(KindOf::Inert) {
            service(self.id, ObjectDestroyServiceAction::QueueTriggerAreaRefresh);
            crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();
        }

        if self.radar_data.is_some() {
            let radar = game_engine::common::system::radar::get_radar_system();
            if let Ok(mut radar_guard) = radar.write() {
                radar_guard.remove_object(self.id);
            }
            self.radar_data = None;
        }

        // C++ Object::~Object tail calls GameLogic::sendObjectDestroyed
        // (GameLogic.cpp:4134) as a plain virtual call — no global lock
        // exists in C++. The Rust global GameLogic mutex is held across the
        // ENTIRE update (game_logic_impl/globals.rs update_game_logic), so
        // objects destroyed by destroyObject / processDestroyList mid-update
        // can never take it here: the try_lock used to silently skip the
        // drawable/client unbind for exactly those objects. Mirror the
        // send_object_destroyed body directly instead — it touches only the
        // game-client bridge, never GameLogic state, so it is safe without
        // the lock.
        if let Ok(logic) = crate::system::game_logic::get_game_logic().try_lock() {
            logic.send_object_destroyed(self.id);
        } else if let Some(client) = crate::helpers::TheGameClient::get() {
            client.clear_object_model_draws(self.id);
            log::trace!("sendObjectDestroyed: obj={}", self.id);
        }

        let _ = self.set_team(None);

        if let Some(group_id) = self.get_group() {
            let _ = crate::ai::with_ai_group_mut(group_id, |group| {
                let _ = group.remove(self.id);
            });
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
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                container_id,
                |container| {
                    if let Some(contain_module) = container.get_contain_mut() {
                        let _ = contain_module.release_object(self.id);
                    }
                },
            );
            let _ = self.on_removed_from(container_id);
        }

        self.upgrade_module_handles.clear();

        let mut modules = std::mem::take(&mut self.modules);
        for entry in modules.drain(..) {
            // The contain binding's on_delete runs against the Object-owned
            // contain field at this entry's list position (take/restore).
            let is_contain = entry.with_module(|module| {
                crate::contain_module_overrides::is_contain_binding(module)
            });
            if is_contain {
                let mut contain = self.contain.take();
                if let Some(contain) = contain.as_mut() {
                    if let Err(err) = contain.on_delete() {
                        warn!(
                            "Contain module on_delete failed for object {}: {}",
                            self.id, err
                        );
                    }
                }
                self.contain = contain;
            }
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

    pub fn get_next_object(&self) -> Option<ObjectID> {
        // C++ Object::getNextObject (Object.h:155) returns m_next with no
        // dual-world gate. Registry lookup already falls back to GameLogic.
        self.next_object_id.filter(|object_id| {
            OBJECT_REGISTRY
                .with_object(*object_id, |_| ())
                .is_some()
        })
    }

    pub fn get_prev_object(&self) -> Option<ObjectID> {
        // C++ Object::getNextObject sibling: m_prev, no empty-world skip.
        self.prev_object_id.filter(|object_id| {
            OBJECT_REGISTRY
                .with_object(*object_id, |_| ())
                .is_some()
        })
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

    pub fn set_builder_id(&mut self, builder_id: ObjectID) {
        self.builder_id = builder_id;
    }

    // Team management. Factory-registered teams are an id. `team_pin` remains
    // only for a team that was never inserted into the factory.
    pub fn get_team(&self) -> Option<TeamID> {
        self.get_team_id()
    }

    pub fn get_team_id(&self) -> Option<TeamID> {
        if let Some(id) = self.team_id {
            return Some(id);
        }
        self.team_pin
            .as_ref()
            .and_then(|t| t.read().ok())
            .map(|g| g.get_id())
    }

    /// Point this object at a factory-owned team. Membership updates go through
    /// [`crate::team::with_team_mut`]; a same-id checkout (caller already holds
    /// that team) is a no-op for the member list.
    pub fn set_team_id(
        &mut self,
        team_id: Option<TeamID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_id = self.get_team_id();
        if old_id == team_id {
            return Ok(());
        }
        self.team_id = team_id;
        self.team_pin = None;
        let object_id = self.id;
        if let Some(old_id) = old_id {
            let _ = crate::team::with_team_mut(old_id, |team| team.remove_member(object_id));
        }
        if let Some(new_id) = team_id {
            let _ = crate::team::with_team_mut(new_id, |team| team.add_member(object_id));
        }
        Ok(())
    }

    pub fn set_team(
        &mut self,
        team: Option<TeamID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // C++ parity (Object::setTeam): if team owner is inactive, force neutral default team.
        let resolved_team = if let Some(team_id) = team {
            let controlling = crate::team::with_team(team_id, |team| {
                team.get_controlling_player_id()
            })
            .flatten();
            let owner_inactive = controlling
                .and_then(|player_id| {
                    crate::player::with_player(player_id as PlayerIndex, |player| {
                        !player.is_player_active()
                    })
                })
                .unwrap_or(false);

            if owner_inactive {
                let neutral_team = player_list().read().ok().and_then(|list| {
                    list.get_neutral_player()
                        .and_then(|neutral| neutral.get_default_team_id())
                });
                return self.set_team_id(neutral_team);
            }
            Some(team_id)
        } else {
            None
        };

        self.set_or_restore_team(resolved_team, false)?;
        self.original_team_name = {
            let team_id = self.get_team_id();
            team_id
                .and_then(|id| crate::team::with_team(id, |team| team.get_name().clone()))
                .unwrap_or_else(AsciiString::new)
        };
        Ok(())
    }

    pub fn set_temporary_team(
        &mut self,
        team: Option<TeamID>,
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
        let restored_team_id = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&original_name));

        let Some(restored_team_id) = restored_team_id else {
            log::warn!(
                "Object::restore_original_team failed to resolve original team '{}'",
                original_name
            );
            return Ok(());
        };

        let current_team_id = self.get_team_id();
        if current_team_id == Some(restored_team_id) {
            return Ok(());
        }

        self.set_team(Some(restored_team_id))?;

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
        team: Option<TeamID>,
        restoring: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let old_team_id = self.get_team_id();

        let old_player_id = old_team_id.and_then(|id| {
            crate::team::with_team(id, |team| team.get_controlling_player_id()).flatten()
        });
        let incoming_player_id = team.and_then(|team_id| {
            crate::team::with_team(team_id, |team| team.get_controlling_player_id()).flatten()
        });
        if old_player_id != incoming_player_id {
            self.adjust_power_for_player(false);
        }

        self.team_id = team;
        self.team_pin = None;

        let new_team_id = self.get_team_id();

        if old_team_id == new_team_id {
            return Ok(());
        }

        let new_player_id = new_team_id.and_then(|id| {
            crate::team::with_team(id, |team| team.get_controlling_player_id()).flatten()
        });

        if old_player_id != new_player_id {
            if let Some(old_id) = old_player_id {
                let _ = crate::player::with_player_mut(old_id as PlayerIndex, |player| {
                    if self.modules_ready && player.get_num_battle_plans_active() > 0 {
                        player.remove_battle_plan_bonuses_for_object(self);
                    }
                    player.remove_owned_object_for_object(self);
                });
            }
            if let Some(new_id) = new_player_id {
                let _ = crate::player::with_player_mut(new_id as PlayerIndex, |player| {
                    player.add_owned_object_for_object(self);
                    if self.modules_ready && player.get_num_battle_plans_active() > 0 {
                        player.apply_battle_plan_bonuses_for_object(self);
                    }
                });
            }
            self.adjust_power_for_player(true);
            self.notify_team_switch_side_effects(
                old_player_id.map(|id| id as i32),
                new_player_id.map(|id| id as i32),
            );
        }

        if old_team_id != new_team_id {
            if let Some(old_id) = old_team_id {
                let _ = crate::team::with_team_mut(old_id, |team| team.remove_member(self.id));
            }
            if let Some(new_id) = new_team_id {
                let _ = crate::team::with_team_mut(new_id, |team| team.add_member(self.id));
            }
        }

        if old_team_id.is_some() && new_team_id.is_some() && !restoring {
            self.on_capture(
                old_player_id.map(|id| id as PlayerIndex),
                new_player_id.map(|id| id as PlayerIndex),
            );
        }

        if !restoring {
            if let Some(new_id) = new_player_id {
                self.award_initial_capture_bonus_if_needed(Some(new_id as PlayerIndex));
            }
        }

        self.refresh_radar_object_from_state();
        self.apply_team_ai_profile();
        self.update_drawable_team_visuals();
        Ok(())
    }

    pub(super) fn apply_team_ai_profile(&mut self) {
        let team_name = self.get_team().and_then(|team_id| {
            crate::team::with_team(team_id, |team| team.get_name().to_string())
        });

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

        if let Some(ai) = self.get_ai_update_interface_mut() {
            let _ = ai.set_attitude(attitude);
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
            if let Some(damage) = damage_info {
                // C++ Object::onDie walks m_behaviors with the object in hand;
                // dispatch_on_die passes this object explicitly so no module
                // re-enters the registry for its own checked-out id.
                crate::object::game_module::dispatch_on_die(&module_entry, self, damage);
            }
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
            if let Some(info) = damage_info {
                info.output.killed_target = true;
            }
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
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            self.has_died_already = true;
        }

        // FIRST, call our die modules
        log::debug!("Object {} calling die modules", self.id);
        self.call_on_die_hooks(Some(damage_info));

        let mut contain = self.contain.take();
        if let Some(contain) = contain.as_mut() {
            if let Err(err) = contain.on_die_with_owner(self, Some(damage_info)) {
                log::warn!("Object {} contain on_die failed: {}", self.id, err);
            }
        }
        self.contain = contain;

        self.on_die_remove_from_radar();

        // Just in case I have been sporting one of those fancy Terrain Decals,
        // I naturally lose it now, because I'm dead.
        self.on_die_fade_terrain_decal();

        // Objects that were spawned from something need to tell their spawner
        // that they have died (C++ Object::onDie spawner block).
        if self.producer_id != INVALID_ID {
            let mut spawn_damage = damage_info.clone();
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                self.producer_id,
                |spawner| {
                    let _ = spawner.with_spawn_behavior_full_interface(|spawn_behavior| {
                        let _ = spawn_behavior.on_spawn_death(self.id, &mut spawn_damage);
                    });
                },
            );
        }

        // Handle partition cell maintenance
        self.handle_partition_cell_maintenance();

        // Notify team of object death. The script stays queued for the normal
        // flush so a busy script engine does not drop every pending team script.
        if let Some(team_id) = self.get_team() {
            let _ = crate::team::with_team_mut(team_id, |team| {
                team.notify_team_of_object_death();
            });
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

        if let Some(player) = self.get_controlling_player() {
            let _ = crate::player::with_player(player, |p| {
                crate::helpers::TheInGameUI::remove_idle_worker(
                    self,
                    p.get_player_index() as Int,
                );
            });
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
        let body: Box<dyn crate::object::body::body_module::BodyModuleInterface> = Box::new(
            crate::object::body::active_body::ActiveBody::new_with_owner(
                module_data,
                obj.get_id(),
            ),
        );
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
        let body: Box<dyn crate::object::body::body_module::BodyModuleInterface> = Box::new(
            crate::object::body::active_body::ActiveBody::new_with_owner(
                module_data,
                obj.get_id(),
            ),
        );
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
        behavior: Box<dyn crate::modules::BehaviorModuleInterface>,
    ) {
        self.behaviors.push(behavior);
    }

    /// Attach radar-object data so Object::attemptDamage can fire
    /// TheRadar->tryUnderAttackEvent (C++ Object.cpp:1852 m_radarData != NULL).
    #[cfg(any(test, feature = "internal"))]
    pub fn set_radar_data_for_test(&mut self, data: Option<Box<RadarObject>>) {
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

#[cfg(test)]
mod team_membership_borrow_tests {
    use super::*;
    use crate::common::DefaultThingTemplate;
    use crate::object::registry::{OBJECT_REGISTRY, test_isolation_lock};
    use crate::player::{Player, PlayerTemplate, player_list};
    use std::sync::{Mutex, OnceLock};
    struct RestorePlayers {
        players: Vec<Player>,
        local_player_index: i32,
    }

    impl RestorePlayers {
        fn replace(players: Vec<Player>) -> Self {
            let mut list = player_list().write().unwrap();
            let previous = Self {
                players: list.take_players(),
                local_player_index: list.get_local_player_index(),
            };
            list.clear();
            for player in players {
                list.add_player(player);
            }
            list.set_local_player_index(0);
            previous
        }
    }

    impl Drop for RestorePlayers {
        fn drop(&mut self) {
            let mut list = player_list()
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            list.clear();
            for player in self.players.drain(..) {
                list.add_player(player);
            }
            list.set_local_player_index(self.local_player_index);
        }
    }

    fn make_playable_player(index: i32, name: &str) -> Player {
        let mut player = Player::new(index);
        let mut template = PlayerTemplate::new(name.to_string());
        template.playable = true;
        player.init(Arc::new(template));
        player
    }

    fn team_for_player(name: &str, player_index: u32) -> TeamID {
        let mut factory = crate::team::get_team_factory()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = factory.init_team(name.into(), AsciiString::new(), false, None);
        let team_id = factory.create_team(name).expect("factory team");
        drop(factory);
        let _ = crate::team::with_team_mut(team_id, |team| {
            team.set_controlling_player_id(Some(player_index));
        });
        team_id
    }

    #[test]
    fn registered_object_team_reassignment_keeps_owned_ids_and_power_balanced() {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _state_guard = TEST_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _registry_guard = test_isolation_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        OBJECT_REGISTRY.clear();

        let first = make_playable_player(0, "OwnershipFirst");
        let second = make_playable_player(1, "OwnershipSecond");
        let _restore_players = RestorePlayers::replace(vec![first, second]);
        let first_team = team_for_player("OwnershipTeam96101", 0);
        let second_team = team_for_player("OwnershipTeam96102", 1);

        let first_power_before =
            crate::player::with_player(0, |player| player.get_energy().production()).unwrap();
        let second_power_before =
            crate::player::with_player(1, |player| player.get_energy().production()).unwrap();
        let mut template = DefaultThingTemplate::new("PowerPlantMembershipTest".to_string());
        template.set_energy_production(10);
        template.add_kind_of(crate::common::KindOf::Structure);
        let object_id = 96_103;
        let mut object = Object::new_test_from_template(object_id, 100.0, Arc::new(template));

        object
            .set_team(Some(first_team))
            .expect("assign first playable team");
        assert_eq!(
            crate::player::with_player(0, |player| player.get_all_objects()).unwrap(),
            vec![object_id]
        );
        assert!(
            crate::player::with_player(1, |player| player.get_all_objects())
                .unwrap()
                .is_empty()
        );
        let first_power_after_add =
            crate::player::with_player(0, |player| player.get_energy().production()).unwrap();
        let first_power_delta = first_power_after_add - first_power_before;
        assert!(
            first_power_delta > 0,
            "first owner should receive plant power"
        );

        object
            .set_team(Some(second_team))
            .expect("reassign second playable team");
        assert!(
            crate::player::with_player(0, |player| player.get_all_objects())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            crate::player::with_player(1, |player| player.get_all_objects()).unwrap(),
            vec![object_id]
        );
        let second_power_after_add =
            crate::player::with_player(1, |player| player.get_energy().production()).unwrap();
        assert_eq!(
            second_power_after_add - second_power_before,
            first_power_delta,
            "reassignment must transfer the same production delta"
        );

        object.set_team(None).expect("remove final playable team");
        assert!(
            crate::player::with_player(1, |player| player.get_all_objects())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            crate::player::with_player(1, |player| player.get_energy().production()).unwrap(),
            second_power_before,
            "removing ownership must restore the prior power production"
        );

        OBJECT_REGISTRY.clear();
    }
}
