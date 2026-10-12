//! Original processDestroyList: unindex first, then actual owner teardown.
use super::*;
use crate::scripting::engine::ScriptEngine;
use crate::scripting::events::NamedObjectTrackerState;

/// Match-owned script observations. The maps are values, not shared registries.
pub(crate) struct NativeDisposalState {
    pub(crate) scripts: ScriptEngine,
    pub(crate) names: NamedObjectTrackerState,
}
impl NativeDisposalState {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            scripts: ScriptEngine::new().map_err(|error| error.to_string())?,
            names: Default::default(),
        })
    }
}

impl AiNativeServices<'_> {
    fn reborrow(&mut self) -> AiNativeServices<'_> {
        AiNativeServices {
            groups: self.groups,
            disposal: self.disposal.as_deref_mut(),
            teams: self.teams,
            client_random: self.client_random,
            idle_worker_events: self.idle_worker_events,
            dirty_ui_objects: self.dirty_ui_objects,
            current_update: self.current_update.clone(),
            update_scheduler: self.update_scheduler,
            terrain: self.terrain,
            pathfinder: self.pathfinder,
            collision: self.collision,
            shroud: self.shroud,
            spatial_objects: self.spatial_objects,
            audio: self.audio,
            audio_locality: self.audio_locality,
            random: self.random,
            rules: self.rules,
            weapon_bonuses: self.weapon_bonuses,
            changed_trigger_objects: self.changed_trigger_objects,
            terrain_moved_objects: self.terrain_moved_objects,
            frame: self.frame,
            unlook_persist_frames: self.unlook_persist_frames,
            slow_death_scale: self.slow_death_scale,
            gravity: self.gravity,
            movement_penalty: self.movement_penalty,
        }
    }
}

/// The same live factory is mutably borrowed only at final deletion. Its
/// removed entry stays alive through the destructor; it is never reinserted.
pub(crate) fn finish_native_destroyed_objects(
    factory: &mut ObjectFactory,
    players: &PlayerList,
    mut services: AiNativeServices<'_>,
) -> Result<usize, String> {
    if services.current_update.is_some() {
        return Err("physical deletion must follow the update phase".into());
    }
    if services.disposal.is_none() {
        return Err("native deletion requires its script owner".into());
    }
    let mut retired = 0;
    while let Some(id) = services.update_scheduler.next_native_destroyed_object() {
        let owner = factory
            .get_object(id)
            .and_then(|slot| slot.get_base_object())
            .ok_or("queued native deletion object absent")?;
        {
            let world = BorrowedAiWorld::new(factory, players, services.reborrow())?;
            world.validate_owned_destroy(id)?;
        }
        services
            .spatial_objects
            .validate_native_retirement(id, &owner)?;
        // Resolve and admit destructor participants before the first unlink.
        // A held team/player loan cannot turn a partially retired object into
        // an apparently successful deletion.
        {
            let object = owner
                .try_write()
                .map_err(|_| "retiring object already borrowed")?;
            let controller = object.controlling_player_id_from_pin()?;
            if let Some(team) = object.get_team() {
                let team = team
                    .try_write()
                    .map_err(|_| "retiring team already borrowed")?;
                if team.has_member(id) {
                    if let Some(controller) = controller {
                        let player = players
                            .get_player(controller as Int)
                            .ok_or("retiring player absent")?;
                        let _guard = player
                            .try_write()
                            .map_err(|_| "retiring player already borrowed")?;
                    }
                }
            }
        }
        // CPP GameLogic.cpp2500-2506 removes all update registrations/list and
        // lookup before Object::~Object callbacks can observe that absence.
        services
            .update_scheduler
            .unlink_native_destroyed_object(id, &owner)?;
        let retiring = factory.take_native_retiring_object(id, &owner)?;
        let (team, controller, is_dozer, notify_count) = {
            let object = owner
                .read()
                .map_err(|_| "retiring native object poisoned")?;
            (
                object.get_team(),
                object.controlling_player_id_from_pin()?,
                object.is_kind_of(KindOf::Dozer),
                !object.is_kind_of(KindOf::Projectile) && !object.is_kind_of(KindOf::Inert),
            )
        };
        {
            let object = owner
                .read()
                .map_err(|_| "retiring native object poisoned")?;
            let positions = crate::ai::object_footprint_positions_with_height(&object, |x, y| {
                services.terrain.get_layer_height(
                    x,
                    y,
                    crate::path::PathfindLayerEnum::Ground,
                    None,
                    false,
                )
            })
            .unwrap_or_else(|| vec![*object.get_position()]);
            services
                .pathfinder
                .remove_object_from_map_at_positions(&positions);
            services.pathfinder.remove_wall_from_object(&object);
        }
        if notify_count {
            services
                .update_scheduler
                .notify_native_object_count_changed(services.frame);
            services
                .disposal
                .as_deref_mut()
                .expect("admitted deletion script owner")
                .scripts
                .set_frame_object_count_changed(services.frame);
        }
        // This admitted family has neither radar data nor a drawable. Original
        // sendObjectDestroyed therefore has no presentation binding to retire.
        if let Some(team) = team {
            let was_member = {
                let mut team = team.write().map_err(|_| "retiring native team poisoned")?;
                let present = team.has_member(id);
                team.remove_member(id);
                present
            };
            if let Some(controller) = controller.filter(|_| was_member) {
                let object = owner
                    .read()
                    .map_err(|_| "retiring native object poisoned")?;
                let mut player = players
                    .get_player(controller as Int)
                    .ok_or("retiring player absent")?
                    .write()
                    .map_err(|_| "retiring native player poisoned")?;
                // Preflight requires zero production/consumption, so the shared
                // membership algorithm cannot produce a missing brownout edge.
                let brownout = player.change_owned_membership_power(&object, false);
                debug_assert!(brownout.is_none());
                if is_dozer {
                    services.idle_worker_events.remove(id, controller as Int);
                }
            }
        }
        owner
            .write()
            .map_err(|_| "retiring native object poisoned")?
            .clear_native_team_pin();
        services
            .collision
            .unregister_object(id)
            .map_err(|error| error.to_string())?;
        services.spatial_objects.retire_native_object(id, &owner)?;
        owner
            .write()
            .map_err(|_| "retiring native object poisoned")?
            .finish_native_destroy_storage()?;
        let disposal = services
            .disposal
            .as_deref_mut()
            .expect("admitted deletion script owner");
        disposal
            .scripts
            .notify_of_object_destruction_owned(id, &mut disposal.names);
        drop(retiring);
        retired += 1;
    }
    Ok(retired)
}
