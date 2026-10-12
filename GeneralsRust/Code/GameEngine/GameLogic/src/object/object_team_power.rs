//! CPP Object::setOrRestoreTeam837-855: release each domain loan at its callback.
use super::object_impl_imports::*;
use super::*;

impl Object {
    pub fn set_team_from_handle_with_power(
        source: &Arc<RwLock<Object>>,
        incoming: Option<Arc<RwLock<Team>>>,
        players: &crate::player::PlayerList,
        power: &mut dyn crate::player::PowerBrownoutServices,
    ) -> Result<(), String> {
        Self::set_or_restore_team_from_handle_with_power(source, incoming, false, players, power)
    }

    pub fn set_or_restore_team_from_handle_with_power(
        source: &Arc<RwLock<Object>>,
        incoming: Option<Arc<RwLock<Team>>>,
        restoring: bool,
        players: &crate::player::PlayerList,
        power: &mut dyn crate::player::PowerBrownoutServices,
    ) -> Result<(), String> {
        Self::set_or_restore_team_from_handle_with_power_and_tail(
            source, incoming, restoring, players, power, None,
        )
    }

    pub(super) fn set_or_restore_team_from_handle_with_power_and_tail(
        source: &Arc<RwLock<Object>>,
        incoming: Option<Arc<RwLock<Team>>>,
        restoring: bool,
        players: &crate::player::PlayerList,
        power: &mut dyn crate::player::PowerBrownoutServices,
        mut tail: Option<
            &mut dyn FnMut(
                &Arc<RwLock<Object>>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
                bool,
            ) -> Result<(), String>,
        >,
    ) -> Result<(), String> {
        power.validate_source(source)?;
        if let Some(team) = &incoming {
            power.validate_team(team)?;
        }
        let (old, old_id, old_controller, new_id, new_controller) = {
            let object = source.read().map_err(|_| "power team source poisoned")?;
            let old = object.get_team();
            let same = match (&old, &incoming) {
                (None, None) => true,
                (Some(old), Some(new)) => Arc::ptr_eq(old, new),
                _ => false,
            };
            if same {
                return Ok(());
            }
            let old_controller = old.as_ref().and_then(|team| {
                team.read()
                    .ok()
                    .and_then(|team| team.get_controlling_player_id())
            });
            let (new_id, new_controller) = incoming
                .as_ref()
                .map(|team| {
                    let team = team.read().map_err(|_| "power incoming team poisoned")?;
                    Ok::<_, String>((Some(team.get_id()), team.get_controlling_player_id()))
                })
                .transpose()?
                .unwrap_or((None, None));
            for controller in [old_controller, new_controller].into_iter().flatten() {
                power.validate_membership(controller as PlayerIndex, &object)?;
            }
            (
                old,
                object.get_team_id(),
                old_controller,
                new_id,
                new_controller,
            )
        };
        let id = source.read().map_err(|_| "power source poisoned")?.get_id();
        // Departure is observed before binding the incoming team.
        if let Some(old) = &old {
            let was_member = {
                let mut old = old.write().map_err(|_| "power departure team poisoned")?;
                let present = old.get_members().contains(&id);
                if present {
                    old.remove_member(id);
                }
                present
            };
            if let Some(controller) = old_controller.filter(|_| was_member) {
                let brownout = {
                    let mut object = source.write().map_err(|_| "power source poisoned")?;
                    let player = players
                        .get_player(controller as PlayerIndex)
                        .ok_or("power old controller absent")?;
                    let mut player = player.write().map_err(|_| "power old player poisoned")?;
                    if object.modules_ready && player.get_num_battle_plans_active() > 0 {
                        player.remove_battle_plan_bonuses_for_object(&mut object);
                    }
                    player.change_owned_membership_power(&object, false)
                };
                if let Some(brownout) = brownout {
                    power.apply_brownout_from_handle(
                        controller as PlayerIndex,
                        brownout,
                        source,
                    )?;
                }
                power.membership_idle_worker(
                    controller as PlayerIndex,
                    &*source.read().map_err(|_| "power source poisoned")?,
                    false,
                )?;
            }
        }
        power.validate_source(source)?;
        {
            let mut object = source.write().map_err(|_| "power source poisoned")?;
            object.team_id = new_id;
            object.team_pin = incoming.clone();
        }
        if let Some(incoming) = &incoming {
            let new_member = {
                let mut incoming = incoming
                    .write()
                    .map_err(|_| "power arrival team poisoned")?;
                let absent = !incoming.get_members().contains(&id);
                if absent {
                    incoming.add_member(id);
                }
                absent
            };
            if let Some(controller) = new_controller.filter(|_| new_member) {
                let brownout = {
                    let object = source.read().map_err(|_| "power source poisoned")?;
                    let player = players
                        .get_player(controller as PlayerIndex)
                        .ok_or("power new controller absent")?;
                    player
                        .write()
                        .map_err(|_| "power new player poisoned")?
                        .change_owned_membership_power(&object, true)
                };
                if let Some(brownout) = brownout {
                    power.apply_brownout_from_handle(
                        controller as PlayerIndex,
                        brownout,
                        source,
                    )?;
                }
                power.membership_idle_worker(
                    controller as PlayerIndex,
                    &*source.read().map_err(|_| "power source poisoned")?,
                    true,
                )?;
                let mut object = source.write().map_err(|_| "power source poisoned")?;
                let player = players
                    .get_player(controller as PlayerIndex)
                    .ok_or("power new controller absent")?;
                let mut player = player.write().map_err(|_| "power new player poisoned")?;
                if object.modules_ready && player.get_num_battle_plans_active() > 0 {
                    player.apply_battle_plan_bonuses_for_object(&mut object);
                }
            }
        }
        power.validate_source(source)?;
        if let Some(tail) = tail.as_mut() {
            // The original team callback observes the admitted owner without
            // holding its Object guard through a nested AI/UI service call.
            tail(
                source,
                old_id,
                new_id,
                old_controller,
                new_controller,
                restoring,
            )?;
        } else {
            let mut object = source.write().map_err(|_| "power source poisoned")?;
            object.notify_team_switch_side_effects(
                old_controller.map(|id| id as i32),
                new_controller.map(|id| id as i32),
            );
            object
                .finish_team_switch_with_services(
                    old_id,
                    new_id,
                    old_controller,
                    new_controller,
                    restoring,
                    Some(players),
                    None,
                    Some(power),
                )
                .map_err(|error| error.to_string())?;
        }
        if !restoring {
            let mut object = source.write().map_err(|_| "power source poisoned")?;
            object.original_team_name = incoming
                .as_ref()
                .and_then(|team| team.read().ok().map(|team| team.get_name().clone()))
                .unwrap_or_else(AsciiString::new);
        }
        Ok(())
    }
}
