//! The supported Object::onCapture modules run in effective INI behavior order.

use super::*;

impl GameLogic {
    pub(super) fn run_object_capture_modules(
        &mut self,
        id: ObjectId,
        old_team: Team,
        new_team: Team,
        owner_changed: bool,
        capture: Option<&ObjectCapture>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<bool> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(false);
        };
        let callbacks = object.behavior_callback_order();
        let is_current = |world: &GameLogic| {
            capture.is_none_or(|capture| {
                world.object_birth_matches(id, &capture.birth)
                    && world.objects.get(&id).is_some_and(|object| {
                        object.team_instance_id == Some(capture.destination)
                            && object.owner_player_id == Some(capture.new_owner)
                    })
            })
        };
        if !is_current(self) {
            return Ok(false);
        }

        let Some(capture) = capture.filter(|_| callbacks.is_declared()) else {
            self.on_capture_tunnel_network_residual_with_effects(id, old_team, new_team, effects);
            self.on_capture_kick_passengers_owned_with_effects(id, owner_changed, effects)?;
            if !is_current(self) {
                return Ok(false);
            }
            self.capture_authored_radar_provider_with_effects(id, effects);
            return Ok(is_current(self));
        };
        let mut cost_index = 0;
        let mut radar_index = 0;
        // Object.cpp:4519–4520. A containment module can synchronously change
        // a child's team and run its entire capture before the next parent slot.
        for module in callbacks.iter() {
            if !is_current(self) {
                return Ok(false);
            }
            let is = |class: &str| module.is(class);
            if is("CostModifierUpgrade") {
                if let Some(cost) = module.cost_modifier(cost_index) {
                    self.capture_cost_modifier_slot(
                        id,
                        &cost,
                        capture.old_owner,
                        Some(capture.new_owner),
                    );
                    cost_index += 1;
                }
            } else if is("PowerPlantUpgrade") || is("OverchargeBehavior") {
                if let Some(index) = callbacks.projection_index(&module) {
                    self.capture_plant_power_bonus_slot(
                        id, index, capture.old_owner, Some(capture.new_owner),
                    );
                }
            } else if is("RadarUpgrade") {
                self.capture_authored_radar_upgrade_slot(id, radar_index);
                radar_index += 1;
            } else if is("TunnelContain") {
                self.on_capture_tunnel_network_residual_with_effects(
                    id, old_team, new_team, effects,
                );
            } else if is("HelixContain") || is("OverlordContain") {
                self.capture_overlord_helix_payload_with_effects(id, effects)?;
            } else if [
                "TransportContain",
                "RailedTransportContain",
                "InternetHackContain",
                "RiderChangeContain",
            ]
            .into_iter()
            .any(is)
            {
                self.capture_passengers_owned_with_effects(id, owner_changed, false, effects);
            }
        }
        if !is_current(self) {
            return Ok(false);
        }
        if radar_index != 0
            && self
                .objects
                .get(&id)
                .is_some_and(|object| !object.is_disabled())
        {
            self.admit_host_radar_provider_with_effects(id, effects);
        }
        Ok(is_current(self))
    }
}
