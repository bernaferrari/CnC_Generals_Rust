//! Constructor callbacks run on the receiving lookup before linked registration.
use super::*;
use generals_sim::object::ConstructorLookupReceipt;
use generals_sim::{GameLogicError, GameLogicResult};

impl GameLogic {
    pub(super) fn verify_factory_birth(
        &self,
        id: ObjectId,
        birth: (u64, u64),
    ) -> GameLogicResult<()> {
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Err(GameLogicError::InvalidObject(id.0));
        }
        Ok(())
    }

    pub(super) fn finish_factory_constructor(
        &mut self,
        id: ObjectId,
        birth: (u64, u64),
        team: Team,
        owner_player_id: Option<u32>,
        receipt: &ConstructorLookupReceipt,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        let outcome = self
            .run_factory_constructor_callbacks(id, birth, team, owner_player_id, interpreter, ui)
            .and_then(|()| self.object_retirement_error().map_or(Ok(()), Err));
        let registration = match outcome {
            Ok(()) => self.objects.register_constructor_lookup(receipt),
            Err(_) if self.spawn_identity_matches(id, birth.0, birth.1) => {
                // Checked callers may retire the retained parent, including
                // an aborted map. Resolve this exact birth before handing it
                // back so traversal and reset can see it during cleanup.
                self.objects.retain_failed_constructor_lookup(receipt)
            }
            Err(error) => return Err(error),
        };
        registration.map_err(|reason| {
            GameLogicError::Configuration(format!("Object {id} constructor registration: {reason}"))
        })?;
        outcome
    }

    fn run_factory_constructor_callbacks(
        &mut self,
        id: ObjectId,
        birth: (u64, u64),
        team: Team,
        owner_player_id: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        // Object.cpp resolves the Team's attitude and attack priority before module callbacks.
        self.inherit_team_ai_defaults(id);
        // C++ Object.cpp:419-462 resolves the selected AIUpdateInterface,
        // assigns attitude/experience, then calls each module's
        // onObjectCreated before radar and GameLogic registration.  Rust
        // uses the receiving lookup while the constructor is pending.
        // Initialize the authored first AI provider before Team callbacks.
        self.initialize_base_ai_on_object_created_owned(id)?;
        self.verify_factory_birth(id, birth)?;
        // DemoTrapUpdate::onObjectCreated enables the Veteran weapon set
        // before selecting the authored mode slot.
        {
            let object = self
                .objects
                .get_mut(&id)
                .expect("constructor lookup object");
            if let Some((slots, proximity)) = object
                .mine_data
                .as_ref()
                .and_then(|data| Some((data.authored_demo_trap?, data.proximity_enabled)))
            {
                use generals_sim::weapon::WeaponSetType;
                object.set_weapon_condition_at_frame(
                    WeaponSetType::Veteran as u8,
                    true,
                    self.frame,
                );
                let _ = object.set_weapon_lock(
                    slots.initial_slot(proximity),
                    crate::game_logic::WeaponLockType::LockedTemporarily,
                );
            }
        }
        // C++ GameLogic map/create: team->setActive() once the team has members.
        self.admit_team_for_host_object_with_effects_checked(id, interpreter, false, ui)?;
        self.verify_factory_birth(id, birth)?;

        // C++ SpecialPowerModule ctor (SpecialPowerModule.cpp:86-101):
        // pre-built non-SharedNSync modules arm their authored reload at
        // creation; StartsPaused pauses once after arming.
        self.init_special_power_ctor_arms(id);

        // C++ GarrisonContain::onObjectCreated InitialRoster spawn.
        let factory_position = self
            .objects
            .get(&id)
            .expect("constructor lookup object")
            .get_position();
        self.apply_garrison_initial_roster_with_effects_checked(
            id,
            team,
            factory_position,
            interpreter,
            ui,
        )?;
        self.verify_factory_birth(id, birth)?;

        // HelixContain/OverlordContain::onObjectCreated creates each
        // PayloadTemplateName on the carrier's exact Team, in INI order.
        // Missing templates are ignored by the original factory lookup.
        let payloads = self.objects[&id]
            .thing()
            .template
            .contain_module
            .payload_template_names
            .clone();
        for payload_name in payloads {
            if !self.templates.contains_key(&payload_name)
                && !(self.receiving_module_content.is_some()
                    && self.ensure_host_spawn_template(&payload_name))
            {
                continue;
            }
            let (team_name, team_id) = {
                let parent = &self.objects[&id];
                (parent.team_instance_name.clone(), parent.team_instance_id)
            };
            let payload_team = team_id.map(|team_id| (team_name.as_str(), team_id, false));
            let Some(payload) = self.create_object_with_initial_team_and_ui_checked(
                &payload_name,
                team,
                owner_player_id,
                Vec3::ZERO,
                FactoryCreationPurpose::AtCreation,
                payload_team,
                interpreter,
                ui,
            )?
            else {
                continue;
            };
            self.verify_factory_birth(id, birth)?;
            // OverlordContain forwards both validation and insertion once
            // its first rider supplies a contain interface.
            let destination = self.redirected_containment_owner(id);
            let (attach_portable, admitted) = {
                let parent = &self.objects[&destination];
                let child = &self.objects[&payload];
                let portable = child.is_kind_of(KindOf::PortableStructure);
                let free_portable = parent.overlord_portable_occupant.is_none();
                let special_helix = parent.is_helix_transport && portable && free_portable;
                let used: usize = parent
                    .contained_unit_ids()
                    .iter()
                    .filter_map(|occupant| self.objects.get(occupant))
                    .map(|occupant| occupant.transport_slot_count())
                    .sum();
                let slots = child.transport_slot_count();
                let ordinary = slots > 0
                    && parent
                        .thing()
                        .template
                        .contain_module
                        .leftover_kind_masks_admit(child.kind_of_cpp_mask())
                    && used.saturating_add(slots) <= parent.transport_capacity();
                let attach_portable = portable
                    && free_portable
                    && (parent.is_helix_transport || parent.is_overlord_style_container());
                (attach_portable, special_helix || ordinary)
            };
            if !admitted {
                continue;
            }
            if attach_portable {
                self.attach_overlord_portable_with_effects(destination, payload, ui);
                self.verify_factory_birth(id, birth)?;
            } else if self
                .objects
                .get_mut(&destination)
                .is_some_and(|parent| parent.add_occupant(payload))
            {
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    payload,
                    Some(destination),
                    true,
                );
                let survived =
                    self.apply_owned_transport_contain_on_enter(destination, payload, ui);
                self.verify_factory_birth(id, birth)?;
                if survived {
                    self.stamp_player_who_entered(destination, payload);
                }
            }
        }

        // Object.cpp:473 adds the radar entry before registering the actor.
        // ThingFactory onCreate and partition admission follow registration.
        self.host_radar_add_object_with_effects(id, ui);
        self.verify_factory_birth(id, birth)?;
        Ok(())
    }
}
