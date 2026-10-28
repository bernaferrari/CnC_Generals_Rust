//! Synchronous receiving named ScriptActions; standalone Core fallback lives in Core.
use super::*;

impl<'a> HostScriptExecutionDriver<'a> {
    pub(super) fn receive_named_attack_priority(
        &mut self,
        name: &str,
        set: &str,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        let interpreter = self
            .interpreter
            .expect("live priority action retains its interpreter");
        let info = interpreter.get_attack_info(set);
        let resolved = info
            .as_ref()
            .map(|info| info.get_name().to_string())
            .filter(|name| !name.is_empty());
        if let Some(name) = resolved.as_deref() {
            interpreter.set_object_attack_priority_set(id.0, name);
        } else {
            interpreter.clear_object_attack_priority_set(id.0);
        }
        self.world
            .set_unit_attack_priority_set(id, resolved.as_deref());
        if let Some(info) = info.filter(|_| resolved.is_some()) {
            let mut receiving = AttackPriorityInfo::new(info.get_name());
            receiving.default_priority = info.default_priority;
            for (template, priority) in &info.priority_map {
                receiving.set_priority_template(template, *priority);
            }
            self.world.register_attack_priority_set(receiving);
        }
        Some(Ok(()))
    }

    pub(super) fn receive_named_evac_disposition(
        &mut self,
        name: &str,
        disposition: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        {
            if let Some(object) = self.world.host_object_mut(id) {
                object.set_garrison_evac_disposition(disposition.max(0) as u8);
            }
        }
        Some(Ok(()))
    }

    pub(super) fn receive_named_panel_flag(
        &mut self,
        name: &str,
        flag: &str,
        enabled: bool,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        {
            if panel_flag_is_indestructible(flag) {
                self.world.set_object_indestructible(id, enabled);
            } else if let Some(object) = self.world.objects.get_mut(&id) {
                object.apply_object_panel_flag_with_partition_owner(
                    flag,
                    enabled,
                    &mut self.world.partition_manager,
                );
            }
        }
        Some(Ok(()))
    }

    pub(super) fn receive_named_warehouse_value(
        &mut self,
        name: &str,
        cash: i32,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        if let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        {
            self.world.apply_warehouse_set_value_to_object(id, cash);
        }
        Some(Ok(()))
    }

    pub(super) fn receive_named_runtime(
        &mut self,
        request: gamelogic::scripting::engine::ScriptNamedRuntimeRequest<'_>,
        this_object: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        use gamelogic::scripting::engine::ScriptNamedRuntimeRequest as Request;
        let name = match request {
            Request::Held { unit, .. }
            | Request::Repulsor { unit, .. }
            | Request::Emoticon { unit, .. }
            | Request::StoppingDistance { unit, .. }
            | Request::FaceWaypoint { unit, .. }
            | Request::Boobytrap { unit, .. }
            | Request::TransferOwnership { unit, .. }
            | Request::FireWeaponPath { unit, .. } => unit,
        };
        let Some(id) = self
            .world
            .owned_named_script_object(name, this_object.map(ObjectId))
        else {
            return Some(Ok(()));
        };
        match request {
            Request::Held { held, .. } => {
                self.world
                    .with_owned_radar_disabled_operation(id, |object| {
                        object.set_status_disabled_held(held)
                    });
            }
            Request::Repulsor { enabled, .. } => {
                if let Some(object) = self.world.host_object_mut(id) {
                    object.repulsor_until_frame = 0;
                    object.set_status_repulsor(enabled);
                }
            }
            Request::Emoticon { name, frames, .. } => {
                if let Some(object) = self.world.host_object_mut(id) {
                    object.set_emoticon(name, frames);
                }
            }
            Request::StoppingDistance { distance, .. } => {
                if !(distance < 0.5)
                    && self
                        .world
                        .host_object(id)
                        .is_some_and(GameLogic::host_script_member_has_cur_locomotor)
                {
                    self.world.host_object_mut(id).unwrap().close_enough_dist = Some(distance);
                }
            }
            Request::FaceWaypoint { waypoint, .. } => {
                if let Some(position) = self.world.host_script_waypoint_position(waypoint) {
                    self.world
                        .host_script_face_unit_with_effects(id, position, &mut self.ui);
                }
            }
            Request::Boobytrap { template, .. } => {
                self.world.host_script_plant_boobytrap_with_effects(
                    template,
                    id,
                    self.interpreter,
                    &mut self.ui,
                );
            }
            Request::FireWeaponPath { path, .. } => {
                self.world
                    .host_script_fire_weapon_following_path_for(id, path, &mut self.ui);
            }
            Request::TransferOwnership { player, .. } => {
                let current = self
                    .interpreter
                    .and_then(|engine| engine.get_current_player_name());
                if let Some(player) = self.script_player_id(player, current.as_deref()) {
                    let birth = self.world.host_object(id).map(|object| {
                        std::sync::Arc::downgrade(&object.visual_draw_identity_token)
                    });
                    if self
                        .world
                        .transfer_object_to_player_with_effects(id, player, &mut self.ui)
                        && birth
                            .as_ref()
                            .is_some_and(|birth| self.world.object_birth_matches(id, birth))
                    {
                        self.world.apply_researched_player_upgrades_to_object_with_interpreter(
                            id,
                            self.interpreter,
                            &mut self.ui,
                        );
                    }
                }
            }
        }
        Some(Ok(()))
    }
}
