//! The original Build completion effects, after the scheduled UnitAI loan ends.
use super::*;
use crate::damage::DamageInfo;
use crate::modules::{AIUpdateInterface, OwnedDozerBuildCompletion};
use crate::player::Player;
use std::sync::Mutex;

enum ConstructionMine {
    Detonate(Arc<RwLock<Object>>, DamageInfo),
    Remove(ObjectID),
}

impl BorrowedAiWorld<'_> {
    pub(super) fn complete_native_dozer_build(
        &mut self,
        ai: &Arc<Mutex<dyn AIUpdateInterface>>,
        receipt: &OwnedDozerBuildCompletion,
    ) -> Result<(), String> {
        if receipt.frame != self.services.frame {
            return Err("construction completion belongs to another frame".into());
        }
        let builder = receipt
            .builder
            .upgrade()
            .ok_or("construction builder retired")?;
        let scaffold = receipt
            .scaffold
            .upgrade()
            .ok_or("construction scaffold retired")?;
        for (id, handle) in [
            (receipt.builder_id, &builder),
            (receipt.scaffold_id, &scaffold),
        ] {
            if !self
                .object(id)
                .is_some_and(|owned| Arc::ptr_eq(&owned, handle))
                || !self
                    .update_scheduler
                    .find_object_by_id(id)
                    .is_some_and(|owned| Arc::ptr_eq(&owned, handle))
            {
                return Err("construction completion no longer has its scheduled owners".into());
            }
        }
        let (mines, player) = {
            let scaffold = scaffold
                .try_read()
                .map_err(|_| "construction scaffold borrowed")?;
            if scaffold.get_builder_id() != receipt.builder_id
                || scaffold.get_construction_percent() < 100.0
                || scaffold.is_destroyed()
                || scaffold.is_effectively_dead()
            {
                return Err("construction completion has no live, completed scaffold claim".into());
            }
            let mines = if scaffold.test_status(ObjectStatusTypes::UnderConstruction) {
                self.prepare_construction_mines(&scaffold)?
            } else {
                Vec::new()
            };
            (mines, self.controller(&scaffold))
        };
        // DozerAIUpdate.cpp:540–545 / Object.cpp:985–1011. Mines see the
        // UnderConstruction bit cleared while the separate rebuild bit remains.
        let was_under_construction = scaffold
            .write()
            .map_err(|_| "construction scaffold poisoned")?
            .begin_owned_construction_completion();
        for mine in mines {
            match mine {
                ConstructionMine::Detonate(owner, mut damage) => {
                    self.attempt_damage_owned(&owner, &mut damage)?;
                }
                ConstructionMine::Remove(id) => self.destroy_object_owned(id)?,
            }
        }
        {
            let mut scaffold = scaffold
                .write()
                .map_err(|_| "construction scaffold poisoned")?;
            if was_under_construction {
                self.maintain_partition(&mut scaffold, false);
            }
            scaffold.clear_status(ObjectStatusTypes::Reconstructing.into());
        }
        ai.try_lock()
            .map_err(|_| "construction sound AI borrowed")?
            .finish_dozer_building_sound(self)?;

        // DozerAIUpdate.cpp:548–564. Damage visuals are evaluated only after
        // clearing the construction model conditions.
        {
            use crate::common::ModelConditionFlags as Model;
            let mut scaffold = scaffold
                .write()
                .map_err(|_| "construction scaffold poisoned")?;
            let _ = scaffold.clear_model_condition_flags(
                Model::AWAITING_CONSTRUCTION
                    | Model::PARTIALLY_CONSTRUCTED
                    | Model::ACTIVELY_BEING_CONSTRUCTED,
            );
            scaffold.set_construction_percent(crate::object::CONSTRUCTION_COMPLETE);
            if let Some(body) = scaffold.get_body_module() {
                let _ = body
                    .lock()
                    .map_err(|_| "construction body poisoned")?
                    .evaluate_visual_condition();
            }
        }
        if let Some(player) = player.as_ref() {
            self.notify_native_structure_completed(receipt, &scaffold, player)?;
            scaffold
                .write()
                .map_err(|_| "construction scaffold poisoned")?
                .on_build_complete();
        }
        {
            let mut scaffold = scaffold
                .write()
                .map_err(|_| "construction scaffold poisoned")?;
            self.maintain_partition(&mut scaffold, false);
            let frame = self.services.frame;
            if let Some(player) = self.controller(&scaffold) {
                let mask = player
                    .read()
                    .map_err(|_| "construction player poisoned")?
                    .get_completed_upgrade_mask();
                scaffold.apply_completed_construction_upgrades(
                    mask,
                    self.update_scheduler.is_loading_map(),
                    &mut |object, disable_proof| {
                        player
                            .write()
                            .expect("admitted construction player")
                            .add_radar(disable_proof);
                        object.extend_radar_at(frame);
                    },
                );
            }
        }
        self.announce_native_structure_completed(receipt, &builder, &scaffold)?;
        ai.try_lock()
            .map_err(|_| "construction End-dock AI borrowed")?
            .finish_owned_dozer_build_completion(receipt, self)
    }

    fn prepare_construction_mines(
        &self,
        scaffold: &Object,
    ) -> Result<Vec<ConstructionMine>, String> {
        let position = scaffold.get_position();
        let radius = scaffold
            .get_geometry_info()
            .get_bounding_circle_radius()
            .max(1.0);
        let center = crate::object::collide::Coord3D::new(position.x, position.y, position.z);
        let mut mines = Vec::new();
        for id in self
            .services
            .collision
            .query_objects_in_radius(&center, radius)
        {
            if id == scaffold.get_id() {
                continue;
            }
            let Some(owner) = self.object(id) else {
                continue;
            };
            let enemy = {
                let mine = owner.try_read().map_err(|_| "construction mine borrowed")?;
                if mine.is_destroyed() || !mine.is_kind_of(KindOf::Mine) {
                    continue;
                }
                scaffold.relationship_to_with_players(&mine, self.players) == Relationship::Enemies
            };
            if enemy {
                let damage = Self::forced_owned_kill_damage(&owner)?;
                self.validate_damage_owned(&owner, &damage)?;
                mines.push(ConstructionMine::Detonate(owner, damage));
            } else {
                self.validate_owned_destroy(id)?;
                mines.push(ConstructionMine::Remove(id));
            }
        }
        Ok(mines)
    }

    // Player.cpp:1639–1720. Keep the footprint and its synchronous fall
    // callbacks before the score, power, AI and presentation phases.
    fn notify_native_structure_completed(
        &mut self,
        receipt: &OwnedDozerBuildCompletion,
        scaffold: &Arc<RwLock<Object>>,
        player: &Arc<RwLock<Player>>,
    ) -> Result<(), String> {
        let frame = self.services.frame;
        let world = self.update_scheduler.owning_world_services();
        self.update_scheduler
            .notify_native_object_count_changed(frame);
        if let Some(disposal) = self.services.disposal.as_deref_mut() {
            disposal.scripts.set_frame_object_count_changed(frame);
        } else if let Some(world) = world.as_ref() {
            if let Some(scripts) = world
                .scripts()
                .engine()
                .read()
                .map_err(|_| "construction scripts poisoned")?
                .as_ref()
            {
                scripts.set_frame_object_count_changed(frame);
            }
        }
        self.refresh_native_construction_footprint(scaffold)?;
        let (index, power, disabled) = {
            let structure = scaffold
                .read()
                .map_err(|_| "construction scaffold poisoned")?;
            let mut player = player.write().map_err(|_| "construction player poisoned")?;
            player.score_structure_construction(
                &structure,
                receipt.is_rebuild,
                Some(self.update_scheduler.is_scoring_enabled()),
            );
            (
                player.get_player_index(),
                structure.get_template().get_energy_production(),
                structure.is_disabled(),
            )
        };
        let brownout = player
            .write()
            .map_err(|_| "construction player poisoned")?
            .apply_construction_power_influence(power, disabled);
        if let Some((index, enabled)) = brownout {
            // Resolve each object after the previous synchronous disable edge.
            for id in self
                .services
                .spatial_objects
                .get_objects_owned_by_player(index as u32)
            {
                let Some(object) = self.object(id) else {
                    continue;
                };
                let mut object = object
                    .write()
                    .map_err(|_| "powered construction object poisoned")?;
                if object.is_kind_of(KindOf::Powered) {
                    if enabled {
                        object.set_disabled(crate::common::DisabledType::DisabledUnderpowered);
                    } else {
                        object.clear_disabled(crate::common::DisabledType::DisabledUnderpowered);
                    }
                }
            }
        }
        if let Some(world) = world.as_ref() {
            if let Some(manager) = world
                .logic_managers()
                .ai_integration
                .write()
                .map_err(|_| "construction AI integration poisoned")?
                .as_mut()
            {
                let _ = manager.with_ai_player_mut(index as u32, |ai| {
                    ai.on_structure_produced(receipt.builder_id, receipt.scaffold_id)
                });
            }
        }
        self.services.dirty_ui_objects.push(receipt.scaffold_id);
        crate::control_bar::mark_ui_dirty_from(world.as_ref());
        if let Some(local) = self.players.get_local_player() {
            let events = {
                let structure = scaffold
                    .read()
                    .map_err(|_| "construction scaffold poisoned")?;
                let local = local
                    .read()
                    .map_err(|_| "construction local player poisoned")?;
                Player::completed_structure_eva(&structure, &local)
            };
            for event in events {
                let _ = crate::helpers::TheEva::set_should_play_from(world.as_ref(), event);
            }
        }
        Ok(())
    }

    fn announce_native_structure_completed(
        &mut self,
        receipt: &OwnedDozerBuildCompletion,
        builder: &Arc<RwLock<Object>>,
        scaffold: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let (world, message, mut voice, position) = {
            let builder = builder
                .read()
                .map_err(|_| "construction builder poisoned")?;
            let Some(controller) = builder.controlling_player_id_from_pin()? else {
                return Ok(());
            };
            if controller as Int != self.players.get_local_player_index() {
                return Ok(());
            }
            let scaffold = scaffold
                .read()
                .map_err(|_| "construction scaffold poisoned")?;
            let template = scaffold.get_template();
            let authored_name = template.get_display_name();
            let name = if authored_name.is_empty() {
                crate::helpers::TheGameText::fetch_for(&scaffold, "INI:MissingDisplayName")
                    .replace("%s", template.get_name().as_str())
            } else {
                authored_name.to_owned()
            };
            let format =
                crate::helpers::TheGameText::fetch_for(&builder, "DOZER:ConstructionComplete");
            let message = if format.contains("%s") {
                format.replace("%s", &name)
            } else {
                format!("{format} {name}")
            };
            (
                builder.owning_world_services(),
                message,
                builder.get_template().get_voice_task_complete(),
                *scaffold.get_position(),
            )
        };
        crate::helpers::TheInGameUI::display_message_from(world.as_ref(), &message);
        if !voice.get_event_name().is_empty() {
            voice.set_object_id(receipt.builder_id);
            self.add_audio_event(&voice);
        }
        use game_engine::common::system::radar::{Coord3D as RadarCoord, RadarEventType};
        self.services.radar.create_event(
            &RadarCoord {
                x: position.x,
                y: position.y,
                z: position.z,
            },
            RadarEventType::Construction,
            4.0,
        );
        Ok(())
    }
}
